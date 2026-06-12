import * as path from "node:path";
import * as fs from "node:fs";
import * as fsp from "node:fs/promises";
import { app, BrowserWindow, dialog, ipcMain } from "electron";

const devServerUrl = process.env.VITE_DEV_SERVER_URL?.trim();

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
    await fsp.writeFile(filePath, text, "utf8");
    const stat = await fsp.stat(filePath);
    return { mtimeMs: stat.mtimeMs, size: stat.size };
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
  ipcMain.handle("fs:pick", async (event) => {
    const window = BrowserWindow.fromWebContents(event.sender);
    const result = window
      ? await dialog.showOpenDialog(window, { properties: ["openFile"] })
      : await dialog.showOpenDialog({ properties: ["openFile"] });
    return result.canceled || result.filePaths.length === 0 ? null : result.filePaths[0];
  });
}

function createWindow(): void {
  const window = new BrowserWindow({
    width: 1200,
    height: 800,
    backgroundColor: "#161616",
    title: "The Editor",
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
  createWindow();
  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
