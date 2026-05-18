import type { EditorModule } from "@the-editor/editor";
import { defaultBracketNavigation } from "./bracketNavigation.js";
import { defaultCommandPalette } from "./commandPalette.js";
import { defaultConfirmPrompt } from "./confirmPrompt.js";
import { defaultEditing } from "./editing.js";
import { defaultGutters } from "./gutters.js";
import { defaultFileTree } from "./fileTree.js";
import { defaultInputPrompt } from "./inputPrompt.js";
import { defaultReload } from "./reload.js";
import { defaultScratch } from "./scratch.js";
import { defaultSave } from "./save.js";
import { defaultWorkbench } from "./workbench.js";
import { defaultWindowCommands } from "./windowCommands.js";
import { defaultWorkbenchControls } from "./workbenchControls.js";
import { defaultWorkingDirectory } from "./workingDirectory.js";

const runtimeModule: EditorModule = async (editor) => {
  await defaultScratch()(editor);
  await defaultSave()(editor);
  await defaultEditing()(editor);
  await defaultBracketNavigation()(editor);
  await defaultReload()(editor);
  await defaultWorkingDirectory()(editor);
  await defaultCommandPalette()(editor);
  await defaultConfirmPrompt()(editor);
  await defaultInputPrompt()(editor);
  await defaultGutters()(editor);
  await defaultWorkbench()(editor);
  await defaultWindowCommands()(editor);
  await defaultWorkbenchControls()(editor);
  await defaultFileTree()(editor);
};

export default runtimeModule;
