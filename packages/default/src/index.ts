import type { KeyInput } from "@the-editor/editor";
import { defaultBracketNavigation } from "./bracketNavigation.js";
import { defaultCommandPalette } from "./commandPalette.js";
import { defaultEditing } from "./editing.js";
import { defaultGutters } from "./gutters.js";
import { defaultFileTree } from "./fileTree.js";
import { defaultInputPrompt } from "./inputPrompt.js";
import { defaultKeymaps } from "./keymaps.js";
import { defaultPicker } from "./picker.js";
import interactionModule from "./interaction.js";
import { defaultReload } from "./reload.js";
import runtimeModule from "./runtime.js";
import { defaultScratch, defaultViewport, type CreateScratchArgs } from "./scratch.js";
import { defaultSave } from "./save.js";
import { getDefaultState, handleKeyInput, refreshActiveSurface, setEditorMode, type DefaultEditorState, type EditorMode } from "./state.js";
import { defaultWindowCommands } from "./windowCommands.js";
import { defaultWorkingDirectory, resolveFilePathFromWorkingDirectory } from "./workingDirectory.js";

export {
  defaultBracketNavigation,
  defaultCommandPalette,
  defaultEditing,
  defaultGutters,
  defaultFileTree,
  defaultInputPrompt,
  defaultKeymaps,
  defaultPicker,
  defaultReload,
  defaultSave,
  defaultScratch,
  defaultViewport,
  defaultWindowCommands,
  defaultWorkingDirectory,
  getDefaultState,
  handleKeyInput,
  interactionModule,
  refreshActiveSurface,
  runtimeModule,
  resolveFilePathFromWorkingDirectory,
  setEditorMode,
  type CreateScratchArgs,
  type DefaultEditorState,
  type EditorMode,
  type KeyInput,
};
