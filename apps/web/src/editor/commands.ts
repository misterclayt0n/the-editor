import { Effect } from "effect";
import type { EditorSnapshot } from "@the-editor/editor";
import {
  nextGraphemeBoundary,
  previousGraphemeBoundary,
  Selection,
  SelectionRange,
  Transaction,
  type RenderableText,
} from "@the-editor/engine";

/**
 * Product-level editing commands for the first vertical slice.
 *
 * These are app commands, not engine primitives: each one reads an immutable
 * snapshot and returns the transaction/selection to apply. The engine only
 * provides the generic pieces (Transaction, Selection, grapheme boundaries).
 */
export type CommandResult = {
  readonly transaction?: Transaction;
  readonly selection: Selection;
};

export type Edit = { readonly from: number; readonly to: number; readonly insert: string };

/** Carets at the end of each edit, shifted by the edits before them. */
export function editsToSelection(edits: readonly Edit[]): Selection {
  let delta = 0;
  const carets = edits.map((edit) => {
    const caret = edit.from + edit.insert.length + delta;
    delta += edit.insert.length - (edit.to - edit.from);
    return SelectionRange.point(caret);
  });
  return Selection.create(carets);
}

function buildResult(text: RenderableText, edits: readonly Edit[]): CommandResult | undefined {
  const effective = edits.filter((edit) => edit.from < edit.to || edit.insert.length > 0);
  if (effective.length === 0) return undefined;
  const transaction = Effect.runSync(
    Transaction.fromReplacements(
      text.length,
      effective.map((edit) => ({ from: edit.from, to: edit.to, text: edit.insert })),
    ).pipe(Effect.orDie),
  );
  return { transaction, selection: editsToSelection(effective) };
}

/** The raw edits for each operation, in document coordinates. Exposed so the
 * multibuffer can route them through editPlan instead of a single transaction. */
export function insertEdits(snapshot: EditorSnapshot, text: string): Edit[] {
  return snapshot.selection.ranges.map((range) => ({ from: range.from, to: range.to, insert: text }));
}

export function deleteBackwardEdits(snapshot: EditorSnapshot): Edit[] {
  const text = snapshot.text;
  return snapshot.selection.ranges.map((range): Edit => {
    if (!range.isCollapsed) return { from: range.from, to: range.to, insert: "" };
    const offset = range.head;
    if (offset === 0) return { from: 0, to: 0, insert: "" };
    const line = text.lineRangeAt(offset);
    if (offset === line.from) {
      const breakLength = offset >= 2 && text.sliceString(offset - 2, offset) === "\r\n" ? 2 : 1;
      return { from: offset - breakLength, to: offset, insert: "" };
    }
    const lineText = text.sliceString(line.from, line.to);
    const previous = line.from + previousGraphemeBoundary(lineText, offset - line.from);
    return { from: previous, to: offset, insert: "" };
  });
}

export function deleteForwardEdits(snapshot: EditorSnapshot): Edit[] {
  const text = snapshot.text;
  return snapshot.selection.ranges.map((range): Edit => {
    if (!range.isCollapsed) return { from: range.from, to: range.to, insert: "" };
    const offset = range.head;
    if (offset === text.length) return { from: offset, to: offset, insert: "" };
    const line = text.lineRangeAt(offset);
    if (offset === line.to) {
      const breakLength =
        offset + 2 <= text.length && text.sliceString(offset, offset + 2) === "\r\n" ? 2 : 1;
      return { from: offset, to: offset + breakLength, insert: "" };
    }
    const lineText = text.sliceString(line.from, line.to);
    const next = line.from + nextGraphemeBoundary(lineText, offset - line.from);
    return { from: offset, to: next, insert: "" };
  });
}

export function insertText(snapshot: EditorSnapshot, text: string): CommandResult | undefined {
  return buildResult(snapshot.text, insertEdits(snapshot, text));
}

export function insertNewline(snapshot: EditorSnapshot): CommandResult | undefined {
  return insertText(snapshot, "\n");
}

export function deleteBackward(snapshot: EditorSnapshot): CommandResult | undefined {
  return buildResult(snapshot.text, deleteBackwardEdits(snapshot));
}

export function deleteForward(snapshot: EditorSnapshot): CommandResult | undefined {
  return buildResult(snapshot.text, deleteForwardEdits(snapshot));
}

function moveHead(text: RenderableText, offset: number, direction: -1 | 1): number {
  const line = text.lineRangeAt(offset);
  if (direction === -1) {
    if (offset === 0) return 0;
    if (offset === line.from) return text.lineRange(line.index - 1).to;
    const lineText = text.sliceString(line.from, line.to);
    return line.from + previousGraphemeBoundary(lineText, offset - line.from);
  }
  if (offset === text.length) return offset;
  if (offset === line.to) return text.lineRange(line.index + 1).from;
  const lineText = text.sliceString(line.from, line.to);
  return line.from + nextGraphemeBoundary(lineText, offset - line.from);
}

function verticalTarget(text: RenderableText, offset: number, direction: -1 | 1): number {
  const line = text.lineRangeAt(offset);
  const targetIndex = line.index + direction;
  if (targetIndex < 0) return 0;
  if (targetIndex >= text.lineCount) return text.length;
  const target = text.lineRange(targetIndex);
  const column = offset - line.from;
  return target.from + Math.min(column, target.to - target.from);
}

function mapRanges(
  selection: Selection,
  map: (range: SelectionRange) => SelectionRange,
): CommandResult {
  return { selection: selection.transform(map) };
}

export function moveHorizontal(
  snapshot: EditorSnapshot,
  direction: -1 | 1,
  extend: boolean,
): CommandResult {
  const text = snapshot.text;
  return mapRanges(snapshot.selection, (range) => {
    if (!extend && !range.isCollapsed) {
      return SelectionRange.point(direction === -1 ? range.from : range.to);
    }
    const head = moveHead(text, range.head, direction);
    return extend ? SelectionRange.create(range.anchor, head) : SelectionRange.point(head);
  });
}

export function moveVertical(
  snapshot: EditorSnapshot,
  direction: -1 | 1,
  extend: boolean,
): CommandResult {
  const text = snapshot.text;
  return mapRanges(snapshot.selection, (range) => {
    const head = verticalTarget(text, range.head, direction);
    return extend ? SelectionRange.create(range.anchor, head) : SelectionRange.point(head);
  });
}

export function moveLineEdge(
  snapshot: EditorSnapshot,
  edge: "start" | "end",
  extend: boolean,
): CommandResult {
  const text = snapshot.text;
  return mapRanges(snapshot.selection, (range) => {
    const line = text.lineRangeAt(range.head);
    const head = edge === "start" ? line.from : line.to;
    return extend ? SelectionRange.create(range.anchor, head) : SelectionRange.point(head);
  });
}
