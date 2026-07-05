import {
  columnAtOffset as engineColumnAtOffset,
  displayGrapheme,
  graphemeSegments,
  offsetAtColumn,
  type ColumnText,
  type GraphemeWidthFn,
  type RenderableText,
} from "@the-editor/engine";
import {
  virtualTextColumns,
  wrapIndentColumns,
  wrapLineBreaks,
  wrapSegmentBounds,
} from "./wrap";

export type EditorVisualOwner = string;
export type TextStyleRun = {
  readonly from: number;
  readonly to: number;
  readonly className?: string;
  readonly style?: Readonly<Record<string, string>>;
  readonly priority?: number;
  readonly owner?: EditorVisualOwner;
};
export type DecorationLayer = "background" | "underline" | "outline";
export type LineDecorationTarget = "content" | "gutter" | "all";
export type LineDecorationScope = "visual-row" | "logical-line";
export type LineDecorationPaint = "background" | "border" | "horizontal-border";
export type VerticalGuideSpec = {
  readonly column: number;
  readonly widthPx?: number;
  readonly role?: string;
  readonly className?: string;
  readonly style?: Readonly<Record<string, string>>;
  readonly priority?: number;
};
export type LineDecorationSpec = {
  readonly target: LineDecorationTarget;
  readonly scope: LineDecorationScope;
  readonly paint: LineDecorationPaint;
  readonly role?: string;
  readonly className?: string;
  readonly style?: Readonly<Record<string, string>>;
  readonly priority?: number;
};
export type VerticalGuide = VerticalGuideSpec & {
  readonly from: number;
  readonly to: number;
  readonly owner?: EditorVisualOwner;
};
export type RangeDecoration = {
  readonly from: number;
  readonly to: number;
  readonly layer: DecorationLayer;
  readonly role?: string;
  readonly className?: string;
  readonly style?: Readonly<Record<string, string>>;
  readonly priority?: number;
  readonly owner?: EditorVisualOwner;
};
export type VirtualTextAffinity = "before" | "after";
export type LineDecoration = LineDecorationSpec & {
  readonly at: number;
  readonly owner?: EditorVisualOwner;
};
export type VirtualText = {
  readonly at: number;
  readonly text: string;
  readonly affinity?: VirtualTextAffinity;
  readonly className?: string;
  readonly style?: Readonly<Record<string, string>>;
  readonly priority?: number;
  readonly owner?: EditorVisualOwner;
};
export type BlockWidget = {
  readonly id: string;
  readonly afterLine: number;
  readonly heightRows: number;
  readonly owner?: EditorVisualOwner;
};
export type EditorVisualsSnapshot = {
  readonly textStyles?: readonly TextStyleRun[];
  readonly rangeDecorations?: readonly RangeDecoration[];
  readonly lineDecorations?: readonly LineDecoration[];
  readonly verticalGuides?: readonly VerticalGuide[];
  readonly virtualText?: readonly VirtualText[];
  readonly blockWidgets?: readonly BlockWidget[];
};

export type LayoutAnchor<Payload = unknown> =
  | {
      readonly kind: "range";
      readonly id: string;
      readonly from: number;
      readonly to: number;
      readonly payload: Payload;
    }
  | { readonly kind: "line"; readonly id: string; readonly line: number; readonly payload: Payload }
  | {
      readonly kind: "block";
      readonly id: string;
      readonly afterLine: number;
      readonly heightRows: number;
      readonly payload: Payload;
    }
  | {
      readonly kind: "overlay";
      readonly id: string;
      readonly at: number;
      /** Soft wrap: an offset AT a break normally displays at the start of
       * the continuation row; "before" pins it to the END of the previous
       * row instead (click-past-row-end caret affinity). */
      readonly assoc?: "before" | undefined;
      readonly payload: Payload;
    };

export type ViewportLayoutRequest<Payload = unknown> = {
  readonly text: RenderableText;
  readonly version: number;
  readonly startLine: number;
  readonly lineCount: number;
  readonly tabWidth?: number;
  readonly lineHeight?: number;
  readonly charWidth?: number;
  readonly paddingTop?: number;
  readonly paddingLeft?: number;
  readonly anchors?: readonly LayoutAnchor<Payload>[];
  readonly visuals?: EditorVisualsSnapshot;
  /** Renderer-measured grapheme grid widths (see GraphemeWidthFn). */
  readonly graphemeWidth?: GraphemeWidthFn | undefined;
  /** Soft wrap width in visual columns; absent = no wrapping. Row counts
   * MUST agree with the GeometryModel the renderer built — both derive
   * from the same wrapLineBreaks. */
  readonly wrapColumns?: number | undefined;
  /** The absolute TEXT-row index of `startLine` (wrapped rows above the
   * window; block-widget rows are accounted separately). Defaults to
   * `startLine` — correct only when nothing above the window wraps. */
  readonly startRow?: number | undefined;
};
export type LayoutRect = {
  readonly top: number;
  readonly left: number;
  readonly width: number;
  readonly height: number;
};
export type LayoutInlineSegment =
  | {
      readonly kind: "text";
      readonly from: number;
      readonly to: number;
      readonly text: string;
      readonly className?: string;
      readonly style?: Readonly<Record<string, string>>;
      readonly owners?: readonly EditorVisualOwner[];
    }
  | {
      readonly kind: "virtual-text";
      readonly at: number;
      readonly text: string;
      readonly affinity: VirtualTextAffinity;
      readonly className?: string;
      readonly style?: Readonly<Record<string, string>>;
      readonly owner?: EditorVisualOwner;
    };
export type LayoutRow =
  | {
      readonly kind: "text";
      readonly row: number;
      readonly line: number;
      readonly from: number;
      readonly to: number;
      readonly text: string;
      readonly segments: readonly LayoutInlineSegment[];
      readonly top: number;
      readonly left: number;
      readonly height: number;
      /** Soft wrap: a continuation row of its line (no line number). */
      readonly continuation?: boolean;
      /** Soft wrap: hanging-indent columns of a continuation row. */
      readonly indentCols?: number;
    }
  | {
      readonly kind: "block";
      readonly row: number;
      readonly afterLine: number;
      readonly heightRows: number;
      readonly top: number;
      readonly left: number;
      readonly height: number;
    };
