import type { EditorHost, EditorModule, KeyInput } from "@the-editor/editor";
import { defaultEditing } from "./editing.js";
import { defaultKeymaps } from "./keymaps.js";
import { defaultScratch, defaultViewport, type CreateScratchArgs, type RenderActiveSurfaceArgs } from "./scratch.js";
import { getDefaultState, handleKeyInput, refreshActiveSurface, setEditorMode, type DefaultEditorState, type EditorMode } from "./state.js";

export async function activate(editor: EditorHost): Promise<void> {
  await editor.use({ id: "default.scratch", activate: defaultScratch() });
  await editor.use({ id: "default.editing", activate: defaultEditing() });
  await editor.use({ id: "default.keymaps", activate: defaultKeymaps() });
}

export function defaultDistribution(): EditorModule {
  return activate;
}

export {
  defaultEditing,
  defaultKeymaps,
  defaultScratch,
  defaultViewport,
  getDefaultState,
  handleKeyInput,
  refreshActiveSurface,
  setEditorMode,
  type CreateScratchArgs,
  type DefaultEditorState,
  type EditorMode,
  type KeyInput,
  type RenderActiveSurfaceArgs,
};
