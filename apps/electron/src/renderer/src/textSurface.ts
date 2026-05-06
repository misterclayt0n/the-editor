import type { CellGridMetrics, CursorPlacement, RenderFrame, RenderRow, SyntaxSpan, Viewport } from "@the-editor/contracts/runtime";
import type { BufferDecoration, InlineTextDecoration, RangeDecoration, ThemeSnapshot } from "@the-editor/editor";
import { resolveThemeStyle, type ResolvedThemeDefinition } from "@the-editor/editor/themes";

export interface SurfaceConfiguration {
  viewport: Viewport;
  metrics: CellGridMetrics;
}

interface TextSurfaceTheme {
  background: string;
  foreground: string;
  cursorBackground: string;
  cursorForeground: string;
  selectionBackground: string;
  syntaxColor(kind: number): string;
}

const PADDING_PX = 18;
const FONT_SIZE_PX = 15;
const LINE_HEIGHT_PX = 22;
const FONT_FAMILY = "Iosevka, Menlo, Monaco, Consolas, 'SF Mono', ui-monospace, monospace";
const FONT = `${FONT_SIZE_PX}px ${FONT_FAMILY}`;
const SEGMENTER = typeof Intl !== "undefined" && "Segmenter" in Intl
  ? new Intl.Segmenter(undefined, { granularity: "grapheme" })
  : undefined;

const fallbackSyntaxScopes: Record<number, string> = {
  1: "keyword",
  2: "type",
  3: "variable",
  4: "string",
  5: "comment",
};

export async function loadEditorFont(): Promise<void> {
  if (!("fonts" in document)) return;
  await document.fonts.load(FONT, "mmmmmmmmmmmmmmmm");
  await document.fonts.ready;
}

export function configureTextSurface(canvas: HTMLCanvasElement): SurfaceConfiguration {
  const rect = (canvas.parentElement ?? canvas).getBoundingClientRect();
  const cssWidth = Math.max(1, Math.floor(rect.width));
  const cssHeight = Math.max(1, Math.floor(rect.height));
  const dpr = window.devicePixelRatio || 1;

  canvas.style.width = `${cssWidth}px`;
  canvas.style.height = `${cssHeight}px`;
  canvas.width = Math.max(1, Math.floor(cssWidth * dpr));
  canvas.height = Math.max(1, Math.floor(cssHeight * dpr));

  const ctx = canvas.getContext("2d");
  const cellWidthPx = ctx ? measureCellWidth(ctx) : 8;
  const contentWidth = Math.max(1, cssWidth - PADDING_PX * 2);
  const contentHeight = Math.max(1, cssHeight - PADDING_PX * 2);

  return {
    metrics: {
      cellWidthPx: Math.ceil(cellWidthPx),
      rowHeightPx: LINE_HEIGHT_PX,
    },
    viewport: {
      firstRow: 0,
      rowCount: Math.max(1, Math.floor(contentHeight / LINE_HEIGHT_PX)),
      firstCol: 0,
      wrapWidth: Math.max(1, Math.floor(contentWidth / cellWidthPx)),
    },
  };
}

export function paintTextSurface(
  canvas: HTMLCanvasElement,
  frame: RenderFrame,
  decorations: BufferDecoration[] = [],
  themeSnapshot?: ThemeSnapshot,
  syntaxLegend: Record<number, string> = {},
): void {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  const dpr = window.devicePixelRatio || 1;
  const cssWidth = canvas.width / dpr;
  const cssHeight = canvas.height / dpr;
  const metrics = rendererMetrics(ctx);
  const theme = resolveTextSurfaceTheme(themeSnapshot?.active, syntaxLegend);

  prepareContext(ctx, dpr);
  ctx.fillStyle = theme.background;
  ctx.fillRect(0, 0, cssWidth, cssHeight);

  const rows = new Map<number, RenderRow>();
  for (const row of frame.rows) rows.set(row.surfaceRow, row);

  paintSelections(ctx, frame, metrics, theme);
  paintDecorationRanges(ctx, decorations, metrics);
  paintRows(ctx, frame, rows, metrics, theme);
  paintInlineTextDecorations(ctx, decorations, metrics, theme);
  paintDecorationUnderlines(ctx, decorations, metrics);
  paintCursors(ctx, frame, rows, metrics, theme);
}

function prepareContext(ctx: CanvasRenderingContext2D, dpr: number): void {
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.imageSmoothingEnabled = false;
  ctx.font = FONT;
  ctx.textAlign = "left";
  ctx.textBaseline = "alphabetic";
  ctx.fontKerning = "normal";
}

interface RendererMetrics {
  cellWidth: number;
  rowHeight: number;
  baselineOffset: number;
}

