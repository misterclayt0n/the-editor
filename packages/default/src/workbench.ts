import type { EditorModule, KeyInput } from "@the-editor/editor";
import type { WorkbenchSessionState, WorkbenchSurfaceConfigureRequest } from "@the-editor/editor/workbench";
import { toDisposable } from "@the-editor/editor";
import { defaultViewport } from "./scratch.js";
import { getDefaultState, handleKeyInput, refreshActiveSurface } from "./state.js";

export function defaultWorkbench(): EditorModule {
  return (editor) => {
    const boot = editor.hooks.tap("workbench.boot", {
      id: "default.workbench.boot",
      async handler() {
        const state = getDefaultState(editor);
        if (state.activeSurfaceId) return;
        await editor.commands.dispatch({
          id: "scratch.create",
          args: { text: "", viewport: defaultViewport() },
        });
      },
    });

    const keyInput = editor.hooks.tap("workbench.keyInput", {
      id: "default.workbench.keyInput",
      async handler(input: KeyInput) {
        await handleKeyInput(editor, input);
        return true;
      },
    });

    const surfaceConfigure = editor.hooks.tap("workbench.surfaceConfigure", {
      id: "default.workbench.surfaceConfigure",
      async handler(request: WorkbenchSurfaceConfigureRequest) {
        const state = getDefaultState(editor);
        if (!state.activeSurfaceId) throw new Error("no active surface");
        if (request.metrics) {
          state.surfaceMetrics = request.metrics;
          await editor.runtime.surfaces.setMetrics({
            surfaceId: state.activeSurfaceId,
            metrics: request.metrics,
          });
        }
        if (request.viewport) state.surfaceViewport = request.viewport;
        await refreshActiveSurface(editor, request.viewport);
        return true;
      },
    });

    const sessionState = editor.hooks.tap("workbench.sessionState", {
      id: "default.workbench.sessionState",
      async handler(): Promise<WorkbenchSessionState> {
        const state = getDefaultState(editor);
        await refreshActiveSurface(editor);
        return {
          mode: state.mode,
          activeDocumentId: state.activeDocumentId,
          activeSurfaceId: state.activeSurfaceId,
          activePath: state.activePath,
          softWrap: state.softWrap,
          frame: state.lastFrame,
          roles: editor.roles.snapshotAll(),
          tools: editor.tools.snapshotAll(),
          decorations: state.decorations,
          diagnostics: editor.diagnostics.all(),
          focus: editor.focus.current(),
          theme: editor.themes.snapshot(),
          syntaxLegend: state.syntaxLegend,
          sessions: editor.sessions.snapshotAll(),
          notifications: editor.notifications.snapshot(),
          workingDirectory: editor.cwd.snapshot(),
          selectionPickerActive: Boolean(editor.getStateSlot("default.editing.selectionPicker")),
        };
      },
    });

    return toDisposable(() => {
      boot.dispose();
      keyInput.dispose();
      surfaceConfigure.dispose();
      sessionState.dispose();
    });
  };
}