export type LayoutPlacement<Payload = unknown> =
  | {
      readonly kind: "range";
      readonly id: string;
      readonly row: number;
      readonly line: number;
      readonly from: number;
      readonly to: number;
      readonly columnStart: number;
      readonly columnEnd: number;
      readonly rects: readonly LayoutRect[];
      readonly payload: Payload;
    }
  | {
      readonly kind: "line";
      readonly id: string;
      readonly row: number;
      readonly line: number;
      readonly payload: Payload;
    }
  | {
      readonly kind: "block";
      readonly id: string;
      readonly row: number;
      readonly afterLine: number;
      readonly heightRows: number;
      readonly rect: LayoutRect;
      readonly payload: Payload;
    }
  | {
      readonly kind: "overlay";
      readonly id: string;
      readonly row: number;
      readonly line: number;
      readonly at: number;
      readonly column: number;
      readonly rect: LayoutRect;
      readonly payload: Payload;
    };
export type LayoutDecoration = {
  readonly layer: DecorationLayer;
  readonly from: number;
  readonly to: number;
  readonly rects: readonly LayoutRect[];
  readonly role?: string;
  readonly className?: string;
  readonly style?: Readonly<Record<string, string>>;
  readonly priority?: number;
  readonly owner?: EditorVisualOwner;
};
export type LayoutVerticalGuide = {
  readonly from: number;
  readonly to: number;
  readonly row: number;
  readonly line: number;
  readonly top: number;
  readonly left: number;
  readonly height: number;
  readonly column: number;
  readonly widthPx?: number;
  readonly role?: string;
  readonly className?: string;
  readonly style?: Readonly<Record<string, string>>;
  readonly priority?: number;
  readonly owner?: EditorVisualOwner;
};
export type LayoutLineDecoration = {
  readonly row: number;
  readonly line: number;
  readonly top: number;
  readonly height: number;
  readonly target: LineDecorationTarget;
  readonly paint: LineDecorationPaint;
  readonly role?: string;
  readonly className?: string;
  readonly style?: Readonly<Record<string, string>>;
  readonly priority?: number;
  readonly owner?: EditorVisualOwner;
};
export type LayoutBlockWidget = {
  readonly id: string;
  readonly afterLine: number;
  readonly row: number;
  readonly rect: LayoutRect;
  readonly owner?: EditorVisualOwner;
};
export type ViewportLayoutSnapshot<Payload = unknown> = {
  readonly version: number;
  readonly startLine: number;
  readonly lineCount: number;
  readonly rows: readonly LayoutRow[];
  readonly placements: readonly LayoutPlacement<Payload>[];
  readonly decorations: readonly LayoutDecoration[];
  readonly lineDecorations: readonly LayoutLineDecoration[];
  readonly verticalGuides: readonly LayoutVerticalGuide[];
  readonly blockWidgets: readonly LayoutBlockWidget[];
};
export type HitTestBias = "before" | "after" | "nearest";
/** Soft-wrap context for a hit test: the wrap width and which visual row
 * (within the resolved line) the y coordinate landed on. */
export type HitTestWrap = { readonly columns: number; readonly row: number };
export type HitTestResult = {
  readonly line: number;
  readonly column: number;
  readonly offset: number;
};

const clamp = (n: number, min: number, max: number) => Math.max(min, Math.min(max, n));
/** A detached string as ColumnText (one wrap segment for column↔offset). */
const stringText = (s: string): ColumnText =>
  ({ length: s.length, sliceString: (a: number, b: number) => s.slice(a, b) }) as ColumnText;

/** The shared column→offset resolution (floor/ceil biases, "nearest" by
 * distance) — runs against a whole line or a single wrap segment. */
function resolveColumn(
  text: ColumnText,
  line: number,
  rawColumn: number,
  tabWidth: number,
  bias: HitTestBias,
  graphemeWidth: GraphemeWidthFn | undefined,
): { offset: number; column: number } {
  const gwArg = graphemeWidth !== undefined ? { graphemeWidth } : {};
  if (bias === "nearest") {
    const beforeOffset = offsetAtColumn({
      text,
      line,
      column: Math.floor(rawColumn),
      tabWidth,
      bias: "before",
      ...gwArg,
    });
    const afterOffset = offsetAtColumn({
      text,
      line,
      column: Math.ceil(rawColumn),
      tabWidth,
      bias: "after",
      ...gwArg,
    });
    const beforeColumn = engineColumnAtOffset({ text, line, offset: beforeOffset, tabWidth, ...gwArg }),
      afterColumn = engineColumnAtOffset({ text, line, offset: afterOffset, tabWidth, ...gwArg });
    const offset =
      Math.abs(rawColumn - beforeColumn) < Math.abs(afterColumn - rawColumn)
        ? beforeOffset
        : afterOffset;
    return { offset, column: offset === beforeOffset ? beforeColumn : afterColumn };
  }
  const targetColumn = bias === "before" ? Math.floor(rawColumn) : Math.ceil(rawColumn);
  const offset = offsetAtColumn({ text, line, column: targetColumn, tabWidth, bias, ...gwArg });
  return { offset, column: engineColumnAtOffset({ text, line, offset, tabWidth, ...gwArg }) };
}
const priority = (x: { readonly priority?: number }) => x.priority ?? 0;
function lineRange(text: RenderableText, line: number): { from: number; to: number } {
  const r = text.lineRange(line);
  return { from: r.from, to: r.to };
}
function lineIndexAtOffset(text: RenderableText, offset: number): number {
  return text.lineRangeAt(clamp(offset, 0, text.length)).index;
}
function columnAtOffset(
  text: RenderableText,
  line: number,
  offset: number,
  tabWidth: number,
  graphemeWidth?: GraphemeWidthFn,
): number {
  return engineColumnAtOffset({
    text,
    line,
    offset,
    tabWidth,
    ...(graphemeWidth !== undefined ? { graphemeWidth } : {}),
  });
}

