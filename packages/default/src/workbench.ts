import type { EditorModule, KeyInput } from "@the-editor/editor";
import { commandId, toDisposable, defineHostStore } from "@the-editor/editor";
import { confirmPrompt } from "./confirmPrompt.js";
import type {
  WorkbenchSessionState,
  WorkbenchSurfaceConfigureRequest,
  WorkbenchPaneId,
  WorkbenchSplitId,
} from "@the-editor/editor/workbench";
import { defaultViewport } from "./scratch.js";
import {
  activeSurfacePane,
  buildWorkbenchSessionState,
  getDefaultState,
  handleKeyInput,
  refreshActiveSurface,
  removeSurfacePaneState,
  surfacePaneState,
  upsertSurfacePaneState,
} from "./state.js";

export function defaultWorkbench(): EditorModule {
  return (editor) => {
    editor.commands.register<{ paneId: WorkbenchPaneId | string }, void>({
      id: commandId("workbench.focusPane"),
      title: "Focus Pane",
      hidden: true,
      async run(_ctx, args) {
        editor.workbench.focusPane(args.paneId);
        await refreshActiveSurface(editor);
        editor.workbench.invalidate("frame", "focus", "layout");
      },
    });

    editor.commands.register<{ groupId: string }, void>({
      id: commandId("workbench.focusGroup"),
      title: "Focus Group",
      hidden: true,
      async run(_ctx, args) {
        editor.workbench.focusGroup(args.groupId);
        await refreshActiveSurface(editor);
        editor.workbench.invalidate("frame", "focus", "layout");
      },
    });

    editor.commands.register<{ paneId?: WorkbenchPaneId | string } | undefined, void>({
      id: commandId("workbench.closePane"),
      title: "Close Pane",
      hidden: true,
      async run(_ctx, args) {
        const paneId = args?.paneId ?? editor.workbench.activePane()?.id;
        if (!paneId) return;
        await closePaneWithSavePrompt(editor, paneId);
      },
    });

    editor.commands.register<{ paneId: WorkbenchPaneId | string; placement: unknown }, void>({
      id: commandId("workbench.movePane"),
      title: "Move Pane",
      hidden: true,
      async run(_ctx, args) {
        const previousActivePaneId = getDefaultState(editor).activePaneId;
        const pane = editor.workbench.movePane(args.paneId, args.placement as never);
        if (pane?.kind === "surface" && previousActivePaneId !== pane.id) {
          const paneState = surfacePaneState(editor, pane.id);
          if (paneState?.viewport) {
            await editor.runtime.surfaces.setViewport({ surfaceId: pane.surfaceId, viewport: paneState.viewport });
          }
          if (paneState?.metrics) {
            await editor.runtime.surfaces.setMetrics({ surfaceId: pane.surfaceId, metrics: paneState.metrics });
          }
        }
        await refreshActiveSurface(editor);
        editor.workbench.invalidate("frame", "focus", "layout", "tabs");
      },
    });

    editor.commands.register<{ splitId: WorkbenchSplitId | string; sizes: number[] }, void>({
      id: commandId("workbench.resizeSplit"),
      title: "Resize Split",
      hidden: true,
      run(_ctx, args) {
        editor.workbench.resizeSplit(args.splitId, args.sizes);
      },
    });

    editor.commands.register<void, void>({
      id: commandId("workbench.vsplit"),
      title: "Split Editor Right",
      aliases: ["vsplit"],
      category: "Workbench",
      palette: "topLevel",
      keywords: ["split", "right", "vertical"],
      icon: { kind: "lucide", name: "columns-2" },
      async run() {
        await splitActiveSurface("right");
      },
    });

    editor.commands.register<void, void>({
      id: commandId("workbench.hsplit"),
      title: "Split Editor Down",
      aliases: ["hsplit"],
      category: "Workbench",
      palette: "topLevel",
      keywords: ["split", "down", "horizontal"],
      icon: { kind: "lucide", name: "rows-2" },
      async run() {
        await splitActiveSurface("down");
      },
    });

    const boot = editor.hooks.tap("workbench.boot", {
      id: "default.workbench.boot",
      async handler() {
        editor.workbench.ensureEditorRoot();
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
        if (editor.overlays.isOpen("hover")) {
          editor.stores.set(defineHostStore({ id: "default.hover.state", init: () => undefined }), { visible: false });
          editor.overlays.close("hover", { restoreFocus: false });
          editor.workbench.invalidate("roles");
        }
        const pane = request.paneId ? surfacePaneState(editor, request.paneId) : activeSurfacePane(editor);
        if (!pane) throw new Error("no active surface");
        if (request.metrics) {
          pane.metrics = request.metrics;
          if (state.activePaneId === pane.paneId) state.surfaceMetrics = request.metrics;
          await editor.runtime.surfaces.setMetrics({
            surfaceId: pane.surfaceId,
            metrics: request.metrics,
          });
        }
        if (request.viewport) {
          pane.viewport = request.viewport;
          if (state.activePaneId === pane.paneId) state.surfaceViewport = request.viewport;
          await editor.runtime.surfaces.setViewport({
            surfaceId: pane.surfaceId,
            viewport: request.viewport,
          });
        }
        await refreshActiveSurface(editor, state.activePaneId === pane.paneId ? request.viewport : undefined);
        return true;
      },
    });

    const sessionState = editor.hooks.tap("workbench.sessionState", {
      id: "default.workbench.sessionState",
      async handler(): Promise<WorkbenchSessionState> {
        return buildWorkbenchSessionState(editor);
      },
    });

    return toDisposable(() => {
      boot.dispose();
      keyInput.dispose();
      surfaceConfigure.dispose();
      sessionState.dispose();
    });

    async function splitActiveSurface(direction: "right" | "down"): Promise<void> {
      const state = getDefaultState(editor);
      const activePane = activeSurfacePane(editor);
      const activeWorkbenchPane = editor.workbench.activePane();
      if (activeWorkbenchPane?.kind !== "surface" || !activePane?.documentId) throw new Error("no active surface");
      const source = await editor.runtime.surfaces.readSelection({ surfaceId: activePane.surfaceId });
      const splitViewport = { ...activePane.viewport, firstCol: 0 };
      const surface = await editor.runtime.surfaces.create({
        documentId: activePane.documentId,
        viewport: splitViewport,
      });
      if (activePane.metrics) {
        await editor.runtime.surfaces.setMetrics({
          surfaceId: surface.surfaceId,
          metrics: activePane.metrics,
        });
      }
      if (state.softWrap) {
        await editor.runtime.surfaces.setSoftWrap({ surfaceId: surface.surfaceId, softWrap: true });
      }
      await editor.runtime.surfaces.applyEdit({
        surfaceId: surface.surfaceId,
        edit: { selection: source.selection },
      });
      const pane = editor.workbench.openSurface({
        title: activePane.title,
        surfaceId: surface.surfaceId,
        documentId: activePane.documentId,
        path: activePane.path,
        closeable: true,
      }, {
        placement: { kind: "split", direction, of: activePane.paneId },
        itemId: activeWorkbenchPane.itemId,
      });
      upsertSurfacePaneState(editor, {
        paneId: pane.id,
        surfaceId: surface.surfaceId,
        documentId: activePane.documentId,
        path: activePane.path,
        title: activePane.title,
        viewport: splitViewport,
        metrics: activePane.metrics,
      });
      await refreshActiveSurface(editor);
      editor.workbench.invalidate("frame", "focus", "layout");
    }
  };
}

