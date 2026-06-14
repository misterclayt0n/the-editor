import * as path from "node:path";
import * as fs from "node:fs";
import * as fsp from "node:fs/promises";
import { spawn, type ChildProcess } from "node:child_process";
import { createRequire } from "node:module";
import { app, BrowserWindow, dialog, ipcMain, nativeTheme } from "electron";

const devServerUrl = process.env.VITE_DEV_SERVER_URL?.trim();
/** Native modules (node-pty) are externalized; require them at runtime. */
const nodeRequire = createRequire(import.meta.url);

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
  registerTerminalHandlers();
  createWindow();
  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