/**
 * Visual columns of a detached string measured from column 0 (tabs advance
 * to grid stops). The renderer's float anchoring and caret math share this
 * with the layout so a wide glyph (an Iosevka em dash is TWO cells) shifts
 * every consumer identically.
 */
export function stringColumns(
  text: string,
  tabWidth: number,
  graphemeWidth?: GraphemeWidthFn,
): number {
  let column = 0;
  for (const segment of graphemeSegments(text)) {
    if (graphemeWidth !== undefined && segment.text !== "\t") {
      column += graphemeWidth(segment.text);
    } else {
      // Same defaults as the engine's columnAtOffset — one column model.
      column += displayGrapheme(segment.text, column, tabWidth).width;
    }
  }
  return column;
}



/**
 * Extra VISUAL columns contributed by virtual text (inlay hints etc.)
 * rendered before `offset` on its line. Virtual text occupies columns but
 * no document offsets, so every geometry computation — caret placement,
 * selection rects, hit testing — must add these in. A virtual anchored
 * exactly AT the offset with "before" affinity renders left of the offset's
 * character, so the caret there sits to its right.
 */
export function virtualColsBefore(
  virtuals: readonly VirtualText[] | undefined,
  text: RenderableText,
  line: number,
  offset: number,
  graphemeWidth?: GraphemeWidthFn,
): number {
  if (virtuals === undefined || virtuals.length === 0) return 0;
  const range = text.lineRange(line);
  let cols = 0;
  for (const vt of virtuals) {
    const at = clamp(vt.at, 0, text.length);
    if (at < range.from || at > range.to) continue;
    if (at < offset || (at === offset && (vt.affinity ?? "after") === "before")) {
      cols += virtualTextColumns(vt.text, graphemeWidth);
    }
  }
  return cols;
}

/** Virtuals anchored on `line`, in visual order. */
function lineVirtuals(
  virtuals: readonly VirtualText[] | undefined,
  text: RenderableText,
  line: number,
): VirtualText[] {
  if (virtuals === undefined || virtuals.length === 0) return [];
  const range = text.lineRange(line);
  return virtuals
    .filter((vt) => {
      const at = clamp(vt.at, 0, text.length);
      return at >= range.from && at <= range.to;
    })
    .sort(
      (a, b) =>
        a.at - b.at ||
        ((a.affinity ?? "after") === "before" ? 0 : 1) - ((b.affinity ?? "after") === "before" ? 0 : 1),
    );
}
function rowTop(row: number, lineHeight: number, paddingTop: number): number {
  return paddingTop + row * lineHeight;
}
// `virtuals` is REQUIRED (pass undefined explicitly): every horizontal rect
// must decide about virtual text, because forgetting it draws the rect
// shifted left of its text on any line with an inlay hint. The optional
// parameter this used to be is exactly how layoutDecorations shipped wrong.
function rectForLine(
  text: RenderableText,
  line: number,
  visualRow: number,
  from: number,
  to: number,
  m: Metrics,
  virtuals: readonly VirtualText[] | undefined,
): LayoutRect {
  const a =
    columnAtOffset(text, line, from, m.tabWidth, m.graphemeWidth) +
      virtualColsBefore(virtuals, text, line, from, m.graphemeWidth),
    b =
      columnAtOffset(text, line, to, m.tabWidth, m.graphemeWidth) +
      virtualColsBefore(virtuals, text, line, to, m.graphemeWidth);
  return {
    top: rowTop(visualRow, m.lineHeight, m.paddingTop),
    left: m.paddingLeft + a * m.charWidth,
    width: Math.max(1, (b - a) * m.charWidth),
    height: m.lineHeight,
  };
}
type Metrics = {
  tabWidth: number;
  lineHeight: number;
  charWidth: number;
  paddingTop: number;
  paddingLeft: number;
  graphemeWidth?: GraphemeWidthFn | undefined;
};

/** Soft-wrap info per window line: segment bounds + hanging indent. */
type WrappedLine = { readonly bounds: readonly number[]; readonly indent: number };
type WrappedBounds = Map<number, WrappedLine>;

/**
 * Visual column of `offset` within one wrap segment, measured from the
 * SEGMENT start — each wrapped row is its own DOM element, so tab stops
 * and virtual-text columns restart there. A virtual anchored exactly at a
 * break belongs to the continuation row (the caret at a wrap point sits on
 * the next visual row, after the hint).
 */
function segmentColumn(
  text: RenderableText,
  segFrom: number,
  segTo: number,
  isLast: boolean,
  offset: number,
  m: Metrics,
  virtuals: readonly VirtualText[] | undefined,
  /** Hanging indent of THIS segment (0 for a line's first segment). */
  indent = 0,
): number {
  const clamped = Math.max(segFrom, Math.min(offset, segTo));
  let column =
    indent + stringColumns(text.sliceString(segFrom, clamped), m.tabWidth, m.graphemeWidth);
  if (virtuals !== undefined) {
    for (const vt of virtuals) {
      const at = clamp(vt.at, 0, text.length);
      if (at < segFrom || at > segTo || (!isLast && at === segTo)) continue;
      if (at < clamped || (at === clamped && (vt.affinity ?? "after") === "before")) {
        column += virtualTextColumns(vt.text, m.graphemeWidth);
      }
    }
  }
  return column;
}

/** The wrap segment containing `offset`. An offset exactly AT a break
 * belongs to the continuation row — unless `assoc` pins it "before"
 * (click-past-row-end affinity keeps the caret on the row it was aimed at). */
function wrappedSegmentAt(
  bounds: readonly number[],
  offset: number,
  assoc?: "before",
): { index: number; from: number; to: number; isLast: boolean } {
  let index = 0;
  while (index + 2 < bounds.length && offset >= bounds[index + 1]!) index++;
  if (assoc === "before" && index > 0 && offset === bounds[index]!) index--;
  return {
    index,
    from: bounds[index]!,
    to: bounds[index + 1]!,
    isLast: index + 2 === bounds.length,
  };
}

