import { type EditorHost, type KeyInput, defineHostStore } from "@the-editor/editor";
import type { Viewport } from "@the-editor/contracts/runtime";
import {
  activeWorkbenchSurfacePane,
  refreshWorkbenchSnapshot,
  removeWorkbenchSurfacePane,
  setWorkbenchEditorMode,
  snapshotWorkbenchSession,
  syncWorkbenchActiveSurface,
  upsertWorkbenchSurfacePane,
  workbenchSnapshotState,
  workbenchSurfacePaneState,
  type DefaultEditorState,
  type EditorMode,
  type SurfacePaneViewState,
} from "./workbenchSnapshot.js";

export type { DefaultEditorState, EditorMode, SurfacePaneViewState };

export function getDefaultState(editor: EditorHost): DefaultEditorState {
  return workbenchSnapshotState(editor);
}

export function setEditorMode(editor: EditorHost, mode: EditorMode): DefaultEditorState {
  editor.interaction.setMode(mode);
  return setWorkbenchEditorMode(editor, mode);
}

export function activeSurfacePane(editor: EditorHost): SurfacePaneViewState | undefined {
  return activeWorkbenchSurfacePane(editor);
}

export function surfacePaneState(editor: EditorHost, paneId: string): SurfacePaneViewState | undefined {
  return workbenchSurfacePaneState(editor, paneId);
}

export function upsertSurfacePaneState(editor: EditorHost, pane: SurfacePaneViewState): SurfacePaneViewState {
  return upsertWorkbenchSurfacePane(editor, pane);
}

export function removeSurfacePaneState(editor: EditorHost, paneId: string): void {
  removeWorkbenchSurfacePane(editor, paneId);
}

export async function refreshActiveSurface(editor: EditorHost, viewport?: Viewport): Promise<DefaultEditorState> {
  return refreshWorkbenchSnapshot(editor, viewport);
}

export async function buildWorkbenchSessionState(editor: EditorHost) {
  return snapshotWorkbenchSession(editor);
}

export function syncActiveSurfaceFromWorkbench(editor: EditorHost): void {
  syncWorkbenchActiveSurface(editor);
}

export async function handleKeyInput(editor: EditorHost, input: KeyInput): Promise<DefaultEditorState> {
  const state = getDefaultState(editor);
  if (editor.overlays.isOpen("hover")) {
    editor.stores.set(defineHostStore({ id: "default.hover.state", init: () => undefined }), { visible: false });
    editor.overlays.close("hover", { restoreFocus: false });
    editor.workbench.invalidate("roles");
  }
  await editor.interaction.handleInput(input);
  await refreshActiveSurface(editor);
  return state;
}
