import type {
  CursorSemantics,
  SurfaceChange,
  SurfaceEdit,
  SurfaceId,
  SurfaceOperation,
  SurfaceSelectionRange,
  SurfaceState,
} from "@the-editor/contracts/runtime";
import {
  commandId,
  perfTraceLog,
  type CommandDispatch,
  type EditorHost,
  type EditorModule,
  type KeyInput,
  defineHostStore, } from "@the-editor/editor";
import { perfSpan } from "./perfDiagnostics.js";
import { getDefaultState, refreshActiveSurface, setEditorMode } from "./state.js";
import { surfaceSnapshot, updateSurfaceSnapshot } from "./surfaceSnapshot.js";

const RUST_OPERATIONS = [
  "move_char_left",
  "move_char_right",
  "extend_char_left",
  "extend_char_right",
  "move_visual_line_up",
  "move_visual_line_down",
  "extend_visual_line_up",
  "extend_visual_line_down",
  "match_brackets",
  "extend_match_brackets",
  "move_next_word_start",
  "move_prev_word_start",
  "move_next_word_end",
  "move_next_long_word_start",
  "move_prev_long_word_start",
  "move_next_long_word_end",
  "extend_next_word_start",
  "extend_prev_word_start",
  "extend_next_word_end",
  "extend_next_long_word_start",
  "extend_prev_long_word_start",
  "extend_next_long_word_end",
  "goto_line_start",
  "goto_line_end",
  "goto_line_end_newline",
  "goto_first_nonwhitespace",
  "extend_to_first_nonwhitespace",
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
  "toggle_comments",
  "select_all_children",
  "select_all_siblings",
  "select_prev_sibling",
  "select_next_sibling",
  "move_parent_node_end",
  "move_parent_node_start",
  "extend_parent_node_end",
  "extend_parent_node_start",
] as const;

type FindCharMotionState = {
  kind: "findCharMotion";
  ch: string;
  direction: "forward" | "backward";
  inclusive: boolean;
  extend: boolean;
  count: number;
};

type PendingInputState =
  | {
    kind: "findChar";
    direction: "forward" | "backward";
    inclusive: boolean;
    extend: boolean;
    count: number;
  }
  | {
    kind: "replaceSelection";
  }
  | {
    kind: "surroundAdd";
  }
  | {
    kind: "surroundDelete";
    count: number;
  }
  | {
    kind: "surroundReplaceTarget";
    count: number;
  }
  | {
    kind: "surroundReplaceWith";
    count: number;
    target: string;
  }
  | {
    kind: "selectTextObject";
    textObjectKind: "around" | "inside";
    count: number;
  }
  | {
    kind: "selectRegister";
  };

type LastMotionState = CommandDispatch | FindCharMotionState;
type SelectionPickerMode = "keep" | "remove";

type SelectionPickerState = {
  mode: SelectionPickerMode;
  candidates: number[];
  index: number;
  originalActiveCursorId: number | null;
};

type JumpListEntry = {
  surfaceId: SurfaceId;
  selection: SurfaceSelectionRange[];
  activeCursorId: number | null;
};

type JumpListState = {
  backward: JumpListEntry[];
  forward: JumpListEntry[];
};

const pendingInputSlot = "default.editing.pendingInput";
const lastMotionSlot = "default.editing.lastMotion";
const selectionPickerSlot = "default.editing.selectionPicker";
const jumpListSlot = "default.editing.jumpList";
const objectSelectionHistorySlot = "default.editing.objectSelectionHistory";
const appendRestoreCursorPendingSlot = "default.editing.appendRestoreCursorPending";
const TAB_WIDTH = 4;

const MOTION_COMMANDS = new Set<string>([
  "move_char_left",
  "move_char_right",
  "extend_char_left",
  "extend_char_right",
  "move_visual_line_up",
  "move_visual_line_down",
  "extend_visual_line_up",
  "extend_visual_line_down",
  "match_brackets",
  "extend_match_brackets",
  "move_next_word_start",
  "move_prev_word_start",
  "move_next_word_end",
  "move_next_long_word_start",
  "move_prev_long_word_start",
  "move_next_long_word_end",
  "extend_next_word_start",
  "extend_prev_word_start",
  "extend_next_word_end",
  "extend_next_long_word_start",
  "extend_prev_long_word_start",
  "extend_next_long_word_end",
  "goto_line_start",
  "goto_line_end",
  "goto_line_end_newline",
  "goto_first_nonwhitespace",
  "extend_to_first_nonwhitespace",
  "goto_document_start",
  "goto_file_start",
  "goto_document_end",
  "goto_last_line",
  "extend_to_line_start",
  "extend_to_line_end",
  "extend_to_file_start",
  "extend_to_file_end",
  "extend_to_last_line",
  "extend_line_below",
  "extend_line_above",
  "extend_line_up",
  "page_up",
  "page_down",
  "page_cursor_half_up",
  "page_cursor_half_down",
  "move_parent_node_end",
  "move_parent_node_start",
  "extend_parent_node_end",
  "extend_parent_node_start",
]);

const SEGMENTER = typeof Intl !== "undefined" && "Segmenter" in Intl
  ? new Intl.Segmenter(undefined, { granularity: "grapheme" })
  : undefined;

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
  "surround_add",
  "surround_delete",
  "surround_replace",
  "switch_case",
  "switch_to_uppercase",
  "switch_to_lowercase",
  "indent",
  "unindent",
  "toggle_comments",
]);

const JUMP_SAVING_OPERATIONS = new Set<string>([
  "goto_document_start",
  "goto_file_start",
  "goto_document_end",
  "goto_last_line",
  "extend_to_file_start",
  "extend_to_file_end",
  "extend_to_last_line",
]);

const INSERT_MODE_COLLAPSE_SELECTION_OPERATIONS = new Set<string>([
  "insert_text",
  "insert_tab",
  "smart_tab",
  "insert_newline",
  "delete_char_backward",
  "delete_char_forward",
  "delete_word_backward",
  "delete_word_forward",
  "kill_to_line_start",
  "kill_to_line_end",
]);

