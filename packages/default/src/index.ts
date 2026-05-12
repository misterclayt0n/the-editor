import type { KeyInput } from "@the-editor/editor";
import { defaultBracketNavigation } from "./bracketNavigation.js";
import { defaultCommandPalette } from "./commandPalette.js";
import { defaultCompletion } from "./completion.js";
import { defaultDiagnostics } from "./diagnostics.js";
import { defaultEditing } from "./editing.js";
import { defaultGutters } from "./gutters.js";
import { defaultFileTree } from "./fileTree.js";
import { defaultHover } from "./hover.js";
import { defaultInputPrompt } from "./inputPrompt.js";
import { defaultLsp } from "./lsp.js";
import { defaultLspNavigation } from "./lspNavigation.js";
import { defaultLspSymbols } from "./lspSymbols.js";
import { defaultKeymaps } from "./keymaps.js";
import { defaultPicker } from "./picker.js";
import interactionModule from "./interaction.js";
import { defaultReload } from "./reload.js";
import runtimeModule from "./runtime.js";
import { defaultScratch, defaultViewport, type CreateScratchArgs, type RenderActiveSurfaceArgs } from "./scratch.js";
import { defaultSave } from "./save.js";
import { defaultSignatureHelp } from "./signatureHelp.js";
import { getDefaultState, handleKeyInput, refreshActiveSurface, setEditorMode, type DefaultEditorState, type EditorMode } from "./state.js";
import { defaultWorkingDirectory, resolveFilePathFromWorkingDirectory } from "./workingDirectory.js";

export {
  defaultBracketNavigation,
  defaultCommandPalette,
  defaultCompletion,
  defaultDiagnostics,
  defaultEditing,
  defaultGutters,
  defaultHover,
  defaultFileTree,
  defaultInputPrompt,
  defaultKeymaps,
  defaultLsp,
  defaultLspNavigation,
  defaultLspSymbols,
  defaultPicker,
  defaultReload,
  defaultSave,
  defaultScratch,
  defaultSignatureHelp,
  defaultViewport,
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
  type RenderActiveSurfaceArgs,
};
