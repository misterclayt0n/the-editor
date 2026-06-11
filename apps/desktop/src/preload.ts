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
