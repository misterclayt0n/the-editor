import type { EditorModule } from "@the-editor/editor";
import { defaultCommandPalette } from "./commandPalette.js";
import { defaultEditing } from "./editing.js";
import { defaultReload } from "./reload.js";
import { defaultScratch } from "./scratch.js";
import { defaultWorkbench } from "./workbench.js";
import { defaultWorkbenchControls } from "./workbenchControls.js";

const runtimeModule: EditorModule = async (editor) => {
  await defaultScratch()(editor);
  await defaultEditing()(editor);
  await defaultReload()(editor);
  await defaultCommandPalette()(editor);
  await defaultWorkbench()(editor);
  await defaultWorkbenchControls()(editor);
};

export default runtimeModule;