async function closePaneWithSavePrompt(editor: Parameters<EditorModule>[0], paneId: WorkbenchPaneId | string): Promise<void> {
  const pane = editor.workbench.findPane(paneId);
  if (!pane) return;

  if (pane.kind === "surface") {
    const status = await editor.runtime.resources.status({ surfaceId: pane.surfaceId }).catch(() => undefined);
    if (status?.dirty && status.path) {
      const label = status.path ?? pane.path ?? pane.title;
      if (status.deletedOnDisk) {
        const action = await confirmPrompt(editor, {
          level: "warning",
          message: `${label} has been deleted on disk and contains unsaved edits. Do you want to save it?`,
          actions: ["Save", "Close", "Cancel"],
        });
        if (action === "Save") {
          await editor.runtime.resources.save({ surfaceId: pane.surfaceId, conflict: "overwrite" });
        } else if (action === "Close") {
          // close without saving
        } else {
          return;
        }
      } else if (status.conflict) {
        const action = await confirmPrompt(editor, {
          level: "warning",
          message: "This file has changed on disk since you started editing it. Do you want to overwrite it?",
          actions: ["Overwrite", "Discard", "Cancel"],
        });
        if (action === "Overwrite") {
          await editor.runtime.resources.save({ surfaceId: pane.surfaceId, conflict: "overwrite" });
        } else if (action === "Discard") {
          await editor.runtime.resources.reload({ surfaceId: pane.surfaceId });
        } else {
          return;
        }
      } else {
        const action = await confirmPrompt(editor, {
          level: "warning",
          message: `${label} contains unsaved edits. Do you want to save it?`,
          actions: ["Save", "Don't Save", "Cancel"],
        });
        if (action === "Save") {
          await editor.runtime.resources.save({ surfaceId: pane.surfaceId });
        } else if (action === "Don't Save") {
          // close without saving
        } else {
          return;
        }
      }
    }
  }

  if (pane.kind === "surface") {
    await editor.runtime.surfaces.destroy({ surfaceId: pane.surfaceId }).catch(() => undefined);
  }
  editor.workbench.closePane(paneId);
  if (pane.kind === "surface") removeSurfacePaneState(editor, pane.id);
  await refreshActiveSurface(editor);
  editor.workbench.invalidate("frame", "focus", "layout", "tabs", "tools");
}

