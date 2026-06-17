import * as path from "node:path";
import * as fs from "node:fs";
import * as fsp from "node:fs/promises";
import { spawn, type ChildProcess } from "node:child_process";
import { createRequire } from "node:module";
import { app, BrowserWindow, dialog, ipcMain, nativeTheme } from "electron";

const devServerUrl = process.env.VITE_DEV_SERVER_URL?.trim();
/** Native modules (node-pty) are externalized; require them at runtime. */
const nodeRequire = createRequire(import.meta.url);

type FffResult<T> = { readonly ok: true; readonly value: T } | { readonly ok: false; readonly error: string };
type FffLocation =
  | { readonly type: "line"; readonly line: number }
  | { readonly type: "position"; readonly line: number; readonly col: number }
  | {
      readonly type: "range";
      readonly start: { readonly line: number; readonly col: number };
      readonly end: { readonly line: number; readonly col: number };
    };
type FffFileItem = {
  readonly relativePath: string;
  readonly fileName: string;
  readonly size: number;
  readonly gitStatus: string;
};
type FffSearchResult = {
  readonly items: readonly FffFileItem[];
  readonly totalMatched: number;
  readonly totalFiles: number;
  readonly location?: FffLocation;
};
type FffGrepMode = "plain" | "regex" | "fuzzy";
type FffGrepCursor = { readonly __brand: "GrepCursor"; readonly _offset: number };
type FffGrepMatch = FffFileItem & {
  readonly isBinary: boolean;
  readonly totalFrecencyScore: number;
  readonly accessFrecencyScore: number;
  readonly modificationFrecencyScore: number;
  readonly modified: number;
  readonly lineNumber: number;
  readonly col: number;
  readonly byteOffset: number;
  readonly lineContent: string;
  readonly matchRanges: readonly (readonly [number, number])[];
  readonly fuzzyScore?: number;
  readonly contextBefore?: readonly string[];
  readonly contextAfter?: readonly string[];
  readonly isDefinition?: boolean;
};
type FffGrepResult = {
  readonly items: readonly FffGrepMatch[];
  readonly totalMatched: number;
  readonly totalFilesSearched: number;
  readonly totalFiles: number;
  readonly filteredFileCount: number;
  readonly nextCursor: FffGrepCursor | null;
  readonly regexFallbackError?: string;
};
type FffGrepOptions = {
  readonly maxFileSize?: number;
  readonly maxMatchesPerFile?: number;
  readonly smartCase?: boolean;
  readonly cursor?: FffGrepCursor | null;
  readonly mode?: FffGrepMode;
  readonly timeBudgetMs?: number;
  readonly beforeContext?: number;
  readonly afterContext?: number;
  readonly pageSize?: number;
  readonly classifyDefinitions?: boolean;
};
type FffMultiGrepOptions = Omit<FffGrepOptions, "mode"> & {
  readonly patterns: readonly string[];
  readonly constraints?: string;
};
type FffFileFinder = {
  readonly isDestroyed: boolean;
  destroy(): void;
  fileSearch(
    query: string,
    options?: { readonly pageSize?: number; readonly currentFile?: string },
  ): FffResult<FffSearchResult>;
  glob(
    pattern: string,
    options?: { readonly pageSize?: number; readonly currentFile?: string },
  ): FffResult<FffSearchResult>;
  grep(query: string, options?: FffGrepOptions): FffResult<FffGrepResult>;
  multiGrep(options: FffMultiGrepOptions): FffResult<FffGrepResult>;
  waitForScan(timeoutMs?: number): Promise<FffResult<boolean>>;
  waitForIndexReady(timeoutMs?: number): Promise<FffResult<boolean>>;
  trackQuery(query: string, selectedFilePath: string): FffResult<boolean>;
};
type FffModule = {
  readonly FileFinder: {
    create(options: {
      readonly basePath: string;
      readonly enableHomeDirScanning?: boolean;
      readonly enableFsRootScanning?: boolean;
    }): FffResult<FffFileFinder>;
  };
};

/**
 * File system host: the main-process half of the renderer's FileSystemPort.
 * The renderer is sandboxed; every IO crosses this boundary.
 */
let nextWatchId = 1;
const watchers = new Map<number, fs.FSWatcher>();

