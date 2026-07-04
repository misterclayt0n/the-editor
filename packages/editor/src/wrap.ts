/**
 * Soft-wrap break computation, pure and deterministic: one function decides
 * where a line breaks, and every consumer — the renderer's row counts, the
 * layout's row emission, hit testing, vertical caret movement — calls the
 * same function with the same inputs, so they cannot disagree about
 * geometry.
 *
 * Semantics:
 * - Breaks prefer word boundaries (a run that would overflow moves whole to
 *   the next visual row; the trailing space stays on the previous row, VS
 *   Code style) and fall back to hard breaks mid-word.
 * - Tab stops restart at each visual row's start — exactly how the DOM
 *   renders each row as its own element.
 * - Continuation rows are indented to the line's leading whitespace
 *   (clamped to half the wrap width), so wrapped code hangs under its
 *   indentation; their capacity shrinks accordingly.
 * - Virtual text (inlay hints) COUNTS toward the width: a hint occupies
 *   real columns, so the row it sits on breaks earlier. A hint anchored
 *   exactly at a break belongs to the continuation row — the same rule the
 *   rendering uses.
 */

import {
  displayGrapheme,
  graphemeSegments,
  type GraphemeWidthFn,
} from "@the-editor/engine";

const SIMPLE_ASCII = /^[\x20-\x7e]*$/;

/** The subset of a virtual-text entry wrapping cares about. */
export type WrapVirtualText = {
  readonly at: number;
  readonly text: string;
};

/**
 * The renderer-published wrap display state of a view: what a wrap-aware
 * consumer outside the renderer (vertical caret movement) needs to compute
 * the SAME breaks the renderer painted. The renderer is the owner; the
 * view-scoped ViewWrapDisplay service mirrors it.
 */
export type WrapDisplay = {
  readonly columns: number;
  readonly tabWidth: number;
  readonly graphemeWidth?: GraphemeWidthFn | undefined;
};

/**
 * Visual columns a VIRTUAL text run occupies. Defaults to its length (inlay
 * hints are overwhelmingly ASCII); a width oracle makes non-ASCII labels
 * exact too.
 */
export function virtualTextColumns(text: string, graphemeWidth?: GraphemeWidthFn): number {
  if (graphemeWidth === undefined) return text.length;
  let columns = 0;
  for (const segment of graphemeSegments(text)) columns += graphemeWidth(segment.text);
  return columns;
}

/**
 * The hanging indent (in columns) continuation rows of this line get: its
 * leading whitespace, clamped to half the wrap width so pathological
 * indentation can never starve a row of capacity.
 */
export function wrapIndentColumns(
  lineText: string,
  tabWidth: number,
  wrapColumns: number,
): number {
  let columns = 0;
  for (const ch of lineText) {
    if (ch === " ") columns += 1;
    else if (ch === "\t") columns += tabWidth - (columns % tabWidth);
    else break;
  }
  return columns <= Math.floor(wrapColumns / 2) ? columns : 0;
}

export type WrapLineArgs = {
  readonly lineText: string;
  readonly wrapColumns: number;
  readonly tabWidth: number;
  readonly graphemeWidth?: GraphemeWidthFn | undefined;
  /** The line's absolute start offset — needed to resolve `virtualText`. */
  readonly lineFrom?: number | undefined;
  /** Virtual text anchored on this line (absolute offsets); occupies width. */
  readonly virtualText?: readonly WrapVirtualText[] | undefined;
};

/**
 * Break offsets for one line, RELATIVE to the line start: each is the first
 * code unit of a continuation row. Empty when the line fits. `wrapColumns`
 * is clamped to at least 1; every visual row keeps at least one grapheme
 * (a grapheme wider than the row still occupies one).
 */
export function wrapLineBreaks(args: WrapLineArgs): number[] {
  const { lineText, tabWidth, graphemeWidth } = args;
  const wrapColumns = Math.max(1, Math.floor(args.wrapColumns));
  const hasVirtuals = args.virtualText !== undefined && args.virtualText.length > 0;
  // Fast path: plain ASCII shorter than the width, with no hints, can
  // never wrap.
  if (!hasVirtuals && lineText.length <= wrapColumns && SIMPLE_ASCII.test(lineText)) return [];
  // Virtual widths bucketed by line-relative anchor (several hints can
  // share one anchor).
  let virtualAt: Map<number, number> | null = null;
  if (hasVirtuals) {
    const lineFrom = args.lineFrom ?? 0;
    virtualAt = new Map();
    for (const vt of args.virtualText!) {
      const at = vt.at - lineFrom;
      if (at < 0 || at > lineText.length) continue;
      virtualAt.set(at, (virtualAt.get(at) ?? 0) + virtualTextColumns(vt.text, graphemeWidth));
    }
    if (virtualAt.size === 0) virtualAt = null;
  }
  if (
    virtualAt === null &&
    lineText.length <= wrapColumns &&
    SIMPLE_ASCII.test(lineText)
  ) {
    return [];
  }
  const indent = wrapIndentColumns(lineText, tabWidth, wrapColumns);
  const segments = [...graphemeSegments(lineText)];
  const breaks: number[] = [];
  let index = 0;
  let rowStart = 0; // grapheme index of the current row's first grapheme
  // `column` is SEGMENT-internal (tab stops restart per row; the hanging
  // indent is rendered as row padding, so it shrinks capacity instead).
  let column = 0;
  let capacity = wrapColumns;
  let breakCandidate = -1; // grapheme index just AFTER the last whitespace
  while (index < segments.length) {
    const segment = segments[index]!;
    const width =
      (virtualAt?.get(segment.from) ?? 0) +
      (graphemeWidth !== undefined && segment.text !== "\t"
        ? graphemeWidth(segment.text)
        : displayGrapheme(segment.text, column, tabWidth).width);
    if (column + width > capacity && index > rowStart) {
      const breakAt = breakCandidate > rowStart ? breakCandidate : index;
      breaks.push(segments[breakAt]!.from);
      rowStart = breakAt;
      index = breakAt; // re-measure from the new row start (tab stops reset)
      column = 0;
      capacity = Math.max(1, wrapColumns - indent);
      breakCandidate = -1;
      continue;
    }
    column += width;
    if (segment.text === " " || segment.text === "\t") breakCandidate = index + 1;
    index++;
  }
  return breaks;
}

/** The visual row (within its line) that `offsetInLine` sits on. An offset
 * exactly AT a break belongs to the continuation row — the caret at a wrap
 * point renders at the start of the next visual row. */
export function wrapRowOfOffset(breaks: readonly number[], offsetInLine: number): number {
  let row = 0;
  for (const breakAt of breaks) {
    if (offsetInLine < breakAt) break;
    row++;
  }
  return row;
}

/** Segment bounds for a line: `[from, ...breaks, to]` in ABSOLUTE offsets. */
export function wrapSegmentBounds(
  lineFrom: number,
  lineTo: number,
  breaks: readonly number[],
): number[] {
  const bounds = [lineFrom];
  for (const breakAt of breaks) bounds.push(lineFrom + breakAt);
  bounds.push(lineTo);
  return bounds;
}