/** Rects for [a, b) on a soft-wrapped line: one per intersected segment. */
function wrappedRangeRects(
  text: RenderableText,
  wrapped: WrappedLine,
  firstRow: number,
  a: number,
  b: number,
  m: Metrics,
  virtuals: readonly VirtualText[] | undefined,
): LayoutRect[] {
  const bounds = wrapped.bounds;
  const rects: LayoutRect[] = [];
  for (let s = 0; s + 1 < bounds.length; s++) {
    const segFrom = bounds[s]!;
    const segTo = bounds[s + 1]!;
    const isLast = s + 2 === bounds.length;
    const sa = Math.max(a, segFrom);
    const sb = Math.min(b, segTo);
    if (sb < sa || (sb === sa && a !== b)) continue;
    const indent = s > 0 ? wrapped.indent : 0;
    const colA = segmentColumn(text, segFrom, segTo, isLast, sa, m, virtuals, indent);
    const colB = segmentColumn(text, segFrom, segTo, isLast, sb, m, virtuals, indent);
    rects.push({
      top: rowTop(firstRow + s, m.lineHeight, m.paddingTop),
      left: m.paddingLeft + colA * m.charWidth,
      width: Math.max(1, (colB - colA) * m.charWidth),
      height: m.lineHeight,
    });
  }
  return rects;
}

/**
 * Total VISUAL columns a laid-out text row occupies: its text (tab-expanded)
 * plus every virtual-text segment on it. Content width, horizontal scroll
 * range and thumb sizing must use this, never the raw text width — inlay
 * hints occupy real columns. Owned here so the arithmetic that inserts
 * virtual segments and the arithmetic that measures them cannot diverge.
 */
export function rowVisualColumns(
  row: Extract<LayoutRow, { kind: "text" }>,
  tabWidth: number,
  graphemeWidth?: GraphemeWidthFn,
): number {
  let cols = stringColumns(row.text, tabWidth, graphemeWidth);
  for (const segment of row.segments) {
    if (segment.kind === "virtual-text") cols += virtualTextColumns(segment.text, graphemeWidth);
  }
  return cols;
}

export function layoutContentHeight(args: {
  readonly text: RenderableText;
  readonly lineHeight: number;
  readonly paddingTop?: number;
  readonly paddingBottom?: number;
  readonly blockWidgets?: readonly BlockWidget[];
}): number {
  const blockRows = (args.blockWidgets ?? [])
    .filter((w) => w.heightRows > 0 && w.afterLine >= 0 && w.afterLine < args.text.lineCount)
    .reduce((n, w) => n + w.heightRows, 0);
  return (
    (args.paddingTop ?? 0) +
    (args.text.lineCount + blockRows) * args.lineHeight +
    (args.paddingBottom ?? args.paddingTop ?? 0)
  );
}

export function hitTestTextPosition(args: {
  readonly text: RenderableText;
  readonly x: number;
  readonly y: number;
  readonly lineHeight: number;
  readonly charWidth: number;
  readonly paddingLeft: number;
  readonly paddingTop: number;
  readonly tabWidth: number;
  readonly bias?: HitTestBias;
  /** Virtual text on/near the hit line; clicks inside it land at anchors. */
  readonly virtualText?: readonly VirtualText[];
  readonly graphemeWidth?: GraphemeWidthFn | undefined;
  readonly wrap?: HitTestWrap | undefined;
}): HitTestResult {
  const lineHeight = Math.max(Number.EPSILON, args.lineHeight),
    charWidth = Math.max(Number.EPSILON, args.charWidth);
  const gw = args.graphemeWidth;
  const line = clamp(
    Math.floor((args.y - args.paddingTop) / lineHeight),
    0,
    args.text.lineCount - 1,
  );
  let rawColumn = Math.max(0, (args.x - args.paddingLeft) / charWidth);
  const bias = args.bias ?? "before";
  if (args.wrap !== undefined) {
    // Soft wrap: resolve within ONE segment of the line — x is measured
    // from the segment's start (its own DOM row), never the line's.
    const range = lineRange(args.text, line);
    const lineText = args.text.sliceString(range.from, range.to);
    const breaks = wrapLineBreaks({
      lineText,
      wrapColumns: args.wrap.columns,
      tabWidth: args.tabWidth,
      graphemeWidth: gw,
      lineFrom: range.from,
      virtualText: args.virtualText,
    });
    const bounds = wrapSegmentBounds(range.from, range.to, breaks);
    const segIndex = clamp(args.wrap.row, 0, bounds.length - 2);
    const segFrom = bounds[segIndex]!;
    const segTo = bounds[segIndex + 1]!;
    const isLast = segIndex + 2 === bounds.length;
    // Continuation rows hang under the line's indent: x starts there.
    if (segIndex > 0) {
      rawColumn = Math.max(
        0,
        rawColumn - wrapIndentColumns(lineText, args.tabWidth, args.wrap.columns),
      );
    }
    // Virtual strip, segment-relative (same fixed walk as below).
    {
      let consumed = 0;
      for (const vt of lineVirtuals(args.virtualText, args.text, line)) {
        const at = clamp(vt.at, 0, args.text.length);
        if (at < segFrom || at > segTo || (!isLast && at === segTo)) continue;
        const startColumn =
          stringColumns(args.text.sliceString(segFrom, at), args.tabWidth, gw) + consumed;
        if (rawColumn < startColumn) break;
        const width = virtualTextColumns(vt.text, gw);
        if (rawColumn < startColumn + width) {
          return { line, column: startColumn - consumed, offset: at };
        }
        consumed += width;
      }
      rawColumn = Math.max(0, rawColumn - consumed);
    }
    const resolved = resolveColumn(
      stringText(args.text.sliceString(segFrom, segTo)),
      0,
      rawColumn,
      args.tabWidth,
      bias,
      gw,
    );
    return { line, column: resolved.column, offset: segFrom + resolved.offset };
  }
  // Strip virtual columns left of x; a hit INSIDE a virtual run resolves
  // to its anchor offset (the caret lands where the hint is attached).
  // The walk stays entirely in VISUAL columns — `consumed` inflates each
  // virtual's start column; rawColumn is deflated ONCE afterwards. (The
  // old version deflated rawColumn inside the loop while ALSO inflating
  // the comparisons: from the second hint on, the windows were doubly
  // shifted and text after two hints resolved into the wrong hint.)
  {
    let consumed = 0;
    for (const vt of lineVirtuals(args.virtualText, args.text, line)) {
      const at = clamp(vt.at, 0, args.text.length);
      const startColumn = columnAtOffset(args.text, line, at, args.tabWidth, gw) + consumed;
      if (rawColumn < startColumn) break;
      const width = virtualTextColumns(vt.text, gw);
      if (rawColumn < startColumn + width) {
        return { line, column: startColumn - consumed, offset: at };
      }
      consumed += width;
    }
    rawColumn = Math.max(0, rawColumn - consumed);
  }
  const resolved = resolveColumn(args.text, line, rawColumn, args.tabWidth, bias, gw);
  return { line, column: resolved.column, offset: resolved.offset };
}