export function defaultEditing(): EditorModule {
  return (editor) => {
    editor.commands.register({ id: commandId("normal_mode"), title: "Normal Mode", run: () => applyMode(editor, "normal") });
    editor.commands.register({ id: commandId("mode.normal"), title: "Normal Mode", run: () => applyMode(editor, "normal") });
    editor.commands.register({ id: commandId("exit_insert_mode"), title: "Exit Insert Mode", run: () => exitInsertMode(editor) });
    editor.commands.register({ id: commandId("insert_mode"), title: "Insert Mode", run: () => applyMode(editor, "insert") });
    editor.commands.register({ id: commandId("mode.insert"), title: "Insert Mode", run: () => applyMode(editor, "insert") });
    editor.commands.register({ id: commandId("select_mode"), title: "Select Mode", run: () => applyMode(editor, "select") });
    editor.commands.register({ id: commandId("exit_select_mode"), title: "Exit Select Mode", run: () => applyMode(editor, "normal") });

    editor.hooks.tap("workbench.keyInput", {
      id: "default.editing.pendingInput",
      priority: 100,
      async handler(input: KeyInput) {
        const pending = takePendingInput(editor);
        if (!pending) return undefined;
        await handlePendingInput(editor, pending, input);
        await refreshActiveSurface(editor);
        return true;
      },
    });

    editor.hooks.tap("workbench.keyInput", {
      id: "default.editing.selectionPicker",
      priority: 110,
      async handler(input: KeyInput) {
        const picker = selectionPickerState(editor);
        if (!picker) return undefined;
        const handled = await handleSelectionPickerInput(editor, picker, input);
        if (handled) await refreshActiveSurface(editor);
        return handled;
      },
    });

    editor.commands.register<{ count?: number } | undefined, void>({
      id: commandId("find_next_char"),
      title: "Find Next Char",
      run(_ctx, args) {
        setPendingInput(editor, {
          kind: "findChar",
          direction: "forward",
          inclusive: true,
          extend: false,
          count: args?.count ?? 1,
        });
      },
    });
    editor.commands.register<{ count?: number } | undefined, void>({
      id: commandId("find_till_char"),
      title: "Find Till Char",
      run(_ctx, args) {
        setPendingInput(editor, {
          kind: "findChar",
          direction: "forward",
          inclusive: false,
          extend: false,
          count: args?.count ?? 1,
        });
      },
    });
    editor.commands.register<{ count?: number } | undefined, void>({
      id: commandId("find_prev_char"),
      title: "Find Prev Char",
      run(_ctx, args) {
        setPendingInput(editor, {
          kind: "findChar",
          direction: "backward",
          inclusive: true,
          extend: false,
          count: args?.count ?? 1,
        });
      },
    });
    editor.commands.register<{ count?: number } | undefined, void>({
      id: commandId("till_prev_char"),
      title: "Till Prev Char",
      run(_ctx, args) {
        setPendingInput(editor, {
          kind: "findChar",
          direction: "backward",
          inclusive: false,
          extend: false,
          count: args?.count ?? 1,
        });
      },
    });
    editor.commands.register<{ count?: number } | undefined, void>({
      id: commandId("extend_next_char"),
      title: "Extend Next Char",
      run(_ctx, args) {
        setPendingInput(editor, {
          kind: "findChar",
          direction: "forward",
          inclusive: true,
          extend: true,
          count: args?.count ?? 1,
        });
      },
    });
    editor.commands.register<{ count?: number } | undefined, void>({
      id: commandId("extend_till_char"),
      title: "Extend Till Char",
      run(_ctx, args) {
        setPendingInput(editor, {
          kind: "findChar",
          direction: "forward",
          inclusive: false,
          extend: true,
          count: args?.count ?? 1,
        });
      },
    });
    editor.commands.register<{ count?: number } | undefined, void>({
      id: commandId("extend_prev_char"),
      title: "Extend Prev Char",
      run(_ctx, args) {
        setPendingInput(editor, {
          kind: "findChar",
          direction: "backward",
          inclusive: true,
          extend: true,
          count: args?.count ?? 1,
        });
      },
    });
    editor.commands.register<{ count?: number } | undefined, void>({
      id: commandId("extend_till_prev_char"),
      title: "Extend Till Prev Char",
      run(_ctx, args) {
        setPendingInput(editor, {
          kind: "findChar",
          direction: "backward",
          inclusive: false,
          extend: true,
          count: args?.count ?? 1,
        });
      },
    });
    editor.commands.register<void, void>({
      id: commandId("replace"),
      title: "Replace",
      run() {
        setPendingInput(editor, { kind: "replaceSelection" });
      },
    });
    editor.commands.register<void, void>({
      id: commandId("replace_with_yanked"),
      title: "Replace With Yanked",
      async run() {
        await replaceWithRegister(editor, '"');
      },
    });
    editor.commands.register<void, void>({
      id: commandId("surround_add"),
      title: "Surround Add",
      run() {
        setPendingInput(editor, { kind: "surroundAdd" });
      },
    });
    editor.commands.register<{ count?: number } | undefined, void>({
      id: commandId("surround_delete"),
      title: "Surround Delete",
      run(_ctx, args) {
        setPendingInput(editor, { kind: "surroundDelete", count: args?.count ?? 1 });
      },
    });
    editor.commands.register<{ count?: number } | undefined, void>({
      id: commandId("surround_replace"),
      title: "Surround Replace",
      run(_ctx, args) {
        setPendingInput(editor, { kind: "surroundReplaceTarget", count: args?.count ?? 1 });
      },
    });
    editor.commands.register<{ count?: number } | undefined, void>({
      id: commandId("select_textobject_around"),
      title: "Select Textobject Around",
      run(_ctx, args) {
        setPendingInput(editor, { kind: "selectTextObject", textObjectKind: "around", count: args?.count ?? 1 });
      },
    });
    editor.commands.register<{ count?: number } | undefined, void>({
      id: commandId("select_textobject_inner"),
      title: "Select Textobject Inner",
      run(_ctx, args) {
        setPendingInput(editor, { kind: "selectTextObject", textObjectKind: "inside", count: args?.count ?? 1 });
      },
    });
    editor.commands.register<void, void>({
      id: commandId("repeat_last_motion"),
      title: "Repeat Last Motion",
      async run() {
        const motion = getLastMotion(editor);
        if (!motion) return;
        if (isFindCharMotionState(motion)) {
          await findCharImpl(editor, motion.ch, motion.direction, motion.inclusive, motion.extend, motion.count);
          return;
        }
        await editor.commands.dispatch(motion);
      },
    });
    editor.commands.register<void, void>({
      id: commandId("select_register"),
      title: "Select Register",
      run() {
        setPendingInput(editor, { kind: "selectRegister" });
      },
    });
    editor.commands.register<void, void>({
      id: commandId("expand_selection"),
      title: "Expand Selection",
      async run() {
        await expandSelection(editor);
      },
    });
    editor.commands.register<void, void>({
      id: commandId("shrink_selection"),
      title: "Shrink Selection",
      async run() {
        await shrinkSelection(editor);
      },
    });
    editor.commands.register<void, void>({
      id: commandId("ensure_selections_forward"),
      title: "Ensure Selections Forward",
      async run() {
        await ensureSelectionsForward(editor);
      },
    });

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
      id: commandId("change_selection"),
      title: "Change Selection",
      async run() {
        await changeSelection(editor, true);
      },
    });
    editor.commands.register({
      id: commandId("change_selection_noyank"),
      title: "Change Selection Without Yank",
      async run() {
        await changeSelection(editor, false);
      },
    });
    editor.commands.register({
      id: commandId("copy_selection_on_next_line"),
      title: "Copy Selection On Next Line",
      async run() {
        await copySelectionOnLine(editor, "forward");
      },
    });
    editor.commands.register({
      id: commandId("copy_selection_on_prev_line"),
      title: "Copy Selection On Previous Line",
      async run() {
        await copySelectionOnLine(editor, "backward");
      },
    });
    editor.commands.register({
      id: commandId("select_regex"),
      title: "Select Regex",
      async run() {
        await editor.commands.dispatch({ id: commandId("input_prompt.open"), args: { kind: "selectRegex" } });
      },
    });
    editor.commands.register({
      id: commandId("split_selection"),
      title: "Split Selection",
      async run() {
        await editor.commands.dispatch({ id: commandId("input_prompt.open"), args: { kind: "splitSelection" } });
      },
    });
    editor.commands.register({
      id: commandId("keep_selections"),
      title: "Keep Selections",
      async run() {
        await editor.commands.dispatch({ id: commandId("input_prompt.open"), args: { kind: "keepSelections" } });
      },
    });
    editor.commands.register({
      id: commandId("remove_selections"),
      title: "Remove Selections",
      async run() {
        await editor.commands.dispatch({ id: commandId("input_prompt.open"), args: { kind: "removeSelections" } });
      },
    });
    editor.commands.register({
      id: commandId("search_selection_detect_word_boundaries"),
      title: "Search Selection Detect Word Boundaries",
      async run() {
        await searchSelection(editor, true);
      },
    });
    editor.commands.register({
      id: commandId("search_selection"),
      title: "Search Selection",
      async run() {
        await searchSelection(editor, false);
      },
    });
    editor.commands.register({
      id: commandId("keep_active_selection"),
      title: "Keep Active Selection",
      async run() {
        await openSelectionPicker(editor, "keep");
      },
    });
    editor.commands.register({
      id: commandId("remove_active_selection"),
      title: "Remove Active Selection",
      async run() {
        await openSelectionPicker(editor, "remove");
      },
    });
    editor.commands.register({
      id: commandId("split_selection_on_newline"),
      title: "Split Selection On Newline",
      async run() {
        await splitSelectionOnNewline(editor);
      },
    });
    editor.commands.register({
      id: commandId("join_selections"),
      title: "Join Selections",
      async run() {
        await joinSelections(editor, false);
      },
    });
    editor.commands.register({
      id: commandId("join_selections_space"),
      title: "Join Selections Space",
      async run() {
        await joinSelections(editor, true);
      },
    });
    editor.commands.register({
      id: commandId("align_selections"),
      title: "Align Selections",
      async run() {
        await alignSelections(editor);
      },
    });
    editor.commands.register({
      id: commandId("trim_selections"),
      title: "Trim Selections",
      async run() {
        await trimSelections(editor);
      },
    });
    editor.commands.register({
      id: commandId("page_up"),
      title: "Page Up",
      async run() {
        await pageCursor(editor, "up", "full");
      },
    });
    editor.commands.register({
      id: commandId("page_down"),
      title: "Page Down",
      async run() {
        await pageCursor(editor, "down", "full");
      },
    });
    editor.commands.register({
      id: commandId("page_cursor_half_up"),
      title: "Page Cursor Half Up",
      async run() {
        await pageCursor(editor, "up", "half");
      },
    });
    editor.commands.register({
      id: commandId("page_cursor_half_down"),
      title: "Page Cursor Half Down",
      async run() {
        await pageCursor(editor, "down", "half");
      },
    });
    editor.commands.register({
      id: commandId("jump_backward"),
      title: "Jump Backward",
      async run(_ctx, args?: { count?: number }) {
        await jumpInSelectionHistory(editor, "backward", args?.count ?? 1);
      },
    });
    editor.commands.register({
      id: commandId("jump_forward"),
      title: "Jump Forward",
      async run(_ctx, args?: { count?: number }) {
        await jumpInSelectionHistory(editor, "forward", args?.count ?? 1);
      },
    });
    editor.commands.register({
      id: commandId("save_selection"),
      title: "Save Selection",
      async run() {
        await saveSelection(editor);
      },
    });
    editor.commands.register({
      id: commandId("increment"),
      title: "Increment",
      async run(_ctx, args?: { count?: number }) {
        await incrementSelections(editor, "increase", args?.count ?? 1);
      },
    });
    editor.commands.register({
      id: commandId("decrement"),
      title: "Decrement",
      async run(_ctx, args?: { count?: number }) {
        await incrementSelections(editor, "decrease", args?.count ?? 1);
      },
    });

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
        await enterAppendMode(editor);
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
      if (MOTION_COMMANDS.has(name)) {
        setLastMotion(editor, {
          id: commandId(name),
          args: args ? { ...args } : undefined,
        });
      }
    },
  });
}

function setPendingInput(editor: EditorHost, pending: PendingInputState | undefined): void {
  editor.stores.set(defineHostStore({ id: pendingInputSlot, init: () => undefined }), pending);
}

function takePendingInput(editor: EditorHost): PendingInputState | undefined {
  const pending = editor.stores.get(defineHostStore<PendingInputState | undefined>({ id: pendingInputSlot, init: () => undefined as PendingInputState | undefined }));
  editor.stores.delete(defineHostStore({ id: pendingInputSlot, init: () => undefined }));
  return pending;
}

function isFindCharMotionState(motion: LastMotionState): motion is FindCharMotionState {
  return (motion as FindCharMotionState).kind === "findCharMotion";
}

