import { type EditorModule } from "@the-editor/editor";

export function defaultKeymaps(): EditorModule {
  return (editor) => {
    editor.keymaps.bind({
      id: "default.scratch.create",
      key: "cmd+n",
      command: "scratch.create",
      args: {
        text: "",
      },
    });

    editor.keymaps.bind({
      id: "default.surface.renderActive",
      key: "cmd+r",
      command: "surface.renderActive",
    });
  };
}