/**
 * Hit test that only reports positions ON the text: returns null when the
 * point is left of the text origin (gutter), past the line's last visual
 * column (text plus virtual text, with half a column of slack), or outside
 * the line band vertically. Hover uses this — clamping to the nearest
 * offset makes empty space to the right of a line "hover" its last token.
 */
export function hitTestTextPositionExact(args: {
  readonly text: RenderableText;
  readonly x: number;
  readonly y: number;
  readonly lineHeight: number;
  readonly charWidth: number;
  readonly paddingLeft: number;
  readonly paddingTop: number;
  readonly tabWidth: number;
  readonly virtualText?: readonly VirtualText[];
  readonly graphemeWidth?: GraphemeWidthFn | undefined;
  readonly wrap?: HitTestWrap | undefined;
}): HitTestResult | null {
  const lineHeight = Math.max(Number.EPSILON, args.lineHeight),
    charWidth = Math.max(Number.EPSILON, args.charWidth);
  const rawLine = Math.floor((args.y - args.paddingTop) / lineHeight);
  if (rawLine < 0 || rawLine >= args.text.lineCount) return null;
  const rawColumn = (args.x - args.paddingLeft) / charWidth;
  if (rawColumn < -0.5) return null;
  const range = lineRange(args.text, rawLine);
  if (args.wrap !== undefined) {
    // Soft wrap: "past the text" is judged against THIS visual row only.
    const lineText = args.text.sliceString(range.from, range.to);
    const breaks = wrapLineBreaks({
      lineText,
      wrapColumns: args.wrap.columns,
      tabWidth: args.tabWidth,
      graphemeWidth: args.graphemeWidth,
      lineFrom: range.from,
      virtualText: args.virtualText,
    });
    const bounds = wrapSegmentBounds(range.from, range.to, breaks);
    const segIndex = Math.max(0, Math.min(args.wrap.row, bounds.length - 2));
    const segFrom = bounds[segIndex]!;
    const segTo = bounds[segIndex + 1]!;
    const isLast = segIndex + 2 === bounds.length;
    const indent =
      segIndex > 0 ? wrapIndentColumns(lineText, args.tabWidth, args.wrap.columns) : 0;
    const segVirtualCols = lineVirtuals(args.virtualText, args.text, rawLine).reduce((n, vt) => {
      const at = clamp(vt.at, 0, args.text.length);
      if (at < segFrom || at > segTo || (!isLast && at === segTo)) return n;
      return n + virtualTextColumns(vt.text, args.graphemeWidth);
    }, 0);
    const maxColumn =
      indent +
      stringColumns(args.text.sliceString(segFrom, segTo), args.tabWidth, args.graphemeWidth) +
      segVirtualCols;
    if (rawColumn > maxColumn + 0.5) return null;
    return hitTestTextPosition({ ...args, bias: "nearest" });
  }
  const virtualCols = lineVirtuals(args.virtualText, args.text, rawLine).reduce(
    (n, vt) => n + virtualTextColumns(vt.text, args.graphemeWidth),
    0,
  );
  const maxColumn =
    columnAtOffset(args.text, rawLine, range.to, args.tabWidth, args.graphemeWidth) + virtualCols;
  if (rawColumn > maxColumn + 0.5) return null;
  return hitTestTextPosition({ ...args, bias: "nearest" });
}

function normalizeRange(
  from: number,
  to: number,
  length: number,
): { from: number; to: number } | undefined {
  const a = clamp(from, 0, length),
    b = clamp(to, 0, length);
  return b > a ? { from: a, to: b } : undefined;
}
function styleSegments(
  text: RenderableText,
  rowFrom: number,
  rowTo: number,
  rowText: string,
  runs: readonly TextStyleRun[] = [],
  virtuals: readonly VirtualText[] = [],
): readonly LayoutInlineSegment[] {
  const activeRuns = runs
    .map((run, index) => ({ run, index, range: normalizeRange(run.from, run.to, text.length) }))
    .filter(
      (x): x is { run: TextStyleRun; index: number; range: { from: number; to: number } } =>
        !!x.range && x.range.to > rowFrom && x.range.from < rowTo,
    );
  const vts = virtuals
    .map((vt, index) => ({ vt, index, at: clamp(vt.at, 0, text.length) }))
    .filter((x) => x.at >= rowFrom && x.at <= rowTo)
    .sort(
      (a, b) =>
        a.at - b.at ||
        ((a.vt.affinity ?? "after") === "before" ? -1 : 1) -
          ((b.vt.affinity ?? "after") === "before" ? -1 : 1) ||
        priority(a.vt) - priority(b.vt) ||
        a.index - b.index,
    );
  const cuts = new Set<number>([0, rowTo - rowFrom]);
  for (const x of activeRuns) {
    cuts.add(clamp(x.range.from, rowFrom, rowTo) - rowFrom);
    cuts.add(clamp(x.range.to, rowFrom, rowTo) - rowFrom);
  }
  for (const x of vts) cuts.add(x.at - rowFrom);
  const orderedCuts = [...cuts].sort((a, b) => a - b);
  const segments: LayoutInlineSegment[] = [];
  const pushVirtuals = (local: number, affinity: VirtualTextAffinity) => {
    for (const x of vts)
      if (x.at - rowFrom === local && (x.vt.affinity ?? "after") === affinity)
        segments.push({
          kind: "virtual-text",
          at: local,
          text: x.vt.text,
          affinity,
          ...(x.vt.className ? { className: x.vt.className } : {}),
          ...(x.vt.style ? { style: x.vt.style } : {}),
          ...(x.vt.owner ? { owner: x.vt.owner } : {}),
        });
  };
  for (let i = 0; i < orderedCuts.length - 1; i++) {
    const from = orderedCuts[i]!,
      to = orderedCuts[i + 1]!;
    pushVirtuals(from, "before");
    if (to > from) {
      const docFrom = rowFrom + from,
        docTo = rowFrom + to;
      const act = activeRuns
        .filter((x) => x.range.from < docTo && x.range.to > docFrom)
        .sort((a, b) => priority(a.run) - priority(b.run) || a.index - b.index);
      const className =
        act
          .map((x) => x.run.className)
          .filter(Boolean)
          .join(" ") || undefined;
      const style = Object.assign({}, ...act.map((x) => x.run.style ?? {}));
      const owners = [...new Set(act.map((x) => x.run.owner).filter((x): x is string => !!x))];
      segments.push({
        kind: "text",
        from,
        to,
        text: rowText.slice(from, to),
        ...(className ? { className } : {}),
        ...(Object.keys(style).length ? { style } : {}),
        ...(owners.length ? { owners } : {}),
      });
    }
    pushVirtuals(from, "after");
  }
  const endLocal = rowTo - rowFrom;
  pushVirtuals(endLocal, "before");
  pushVirtuals(endLocal, "after");
  return segments.length
    ? segments
    : [{ kind: "text", from: 0, to: rowText.length, text: rowText }];
}

