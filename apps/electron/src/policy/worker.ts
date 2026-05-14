import { parentPort, workerData } from "node:worker_threads";
import type { KeyInput } from "@the-editor/editor";
import { createEditorHost, createRuntimeClient } from "@the-editor/editor";
import { bootWorkbench, configureWorkbenchSurface, getWorkbenchSessionState, handleWorkbenchKeyInput, type WorkbenchSessionState } from "@the-editor/editor/workbench";
import { ElectronPackageDiscovery } from "./packageDiscovery.js";
import { createPolicyPackageTargetAdapters } from "./packageTargetAdapters.js";
import { PolicyTransactionController } from "./transactions.js";

if (!parentPort) throw new Error("policy worker requires parentPort");

const runtime = createRuntimeClient();
const initialCwd = typeof workerData?.cwd === "string" ? workerData.cwd : process.cwd();
const editor = createEditorHost({ runtime, cwd: initialCwd, packageTargetAdapters: createPolicyPackageTargetAdapters() });
const packageDiscovery = new ElectronPackageDiscovery(
  typeof workerData?.root === "string" ? workerData.root : process.cwd(),
  () => editor.cwd.current(),
);

editor.hooks.tap("packages.discover", {
  id: "electron.discovery",
  handler: async () => {
    await packageDiscovery.sync(editor);
  },
});

const transactions = new PolicyTransactionController({
  editor,
  buildState: () => getWorkbenchSessionState(editor),
  publish: (payload) => {
    parentPort?.postMessage(payload);
  },
  toPayload: (state) => buildSessionPayload(state),
  onError: (error) => {
    parentPort?.postMessage({ type: "policy.error", ...serializeError(error) });
  },
});

editor.sessions.subscribe((event) => {
  if (event.type === "updated") {
    parentPort?.postMessage({ type: "default.sessionUpdated", session: event.session });
    return;
  }
  parentPort?.postMessage({ type: "default.sessionDisposed", sessionId: event.sessionId });
});

parentPort.on("message", (message: unknown) => {
  if (!isObject(message)) return;
  if (message.type === "policy.request") {
    void handlePolicyRequest(message as unknown as PolicyRequest);
  }
});

void boot();

async function boot(): Promise<void> {
  try {
    await editor.packages.discover();
    await editor.packages.loadAll();
    await bootWorkbench(editor);
    await transactions.publishNow();
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
        result = (await transactions.run(async () => {
          await editor.commands.dispatch({ id: "scratch.create", args: request.params });
        })).payload;
        break;
      }
      case "file.open": {
        result = (await transactions.run(async () => {
          await editor.commands.dispatch({ id: "file.open", args: request.params });
        })).payload;
        break;
      }
      case "view.toggleSoftWrap": {
        result = (await transactions.run(async () => {
          await editor.commands.dispatch({ id: "view.toggleSoftWrap" });
        })).payload;
        break;
      }
      case "input.keyDown": {
        const input = request.params as KeyInput;
        const transaction = await transactions.run(async () => {
          const handled = await handleWorkbenchKeyInput(editor, input);
          if (!handled) throw new Error("no workbench key input handler");
        });
        result = transaction.payload;
        break;
      }
      case "surface.configure": {
        const transaction = await transactions.run(async () => {
          const handled = await configureWorkbenchSurface(editor, request.params as SurfaceConfigureParams);
          if (!handled) throw new Error("no workbench surface configure handler");
        });
        result = transaction.payload;
        break;
      }
      case "command.dispatch": {
        const params = request.params as { id: string; args?: unknown };
        result = (await transactions.run(async () => {
          await editor.commands.dispatch({ id: params.id, args: params.args });
        })).payload;
        break;
      }
      case "session.dispatch": {
        const params = request.params as { sessionId: string; method: string; args?: unknown };
        result = await editor.sessions.dispatch(params.sessionId, params.method, params.args);
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

function buildSessionPayload(state: WorkbenchSessionState) {
  return {
    type: "default.sessionChanged",
    roles: state.roles ?? {},
    tools: state.tools ?? [],
    focus: state.focus ?? editor.input.current(),
    packages: editor.packages.snapshot(),
    diagnostics: state.diagnostics ?? [],
    interaction: editor.interaction.snapshot(),
    theme: state.theme ?? editor.themes.snapshot(),
    syntaxLegend: state.syntaxLegend ?? {},
    sessions: state.sessions ?? editor.sessions.snapshotAll(),
    notifications: state.notifications ?? editor.notifications.snapshot(),
    workingDirectory: state.workingDirectory ?? editor.cwd.snapshot(),
    selectionPickerActive: state.selectionPickerActive ?? false,
    editorLayout: state.editorLayout,
  };
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
