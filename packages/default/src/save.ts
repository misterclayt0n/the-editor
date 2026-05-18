import { commandId, type EditorModule } from "@the-editor/editor";
import { confirmPrompt } from "./confirmPrompt.js";
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
          const status = await editor.runtime.resources.status({ surfaceId: pane.surfaceId });
          let outcome: "saved" | "discarded" = "saved";
          if (status.conflict) {
            const action = await confirmPrompt(editor, status.deletedOnDisk
              ? {
                level: "warning",
                message: "This file has been deleted on disk since you started editing it. Do you want to recreate it?",
                actions: ["Save", "Cancel"],
              }
              : {
                level: "warning",
                message: "This file has changed on disk since you started editing it. Do you want to overwrite it?",
                actions: ["Overwrite", "Discard", "Cancel"],
              });

            if (action === "Overwrite" || action === "Save") {
              await editor.runtime.resources.save({ surfaceId: pane.surfaceId, conflict: "overwrite" });
            } else if (action === "Discard") {
              await editor.runtime.resources.reload({ surfaceId: pane.surfaceId });
              outcome = "discarded";
            } else {
              return;
            }
          } else {
            await editor.runtime.resources.save({ surfaceId: pane.surfaceId });
          }
          await refreshActiveSurface(editor);
          editor.workbench.invalidate("frame", "tabs", "notifications");
          editor.notifications.info("save", outcome === "discarded" ? "discarded local changes" : (pane.path ? `saved ${pane.path}` : "saved file"));
        } catch (error) {
          editor.notifications.error("save", error instanceof Error ? error.message : String(error));
        }
      },
    });
  };
}