function setLastMotion(editor: EditorHost, motion: LastMotionState | undefined): void {
  editor.stores.set(defineHostStore({ id: lastMotionSlot, init: () => undefined }), motion);
}

function getLastMotion(editor: EditorHost): LastMotionState | undefined {
  return editor.stores.get(defineHostStore<LastMotionState | undefined>({ id: lastMotionSlot, init: () => undefined as LastMotionState | undefined }));
}

async function handlePendingInput(
  editor: EditorHost,
  pending: PendingInputState,
  input: KeyInput,
): Promise<void> {
  switch (pending.kind) {
    case "findChar": {
      const ch = firstCodePoint(input.text);
      if (!ch) return;
      await findCharImpl(editor, ch, pending.direction, pending.inclusive, pending.extend, pending.count);
      setLastMotion(editor, {
        kind: "findCharMotion",
        ch,
        direction: pending.direction,
        inclusive: pending.inclusive,
        extend: pending.extend,
        count: pending.count,
      });
      return;
    }
    case "replaceSelection": {
      if (input.key === "ret") {
        await replaceSelectionWithStr(editor, (await readActiveSurfaceState(editor)).lineEnding);
        return;
      }
      const ch = firstCodePoint(input.text);
      if (!ch) return;
      await replaceSelectionWithStr(editor, ch);
      return;
    }
    case "surroundAdd": {
      const pair = input.key === "ret"
        ? newlineSurroundPair((await readActiveSurfaceState(editor)).lineEnding)
        : surroundPairForInput(firstCodePoint(input.text));
      if (!pair) return;
      await applyOperation(editor, { name: "surround_add", values: pair });
      if (getDefaultState(editor).mode === "select") await applyMode(editor, "normal");
      return;
    }
    case "surroundDelete": {
      const ch = firstCodePoint(input.text);
      if (input.key !== "m" && !ch) return;
      await applyOperation(editor, {
        name: "surround_delete",
        count: pending.count,
        text: input.key === "m" ? undefined : ch,
      });
      if (getDefaultState(editor).mode === "select") await applyMode(editor, "normal");
      return;
    }
    case "surroundReplaceTarget": {
      const ch = firstCodePoint(input.text);
      if (!ch) return;
      setPendingInput(editor, { kind: "surroundReplaceWith", count: pending.count, target: ch });
      return;
    }
    case "surroundReplaceWith": {
      const pair = surroundPairForInput(firstCodePoint(input.text));
      if (!pair) return;
      await applyOperation(editor, {
        name: "surround_replace",
        count: pending.count,
        text: pending.target,
        values: pair,
      });
      if (getDefaultState(editor).mode === "select") await applyMode(editor, "normal");
      return;
    }
    case "selectTextObject": {
      const ch = firstCodePoint(input.text);
      if (!ch) return;
      const operation = textObjectOperationForInput(pending.textObjectKind, ch, pending.count);
      if (!operation) return;
      await applyOperation(editor, operation);
      return;
    }
    case "selectRegister": {
      const ch = firstCodePoint(input.text);
      if (!ch) return;
      currentRegisterState(editor, ch);
      return;
    }
  }
}

async function findCharImpl(
  editor: EditorHost,
  ch: string,
  direction: "forward" | "backward",
  inclusive: boolean,
  extend: boolean,
  count: number,
): Promise<void> {
  const state = await readActiveSurfaceState(editor);
  const boundaries = graphemeBoundaries(state.text);
  const chars = Array.from(state.text);
  const nextSelection = state.selection.map((range) => {
    const cursor = rangeCursor(range, boundaries, chars.length);
    const searchPos = findCharSearchStart(
      range,
      direction,
      inclusive,
      cursor,
      boundaries,
      chars.length,
    );
    const found = direction === "forward"
      ? findNthNext(chars, ch, searchPos, count)
      : findNthPrev(chars, ch, searchPos, count);
    if (found === undefined) return range;

    const target = inclusive
      ? found
      : direction === "forward"
        ? Math.max(0, found - 1)
        : found + 1;

    return (extend || getDefaultState(editor).mode === "select")
      ? rangePutCursor(range, boundaries, chars.length, target, true)
      : rangePutCursor({ ...range, anchor: cursor, head: cursor }, boundaries, chars.length, target, true);
  });

  await applyEdit(editor, { selection: nextSelection });
}

async function changeSelection(editor: EditorHost, yank: boolean): Promise<void> {
  const state = await readActiveSurfaceState(editor);
  const linewise = selectionIsLinewise(state);

  await applyOperation(editor, { name: yank ? "delete_selection" : "delete_selection_noyank" }, false);
  if (linewise) {
    await applyOperation(editor, { name: "open_above", lineEnding: "\n" }, false);
  }
  await applyMode(editor, "insert");
}

async function copySelectionOnLine(
  editor: EditorHost,
  direction: "forward" | "backward",
): Promise<void> {
  const state = await readActiveSurfaceState(editor);
  const boundaries = graphemeBoundaries(state.text);
  const chars = Array.from(state.text);
  const lines = lineInfos(chars);
  let nextCursorId = state.selection.reduce((max, range) => Math.max(max, range.cursorId), 0) + 1;
  const selection = [...state.selection];

  for (const range of state.selection) {
    const positions = rangeLinePositions(range, boundaries, chars.length, lines);
    if (!positions) continue;

    const height = Math.abs(positions.anchor.line - positions.head.line) + 1;
    let offset = height;

    while (true) {
      const anchorLine = direction === "forward"
        ? positions.anchor.line + offset
        : positions.anchor.line - offset;
      const headLine = direction === "forward"
        ? positions.head.line + offset
        : positions.head.line - offset;
      if (anchorLine < 0 || headLine < 0 || anchorLine >= lines.length || headLine >= lines.length) {
        break;
      }

      const anchorIdx = charAtLineCol(lines, anchorLine, positions.anchor.col);
      const headIdx = charAtLineCol(lines, headLine, positions.head.col);
      if (anchorIdx !== undefined && headIdx !== undefined) {
        selection.push(rangePutCursor(
          { cursorId: nextCursorId++, anchor: anchorIdx, head: anchorIdx },
          boundaries,
          chars.length,
          headIdx,
          true,
        ));
        break;
      }

      if (anchorLine === 0 && headLine === 0) break;
      offset += height;
    }
  }

  await applyEdit(editor, { selection });
}

async function splitSelectionOnNewline(editor: EditorHost): Promise<void> {
  const state = await readActiveSurfaceState(editor);
  const chars = Array.from(state.text);
  const selection: SurfaceSelectionRange[] = [];
  let nextCursorId = state.selection.reduce((max, range) => Math.max(max, range.cursorId), 0) + 1;

  for (const range of state.selection) {
    if (range.anchor === range.head) {
      selection.push({ ...range });
      continue;
    }

    const from = Math.min(range.anchor, range.head);
    const to = Math.max(range.anchor, range.head);
    const startIndex = selection.length;
    let assigned = false;
    let start = from;

    for (let index = from; index < to; index += 1) {
      const lineEndingLength = lineEndingLengthAt(chars, index);
      if (!lineEndingLength) continue;

      const piece = orientSelectionRange(range, start, index);
      const cursorId = !assigned && rangeContainsInclusive(piece, range.head)
        ? (assigned = true, range.cursorId)
        : nextCursorId++;
      selection.push({ cursorId, ...piece });

      start = index + lineEndingLength;
      if (lineEndingLength === 2) index += 1;
    }

    if (start < to) {
      const piece = orientSelectionRange(range, start, to);
      const cursorId = !assigned && rangeContainsInclusive(piece, range.head)
        ? (assigned = true, range.cursorId)
        : nextCursorId++;
      selection.push({ cursorId, ...piece });
    }

    if (selection.length > startIndex && !assigned) {
      selection[startIndex] = { ...selection[startIndex]!, cursorId: range.cursorId };
    }
  }

  await applyEdit(editor, { selection });
}

async function joinSelections(editor: EditorHost, selectSpace: boolean): Promise<void> {
  const state = await readActiveSurfaceState(editor);
  const chars = Array.from(state.text);
  const lines = lineInfos(chars);
  const changes: SurfaceChange[] = [];

  for (const selection of state.selection) {
    const lineRange = selectionLineJoinRange(selection, chars.length, lines);
    if (!lineRange) continue;

    for (let line = lineRange.start; line < lineRange.end; line += 1) {
      const nextLine = lines[line + 1];
      if (!nextLine) break;

      const from = lineContentEndCharIndex(lines, chars, line);
      let to = nextLine.start;
      while (to < nextLine.end && (chars[to] === " " || chars[to] === "\t")) to += 1;

      const separator = to === lineContentEndCharIndex(lines, chars, line + 1) ? undefined : " ";
      changes.push({ start: from, end: to, text: separator });
    }
  }

  if (!changes.length) return;

  changes.sort((a, b) => a.start - b.start || a.end - b.end);
  const deduped = changes.filter((change, index) => {
    const previous = changes[index - 1];
    return !previous
      || previous.start !== change.start
      || previous.end !== change.end
      || previous.text !== change.text;
  });

  const edit: SurfaceEdit = { changes: deduped };
  if (selectSpace) {
    let offset = 0;
    let nextCursorId = state.selection.reduce((max, range) => Math.max(max, range.cursorId), 0) + 1;
    edit.selection = deduped.flatMap((change) => {
      const removed = change.end - change.start;
      if (change.text) {
        const point = change.start - offset;
        offset += removed - change.text.length;
        return [{ cursorId: nextCursorId++, anchor: point, head: point } satisfies SurfaceSelectionRange];
      }
      offset += removed;
      return [];
    });
  }

  await applyEdit(editor, edit, true);
}

