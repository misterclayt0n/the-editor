import type { CursorRangeSpec, SelectionSpec, SurfaceId, SurfaceSnapshot } from "@the-editor/contracts/runtime";
import { commandId, type EditorHost, type EditorModule } from "@the-editor/editor";
import { getDefaultState, setEditorMode } from "./state.js";

export function defaultEditing(): EditorModule {
  return (editor) => {
    editor.commands.register({ id: commandId("mode.normal"), title: "Normal Mode", run: () => setEditorMode(editor, "normal") });
    editor.commands.register({ id: commandId("mode.insert"), title: "Insert Mode", run: () => setEditorMode(editor, "insert") });

    registerSelectionCommand(editor, "move.charLeft", (snapshot) => movePrimary(editor, snapshot, -1));
    registerSelectionCommand(editor, "move.charRight", (snapshot) => movePrimary(editor, snapshot, 1));
    registerSelectionCommand(editor, "move.visualLineUp", (snapshot) => moveLine(editor, snapshot, -1));
    registerSelectionCommand(editor, "move.visualLineDown", (snapshot) => moveLine(editor, snapshot, 1));
    registerSelectionCommand(editor, "goto.lineStart", lineStart);
    registerSelectionCommand(editor, "goto.lineEnd", lineEnd);
    registerSelectionCommand(editor, "goto.documentStart", documentStart);
    registerSelectionCommand(editor, "goto.documentEnd", documentEnd);

    editor.commands.register<{ text: string }, void>({
      id: commandId("edit.insertText"),
      title: "Insert Text",
      async run(_ctx, args) {
        await applyTextReplacement(editor, args.text);
      },
    });

    editor.commands.register({
      id: commandId("edit.insertNewline"),
      title: "Insert Newline",
      async run() {
        await insertNewline(editor);
      },
    });

    editor.commands.register({
      id: commandId("edit.deleteBackward"),
      title: "Delete Backward",
      async run() {
        await deleteRelative(editor, -1);
      },
    });

    editor.commands.register({
      id: commandId("edit.deleteForward"),
      title: "Delete Forward",
      async run() {
        await deleteRelative(editor, 1);
      },
    });
  };
}

function registerSelectionCommand(
  editor: EditorHost,
  id: string,
  select: (snapshot: SurfaceSnapshot) => SelectionSpec | Promise<SelectionSpec>,
): void {
  editor.commands.register({
    id: commandId(id),
    title: id,
    async run() {
      const { surfaceId, snapshot } = await activeSnapshot(editor);
      if (!id.startsWith("move.visualLine")) getDefaultState(editor).preferredColumn = undefined;
      await editor.runtime.surfaces.setSelection({ surfaceId, selection: await select(snapshot) });
    },
  });
}

async function applyTextReplacement(editor: EditorHost, text: string): Promise<void> {
  const state = getDefaultState(editor);
  state.preferredColumn = undefined;
  const { surfaceId, snapshot } = await activeSnapshot(editor);
  const range = primaryRange(snapshot);
  const start = Math.min(range.anchor, range.head);
  const end = Math.max(range.anchor, range.head);
  const next = start + [...text].length;
  await editor.runtime.surfaces.applyTransaction({
    surfaceId,
    transaction: { replacements: [{ start, end, text }] },
    selection: selectionWithPrimary(snapshot, { ...range, anchor: next, head: next }),
  });
}

async function insertNewline(editor: EditorHost): Promise<void> {
  const state = getDefaultState(editor);
  state.preferredColumn = undefined;
  const { surfaceId, snapshot } = await activeSnapshot(editor);
  const range = primaryRange(snapshot);
  const pos = range.head;
  const chars = [...snapshot.text];
  const lineStart = snapshot.text.lastIndexOf("\n", Math.max(0, pos - 1)) + 1;
  const beforeCursor = chars.slice(lineStart, pos).join("");
  const lastNonWhitespace = lastNonWhitespaceIndex(beforeCursor);
  const firstTrailingWhitespace = lastNonWhitespace === undefined ? lineStart : lineStart + lastNonWhitespace + 1;
  const indent = lastNonWhitespace === undefined
    ? ""
    : (await editor.runtime.surfaces.indentForNewline({ surfaceId, pos })).indent;
  const prev = pos === 0 ? " " : chars[pos - 1] ?? " ";
  const curr = chars[pos] ?? " ";
  const base = `\n${indent}`;
  const insert = isAutoPair(prev, curr) ? `${base}  \n${indent}` : base;
  const next = firstTrailingWhitespace + [...base].length;
  await editor.runtime.surfaces.applyTransaction({
    surfaceId,
    transaction: { replacements: [{ start: firstTrailingWhitespace, end: pos, text: insert }] },
    selection: selectionWithPrimary(snapshot, { ...range, anchor: next, head: next }),
  });
}

