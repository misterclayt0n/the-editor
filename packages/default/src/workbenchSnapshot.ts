import type { CellGridMetrics, DocumentId, RenderFrame, SurfaceId, Viewport } from "@the-editor/contracts/runtime";
import type { BufferDecoration, EditorHost, GutterSnapshot, RoleSnapshot, WorkbenchPaneId } from "@the-editor/editor";
import type { WorkbenchSessionState } from "@the-editor/editor/workbench";

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
  lineCount?: number;
  gutters?: GutterSnapshot[];
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
  gutters: GutterSnapshot[];
  roleStates: Record<string, RoleSnapshot>;
  syntaxLegend: Record<number, string>;
  surfaceViewport?: Viewport;
  surfaceMetrics?: CellGridMetrics;
  currentRegister?: string;
  surfacePanes: Map<WorkbenchPaneId, SurfacePaneViewState>;
}

const defaultEditorStateSlot = "default.editor.state";

export function workbenchSnapshotState(editor: EditorHost): DefaultEditorState {
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
    gutters: [],
    roleStates: {},
    syntaxLegend: {},
    surfaceViewport: undefined,
    surfaceMetrics: undefined,
    currentRegister: undefined,
    surfacePanes: new Map(),
  }));
}

export function setWorkbenchEditorMode(editor: EditorHost, mode: EditorMode): DefaultEditorState {
  const state = workbenchSnapshotState(editor);
  state.mode = mode;
  return state;
}

