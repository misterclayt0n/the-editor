import { parentPort } from "node:worker_threads";
import { activate, getScratchState } from "@the-editor/default";
import { createEditorHost, createRuntimeClient } from "@the-editor/editor";

if (!parentPort) throw new Error("policy worker requires parentPort");

const runtime = createRuntimeClient();
const editor = createEditorHost({ runtime });

parentPort.on("message", (message: unknown) => {
  if (!isObject(message)) return;
  if (message.type === "policy.request") {
    void handlePolicyRequest(message as unknown as PolicyRequest);
  }
});

void boot();

async function boot(): Promise<void> {
  try {
    await activate(editor);
    parentPort?.postMessage({ type: "policy.ready" });
  } catch (error) {
    parentPort?.postMessage({ type: "policy.error", ...serializeError(error) });
  }
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

interface PolicyRequest {
  type: "policy.request";
  id: number;
  method: string;
  params?: unknown;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function serializeError(error: unknown): { message: string; stack?: string } {
  if (error instanceof Error) return { message: error.message, stack: error.stack };
  return { message: String(error) };
}
