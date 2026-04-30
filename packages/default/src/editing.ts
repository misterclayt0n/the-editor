import type { SelectionTransform, SurfaceOperation } from "@the-editor/contracts/runtime";
import { commandId, type EditorHost, type EditorModule } from "@the-editor/editor";
import { getDefaultState, setEditorMode } from "./state.js";

export function defaultEditing(): EditorModule {
  return (editor) => {
    editor.commands.register({
      id: commandId("mode.normal"),
      title: "Normal Mode",
      run() {
        setEditorMode(editor, "normal");
      },
    });

    editor.commands.register({
      id: commandId("mode.insert"),
      title: "Insert Mode",
      run() {
        setEditorMode(editor, "insert");
      },
    });

    registerTransform(editor, "move.charLeft", {
      kind: "horizontal",
      direction: "backward",
      count: 1,
      mode: "move",
    });
    registerTransform(editor, "move.charRight", {
      kind: "horizontal",
      direction: "forward",
      count: 1,
      mode: "move",
    });
    registerTransform(editor, "move.visualLineUp", {
      kind: "visualVertical",
      direction: "backward",
      count: 1,
      mode: "move",
    });
    registerTransform(editor, "move.visualLineDown", {
      kind: "visualVertical",
      direction: "forward",
      count: 1,
      mode: "move",
    });
    registerTransform(editor, "goto.lineStart", { kind: "lineStart", mode: "move" });
    registerTransform(editor, "goto.lineEnd", { kind: "lineEnd", mode: "move" });
    registerTransform(editor, "goto.documentStart", { kind: "documentStart", mode: "move" });
    registerTransform(editor, "goto.documentEnd", { kind: "documentEnd", mode: "move" });

    editor.commands.register<{ text: string }, void>({
      id: commandId("edit.insertText"),
      title: "Insert Text",
      async run(_ctx, args) {
        await applyOperation(editor, { kind: "insertText", text: args.text });
      },
    });

    editor.commands.register({
      id: commandId("edit.insertNewline"),
      title: "Insert Newline",
      async run() {
        await applyOperation(editor, { kind: "insertNewline" });
      },
    });

    editor.commands.register({
      id: commandId("edit.deleteBackward"),
      title: "Delete Backward",
      async run() {
        await applyOperation(editor, { kind: "deleteBackward" });
      },
    });

    editor.commands.register({
      id: commandId("edit.deleteForward"),
      title: "Delete Forward",
      async run() {
        await applyOperation(editor, { kind: "deleteForward" });
      },
    });
  };
}

function registerTransform(editor: EditorHost, id: string, transform: SelectionTransform): void {
  editor.commands.register({
    id: commandId(id),
    title: id,
    async run() {
      await applyOperation(editor, { kind: "transformSelection", transform });
    },
  });
}

async function applyOperation(editor: EditorHost, operation: SurfaceOperation): Promise<void> {
  const state = getDefaultState(editor);
  if (!state.activeSurfaceId) throw new Error("no active surface");
  await editor.runtime.surfaces.applyOperation({
    surfaceId: state.activeSurfaceId,
    operation,
  });
}
