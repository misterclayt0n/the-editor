import type { EditorModule } from "@the-editor/editor";
import { defaultKeymaps } from "./keymaps.js";

const interactionModule: EditorModule = (editor) => defaultKeymaps()(editor);

export default interactionModule;
