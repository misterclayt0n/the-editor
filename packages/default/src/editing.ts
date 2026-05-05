import type { SurfaceId, SurfaceOperation } from "@the-editor/contracts/runtime";
import { commandId, type EditorHost, type EditorModule } from "@the-editor/editor";
import { getDefaultState, setEditorMode } from "./state.js";

const RUST_OPERATIONS = [
  "move_char_left",
  "move_char_right",
  "extend_char_left",
  "extend_char_right",
  "move_visual_line_up",
  "move_visual_line_down",
  "extend_visual_line_up",
  "extend_visual_line_down",
  "move_next_word_start",
  "move_prev_word_start",
  "move_next_word_end",
  "extend_next_word_start",
  "extend_prev_word_start",
  "extend_next_word_end",
  "goto_line_start",
  "goto_line_end",
  "goto_line_end_newline",
  "goto_first_nonwhitespace",
  "goto_document_start",
  "goto_file_start",
  "goto_document_end",
  "goto_last_line",
  "extend_to_line_start",
  "extend_to_line_end",
  "extend_to_file_start",
  "extend_to_file_end",
  "extend_to_last_line",
  "select_all",
  "collapse_selection",
  "flip_selections",
  "merge_selections",
  "merge_consecutive_selections",
  "extend_line_below",
  "extend_line_above",
  "extend_line_up",
  "extend_to_line_bounds",
  "shrink_to_line_bounds",
  "delete_char_backward",
  "delete_char_forward",
  "delete_word_backward",
  "delete_word_forward",
  "kill_to_line_start",
  "kill_to_line_end",
  "switch_case",
  "switch_to_uppercase",
  "switch_to_lowercase",
  "indent",
  "unindent",
] as const;

const CHECKPOINT_AFTER_OPERATION = new Set<string>([
  "insert_text",
  "insert_tab",
  "smart_tab",
  "insert_newline",
  "open_above",
  "open_below",
  "delete_char_backward",
  "delete_char_forward",
  "delete_selection",
  "delete_selection_noyank",
  "delete_word_backward",
  "delete_word_forward",
  "kill_to_line_start",
  "kill_to_line_end",
  "paste",
  "switch_case",
  "switch_to_uppercase",
  "switch_to_lowercase",
  "indent",
  "unindent",
]);

export function defaultEditing(): EditorModule {
  return (editor) => {
    editor.commands.register({ id: commandId("normal_mode"), title: "Normal Mode", run: () => applyMode(editor, "normal") });
    editor.commands.register({ id: commandId("mode.normal"), title: "Normal Mode", run: () => applyMode(editor, "normal") });
    editor.commands.register({ id: commandId("insert_mode"), title: "Insert Mode", run: () => applyMode(editor, "insert") });
    editor.commands.register({ id: commandId("mode.insert"), title: "Insert Mode", run: () => applyMode(editor, "insert") });
    editor.commands.register({ id: commandId("select_mode"), title: "Select Mode", run: () => applyMode(editor, "select") });
    editor.commands.register({ id: commandId("exit_select_mode"), title: "Exit Select Mode", run: () => applyMode(editor, "normal") });

    for (const name of RUST_OPERATIONS) registerOperation(editor, name);

    registerOperation(editor, "insert_text", (args: { text?: string } | undefined) => ({ name: "insert_text", text: args?.text ?? "" }));
    editor.commands.register<{ text: string }, void>({
      id: commandId("edit.insertText"),
      title: "Insert Text",
      async run(_ctx, args) {
        await applyOperation(editor, { name: "insert_text", text: args.text });
      },
    });

    editor.commands.register({
      id: commandId("insert_newline"),
      title: "Insert Newline",
      async run() {
        await applyOperation(editor, { name: "insert_newline" });
      },
    });
    editor.commands.register({ id: commandId("edit.insertNewline"), title: "Insert Newline", run: () => editor.commands.dispatch({ id: commandId("insert_newline") }) });

    editor.commands.register({ id: commandId("smart_tab"), title: "Smart Tab", run: () => applyOperation(editor, { name: "smart_tab", text: "\t" }) });
    editor.commands.register({ id: commandId("insert_tab"), title: "Insert Tab", run: () => applyOperation(editor, { name: "insert_tab", text: "\t" }) });
    editor.commands.register({ id: commandId("edit.deleteBackward"), title: "Delete Backward", run: () => applyOperation(editor, { name: "delete_char_backward" }) });
    editor.commands.register({ id: commandId("edit.deleteForward"), title: "Delete Forward", run: () => applyOperation(editor, { name: "delete_char_forward" }) });

    editor.commands.register({
      id: commandId("delete_selection"),
      title: "Delete Selection",
      async run() {
        await applyOperation(editor, { name: "delete_selection" });
        if (getDefaultState(editor).mode === "select") await applyMode(editor, "normal");
      },
    });
    editor.commands.register({
      id: commandId("delete_selection_noyank"),
      title: "Delete Selection Without Yank",
      async run() {
        await applyOperation(editor, { name: "delete_selection_noyank" });
        if (getDefaultState(editor).mode === "select") await applyMode(editor, "normal");
      },
    });
    editor.commands.register({
      id: commandId("yank"),
      title: "Yank",
      async run() {
        await applyOperation(editor, { name: "yank" });
        await applyMode(editor, "normal");
      },
    });
    editor.commands.register({
      id: commandId("undo"),
      title: "Undo",
      async run(_ctx, args?: { count?: number }) {
        await editor.runtime.surfaces.undo({ surfaceId: activeSurfaceId(editor), count: args?.count });
      },
    });
    editor.commands.register({
      id: commandId("redo"),
      title: "Redo",
      async run(_ctx, args?: { count?: number }) {
        await editor.runtime.surfaces.redo({ surfaceId: activeSurfaceId(editor), count: args?.count });
      },
    });
    editor.commands.register({
      id: commandId("earlier"),
      title: "Earlier",
      async run(_ctx, args?: { count?: number }) {
        await editor.runtime.surfaces.earlier({ surfaceId: activeSurfaceId(editor), count: args?.count });
      },
    });
    editor.commands.register({
      id: commandId("later"),
      title: "Later",
      async run(_ctx, args?: { count?: number }) {
        await editor.runtime.surfaces.later({ surfaceId: activeSurfaceId(editor), count: args?.count });
      },
    });
    editor.commands.register({
      id: commandId("paste_after"),
      title: "Paste After",
      async run() {
        await pasteFromRegister(editor, '"', true);
      },
    });
    editor.commands.register({
      id: commandId("paste_before"),
      title: "Paste Before",
      async run() {
        await pasteFromRegister(editor, '"', false);
      },
    });
    editor.commands.register({
      id: commandId("yank_to_clipboard"),
      title: "Yank To Clipboard",
      async run() {
        await applyOperation(editor, { name: "yank" });
        await mirrorRegisterToClipboard(editor, '"');
        await applyMode(editor, "normal");
      },
    });
    editor.commands.register({
      id: commandId("yank_main_selection_to_clipboard"),
      title: "Yank Main Selection To Clipboard",
      async run() {
        await applyOperation(editor, { name: "yank_main_selection" });
        await mirrorRegisterToClipboard(editor, '"');
        await applyMode(editor, "normal");
      },
    });
    editor.commands.register({
      id: commandId("paste_clipboard_after"),
      title: "Paste Clipboard After",
      async run() {
        await pasteFromClipboard(editor, true);
      },
    });
    editor.commands.register({
      id: commandId("paste_clipboard_before"),
      title: "Paste Clipboard Before",
      async run() {
        await pasteFromClipboard(editor, false);
      },
    });

    editor.commands.register({
      id: commandId("insert_at_line_start"),
      title: "Insert At Line Start",
      async run() {
        await applyOperation(editor, { name: "goto_first_nonwhitespace" });
        await applyMode(editor, "insert");
      },
    });
    editor.commands.register({
      id: commandId("insert_at_line_end"),
      title: "Insert At Line End",
      async run() {
        await applyOperation(editor, { name: "goto_line_end" });
        await applyMode(editor, "insert");
      },
    });
    editor.commands.register({
      id: commandId("append_mode"),
      title: "Append Mode",
      async run() {
        await applyOperation(editor, { name: "move_char_right" });
        await applyMode(editor, "insert");
      },
    });
    editor.commands.register({
      id: commandId("open_below"),
      title: "Open Below",
      async run() {
        await applyOperation(editor, { name: "open_below", lineEnding: "\n" });
        await applyMode(editor, "insert");
      },
    });
    editor.commands.register({
      id: commandId("open_above"),
      title: "Open Above",
      async run() {
        await applyOperation(editor, { name: "open_above", lineEnding: "\n" });
        await applyMode(editor, "insert");
      },
    });
  };
}