export function layoutViewport<Payload = unknown>(
  request: ViewportLayoutRequest<Payload>,
): ViewportLayoutSnapshot<Payload> {
  const m: Metrics = {
    tabWidth: request.tabWidth ?? 4,
    lineHeight: request.lineHeight ?? 1,
    charWidth: request.charWidth ?? 1,
    paddingTop: request.paddingTop ?? 0,
    paddingLeft: request.paddingLeft ?? 0,
    graphemeWidth: request.graphemeWidth,
  };
  const anchors = request.anchors ?? [],
    visuals = request.visuals ?? {};
  const start = Math.max(0, request.startLine),
    end = Math.min(request.text.lineCount, start + Math.max(0, request.lineCount));
  const rows: LayoutRow[] = [],
    lineToRow = new Map<number, number>(),
    blockRowById = new Map<string, number>(),
    widgetByRow = new Map<string, number>();
  const blockItems = [
    ...anchors
      .filter((a): a is Extract<LayoutAnchor<Payload>, { kind: "block" }> => a.kind === "block")
      .map((a, index) => ({
        type: "anchor" as const,
        id: a.id,
        afterLine: a.afterLine,
        heightRows: a.heightRows,
        index,
        payload: a.payload,
      })),
    ...(visuals.blockWidgets ?? []).map((w, index) => ({
      type: "widget" as const,
      id: w.id,
      afterLine: w.afterLine,
      heightRows: w.heightRows,
      index,
      owner: w.owner,
    })),
  ]
    .filter((b) => b.heightRows > 0)
    .sort((a, b) => a.afterLine - b.afterLine || a.index - b.index);
  // All geometry is document-absolute: a row's `top` is its offset inside the
  // full content box (the same coordinate space as layoutContentHeight and
  // hitTestTextPosition), so the renderer can place viewport rows directly
  // without knowing what lies outside the window. Block rows that sit entirely
  // above the window shift the whole window down.
  let row = request.startRow ?? start;
  // Blocks above the window shift it down. A "leading" block (afterLine < 0)
  // belongs before the first line: at the very top it is EMITTED as the first
  // row(s); off the top of the window it only contributes its height.
  for (const b of blockItems) {
    if (b.afterLine >= start) break;
    if (start === 0 && b.afterLine < 0) {
      if (b.type === "anchor") blockRowById.set(b.id, row);
      else widgetByRow.set(b.id, row);
      rows.push({
        kind: "block",
        row,
        afterLine: b.afterLine,
        heightRows: b.heightRows,
        top: rowTop(row, m.lineHeight, m.paddingTop),
        left: m.paddingLeft,
        height: b.heightRows * m.lineHeight,
      });
    }
    row += b.heightRows;
  }
  // Soft wrap: each window line's segment bounds, shared by rects/overlays.
  const wrappedBounds: WrappedBounds | null = request.wrapColumns !== undefined ? new Map() : null;
  for (let line = start; line < end; line++) {
    const r = request.text.lineRange(line),
      rowText = request.text.sliceString(r.from, r.to);
    lineToRow.set(line, row);
    if (wrappedBounds !== null) {
      const breaks = wrapLineBreaks({
        lineText: rowText,
        wrapColumns: request.wrapColumns!,
        tabWidth: m.tabWidth,
        graphemeWidth: m.graphemeWidth,
        lineFrom: r.from,
        virtualText: visuals.virtualText,
      });
      const bounds = wrapSegmentBounds(r.from, r.to, breaks);
      const indent =
        breaks.length > 0 ? wrapIndentColumns(rowText, m.tabWidth, request.wrapColumns!) : 0;
      wrappedBounds.set(line, { bounds, indent });
      for (let s = 0; s + 1 < bounds.length; s++) {
        const segFrom = bounds[s]!;
        const segTo = bounds[s + 1]!;
        const isLast = s + 2 === bounds.length;
        // A virtual anchored exactly at a break renders once, on the
        // continuation row (where its anchor's caret sits).
        const segVirtuals = visuals.virtualText?.filter((vt) => {
          const at = clamp(vt.at, 0, request.text.length);
          return at >= segFrom && (isLast ? at <= segTo : at < segTo);
        });
        rows.push({
          kind: "text",
          row,
          line,
          from: segFrom,
          to: segTo,
          text: rowText.slice(segFrom - r.from, segTo - r.from),
          segments: styleSegments(
            request.text,
            segFrom,
            segTo,
            rowText.slice(segFrom - r.from, segTo - r.from),
            visuals.textStyles,
            segVirtuals,
          ),
          top: rowTop(row, m.lineHeight, m.paddingTop),
          left: m.paddingLeft,
          height: m.lineHeight,
          ...(s > 0 ? { continuation: true } : {}),
          ...(s > 0 && indent > 0 ? { indentCols: indent } : {}),
        });
        row++;
      }
    } else {
      rows.push({
        kind: "text",
        row,
        line,
        from: r.from,
        to: r.to,
        text: rowText,
        segments: styleSegments(
          request.text,
          r.from,
          r.to,
          rowText,
          visuals.textStyles,
          visuals.virtualText,
        ),
        top: rowTop(row, m.lineHeight, m.paddingTop),
        left: m.paddingLeft,
        height: m.lineHeight,
      });
      row++;
    }
    for (const b of blockItems)
      if (b.afterLine === line) {
        if (b.type === "anchor") blockRowById.set(b.id, row);
        else widgetByRow.set(b.id, row);
        rows.push({
          kind: "block",
          row,
          afterLine: b.afterLine,
          heightRows: b.heightRows,
          top: rowTop(row, m.lineHeight, m.paddingTop),
          left: m.paddingLeft,
          height: b.heightRows * m.lineHeight,
        });
        row += b.heightRows;
      }
  }

  const placements: LayoutPlacement<Payload>[] = [];
  for (const anchor of anchors) {
    if (anchor.kind === "line") {
      const row = lineToRow.get(anchor.line);
      if (row !== undefined)
        placements.push({
          kind: "line",
          id: anchor.id,
          row,
          line: anchor.line,
          payload: anchor.payload,
        });
    } else if (anchor.kind === "block") {
      const row = blockRowById.get(anchor.id);
      if (row !== undefined && anchor.heightRows > 0)
        placements.push({
          kind: "block",
          id: anchor.id,
          row,
          afterLine: anchor.afterLine,
          heightRows: anchor.heightRows,
          rect: {
            top: rowTop(row, m.lineHeight, m.paddingTop),
            left: m.paddingLeft,
            width: 0,
            height: anchor.heightRows * m.lineHeight,
          },
          payload: anchor.payload,
        });
    } else if (anchor.kind === "overlay") {
      const at = clamp(anchor.at, 0, request.text.length),
        line = lineIndexAtOffset(request.text, at),
        row = lineToRow.get(line);
      if (row !== undefined) {
        const wrapped = wrappedBounds?.get(line);
        let visualRow = row;
        let column: number;
        if (wrapped !== undefined) {
          const seg = wrappedSegmentAt(wrapped.bounds, at, anchor.assoc);
          visualRow = row + seg.index;
          column = segmentColumn(
            request.text,
            seg.from,
            seg.to,
            seg.isLast,
            at,
            m,
            visuals.virtualText,
            seg.index > 0 ? wrapped.indent : 0,
          );
        } else {
          column =
            columnAtOffset(request.text, line, at, m.tabWidth, m.graphemeWidth) +
            virtualColsBefore(visuals.virtualText, request.text, line, at, m.graphemeWidth);
        }
        placements.push({
          kind: "overlay",
          id: anchor.id,
          row: visualRow,
          line,
          at,
          column,
          rect: {
            top: rowTop(visualRow, m.lineHeight, m.paddingTop),
            left: m.paddingLeft + column * m.charWidth,
            width: 1,
            height: m.lineHeight,
          },
          payload: anchor.payload,
        });
      }
    } else {
      addRangePlacement(
        request.text,
        start,
        end,
        lineToRow,
        m,
        anchor,
        placements,
        visuals.virtualText,
        wrappedBounds,
      );
    }
  }
  const decorations = layoutDecorations(
    request.text,
    start,
    end,
    lineToRow,
    m,
    visuals.rangeDecorations ?? [],
    visuals.virtualText,
    wrappedBounds,
  );
  const lineDecorations = layoutLineDecorations(
    request.text,
    start,
    end,
    lineToRow,
    m,
    visuals.lineDecorations ?? [],
    wrappedBounds,
  );
  const verticalGuides = layoutVerticalGuides(
    request.text,
    start,
    end,
    lineToRow,
    m,
    visuals.verticalGuides ?? [],
    wrappedBounds,
  );
  const blockWidgets = (visuals.blockWidgets ?? []).flatMap((w): LayoutBlockWidget[] => {
    const row = widgetByRow.get(w.id);
    return row === undefined || w.heightRows <= 0
      ? []
      : [
          {
            id: w.id,
            afterLine: w.afterLine,
            row,
            rect: {
              top: rowTop(row, m.lineHeight, m.paddingTop),
              left: m.paddingLeft,
              width: 0,
              height: w.heightRows * m.lineHeight,
            },
            ...(w.owner ? { owner: w.owner } : {}),
          },
        ];
  });
  return {
    version: request.version,
    startLine: start,
    lineCount: end - start,
    rows,
    placements,
    decorations,
    lineDecorations,
    verticalGuides,
    blockWidgets,
  };
}

