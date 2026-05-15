import { execFile } from "node:child_process";
import { relative, sep } from "node:path";
import { promisify } from "node:util";
import type { GutterRow, SurfaceId } from "@the-editor/contracts/runtime";
import type { EditorModule, GutterLine } from "@the-editor/editor";

const execFileAsync = promisify(execFile);

export function defaultGutters(): EditorModule {
  return (editor) => {
    editor.gutters.registerProvider({
      id: "default.vcsChanges",
      title: "VCS Changes",
      lane: "vcs",
      laneSpec: {
        id: "vcs",
        order: 20,
        widthPx: 7,
        paddingLeftPx: 2,
        paddingRightPx: 2,
      },
      provide(ctx) {
        if (!ctx.activePath || !ctx.gutterRows?.length || ctx.activeSurfaceId === undefined || ctx.revision === undefined) return [];
        const root = editor.cwd.current();
        const relPath = normalizePath(relative(root, ctx.activePath));
        if (!relPath || relPath.startsWith("..")) return [];
        const key = `${root}\0${relPath}`;
        const base = vcsBaseCache.get(key);
        if (!base) {
          scheduleVcsBaseLoad(editor, root, relPath, ctx.activeSurfaceId);
          return [];
        }
        if (base.inFlight || base.text === null) return [];
        const cached = vcsDiffCache.get(key);
        if (!cached || cached.baseText !== base.text || cached.revision !== ctx.revision) {
          scheduleVcsDiff(editor, key, base.text, ctx.activeSurfaceId, ctx.revision);
        }
        return gutterLinesForHunks(ctx.gutterRows, cached?.hunks ?? []);
      },
    });

  };
}

interface DiffHunk {
  start: number;
  count: number;
  kind: "added" | "modified" | "deleted";
}

interface VcsBaseCacheEntry {
  text: string | null;
  inFlight: boolean;
}

interface VcsDiffCacheEntry {
  baseText: string;
  revision: number;
  hunks: DiffHunk[];
  inFlight?: boolean;
}

const vcsBaseCache = new Map<string, VcsBaseCacheEntry>();
const vcsDiffCache = new Map<string, VcsDiffCacheEntry>();
const vcsDiffJobs = new Map<string, { latestSurfaceId: SurfaceId; latestRevision: number; running: boolean }>();

function scheduleVcsBaseLoad(editor: Parameters<EditorModule>[0], root: string, relPath: string, surfaceId: SurfaceId | undefined): void {
  const key = `${root}\0${relPath}`;
  const current = vcsBaseCache.get(key);
  if (current?.inFlight) return;
  vcsBaseCache.set(key, { text: current?.text ?? null, inFlight: true });
  void loadVcsBase(root, relPath).then((text) => {
    vcsBaseCache.set(key, { text, inFlight: false });
    vcsDiffCache.delete(key);
    if (surfaceId !== undefined) editor.workbench.invalidateSurface(surfaceId, "unknown");
  }).catch(() => {
    vcsBaseCache.set(key, { text: null, inFlight: false });
  });
}

function scheduleVcsDiff(editor: Parameters<EditorModule>[0], key: string, baseText: string, surfaceId: SurfaceId, revision: number): void {
  const current = vcsDiffCache.get(key);
  if (current?.revision === revision && current.baseText === baseText && !current.inFlight) return;
  const job = vcsDiffJobs.get(key);
  if (job) {
    job.latestSurfaceId = surfaceId;
    job.latestRevision = revision;
    return;
  }

  const nextJob = { latestSurfaceId: surfaceId, latestRevision: revision, running: false };
  vcsDiffJobs.set(key, nextJob);
  queueMicrotask(() => void runVcsDiffJob(editor, key, baseText));
}

async function runVcsDiffJob(editor: Parameters<EditorModule>[0], key: string, baseText: string): Promise<void> {
  const job = vcsDiffJobs.get(key);
  if (!job || job.running) return;
  job.running = true;
  try {
    while (true) {
      const surfaceId = job.latestSurfaceId;
      const revision = job.latestRevision;
      vcsDiffCache.set(key, { baseText, revision, hunks: vcsDiffCache.get(key)?.hunks ?? [], inFlight: true });
      const { text } = await editor.runtime.surfaces.readText({ surfaceId });
      if (job.latestRevision !== revision || job.latestSurfaceId !== surfaceId) continue;
      const hunks = diffLines(baseText, text);
      vcsDiffCache.set(key, { baseText, revision, hunks });
      editor.workbench.invalidateSurface(surfaceId, "unknown");
      break;
    }
  } catch {
    vcsDiffCache.delete(key);
  } finally {
    vcsDiffJobs.delete(key);
  }
}

