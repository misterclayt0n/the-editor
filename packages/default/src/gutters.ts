import { relative, sep } from "node:path";
import type { GutterRow, SurfaceId, VcsHunk } from "@the-editor/contracts/runtime";
import type { EditorModule, GutterLine } from "@the-editor/editor";

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
        const cached = vcsHunkCache.get(key);
        if (!cached || cached.revision !== ctx.revision) {
          scheduleVcsHunkRefresh(editor, key, root, relPath, ctx.activeSurfaceId, ctx.revision);
        }
        return gutterLinesForHunks(ctx.gutterRows, cached?.hunks ?? []);
      },
    });
  };
}

interface VcsHunkCacheEntry {
  revision: number;
  hunks: VcsHunk[];
  inFlight?: boolean;
}

const vcsHunkCache = new Map<string, VcsHunkCacheEntry>();
const vcsHunkJobs = new Map<string, { root: string; relPath: string; latestSurfaceId: SurfaceId; latestRevision: number; running: boolean }>();

function scheduleVcsHunkRefresh(
  editor: Parameters<EditorModule>[0],
  key: string,
  root: string,
  relPath: string,
  surfaceId: SurfaceId,
  revision: number,
): void {
  const current = vcsHunkCache.get(key);
  if (current?.revision === revision && !current.inFlight) return;
  const job = vcsHunkJobs.get(key);
  if (job) {
    job.root = root;
    job.relPath = relPath;
    job.latestSurfaceId = surfaceId;
    job.latestRevision = revision;
    return;
  }
  const nextJob = { root, relPath, latestSurfaceId: surfaceId, latestRevision: revision, running: false };
  vcsHunkJobs.set(key, nextJob);
  queueMicrotask(() => void runVcsHunkJob(editor, key));
}

async function runVcsHunkJob(editor: Parameters<EditorModule>[0], key: string): Promise<void> {
  const job = vcsHunkJobs.get(key);
  if (!job || job.running) return;
  job.running = true;
  try {
    while (true) {
      const surfaceId = job.latestSurfaceId;
      const revision = job.latestRevision;
      vcsHunkCache.set(key, { revision, hunks: vcsHunkCache.get(key)?.hunks ?? [], inFlight: true });
      const { hunks } = await editor.runtime.vcs.queryHunks({ surfaceId, rootPath: job.root, relativePath: job.relPath });
      if (job.latestRevision !== revision || job.latestSurfaceId !== surfaceId) continue;
      vcsHunkCache.set(key, { revision, hunks });
      editor.workbench.invalidateSurface(surfaceId, "unknown");
      break;
    }
  } catch {
    vcsHunkCache.delete(key);
  } finally {
    vcsHunkJobs.delete(key);
  }
}

function gutterLinesForHunks(rows: readonly GutterRow[], hunks: readonly VcsHunk[]): GutterLine[] {
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

function vcsColor(kind: VcsHunk["kind"]): string {
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
