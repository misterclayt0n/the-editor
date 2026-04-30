import { parentPort } from "node:worker_threads";
import { activate, getScratchState } from "@the-editor/default";
import { createEditorHost } from "@the-editor/editor";
import type { JsonValue, RuntimeApi } from "@the-editor/contracts/runtime";

if (!parentPort) throw new Error("policy worker requires parentPort");

const runtime = createRuntimeBridge();
const editor = createEditorHost({ runtime });

parentPort.on("message", (message: unknown) => {
  if (!isObject(message)) return;
  if (message.type === "runtime.response") {
    runtime.handleResponse(message as unknown as RuntimeResponse);
    return;
  }
  if (message.type === "policy.request") {
    void handlePolicyRequest(message as unknown as PolicyRequest);
  }
});

void boot();

async function boot(): Promise<void> {
  await activate(editor);
  parentPort?.postMessage({ type: "policy.ready" });
}

async function handlePolicyRequest(request: PolicyRequest): Promise<void> {
  try {
    let result: unknown;
    switch (request.method) {
      case "default.createScratch": {
        await editor.commands.dispatch({ id: "scratch.create", args: request.params });
        const state = getScratchState(editor);
        result = {
          activeDocumentId: state.activeDocumentId,
          activeSurfaceId: state.activeSurfaceId,
          frame: state.lastFrame,
        };
        parentPort?.postMessage({ type: "default.activeSurfaceChanged", ...(result as Record<string, unknown>) });
        break;
      }
      default:
        throw new Error(`unknown policy method ${request.method}`);
    }
    parentPort?.postMessage({ type: "policy.response", id: request.id, result });
  } catch (error) {
    parentPort?.postMessage({ type: "policy.response", id: request.id, error: serializeError(error) });
  }
}

interface RuntimeResponse {
  type: "runtime.response";
  id: number;
  result?: unknown;
  error?: unknown;
}

interface PolicyRequest {
  type: "policy.request";
  id: number;
  method: string;
  params?: unknown;
}

function createRuntimeBridge(): RuntimeApi & { handleResponse(response: RuntimeResponse): void } {
  let nextId = 1;
  const pending = new Map<number, { resolve(value: unknown): void; reject(error: unknown): void }>();

  function request<TResult>(method: string, params: unknown): Promise<TResult> {
    const id = nextId++;
    parentPort!.postMessage({ type: "runtime.request", id, method, params });
    return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
  }

  return {
    server: { ping: () => request("server.ping", {}) },
    documents: {
      openFile: (params: Parameters<RuntimeApi["documents"]["openFile"]>[0]) => request("documents.openFile", params),
      createScratch: (params: Parameters<RuntimeApi["documents"]["createScratch"]>[0]) => request("documents.createScratch", params),
    },
    surfaces: {
      create: (params: Parameters<RuntimeApi["surfaces"]["create"]>[0]) => request("surfaces.create", params),
      setViewport: (params: Parameters<RuntimeApi["surfaces"]["setViewport"]>[0]) => request("surfaces.setViewport", params),
      renderFrame: (params: Parameters<RuntimeApi["surfaces"]["renderFrame"]>[0]) => request("surfaces.renderFrame", params),
    },
    resources: {
      save: (params: Parameters<RuntimeApi["resources"]["save"]>[0]) => request("resources.save", params),
    },
    handleResponse(response: RuntimeResponse) {
      const entry = pending.get(response.id);
      if (!entry) return;
      pending.delete(response.id);
      if (response.error) entry.reject(response.error);
      else entry.resolve(response.result);
    },
  };
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function serializeError(error: unknown): { message: string; stack?: string } {
  if (error instanceof Error) return { message: error.message, stack: error.stack };
  return { message: String(error) };
}

void ({} satisfies JsonValue);
