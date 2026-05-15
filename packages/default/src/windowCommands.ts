import type { EditorHost, EditorModule } from "@the-editor/editor";
import { commandId, defineHostStore } from "@the-editor/editor";
import type { SurfaceSelectionRange } from "@the-editor/contracts/runtime";
import type { WorkbenchPaneDescriptor, WorkbenchPaneId } from "@the-editor/editor/workbench";
import { defaultViewport } from "./scratch.js";
import { activeSurfacePane, getDefaultState, refreshActiveSurface, removeSurfacePaneState, upsertSurfacePaneState } from "./state.js";

type Direction = "left" | "right" | "up" | "down";
type BufferNavState = { history: string[]; modified?: string };
const bufferNavSlot = "default.window.bufferNav";

export function defaultWindowCommands(): EditorModule {
  return (editor) => {
    editor.hooks.tap("window.bufferHistory", {
      id: "default.window.bufferHistory",
      handler() { recordActiveBuffer(editor); return false; },
    });
    editor.hooks.tap("workbench.keyInput", {
      id: "default.window.modifiedBufferHistory",
      handler() { if (getDefaultState(editor).mode === "insert") recordModifiedBuffer(editor); return false; },
    });
    register(editor, "goto_window_top", () => gotoWindow(editor, "top"));
    register(editor, "goto_window_center", () => gotoWindow(editor, "center"));
    register(editor, "goto_window_bottom", () => gotoWindow(editor, "bottom"));
    register(editor, "goto_last_accessed_file", () => gotoLastAccessedFile(editor));
    register(editor, "goto_last_modified_file", () => gotoLastModifiedFile(editor));
    register(editor, "goto_next_buffer", () => gotoBuffer(editor, "next"));
    register(editor, "goto_previous_buffer", () => gotoBuffer(editor, "prev"));
    register(editor, "rotate_view", () => rotateView(editor));
    register(editor, "hsplit", () => editor.commands.dispatch({ id: commandId("workbench.hsplit") }));
    register(editor, "vsplit", () => editor.commands.dispatch({ id: commandId("workbench.vsplit") }));
    register(editor, "transpose_view", () => transposeView(editor));
    register(editor, "goto_file_hsplit", async () => { await editor.commands.dispatch({ id: commandId("hsplit") }); await editor.commands.dispatch({ id: commandId("file_picker") }); });
    register(editor, "goto_file_vsplit", async () => { await editor.commands.dispatch({ id: commandId("vsplit") }); await editor.commands.dispatch({ id: commandId("file_picker") }); });
    register(editor, "wclose", () => closeView(editor));
    register(editor, "wonly", () => onlyView(editor));
    register(editor, "jump_view_left", () => jumpView(editor, "left"));
    register(editor, "jump_view_right", () => jumpView(editor, "right"));
    register(editor, "jump_view_up", () => jumpView(editor, "up"));
    register(editor, "jump_view_down", () => jumpView(editor, "down"));
    register(editor, "swap_view_left", () => swapView(editor, "left"));
    register(editor, "swap_view_right", () => swapView(editor, "right"));
    register(editor, "swap_view_up", () => swapView(editor, "up"));
    register(editor, "swap_view_down", () => swapView(editor, "down"));
    register(editor, "hsplit_new", () => splitNewScratch(editor, "down"));
    register(editor, "vsplit_new", () => splitNewScratch(editor, "right"));
  };
}

function register(editor: EditorHost, id: string, run: () => Promise<unknown> | unknown): void {
  editor.commands.register({ id: commandId(id), title: id, category: "Window", async run() { await run(); } });
}

async function gotoWindow(editor: EditorHost, align: "top" | "center" | "bottom"): Promise<void> {
  const pane = activeSurfacePane(editor);
  if (!pane?.surfaceId) return;
  const [{ text }, selectionState, { viewport }] = await Promise.all([
    editor.runtime.surfaces.readText({ surfaceId: pane.surfaceId }),
    editor.runtime.surfaces.readSelection({ surfaceId: pane.surfaceId }),
    editor.runtime.surfaces.readViewport({ surfaceId: pane.surfaceId }),
  ]);
  const height = pane.viewport.rowCount || viewport.rowCount || 0;
  if (!height) return;
  const row = align === "top" ? 0 : align === "center" ? Math.floor((height - 1) / 2) : height - 1;
  const targetLine = (pane.viewport.firstRow ?? viewport.firstRow ?? 0) + row;
  const target = byteOffsetAtLine(text, targetLine);
  const extend = getDefaultState(editor).mode === "select";
  const selection = selectionState.selection.map((range) => putCursor(range, text, target, extend));
  await editor.runtime.surfaces.applyEdit({ surfaceId: pane.surfaceId, edit: { selection } });
  await refreshActiveSurface(editor);
  editor.workbench.invalidate("frame");
}

