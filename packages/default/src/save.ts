import { commandId, type EditorModule } from "@the-editor/editor";
import { activeSurfacePane, refreshActiveSurface } from "./state.js";

export function defaultSave(): EditorModule {
  return (editor) => {
    editor.commands.register<void, void>({
      id: commandId("write"),
      title: "Save File",
      aliases: ["write", "w"],
      category: "File",
      palette: "topLevel",
      keywords: ["save"],
      icon: { kind: "lucide", name: "save" },
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
  };
}
