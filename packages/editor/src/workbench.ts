import type { CellGridMetrics, DocumentId, RenderFrame, SurfaceId, Viewport } from "@the-editor/contracts/runtime";
import type { BufferDecoration } from "./decorations.js";
import type { DiagnosticRecord } from "./diagnostics.js";
import type { EditorHost } from "./editorHost.js";
import type { HealthDiagnosticEntry } from "./health.js";
import type { FocusTarget } from "./focus.js";
import type { GutterSnapshot } from "./gutters.js";
import type { NotificationEntry } from "./notifications.js";
import type { RoleSnapshot } from "./roles.js";
import type { InteractiveSessionSnapshot } from "./sessions.js";
import type { ThemeSnapshot } from "./themes.js";
import type { ToolInstance, ToolRootTarget, ToolSnapshot, ToolTypeId } from "./tools.js";
import type { WorkingDirectorySnapshot } from "./workingDirectory.js";

export interface WorkbenchKeyInput {
  key: string;
  text?: string;
}

export interface WorkbenchSurfaceConfigureRequest {
  paneId?: WorkbenchPaneId;
  viewport?: Viewport;
  metrics?: CellGridMetrics;
}

export type WorkbenchInvalidationKind =
  | "all"
  | "frame"
  | "roles"
  | "tools"
  | "decorations"
  | "gutters"
  | "focus"
  | "interaction"
  | "theme"
  | "notifications"
  | "cwd"
  | "layout"
  | "tabs";

export type WorkbenchItemId = string & { readonly __brand?: "WorkbenchItemId" };
export type WorkbenchPaneId = string & { readonly __brand?: "WorkbenchPaneId" };
export type WorkbenchGroupId = string & { readonly __brand?: "WorkbenchGroupId" };
export type WorkbenchSplitId = string & { readonly __brand?: "WorkbenchSplitId" };
export type WorkbenchSplitAxis = "row" | "column";
export type WorkbenchSplitDirection = "left" | "right" | "up" | "down";
export type WorkbenchDockTarget = Exclude<ToolRootTarget, "editor">;

export interface WorkbenchTabBadge {
  text: string;
  tone?: string;
}

export interface WorkbenchPaneTabState {
  preview?: boolean;
  pinned?: boolean;
  subtitle?: string;
  badges?: WorkbenchTabBadge[];
}

export interface WorkbenchItemBaseDescriptor {
  id: WorkbenchItemId;
  kind: "surface" | "tool";
  typeId: string;
  title: string;
  subtitle?: string;
  path?: string;
  dirty?: boolean;
  closeable: boolean;
  state?: unknown;
}

export interface WorkbenchSurfaceItemDescriptor extends WorkbenchItemBaseDescriptor {
  kind: "surface";
  typeId: "surface";
  documentId?: DocumentId;
}

export interface WorkbenchToolItemDescriptor extends WorkbenchItemBaseDescriptor {
  kind: "tool";
  typeId: ToolTypeId;
  toolId: string;
  packageId?: string;
  input?: unknown;
}

export type WorkbenchItemDescriptor = WorkbenchSurfaceItemDescriptor | WorkbenchToolItemDescriptor;

export interface WorkbenchSurfacePaneDescriptor {
  id: WorkbenchPaneId;
  kind: "surface";
  itemId: WorkbenchItemId;
  tab: WorkbenchPaneTabState;
  title: string;
  surfaceId: SurfaceId;
  documentId?: DocumentId;
  path?: string;
  closeable: boolean;
  state?: unknown;
}

export interface WorkbenchToolPaneDescriptor {
  id: WorkbenchPaneId;
  kind: "tool";
  itemId: WorkbenchItemId;
  tab: WorkbenchPaneTabState;
  title: string;
  toolId: string;
  typeId: ToolTypeId;
  closeable: boolean;
  packageId?: string;
  input?: unknown;
  state?: unknown;
}

export type WorkbenchPaneDescriptor = WorkbenchSurfacePaneDescriptor | WorkbenchToolPaneDescriptor;

export interface WorkbenchGroupSnapshot {
  id: WorkbenchGroupId;
  paneIds: WorkbenchPaneId[];
  activePaneId?: WorkbenchPaneId;
}

export interface WorkbenchGroupNodeSnapshot {
  kind: "group";
  groupId: WorkbenchGroupId;
}

export interface WorkbenchSplitNodeSnapshot {
  kind: "split";
  id: WorkbenchSplitId;
  axis: WorkbenchSplitAxis;
  sizes: number[];
  children: WorkbenchEditorLayoutNodeSnapshot[];
}

export type WorkbenchEditorLayoutNodeSnapshot = WorkbenchGroupNodeSnapshot | WorkbenchSplitNodeSnapshot;

export interface WorkbenchEditorLayoutSnapshot {
  root: WorkbenchEditorLayoutNodeSnapshot | null;
  groups: WorkbenchGroupSnapshot[];
  items: WorkbenchItemDescriptor[];
  panes: WorkbenchPaneDescriptor[];
  activeGroupId?: WorkbenchGroupId;
  activePaneId?: WorkbenchPaneId;
  activeItemId?: WorkbenchItemId;
}