async function searchSelection(editor: EditorHost, detectWordBoundaries: boolean): Promise<void> {
  const state = await readActiveSurfaceState(editor);
  const chars = Array.from(state.text);
  const patterns = [...new Set(state.selection.map((selection) => {
    const from = Math.min(selection.anchor, selection.head);
    const to = Math.max(selection.anchor, selection.head);
    const fragment = chars.slice(from, to).join("");
    const prefix = detectWordBoundaries && isAtWordStart(chars, from) ? "\\b" : "";
    const suffix = detectWordBoundaries && isAtWordEnd(chars, to) ? "\\b" : "";
    return `${prefix}${escapeRegex(fragment)}${suffix}`;
  }))].filter(Boolean);
  if (!patterns.length) return;
  await editor.runtime.registers.write({ name: "/", values: [patterns.join("|")] });
}

async function openSelectionPicker(editor: EditorHost, mode: SelectionPickerMode): Promise<void> {
  const state = await readActiveSurfaceState(editor);
  if (!state.selection.length) return;
  if (mode === "remove" && state.selection.length <= 1) return;

  const candidates = state.selection.map((range) => range.cursorId);
  const index = 0;
  editor.stores.set(defineHostStore<SelectionPickerState | undefined>({ id: selectionPickerSlot, init: () => undefined }), {
    mode,
    candidates,
    index,
    originalActiveCursorId: state.activeCursorId,
  } satisfies SelectionPickerState);
  await editor.runtime.surfaces.setActiveCursor({
    surfaceId: activeSurfaceId(editor),
    cursorId: candidates[index] ?? null,
  });
}

function selectionPickerState(editor: EditorHost): SelectionPickerState | undefined {
  return editor.stores.get(defineHostStore<SelectionPickerState | undefined>({ id: selectionPickerSlot, init: () => undefined as SelectionPickerState | undefined }));
}

function clearSelectionPicker(editor: EditorHost): void {
  editor.stores.delete(defineHostStore({ id: selectionPickerSlot, init: () => undefined }));
}

async function handleSelectionPickerInput(
  editor: EditorHost,
  picker: SelectionPickerState,
  input: KeyInput,
): Promise<boolean> {
  switch (input.key) {
    case "up":
      await moveSelectionPicker(editor, -1);
      return true;
    case "down":
      await moveSelectionPicker(editor, 1);
      return true;
    case "ret":
      await confirmSelectionPicker(editor);
      return true;
    case "esc":
      await cancelSelectionPicker(editor);
      return true;
    default:
      return false;
  }
}

async function moveSelectionPicker(editor: EditorHost, delta: number): Promise<void> {
  const picker = selectionPickerState(editor);
  if (!picker?.candidates.length) return;
  picker.index = (picker.index + delta + picker.candidates.length) % picker.candidates.length;
  await editor.runtime.surfaces.setActiveCursor({
    surfaceId: activeSurfaceId(editor),
    cursorId: picker.candidates[picker.index] ?? null,
  });
}

async function confirmSelectionPicker(editor: EditorHost): Promise<void> {
  const picker = selectionPickerState(editor);
  if (!picker) return;
  clearSelectionPicker(editor);
  const cursorId = picker.candidates[picker.index];
  if (cursorId === undefined) return;
  await chooseSelection(editor, cursorId, picker.mode);
}

async function cancelSelectionPicker(editor: EditorHost): Promise<void> {
  const picker = selectionPickerState(editor);
  if (!picker) return;
  clearSelectionPicker(editor);
  await editor.runtime.surfaces.setActiveCursor({
    surfaceId: activeSurfaceId(editor),
    cursorId: picker.originalActiveCursorId,
  });
}

async function chooseSelection(editor: EditorHost, cursorId: number, mode: SelectionPickerMode): Promise<void> {
  const state = await readActiveSurfaceState(editor);
  const index = state.selection.findIndex((range) => range.cursorId === cursorId);
  if (index < 0) return;

  if (mode === "keep") {
    await applyEdit(editor, { selection: [cloneSelectionRange(state.selection[index]!)] });
    await editor.runtime.surfaces.setActiveCursor({ surfaceId: activeSurfaceId(editor), cursorId });
    return;
  }

  if (state.selection.length <= 1) return;
  const nextSelection = state.selection.filter((range) => range.cursorId !== cursorId).map(cloneSelectionRange);
  await applyEdit(editor, { selection: nextSelection });
  await editor.runtime.surfaces.setActiveCursor({
    surfaceId: activeSurfaceId(editor),
    cursorId: nextSelection[Math.min(index, nextSelection.length - 1)]?.cursorId ?? null,
  });
}

async function alignSelections(editor: EditorHost): Promise<void> {
  const state = await readActiveSurfaceState(editor);
  const chars = Array.from(state.text);
  const lines = lineInfos(chars);
  const columns: Array<Array<{ insertPos: number; cursorCol: number }>> = [];
  let lastLine = Number.MAX_SAFE_INTEGER;
  let colIdx = 0;

  for (const range of state.selection) {
    const headLine = lineAtChar(lines, range.head);
    const anchorLine = lineAtChar(lines, range.anchor);
    if (headLine === undefined || anchorLine === undefined || headLine !== anchorLine) return;

    colIdx = headLine === lastLine ? colIdx + 1 : 0;
    columns[colIdx] ??= [];
    columns[colIdx]!.push({
      insertPos: Math.min(range.anchor, range.head),
      cursorCol: visualColumn(chars, lines[headLine]!.start, range.head),
    });
    lastLine = headLine;
  }

  const changes: SurfaceChange[] = [];
  const offsets: number[] = new Array(columns[0]?.length ?? 0).fill(0);

  for (const column of columns) {
    const maxCol = Math.max(0, ...column.map(({ cursorCol }, row) => cursorCol + (offsets[row] ?? 0)));
    for (const [row, entry] of column.entries()) {
      offsets[row] ??= 0;
      const insertCount = Math.max(0, maxCol - (entry.cursorCol + offsets[row]!));
      if (!insertCount) continue;
      offsets[row]! += insertCount;
      changes.push({ start: entry.insertPos, end: entry.insertPos, text: " ".repeat(insertCount) });
    }
  }

  if (!changes.length) return;
  changes.sort((a, b) => a.start - b.start);
  await applyEdit(editor, { changes }, true);
  if (getDefaultState(editor).mode === "select") await applyMode(editor, "normal");
}

async function trimSelections(editor: EditorHost): Promise<void> {
  const state = await readActiveSurfaceState(editor);
  const chars = Array.from(state.text);
  const trimmed = state.selection.flatMap((range) => {
    const from = Math.min(range.anchor, range.head);
    const to = Math.max(range.anchor, range.head);
    if (from === to) return [];
    if (chars.slice(from, to).every((ch) => /\s/.test(ch))) return [];

    let start = from;
    let end = to;
    while (start < end && /\s/.test(chars[start] ?? "")) start += 1;
    while (end > start && /\s/.test(chars[end - 1] ?? "")) end -= 1;
    return [{ cursorId: range.cursorId, ...orientSelectionRange(range, start, end) } satisfies SurfaceSelectionRange];
  });

  if (trimmed.length) {
    await applyEdit(editor, { selection: trimmed });
    return;
  }

  const boundaries = graphemeBoundaries(state.text);
  const collapsed = state.selection.map((range) => {
    const cursor = rangeCursor(range, boundaries, chars.length);
    return { cursorId: range.cursorId, anchor: cursor, head: cursor } satisfies SurfaceSelectionRange;
  });
  const fallback = collapsed[0];
  if (!fallback) return;
  await applyEdit(editor, { selection: [fallback] });
}

async function replaceSelectionWithStr(editor: EditorHost, replacement: string): Promise<void> {
  const state = await readActiveSurfaceState(editor);
  const boundaries = graphemeBoundaries(state.text);
  const chars = Array.from(state.text);
  const changes = state.selection.flatMap((range) => {
    const from = Math.min(range.anchor, range.head);
    const to = range.anchor === range.head
      ? nextGraphemeBoundary(boundaries, from, chars.length)
      : Math.max(range.anchor, range.head);
    const graphemeCount = graphemeCountInRange(boundaries, from, to);
    if (graphemeCount === 0) return [];
    return [{
      start: from,
      end: to,
      text: replacement.repeat(graphemeCount),
    } satisfies SurfaceChange];
  });

  if (changes.length) {
    await applyEdit(editor, { changes }, true);
  }
  if (getDefaultState(editor).mode === "select") {
    await applyMode(editor, "normal");
  }
}

async function replaceWithRegister(editor: EditorHost, name: string): Promise<void> {
  const { values } = await editor.runtime.registers.read({ name });
  const replacement = values?.[0];
  if (!replacement) {
    await applyMode(editor, "normal");
    return;
  }

  const state = await readActiveSurfaceState(editor);
  const boundaries = graphemeBoundaries(state.text);
  const chars = Array.from(state.text);
  const changes = state.selection.map((range) => ({
    start: Math.min(range.anchor, range.head),
    end: range.anchor === range.head
      ? nextGraphemeBoundary(boundaries, Math.min(range.anchor, range.head), chars.length)
      : Math.max(range.anchor, range.head),
    text: replacement,
  } satisfies SurfaceChange));

  await applyEdit(editor, { changes }, true);
  await applyMode(editor, "normal");
}