async function gotoBuffer(editor: EditorHost, direction: "next" | "prev"): Promise<void> {
  recordActiveBuffer(editor);
  const layout = editor.workbench.snapshotEditorLayout();
  const panes = layout.panes.filter((pane) => pane.kind === "surface");
  if (panes.length < 2 || !layout.activePaneId) return;
  const index = panes.findIndex((pane) => pane.id === layout.activePaneId);
  const next = panes[(index + (direction === "next" ? 1 : -1) + panes.length) % panes.length];
  if (next) await focusPane(editor, next.id);
}

async function gotoLastAccessedFile(editor: EditorHost): Promise<void> {
  recordActiveBuffer(editor);
  const state = bufferNavState(editor);
  const current = getDefaultState(editor).activePath;
  const target = [...state.history].reverse().find((path) => path && path !== current);
  if (target) await editor.commands.dispatch({ id: commandId("file.open"), args: { path: target } });
}

async function gotoLastModifiedFile(editor: EditorHost): Promise<void> {
  const target = bufferNavState(editor).modified;
  if (target) await editor.commands.dispatch({ id: commandId("file.open"), args: { path: target } });
}

async function rotateView(editor: EditorHost): Promise<void> {
  const layout = editor.workbench.snapshotEditorLayout();
  const panes = layout.panes;
  if (panes.length < 2 || !layout.activePaneId) return;
  const index = panes.findIndex((pane) => pane.id === layout.activePaneId);
  const next = panes[(index + 1 + panes.length) % panes.length];
  if (next) await focusPane(editor, next.id);
}

async function closeView(editor: EditorHost): Promise<void> {
  const layout = editor.workbench.snapshotEditorLayout();
  const active = editor.workbench.activePane();
  if (!active || layout.panes.length <= 1) return;
  editor.workbench.closePane(active.id);
  if (active.kind === "surface") removeSurfacePaneState(editor, active.id);
  await afterLayoutChange(editor);
}

async function onlyView(editor: EditorHost): Promise<void> {
  const active = editor.workbench.activePane();
  if (!active) return;
  for (const pane of editor.workbench.snapshotEditorLayout().panes) {
    if (pane.id === active.id) continue;
    editor.workbench.closePane(pane.id);
    if (pane.kind === "surface") removeSurfacePaneState(editor, pane.id);
  }
  await afterLayoutChange(editor);
}

async function jumpView(editor: EditorHost, direction: Direction): Promise<void> {
  const target = neighborPane(editor, direction);
  if (target) await focusPane(editor, target.id);
}

async function swapView(editor: EditorHost, direction: Direction): Promise<void> {
  const active = editor.workbench.activePane();
  const target = neighborPane(editor, direction);
  if (!active || !target) return;
  const opposite = oppositeDirection(direction);
  editor.workbench.movePane(active.id, { kind: "split", direction, of: target.id });
  editor.workbench.movePane(target.id, { kind: "split", direction: opposite, of: active.id });
  await focusPane(editor, active.id);
}

async function transposeView(editor: EditorHost): Promise<void> {
  const layout = editor.workbench.snapshotEditorLayout();
  const active = layout.activePaneId;
  if (!active || layout.panes.length < 2) return;
  const root = layout.root;
  if (!root || root.kind !== "split") return;
  const nextDirection: Direction = root.axis === "row" ? "down" : "right";
  const panes = layout.panes.filter((pane) => pane.id !== active);
  for (const pane of panes) editor.workbench.movePane(pane.id, { kind: "split", direction: nextDirection, of: active });
  await afterLayoutChange(editor);
}

async function splitNewScratch(editor: EditorHost, direction: "right" | "down"): Promise<void> {
  const active = editor.workbench.activePane();
  const viewport = activeSurfacePane(editor)?.viewport ?? getDefaultState(editor).surfaceViewport ?? defaultViewport();
  const document = await editor.runtime.documents.createScratch({ text: "" });
  const surface = await editor.runtime.surfaces.create({ documentId: document.documentId, viewport });
  const pane = editor.workbench.openSurface({
    title: "Scratch",
    surfaceId: surface.surfaceId,
    documentId: document.documentId,
    closeable: true,
  }, { placement: { kind: "split", direction, of: active?.id ?? "active" } });
  upsertSurfacePaneState(editor, {
    paneId: pane.id,
    surfaceId: surface.surfaceId,
    documentId: document.documentId,
    title: "Scratch",
    viewport,
  });
  await afterLayoutChange(editor);
}

