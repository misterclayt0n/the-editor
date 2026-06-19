import { contextBridge, ipcRenderer } from "electron";

type FileEventPayload = {
  readonly watchId: number;
  readonly path: string;
  readonly kind: "changed" | "deleted";
};

const watchListeners = new Map<number, (event: { path: string; kind: string }) => void>();

ipcRenderer.on("fs:event", (_event, payload: FileEventPayload) => {
  watchListeners.get(payload.watchId)?.({ path: payload.path, kind: payload.kind });
});

contextBridge.exposeInMainWorld("theEditorDesktop", {
  platform: process.platform,
});

/**
 * The renderer-side half of FileSystemPort. The web app wraps this bridge
 * when it exists (Electron); otherwise it falls back to the File System
 * Access API.
 */
/**
 * The SYNCHRONOUS syntax bridge: the foreground tree lives in this renderer
 * process (the window runs without the OS sandbox so the preload can load
 * native tree-sitter), and the editor queries highlights as plain function
 * calls during layout — no round trip, no frame without colors.
 */
try {
  // Lazy require so a missing native toolchain degrades to "no highlights"
  // instead of breaking the preload (and with it, file IO).
  const { createSyntaxEngine, defaultRequire } =
    require("./syntax-engine.cjs") as typeof import("./syntax-engine");
  const engine = createSyntaxEngine(defaultRequire);
  contextBridge.exposeInMainWorld("theEditorSyntaxSync", {
    open: (docId: string, path: string | undefined, text: string) =>
      engine.open(docId, path, text),
    edit: (docId: string, edits: ReadonlyArray<{ start: number; end: number; text: string }>) =>
      engine.edit(docId, edits),
    pump: (docId: string, budgetMs: number) => engine.pump(docId, budgetMs),
    highlights: (docId: string, from: number, to: number) => engine.highlights(docId, from, to),
    close: (docId: string) => engine.close(docId),
  });
} catch (error) {
  console.error("[preload] syntax engine unavailable:", error);
}

/**
 * The renderer-side half of ProcessPort (language servers and other
 * external tools). Byte chunks; listener routing by process id.
 */
const procDataListeners = new Map<
  number,
  (chunk: Uint8Array, stream: "stdout" | "stderr") => void
>();
const procExitListeners = new Map<number, (code: number | null) => void>();
ipcRenderer.on(
  "proc:data",
  (_event, payload: { id: number; stream: "stdout" | "stderr"; chunk: Uint8Array }) => {
    procDataListeners.get(payload.id)?.(payload.chunk, payload.stream);
  },
);
ipcRenderer.on("proc:exit", (_event, payload: { id: number; code: number | null }) => {
  procExitListeners.get(payload.id)?.(payload.code);
  procDataListeners.delete(payload.id);
  procExitListeners.delete(payload.id);
});

contextBridge.exposeInMainWorld("theEditorProc", {
  spawn: (options: { command: string; args?: string[]; cwd?: string }): Promise<number | null> =>
    ipcRenderer.invoke("proc:spawn", options),
  write: (id: number, chunk: Uint8Array): Promise<void> =>
    ipcRenderer.invoke("proc:write", id, chunk),
  kill: (id: number): Promise<void> => {
    procDataListeners.delete(id);
    procExitListeners.delete(id);
    return ipcRenderer.invoke("proc:kill", id);
  },
  onData: (
    id: number,
    listener: (chunk: Uint8Array, stream: "stdout" | "stderr") => void,
  ): void => {
    procDataListeners.set(id, listener);
  },
  onExit: (id: number, listener: (code: number | null) => void): void => {
    procExitListeners.set(id, listener);
  },
});

type FileFinderGrepMode = "plain" | "regex" | "fuzzy";
type FileFinderGrepCursor = { readonly __brand: "GrepCursor"; readonly _offset: number };
type FileFinderGrepMatch = {
  readonly relativePath: string;
  readonly fileName: string;
  readonly gitStatus: string;
  readonly size: number;
  readonly modified: number;
  readonly isBinary: boolean;
  readonly totalFrecencyScore: number;
  readonly accessFrecencyScore: number;
  readonly modificationFrecencyScore: number;
  readonly lineNumber: number;
  readonly col: number;
  readonly byteOffset: number;
  readonly lineContent: string;
  readonly matchRanges: Array<[number, number]>;
  readonly fuzzyScore?: number;
  readonly contextBefore?: string[];
  readonly contextAfter?: string[];
  readonly isDefinition?: boolean;
};
type FileFinderGrepResult = {
  readonly items: FileFinderGrepMatch[];
  readonly totalMatched: number;
  readonly totalFilesSearched: number;
  readonly totalFiles: number;
  readonly filteredFileCount: number;
  readonly nextCursor: FileFinderGrepCursor | null;
  readonly regexFallbackError?: string;
};

