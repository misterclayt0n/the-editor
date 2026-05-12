import { commandId, matchesCommandPaletteItem, type CommandPaletteItem, type EditorModule } from "@the-editor/editor";
import { activeSurfacePane, refreshActiveSurface } from "./state.js";

export function defaultSave(): EditorModule {
  return (editor) => {
    editor.commands.register<void, void>({
      id: commandId("write"),
      title: "Write File",
      category: "File",
      keywords: [":write", "write", ":w", "w", "save"],
      async run() {
        const pane = activeSurfacePane(editor);
        if (!pane?.surfaceId) {
          editor.notifications.warn("save", "no active surface to save");
          return;
        }
        try {
          await editor.runtime.resources.save({ surfaceId: pane.surfaceId });
          await refreshActiveSurface(editor);
          editor.workbench.invalidate("frame", "tabs", "notifications");
          editor.notifications.info("save", pane.path ? `saved ${pane.path}` : "saved file");
        } catch (error) {
          editor.notifications.error("save", error instanceof Error ? error.message : String(error));
        }
      },
    });

    editor.palette.registerProvider({
      id: "default.write",
      title: "Write",
      priority: 95,
      provide(_ctx, query) {
        const item: CommandPaletteItem = {
          id: "write",
          title: ":write",
          detail: "Write buffer to file",
          group: "File",
          keywords: ["write", ":write", "w", ":w", "save"],
          command: { id: commandId("write") },
        };
        return matchesCommandPaletteItem(query, item) ? [item] : [];
      },
    });
  };
}
