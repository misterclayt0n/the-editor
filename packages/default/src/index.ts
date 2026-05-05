import type { KeyInput } from "@the-editor/editor";
import { defaultCommandPalette } from "./commandPalette.js";
import { defaultEditing } from "./editing.js";
import { defaultKeymaps } from "./keymaps.js";
import interactionModule from "./interaction.js";
import { defaultReload } from "./reload.js";
import runtimeModule from "./runtime.js";
import { defaultScratch, defaultViewport, type CreateScratchArgs, type RenderActiveSurfaceArgs } from "./scratch.js";
import { getDefaultState, handleKeyInput, refreshActiveSurface, setEditorMode, type DefaultEditorState, type EditorMode } from "./state.js";

export {
  defaultCommandPalette,
  defaultEditing,
  defaultKeymaps,
  defaultReload,
  defaultScratch,
  defaultViewport,
  getDefaultState,
  handleKeyInput,
  interactionModule,
  refreshActiveSurface,
  runtimeModule,
  setEditorMode,
  type CreateScratchArgs,
  type DefaultEditorState,
  type EditorMode,
  type KeyInput,
  type RenderActiveSurfaceArgs,
};
