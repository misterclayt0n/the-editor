import type { EditorHost, EditorModule } from "@the-editor/editor";
import { defaultKeymaps } from "./keymaps.js";
import { defaultScratch, getScratchState, type ScratchState } from "./scratch.js";

export async function activate(editor: EditorHost): Promise<void> {
  await editor.use({ id: "default.scratch", activate: defaultScratch() });
  await editor.use({ id: "default.keymaps", activate: defaultKeymaps() });
}

export function defaultDistribution(): EditorModule {
  return activate;
}

export { defaultKeymaps, defaultScratch, getScratchState, type ScratchState };