function registerFileSystemHandlers(): void {
  ipcMain.handle("fs:read", async (_event, filePath: string) => {
    return fsp.readFile(filePath, "utf8");
  });
  ipcMain.handle("fs:write", async (_event, filePath: string, text: string) => {
    await fsp.mkdir(path.dirname(filePath), { recursive: true });
    await fsp.writeFile(filePath, text, "utf8");
    const stat = await fsp.stat(filePath);
    return { mtimeMs: stat.mtimeMs, size: stat.size };
  });
  ipcMain.handle("fs:mkdir", async (_event, dirPath: string) => {
    await fsp.mkdir(dirPath, { recursive: true });
  });
  ipcMain.handle("fs:stat", async (_event, filePath: string) => {
    try {
      const stat = await fsp.stat(filePath);
      return { mtimeMs: stat.mtimeMs, size: stat.size };
    } catch {
      return null;
    }
  });
  ipcMain.handle("fs:watch", (event, filePath: string) => {
    const id = nextWatchId++;
    const sender = event.sender;
    try {
      // Watch the parent directory: editors (vim, jj, formatters) replace
      // files atomically via rename, which per-file watchers lose track of.
      const dir = path.dirname(filePath);
      const base = path.basename(filePath);
      let debounce: NodeJS.Timeout | null = null;
      const watcher = fs.watch(dir, (eventType, fileName) => {
        if (fileName !== null && fileName !== base) return;
        if (debounce !== null) clearTimeout(debounce);
        debounce = setTimeout(() => {
          debounce = null;
          if (sender.isDestroyed()) return;
          const kind = fs.existsSync(filePath) ? "changed" : "deleted";
          sender.send("fs:event", { watchId: id, path: filePath, kind });
        }, 30);
      });
      watchers.set(id, watcher);
    } catch {
      // Watching is best-effort; the port reports capability anyway.
    }
    return id;
  });
  ipcMain.handle("fs:unwatch", (_event, id: number) => {
    watchers.get(id)?.close();
    watchers.delete(id);
  });
  ipcMain.handle("fs:readdir", async (_event, dirPath: string) => {
    const entries = await fsp.readdir(dirPath, { withFileTypes: true });
    return entries.map((entry) => ({
      name: entry.name,
      kind: entry.isDirectory() ? "directory" : "file",
    }));
  });
  ipcMain.handle("fs:watchdir", (event, dirPath: string) => {
    const id = nextWatchId++;
    const sender = event.sender;
    try {
      let debounce: NodeJS.Timeout | null = null;
      const pending = new Set<string>();
      // Recursive watch (macOS/Windows native; tree + VCS refresh source).
      const watcher = fs.watch(dirPath, { recursive: true }, (_eventType, fileName) => {
        if (fileName !== null) pending.add(path.join(dirPath, String(fileName)));
        if (debounce !== null) clearTimeout(debounce);
        debounce = setTimeout(() => {
          debounce = null;
          if (sender.isDestroyed()) return;
          const paths = [...pending];
          pending.clear();
          for (const changed of paths.length > 0 ? paths : [dirPath]) {
            sender.send("fs:event", { watchId: id, path: changed, kind: "changed" });
          }
        }, 80);
      });
      watchers.set(id, watcher);
    } catch {
      // Best-effort, like fs:watch.
    }
    return id;
  });
  ipcMain.handle("fs:pickfolder", async (event) => {
    const window = BrowserWindow.fromWebContents(event.sender);
    const result = window
      ? await dialog.showOpenDialog(window, { properties: ["openDirectory"] })
      : await dialog.showOpenDialog({ properties: ["openDirectory"] });
    return result.canceled || result.filePaths.length === 0 ? null : result.filePaths[0];
  });
  ipcMain.handle("fs:pick", async (event) => {
    const window = BrowserWindow.fromWebContents(event.sender);
    const result = window
      ? await dialog.showOpenDialog(window, { properties: ["openFile"] })
      : await dialog.showOpenDialog({ properties: ["openFile"] });
    return result.canceled || result.filePaths.length === 0 ? null : result.filePaths[0];
  });
}

/**
 * Process host: the main-process half of the renderer's ProcessPort.
 * Language servers (and any future external tool) run as ordinary child
 * processes here; stdio crosses to the renderer as raw byte chunks.
 */
let nextProcId = 1;
const procs = new Map<number, ChildProcess>();

