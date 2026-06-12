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
});
