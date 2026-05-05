import { commandId, type EditorModule } from "@the-editor/editor";

export function defaultReload(): EditorModule {
  return (editor) => {
    editor.commands.register<{ specifier?: string } | undefined, void>({
      id: commandId("editor.reload"),
      title: ":reload",
      category: "Workbench",
      keywords: ["reload", ":reload"],
      async run(_ctx, args) {
        await editor.roles.close("commandPalette");
        await editor.packages.reload(args?.specifier);
      },
    });
  };
}
