import {
  columnAtOffset as engineColumnAtOffset,
  offsetAtColumn,
  type RenderableText,
} from "@the-editor/engine";

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
export type RangeDecoration = {
  readonly from: number;
  readonly to: number;
  readonly layer: DecorationLayer;
  readonly className?: string;
  readonly style?: Readonly<Record<string, string>>;
  readonly priority?: number;
  readonly owner?: EditorVisualOwner;
};
export type VirtualTextAffinity = "before" | "after";
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
  readonly blockWidgets: readonly LayoutBlockWidget[];
};
export type HitTestBias = "before" | "after" | "nearest";
export type HitTestResult = {
  readonly line: number;
  readonly column: number;
  readonly offset: number;
};

const clamp = (n: number, min: number, max: number) => Math.max(min, Math.min(max, n));
const priority = (x: { readonly priority?: number }) => x.priority ?? 0;
function lineRange(text: RenderableText, line: number): { from: number; to: number } {
  const r = text.lineRange(line);
  return { from: r.from, to: r.to };
}
function lineIndexAtOffset(text: RenderableText, offset: number): number {
  return text.lineRangeAt(clamp(offset, 0, text.length)).index;
}
function columnAtOffset(text: RenderableText, line: number, offset: number, tabWidth: number): number {
  return engineColumnAtOffset({ text, line, offset, tabWidth });
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
): number {
  if (virtuals === undefined || virtuals.length === 0) return 0;
  const range = text.lineRange(line);
  let cols = 0;
  for (const vt of virtuals) {
    const at = clamp(vt.at, 0, text.length);
    if (at < range.from || at > range.to) continue;
    if (at < offset || (at === offset && (vt.affinity ?? "after") === "before")) {
      cols += vt.text.length;
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
function rectForLine(
  text: RenderableText,
  line: number,
  visualRow: number,
  from: number,
  to: number,
  m: Metrics,
  virtuals?: readonly VirtualText[],
): LayoutRect {
  const a =
    columnAtOffset(text, line, from, m.tabWidth) + virtualColsBefore(virtuals, text, line, from),
    b = columnAtOffset(text, line, to, m.tabWidth) + virtualColsBefore(virtuals, text, line, to);
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
};

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
}): HitTestResult {
  const lineHeight = Math.max(Number.EPSILON, args.lineHeight),
    charWidth = Math.max(Number.EPSILON, args.charWidth);
  const line = clamp(
    Math.floor((args.y - args.paddingTop) / lineHeight),
    0,
    args.text.lineCount - 1,
  );
  let rawColumn = Math.max(0, (args.x - args.paddingLeft) / charWidth);
  const bias = args.bias ?? "before";
  // Strip virtual columns left of x; a hit INSIDE a virtual run resolves
  // to its anchor offset (the caret lands where the hint is attached).
  {
    let consumed = 0;
    for (const vt of lineVirtuals(args.virtualText, args.text, line)) {
      const at = clamp(vt.at, 0, args.text.length);
      const baseColumn =
        columnAtOffset(args.text, line, at, args.tabWidth) + consumed;
      if (rawColumn < baseColumn) break;
      if (rawColumn < baseColumn + vt.text.length) {
        return { line, column: baseColumn - consumed, offset: at };
      }
      consumed += vt.text.length;
      rawColumn -= vt.text.length;
    }
  }
  if (bias === "nearest") {
    const beforeOffset = offsetAtColumn({
      text: args.text,
      line,
      column: Math.floor(rawColumn),
      tabWidth: args.tabWidth,
      bias: "before",
    });
    const afterOffset = offsetAtColumn({
      text: args.text,
      line,
      column: Math.ceil(rawColumn),
      tabWidth: args.tabWidth,
      bias: "after",
    });
    const beforeColumn = columnAtOffset(args.text, line, beforeOffset, args.tabWidth),
      afterColumn = columnAtOffset(args.text, line, afterOffset, args.tabWidth);
    const offset =
      Math.abs(rawColumn - beforeColumn) < Math.abs(afterColumn - rawColumn)
        ? beforeOffset
        : afterOffset;
    return { line, column: offset === beforeOffset ? beforeColumn : afterColumn, offset };
  }
  const targetColumn = bias === "before" ? Math.floor(rawColumn) : Math.ceil(rawColumn);
  const offset = offsetAtColumn({
    text: args.text,
    line,
    column: targetColumn,
    tabWidth: args.tabWidth,
    bias,
  });
  return { line, column: columnAtOffset(args.text, line, offset, args.tabWidth), offset };
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
}): HitTestResult | null {
  const lineHeight = Math.max(Number.EPSILON, args.lineHeight),
    charWidth = Math.max(Number.EPSILON, args.charWidth);
  const rawLine = Math.floor((args.y - args.paddingTop) / lineHeight);
  if (rawLine < 0 || rawLine >= args.text.lineCount) return null;
  const rawColumn = (args.x - args.paddingLeft) / charWidth;
  if (rawColumn < -0.5) return null;
  const range = lineRange(args.text, rawLine);
  const virtualCols = lineVirtuals(args.virtualText, args.text, rawLine).reduce(
    (n, vt) => n + vt.text.length,
    0,
  );
  const maxColumn =
    columnAtOffset(args.text, rawLine, range.to, args.tabWidth) + virtualCols;
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
  let row = start;
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
  for (let line = start; line < end; line++) {
    const r = request.text.lineRange(line),
      rowText = request.text.sliceString(r.from, r.to);
    lineToRow.set(line, row);
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
        const column =
          columnAtOffset(request.text, line, at, m.tabWidth) +
          virtualColsBefore(visuals.virtualText, request.text, line, at);
        placements.push({
          kind: "overlay",
          id: anchor.id,
          row,
          line,
          at,
          column,
          rect: {
            top: rowTop(row, m.lineHeight, m.paddingTop),
            left: m.paddingLeft + column * m.charWidth,
            width: 1,
            height: m.lineHeight,
          },
          payload: anchor.payload,
        });
      }
    } else {
      addRangePlacement(request.text, start, end, lineToRow, m, anchor, placements, visuals.virtualText);
    }
  }
  const decorations = layoutDecorations(
    request.text,
    start,
    end,
    lineToRow,
    m,
    visuals.rangeDecorations ?? [],
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
    const columnStart =
        columnAtOffset(text, line, a, m.tabWidth) + virtualColsBefore(virtuals, text, line, a),
      columnEnd =
        columnAtOffset(text, line, b, m.tabWidth) + virtualColsBefore(virtuals, text, line, b);
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
function layoutDecorations(
  text: RenderableText,
  start: number,
  end: number,
  lineToRow: Map<number, number>,
  m: Metrics,
  decorations: readonly RangeDecoration[],
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
        if (b >= a) rects.push(rectForLine(text, line, row, a, b, m));
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