function registerProcessHandlers(): void {
  ipcMain.handle(
    "proc:spawn",
    (event, options: { command: string; args?: string[]; cwd?: string }) => {
      const sender = event.sender;
      try {
        const child = spawn(options.command, options.args ?? [], {
          ...(options.cwd ? { cwd: options.cwd } : {}),
          stdio: ["pipe", "pipe", "pipe"],
        });
        const id = nextProcId++;
        procs.set(id, child);
        child.stdout!.on("data", (chunk: Buffer) => {
          if (!sender.isDestroyed()) {
            sender.send("proc:data", { id, stream: "stdout", chunk: new Uint8Array(chunk) });
          }
        });
        child.stderr!.on("data", (chunk: Buffer) => {
          if (!sender.isDestroyed()) {
            sender.send("proc:data", { id, stream: "stderr", chunk: new Uint8Array(chunk) });
          }
        });
        child.on("exit", (code) => {
          procs.delete(id);
          if (!sender.isDestroyed()) sender.send("proc:exit", { id, code });
        });
        child.on("error", () => {
          procs.delete(id);
          if (!sender.isDestroyed()) sender.send("proc:exit", { id, code: -1 });
        });
        return id;
      } catch {
        return null;
      }
    },
  );
  ipcMain.handle("proc:write", (_event, id: number, chunk: Uint8Array) => {
    procs.get(id)?.stdin?.write(Buffer.from(chunk));
  });
  ipcMain.handle("proc:kill", (_event, id: number) => {
    procs.get(id)?.kill();
    procs.delete(id);
  });
  app.on("will-quit", () => {
    for (const child of procs.values()) child.kill();
    procs.clear();
  });
}

/**
 * fff host: one native index for the active project root. The renderer sends
 * only queries and receives serializable file items; @ff-labs/fff-node stays in
 * the trusted Node side where native libraries can load.
 */
let fffModule: FffModule | null = null;
let fffLoad: Promise<FffModule | null> | null = null;
type FffEntry = { readonly finder: FffFileFinder; lastUsed: number };
/**
 * One native index per workspace root. Switching workspaces must NOT destroy
 * the previous index (re-scanning is expensive), so finders stay warm in an
 * LRU map; only the least-recently-used is evicted once past the cap.
 */
const fffFinders = new Map<string, FffEntry>();
const fffPending = new Map<string, Promise<FffFileFinder>>();
const MAX_FFF_FINDERS = 4;

async function loadFff(): Promise<FffModule | null> {
  if (fffModule !== null) return fffModule;
  fffLoad ??= import("@ff-labs/fff-node")
    .then((mod) => {
      fffModule = mod as unknown as FffModule;
      return fffModule;
    })
    .catch((error) => {
      console.warn("[file-finder] @ff-labs/fff-node unavailable; file finder disabled.", error);
      return null;
    });
  return fffLoad;
}

function normalizeRoot(root: string): string {
  return root.replace(/\/+$/, "") || "/";
}

async function getFff(rootInput: string): Promise<FffResult<FffFileFinder>> {
  const root = normalizeRoot(rootInput);
  const existing = fffFinders.get(root);
  if (existing !== undefined && !existing.finder.isDestroyed) {
    existing.lastUsed = Date.now();
    return { ok: true, value: existing.finder };
  }
  const pending = fffPending.get(root);
  if (pending !== undefined) {
    try {
      return { ok: true, value: await pending };
    } catch (error) {
      return { ok: false, error: error instanceof Error ? error.message : String(error) };
    }
  }

  if (existing !== undefined) fffFinders.delete(root); // destroyed: drop the stale entry
  const mod = await loadFff();
  if (mod === null) return { ok: false, error: "fff is unavailable in this desktop build" };

  const promise = (async () => {
    const created = mod.FileFinder.create({
      basePath: root,
      enableHomeDirScanning: true,
      enableFsRootScanning: false,
    });
    if (!created.ok) throw new Error(created.error);
    const finder = created.value;
    await finder.waitForScan(2_000);
    fffFinders.set(root, { finder, lastUsed: Date.now() });
    evictFffFinders();
    return finder;
  })();
  fffPending.set(root, promise);
  try {
    return { ok: true, value: await promise };
  } catch (error) {
    return { ok: false, error: error instanceof Error ? error.message : String(error) };
  } finally {
    if (fffPending.get(root) === promise) fffPending.delete(root);
  }
}

/** Destroy least-recently-used finders once the warm set exceeds the cap. The
 *  just-created finder carries the newest timestamp, so it is never evicted. */
function evictFffFinders(): void {
  while (fffFinders.size > MAX_FFF_FINDERS) {
    let oldestRoot: string | undefined;
    let oldest = Infinity;
    for (const [candidate, entry] of fffFinders) {
      if (entry.lastUsed < oldest) {
        oldest = entry.lastUsed;
        oldestRoot = candidate;
      }
    }
    if (oldestRoot === undefined) break;
    fffFinders.get(oldestRoot)?.finder.destroy();
    fffFinders.delete(oldestRoot);
  }
}