async function focusPane(editor: EditorHost, paneId: WorkbenchPaneId): Promise<void> {
  editor.workbench.focusPane(paneId);
  await refreshActiveSurface(editor);
  editor.workbench.invalidate("frame", "focus", "layout", "tabs");
}

async function afterLayoutChange(editor: EditorHost): Promise<void> {
  await refreshActiveSurface(editor);
  editor.workbench.invalidate("frame", "focus", "layout", "tabs", "tools");
}

function bufferNavState(editor: EditorHost): BufferNavState {
  return editor.stores.host(defineHostStore({ id: bufferNavSlot, init: () => ({ history: [] }) }));
}

function recordActiveBuffer(editor: EditorHost): void {
  const path = getDefaultState(editor).activePath;
  if (!path) return;
  const state = bufferNavState(editor);
  if (state.history.at(-1) !== path) state.history.push(path);
  if (state.history.length > 64) state.history.splice(0, state.history.length - 64);
}

function recordModifiedBuffer(editor: EditorHost): void {
  const path = getDefaultState(editor).activePath;
  if (path) bufferNavState(editor).modified = path;
}

function putCursor(range: SurfaceSelectionRange, text: string, target: number, extend: boolean): SurfaceSelectionRange {
  const head = Math.max(0, Math.min(target, new TextEncoder().encode(text).length));
  return extend ? { ...range, head } : { ...range, anchor: head, head };
}

function byteOffsetAtLine(text: string, line: number): number {
  if (line <= 0) return 0;
  let current = 0;
  for (let i = 0; i < text.length; i++) {
    if (text.charCodeAt(i) === 10 && ++current === line) return new TextEncoder().encode(text.slice(0, i + 1)).length;
  }
  return new TextEncoder().encode(text).length;
}

function neighborPane(editor: EditorHost, direction: Direction): WorkbenchPaneDescriptor | undefined {
  const layout = editor.workbench.snapshotEditorLayout();
  const activeId = layout.activePaneId;
  if (!activeId) return undefined;
  const leaves = layoutLeaves(layout.root);
  const active = leaves.find((leaf) => leaf.group.paneIds.includes(activeId));
  if (!active) return undefined;
  const candidates = leaves.filter((leaf) => leaf !== active).filter((leaf) => {
    if (direction === "left") return leaf.x + leaf.w <= active.x;
    if (direction === "right") return leaf.x >= active.x + active.w;
    if (direction === "up") return leaf.y + leaf.h <= active.y;
    return leaf.y >= active.y + active.h;
  });
  candidates.sort((a, b) => distance(active, a, direction) - distance(active, b, direction));
  const group = candidates[0]?.group;
  const paneId = group?.activePaneId ?? group?.paneIds[0];
  return paneId ? layout.panes.find((pane) => pane.id === paneId) : undefined;

  function layoutLeaves(node: typeof layout.root, x = 0, y = 0, w = 1, h = 1): Array<{ group: { paneIds: WorkbenchPaneId[]; activePaneId?: WorkbenchPaneId }; x: number; y: number; w: number; h: number }> {
    if (!node) return [];
    if (node.kind === "group") {
      const group = layout.groups.find((entry) => entry.id === node.groupId);
      return group ? [{ group, x, y, w, h }] : [];
    }
    const total = node.sizes.reduce((sum, size) => sum + size, 0) || node.children.length;
    let offset = 0;
    const out: ReturnType<typeof layoutLeaves> = [];
    for (let i = 0; i < node.children.length; i++) {
      const fraction = (node.sizes[i] ?? 1) / total;
      if (node.axis === "row") out.push(...layoutLeaves(node.children[i], x + offset, y, w * fraction, h));
      else out.push(...layoutLeaves(node.children[i], x, y + offset, w, h * fraction));
      offset += node.axis === "row" ? w * fraction : h * fraction;
    }
    return out;
  }
}

function distance(a: { x: number; y: number; w: number; h: number }, b: { x: number; y: number; w: number; h: number }, direction: Direction): number {
  const ax = a.x + a.w / 2, ay = a.y + a.h / 2, bx = b.x + b.w / 2, by = b.y + b.h / 2;
  const primary = direction === "left" ? ax - bx : direction === "right" ? bx - ax : direction === "up" ? ay - by : by - ay;
  const secondary = direction === "left" || direction === "right" ? Math.abs(ay - by) : Math.abs(ax - bx);
  return primary * 10 + secondary;
}

function oppositeDirection(direction: Direction): Direction {
  return direction === "left" ? "right" : direction === "right" ? "left" : direction === "up" ? "down" : "up";
}