contextBridge.exposeInMainWorld("theEditorFileFinder", {
  search: (input: {
    root: string;
    query: string;
    limit: number;
    currentFile?: string;
  }): Promise<
    | {
        ok: true;
        value: {
          items: Array<{ relativePath: string; fileName: string; size: number; gitStatus: string }>;
          totalMatched: number;
          totalFiles: number;
          location?:
            | { type: "line"; line: number }
            | { type: "position"; line: number; col: number }
            | {
                type: "range";
                start: { line: number; col: number };
                end: { line: number; col: number };
              };
        };
      }
    | { ok: false; error: string }
  > => ipcRenderer.invoke("finder:search", input),
  grep: (input: {
    root: string;
    query: string;
    limit: number;
    mode?: FileFinderGrepMode;
    cursor?: FileFinderGrepCursor | null;
    maxFileSize?: number;
    maxMatchesPerFile?: number;
    smartCase?: boolean;
    timeBudgetMs?: number;
    beforeContext?: number;
    afterContext?: number;
    classifyDefinitions?: boolean;
  }): Promise<{ ok: true; value: FileFinderGrepResult } | { ok: false; error: string }> =>
    ipcRenderer.invoke("finder:grep", input),
  multiGrep: (input: {
    root: string;
    patterns: string[];
    constraints?: string;
    limit: number;
    cursor?: FileFinderGrepCursor | null;
    maxFileSize?: number;
    maxMatchesPerFile?: number;
    smartCase?: boolean;
    timeBudgetMs?: number;
    beforeContext?: number;
    afterContext?: number;
    classifyDefinitions?: boolean;
  }): Promise<{ ok: true; value: FileFinderGrepResult } | { ok: false; error: string }> =>
    ipcRenderer.invoke("finder:multi-grep", input),
  track: (input: {
    root: string;
    query: string;
    relativePath: string;
  }): Promise<{ ok: true; value: boolean } | { ok: false; error: string }> =>
    ipcRenderer.invoke("finder:track", input),
});

/**
 * The renderer-side half of TerminalPort (the terminal panel's PTY). A single
 * merged byte stream + window size, routed by terminal id.
 */
const termDataListeners = new Map<number, (data: string) => void>();
const termExitListeners = new Map<number, (code: number | null) => void>();
ipcRenderer.on("term:data", (_event, payload: { id: number; data: string }) => {
  termDataListeners.get(payload.id)?.(payload.data);
});
ipcRenderer.on("term:exit", (_event, payload: { id: number; code: number | null }) => {
  termExitListeners.get(payload.id)?.(payload.code);
  termDataListeners.delete(payload.id);
  termExitListeners.delete(payload.id);
});

contextBridge.exposeInMainWorld("theEditorTerminal", {
  spawn: (options: { cols: number; rows: number; cwd?: string; shell?: string }): Promise<number | null> =>
    ipcRenderer.invoke("term:spawn", options),
  write: (id: number, data: string): void => ipcRenderer.send("term:write", id, data),
  resize: (id: number, cols: number, rows: number): void => ipcRenderer.send("term:resize", id, cols, rows),
  kill: (id: number): void => {
    termDataListeners.delete(id);
    termExitListeners.delete(id);
    ipcRenderer.send("term:kill", id);
  },
  onData: (id: number, listener: (data: string) => void): void => {
    termDataListeners.set(id, listener);
  },
  onExit: (id: number, listener: (code: number | null) => void): void => {
    termExitListeners.set(id, listener);
  },
});

contextBridge.exposeInMainWorld("theEditorFs", {
  readFile: (path: string): Promise<string> => ipcRenderer.invoke("fs:read", path),
  writeFile: (path: string, text: string): Promise<{ mtimeMs: number; size: number }> =>
    ipcRenderer.invoke("fs:write", path, text),
  stat: (path: string): Promise<{ mtimeMs: number; size: number } | null> =>
    ipcRenderer.invoke("fs:stat", path),
  watch: async (
    path: string,
    onEvent: (event: { path: string; kind: string }) => void,
  ): Promise<number> => {
    const id: number = await ipcRenderer.invoke("fs:watch", path);
    watchListeners.set(id, onEvent);
    return id;
  },
  unwatch: (id: number): Promise<void> => {
    watchListeners.delete(id);
    return ipcRenderer.invoke("fs:unwatch", id);
  },
  pickFile: (): Promise<string | null> => ipcRenderer.invoke("fs:pick"),
  pickFolder: (): Promise<string | null> => ipcRenderer.invoke("fs:pickfolder"),
  readDir: (
    path: string,
  ): Promise<Array<{ name: string; kind: "file" | "directory" }>> =>
    ipcRenderer.invoke("fs:readdir", path),
  mkdir: (path: string): Promise<void> => ipcRenderer.invoke("fs:mkdir", path),
  watchDir: async (
    path: string,
    onEvent: (event: { path: string; kind: string }) => void,
  ): Promise<number> => {
    const id: number = await ipcRenderer.invoke("fs:watchdir", path);
    watchListeners.set(id, onEvent);
    return id;
  },
});
