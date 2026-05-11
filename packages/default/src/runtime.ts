import type { EditorModule } from "@the-editor/editor";
import { defaultCommandPalette } from "./commandPalette.js";
import { defaultEditing } from "./editing.js";
import { defaultGutters } from "./gutters.js";
import { defaultInputPrompt } from "./inputPrompt.js";
import { defaultPicker } from "./picker.js";
import { defaultReload } from "./reload.js";
import { defaultScratch } from "./scratch.js";
import { defaultWorkbench } from "./workbench.js";
import { defaultWorkbenchControls } from "./workbenchControls.js";
import { defaultWorkingDirectory } from "./workingDirectory.js";

const runtimeModule: EditorModule = async (editor) => {
  await defaultScratch()(editor);
  await defaultEditing()(editor);
  await defaultReload()(editor);
  await defaultWorkingDirectory()(editor);
  await defaultCommandPalette()(editor);
  await defaultInputPrompt()(editor);
  await defaultPicker()(editor);
  await defaultGutters()(editor);
  await defaultWorkbench()(editor);
  await defaultWorkbenchControls()(editor);
};

export default runtimeModule;
