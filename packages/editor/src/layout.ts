import { columnAtOffset as engineColumnAtOffset, offsetAtColumn, type TextBuffer } from "@the-editor/engine";

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
  readonly text: TextBuffer;
  readonly version: number;
  readonly startLine: number;
  readonly lineCount: number;
  readonly tabWidth?: number;
  readonly lineHeight?: number;
  readonly charWidth?: number;
  readonly paddingTop?: number;
  readonly paddingLeft?: number;
  readonly anchors?: readonly LayoutAnchor<Payload>[];
};

export type LayoutRect = {
  readonly top: number;
  readonly left: number;
  readonly width: number;
  readonly height: number;
};

export type LayoutRow =
  | {
      readonly kind: "text";
      readonly row: number;
      readonly line: number;
      readonly from: number;
      readonly to: number;
      readonly text: string;
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

export type ViewportLayoutSnapshot<Payload = unknown> = {
  readonly version: number;
  readonly startLine: number;
  readonly lineCount: number;
  readonly rows: readonly LayoutRow[];
  readonly placements: readonly LayoutPlacement<Payload>[];
};

export type HitTestBias = "before" | "after" | "nearest";

export type HitTestResult = {
  readonly line: number;
  readonly column: number;
  readonly offset: number;
};

export function layoutContentHeight(args: {
  readonly text: TextBuffer;
  readonly lineHeight: number;
  readonly paddingTop?: number;
  readonly paddingBottom?: number;
}): number {
  return (args.paddingTop ?? 0) + args.text.lineCount * args.lineHeight + (args.paddingBottom ?? args.paddingTop ?? 0);
}

export function hitTestTextPosition(args: {
  readonly text: TextBuffer;
  readonly x: number;
  readonly y: number;
  readonly lineHeight: number;
  readonly charWidth: number;
  readonly paddingLeft: number;
  readonly paddingTop: number;
  readonly tabWidth: number;
  readonly bias?: HitTestBias;
}): HitTestResult {
  const lineHeight = Math.max(Number.EPSILON, args.lineHeight);
  const charWidth = Math.max(Number.EPSILON, args.charWidth);
  const line = Math.max(
    0,
    Math.min(args.text.lineCount - 1, Math.floor((args.y - args.paddingTop) / lineHeight)),
  );
  const rawColumn = Math.max(0, (args.x - args.paddingLeft) / charWidth);
  const bias = args.bias ?? "before";
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
    const beforeColumn = engineColumnAtOffset({
      text: args.text,
      line,
      offset: beforeOffset,
      tabWidth: args.tabWidth,
    });
    const afterColumn = engineColumnAtOffset({
      text: args.text,
      line,
      offset: afterOffset,
      tabWidth: args.tabWidth,
    });
    const offset =
      Math.abs(rawColumn - beforeColumn) < Math.abs(afterColumn - rawColumn)
        ? beforeOffset
        : afterOffset;
    const column = offset === beforeOffset ? beforeColumn : afterColumn;
    return { line, column, offset };
  }
  const targetColumn = bias === "before" ? Math.floor(rawColumn) : Math.ceil(rawColumn);
  const offset = offsetAtColumn({
    text: args.text,
    line,
    column: targetColumn,
    tabWidth: args.tabWidth,
    bias,
  });
  const column = engineColumnAtOffset({
    text: args.text,
    line,
    offset,
    tabWidth: args.tabWidth,
  });
  return { line, column, offset };
}

function lineRange(text: TextBuffer, line: number): { from: number; to: number } {
  const r = text.lineRange(line);
  return { from: r.from, to: r.to };
}

function lineIndexAtOffset(text: TextBuffer, offset: number): number {
  return text.lineRangeAt(Math.max(0, Math.min(text.length, offset))).index;
}

function columnAtOffset(text: TextBuffer, line: number, offset: number, tabWidth: number): number {
  return engineColumnAtOffset({ text, line, offset, tabWidth });
}