type FinderGrepInput = {
  readonly root: string;
  readonly query: string;
  readonly limit: number;
  readonly mode?: FffGrepMode;
  readonly cursor?: FffGrepCursor | null;
  readonly maxFileSize?: number;
  readonly maxMatchesPerFile?: number;
  readonly smartCase?: boolean;
  readonly timeBudgetMs?: number;
  readonly beforeContext?: number;
  readonly afterContext?: number;
  readonly classifyDefinitions?: boolean;
};

type FinderMultiGrepInput = Omit<FinderGrepInput, "query" | "mode"> & {
  readonly patterns: readonly string[];
  readonly constraints?: string;
};

function boundedInt(value: number, fallback: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return fallback;
  return Math.min(Math.max(Math.trunc(value), min), max);
}

function grepOptions(input: FinderGrepInput | FinderMultiGrepInput): FffGrepOptions {
  return {
    pageSize: boundedInt(input.limit, 50, 1, 10_000),
    ...(input.cursor !== undefined ? { cursor: input.cursor } : {}),
    ...(input.maxFileSize !== undefined ? { maxFileSize: boundedInt(input.maxFileSize, 10 * 1024 * 1024, 1, Number.MAX_SAFE_INTEGER) } : {}),
    ...(input.maxMatchesPerFile !== undefined ? { maxMatchesPerFile: boundedInt(input.maxMatchesPerFile, 200, 0, 100_000) } : {}),
    ...(input.smartCase !== undefined ? { smartCase: input.smartCase } : {}),
    ...(input.timeBudgetMs !== undefined ? { timeBudgetMs: boundedInt(input.timeBudgetMs, 0, 0, 60_000) } : {}),
    ...(input.beforeContext !== undefined ? { beforeContext: boundedInt(input.beforeContext, 0, 0, 1_000) } : {}),
    ...(input.afterContext !== undefined ? { afterContext: boundedInt(input.afterContext, 0, 0, 1_000) } : {}),
    ...(input.classifyDefinitions !== undefined ? { classifyDefinitions: input.classifyDefinitions } : {}),
    ...("mode" in input && input.mode !== undefined ? { mode: input.mode } : {}),
  };
}

async function waitForFffContentIndex(finder: FffFileFinder): Promise<FffResult<boolean>> {
  const ready = await finder.waitForIndexReady(2_000);
  return ready.ok ? ready : { ok: false, error: ready.error };
}

function multiGrepOptions(input: FinderMultiGrepInput): FffMultiGrepOptions {
  const { mode: _mode, ...options } = grepOptions(input);
  return {
    ...options,
    patterns: input.patterns,
    ...(input.constraints !== undefined ? { constraints: input.constraints } : {}),
  };
}

function registerFileFinderHandlers(): void {
  ipcMain.handle(
    "finder:search",
    async (
      _event,
      input: { root: string; query: string; limit: number; currentFile?: string },
    ): Promise<FffResult<FffSearchResult>> => {
      const found = await getFff(input.root);
      if (!found.ok) return found;
      const query = input.query.trim();
      const pageSize = Math.min(Math.max(input.limit, 1), 200);
      const options = {
        pageSize,
        ...(input.currentFile !== undefined ? { currentFile: input.currentFile } : {}),
      };
      return query.length === 0
        ? found.value.glob("**/*", options)
        : found.value.fileSearch(query, options);
    },
  );
  ipcMain.handle(
    "finder:grep",
    async (_event, input: FinderGrepInput): Promise<FffResult<FffGrepResult>> => {
      const found = await getFff(input.root);
      if (!found.ok) return found;
      const ready = await waitForFffContentIndex(found.value);
      if (!ready.ok) return ready;
      return found.value.grep(input.query, grepOptions(input));
    },
  );
  ipcMain.handle(
    "finder:multi-grep",
    async (_event, input: FinderMultiGrepInput): Promise<FffResult<FffGrepResult>> => {
      const found = await getFff(input.root);
      if (!found.ok) return found;
      const patterns = input.patterns.filter((pattern) => pattern.length > 0);
      if (patterns.length === 0) return { ok: false, error: "multiGrep requires at least one non-empty pattern" };
      const ready = await waitForFffContentIndex(found.value);
      if (!ready.ok) return ready;
      return found.value.multiGrep(multiGrepOptions({ ...input, patterns }));
    },
  );
  ipcMain.handle(
    "finder:track",
    async (
      _event,
      input: { root: string; query: string; relativePath: string },
    ): Promise<FffResult<boolean>> => {
      const found = await getFff(input.root);
      if (!found.ok) return found;
      return found.value.trackQuery(input.query, input.relativePath);
    },
  );
  app.on("will-quit", () => {
    for (const entry of fffFinders.values()) entry.finder.destroy();
    fffFinders.clear();
  });
}