function rendererMetrics(ctx: CanvasRenderingContext2D): RendererMetrics {
  return {
    cellWidth: measureCellWidth(ctx),
    rowHeight: LINE_HEIGHT_PX,
    baselineOffset: Math.round((LINE_HEIGHT_PX - FONT_SIZE_PX) / 2 + FONT_SIZE_PX * 0.82),
  };
}

function measureCellWidth(ctx: CanvasRenderingContext2D): number {
  ctx.font = FONT;
  ctx.fontKerning = "normal";
  const sample = "mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm";
  return ctx.measureText(sample).width / sample.length;
}

function paintSelections(ctx: CanvasRenderingContext2D, frame: RenderFrame, metrics: RendererMetrics, theme: TextSurfaceTheme): void {
  const spans = frame.selections;
  ctx.fillStyle = theme.selectionBackground;
  for (const span of spans) {
    const x = xForCol(span.startCol, metrics);
    const y = yForRow(span.surfaceRow, metrics);
    const width = Math.max(metrics.cellWidth, (span.endCol - span.startCol) * metrics.cellWidth);
    ctx.fillRect(x, y, width, metrics.rowHeight);
  }
}

function paintDecorationRanges(
  ctx: CanvasRenderingContext2D,
  decorations: BufferDecoration[],
  metrics: RendererMetrics,
): void {
  for (const decoration of decorations) {
    if (decoration.kind !== "range" || !decoration.background) continue;
    paintRangeDecoration(ctx, decoration, metrics, decoration.background);
  }
}

function paintRows(
  ctx: CanvasRenderingContext2D,
  frame: RenderFrame,
  rows: Map<number, RenderRow>,
  metrics: RendererMetrics,
  theme: TextSurfaceTheme,
): void {
  const syntaxByRow = syntaxSpansByRow(frame.syntax);
  for (const row of rows.values()) {
    const cells = cellsForText(row.text);
    const spans = syntaxByRow.get(row.surfaceRow) ?? [];
    if (spans.length === 0) {
      drawCells(ctx, cells, 0, cells.length, row.surfaceRow, theme.foreground, metrics);
      continue;
    }

    let col = 0;
    for (const span of spans) {
      if (span.startCol > col) {
        drawCells(ctx, cells, col, span.startCol, row.surfaceRow, theme.foreground, metrics);
      }
      drawCells(
        ctx,
        cells,
        span.startCol,
        span.endCol,
        row.surfaceRow,
        theme.syntaxColor(span.kind),
        metrics,
      );
      col = Math.max(col, span.endCol);
    }
    if (col < cells.length) {
      drawCells(ctx, cells, col, cells.length, row.surfaceRow, theme.foreground, metrics);
    }
  }
}

function paintInlineTextDecorations(
  ctx: CanvasRenderingContext2D,
  decorations: BufferDecoration[],
  metrics: RendererMetrics,
  theme: TextSurfaceTheme,
): void {
  for (const decoration of decorations) {
    if (decoration.kind !== "inlineText") continue;
    paintInlineTextDecoration(ctx, decoration, metrics, theme);
  }
}

function paintDecorationUnderlines(
  ctx: CanvasRenderingContext2D,
  decorations: BufferDecoration[],
  metrics: RendererMetrics,
): void {
  for (const decoration of decorations) {
    if (decoration.kind !== "range" || !decoration.underline) continue;
    const y = yForRow(decoration.surfaceRow, metrics) + metrics.rowHeight - 2;
    ctx.fillStyle = decoration.underline;
    ctx.fillRect(
      xForCol(decoration.startCol, metrics),
      y,
      Math.max(metrics.cellWidth, (decoration.endCol - decoration.startCol) * metrics.cellWidth),
      1,
    );
  }
}

function paintCursors(
  ctx: CanvasRenderingContext2D,
  frame: RenderFrame,
  rows: Map<number, RenderRow>,
  metrics: RendererMetrics,
  theme: TextSurfaceTheme,
): void {
  for (const cursor of frame.cursors) {
    if (cursor.semantics === "InsertionPoint") paintBarCursor(ctx, cursor, metrics, theme);
    else paintBlockCursor(ctx, cursor, rows.get(cursor.surfaceRow), metrics, theme);
  }
}

function paintBarCursor(ctx: CanvasRenderingContext2D, cursor: CursorPlacement, metrics: RendererMetrics, theme: TextSurfaceTheme): void {
  const width = 2;
  ctx.fillStyle = theme.cursorBackground;
  ctx.fillRect(Math.round(xForCol(cursor.col, metrics)), yForRow(cursor.surfaceRow, metrics), width, metrics.rowHeight);
}

