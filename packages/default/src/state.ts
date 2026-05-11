import type { CellGridMetrics, DocumentId, RenderFrame, SurfaceId, Viewport } from "@the-editor/contracts/runtime";
import { commandId, type BufferDecoration, type EditorHost, type KeyInput, type RoleSnapshot, type WorkbenchPaneId } from "@the-editor/editor";

export type EditorMode = "normal" | "insert" | "select";

export interface SurfacePaneViewState {
  paneId: WorkbenchPaneId;
  surfaceId: SurfaceId;
  documentId?: DocumentId;
  path?: string;
  title: string;
  viewport: Viewport;
  metrics?: CellGridMetrics;
  frame?: RenderFrame;
}

export interface DefaultEditorState {
  mode: EditorMode;
  activeDocumentId: DocumentId | undefined;
  activeSurfaceId: SurfaceId | undefined;
  activePath: string | undefined;
  activePaneId: WorkbenchPaneId | undefined;
  softWrap: boolean;
  preferredColumn: number | undefined;
  lastFrame: RenderFrame | undefined;
  decorations: BufferDecoration[];
  roleStates: Record<string, RoleSnapshot>;
  syntaxLegend: Record<number, string>;
  surfaceViewport?: Viewport;
  surfaceMetrics?: CellGridMetrics;
  currentRegister?: string;
  surfacePanes: Map<WorkbenchPaneId, SurfacePaneViewState>;
}

const defaultEditorStateSlot = "default.editor.state";

export function getDefaultState(editor: EditorHost): DefaultEditorState {
  return editor.stateSlot(defaultEditorStateSlot, () => ({
    mode: "normal",
    activeDocumentId: undefined,
    activeSurfaceId: undefined,
    activePath: undefined,
    activePaneId: undefined,
    softWrap: false,
    preferredColumn: undefined,
    lastFrame: undefined,
    decorations: [],
    roleStates: {},
    syntaxLegend: {},
    surfaceViewport: undefined,
    surfaceMetrics: undefined,
    currentRegister: undefined,
    surfacePanes: new Map(),
  }));
}

export function setEditorMode(editor: EditorHost, mode: EditorMode): DefaultEditorState {
  const state = getDefaultState(editor);
  state.mode = mode;
  return state;
}

export function activeSurfacePane(editor: EditorHost): SurfacePaneViewState | undefined {
  const state = getDefaultState(editor);
  syncActiveSurfaceFromWorkbench(editor, state);
  return state.activePaneId ? state.surfacePanes.get(state.activePaneId) : undefined;
}

export function surfacePaneState(editor: EditorHost, paneId: WorkbenchPaneId | string): SurfacePaneViewState | undefined {
  return getDefaultState(editor).surfacePanes.get(paneId as WorkbenchPaneId);
}

export function upsertSurfacePaneState(editor: EditorHost, pane: SurfacePaneViewState): SurfacePaneViewState {
  getDefaultState(editor).surfacePanes.set(pane.paneId, pane);
  syncActiveSurfaceFromWorkbench(editor);
  return pane;
}

export function removeSurfacePaneState(editor: EditorHost, paneId: WorkbenchPaneId | string): void {
  const state = getDefaultState(editor);
  state.surfacePanes.delete(paneId as WorkbenchPaneId);
  syncActiveSurfaceFromWorkbench(editor, state);
}

export async function refreshActiveSurface(editor: EditorHost, viewport?: Viewport): Promise<DefaultEditorState> {
  const state = getDefaultState(editor);
  syncActiveSurfaceFromWorkbench(editor, state);

  if (viewport && state.activePaneId) {
    const activePane = state.surfacePanes.get(state.activePaneId);
    if (activePane) activePane.viewport = viewport;
  }

  for (const pane of state.surfacePanes.values()) {
    pane.frame = await editor.runtime.surfaces.renderFrame({ surfaceId: pane.surfaceId });
  }

  state.lastFrame = state.activePaneId ? state.surfacePanes.get(state.activePaneId)?.frame : undefined;
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

function shouldRefreshSyntaxLegend(state: DefaultEditorState): boolean {
  if (!Object.keys(state.syntaxLegend).length) return true;
  const kinds = state.lastFrame?.syntax ?? [];
  return kinds.some((span) => state.syntaxLegend[span.kind] === undefined);
}

function syncActiveSurfaceFromWorkbench(editor: EditorHost, state = getDefaultState(editor)): void {
  const activePane = editor.workbench.activePane();
  if (activePane?.kind !== "surface") {
    state.activePaneId = undefined;
    state.activeDocumentId = undefined;
    state.activeSurfaceId = undefined;
    state.activePath = undefined;
    state.lastFrame = undefined;
    return;
  }

  const paneState = state.surfacePanes.get(activePane.id);
  state.activePaneId = activePane.id;
  state.activeDocumentId = activePane.documentId;
  state.activeSurfaceId = activePane.surfaceId;
  state.activePath = activePane.path;
  state.surfaceViewport = paneState?.viewport;
  state.surfaceMetrics = paneState?.metrics;
  state.lastFrame = paneState?.frame;
}