/**
 * Terminal host: the main-process half of the renderer's TerminalPort. Real
 * PTYs need node-pty (native); the require is guarded so a missing/unbuilt
 * binary degrades to "no terminal" instead of crashing the app. Run
 * `pnpm --filter @the-editor/desktop exec electron-rebuild -f -w node-pty`
 * if it fails to load under Electron's ABI.
 */
type Pty = {
  readonly pid: number;
  onData(cb: (data: string) => void): void;
  onExit(cb: (e: { exitCode: number }) => void): void;
  write(data: string): void;
  resize(cols: number, rows: number): void;
  kill(): void;
};
type PtyModule = {
  spawn(
    file: string,
    args: string[],
    opts: { name: string; cols: number; rows: number; cwd: string; env: NodeJS.ProcessEnv },
  ): Pty;
};

let ptyModule: PtyModule | null = null;
let ptyLoadAttempted = false;
function loadPty(): PtyModule | null {
  if (!ptyLoadAttempted) {
    ptyLoadAttempted = true;
    try {
      ptyModule = nodeRequire("node-pty") as PtyModule;
    } catch (error) {
      console.warn("[terminal] node-pty unavailable; terminal disabled.", error);
      ptyModule = null;
    }
  }
  return ptyModule;
}

let nextTermId = 1;
const terms = new Map<number, Pty>();

function registerTerminalHandlers(): void {
  ipcMain.handle(
    "term:spawn",
    (event, options: { cols: number; rows: number; cwd?: string; shell?: string }) => {
      const pty = loadPty();
      if (pty === null) return null;
      const sender = event.sender;
      try {
        const shell =
          options.shell ?? process.env.SHELL ?? (process.platform === "win32" ? "powershell.exe" : "/bin/bash");
        const child = pty.spawn(shell, [], {
          name: "xterm-256color",
          cols: Math.max(1, options.cols),
          rows: Math.max(1, options.rows),
          cwd: options.cwd ?? process.env.HOME ?? process.cwd(),
          env: { ...process.env, TERM: "xterm-256color" },
        });
        const id = nextTermId++;
        terms.set(id, child);
        child.onData((data) => {
          if (!sender.isDestroyed()) sender.send("term:data", { id, data });
        });
        child.onExit(({ exitCode }) => {
          terms.delete(id);
          if (!sender.isDestroyed()) sender.send("term:exit", { id, code: exitCode });
        });
        return id;
      } catch (error) {
        console.warn("[terminal] spawn failed", error);
        return null;
      }
    },
  );
  ipcMain.handle("term:write", (_event, id: number, data: string) => {
    terms.get(id)?.write(data);
  });
  ipcMain.handle("term:resize", (_event, id: number, cols: number, rows: number) => {
    try {
      terms.get(id)?.resize(Math.max(1, cols), Math.max(1, rows));
    } catch {
      // A resize on a dying PTY is harmless to drop.
    }
  });
  ipcMain.handle("term:kill", (_event, id: number) => {
    terms.get(id)?.kill();
    terms.delete(id);
  });
  app.on("will-quit", () => {
    for (const child of terms.values()) child.kill();
    terms.clear();
  });
}

function createWindow(): void {
  // Dark chrome end to end — the native title bar follows the theme.
  nativeTheme.themeSource = "dark";
  const window = new BrowserWindow({
    width: 1200,
    height: 800,
    backgroundColor: "#141415",
    title: "The Editor",
    // Reclaim the title bar: the renderer paints its own themed bar (see
    // TitleBar.tsx) and the traffic lights float over it, centered in the
    // 44px-tall band the renderer reserves on the left.
    titleBarStyle: "hiddenInset",
    // x/y chosen to match the standard macOS inset and to vertically center
    // the 12px lights in the 36px title bar (center ≈ y17), so they line up
    // with the toggle button (also centered in the bar).
    trafficLightPosition: { x: 13, y: 11 },
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      // The preload hosts the synchronous syntax engine (native
      // tree-sitter); that requires the unsandboxed renderer. Context
      // isolation stays on and the page still has no node access.
      sandbox: false,
    },
  });

  if (devServerUrl) {
    void window.loadURL(devServerUrl);
  } else {
    void window.loadFile(path.join(__dirname, "..", "..", "web", "dist", "index.html"));
  }
}

void app.whenReady().then(() => {
  registerFileSystemHandlers();
  registerProcessHandlers();
  registerFileFinderHandlers();
  registerTerminalHandlers();
  createWindow();
  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