function paintBlockCursor(
  ctx: CanvasRenderingContext2D,
  cursor: CursorPlacement,
  row: RenderRow | undefined,
  metrics: RendererMetrics,
  theme: TextSurfaceTheme,
): void {
  const cells = cellsForText(row?.text ?? "");
  const cell = cells[cursor.col] ?? " ";
  const x = xForCol(cursor.col, metrics);
  const y = yForRow(cursor.surfaceRow, metrics);

  ctx.fillStyle = theme.cursorBackground;
  ctx.fillRect(x, y, metrics.cellWidth, metrics.rowHeight);
  if (cell !== "\t") {
    ctx.fillStyle = theme.cursorForeground;
    ctx.fillText(cell, x, y + metrics.baselineOffset);
  }
}

function paintRangeDecoration(
  ctx: CanvasRenderingContext2D,
  decoration: RangeDecoration,
  metrics: RendererMetrics,
  color: string,
): void {
  ctx.fillStyle = color;
  ctx.fillRect(
    xForCol(decoration.startCol, metrics),
    yForRow(decoration.surfaceRow, metrics),
    Math.max(metrics.cellWidth, (decoration.endCol - decoration.startCol) * metrics.cellWidth),
    metrics.rowHeight,
  );
}

function paintInlineTextDecoration(
  ctx: CanvasRenderingContext2D,
  decoration: InlineTextDecoration,
  metrics: RendererMetrics,
  theme: TextSurfaceTheme,
): void {
  ctx.save();
  ctx.globalAlpha = decoration.opacity ?? 0.65;
  ctx.fillStyle = decoration.foreground ?? resolveInlineDecorationColor(theme);
  ctx.fillText(decoration.text, xForCol(decoration.col, metrics), yForRow(decoration.surfaceRow, metrics) + metrics.baselineOffset);
  ctx.restore();
}

function drawCells(
  ctx: CanvasRenderingContext2D,
  cells: string[],
  startCol: number,
  endCol: number,
  row: number,
  color: string,
  metrics: RendererMetrics,
): void {
  const text = textForCols(cells, startCol, endCol);
  if (!text) return;
  ctx.fillStyle = color;
  ctx.fillText(text, xForCol(startCol, metrics), yForRow(row, metrics) + metrics.baselineOffset);
}

function textForCols(cells: string[], startCol: number, endCol: number): string {
  return cells
    .slice(startCol, Math.min(endCol, cells.length))
    .filter((cell) => cell !== "\t")
    .join("");
}

function cellsForText(text: string): string[] {
  const cells: string[] = [];
  const graphemes = SEGMENTER
    ? Array.from(SEGMENTER.segment(text), (segment) => segment.segment)
    : Array.from(text);
  for (const grapheme of graphemes) {
    if (grapheme === "\t") {
      cells.push("\t", " ", " ", " ");
    } else {
      cells.push(grapheme);
    }
  }
  return cells;
}

function resolveTextSurfaceTheme(
  activeTheme: ResolvedThemeDefinition | undefined,
  syntaxLegend: Record<number, string>,
): TextSurfaceTheme {
  const background = resolveThemeStyle(activeTheme ?? { styles: {} }, "ui.background")?.bg ?? "#101010";
  const foreground = resolveThemeStyle(activeTheme ?? { styles: {} }, "ui.text")?.fg ?? "#d8d8d8";
  const selectionBackground = resolveThemeStyle(activeTheme ?? { styles: {} }, "ui.selection")?.bg ?? "#264f78";
  const cursorStyle = resolveThemeStyle(activeTheme ?? { styles: {} }, "ui.cursor.insert", ["ui.cursor.active.insert", "ui.cursor"]);
  const cursorBackground = cursorStyle?.bg ?? cursorStyle?.fg ?? "#e6e6e6";
  const cursorForeground = cursorStyle?.fg ?? background;

  return {
    background,
    foreground,
    selectionBackground,
    cursorBackground,
    cursorForeground,
    syntaxColor(kind) {
      const scope = syntaxLegend[kind] ?? fallbackSyntaxScopes[kind];
      if (!scope) return foreground;
      return resolveThemeStyle(activeTheme ?? { styles: {} }, scope, "ui.text")?.fg ?? foreground;
    },
  };
}

function resolveInlineDecorationColor(theme: TextSurfaceTheme): string {
  return theme.foreground;
}

function syntaxSpansByRow(spans: SyntaxSpan[]): Map<number, SyntaxSpan[]> {
  const byRow = new Map<number, SyntaxSpan[]>();
  for (const span of spans) {
    const row = byRow.get(span.surfaceRow);
    if (row) row.push(span);
    else byRow.set(span.surfaceRow, [span]);
  }
  for (const row of byRow.values()) row.sort((a, b) => a.startCol - b.startCol);
  return byRow;
}

function xForCol(col: number, metrics: RendererMetrics): number {
  return PADDING_PX + col * metrics.cellWidth;
}

function yForRow(row: number, metrics: RendererMetrics): number {
  return PADDING_PX + row * metrics.rowHeight;
}