async function loadVcsBase(root: string, relPath: string): Promise<string | null> {
  try {
    const { stdout } = await execFileAsync("jj", ["file", "show", "--revision", "@-", relPath], { cwd: root, maxBuffer: 8 * 1024 * 1024 });
    return stdout.toString();
  } catch {
    // Fall through to git for non-jj workspaces.
  }
  try {
    const { stdout } = await execFileAsync("git", ["-C", root, "show", `HEAD:${relPath}`], { maxBuffer: 8 * 1024 * 1024 });
    return stdout.toString();
  } catch {
    return null;
  }
}

function diffLines(baseText: string, currentText: string): DiffHunk[] {
  if (baseText === currentText) return [];
  const oldLines = splitLines(baseText);
  const newLines = splitLines(currentText);
  const n = oldLines.length;
  const m = newLines.length;
  const dp = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i -= 1) {
    for (let j = m - 1; j >= 0; j -= 1) {
      dp[i]![j] = oldLines[i] === newLines[j]
        ? dp[i + 1]![j + 1]! + 1
        : Math.max(dp[i + 1]![j]!, dp[i]![j + 1]!);
    }
  }

  const hunks: DiffHunk[] = [];
  let i = 0;
  let j = 0;
  let pending: { oldCount: number; newStart: number; newCount: number } | null = null;
  const flush = () => {
    if (!pending) return;
    const kind = pending.newCount === 0 ? "deleted" : pending.oldCount === 0 ? "added" : "modified";
    hunks.push({ start: Math.max(1, pending.newStart), count: Math.max(1, pending.newCount), kind });
    pending = null;
  };
  const addChange = (oldDelta: number, newDelta: number) => {
    pending ??= { oldCount: 0, newStart: j + 1, newCount: 0 };
    pending.oldCount += oldDelta;
    pending.newCount += newDelta;
  };

  while (i < n || j < m) {
    if (i < n && j < m && oldLines[i] === newLines[j]) {
      flush();
      i += 1;
      j += 1;
    } else if (j < m && (i === n || dp[i]![j + 1]! >= dp[i + 1]![j]!)) {
      addChange(0, 1);
      j += 1;
    } else {
      addChange(1, 0);
      i += 1;
    }
  }
  flush();
  return hunks;
}

function splitLines(text: string): string[] {
  if (!text) return [];
  const normalized = text.endsWith("\n") ? text.slice(0, -1) : text;
  return normalized.length ? normalized.split(/\r?\n/) : [];
}

function gutterLinesForHunks(rows: readonly GutterRow[], hunks: readonly DiffHunk[]): GutterLine[] {
  const lines: GutterLine[] = [];
  for (const hunk of hunks) {
    let startRow: number | undefined;
    let endRow: number | undefined;
    for (const row of rows) {
      if (row.lineNumber === null || !row.startsLogicalLine) continue;
      if (row.lineNumber < hunk.start || row.lineNumber >= hunk.start + hunk.count) continue;
      startRow ??= row.surfaceRow;
      endRow = row.surfaceRow;
    }
    if (startRow === undefined || endRow === undefined) continue;
    lines.push({
      id: `vcs:${hunk.kind}:${hunk.start}`,
      surfaceRow: startRow,
      surfaceRowEnd: endRow,
      kind: "bar",
      color: vcsColor(hunk.kind),
      title: hunk.kind === "added" ? "Added lines" : hunk.kind === "deleted" ? "Deleted lines" : "Modified lines",
    });
  }
  return lines;
}

function vcsColor(kind: DiffHunk["kind"]): string {
  switch (kind) {
    case "added":
      return "#3fb950";
    case "deleted":
      return "#f85149";
    case "modified":
      return "#d29922";
  }
}

function normalizePath(path: string): string {
  return sep === "/" ? path : path.split(sep).join("/");
}