export interface WorkbenchTabPlacement {
  kind: "tab";
  group?: WorkbenchGroupId | "active";
  index?: number;
}

export interface WorkbenchSplitPlacement {
  kind: "split";
  direction: WorkbenchSplitDirection;
  of?: WorkbenchGroupId | WorkbenchPaneId | "active";
}

export interface WorkbenchDockPlacement {
  kind: "dock";
  root: WorkbenchDockTarget;
}

export type WorkbenchPanePlacement = WorkbenchTabPlacement | WorkbenchSplitPlacement;
export type WorkbenchToolPlacement = WorkbenchPanePlacement | WorkbenchDockPlacement;

export interface WorkbenchOpenSurfaceOptions {
  placement?: WorkbenchPanePlacement;
  focus?: boolean;
  itemId?: WorkbenchItemId;
  tab?: WorkbenchPaneTabState;
}

export interface WorkbenchOpenToolOptions {
  title?: string;
  input?: unknown;
  placement?: WorkbenchToolPlacement;
  focus?: boolean;
  itemId?: WorkbenchItemId;
  tab?: WorkbenchPaneTabState;
}

export interface WorkbenchSessionState {
  mode?: string;
  activeDocumentId?: DocumentId;
  activeSurfaceId?: SurfaceId;
  activePath?: string;
  softWrap?: boolean;
  frame?: RenderFrame;
  roles?: Record<string, RoleSnapshot>;
  tools?: ToolSnapshot[];
  decorations?: BufferDecoration[];
  gutters?: GutterSnapshot[];
  diagnostics?: DiagnosticRecord[];
  healthDiagnostics?: HealthDiagnosticEntry[];
  focus?: FocusTarget;
  theme?: ThemeSnapshot;
  syntaxLegend?: Record<number, string>;
  sessions?: InteractiveSessionSnapshot[];
  notifications?: NotificationEntry[];
  workingDirectory?: WorkingDirectorySnapshot;
  selectionPickerActive?: boolean;
  editorLayout?: WorkbenchEditorLayoutSnapshot;
}

interface WorkbenchGroupState {
  id: WorkbenchGroupId;
  paneIds: WorkbenchPaneId[];
  activePaneId?: WorkbenchPaneId;
}

interface WorkbenchSplitNodeState {
  kind: "split";
  id: WorkbenchSplitId;
  axis: WorkbenchSplitAxis;
  sizes: number[];
  children: WorkbenchEditorLayoutNodeState[];
}

interface WorkbenchGroupNodeState {
  kind: "group";
  groupId: WorkbenchGroupId;
}

type WorkbenchEditorLayoutNodeState = WorkbenchSplitNodeState | WorkbenchGroupNodeState;

interface WorkbenchLayoutState {
  nextItem: number;
  nextPane: number;
  nextGroup: number;
  nextSplit: number;
  root: WorkbenchEditorLayoutNodeState | null;
  groups: Map<WorkbenchGroupId, WorkbenchGroupState>;
  items: Map<WorkbenchItemId, WorkbenchItemDescriptor>;
  panes: Map<WorkbenchPaneId, WorkbenchPaneDescriptor>;
  activeGroupId?: WorkbenchGroupId;
  activePaneId?: WorkbenchPaneId;
  activeItemId?: WorkbenchItemId;
}

const workbenchLayoutSlot = "workbench.layout";

export class WorkbenchController {
  private readonly listeners = new Set<(kinds: readonly WorkbenchInvalidationKind[]) => void>();

  constructor(private readonly editor: EditorHost) {}

  invalidate(...kinds: WorkbenchInvalidationKind[]): void {
    const nextKinds = [...new Set<WorkbenchInvalidationKind>(kinds.length ? kinds : ["all"])];
    for (const listener of this.listeners) listener(nextKinds);
  }

