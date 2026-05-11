import { parentPort, workerData } from "node:worker_threads";
import type { KeyInput } from "@the-editor/editor";
import { createEditorHost, createRuntimeClient } from "@the-editor/editor";
import { bootWorkbench, configureWorkbenchSurface, getWorkbenchSessionState, handleWorkbenchKeyInput } from "@the-editor/editor/workbench";
import { ElectronPackageDiscovery } from "./packageDiscovery.js";
import { createPolicyPackageTargetAdapters } from "./packageTargetAdapters.js";

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

let sessionPostScheduled = false;
editor.workbench.subscribe(() => {
  scheduleSessionStatePost();
});
editor.sessions.subscribe((event) => {
  if (event.type === "updated") {
    if (event.session.kind === "picker") {
      console.log(`[picker-worker] session.updated ${JSON.stringify({
        id: event.session.id,
        revision: event.session.revision,
        role: event.session.role,
        query: (event.session.state as { query?: unknown }).query,
        selectedIndex: (event.session.state as { selectedIndex?: unknown }).selectedIndex,
        itemCount: Array.isArray((event.session.state as { items?: unknown }).items)
          ? ((event.session.state as { items: unknown[] }).items).length
          : undefined,
      })}`);
    }
    parentPort?.postMessage({ type: "default.sessionUpdated", session: event.session });
    return;
  }
  console.log(`[picker-worker] session.disposed ${JSON.stringify({ sessionId: event.sessionId })}`);
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
    await postSessionState();
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
        result = await postSessionState();
        break;
      }
      case "file.open": {
        await editor.commands.dispatch({ id: "file.open", args: request.params });
        result = await postSessionState();
        break;
      }
      case "view.toggleSoftWrap": {
        await editor.commands.dispatch({ id: "view.toggleSoftWrap" });
        result = await postSessionState();
        break;
      }
      case "input.keyDown": {
        const input = request.params as KeyInput;
        const focusBefore = editor.focus.current();
        if (focusBefore.kind === "role" && focusBefore.role === "picker") {
          console.log(`[picker-worker] input.keyDown ${JSON.stringify({ input, focusBefore })}`);
        }
        const handled = await handleWorkbenchKeyInput(editor, input);
        const focusAfter = editor.focus.current();
        if (focusBefore.kind === "role" && focusBefore.role === "picker") {
          console.log(`[picker-worker] input.keyDown.result ${JSON.stringify({ input, handled, focusAfter })}`);
        }
        if (!handled) throw new Error("no workbench key input handler");
        result = await postSessionState();
        break;
      }
      case "surface.configure": {
        const handled = await configureWorkbenchSurface(editor, request.params as SurfaceConfigureParams);
        if (!handled) throw new Error("no workbench surface configure handler");
        result = await postSessionState();
        break;
      }
      case "command.dispatch": {
        const params = request.params as { id: string; args?: unknown };
        const focusBefore = editor.focus.current();
        const shouldTrace = params.id.startsWith("picker.")
          || params.id === "file_picker"
          || (focusBefore.kind === "role" && focusBefore.role === "picker");
        if (shouldTrace) {
          console.log(`[picker-worker] command.dispatch ${JSON.stringify({
            id: params.id,
            args: params.args,
            focusBefore,
          })}`);
        }
        await editor.commands.dispatch({ id: params.id, args: params.args });
        const focusAfter = editor.focus.current();
        if (shouldTrace) {
          console.log(`[picker-worker] command.dispatch.done ${JSON.stringify({
            id: params.id,
            focusAfter,
            pickerVisible: Boolean(editor.roles.snapshot("picker")?.state && (editor.roles.snapshot("picker")?.state as { visible?: unknown }).visible),
          })}`);
        }
        result = await postSessionState();
        break;
      }
      case "session.dispatch": {
        const params = request.params as { sessionId: string; method: string; args?: unknown };
        console.log(`[picker-worker] session.dispatch ${JSON.stringify(params)}`);
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

function scheduleSessionStatePost(): void {
  if (sessionPostScheduled) return;
  sessionPostScheduled = true;
  queueMicrotask(() => {
    sessionPostScheduled = false;
    void postSessionState().catch((error) => {
      parentPort?.postMessage({ type: "policy.error", ...serializeError(error) });
    });
  });
}

async function postSessionState() {
  const state = await getWorkbenchSessionState(editor);
  const payload = {
    type: "default.sessionChanged",
    mode: state.mode,
    activeDocumentId: state.activeDocumentId,
    activeSurfaceId: state.activeSurfaceId,
    activePath: state.activePath,
    softWrap: state.softWrap,
    frame: state.frame,
    roles: state.roles ?? {},
    tools: state.tools ?? [],
    decorations: state.decorations ?? [],
    focus: state.focus ?? editor.focus.current(),
    packages: editor.packages.snapshot(),
    diagnostics: state.diagnostics ?? [],
    interaction: editor.interaction.snapshot(),
    theme: state.theme ?? editor.themes.snapshot(),
    syntaxLegend: state.syntaxLegend ?? {},
    sessions: state.sessions ?? editor.sessions.snapshotAll(),
    notifications: state.notifications ?? editor.notifications.snapshot(),
    workingDirectory: state.workingDirectory ?? editor.cwd.snapshot(),
    selectionPickerActive: state.selectionPickerActive ?? false,
  };
  const pickerRole = (payload.roles as Record<string, { state?: { visible?: unknown; sessionId?: unknown } } | undefined>).picker;
  const pickerSessions = (payload.sessions as Array<{ id: string; kind: string; revision: number; state?: { query?: unknown; selectedIndex?: unknown; items?: unknown[] } }> )
    .filter((session) => session.kind === "picker")
    .map((session) => ({
      id: session.id,
      revision: session.revision,
      query: session.state?.query,
      selectedIndex: session.state?.selectedIndex,
      itemCount: Array.isArray(session.state?.items) ? session.state.items.length : undefined,
    }));
  console.log(`[picker-worker] postSessionState ${JSON.stringify({
    focus: payload.focus,
    pickerVisible: pickerRole?.state?.visible,
    pickerSessionId: pickerRole?.state?.sessionId,
    pickerSessions,
  })}`);
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