async function deleteRelative(editor: EditorHost, direction: -1 | 1): Promise<void> {
  const state = getDefaultState(editor);
  state.preferredColumn = undefined;
  const { surfaceId, snapshot } = await activeSnapshot(editor);
  const range = primaryRange(snapshot);
  let start = Math.min(range.anchor, range.head);
  let end = Math.max(range.anchor, range.head);
  if (start === end) {
    if (direction < 0) start = Math.max(0, start - 1);
    else end = Math.min([...snapshot.text].length, end + 1);
  }
  if (start === end) return;
  await editor.runtime.surfaces.applyTransaction({
    surfaceId,
    transaction: { replacements: [{ start, end }] },
    selection: selectionWithPrimary(snapshot, { ...range, anchor: start, head: start }),
  });
}

async function activeSnapshot(editor: EditorHost): Promise<{ surfaceId: SurfaceId; snapshot: SurfaceSnapshot }> {
  const state = getDefaultState(editor);
  if (!state.activeSurfaceId) throw new Error("no active surface");
  return {
    surfaceId: state.activeSurfaceId,
    snapshot: await editor.runtime.surfaces.snapshot({ surfaceId: state.activeSurfaceId }),
  };
}

function primaryRange(snapshot: SurfaceSnapshot): CursorRangeSpec {
  return snapshot.selection.ranges.find((range) => range.id === snapshot.activeCursorId) ?? snapshot.selection.ranges[0]!;
}

function selectionWithPrimary(snapshot: SurfaceSnapshot, primary: CursorRangeSpec): SelectionSpec {
  const current = primaryRange(snapshot);
  return {
    ranges: snapshot.selection.ranges.map((range) => (range === current ? primary : range)),
  };
}

function movePrimary(editor: EditorHost, snapshot: SurfaceSnapshot, delta: number): SelectionSpec {
  getDefaultState(editor).preferredColumn = undefined;
  const range = primaryRange(snapshot);
  const pos = clamp(range.head + delta, 0, [...snapshot.text].length);
  return selectionWithPrimary(snapshot, { ...range, anchor: pos, head: pos });
}

function lineStart(snapshot: SurfaceSnapshot): SelectionSpec {
  const range = primaryRange(snapshot);
  const pos = snapshot.text.lastIndexOf("\n", Math.max(0, range.head - 1)) + 1;
  return selectionWithPrimary(snapshot, { ...range, anchor: pos, head: pos });
}

function lineEnd(snapshot: SurfaceSnapshot): SelectionSpec {
  const range = primaryRange(snapshot);
  const next = snapshot.text.indexOf("\n", range.head);
  const pos = next < 0 ? [...snapshot.text].length : next;
  return selectionWithPrimary(snapshot, { ...range, anchor: pos, head: pos });
}

function documentStart(snapshot: SurfaceSnapshot): SelectionSpec {
  const range = primaryRange(snapshot);
  return selectionWithPrimary(snapshot, { ...range, anchor: 0, head: 0 });
}

function documentEnd(snapshot: SurfaceSnapshot): SelectionSpec {
  const range = primaryRange(snapshot);
  const pos = [...snapshot.text].length;
  return selectionWithPrimary(snapshot, { ...range, anchor: pos, head: pos });
}

function moveLine(editor: EditorHost, snapshot: SurfaceSnapshot, delta: -1 | 1): SelectionSpec {
  const state = getDefaultState(editor);
  const range = primaryRange(snapshot);
  const lines = lineStarts(snapshot.text);
  const currentLine = lineIndexForPos(lines, range.head);
  const currentCol = range.head - lines[currentLine]!;
  const preferredColumn = state.preferredColumn ?? currentCol;
  state.preferredColumn = preferredColumn;
  const nextLine = clamp(currentLine + delta, 0, lines.length - 1);
  const nextLineStart = lines[nextLine]!;
  const nextLineEnd = lineEndOffset(snapshot.text, nextLineStart);
  const pos = nextLineStart + Math.min(preferredColumn, nextLineEnd - nextLineStart);
  return selectionWithPrimary(snapshot, { ...range, anchor: pos, head: pos });
}

function lineStarts(text: string): number[] {
  const starts = [0];
  for (let i = 0; i < text.length; i++) {
    if (text[i] === "\n") starts.push(i + 1);
  }
  return starts;
}

function lineIndexForPos(starts: number[], pos: number): number {
  let low = 0;
  let high = starts.length - 1;
  while (low <= high) {
    const mid = Math.floor((low + high) / 2);
    if (starts[mid]! <= pos) low = mid + 1;
    else high = mid - 1;
  }
  return Math.max(0, high);
}

function lineEndOffset(text: string, lineStart: number): number {
  const end = text.indexOf("\n", lineStart);
  return end < 0 ? [...text].length : end;
}

function lastNonWhitespaceIndex(text: string): number | undefined {
  for (let i = text.length - 1; i >= 0; i--) {
    if (!/\s/.test(text[i]!)) return i;
  }
  return undefined;
}

function isAutoPair(prev: string, curr: string): boolean {
  return (prev === "(" && curr === ")") || (prev === "[" && curr === "]") || (prev === "{" && curr === "}");
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}
