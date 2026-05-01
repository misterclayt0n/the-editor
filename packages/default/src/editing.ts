import type { SurfaceId, TextCommand } from "@the-editor/contracts/runtime";
import { commandId, type EditorHost, type EditorModule } from "@the-editor/editor";
import { getDefaultState, setEditorMode } from "./state.js";

export function defaultEditing(): EditorModule {
  return (editor) => {
    editor.commands.register({ id: commandId("mode.normal"), title: "Normal Mode", run: () => setEditorMode(editor, "normal") });
    editor.commands.register({ id: commandId("mode.insert"), title: "Insert Mode", run: () => setEditorMode(editor, "insert") });

    registerTextCommand(editor, "move.charLeft", () => ({ kind: "moveCharLeft" }));
    registerTextCommand(editor, "move.charRight", () => ({ kind: "moveCharRight" }));
    registerTextCommand(editor, "move.visualLineUp", () => ({ kind: "moveVisualLineUp", preferredColumn: getDefaultState(editor).preferredColumn }));
    registerTextCommand(editor, "move.visualLineDown", () => ({ kind: "moveVisualLineDown", preferredColumn: getDefaultState(editor).preferredColumn }));
    registerTextCommand(editor, "goto.lineStart", () => ({ kind: "gotoLineStart" }));
    registerTextCommand(editor, "goto.lineEnd", () => ({ kind: "gotoLineEnd" }));
    registerTextCommand(editor, "goto.documentStart", () => ({ kind: "gotoDocumentStart" }));
    registerTextCommand(editor, "goto.documentEnd", () => ({ kind: "gotoDocumentEnd" }));

    editor.commands.register<{ text: string }, void>({
      id: commandId("edit.insertText"),
      title: "Insert Text",
      async run(_ctx, args) {
        await executeTextCommand(editor, { kind: "insertText", text: args.text });
      },
    });

    editor.commands.register({
      id: commandId("edit.insertNewline"),
      title: "Insert Newline",
      async run() {
        const surfaceId = activeSurfaceId(editor);
        const snapshot = await editor.runtime.surfaces.snapshot({ surfaceId });
        const pos = snapshot.selection.ranges.find((range) => range.id === snapshot.activeCursorId)?.head ?? snapshot.selection.ranges[0]?.head ?? 0;
        const { indent } = await editor.runtime.surfaces.indentForNewline({ surfaceId, pos });
        await executeTextCommand(editor, { kind: "insertNewline", indent, indentUnit: "  " });
      },
    });

    editor.commands.register({
      id: commandId("edit.deleteBackward"),
      title: "Delete Backward",
      async run() {
        await executeTextCommand(editor, { kind: "deleteBackward" });
      },
    });

    editor.commands.register({
      id: commandId("edit.deleteForward"),
      title: "Delete Forward",
      async run() {
        await executeTextCommand(editor, { kind: "deleteForward" });
      },
    });
  };
}

function registerTextCommand(editor: EditorHost, id: string, command: () => TextCommand): void {
  editor.commands.register({
    id: commandId(id),
    title: id,
    async run() {
      await executeTextCommand(editor, command());
    },
  });
}

async function executeTextCommand(editor: EditorHost, command: TextCommand): Promise<void> {
  const state = getDefaultState(editor);
  const result = await editor.runtime.surfaces.executeTextCommand({
    surfaceId: activeSurfaceId(editor),
    command,
  });
  state.preferredColumn = result.preferredColumn;
}

function activeSurfaceId(editor: EditorHost): SurfaceId {
  const state = getDefaultState(editor);
  if (!state.activeSurfaceId) throw new Error("no active surface");
  return state.activeSurfaceId;
}