  subscribe(listener: (kinds: readonly WorkbenchInvalidationKind[]) => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  ensureEditorRoot(): WorkbenchGroupId {
    const state = this.layoutState();
    if (state.root && state.activeGroupId) return state.activeGroupId;
    const group = this.createGroup(state);
    state.root = { kind: "group", groupId: group.id };
    state.activeGroupId = group.id;
    state.activePaneId = group.activePaneId;
    state.activeItemId = group.activePaneId ? state.panes.get(group.activePaneId)?.itemId : undefined;
    this.invalidate("layout", "focus", "tabs");
    return group.id;
  }

  openSurface(
    descriptor: Omit<WorkbenchSurfacePaneDescriptor, "id" | "kind" | "itemId" | "tab">,
    options: WorkbenchOpenSurfaceOptions = {},
  ): WorkbenchSurfacePaneDescriptor {
    const state = this.layoutState();
    const item = this.upsertSurfaceItem(descriptor, options.itemId);
    const pane: WorkbenchSurfacePaneDescriptor = {
      id: workbenchPaneId(`pane:${state.nextPane++}`),
      kind: "surface",
      itemId: item.id,
      tab: normalizePaneTab(options.tab),
      title: item.title,
      surfaceId: descriptor.surfaceId,
      documentId: descriptor.documentId,
      path: descriptor.path ?? item.path,
      closeable: item.closeable,
      state: descriptor.state,
    };
    this.placePane(pane, options.placement);
    if (options.focus !== false) this.focusPane(pane.id);
    else this.invalidate("layout", "tabs");
    return pane;
  }

  replaceActiveSurface(
    descriptor: Omit<WorkbenchSurfacePaneDescriptor, "id" | "kind" | "itemId" | "tab">,
  ): WorkbenchSurfacePaneDescriptor {
    const state = this.layoutState();
    const activePane = state.activePaneId ? state.panes.get(state.activePaneId) : undefined;
    if (activePane?.kind === "surface") {
      const previousItemId = activePane.itemId;
      const item = this.upsertSurfaceItem(descriptor);
      const nextPane: WorkbenchSurfacePaneDescriptor = {
        id: activePane.id,
        kind: "surface",
        itemId: item.id,
        tab: activePane.tab,
        title: item.title,
        surfaceId: descriptor.surfaceId,
        documentId: descriptor.documentId,
        path: descriptor.path ?? item.path,
        closeable: item.closeable,
        state: descriptor.state,
      };
      state.panes.set(activePane.id, nextPane);
      this.removeOrphanedItem(previousItemId);
      this.focusPane(activePane.id);
      this.invalidate("layout", "focus", "tabs");
      return nextPane;
    }

    const groupId = this.resolveTabGroup(state, { kind: "tab", group: state.activeGroupId ?? "active" });
    if (groupId) state.activeGroupId = groupId;
    return this.openSurface(descriptor, { placement: { kind: "tab", group: groupId ?? "active" } });
  }

  openTool(typeId: ToolTypeId | string, options: WorkbenchOpenToolOptions = {}): ToolInstance {
    const type = this.editor.tools.getType(typeId);
    if (!type) throw new Error(`unknown tool type: ${typeId}`);

    const dockRoot = options.placement?.kind === "dock"
      ? options.placement.root
      : !options.placement && type.root !== "editor"
        ? type.root
        : undefined;

    if (dockRoot) {
      const instance = this.editor.tools.open(type.id, {
        title: options.title,
        input: options.input,
        root: dockRoot,
        focus: options.focus,
      });
      this.invalidate("tools", "focus");
      return instance;
    }

    const instance = this.editor.tools.open(type.id, {
      title: options.title,
      input: options.input,
      root: "editor",
      focus: false,
    });
    const existingPane = this.findPaneForTool(instance.id);
    if (existingPane) {
      this.upsertToolItem(instance, existingPane.itemId);
      if (options.focus !== false) this.focusPane(existingPane.id);
      else this.invalidate("layout", "tools", "tabs");
      return instance;
    }

    const item = this.upsertToolItem(instance, options.itemId);
    const pane: WorkbenchToolPaneDescriptor = {
      id: workbenchPaneId(`pane:${this.layoutState().nextPane++}`),
      kind: "tool",
      itemId: item.id,
      tab: normalizePaneTab(options.tab),
      title: item.title,
      toolId: instance.id,
      typeId: instance.typeId,
      closeable: item.closeable,
      packageId: instance.packageId,
      input: instance.input,
    };
    this.placePane(pane, options.placement && options.placement.kind !== "dock" ? options.placement : undefined);
    if (options.focus !== false) this.focusPane(pane.id);
    else this.invalidate("layout", "tools", "tabs");
    return instance;
  }

  allItems(): WorkbenchItemDescriptor[] {
    return [...this.layoutState().items.values()];
  }

  findItem(id: WorkbenchItemId | string): WorkbenchItemDescriptor | undefined {
    return this.layoutState().items.get(workbenchItemId(id));
  }

  activeItem(): WorkbenchItemDescriptor | undefined {
    const activeItemId = this.layoutState().activeItemId;
    return activeItemId ? this.layoutState().items.get(activeItemId) : undefined;
  }

  updateItem(id: WorkbenchItemId | string, patch: Partial<WorkbenchItemDescriptor>): WorkbenchItemDescriptor | undefined {
    const state = this.layoutState();
    const current = state.items.get(workbenchItemId(id));
    if (!current) return undefined;
    const next = current.kind === "surface"
      ? {
        ...current,
        ...patch,
        id: current.id,
        kind: "surface",
        typeId: "surface",
      } as WorkbenchSurfaceItemDescriptor
      : {
        ...current,
        ...patch,
        id: current.id,
        kind: "tool",
        typeId: current.typeId,
      } as WorkbenchToolItemDescriptor;
    state.items.set(current.id, next);
    this.syncPanesForItem(next);
    this.invalidate("layout", "tabs", "tools");
    return next;
  }

  findPane(id: WorkbenchPaneId | string): WorkbenchPaneDescriptor | undefined {
    return this.layoutState().panes.get(workbenchPaneId(id));
  }

  findPanesForItem(itemId: WorkbenchItemId | string): WorkbenchPaneDescriptor[] {
    const normalized = workbenchItemId(itemId);
    return [...this.layoutState().panes.values()].filter((pane) => pane.itemId === normalized);
  }

  updatePaneTab(id: WorkbenchPaneId | string, patch: Partial<WorkbenchPaneTabState>): WorkbenchPaneDescriptor | undefined {
    const state = this.layoutState();
    const pane = state.panes.get(workbenchPaneId(id));
    if (!pane) return undefined;
    const nextPane: WorkbenchPaneDescriptor = {
      ...pane,
      tab: normalizePaneTab({ ...pane.tab, ...patch }),
    };
    state.panes.set(nextPane.id, nextPane);
    this.invalidate("layout", "tabs");
    return nextPane;
  }

  findPaneForTool(toolId: string): WorkbenchToolPaneDescriptor | undefined {
    for (const pane of this.layoutState().panes.values()) {
      if (pane.kind === "tool" && pane.toolId === toolId) return pane;
    }
    return undefined;
  }

  focusPane(id: WorkbenchPaneId | string): WorkbenchPaneDescriptor | undefined {
    const pane = this.layoutState().panes.get(workbenchPaneId(id));
    if (!pane) return undefined;
    const group = this.groupForPane(pane.id);
    if (!group) return undefined;
    group.activePaneId = pane.id;
    const state = this.layoutState();
    state.activeGroupId = group.id;
    state.activePaneId = pane.id;
    state.activeItemId = pane.itemId;
    if (pane.kind === "tool") this.editor.focus.setTool(pane.toolId);
    else this.editor.focus.setEditor();
    this.invalidate("layout", "focus", "tabs");
    return pane;
  }

  focusGroup(id: WorkbenchGroupId | string): WorkbenchGroupSnapshot | undefined {
    const state = this.layoutState();
    const group = state.groups.get(workbenchGroupId(id));
    if (!group) return undefined;
    state.activeGroupId = group.id;
    state.activePaneId = group.activePaneId;
    state.activeItemId = group.activePaneId ? state.panes.get(group.activePaneId)?.itemId : undefined;
    if (group.activePaneId) {
      this.focusPane(group.activePaneId);
    } else {
      this.editor.focus.setEditor();
      this.invalidate("layout", "focus", "tabs");
    }
    return this.snapshotGroup(group);
  }

  movePane(id: WorkbenchPaneId | string, placement: WorkbenchPanePlacement): WorkbenchPaneDescriptor | undefined {
    const state = this.layoutState();
    const pane = state.panes.get(workbenchPaneId(id));
    if (!pane) return undefined;

    const targetGroupId = placement.kind === "split"
      ? this.resolveSplitTarget(state, placement)
      : this.resolveTabGroup(state, placement);
    if (!targetGroupId) return undefined;

    const sourceGroup = this.groupForPane(pane.id);
    if (sourceGroup?.id === targetGroupId && pane.kind === "surface" && groupHasSurfaceItem(state, sourceGroup, pane.itemId)) {
      return this.focusPane(pane.id);
    }
    if (placement.kind === "tab" && groupHasSurfaceItem(state, state.groups.get(targetGroupId), pane.itemId)) {
      this.removePane(pane.id);
      this.removeOrphanedItem(pane.itemId);
      this.syncFocusToActivePane();
      state.root = collapseSplitTree(pruneEmptyGroups(state.root, state.groups));
      this.invalidate("layout", "focus", "tabs");
      return pane;
    }
    if (placement.kind === "split" && sourceGroup?.id === targetGroupId && sourceGroup.paneIds.length <= 1) {
      return undefined;
    }
    const sourceIndex = sourceGroup?.paneIds.indexOf(pane.id) ?? -1;
    const tabIndex = placement.kind === "tab" && typeof placement.index === "number"
      ? sourceGroup?.id === targetGroupId && sourceIndex >= 0 && placement.index > sourceIndex
        ? placement.index - 1
        : placement.index
      : undefined;

    this.detachPaneFromGroup(pane.id);

    if (placement.kind === "split") {
      const group = this.createGroup(state);
      this.insertTabIntoGroup(pane.id, group.id, undefined);
      state.root = insertSplitForGroup(
        state.root,
        targetGroupId,
        directionAxis(placement.direction),
        splitAfter(placement.direction),
        { kind: "group", groupId: group.id },
        () => workbenchSplitId(`split:${state.nextSplit++}`),
      );
      state.activeGroupId = group.id;
    } else {
      this.insertTabIntoGroup(pane.id, targetGroupId, tabIndex);
      state.activeGroupId = targetGroupId;
    }

    state.activePaneId = pane.id;
    state.activeItemId = pane.itemId;
    this.focusPane(pane.id);
    state.root = collapseSplitTree(pruneEmptyGroups(state.root, state.groups));
    this.invalidate("layout", "focus", "tabs");
    return pane;
  }

  closePane(id: WorkbenchPaneId | string): void {
    const pane = this.findPane(id);
    if (!pane) return;
    if (pane.kind === "tool") {
      if (this.editor.tools.getInstance(pane.toolId)) {
        this.editor.tools.close(pane.toolId);
        return;
      }
      this.removePane(pane.id);
      this.syncFocusToActivePane();
      this.invalidate("layout", "focus", "tools", "tabs");
      return;
    }
    this.removePane(pane.id);
    this.syncFocusToActivePane();
    this.invalidate("layout", "focus", "tools", "tabs");
  }

  closePanesForTool(toolId: string): void {
    let changed = false;
    for (const pane of [...this.layoutState().panes.values()]) {
      if (pane.kind !== "tool" || pane.toolId !== toolId) continue;
      this.removePane(pane.id);
      changed = true;
    }
    if (!changed) return;
    this.syncFocusToActivePane();
    this.invalidate("layout", "focus", "tools", "tabs");
  }

  resizeSplit(id: WorkbenchSplitId | string, sizes: readonly number[]): void {
    const split = findSplitNode(this.layoutState().root, workbenchSplitId(id));
    if (!split) return;
    split.sizes = normalizeSizes(sizes, split.children.length);
    this.invalidate("layout");
  }

  activePane(): WorkbenchPaneDescriptor | undefined {
    const activePaneId = this.layoutState().activePaneId;
    return activePaneId ? this.layoutState().panes.get(activePaneId) : undefined;
  }

  activeGroup(): WorkbenchGroupSnapshot | undefined {
    const activeGroupId = this.layoutState().activeGroupId;
    const group = activeGroupId ? this.layoutState().groups.get(activeGroupId) : undefined;
    return group ? this.snapshotGroup(group) : undefined;
  }

  snapshotEditorLayout(options?: {
    mapItem?: (item: WorkbenchItemDescriptor) => WorkbenchItemDescriptor;
    mapPane?: (pane: WorkbenchPaneDescriptor) => WorkbenchPaneDescriptor;
  }): WorkbenchEditorLayoutSnapshot {
    const state = this.layoutState();
    return {
      root: snapshotNode(state.root),
      groups: [...state.groups.values()].map((group) => this.snapshotGroup(group)),
      items: [...state.items.values()].map((item) => options?.mapItem ? options.mapItem({ ...item }) : { ...item }),
      panes: [...state.panes.values()].map((pane) => options?.mapPane ? options.mapPane({ ...pane }) : { ...pane }),
      activeGroupId: state.activeGroupId,
      activePaneId: state.activePaneId,
      activeItemId: state.activeItemId,
    };
  }

  private layoutState(): WorkbenchLayoutState {
    return this.editor.stateSlot(workbenchLayoutSlot, () => ({
      nextItem: 1,
      nextPane: 1,
      nextGroup: 1,
      nextSplit: 1,
      root: null,
      groups: new Map(),
      items: new Map(),
      panes: new Map(),
      activeGroupId: undefined,
      activePaneId: undefined,
      activeItemId: undefined,
    }));
  }

  private upsertSurfaceItem(
    descriptor: Omit<WorkbenchSurfacePaneDescriptor, "id" | "kind" | "itemId" | "tab">,
    itemId?: WorkbenchItemId,
  ): WorkbenchSurfaceItemDescriptor {
    const state = this.layoutState();
    const existing = itemId ? state.items.get(itemId) : undefined;
    if (existing?.kind === "surface") {
      const next: WorkbenchSurfaceItemDescriptor = {
        ...existing,
        title: descriptor.title,
        path: descriptor.path,
        closeable: descriptor.closeable,
        documentId: descriptor.documentId,
      };
      state.items.set(existing.id, next);
      this.syncPanesForItem(next);
      return next;
    }

    const item: WorkbenchSurfaceItemDescriptor = {
      id: workbenchItemId(`item:${state.nextItem++}`),
      kind: "surface",
      typeId: "surface",
      title: descriptor.title,
      path: descriptor.path,
      closeable: descriptor.closeable,
      documentId: descriptor.documentId,
    };
    state.items.set(item.id, item);
    return item;
  }

  private upsertToolItem(instance: ToolInstance, itemId?: WorkbenchItemId): WorkbenchToolItemDescriptor {
    const state = this.layoutState();
    const existing = itemId
      ? state.items.get(itemId)
      : [...state.items.values()].find((candidate) => candidate.kind === "tool" && candidate.toolId === instance.id);
    if (existing?.kind === "tool") {
      const next: WorkbenchToolItemDescriptor = {
        ...existing,
        title: instance.title,
        closeable: instance.closeable,
        typeId: instance.typeId,
        toolId: instance.id,
        packageId: instance.packageId,
        input: instance.input,
      };
      state.items.set(existing.id, next);
      this.syncPanesForItem(next);
      return next;
    }

    const item: WorkbenchToolItemDescriptor = {
      id: workbenchItemId(`item:${state.nextItem++}`),
      kind: "tool",
      typeId: instance.typeId,
      toolId: instance.id,
      title: instance.title,
      closeable: instance.closeable,
      packageId: instance.packageId,
      input: instance.input,
    };
    state.items.set(item.id, item);
    return item;
  }

  private syncPanesForItem(item: WorkbenchItemDescriptor): void {
    const state = this.layoutState();
    for (const [paneId, pane] of state.panes.entries()) {
      if (pane.itemId !== item.id) continue;
      state.panes.set(paneId, paneWithItem(pane, item));
    }
    if (state.activePaneId) state.activeItemId = state.panes.get(state.activePaneId)?.itemId;
  }

  private removeOrphanedItem(itemId: WorkbenchItemId): void {
    const state = this.layoutState();
    if ([...state.panes.values()].some((pane) => pane.itemId === itemId)) return;
    state.items.delete(itemId);
    if (state.activeItemId === itemId) {
      state.activeItemId = state.activePaneId ? state.panes.get(state.activePaneId)?.itemId : undefined;
    }
  }

  private placePane(pane: WorkbenchPaneDescriptor, placement?: WorkbenchPanePlacement): void {
    const state = this.layoutState();
    state.panes.set(pane.id, pane);
    if (!state.root) this.ensureEditorRoot();

    const normalized = placement ?? { kind: "tab", group: "active" };
    if (normalized.kind === "split") {
      const targetGroupId = this.resolveSplitTarget(state, normalized);
      if (!targetGroupId) {
        this.insertTabIntoGroup(pane.id, this.ensureEditorRoot(), undefined);
        state.activeItemId = pane.itemId;
        return;
      }
      const group = this.createGroup(state);
      this.insertTabIntoGroup(pane.id, group.id, undefined);
      state.root = insertSplitForGroup(
        state.root,
        targetGroupId,
        directionAxis(normalized.direction),
        splitAfter(normalized.direction),
        { kind: "group", groupId: group.id },
        () => workbenchSplitId(`split:${state.nextSplit++}`),
      );
      state.activeGroupId = group.id;
      state.activePaneId = pane.id;
      state.activeItemId = pane.itemId;
      return;
    }

    const groupId = this.resolveTabGroup(state, normalized) ?? this.ensureEditorRoot();
    this.insertTabIntoGroup(pane.id, groupId, normalized.index);
    state.activeGroupId = groupId;
    state.activePaneId = pane.id;
    state.activeItemId = pane.itemId;
  }

  private insertTabIntoGroup(paneId: WorkbenchPaneId, groupId: WorkbenchGroupId, index?: number): void {
    const state = this.layoutState();
    const group = state.groups.get(groupId) ?? this.createGroup(state, groupId);
    const clampedIndex = typeof index === "number"
      ? Math.max(0, Math.min(index, group.paneIds.length))
      : group.paneIds.length;
    group.paneIds.splice(clampedIndex, 0, paneId);
    group.activePaneId = paneId;
  }

  private createGroup(state: WorkbenchLayoutState, id = workbenchGroupId(`group:${state.nextGroup++}`)): WorkbenchGroupState {
    const existing = state.groups.get(id);
    if (existing) return existing;
    const group: WorkbenchGroupState = { id, paneIds: [], activePaneId: undefined };
    state.groups.set(id, group);
    return group;
  }

  private resolveTabGroup(state: WorkbenchLayoutState, placement: WorkbenchTabPlacement): WorkbenchGroupId | undefined {
    if (placement.group && placement.group !== "active") return state.groups.has(workbenchGroupId(placement.group))
      ? workbenchGroupId(placement.group)
      : undefined;
    return state.activeGroupId ?? firstGroupId(state.root) ?? undefined;
  }

  private resolveSplitTarget(state: WorkbenchLayoutState, placement: WorkbenchSplitPlacement): WorkbenchGroupId | undefined {
    if (!placement.of || placement.of === "active") {
      if (state.activePaneId) return this.groupForPane(state.activePaneId)?.id;
      return state.activeGroupId ?? firstGroupId(state.root) ?? undefined;
    }
    const pane = state.panes.get(workbenchPaneId(placement.of));
    if (pane) return this.groupForPane(pane.id)?.id;
    const groupId = workbenchGroupId(placement.of);
    return state.groups.has(groupId) ? groupId : undefined;
  }

  private groupForPane(id: WorkbenchPaneId): WorkbenchGroupState | undefined {
    for (const group of this.layoutState().groups.values()) {
      if (group.paneIds.includes(id)) return group;
    }
    return undefined;
  }

  private detachPaneFromGroup(id: WorkbenchPaneId): void {
    const state = this.layoutState();
    const group = this.groupForPane(id);
    if (!group) return;
    const index = group.paneIds.indexOf(id);
    if (index >= 0) group.paneIds.splice(index, 1);
    if (group.activePaneId === id) {
      group.activePaneId = group.paneIds[Math.min(index, group.paneIds.length - 1)] ?? group.paneIds[group.paneIds.length - 1];
    }
    if (group.paneIds.length === 0 && state.groups.size > 1) {
      state.groups.delete(group.id);
      state.root = pruneEmptyGroups(state.root, state.groups);
    }
    state.root = collapseSplitTree(state.root);
  }

  private removePane(id: WorkbenchPaneId): void {
    const state = this.layoutState();
    const pane = state.panes.get(id);
    if (!pane) return;
    const group = this.groupForPane(id);
    state.panes.delete(id);
    this.removeOrphanedItem(pane.itemId);
    if (!group) return;
    const index = group.paneIds.indexOf(id);
    if (index >= 0) group.paneIds.splice(index, 1);
    if (group.activePaneId === id) {
      group.activePaneId = group.paneIds[Math.min(index, group.paneIds.length - 1)] ?? group.paneIds[group.paneIds.length - 1];
    }

    if (group.paneIds.length === 0 && state.groups.size > 1) {
      state.groups.delete(group.id);
      state.root = pruneEmptyGroups(state.root, state.groups);
    }

    state.root = collapseSplitTree(state.root);

    const activeGroup = state.activeGroupId ? state.groups.get(state.activeGroupId) : undefined;
    if (!activeGroup || !activeGroup.paneIds.length) {
      const fallbackGroup = [...state.groups.values()].find((candidate) => candidate.paneIds.length);
      state.activeGroupId = fallbackGroup?.id;
      state.activePaneId = fallbackGroup?.activePaneId;
      state.activeItemId = fallbackGroup?.activePaneId ? state.panes.get(fallbackGroup.activePaneId)?.itemId : undefined;
    } else if (state.activePaneId === id) {
      state.activeGroupId = activeGroup.id;
      state.activePaneId = activeGroup.activePaneId;
      state.activeItemId = activeGroup.activePaneId ? state.panes.get(activeGroup.activePaneId)?.itemId : undefined;
    }

    if (!state.root) {
      const nextGroup = this.createGroup(state);
      state.root = { kind: "group", groupId: nextGroup.id };
      state.activeGroupId = nextGroup.id;
      state.activePaneId = nextGroup.activePaneId;
      state.activeItemId = nextGroup.activePaneId ? state.panes.get(nextGroup.activePaneId)?.itemId : undefined;
    }
  }

  private syncFocusToActivePane(): void {
    const pane = this.activePane();
    if (!pane) {
      this.editor.focus.setEditor();
      return;
    }
    if (pane.kind === "tool") this.editor.focus.setTool(pane.toolId);
    else this.editor.focus.setEditor();
  }

  private snapshotGroup(group: WorkbenchGroupState): WorkbenchGroupSnapshot {
    return {
      id: group.id,
      paneIds: [...group.paneIds],
      activePaneId: group.activePaneId,
    };
  }
}

export async function bootWorkbench(editor: EditorHost): Promise<void> {
  await editor.hooks.event("workbench.boot", undefined);
}

export async function handleWorkbenchKeyInput(editor: EditorHost, input: WorkbenchKeyInput): Promise<boolean> {
  const handled = await editor.hooks.first<WorkbenchKeyInput, boolean>("workbench.keyInput", input);
  if (handled !== undefined) return handled;

  const transition = await editor.interaction.handleInput(input);
  return transition.handled ?? Boolean(transition.effects?.length);
}

export async function configureWorkbenchSurface(editor: EditorHost, request: WorkbenchSurfaceConfigureRequest): Promise<boolean> {
  return (await editor.hooks.first<WorkbenchSurfaceConfigureRequest, boolean>("workbench.surfaceConfigure", request)) ?? false;
}

export async function getWorkbenchSessionState(editor: EditorHost): Promise<WorkbenchSessionState> {
  return (await editor.hooks.first<void, WorkbenchSessionState>("workbench.sessionState", undefined)) ?? {};
}

export function workbenchItemId(id: WorkbenchItemId | string): WorkbenchItemId {
  return id as WorkbenchItemId;
}

export function workbenchPaneId(id: WorkbenchPaneId | string): WorkbenchPaneId {
  return id as WorkbenchPaneId;
}

export function workbenchGroupId(id: WorkbenchGroupId | string): WorkbenchGroupId {
  return id as WorkbenchGroupId;
}

export function workbenchSplitId(id: WorkbenchSplitId | string): WorkbenchSplitId {
  return id as WorkbenchSplitId;
}

function normalizePaneTab(tab: WorkbenchPaneTabState | undefined): WorkbenchPaneTabState {
  return {
    preview: tab?.preview,
    pinned: tab?.pinned,
    subtitle: tab?.subtitle,
    badges: tab?.badges ? [...tab.badges] : undefined,
  };
}

function paneWithItem(pane: WorkbenchPaneDescriptor, item: WorkbenchItemDescriptor): WorkbenchPaneDescriptor {
  if (pane.kind === "surface" && item.kind === "surface") {
    return {
      ...pane,
      title: item.title,
      documentId: item.documentId ?? pane.documentId,
      path: item.path,
      closeable: item.closeable,
    };
  }
  if (pane.kind === "tool" && item.kind === "tool") {
    return {
      ...pane,
      title: item.title,
      typeId: item.typeId,
      closeable: item.closeable,
      packageId: item.packageId,
      input: item.input,
    };
  }
  return pane;
}

function groupHasSurfaceItem(
  state: WorkbenchLayoutState,
  group: WorkbenchGroupState | undefined,
  itemId: WorkbenchItemId,
): boolean {
  if (!group) return false;
  return group.paneIds.some((paneId) => {
    const pane = state.panes.get(paneId);
    return pane?.kind === "surface" && pane.itemId === itemId;
  });
}

function directionAxis(direction: WorkbenchSplitDirection): WorkbenchSplitAxis {
  return direction === "left" || direction === "right" ? "row" : "column";
}

function splitAfter(direction: WorkbenchSplitDirection): boolean {
  return direction === "right" || direction === "down";
}

function firstGroupId(node: WorkbenchEditorLayoutNodeState | null): WorkbenchGroupId | undefined {
  if (!node) return undefined;
  if (node.kind === "group") return node.groupId;
  for (const child of node.children) {
    const groupId = firstGroupId(child);
    if (groupId) return groupId;
  }
  return undefined;
}

function snapshotNode(node: WorkbenchEditorLayoutNodeState | null): WorkbenchEditorLayoutNodeSnapshot | null {
  if (!node) return null;
  if (node.kind === "group") return { kind: "group", groupId: node.groupId };
  return {
    kind: "split",
    id: node.id,
    axis: node.axis,
    sizes: [...node.sizes],
    children: node.children.map((child) => snapshotNode(child)!).filter(Boolean),
  };
}

function insertSplitForGroup(
  node: WorkbenchEditorLayoutNodeState | null,
  targetGroupId: WorkbenchGroupId,
  axis: WorkbenchSplitAxis,
  after: boolean,
  newLeaf: WorkbenchGroupNodeState,
  nextSplitId: () => WorkbenchSplitId,
): WorkbenchEditorLayoutNodeState {
  if (!node) {
    return newLeaf;
  }
  if (node.kind === "group") {
    if (node.groupId !== targetGroupId) return node;
    return {
      kind: "split",
      id: nextSplitId(),
      axis,
      sizes: [1, 1],
      children: after ? [node, newLeaf] : [newLeaf, node],
    };
  }

  const childIndex = node.children.findIndex((child) => containsGroup(child, targetGroupId));
  if (childIndex < 0) return node;

  if (node.axis === axis) {
    const nextChildren = [...node.children];
    nextChildren.splice(after ? childIndex + 1 : childIndex, 0, newLeaf);
    const nextSizes = insertSize(node.sizes, childIndex, after);
    return {
      ...node,
      children: nextChildren,
      sizes: nextSizes,
    };
  }

  const nextChildren = [...node.children];
  nextChildren[childIndex] = insertSplitForGroup(nextChildren[childIndex]!, targetGroupId, axis, after, newLeaf, nextSplitId);
  return {
    ...node,
    children: nextChildren,
  };
}

function containsGroup(node: WorkbenchEditorLayoutNodeState, targetGroupId: WorkbenchGroupId): boolean {
  if (node.kind === "group") return node.groupId === targetGroupId;
  return node.children.some((child) => containsGroup(child, targetGroupId));
}

function pruneEmptyGroups(
  node: WorkbenchEditorLayoutNodeState | null,
  groups: ReadonlyMap<WorkbenchGroupId, WorkbenchGroupState>,
): WorkbenchEditorLayoutNodeState | null {
  if (!node) return null;
  if (node.kind === "group") {
    return groups.has(node.groupId) ? node : null;
  }
  const children = node.children
    .map((child) => pruneEmptyGroups(child, groups))
    .filter((child): child is WorkbenchEditorLayoutNodeState => Boolean(child));
  if (!children.length) return null;
  return {
    ...node,
    children,
    sizes: normalizeSizes(node.sizes, children.length),
  };
}

function collapseSplitTree(node: WorkbenchEditorLayoutNodeState | null): WorkbenchEditorLayoutNodeState | null {
  if (!node || node.kind === "group") return node;
  const children = node.children
    .map((child) => collapseSplitTree(child))
    .filter((child): child is WorkbenchEditorLayoutNodeState => Boolean(child));
  if (!children.length) return null;
  if (children.length === 1) return children[0]!;
  return {
    ...node,
    children,
    sizes: normalizeSizes(node.sizes, children.length),
  };
}

function insertSize(existing: readonly number[], childIndex: number, after: boolean): number[] {
  const normalized = normalizeSizes(existing, existing.length || childIndex + 1);
  const targetIndex = Math.max(0, Math.min(childIndex, normalized.length - 1));
  const targetSize = normalized[targetIndex] ?? 1 / Math.max(1, normalized.length);
  const splitSize = targetSize / 2;
  const next = [...normalized];
  next[targetIndex] = splitSize;
  next.splice(after ? targetIndex + 1 : targetIndex, 0, splitSize);
  return normalizeSizes(next, next.length);
}

function normalizeSizes(input: readonly number[], length: number): number[] {
  const next = input.slice(0, length).map((value) => Math.max(0.01, Number.isFinite(value) ? value : 0.01));
  while (next.length < length) next.push(1);
  const total = next.reduce((sum, value) => sum + value, 0) || length;
  return next.map((value) => value / total);
}

function findSplitNode(node: WorkbenchEditorLayoutNodeState | null, targetId: WorkbenchSplitId): WorkbenchSplitNodeState | undefined {
  if (!node || node.kind === "group") return undefined;
  if (node.id === targetId) return node;
  for (const child of node.children) {
    const match = findSplitNode(child, targetId);
    if (match) return match;
  }
  return undefined;
}