function registerOperation(
  editor: EditorHost,
  name: string,
  operation: (args: { count?: number; text?: string } | undefined) => SurfaceOperation = (args) => ({ name, count: args?.count }),
): void {
  editor.commands.register<{ count?: number; text?: string } | undefined, void>({
    id: commandId(name),
    title: name,
    async run(_ctx, args) {
      await applyOperation(editor, operation(args));
    },
  });
}

async function applyMode(editor: EditorHost, mode: "normal" | "insert" | "select"): Promise<void> {
  const previous = getDefaultState(editor).mode;
  if (previous === "insert" && mode !== "insert") {
    await editor.runtime.surfaces.commitUndoCheckpoint({ surfaceId: activeSurfaceId(editor) });
  }
  setEditorMode(editor, mode);
}

async function pasteFromRegister(editor: EditorHost, name: string, after: boolean): Promise<void> {
  const { values } = await editor.runtime.registers.read({ name });
  if (!values?.length) return;
  await applyOperation(editor, { name: "paste", values, after });
  await applyMode(editor, "normal");
}

async function pasteFromClipboard(editor: EditorHost, after: boolean): Promise<void> {
  const { value } = await editor.runtime.clipboard.read({ kind: "Clipboard" });
  if (!value) return;
  await applyOperation(editor, { name: "paste", values: [value], after });
  await applyMode(editor, "normal");
}

async function mirrorRegisterToClipboard(editor: EditorHost, name: string): Promise<void> {
  const { values } = await editor.runtime.registers.read({ name });
  if (!values?.length) return;
  await editor.runtime.clipboard.write({ kind: "Clipboard", value: values.join("\n") });
}

async function applyOperation(editor: EditorHost, operation: SurfaceOperation): Promise<void> {
  await editor.runtime.surfaces.applyOperation({
    surfaceId: activeSurfaceId(editor),
    operation,
  });
  if (getDefaultState(editor).mode !== "insert" && CHECKPOINT_AFTER_OPERATION.has(operation.name)) {
    await editor.runtime.surfaces.commitUndoCheckpoint({ surfaceId: activeSurfaceId(editor) });
  }
}

function activeSurfaceId(editor: EditorHost): SurfaceId {
  const state = getDefaultState(editor);
  if (!state.activeSurfaceId) throw new Error("no active surface");
  return state.activeSurfaceId;
}
