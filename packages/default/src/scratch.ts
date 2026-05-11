import type { RenderFrame, Viewport } from "@the-editor/contracts/runtime";
import { commandId, type EditorHost, type EditorModule, type WorkbenchPaneDescriptor } from "@the-editor/editor";
import {
  activeSurfacePane,
  getDefaultState,
  refreshActiveSurface,
  setEditorMode,
  upsertSurfacePaneState,
  type DefaultEditorState,
} from "./state.js";
import { resolveFilePathFromWorkingDirectory } from "./workingDirectory.js";

export interface CreateScratchArgs {
  text?: string;
  viewport?: Viewport;
}

export interface OpenFileArgs {
  path: string;
  viewport?: Viewport;
}

export interface RenderActiveSurfaceArgs {
  viewport?: Viewport;
}

export function defaultScratch(): EditorModule {
  return (editor) => {
    editor.commands.register<CreateScratchArgs | undefined, DefaultEditorState>({
      id: commandId("scratch.create"),
      title: "Create Scratch Buffer",
      async run(_ctx, args) {
        const state = getDefaultState(editor);
        const activePane = activeSurfacePane(editor);
        const viewport = args?.viewport ?? activePane?.viewport ?? state.surfaceViewport ?? defaultViewport();
        const metrics = activePane?.metrics ?? state.surfaceMetrics;
        const document = await editor.runtime.documents.createScratch({
          text: args?.text ?? "",
        });
        const surface = await editor.runtime.surfaces.create({
          documentId: document.documentId,
          viewport,
        });
        if (metrics) {
          await editor.runtime.surfaces.setMetrics({
            surfaceId: surface.surfaceId,
            metrics,
          });
        }
        if (state.softWrap) {
          await editor.runtime.surfaces.setSoftWrap({ surfaceId: surface.surfaceId, softWrap: true });
        }

        const pane = editor.workbench.openSurface({
          title: "Scratch",
          surfaceId: surface.surfaceId,
          documentId: document.documentId,
          path: undefined,
          closeable: true,
        });
        upsertSurfacePaneState(editor, {
          paneId: pane.id,
          surfaceId: surface.surfaceId,
          documentId: document.documentId,
          path: undefined,
          title: "Scratch",
          viewport,
          metrics,
        });
        setEditorMode(editor, "normal");
        editor.focus.setEditor();
        await refreshActiveSurface(editor, viewport);
        editor.workbench.invalidate("frame", "focus", "layout", "tabs");
        return state;
      },
    });

    editor.commands.register<OpenFileArgs, DefaultEditorState>({
      id: commandId("file.open"),
      title: "Open File",
      async run(_ctx, args) {
        const state = getDefaultState(editor);
        const activePane = activeSurfacePane(editor);
        const viewport = args.viewport ?? activePane?.viewport ?? state.surfaceViewport ?? defaultViewport();
        const metrics = activePane?.metrics ?? state.surfaceMetrics;
        const path = resolveFilePathFromWorkingDirectory(editor, args.path);
        const existingPane = findOpenFilePane(editor, path);
        if (existingPane) {
          editor.workbench.focusPane(existingPane.id);
          setEditorMode(editor, "normal");
          editor.focus.setEditor();
          await refreshActiveSurface(editor);
          editor.workbench.invalidate("frame", "focus", "layout", "tabs");
          return state;
        }

        const document = await editor.runtime.documents.openFile({ path });
        const surface = await editor.runtime.surfaces.create({
          documentId: document.documentId,
          viewport,
        });
        if (metrics) {
          await editor.runtime.surfaces.setMetrics({
            surfaceId: surface.surfaceId,
            metrics,
          });
        }
        if (state.softWrap) {
          await editor.runtime.surfaces.setSoftWrap({ surfaceId: surface.surfaceId, softWrap: true });
        }

        const title = fileTitle(path);
        const pane = await shouldReplaceInitialScratch(editor)
          ? editor.workbench.replaceActiveSurface({
            title,
            surfaceId: surface.surfaceId,
            documentId: document.documentId,
            path,
            closeable: true,
          })
          : editor.workbench.openSurface({
            title,
            surfaceId: surface.surfaceId,
            documentId: document.documentId,
            path,
            closeable: true,
          });
        upsertSurfacePaneState(editor, {
          paneId: pane.id,
          surfaceId: surface.surfaceId,
          documentId: document.documentId,
          path,
          title,
          viewport,
          metrics,
        });
        setEditorMode(editor, "normal");
        editor.focus.setEditor();
        await refreshActiveSurface(editor, viewport);
        editor.workbench.invalidate("frame", "focus", "layout", "tabs");
        return state;
      },
    });

    editor.commands.register<undefined, DefaultEditorState>({
      id: commandId("view.toggleSoftWrap"),
      title: "Toggle Soft Wrap",
      async run() {
        const state = getDefaultState(editor);
        if (!state.surfacePanes.size) throw new Error("no active surface");
        state.softWrap = !state.softWrap;
        for (const pane of state.surfacePanes.values()) {
          await editor.runtime.surfaces.setSoftWrap({ surfaceId: pane.surfaceId, softWrap: state.softWrap });
        }
        await refreshActiveSurface(editor);
        editor.workbench.invalidate("frame");
        return state;
      },
    });

    editor.commands.register<RenderActiveSurfaceArgs | undefined, RenderFrame>({
      id: commandId("surface.renderActive"),
      title: "Render Active Surface",
      async run(_ctx, args) {
        const state = await refreshActiveSurface(editor, args?.viewport);
        if (!state.lastFrame) {
          throw new Error("no active surface");
        }
        return state.lastFrame;
      },
    });
  };
}

export function defaultViewport(): Viewport {
  return {
    firstRow: 0,
    rowCount: 24,
    firstCol: 0,
    wrapWidth: 80,
  };
}

async function shouldReplaceInitialScratch(editor: EditorHost): Promise<boolean> {
  const activePane = activeSurfacePane(editor);
  if (!activePane || activePane.path || activePane.title !== "Scratch") return false;
  const layout = editor.workbench.snapshotEditorLayout();
  if (layout.panes.length !== 1) return false;
  const state = await editor.runtime.surfaces.readState({ surfaceId: activePane.surfaceId });
  return state.text.length === 0;
}

function findOpenFilePane(editor: EditorHost, path: string): Extract<WorkbenchPaneDescriptor, { kind: "surface" }> | undefined {
  const item = editor.workbench.allItems().find((candidate) => candidate.kind === "surface" && candidate.path === path);
  if (!item) return undefined;
  return editor.workbench.findPanesForItem(item.id).find((pane): pane is Extract<WorkbenchPaneDescriptor, { kind: "surface" }> => pane.kind === "surface");
}

function fileTitle(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  const slash = normalized.lastIndexOf("/");
  return slash >= 0 ? normalized.slice(slash + 1) : normalized;
}