function jumpListState(editor: EditorHost): JumpListState {
  return editor.stores.host(defineHostStore({ id: jumpListSlot, init: () => ({ backward: [], forward: [] }) }));
}

function cloneJumpEntry(entry: JumpListEntry): JumpListEntry {
  return {
    surfaceId: entry.surfaceId,
    selection: entry.selection.map(cloneSelectionRange),
    activeCursorId: entry.activeCursorId,
  };
}

function sameJumpEntry(left: JumpListEntry, right: JumpListEntry): boolean {
  return left.surfaceId === right.surfaceId
    && left.activeCursorId === right.activeCursorId
    && left.selection.length === right.selection.length
    && left.selection.every((range, index) => {
      const other = right.selection[index];
      return other
        && range.cursorId === other.cursorId
        && range.anchor === other.anchor
        && range.head === other.head;
    });
}

async function currentJumpEntry(editor: EditorHost): Promise<JumpListEntry> {
  const state = await readActiveSurfaceState(editor);
  return {
    surfaceId: activeSurfaceId(editor),
    selection: state.selection.map(cloneSelectionRange),
    activeCursorId: state.activeCursorId,
  };
}

async function saveSelection(editor: EditorHost): Promise<void> {
  const jumpList = jumpListState(editor);
  const current = await currentJumpEntry(editor);
  if (jumpList.backward.at(-1) && sameJumpEntry(jumpList.backward.at(-1)!, current)) return;
  jumpList.backward.push(current);
  jumpList.forward = [];
}

async function jumpInSelectionHistory(
  editor: EditorHost,
  direction: "backward" | "forward",
  count: number,
): Promise<void> {
  const jumpList = jumpListState(editor);
  let current = await currentJumpEntry(editor);
  let remaining = Math.max(1, count);

  while (remaining > 0) {
    const source = direction === "backward" ? jumpList.backward : jumpList.forward;
    const target = source.pop();
    if (!target) break;
    if (sameJumpEntry(target, current)) continue;

    (direction === "backward" ? jumpList.forward : jumpList.backward).push(cloneJumpEntry(current));
    await applyJumpEntry(editor, target);
    current = cloneJumpEntry(target);
    remaining -= 1;
  }
}

async function applyJumpEntry(editor: EditorHost, entry: JumpListEntry): Promise<void> {
  if (entry.surfaceId !== activeSurfaceId(editor)) {
    const pane = editor.workbench.snapshotEditorLayout().panes.find((candidate) => candidate.kind === "surface" && candidate.surfaceId === entry.surfaceId);
    if (!pane) return;
    editor.workbench.focusPane(pane.id);
  }
  await editor.runtime.surfaces.applyEdit({
    surfaceId: entry.surfaceId,
    edit: { selection: entry.selection.map(cloneSelectionRange) },
  });
  await editor.runtime.surfaces.setActiveCursor({
    surfaceId: entry.surfaceId,
    cursorId: entry.activeCursorId,
  });
  await refreshActiveSurface(editor);
  editor.workbench.invalidate("frame", "focus", "layout", "tabs");
}

function currentRegisterState(editor: EditorHost, next?: string): string | undefined {
  const state = getDefaultState(editor);
  if (next !== undefined) state.currentRegister = next;
  return state.currentRegister;
}

function objectSelectionHistory(editor: EditorHost): SurfaceSelectionRange[][] {
  return editor.stores.host(defineHostStore({ id: objectSelectionHistorySlot, init: () => [] }));
}

function cloneSelection(selection: readonly SurfaceSelectionRange[]): SurfaceSelectionRange[] {
  return selection.map(cloneSelectionRange);
}

function selectionContainsSelection(
  current: readonly SurfaceSelectionRange[],
  previous: readonly SurfaceSelectionRange[],
): boolean {
  return previous.every((needle) => current.some((haystack) => {
    const currentFrom = Math.min(haystack.anchor, haystack.head);
    const currentTo = Math.max(haystack.anchor, haystack.head);
    const previousFrom = Math.min(needle.anchor, needle.head);
    const previousTo = Math.max(needle.anchor, needle.head);
    return currentFrom <= previousFrom && currentTo >= previousTo;
  }));
}

async function expandSelection(editor: EditorHost): Promise<void> {
  const before = await readActiveSurfaceState(editor);
  await applyOperation(editor, { name: "expand_selection" });
  const after = await readActiveSurfaceState(editor);
  if (sameSelection(before.selection, after.selection)) return;
  objectSelectionHistory(editor).push(cloneSelection(before.selection));
}

async function shrinkSelection(editor: EditorHost): Promise<void> {
  const before = await readActiveSurfaceState(editor);
  const history = objectSelectionHistory(editor);
  const previous = history.at(-1);
  if (previous && selectionContainsSelection(before.selection, previous)) {
    history.pop();
    await applyEdit(editor, { selection: cloneSelection(previous) });
    return;
  }
  history.length = 0;
  await applyOperation(editor, { name: "shrink_selection" });
}

async function ensureSelectionsForward(editor: EditorHost): Promise<void> {
  const state = await readActiveSurfaceState(editor);
  const selection = state.selection.map((range) => ({
    cursorId: range.cursorId,
    anchor: Math.min(range.anchor, range.head),
    head: Math.max(range.anchor, range.head),
  } satisfies SurfaceSelectionRange));
  if (sameSelection(state.selection, selection)) return;
  await applyEdit(editor, { selection });
}

function sameSelection(
  left: readonly SurfaceSelectionRange[],
  right: readonly SurfaceSelectionRange[],
): boolean {
  return left.length === right.length
    && left.every((range, index) => {
      const other = right[index];
      return other
        && range.cursorId === other.cursorId
        && range.anchor === other.anchor
        && range.head === other.head;
    });
}

async function incrementSelections(
  editor: EditorHost,
  direction: "increase" | "decrease",
  count: number,
): Promise<void> {
  const state = await readActiveSurfaceState(editor);
  const sign = direction === "increase" ? 1 : -1;
  let amount = sign * Math.max(1, count);
  const increaseBy = currentRegisterState(editor) === "#" ? sign : 0;
  const chars = Array.from(state.text);
  const changes: SurfaceChange[] = [];
  const selection: SurfaceSelectionRange[] = [];
  let cumulativeLengthDiff = 0;

  for (const range of state.selection) {
    const from = Math.min(range.anchor, range.head);
    const to = Math.max(range.anchor, range.head);
    const selectedText = chars.slice(from, to).join("");
    const nextText = incrementInteger(selectedText, amount) ?? incrementDateTime(selectedText, amount);
    const newFrom = from + cumulativeLengthDiff;

    amount += increaseBy;

    if (nextText === undefined) {
      selection.push(cloneSelectionRange(range));
      continue;
    }

    const newLength = Array.from(nextText).length;
    const newTo = newFrom + newLength;
    selection.push({ cursorId: range.cursorId, ...orientSelectionRange(range, newFrom, newTo) });
    cumulativeLengthDiff += newLength - (to - from);
    changes.push({ start: from, end: to, text: nextText });
  }

  if (!changes.length) return;
  await applyEdit(editor, { changes, selection }, true);
  if (getDefaultState(editor).mode === "select") await applyMode(editor, "normal");
}

async function pageCursor(
  editor: EditorHost,
  direction: "up" | "down",
  size: "full" | "half",
): Promise<void> {
  const current = await readActiveSurfaceState(editor);
  if (!current.viewport.rowCount) return;
  const count = size === "half"
    ? Math.max(1, Math.floor(current.viewport.rowCount / 2))
    : Math.max(1, current.viewport.rowCount - 2);
  const mode = getDefaultState(editor).mode;
  const movement = direction === "up"
    ? (mode === "select" ? "extend_visual_line_up" : "move_visual_line_up")
    : (mode === "select" ? "extend_visual_line_down" : "move_visual_line_down");
  await applyOperation(editor, { name: movement, count }, false);
  const next = await readActiveSurfaceState(editor);
  const firstRow = direction === "up"
    ? Math.max(0, current.viewport.firstRow - count)
    : current.viewport.firstRow + count;
  await editor.runtime.surfaces.setViewport({
    surfaceId: activeSurfaceId(editor),
    viewport: { ...next.viewport, firstRow },
  });
}

async function readActiveSurfaceState(editor: EditorHost, options: { preferSnapshot?: boolean } = {}): Promise<SurfaceState> {
  const surfaceId = activeSurfaceId(editor);
  const cached = options.preferSnapshot ? surfaceSnapshot(editor, surfaceId) : undefined;
  if (cached) return cached;
  const span = perfSpan("editing", "readActiveSurfaceState", { surfaceId, preferSnapshot: Boolean(options.preferSnapshot) }, 8);
  const state = await editor.runtime.surfaces.readState({ surfaceId });
  span.end({ textLength: state.text.length, revision: state.revision });
  updateSurfaceSnapshot(editor, surfaceId, state);
  return state;
}

async function applyEdit(editor: EditorHost, edit: SurfaceEdit, checkpoint = false): Promise<void> {
  const surfaceId = activeSurfaceId(editor);
  perfTraceLog(editor, "runtime", "applyEdit.start", { surfaceId });
  const state = await editor.runtime.surfaces.applyEdit({ surfaceId, edit });
  perfTraceLog(editor, "runtime", "applyEdit.end", { surfaceId, revision: state.revision, textLength: state.text.length });
  updateSurfaceSnapshot(editor, surfaceId, state);
  if (checkpoint && getDefaultState(editor).mode !== "insert") {
    await editor.runtime.surfaces.commitUndoCheckpoint({ surfaceId });
  }
}

