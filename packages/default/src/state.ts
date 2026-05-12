import { commandId, type EditorHost, type KeyInput } from "@the-editor/editor";
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
    editor.setStateSlot("default.hover.state", { visible: false });
    editor.overlays.close("hover", { restoreFocus: false });
    editor.workbench.invalidate("roles");
  }
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