export function syncWorkbenchActiveSurface(editor: EditorHost, state = workbenchSnapshotState(editor)): void {
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

export function activeWorkbenchSurfacePane(editor: EditorHost): SurfacePaneViewState | undefined {
  const state = workbenchSnapshotState(editor);
  syncWorkbenchActiveSurface(editor, state);
  return state.activePaneId ? state.surfacePanes.get(state.activePaneId) : undefined;
}

export function workbenchSurfacePaneState(editor: EditorHost, paneId: WorkbenchPaneId | string): SurfacePaneViewState | undefined {
  return workbenchSnapshotState(editor).surfacePanes.get(paneId as WorkbenchPaneId);
}

export function upsertWorkbenchSurfacePane(editor: EditorHost, pane: SurfacePaneViewState): SurfacePaneViewState {
  workbenchSnapshotState(editor).surfacePanes.set(pane.paneId, pane);
  syncWorkbenchActiveSurface(editor);
  return pane;
}

export function removeWorkbenchSurfacePane(editor: EditorHost, paneId: WorkbenchPaneId | string): void {
  const state = workbenchSnapshotState(editor);
  state.surfacePanes.delete(paneId as WorkbenchPaneId);
  syncWorkbenchActiveSurface(editor, state);
}

export async function refreshWorkbenchSnapshot(editor: EditorHost, viewport?: Viewport): Promise<DefaultEditorState> {
  const state = workbenchSnapshotState(editor);
  syncWorkbenchActiveSurface(editor, state);

  if (viewport && state.activePaneId) {
    const activePane = state.surfacePanes.get(state.activePaneId);
    if (activePane) activePane.viewport = viewport;
  }

  for (const pane of visibleSurfacePanes(editor, state)) {
    pane.frame = await editor.runtime.surfaces.renderFrame({ surfaceId: pane.surfaceId });
    const surfaceState = await editor.runtime.surfaces.readState({ surfaceId: pane.surfaceId });
    pane.lineCount = surfaceState.lineCount;
    pane.gutters = await editor.gutters?.collect({
      mode: state.mode,
      activeDocumentId: pane.documentId,
      activeSurfaceId: pane.surfaceId,
      activePath: pane.path,
      frame: pane.frame,
      lineCount: pane.lineCount,
      text: surfaceState.text,
    }) ?? [];
  }

  state.lastFrame = state.activePaneId ? state.surfacePanes.get(state.activePaneId)?.frame : undefined;
  if (shouldRefreshSyntaxLegend(state)) {
    state.syntaxLegend = await editor.runtime.syntax.highlightLegend();
  }
  state.roleStates = editor.roles.snapshotAll();
  const activeSurfaceText = state.activeSurfaceId
    ? await editor.runtime.surfaces.readState({ surfaceId: state.activeSurfaceId }).then((surface) => surface.text).catch(() => undefined)
    : undefined;
  const surfaceContext = {
    mode: state.mode,
    activeDocumentId: state.activeDocumentId,
    activeSurfaceId: state.activeSurfaceId,
    activePath: state.activePath,
    frame: state.lastFrame,
    text: activeSurfaceText,
    lineCount: state.activePaneId ? state.surfacePanes.get(state.activePaneId)?.lineCount : undefined,
  };
  state.decorations = await editor.decorations.collect(surfaceContext);
  state.gutters = state.activePaneId ? state.surfacePanes.get(state.activePaneId)?.gutters ?? [] : [];
  return state;
}

export async function snapshotWorkbenchSession(editor: EditorHost): Promise<WorkbenchSessionState> {
  const state = workbenchSnapshotState(editor);
  const tools = editor.tools.snapshotAll();
  const toolById = new Map(tools.map((tool) => [tool.id, tool] as const));
  return {
    mode: state.mode,
    activeDocumentId: state.activeDocumentId,
    activeSurfaceId: state.activeSurfaceId,
    activePath: state.activePath,
    softWrap: state.softWrap,
    frame: state.lastFrame,
    roles: editor.roles.snapshotAll(),
    tools,
    decorations: state.decorations,
    gutters: state.gutters,
    diagnostics: editor.diagnostics.all(),
    healthDiagnostics: editor.health.all(),
    focus: editor.focus.current(),
    theme: editor.themes.snapshot(),
    syntaxLegend: state.syntaxLegend,
    sessions: editor.sessions.snapshotAll(),
    notifications: editor.notifications.snapshot(),
    workingDirectory: editor.cwd.snapshot(),
    selectionPickerActive: Boolean(editor.getStateSlot("default.editing.selectionPicker")),
    editorLayout: editor.workbench.snapshotEditorLayout({
      mapItem: (item) => decorateItemSnapshot(item, toolById),
      mapPane: (pane) => decoratePaneSnapshot(pane, state, toolById),
    }),
  };
}

function visibleSurfacePanes(editor: EditorHost, state: DefaultEditorState): SurfacePaneViewState[] {
  const layout = editor.workbench.snapshotEditorLayout();
  const paneIds = new Set<WorkbenchPaneId>();
  for (const group of layout.groups) {
    if (group.activePaneId) paneIds.add(group.activePaneId as WorkbenchPaneId);
  }
  if (state.activePaneId) paneIds.add(state.activePaneId);
  return [...paneIds]
    .map((paneId) => state.surfacePanes.get(paneId))
    .filter((pane): pane is SurfacePaneViewState => Boolean(pane));
}

function shouldRefreshSyntaxLegend(state: DefaultEditorState): boolean {
  if (!Object.keys(state.syntaxLegend).length) return true;
  const kinds = state.lastFrame?.syntax ?? [];
  return kinds.some((span) => state.syntaxLegend[span.kind] === undefined);
}

function decorateItemSnapshot(item: Awaited<ReturnType<EditorHost["workbench"]["snapshotEditorLayout"]>>["items"][number], toolById: Map<string, ReturnType<EditorHost["tools"]["snapshotAll"]>[number]>) {
  if (item.kind !== "tool") return item;
  const tool = toolById.get(item.toolId);
  return {
    ...item,
    title: tool?.title ?? item.title,
    packageId: tool?.packageId ?? item.packageId,
    input: tool?.input ?? item.input,
    state: tool?.state,
  };
}

function decoratePaneSnapshot(pane: Awaited<ReturnType<EditorHost["workbench"]["snapshotEditorLayout"]>>["panes"][number], state: DefaultEditorState, toolById: Map<string, ReturnType<EditorHost["tools"]["snapshotAll"]>[number]>) {
  if (pane.kind === "surface") {
    const surface = state.surfacePanes.get(pane.id);
    return {
      ...pane,
      state: surface
        ? {
          frame: surface.frame,
          lineCount: surface.lineCount,
          gutters: surface.gutters,
          viewport: surface.viewport,
          metrics: surface.metrics,
          path: surface.path,
          title: surface.title,
        }
        : undefined,
    };
  }

  const tool = toolById.get(pane.toolId);
  return {
    ...pane,
    packageId: tool?.packageId ?? pane.packageId,
    input: tool?.input ?? pane.input,
    state: tool ? { tool } : undefined,
  };
}