function surroundPairForInput(ch: string | undefined): [string, string] | undefined {
  if (!ch) return undefined;
  switch (ch) {
    case "(":
    case ")":
      return ["(", ")"];
    case "[":
    case "]":
      return ["[", "]"];
    case "{":
    case "}":
      return ["{", "}"];
    case "<":
    case ">":
      return ["<", ">"];
    case '"':
      return ['"', '"'];
    case "'":
      return ["'", "'"];
    case "`":
      return ["`", "`"];
    default:
      return [ch, ch];
  }
}

function newlineSurroundPair(lineEnding: string): [string, string] {
  return [lineEnding, lineEnding];
}

function textObjectOperationForInput(
  kind: "around" | "inside",
  ch: string,
  count: number,
): SurfaceOperation | undefined {
  const suffix = kind === "inside" ? "inner" : "around";
  switch (ch) {
    case "w":
      return { name: `select_word_${suffix}`, count };
    case "W":
      return { name: `select_long_word_${suffix}`, count };
    case "p":
      return { name: `select_paragraph_${suffix}`, count };
    case "m":
      return { name: `select_closest_pair_${suffix}`, count };
    case "t":
      return { name: `select_syntax_object_${suffix}`, count, text: "class" };
    case "f":
      return { name: `select_syntax_object_${suffix}`, count, text: "function" };
    case "a":
      return { name: `select_syntax_object_${suffix}`, count, text: "parameter" };
    case "c":
      return { name: `select_syntax_object_${suffix}`, count, text: "comment" };
    case "T":
      return { name: `select_syntax_object_${suffix}`, count, text: "test" };
    case "e":
      return { name: `select_syntax_object_${suffix}`, count, text: "entry" };
    case "x":
      return { name: `select_syntax_object_${suffix}`, count, text: "xml-element" };
    default:
      return ch.match(/^[^A-Za-z0-9]$/)
        ? { name: `select_pair_${suffix}`, count, text: ch }
        : undefined;
  }
}

function selectionIsLinewise(state: SurfaceState): boolean {
  const chars = Array.from(state.text);
  const lineStarts = new Set(lineInfos(chars).map((line) => line.start));
  return state.selection.every((range) => {
    const from = Math.min(range.anchor, range.head);
    const to = Math.max(range.anchor, range.head);
    return from !== to
      && lineStarts.has(from)
      && lineStarts.has(to)
      && chars.slice(from, to).includes("\n");
  });
}

function cloneSelectionRange(range: SurfaceSelectionRange): SurfaceSelectionRange {
  return { ...range };
}

function orientSelectionRange(
  range: SurfaceSelectionRange,
  start: number,
  end: number,
): Pick<SurfaceSelectionRange, "anchor" | "head"> {
  return range.anchor <= range.head
    ? { anchor: start, head: end }
    : { anchor: end, head: start };
}

function rangeContainsInclusive(
  range: Pick<SurfaceSelectionRange, "anchor" | "head">,
  pos: number,
): boolean {
  const from = Math.min(range.anchor, range.head);
  const to = Math.max(range.anchor, range.head);
  return from <= pos && pos <= to;
}

function lineEndingLengthAt(chars: readonly string[], index: number): number {
  const ch = chars[index];
  if (ch === "\r" && chars[index + 1] === "\n") return 2;
  if (ch === "\n" || ch === "\r") return 1;
  return 0;
}

type LineInfo = {
  start: number;
  end: number;
};

function lineInfos(chars: readonly string[]): LineInfo[] {
  const lines: LineInfo[] = [];
  let start = 0;
  for (let index = 0; index < chars.length; index += 1) {
    if (chars[index] !== "\n") continue;
    lines.push({ start, end: index + 1 });
    start = index + 1;
  }
  lines.push({ start, end: chars.length });
  return lines;
}

function lineAtChar(lines: readonly LineInfo[], charIdx: number): number | undefined {
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (charIdx < line.end || (charIdx === line.start && line.start === line.end)) return index;
  }
  return undefined;
}

function visualColumn(chars: readonly string[], lineStart: number, charIdx: number): number {
  let col = 0;
  for (let index = lineStart; index < charIdx; index += 1) {
    col += chars[index] === "\t" ? TAB_WIDTH : 1;
  }
  return col;
}

function lineContentEndCharIndex(lines: readonly LineInfo[], chars: readonly string[], line: number): number {
  const info = lines[line];
  if (!info) return chars.length;
  let end = info.end;
  if (end > info.start && chars[end - 1] === "\n") end -= 1;
  if (end > info.start && chars[end - 1] === "\r") end -= 1;
  return end;
}

function selectionLineJoinRange(
  range: SurfaceSelectionRange,
  len: number,
  lines: readonly LineInfo[],
): { start: number; end: number } | undefined {
  const from = Math.min(range.anchor, range.head);
  const to = Math.max(range.anchor, range.head);
  const startLine = lineAtChar(lines, from);
  if (startLine === undefined) return undefined;

  const endChar = from === to
    ? from
    : Math.max(from, Math.min(len, to) - 1);
  const endLine = to >= len
    ? lines.length - 1
    : lineAtChar(lines, Math.max(0, endChar));
  if (endLine === undefined) return undefined;

  return startLine === endLine
    ? { start: startLine, end: Math.min(startLine + 1, lines.length - 1) }
    : { start: startLine, end: endLine };
}

function charAtLineCol(lines: readonly LineInfo[], line: number, col: number): number | undefined {
  const info = lines[line];
  if (!info) return undefined;
  const idx = info.start + col;
  return idx < info.end ? idx : undefined;
}

function rangeLinePositions(
  range: SurfaceSelectionRange,
  boundaries: readonly number[],
  len: number,
  lines: readonly LineInfo[],
): { anchor: { line: number; col: number }; head: { line: number; col: number } } | undefined {
  const anchorChar = range.anchor === range.head
    ? range.anchor
    : range.anchor < range.head
      ? range.anchor
      : prevGraphemeBoundary(boundaries, range.anchor);
  const headChar = range.anchor === range.head
    ? range.head
    : range.anchor < range.head
      ? prevGraphemeBoundary(boundaries, range.head)
      : range.head;
  if (anchorChar >= len || headChar >= len) return undefined;

  const anchorLine = lineAtChar(lines, anchorChar);
  const headLine = lineAtChar(lines, headChar);
  if (anchorLine === undefined || headLine === undefined) return undefined;

  return {
    anchor: { line: anchorLine, col: anchorChar - lines[anchorLine].start },
    head: { line: headLine, col: headChar - lines[headLine].start },
  };
}

function findCharSearchStart(
  range: SurfaceSelectionRange,
  direction: "forward" | "backward",
  inclusive: boolean,
  cursor: number,
  boundaries: readonly number[],
  len: number,
): number {
  const isEmpty = range.anchor === range.head;
  if (direction === "forward") {
    if (isEmpty) return Math.min(cursor + 1, len);
    if (inclusive) return Math.min(nextGraphemeBoundary(boundaries, cursor, len), len);
    return Math.min(nextGraphemeBoundary(boundaries, cursor, len) + 1, len);
  }

  if (isEmpty) return cursor;
  if (inclusive) return Math.max(0, cursor - 1);
  return Math.max(0, prevGraphemeBoundary(boundaries, cursor) - 1);
}

function graphemeBoundaries(text: string): number[] {
  const boundaries: number[] = [];
  let index = 0;
  if (SEGMENTER) {
    for (const segment of SEGMENTER.segment(text)) {
      boundaries.push(index);
      index += Array.from(segment.segment).length;
    }
  } else {
    for (const ch of Array.from(text)) {
      boundaries.push(index);
      index += Array.from(ch).length;
    }
  }
  return boundaries;
}

function nextGraphemeBoundary(boundaries: readonly number[], charIdx: number, len: number): number {
  for (const boundary of boundaries) {
    if (boundary > charIdx) return boundary;
  }
  return len;
}

function prevGraphemeBoundary(boundaries: readonly number[], charIdx: number): number {
  let previous = 0;
  for (const boundary of boundaries) {
    if (boundary >= charIdx) break;
    previous = boundary;
  }
  return previous;
}

function graphemeCountInRange(boundaries: readonly number[], start: number, end: number): number {
  let count = 0;
  for (const boundary of boundaries) {
    if (boundary < start) continue;
    if (boundary >= end) break;
    count += 1;
  }
  return count;
}

function rangeCursor(range: SurfaceSelectionRange, boundaries: readonly number[], len: number): number {
  if (range.head > range.anchor) {
    return prevGraphemeBoundary(boundaries, range.head);
  }
  return Math.min(range.head, len);
}

function rangePutCursor(
  range: SurfaceSelectionRange,
  boundaries: readonly number[],
  len: number,
  charIdx: number,
  extend: boolean,
): SurfaceSelectionRange {
  const target = Math.max(0, Math.min(charIdx, len));
  if (!extend) {
    return { ...range, anchor: target, head: target };
  }

  const anchor = range.head >= range.anchor
    ? (target < range.anchor ? nextGraphemeBoundary(boundaries, range.anchor, len) : range.anchor)
    : (target >= range.anchor ? prevGraphemeBoundary(boundaries, range.anchor) : range.anchor);

  if (anchor <= target) {
    return { ...range, anchor, head: nextGraphemeBoundary(boundaries, target, len) };
  }

  return { ...range, anchor, head: target };
}