function addRangePlacement<Payload>(
  text: RenderableText,
  start: number,
  end: number,
  lineToRow: Map<number, number>,
  m: Metrics,
  anchor: Extract<LayoutAnchor<Payload>, { kind: "range" }>,
  out: LayoutPlacement<Payload>[],
  virtuals?: readonly VirtualText[],
  wrapped?: WrappedBounds | null,
) {
  const range = normalizeRange(anchor.from, anchor.to, text.length);
  if (!range) return;
  const first = lineIndexAtOffset(text, range.from),
    last = lineIndexAtOffset(text, Math.max(range.from, range.to - 1));
  for (let line = Math.max(start, first); line <= Math.min(end - 1, last); line++) {
    const row = lineToRow.get(line);
    if (row === undefined) continue;
    const lr = lineRange(text, line),
      a = Math.max(range.from, lr.from),
      b = Math.min(range.to, lr.to);
    if (b < a) continue;
    const wrappedLine = wrapped?.get(line);
    if (wrappedLine !== undefined) {
      const segA = wrappedSegmentAt(wrappedLine.bounds, a);
      const segB = wrappedSegmentAt(wrappedLine.bounds, Math.max(a, b - 1));
      out.push({
        kind: "range",
        id: anchor.id,
        row: row + segA.index,
        line,
        from: a,
        to: b,
        columnStart: segmentColumn(
          text,
          segA.from,
          segA.to,
          segA.isLast,
          a,
          m,
          virtuals,
          segA.index > 0 ? wrappedLine.indent : 0,
        ),
        columnEnd: segmentColumn(
          text,
          segB.from,
          segB.to,
          segB.isLast,
          b,
          m,
          virtuals,
          segB.index > 0 ? wrappedLine.indent : 0,
        ),
        rects: wrappedRangeRects(text, wrappedLine, row, a, b, m, virtuals),
        payload: anchor.payload,
      });
      continue;
    }
    const columnStart =
        columnAtOffset(text, line, a, m.tabWidth, m.graphemeWidth) +
        virtualColsBefore(virtuals, text, line, a, m.graphemeWidth),
      columnEnd =
        columnAtOffset(text, line, b, m.tabWidth, m.graphemeWidth) +
        virtualColsBefore(virtuals, text, line, b, m.graphemeWidth);
    out.push({
      kind: "range",
      id: anchor.id,
      row,
      line,
      from: a,
      to: b,
      columnStart,
      columnEnd,
      rects: [rectForLine(text, line, row, a, b, m, virtuals)],
      payload: anchor.payload,
    });
  }
}
function visualRowsForLine(line: number, row: number, wrapped?: WrappedBounds | null): number {
  const wrappedLine = wrapped?.get(line);
  return wrappedLine === undefined ? 1 : Math.max(1, wrappedLine.bounds.length - 1);
}

