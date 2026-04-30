import { parentPort } from "node:worker_threads";
import { activate, defaultViewport, getDefaultState, handleKeyInput } from "@the-editor/default";
import type { KeyInput } from "@the-editor/editor";
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
    await editor.commands.dispatch({
      id: "scratch.create",
      args: { text: "", viewport: defaultViewport() },
    });
    postSessionState();
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
        result = postSessionState();
        break;
      }
      case "input.keyDown": {
        await handleKeyInput(editor, request.params as KeyInput);
        result = postSessionState();
        break;
      }
      case "surface.configure": {
        const state = getDefaultState(editor);
        const params = request.params as SurfaceConfigureParams;
        if (!state.activeSurfaceId) throw new Error("no active surface");
        if (params.metrics) {
          await editor.runtime.surfaces.setMetrics({ surfaceId: state.activeSurfaceId, metrics: params.metrics });
        }
        await editor.commands.dispatch({ id: "surface.renderActive", args: { viewport: params.viewport } });
        result = postSessionState();
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

function postSessionState() {
  const state = getDefaultState(editor);
  const payload = {
    type: "default.sessionChanged",
    mode: state.mode,
    activeDocumentId: state.activeDocumentId,
    activeSurfaceId: state.activeSurfaceId,
    frame: state.lastFrame,
  };
  parentPort?.postMessage(payload);
  return payload;
}

interface PolicyRequest {
  type: "policy.request";
  id: number;
  method: string;
  params?: unknown;
}

interface SurfaceConfigureParams {
  viewport?: { firstRow: number; rowCount: number; firstCol: number; wrapWidth: number };
  metrics?: { cellWidthPx: number; rowHeightPx: number };
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function serializeError(error: unknown): { message: string; stack?: string } {
  if (error instanceof Error) return { message: error.message, stack: error.stack };
  return { message: String(error) };
}
