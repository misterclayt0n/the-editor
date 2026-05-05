import type { DocumentId, RenderFrame, SurfaceId, Viewport } from "@the-editor/contracts/runtime";
import { commandId, type EditorHost, type KeyInput } from "@the-editor/editor";

export type EditorMode = "normal" | "insert" | "select";

export interface DefaultEditorState {
  mode: EditorMode;
  activeDocumentId: DocumentId | undefined;
  activeSurfaceId: SurfaceId | undefined;
  activePath: string | undefined;
  softWrap: boolean;
  preferredColumn: number | undefined;
  lastFrame: RenderFrame | undefined;
}

const defaultState = new WeakMap<EditorHost, DefaultEditorState>();

export function getDefaultState(editor: EditorHost): DefaultEditorState {
  let state = defaultState.get(editor);
  if (!state) {
    state = {
      mode: "normal",
      activeDocumentId: undefined,
      activeSurfaceId: undefined,
      activePath: undefined,
      softWrap: false,
      preferredColumn: undefined,
      lastFrame: undefined,
    };
    defaultState.set(editor, state);
  }
  return state;
}

export function setEditorMode(editor: EditorHost, mode: EditorMode): DefaultEditorState {
  const state = getDefaultState(editor);
  state.mode = mode;
  return state;
}

export async function refreshActiveSurface(editor: EditorHost, viewport?: Viewport): Promise<DefaultEditorState> {
  const state = getDefaultState(editor);
  if (!state.activeSurfaceId) return state;
  state.lastFrame = await editor.runtime.surfaces.renderFrame({
    surfaceId: state.activeSurfaceId,
    viewport,
  });
  return state;
}

export async function handleKeyInput(editor: EditorHost, input: KeyInput): Promise<DefaultEditorState> {
  const state = getDefaultState(editor);
  const result = await editor.keymaps.press(state.mode, input);

  if (result.kind === "matched") {
    for (const dispatch of result.dispatches) {
      await editor.commands.dispatch(dispatch);
    }
    await refreshActiveSurface(editor);
    return state;
  }

  if (state.mode === "insert" && input.text) {
    await editor.commands.dispatch({
      id: commandId("edit.insertText"),
      args: { text: input.text },
    });
    await refreshActiveSurface(editor);
  }

  return state;
}