function layoutVerticalGuides(
  text: RenderableText,
  start: number,
  end: number,
  lineToRow: Map<number, number>,
  m: Metrics,
  guides: readonly VerticalGuide[],
  wrapped?: WrappedBounds | null,
): readonly LayoutVerticalGuide[] {
  return guides
    .flatMap((guide, index) => {
      const from = clamp(guide.from, 0, text.length);
      const to = clamp(guide.to, from, text.length);
      const first = lineIndexAtOffset(text, from);
      const last = lineIndexAtOffset(text, Math.max(from, to - 1));
      const firstVisible = Math.max(start, first);
      const lastVisible = Math.min(end - 1, last);
      if (lastVisible < firstVisible) return [];
      const startRow = lineToRow.get(firstVisible);
      const endStartRow = lineToRow.get(lastVisible);
      if (startRow === undefined || endStartRow === undefined) return [];
      const endRow = endStartRow + visualRowsForLine(lastVisible, endStartRow, wrapped);
      return [
        {
          ...guide,
          from,
          to,
          row: startRow,
          line: firstVisible,
          top: rowTop(startRow, m.lineHeight, m.paddingTop),
          left: m.paddingLeft + guide.column * m.charWidth,
          height: Math.max(1, endRow - startRow) * m.lineHeight,
          _index: index,
        } as LayoutVerticalGuide & { readonly _index: number },
      ];
    })
    .sort((a, b) => priority(a) - priority(b) || a._index - b._index)
    .map(({ _index, ...guide }) => guide);
}

function layoutLineDecorations(
  text: RenderableText,
  start: number,
  end: number,
  lineToRow: Map<number, number>,
  m: Metrics,
  decorations: readonly LineDecoration[],
  wrapped?: WrappedBounds | null,
): readonly LayoutLineDecoration[] {
  return decorations
    .flatMap((dec, index) => {
      const at = clamp(dec.at, 0, text.length);
      const line = lineIndexAtOffset(text, at);
      if (line < start || line >= end) return [];
      const row = lineToRow.get(line);
      if (row === undefined) return [];
      const wrappedLine = wrapped?.get(line);
      let visualRow = row;
      let heightRows = 1;
      if (wrappedLine !== undefined) {
        if (dec.scope === "logical-line") {
          heightRows = Math.max(1, wrappedLine.bounds.length - 1);
        } else {
          visualRow = row + wrappedSegmentAt(wrappedLine.bounds, at).index;
        }
      }
      return [
        {
          ...dec,
          row: visualRow,
          line,
          top: rowTop(visualRow, m.lineHeight, m.paddingTop),
          height: heightRows * m.lineHeight,
          _index: index,
        } as LayoutLineDecoration & { readonly _index: number },
      ];
    })
    .sort((a, b) => priority(a) - priority(b) || a._index - b._index)
    .map(({ _index, ...dec }) => dec);
}

function layoutDecorations(
  text: RenderableText,
  start: number,
  end: number,
  lineToRow: Map<number, number>,
  m: Metrics,
  decorations: readonly RangeDecoration[],
  virtuals: readonly VirtualText[] | undefined,
  wrapped?: WrappedBounds | null,
): readonly LayoutDecoration[] {
  const layerOrder: Record<DecorationLayer, number> = { background: 0, underline: 1, outline: 2 };
  return decorations
    .flatMap((dec, index) => {
      const range = normalizeRange(dec.from, dec.to, text.length);
      if (!range) return [];
      const rects: LayoutRect[] = [];
      const first = lineIndexAtOffset(text, range.from),
        last = lineIndexAtOffset(text, Math.max(range.from, range.to - 1));
      for (let line = Math.max(start, first); line <= Math.min(end - 1, last); line++) {
        const row = lineToRow.get(line);
        if (row === undefined) continue;
        const lr = lineRange(text, line),
          a = Math.max(range.from, lr.from),
          b = Math.min(range.to, lr.to);
        if (b < a) continue;
        const wrappedLine = wrapped?.get(line);
        if (wrappedLine !== undefined) {
          rects.push(...wrappedRangeRects(text, wrappedLine, row, a, b, m, virtuals));
        } else {
          rects.push(rectForLine(text, line, row, a, b, m, virtuals));
        }
      }
      return rects.length
        ? [
            { ...dec, from: range.from, to: range.to, rects, _index: index } as LayoutDecoration & {
              _index: number;
            },
          ]
        : [];
    })
    .sort(
      (a, b) =>
        layerOrder[a.layer] - layerOrder[b.layer] ||
        priority(a) - priority(b) ||
        a._index - b._index,
    )
    .map(({ _index, ...d }) => d);
}
