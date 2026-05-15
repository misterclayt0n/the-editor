import { app, BrowserWindow, dialog, ipcMain, shell } from "electron";
import path from "node:path";
import { Worker } from "node:worker_threads";
import { MainNativeRuntimeApi } from "./nativeRuntimeApi.js";

const isDev = process.env.THE_EDITOR_DEV_SERVER_URL !== undefined;
const runtime = new MainNativeRuntimeApi();

let mainWindow: BrowserWindow | undefined;
let policyWorker: Worker | undefined;
const pendingPolicyRequests = new Map<number, { resolve(value: unknown): void; reject(error: unknown): void }>();
let nextPolicyRequestId = 1;

async function createWindow(): Promise<void> {
  mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    titleBarStyle: process.platform === "darwin" ? "hiddenInset" : "default",
    trafficLightPosition: process.platform === "darwin" ? { x: 16, y: 18 } : undefined,
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      scrollBounce: true,
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
    },
  });

  mainWindow.webContents.on("did-fail-load", (_event, errorCode, errorDescription, validatedURL) => {
    console.error(`renderer failed to load ${validatedURL}: ${errorCode} ${errorDescription}`);
  });
  mainWindow.webContents.on("render-process-gone", (_event, details) => {
    console.error("renderer process gone", details);
  });

  if (isDev) {
    await mainWindow.loadURL(process.env.THE_EDITOR_DEV_SERVER_URL!);
    mainWindow.webContents.openDevTools({ mode: "detach" });
  } else {
    await mainWindow.loadFile(path.join(__dirname, "../renderer/index.html"));
  }
}

function startPolicyWorker(): void {
  policyWorker = new Worker(path.join(__dirname, "../policy/worker.cjs"), {
    workerData: {
      root: path.resolve(__dirname, "../../../../"),
      cwd: resolveLaunchCwd(),
    },
  });

  policyWorker.on("message", (message: unknown) => {
    if (!isObject(message)) return;
    if (message.type === "runtime.request") {
      void handleRuntimeRequest(message as unknown as RuntimeBridgeRequest);
      return;
    }
    if (message.type === "policy.response") {
      handlePolicyResponse(message as unknown as PolicyBridgeResponse);
      return;
    }
    mainWindow?.webContents.send("policy:event", message);
  });

  policyWorker.on("error", (error) => {
    console.error("policy worker error", error);
    mainWindow?.webContents.send("policy:event", {
      type: "policy.error",
      message: error.message,
      stack: error.stack,
    });
  });

  policyWorker.on("exit", (code) => {
    policyWorker = undefined;
    rejectPendingPolicyRequests(new Error(`policy worker exited with code ${code}`));
  });
}

ipcMain.handle("app:open-file-dialog", async () => {
  const result = await dialog.showOpenDialog(mainWindow!, {
    properties: ["openFile"],
  });
  return result.canceled ? null : result.filePaths[0];
});

ipcMain.handle("app:open-external", async (_event, url: unknown) => {
  if (typeof url !== "string" || !url.trim()) throw new Error("invalid external url");
  await shell.openExternal(url);
  return true;
});

ipcMain.handle("policy:request", async (_event, request: { method: string; params?: unknown }) => {
  if (!policyWorker) throw new Error("policy worker is not running");
  const id = nextPolicyRequestId++;
  policyWorker.postMessage({ type: "policy.request", id, method: request.method, params: request.params });
  return new Promise((resolve, reject) => {
    pendingPolicyRequests.set(id, { resolve, reject });
  });
});

app.whenReady().then(async () => {
  await createWindow();
  startPolicyWorker();
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});

app.on("before-quit", () => {
  policyWorker?.terminate();
});

interface PolicyBridgeResponse {
  type: "policy.response";
  id: number;
  result?: unknown;
  error?: unknown;
}

interface RuntimeBridgeRequest {
  type: "runtime.request";
  id: number;
  method: string;
  params?: unknown;
}

async function handleRuntimeRequest(request: RuntimeBridgeRequest): Promise<void> {
  try {
    const result = await runtime.request(request.method, request.params);
    policyWorker?.postMessage({ type: "runtime.response", id: request.id, result });
  } catch (error) {
    policyWorker?.postMessage({ type: "runtime.response", id: request.id, error: serializeError(error) });
  }
}

function handlePolicyResponse(response: PolicyBridgeResponse): void {
  const pending = pendingPolicyRequests.get(response.id);
  if (!pending) return;
  pendingPolicyRequests.delete(response.id);
  if (response.error) pending.reject(response.error);
  else pending.resolve(response.result);
}

function rejectPendingPolicyRequests(error: Error): void {
  for (const pending of pendingPolicyRequests.values()) pending.reject(error);
  pendingPolicyRequests.clear();
}

function resolveLaunchCwd(): string {
  const initCwd = process.env.INIT_CWD?.trim();
  return initCwd || process.cwd();
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function serializeError(error: unknown): { message: string; stack?: string } {
  if (error instanceof Error) return { message: error.message, stack: error.stack };
  return { message: String(error) };
}
