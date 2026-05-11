import type { CellGridMetrics, DocumentId, RenderFrame, SurfaceId, Viewport } from "@the-editor/contracts/runtime";
import { commandId, type BufferDecoration, type EditorHost, type KeyInput, type RoleSnapshot } from "@the-editor/editor";

export type EditorMode = "normal" | "insert" | "select";

export interface DefaultEditorState {
  mode: EditorMode;
  activeDocumentId: DocumentId | undefined;
  activeSurfaceId: SurfaceId | undefined;
  activePath: string | undefined;
  softWrap: boolean;
  preferredColumn: number | undefined;
  lastFrame: RenderFrame | undefined;
  decorations: BufferDecoration[];
  roleStates: Record<string, RoleSnapshot>;
  syntaxLegend: Record<number, string>;
  surfaceViewport?: Viewport;
  surfaceMetrics?: CellGridMetrics;
  currentRegister?: string;
}

const defaultEditorStateSlot = "default.editor.state";

export function getDefaultState(editor: EditorHost): DefaultEditorState {
  return editor.stateSlot(defaultEditorStateSlot, () => ({
    mode: "normal",
    activeDocumentId: undefined,
    activeSurfaceId: undefined,
    activePath: undefined,
    softWrap: false,
    preferredColumn: undefined,
    lastFrame: undefined,
    decorations: [],
    roleStates: {},
    syntaxLegend: {},
    surfaceViewport: undefined,
    surfaceMetrics: undefined,
    currentRegister: undefined,
  }));
}

export function setEditorMode(editor: EditorHost, mode: EditorMode): DefaultEditorState {
  const state = getDefaultState(editor);
  state.mode = mode;
  return state;
}

export async function refreshActiveSurface(editor: EditorHost, viewport?: Viewport): Promise<DefaultEditorState> {
  const state = getDefaultState(editor);
  if (state.activeSurfaceId) {
    state.lastFrame = await editor.runtime.surfaces.renderFrame({
      surfaceId: state.activeSurfaceId,
      viewport,
    });
  }
  if (shouldRefreshSyntaxLegend(state)) {
    state.syntaxLegend = await editor.runtime.syntax.highlightLegend();
  }
  state.roleStates = editor.roles.snapshotAll();
  state.decorations = await editor.decorations.collect({
    mode: state.mode,
    activeDocumentId: state.activeDocumentId,
    activeSurfaceId: state.activeSurfaceId,
    activePath: state.activePath,
    frame: state.lastFrame,
  });
  return state;
}

function shouldRefreshSyntaxLegend(state: DefaultEditorState): boolean {
  if (!Object.keys(state.syntaxLegend).length) return true;
  const kinds = state.lastFrame?.syntax ?? [];
  return kinds.some((span) => state.syntaxLegend[span.kind] === undefined);
}

export async function handleKeyInput(editor: EditorHost, input: KeyInput): Promise<DefaultEditorState> {
  const state = getDefaultState(editor);
  const scopes = editor.focus.scopes(state.mode);
  const result = await editor.keymaps.pressScopes(scopes, input);
  const focus = editor.focus.current();
  const paletteFocused = focus.kind === "role" && focus.role === "commandPalette";
  const editorFocused = focus.kind === "editor";

  if (result.kind === "matched") {
    for (const dispatch of result.dispatches) {
      await editor.commands.dispatch(dispatch);
    }
    await refreshActiveSurface(editor);
    return state;
  }

  if (paletteFocused && input.text) {
    await editor.commands.dispatch({
      id: commandId("command_palette.appendText"),
      args: { text: input.text },
    });
    await refreshActiveSurface(editor);
    return state;
  }

  if (editorFocused && state.mode === "insert" && input.text) {
    await editor.commands.dispatch({
      id: commandId("edit.insertText"),
      args: { text: input.text },
    });
    await refreshActiveSurface(editor);
  }

  return state;
}