function findNthNext(chars: readonly string[], ch: string, start: number, count: number): number | undefined {
  let remaining = Math.max(1, count);
  for (let index = Math.max(0, start); index < chars.length; index += 1) {
    if (chars[index] !== ch) continue;
    remaining -= 1;
    if (remaining === 0) return index;
  }
  return undefined;
}

function findNthPrev(chars: readonly string[], ch: string, start: number, count: number): number | undefined {
  let remaining = Math.max(1, count);
  if (!chars.length) return undefined;
  for (let index = Math.min(Math.max(0, start), chars.length - 1); index >= 0; index -= 1) {
    if (chars[index] !== ch) continue;
    remaining -= 1;
    if (remaining === 0) return index;
  }
  return undefined;
}

function firstCodePoint(text: string | undefined): string | undefined {
  return text ? Array.from(text)[0] : undefined;
}

function isWordChar(ch: string | undefined): boolean {
  return Boolean(ch?.match(/^[\p{L}\p{N}_]$/u));
}

function isAtWordStart(chars: readonly string[], index: number): boolean {
  if (index >= chars.length) return false;
  const ch = chars[index];
  if (index === 0) return isWordChar(ch);
  return !isWordChar(chars[index - 1]) && isWordChar(ch);
}

function isAtWordEnd(chars: readonly string[], index: number): boolean {
  if (index === 0 || index >= chars.length) return false;
  return isWordChar(chars[index - 1]) && !isWordChar(chars[index]);
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function incrementInteger(selectedText: string, amount: number): string | undefined {
  const separator = "_";
  if (!selectedText || selectedText.startsWith(separator) || selectedText.endsWith(separator)) return undefined;

  const radix = selectedText.startsWith("0x")
    ? 16
    : selectedText.startsWith("0o")
      ? 8
      : selectedText.startsWith("0b")
        ? 2
        : 10;

  const separatorRtlIndexes = Array.from(selectedText).reverse().flatMap((ch, index) => ch === separator ? [index] : []);
  const word = selectedText.replaceAll(separator, "");
  let nextText: string;

  if (radix === 10) {
    let value: bigint;
    try {
      value = BigInt(word);
    } catch {
      return undefined;
    }
    const nextValue = value + BigInt(amount);
    const formatLength = (
      value < 0n && nextValue >= 0n
        ? word.length - 1
        : value >= 0n && nextValue < 0n
          ? word.length + 1
          : word.length
    ) - separatorRtlIndexes.length;

    nextText = word.startsWith("0") || word.startsWith("-0")
      ? padSignedDecimal(nextValue, formatLength)
      : nextValue.toString();
  } else {
    const digits = word.slice(2);
    let value: bigint;
    try {
      value = BigInt(word);
    } catch {
      return undefined;
    }
    const nextValue = value + BigInt(amount) < 0n ? 0n : value + BigInt(amount);
    const formatLength = selectedText.length - 2 - separatorRtlIndexes.length;

    if (radix === 2) nextText = `0b${nextValue.toString(2).padStart(formatLength, "0")}`;
    else if (radix === 8) nextText = `0o${nextValue.toString(8).padStart(formatLength, "0")}`;
    else {
      const lowerCount = Array.from(digits).filter((ch) => ch >= "a" && ch <= "f").length;
      const upperCount = Array.from(digits).filter((ch) => ch >= "A" && ch <= "F").length;
      const rendered = nextValue.toString(16).padStart(formatLength, "0");
      nextText = `0x${upperCount > lowerCount ? rendered.toUpperCase() : rendered}`;
    }
  }

  for (const rtlIndex of separatorRtlIndexes) {
    if (rtlIndex >= nextText.length) continue;
    const insertAt = nextText.length - rtlIndex;
    if (insertAt <= 0) continue;
    nextText = `${nextText.slice(0, insertAt)}_${nextText.slice(insertAt)}`;
  }

  if (nextText.length > selectedText.length && separatorRtlIndexes.length) {
    const spacing = separatorRtlIndexes.length > 1
      ? separatorRtlIndexes[separatorRtlIndexes.length - 1]! - separatorRtlIndexes[separatorRtlIndexes.length - 2]! - 1
      : separatorRtlIndexes[0]!;
    const prefixLength = radix === 10 ? 0 : 2;
    let index = nextText.indexOf("_");
    while (index >= 0 && index - prefixLength > spacing) {
      index -= spacing;
      nextText = `${nextText.slice(0, index)}_${nextText.slice(index)}`;
    }
  }

  return nextText;
}

function padSignedDecimal(value: bigint, width: number): string {
  const rendered = value.toString();
  if (rendered.startsWith("-")) return `-${rendered.slice(1).padStart(Math.max(0, width - 1), "0")}`;
  return rendered.padStart(width, "0");
}

const SHORT_MONTH_NAMES = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const SHORT_WEEKDAY_NAMES = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

function incrementDateTime(selectedText: string, amount: number): string | undefined {
  return incrementDateTimeYmd(selectedText, amount)
    ?? incrementDateTimeTextualDate(selectedText, amount)
    ?? incrementClockTime(selectedText, amount);
}

function incrementDateTimeYmd(selectedText: string, amount: number): string | undefined {
  const match = selectedText.match(/^(\d{4})([-\/])(\d{2})\2(\d{2})(?: (\d{2}):(\d{2})(?::(\d{2}))?)?$/);
  if (!match) return undefined;
  const [, yearText, separator, monthText, dayText, hourText, minuteText, secondText] = match;
  const year = Number(yearText);
  const month = Number(monthText);
  const day = Number(dayText);
  const hour = Number(hourText ?? "0");
  const minute = Number(minuteText ?? "0");
  const second = Number(secondText ?? "0");
  const date = new Date(Date.UTC(year, month - 1, day, hour, minute, second));
  if (!sameUtcDateTimeParts(date, year, month, day, hour, minute, second)) return undefined;

  const next = new Date(date.getTime() + amount * (hourText ? 60_000 : 86_400_000));
  const y = next.getUTCFullYear().toString().padStart(4, "0");
  const m = `${next.getUTCMonth() + 1}`.padStart(2, "0");
  const d = `${next.getUTCDate()}`.padStart(2, "0");
  if (!hourText) return `${y}${separator}${m}${separator}${d}`;

  const hh = `${next.getUTCHours()}`.padStart(2, "0");
  const mm = `${next.getUTCMinutes()}`.padStart(2, "0");
  if (!secondText) return `${y}${separator}${m}${separator}${d} ${hh}:${mm}`;
  const ss = `${next.getUTCSeconds()}`.padStart(2, "0");
  return `${y}${separator}${m}${separator}${d} ${hh}:${mm}:${ss}`;
}

function incrementDateTimeTextualDate(selectedText: string, amount: number): string | undefined {
  const weekdayMonthDayYear = selectedText.match(/^(Sun|Mon|Tue|Wed|Thu|Fri|Sat) (Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) (\d{2}) (\d{4})$/);
  if (weekdayMonthDayYear) {
    const [, , monthName, dayText, yearText] = weekdayMonthDayYear;
    const next = addUtcDays(Number(yearText), monthIndex(monthName) + 1, Number(dayText), amount);
    if (!next) return undefined;
    return `${SHORT_WEEKDAY_NAMES[next.getUTCDay()]} ${SHORT_MONTH_NAMES[next.getUTCMonth()]} ${`${next.getUTCDate()}`.padStart(2, "0")} ${next.getUTCFullYear().toString().padStart(4, "0")}`;
  }

  const dayMonthYear = selectedText.match(/^(\d{2})-(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)-(\d{4})$/);
  if (dayMonthYear) {
    const [, dayText, monthName, yearText] = dayMonthYear;
    const next = addUtcDays(Number(yearText), monthIndex(monthName) + 1, Number(dayText), amount);
    if (!next) return undefined;
    return `${`${next.getUTCDate()}`.padStart(2, "0")}-${SHORT_MONTH_NAMES[next.getUTCMonth()]}-${next.getUTCFullYear().toString().padStart(4, "0")}`;
  }

  const yearMonthDay = selectedText.match(/^(\d{4}) (Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) (\d{2})$/);
  if (yearMonthDay) {
    const [, yearText, monthName, dayText] = yearMonthDay;
    const next = addUtcDays(Number(yearText), monthIndex(monthName) + 1, Number(dayText), amount);
    if (!next) return undefined;
    return `${next.getUTCFullYear().toString().padStart(4, "0")} ${SHORT_MONTH_NAMES[next.getUTCMonth()]} ${`${next.getUTCDate()}`.padStart(2, "0")}`;
  }

  const monthDayYear = selectedText.match(/^(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) (\d{2}), (\d{4})$/);
  if (monthDayYear) {
    const [, monthName, dayText, yearText] = monthDayYear;
    const next = addUtcDays(Number(yearText), monthIndex(monthName) + 1, Number(dayText), amount);
    if (!next) return undefined;
    return `${SHORT_MONTH_NAMES[next.getUTCMonth()]} ${`${next.getUTCDate()}`.padStart(2, "0")}, ${next.getUTCFullYear().toString().padStart(4, "0")}`;
  }

  return undefined;
}

function incrementClockTime(selectedText: string, amount: number): string | undefined {
  const twelveHour = selectedText.match(/^([1-9]|1[0-2]):([0-5]\d)(?::([0-5]\d))? (am|pm|AM|PM)$/);
  if (twelveHour) {
    const [, hourText, minuteText, secondText, meridiem] = twelveHour;
    let hour = Number(hourText) % 12;
    if (meridiem.toLowerCase() === "pm") hour += 12;
    const totalSeconds = ((hour * 60) + Number(minuteText)) * 60 + Number(secondText ?? "0");
    const next = mod(totalSeconds + amount * 60, 24 * 60 * 60);
    const nextHour24 = Math.floor(next / 3600);
    const nextMinute = Math.floor((next % 3600) / 60);
    const nextSecond = next % 60;
    const nextMeridiem = nextHour24 >= 12 ? "pm" : "am";
    const nextHour12 = nextHour24 % 12 || 12;
    const renderedMeridiem = meridiem === meridiem.toUpperCase() ? nextMeridiem.toUpperCase() : nextMeridiem;
    if (!secondText) return `${nextHour12}:${`${nextMinute}`.padStart(2, "0")} ${renderedMeridiem}`;
    return `${nextHour12}:${`${nextMinute}`.padStart(2, "0")}:${`${nextSecond}`.padStart(2, "0")} ${renderedMeridiem}`;
  }

  const twentyFourHour = selectedText.match(/^(\d{2}):(\d{2})(?::(\d{2}))?$/);
  if (!twentyFourHour) return undefined;
  const [, hourText, minuteText, secondText] = twentyFourHour;
  const hour = Number(hourText);
  const minute = Number(minuteText);
  const second = Number(secondText ?? "0");
  if (hour > 23) return undefined;
  const totalSeconds = ((hour * 60) + minute) * 60 + second;
  const next = mod(totalSeconds + amount * 60, 24 * 60 * 60);
  const nextHour = Math.floor(next / 3600);
  const nextMinute = Math.floor((next % 3600) / 60);
  const nextSecond = next % 60;
  if (!secondText) return `${`${nextHour}`.padStart(2, "0")}:${`${nextMinute}`.padStart(2, "0")}`;
  return `${`${nextHour}`.padStart(2, "0")}:${`${nextMinute}`.padStart(2, "0")}:${`${nextSecond}`.padStart(2, "0")}`;
}

function monthIndex(value: string): number {
  return SHORT_MONTH_NAMES.indexOf(value);
}

function addUtcDays(year: number, month: number, day: number, amount: number): Date | undefined {
  const date = new Date(Date.UTC(year, month - 1, day));
  if (!sameUtcDateTimeParts(date, year, month, day, 0, 0, 0)) return undefined;
  return new Date(date.getTime() + amount * 86_400_000);
}

function sameUtcDateTimeParts(date: Date, year: number, month: number, day: number, hour: number, minute: number, second: number): boolean {
  return date.getUTCFullYear() === year
    && date.getUTCMonth() + 1 === month
    && date.getUTCDate() === day
    && date.getUTCHours() === hour
    && date.getUTCMinutes() === minute
    && date.getUTCSeconds() === second;
}

function mod(value: number, base: number): number {
  return ((value % base) + base) % base;
}

async function applyMode(editor: EditorHost, mode: "normal" | "insert" | "select"): Promise<void> {
  const previous = getDefaultState(editor).mode;
  if (previous === "insert" && mode !== "insert") {
    if (appendRestoreCursorPending(editor)) {
      await restoreAppendModeSelection(editor);
      setAppendRestoreCursorPending(editor, false);
    }
    await editor.runtime.surfaces.commitUndoCheckpoint({ surfaceId: activeSurfaceId(editor) });
  }

  if (mode !== "insert") {
    setAppendRestoreCursorPending(editor, false);
  }

  if (previous === "normal" && mode === "insert") {
    await applyInsertModeSelection(editor);
  }

  setEditorMode(editor, mode);
  await syncActiveSurfaceCursorSemantics(editor, mode === "insert" ? "InsertionPoint" : "Cell");
}

async function exitInsertMode(editor: EditorHost): Promise<void> {
  if (getDefaultState(editor).mode !== "insert") {
    await applyMode(editor, "normal");
    return;
  }
  await applyMode(editor, "normal");
  await editor.overlays.dismissAttachedToEditor({ restoreFocus: false, fallback: { kind: "editor" } });
}

async function enterAppendMode(editor: EditorHost): Promise<void> {
  await enterInsertModeDirect(editor);
  setAppendRestoreCursorPending(editor, true);
  const state = await readActiveSurfaceState(editor);
  const boundaries = graphemeBoundaries(state.text);
  const len = Array.from(state.text).length;
  const selection = state.selection.map((range) => appendModeSelectionRange(range, boundaries, len));
  if (!sameSelection(state.selection, selection)) {
    await applyEdit(editor, { selection });
  }
}

async function enterInsertModeDirect(editor: EditorHost): Promise<void> {
  setAppendRestoreCursorPending(editor, false);
  setEditorMode(editor, "insert");
  await syncActiveSurfaceCursorSemantics(editor, "InsertionPoint");
}

async function applyInsertModeSelection(editor: EditorHost): Promise<void> {
  const state = await readActiveSurfaceState(editor);
  const selection = state.selection.map(insertModeSelectionRange);
  if (sameSelection(state.selection, selection)) return;
  await applyEdit(editor, { selection });
}

function insertModeSelectionRange(range: SurfaceSelectionRange): SurfaceSelectionRange {
  const from = Math.min(range.anchor, range.head);
  const to = Math.max(range.anchor, range.head);
  return { cursorId: range.cursorId, anchor: to, head: from };
}

async function restoreAppendModeSelection(editor: EditorHost): Promise<void> {
  const state = await readActiveSurfaceState(editor);
  const boundaries = graphemeBoundaries(state.text);
  const selection = state.selection.map((range) => {
    const head = range.head;
    return {
      cursorId: range.cursorId,
      anchor: prevGraphemeBoundary(boundaries, head),
      head,
    } satisfies SurfaceSelectionRange;
  });
  if (sameSelection(state.selection, selection)) return;
  await applyEdit(editor, { selection });
}

function appendModeSelectionRange(
  range: SurfaceSelectionRange,
  boundaries: readonly number[],
  len: number,
): SurfaceSelectionRange {
  if (range.anchor === range.head) {
    const next = nextGraphemeBoundary(boundaries, range.head, len);
    return { cursorId: range.cursorId, anchor: range.anchor, head: next };
  }
  return {
    cursorId: range.cursorId,
    anchor: Math.min(range.anchor, range.head),
    head: Math.max(range.anchor, range.head),
  };
}

function appendRestoreCursorPending(editor: EditorHost): boolean {
  return editor.stores.get(defineHostStore<boolean | undefined>({ id: appendRestoreCursorPendingSlot, init: () => undefined as boolean | undefined })) ?? false;
}

function setAppendRestoreCursorPending(editor: EditorHost, pending: boolean): void {
  if (pending) editor.stores.set(defineHostStore({ id: appendRestoreCursorPendingSlot, init: () => undefined }), true);
  else editor.stores.delete(defineHostStore({ id: appendRestoreCursorPendingSlot, init: () => undefined }));
}

async function syncActiveSurfaceCursorSemantics(editor: EditorHost, semantics: CursorSemantics): Promise<void> {
  const surfaceId = getDefaultState(editor).activeSurfaceId;
  if (!surfaceId) return;
  await editor.runtime.surfaces.setCursorSemantics({ surfaceId, semantics });
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

async function applyOperation(
  editor: EditorHost,
  operation: SurfaceOperation,
  checkpoint = true,
): Promise<void> {
  const surfaceId = getDefaultState(editor).activeSurfaceId;
  if (!surfaceId) return;
  perfTraceLog(editor, "runtime", "applyOperation.start", { surfaceId, operation: operation.name });
  const span = perfSpan("editing", "applyOperation", { surfaceId, operation: operation.name, mode: getDefaultState(editor).mode }, 8);
  if (getDefaultState(editor).mode === "insert" && INSERT_MODE_COLLAPSE_SELECTION_OPERATIONS.has(operation.name)) {
    const collapseSpan = perfSpan("editing", "collapseInsertSelections", { surfaceId, operation: operation.name }, 4);
    await collapseInsertModeSelections(editor);
    collapseSpan.end();
  }
  if (JUMP_SAVING_OPERATIONS.has(operation.name)) {
    await saveSelection(editor);
  }
  const runtimeSpan = perfSpan("editing", "runtime.applyOperation", { surfaceId, operation: operation.name }, 4);
  const state = await editor.runtime.surfaces.applyOperation({
    surfaceId,
    operation,
  });
  runtimeSpan.end({ textLength: state.text.length, revision: state.revision });
  updateSurfaceSnapshot(editor, surfaceId, state);
  if (checkpoint && getDefaultState(editor).mode !== "insert" && CHECKPOINT_AFTER_OPERATION.has(operation.name)) {
    await editor.runtime.surfaces.commitUndoCheckpoint({ surfaceId });
  }
  span.end({ textLength: state.text.length, revision: state.revision });
  perfTraceLog(editor, "runtime", "applyOperation.end", { surfaceId, operation: operation.name, revision: state.revision, textLength: state.text.length });
}

async function collapseInsertModeSelections(editor: EditorHost): Promise<void> {
  const state = await readActiveSurfaceState(editor, { preferSnapshot: true });
  const selection = state.selection.map((range) => ({
    cursorId: range.cursorId,
    anchor: range.head,
    head: range.head,
  }));
  if (sameSelection(state.selection, selection)) return;
  await applyEdit(editor, { selection });
}

function activeSurfaceId(editor: EditorHost): SurfaceId {
  const state = getDefaultState(editor);
  if (!state.activeSurfaceId) throw new Error("no active surface");
  return state.activeSurfaceId;
}
