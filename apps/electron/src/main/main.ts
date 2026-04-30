import { app, BrowserWindow, ipcMain } from "electron";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { Worker } from "node:worker_threads";
import { spawnRuntimeClient } from "@the-editor/editor/runtimeClient";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const isDev = process.env.THE_EDITOR_DEV_SERVER_URL !== undefined;

let mainWindow: BrowserWindow | undefined;
let policyWorker: Worker | undefined;
const runtime = spawnRuntimeClient({ cwd: path.resolve(__dirname, "../../../../") });
const pendingRuntimeRequests = new Map<number, { resolve(value: unknown): void; reject(error: unknown): void }>();
let nextRuntimeRequestId = 1;

async function createWindow(): Promise<void> {
  mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    backgroundColor: "#111111",
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  mainWindow.webContents.on("console-message", (_event, level, message, line, sourceId) => {
    console.log(`[renderer:${level}] ${message} (${sourceId}:${line})`);
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
    const rendererIndex = path.join(__dirname, "../renderer/index.html");
    console.log(`loading renderer ${rendererIndex}`);
    await mainWindow.loadFile(rendererIndex);
  }
}

function startPolicyWorker(): void {
  policyWorker = new Worker(path.join(__dirname, "../policy/worker.cjs"), {
    workerData: { root: path.resolve(__dirname, "../../../../") },
  });

  policyWorker.on("message", (message: unknown) => {
    if (!isObject(message)) return;
    if (message.type === "runtime.request") {
      void handlePolicyRuntimeRequest(message as unknown as RuntimeBridgeRequest);
      return;
    }
    if (message.type === "policy.response") {
      handlePolicyResponse(message as unknown as PolicyBridgeResponse);
      return;
    }
    mainWindow?.webContents.send("policy:event", message);
  });

  policyWorker.on("error", (error) => {
    mainWindow?.webContents.send("policy:event", {
      type: "policy.error",
      message: error.message,
      stack: error.stack,
    });
  });
}

async function handlePolicyRuntimeRequest(message: RuntimeBridgeRequest): Promise<void> {
  try {
    const result = await runtime.request(message.method, message.params ?? {});
    policyWorker?.postMessage({ type: "runtime.response", id: message.id, result });
  } catch (error) {
    policyWorker?.postMessage({
      type: "runtime.response",
      id: message.id,
      error: serializeError(error),
    });
  }
}

ipcMain.handle("runtime:request", async (_event, request: { method: string; params?: unknown }) => {
  return runtime.request(request.method, (request.params ?? {}) as object);
});

ipcMain.handle("policy:request", async (_event, request: { method: string; params?: unknown }) => {
  if (!policyWorker) throw new Error("policy worker is not running");
  const id = nextRuntimeRequestId++;
  policyWorker.postMessage({ type: "policy.request", id, method: request.method, params: request.params });
  return new Promise((resolve, reject) => {
    pendingRuntimeRequests.set(id, { resolve, reject });
  });
});

app.whenReady().then(async () => {
  startPolicyWorker();
  await createWindow();
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});

app.on("before-quit", () => {
  policyWorker?.terminate();
  runtime.dispose();
});

interface RuntimeBridgeRequest {
  type: "runtime.request";
  id: number;
  method: string;
  params?: unknown;
}

interface PolicyBridgeResponse {
  type: "policy.response";
  id: number;
  result?: unknown;
  error?: unknown;
}

function handlePolicyResponse(response: PolicyBridgeResponse): void {
  const pending = pendingRuntimeRequests.get(response.id);
  if (!pending) return;
  pendingRuntimeRequests.delete(response.id);
  if (response.error) pending.reject(response.error);
  else pending.resolve(response.result);
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function serializeError(error: unknown): { message: string; stack?: string } {
  if (error instanceof Error) return { message: error.message, stack: error.stack };
  return { message: String(error) };
}
