import type { DocumentId, RenderFrameDelta, SurfaceId, Viewport } from "@the-editor/contracts/runtime";
import { commandId, type EditorHost, type EditorModule } from "@the-editor/editor";

export interface ScratchState {
  activeDocumentId: DocumentId | undefined;
  activeSurfaceId: SurfaceId | undefined;
  lastFrame: RenderFrameDelta | undefined;
}

export interface CreateScratchArgs {
  text?: string;
  viewport?: Viewport;
}

export interface RenderActiveSurfaceArgs {
  viewport?: Viewport;
  sinceGeneration?: number | null;
}

export const scratchState = new WeakMap<EditorHost, ScratchState>();

export function getScratchState(editor: EditorHost): ScratchState {
  let state = scratchState.get(editor);
  if (!state) {
    state = {
      activeDocumentId: undefined,
      activeSurfaceId: undefined,
      lastFrame: undefined,
    };
    scratchState.set(editor, state);
  }
  return state;
}

export function defaultScratch(): EditorModule {
  return (editor) => {
    editor.commands.register<CreateScratchArgs | undefined, ScratchState>({
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

        const state = getScratchState(editor);
        state.activeDocumentId = document.documentId;
        state.activeSurfaceId = surface.surfaceId;
        state.lastFrame = await editor.runtime.surfaces.renderFrame({
          surfaceId: surface.surfaceId,
          sinceGeneration: null,
        });
        await editor.hooks.event("default.activeSurfaceChanged", {
          documentId: state.activeDocumentId,
          surfaceId: state.activeSurfaceId,
          frame: state.lastFrame,
        });
        return state;
      },
    });

    editor.commands.register<RenderActiveSurfaceArgs | undefined, RenderFrameDelta>({
      id: commandId("surface.renderActive"),
      title: "Render Active Surface",
      async run(_ctx, args) {
        const state = getScratchState(editor);
        if (!state.activeSurfaceId) {
          throw new Error("no active surface");
        }
        state.lastFrame = await editor.runtime.surfaces.renderFrame({
          surfaceId: state.activeSurfaceId,
          viewport: args?.viewport,
          sinceGeneration: args?.sinceGeneration ?? state.lastFrame?.generation ?? null,
        });
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
