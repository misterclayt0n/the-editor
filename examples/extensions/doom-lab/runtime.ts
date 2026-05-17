import { commandId, matchesCommandPaletteItem, toolTypeId, type CommandPaletteItem, type EditorModule } from "@the-editor/editor";

const doomToolTypeId = toolTypeId("doom-lab.tool");

const doomLabRuntime: EditorModule = (editor) => {
  editor.tools.registerType({
    id: doomToolTypeId,
    title: "Doom",
    root: "overlayModal",
    instancePolicy: "singleton",
    closeable: true,
    persistenceKey: "doom-lab.tool",
  });

  editor.commands.register<void, string>({
    id: commandId("doom_lab.open"),
    title: "Open Doom",
    category: "Extensions",
    keywords: ["doom", "game", "shareware"],
    run() {
      return editor.tools.open(doomToolTypeId, "Doom").id;
    },
  });

  editor.commands.register<void, void>({
    id: commandId("doom_lab.close"),
    title: "Close Doom",
    category: "Extensions",
    hidden: true,
    run() {
      for (const tool of editor.tools.allInstances()) {
        if (tool.typeId === doomToolTypeId) editor.tools.close(tool.id);
      }
    },
  });

  editor.palette.registerProvider({
    id: "doom-lab.commands",
    title: "Doom Lab",
    priority: 15,
    provide(ctx) {
      return [
        paletteItem({
          id: "doom-lab:open",
          title: "Extensions: Open Doom",
          detail: "doom_lab.open",
          keywords: ["doom", "game", "fps", "shareware", "extension"],
          command: { id: commandId("doom_lab.open") },
        }),
      ].filter((item) => matchesCommandPaletteItem(ctx.query, item));
    },
  });
};

export default doomLabRuntime;

function paletteItem(item: CommandPaletteItem): CommandPaletteItem {
  return item;
}
