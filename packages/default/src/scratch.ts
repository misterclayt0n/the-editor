import type { RenderFrameDelta, Viewport } from "@the-editor/contracts/runtime";
import { commandId, type EditorHost, type EditorModule } from "@the-editor/editor";
import { getDefaultState, refreshActiveSurface, setEditorMode, type DefaultEditorState } from "./state.js";

export interface CreateScratchArgs {
  text?: string;
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
        const viewport = args?.viewport ?? defaultViewport();
        const document = await editor.runtime.documents.createScratch({
          text: args?.text ?? "",
        });
        const surface = await editor.runtime.surfaces.create({
          documentId: document.documentId,
          viewport,
        });

        const state = getDefaultState(editor);
        state.activeDocumentId = document.documentId;
        state.activeSurfaceId = surface.surfaceId;
        setEditorMode(editor, "normal");
        await refreshActiveSurface(editor, viewport);
        return state;
      },
    });

    editor.commands.register<RenderActiveSurfaceArgs | undefined, RenderFrameDelta>({
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
