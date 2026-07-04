import { Effect } from "effect";
import {
  stringColumns,
  wrapIndentColumns,
  wrapLineBreaks,
  wrapSegmentBounds,
  type EditorSnapshot,
  type WrapDisplay,
  type WrapVirtualText,
} from "@the-editor/editor";
import {
  nextGraphemeBoundary,
  offsetAtColumn,
  previousGraphemeBoundary,
  Selection,
  SelectionRange,
  Transaction,
  type ColumnText,
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

/**
 * The wrap state vertical movement needs to move by VISUAL rows: the
 * renderer-published display (columns, tab width, measured glyph widths)
 * plus the document's inlay hints — both occupy wrap width, and the breaks
 * computed here MUST equal the ones the renderer painted.
 */
export type VerticalWrapContext = WrapDisplay & {
  readonly virtualTextBetween: (from: number, to: number) => readonly WrapVirtualText[];
};

const stringText = (s: string): ColumnText =>
  ({ length: s.length, sliceString: (a: number, b: number) => s.slice(a, b) }) as ColumnText;

type WrappedLineInfo = {
  readonly from: number;
  readonly bounds: readonly number[];
  readonly indent: number;
};

function wrappedLineInfo(
  text: RenderableText,
  line: number,
  wrap: VerticalWrapContext,
): WrappedLineInfo {
  const range = text.lineRange(line);
  const lineText = text.sliceString(range.from, range.to);
  const breaks = wrapLineBreaks({
    lineText,
    wrapColumns: wrap.columns,
    tabWidth: wrap.tabWidth,
    graphemeWidth: wrap.graphemeWidth,
    lineFrom: range.from,
    virtualText: wrap.virtualTextBetween(range.from, range.to),
  });
  return {
    from: range.from,
    bounds: wrapSegmentBounds(range.from, range.to, breaks),
    indent: breaks.length > 0 ? wrapIndentColumns(lineText, wrap.tabWidth, wrap.columns) : 0,
  };
}

/**
 * One VISUAL row up/down: within a wrapped line first, crossing to the
 * adjacent line's nearest visual row otherwise. The goal column is the
 * caret's visual column on its current row (hanging indent included).
 */
function verticalTargetWrapped(
  text: RenderableText,
  offset: number,
  direction: -1 | 1,
  wrap: VerticalWrapContext,
): number {
  const line = text.lineRangeAt(offset);
  const info = wrappedLineInfo(text, line.index, wrap);
  let seg = 0;
  while (seg + 2 < info.bounds.length && offset >= info.bounds[seg + 1]!) seg++;
  const segFrom = info.bounds[seg]!;
  const goal =
    (seg > 0 ? info.indent : 0) +
    stringColumns(text.sliceString(segFrom, offset), wrap.tabWidth, wrap.graphemeWidth);
  let target = info;
  let targetSeg = seg + direction;
  if (targetSeg < 0) {
    if (line.index === 0) return 0;
    target = wrappedLineInfo(text, line.index - 1, wrap);
    targetSeg = target.bounds.length - 2;
  } else if (targetSeg > info.bounds.length - 2) {
    if (line.index >= text.lineCount - 1) return text.length;
    target = wrappedLineInfo(text, line.index + 1, wrap);
    targetSeg = 0;
  }
  const tFrom = target.bounds[targetSeg]!;
  const tTo = target.bounds[targetSeg + 1]!;
  const segText = text.sliceString(tFrom, tTo);
  const innerColumn = Math.max(0, Math.floor(goal - (targetSeg > 0 ? target.indent : 0)));
  const resolved =
    tFrom +
    offsetAtColumn({
      text: stringText(segText),
      line: 0,
      column: innerColumn,
      tabWidth: wrap.tabWidth,
      bias: "before",
      ...(wrap.graphemeWidth !== undefined ? { graphemeWidth: wrap.graphemeWidth } : {}),
    });
  // An offset exactly AT the segment end displays on the NEXT visual row —
  // landing there would skip the row we aimed at. Hold one grapheme short.
  if (resolved >= tTo && targetSeg + 2 < target.bounds.length && segText.length > 0) {
    return tFrom + previousGraphemeBoundary(segText, segText.length);
  }
  return resolved;
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
  wrap?: VerticalWrapContext | null,
): CommandResult {
  const text = snapshot.text;
  return mapRanges(snapshot.selection, (range) => {
    const head =
      wrap != null
        ? verticalTargetWrapped(text, range.head, direction, wrap)
        : verticalTarget(text, range.head, direction);
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
