import { app, BrowserWindow } from "electron";
import { Effect } from "effect";
import { join } from "node:path";

const webDevServerUrl = process.env["THE_EDITOR_WEB_URL"] ?? "http://localhost:5173";

const createWindow = Effect.sync(() => {
  const window = new BrowserWindow({
    width: 1200,
    height: 800,
    backgroundColor: "#1e1e1e",
    webPreferences: {
      preload: join(__dirname, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
    },
  });

  void window.loadURL(webDevServerUrl);
  window.webContents.openDevTools({ mode: "detach" });
});

void app.whenReady().then(() => {
  Effect.runSync(createWindow);

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) Effect.runSync(createWindow);
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