function textTop(line: number, lineHeight: number, paddingTop: number): number {
  return paddingTop + line * lineHeight;
}

export function layoutViewport<Payload = unknown>(
  request: ViewportLayoutRequest<Payload>,
): ViewportLayoutSnapshot<Payload> {
  const tabWidth = request.tabWidth ?? 4;
  const lineHeight = request.lineHeight ?? 1;
  const charWidth = request.charWidth ?? 1;
  const paddingTop = request.paddingTop ?? 0;
  const paddingLeft = request.paddingLeft ?? 0;
  const anchors = request.anchors ?? [];
  const start = Math.max(0, request.startLine);
  const end = Math.min(request.text.lineCount, start + Math.max(0, request.lineCount));
  const rows: LayoutRow[] = [];
  const lineToRow = new Map<number, number>();
  const blockRowById = new Map<string, number>();
  let row = 0;
  for (let line = start; line < end; line++) {
    const r = request.text.lineRange(line);
    lineToRow.set(line, row);
    rows.push({
      kind: "text",
      row,
      line,
      from: r.from,
      to: r.to,
      text: request.text.sliceString(r.from, r.to),
      top: textTop(line, lineHeight, paddingTop),
      left: paddingLeft,
      height: lineHeight,
    });
    row++;
    for (const anchor of anchors) {
      if (anchor.kind !== "block" || anchor.afterLine !== line || anchor.heightRows <= 0) continue;
      blockRowById.set(anchor.id, row);
      rows.push({
        kind: "block",
        row,
        afterLine: anchor.afterLine,
        heightRows: anchor.heightRows,
        top: textTop(anchor.afterLine + 1, lineHeight, paddingTop),
        left: paddingLeft,
        height: anchor.heightRows * lineHeight,
      });
      row += anchor.heightRows;
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
            top: textTop(anchor.afterLine + 1, lineHeight, paddingTop),
            left: paddingLeft,
            width: 0,
            height: anchor.heightRows * lineHeight,
          },
          payload: anchor.payload,
        });
    } else if (anchor.kind === "overlay") {
      const at = Math.max(0, Math.min(request.text.length, anchor.at));
      const line = lineIndexAtOffset(request.text, at);
      const row = lineToRow.get(line);
      if (row !== undefined) {
        const column = columnAtOffset(request.text, line, at, tabWidth);
        placements.push({
          kind: "overlay",
          id: anchor.id,
          row,
          line,
          at,
          column,
          rect: {
            top: textTop(line, lineHeight, paddingTop),
            left: paddingLeft + column * charWidth,
            width: 1,
            height: lineHeight,
          },
          payload: anchor.payload,
        });
      }
    } else {
      const from = Math.max(0, Math.min(request.text.length, anchor.from));
      const to = Math.max(0, Math.min(request.text.length, anchor.to));
      if (to <= from) continue;
      const first = lineIndexAtOffset(request.text, from);
      const last = lineIndexAtOffset(request.text, Math.max(from, to - 1));
      for (let line = Math.max(start, first); line <= Math.min(end - 1, last); line++) {
        const row = lineToRow.get(line);
        if (row === undefined) continue;
        const lr = lineRange(request.text, line);
        const a = Math.max(from, lr.from),
          b = Math.min(to, lr.to);
        if (b < a) continue;
        const columnStart = columnAtOffset(request.text, line, a, tabWidth);
        const columnEnd = columnAtOffset(request.text, line, b, tabWidth);
        placements.push({
          kind: "range",
          id: anchor.id,
          row,
          line,
          from: a,
          to: b,
          columnStart,
          columnEnd,
          rects: [
            {
              top: textTop(line, lineHeight, paddingTop),
              left: paddingLeft + columnStart * charWidth,
              width: Math.max(1, (columnEnd - columnStart) * charWidth),
              height: lineHeight,
            },
          ],
          payload: anchor.payload,
        });
      }
    }
  }
  return { version: request.version, startLine: start, lineCount: end - start, rows, placements };
}
