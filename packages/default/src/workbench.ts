import type { EditorModule, KeyInput } from "@the-editor/editor";
import { commandId, matchesCommandPaletteItem, toDisposable, type CommandPaletteItem, defineHostStore } from "@the-editor/editor";
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

    editor.commands.register<{ paneId: WorkbenchPaneId | string }, void>({
      id: commandId("workbench.closePane"),
      title: "Close Pane",
      hidden: true,
      async run(_ctx, args) {
        const pane = editor.workbench.findPane(args.paneId);
        editor.workbench.closePane(args.paneId);
        if (pane?.kind === "surface") removeSurfacePaneState(editor, pane.id);
        await refreshActiveSurface(editor);
        editor.workbench.invalidate("frame", "focus", "layout", "tools");
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
      category: "Workbench",
      keywords: [":vsplit", "vsplit", "split", "right", "vertical"],
      async run() {
        await splitActiveSurface("right");
      },
    });

    editor.commands.register<void, void>({
      id: commandId("workbench.hsplit"),
      title: "Split Editor Down",
      category: "Workbench",
      keywords: [":hsplit", "hsplit", "split", "down", "horizontal"],
      async run() {
        await splitActiveSurface("down");
      },
    });

    editor.palette.registerProvider({
      id: "default.splits",
      title: "Splits",
      priority: 90,
      provide(_ctx, query) {
        const items: CommandPaletteItem[] = [
          {
            id: "split:vsplit",
            title: ":vsplit",
            detail: "Split editor right",
            group: "Workbench",
            keywords: [":vsplit", "vsplit", "split", "right", "vertical"],
            command: { id: commandId("workbench.vsplit") },
          },
          {
            id: "split:hsplit",
            title: ":hsplit",
            detail: "Split editor down",
            group: "Workbench",
            keywords: [":hsplit", "hsplit", "split", "down", "horizontal"],
            command: { id: commandId("workbench.hsplit") },
          },
        ];
        return items.filter((item) => matchesCommandPaletteItem(query, item));
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
      const source = await editor.runtime.surfaces.readState({ surfaceId: activePane.surfaceId });
      const surface = await editor.runtime.surfaces.create({
        documentId: activePane.documentId,
        viewport: activePane.viewport,
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
        viewport: activePane.viewport,
        metrics: activePane.metrics,
      });
      await refreshActiveSurface(editor);
      editor.workbench.invalidate("frame", "focus", "layout");
    }
  };
}

