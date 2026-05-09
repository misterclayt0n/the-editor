import type { RenderFrame, Viewport } from "@the-editor/contracts/runtime";
import { commandId, type EditorHost, type EditorModule } from "@the-editor/editor";
import { getDefaultState, refreshActiveSurface, setEditorMode, type DefaultEditorState } from "./state.js";

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
        const viewport = args?.viewport ?? state.surfaceViewport ?? defaultViewport();
        const document = await editor.runtime.documents.createScratch({
          text: args?.text ?? "",
        });
        const surface = await editor.runtime.surfaces.create({
          documentId: document.documentId,
          viewport,
        });
        if (state.surfaceMetrics) {
          await editor.runtime.surfaces.setMetrics({
            surfaceId: surface.surfaceId,
            metrics: state.surfaceMetrics,
          });
        }
        if (state.softWrap) {
          await editor.runtime.surfaces.setSoftWrap({ surfaceId: surface.surfaceId, softWrap: true });
        }

        state.activeDocumentId = document.documentId;
        state.activeSurfaceId = surface.surfaceId;
        state.activePath = undefined;
        setEditorMode(editor, "normal");
        editor.focus.setEditor();
        await refreshActiveSurface(editor, viewport);
        return state;
      },
    });

    editor.commands.register<OpenFileArgs, DefaultEditorState>({
      id: commandId("file.open"),
      title: "Open File",
      async run(_ctx, args) {
        const state = getDefaultState(editor);
        const viewport = args.viewport ?? state.surfaceViewport ?? defaultViewport();
        const document = await editor.runtime.documents.openFile({ path: args.path });
        const surface = await editor.runtime.surfaces.create({
          documentId: document.documentId,
          viewport,
        });
        if (state.surfaceMetrics) {
          await editor.runtime.surfaces.setMetrics({
            surfaceId: surface.surfaceId,
            metrics: state.surfaceMetrics,
          });
        }
        if (state.softWrap) {
          await editor.runtime.surfaces.setSoftWrap({ surfaceId: surface.surfaceId, softWrap: true });
        }

        state.activeDocumentId = document.documentId;
        state.activeSurfaceId = surface.surfaceId;
        state.activePath = args.path;
        setEditorMode(editor, "normal");
        editor.focus.setEditor();
        await refreshActiveSurface(editor, viewport);
        return state;
      },
    });

    editor.commands.register<undefined, DefaultEditorState>({
      id: commandId("view.toggleSoftWrap"),
      title: "Toggle Soft Wrap",
      async run() {
        const state = getDefaultState(editor);
        if (!state.activeSurfaceId) throw new Error("no active surface");
        state.softWrap = !state.softWrap;
        await editor.runtime.surfaces.setSoftWrap({ surfaceId: state.activeSurfaceId, softWrap: state.softWrap });
        await refreshActiveSurface(editor);
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
