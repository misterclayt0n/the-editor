import type {
  SurfaceChange,
  SurfaceEdit,
  SurfaceId,
  SurfaceOperation,
  SurfaceSelectionRange,
  SurfaceState,
} from "@the-editor/contracts/runtime";
import { commandId, type CommandDispatch, type EditorHost, type EditorModule, type KeyInput } from "@the-editor/editor";
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
  };

type LastMotionState = CommandDispatch | FindCharMotionState;

const pendingInputSlot = "default.editing.pendingInput";
const lastMotionSlot = "default.editing.lastMotion";

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
]);

export function defaultEditing(): EditorModule {
  return (editor) => {
    editor.commands.register({ id: commandId("normal_mode"), title: "Normal Mode", run: () => applyMode(editor, "normal") });
    editor.commands.register({ id: commandId("mode.normal"), title: "Normal Mode", run: () => applyMode(editor, "normal") });
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
        return true;
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
  editor.setStateSlot(pendingInputSlot, pending);
}

function takePendingInput(editor: EditorHost): PendingInputState | undefined {
  const pending = editor.getStateSlot<PendingInputState | undefined>(pendingInputSlot);
  editor.deleteStateSlot(pendingInputSlot);
  return pending;
}

function isFindCharMotionState(motion: LastMotionState): motion is FindCharMotionState {
  return (motion as FindCharMotionState).kind === "findCharMotion";
}

function setLastMotion(editor: EditorHost, motion: LastMotionState | undefined): void {
  editor.setStateSlot(lastMotionSlot, motion);
}

function getLastMotion(editor: EditorHost): LastMotionState | undefined {
  return editor.getStateSlot<LastMotionState | undefined>(lastMotionSlot);
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

async function readActiveSurfaceState(editor: EditorHost): Promise<SurfaceState> {
  return editor.runtime.surfaces.readState({ surfaceId: activeSurfaceId(editor) });
}

async function applyEdit(editor: EditorHost, edit: SurfaceEdit, checkpoint = false): Promise<void> {
  await editor.runtime.surfaces.applyEdit({
    surfaceId: activeSurfaceId(editor),
    edit,
  });
  if (checkpoint && getDefaultState(editor).mode !== "insert") {
    await editor.runtime.surfaces.commitUndoCheckpoint({ surfaceId: activeSurfaceId(editor) });
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

async function applyOperation(
  editor: EditorHost,
  operation: SurfaceOperation,
  checkpoint = true,
): Promise<void> {
  await editor.runtime.surfaces.applyOperation({
    surfaceId: activeSurfaceId(editor),
    operation,
  });
  if (checkpoint && getDefaultState(editor).mode !== "insert" && CHECKPOINT_AFTER_OPERATION.has(operation.name)) {
    await editor.runtime.surfaces.commitUndoCheckpoint({ surfaceId: activeSurfaceId(editor) });
  }
}

function activeSurfaceId(editor: EditorHost): SurfaceId {
  const state = getDefaultState(editor);
  if (!state.activeSurfaceId) throw new Error("no active surface");
  return state.activeSurfaceId;
}
