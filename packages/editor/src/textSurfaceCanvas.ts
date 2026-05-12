import type { CellGridMetrics, CursorPlacement, RenderFrame, RenderRow, SyntaxSpan, Viewport } from "@the-editor/contracts/runtime";
import type { BufferDecoration, InlineTextDecoration, RangeDecoration } from "./decorations.js";
import type { GutterSnapshot } from "./gutters.js";
import type { ThemeSnapshot } from "./themes.js";
import { resolveThemeStyle, resolveThemeStyleLoose, type ResolvedThemeDefinition } from "./themes.js";

export interface CanvasTextSurfaceConfiguration {
  viewport: Viewport;
  metrics: CellGridMetrics;
}

export interface CanvasTextSurfaceStyleOptions {
  paddingPx?: number;
  fontSizePx?: number;
  lineHeightPx?: number;
  fontFamily?: string;
  gutterReservePx?: number;
}

export interface CanvasTextSurfacePaintOptions {
  selectionPickerActive?: boolean;
  showCursors?: boolean;
  offsetXPx?: number;
  offsetYPx?: number;
  gutters?: GutterSnapshot[];
}

interface TextSurfaceTheme {
  background: string;
  foreground: string;
  cursorBackground: string;
  cursorForeground: string;
  selectionBackground: string;
  gutterForeground: string;
  syntaxColor(kind: number): string;
}

const DEFAULT_TEXT_SURFACE_STYLE = {
  paddingPx: 18,
  fontSizePx: 15,
  lineHeightPx: 22,
  fontFamily: "Iosevka, Menlo, Monaco, Consolas, 'SF Mono', ui-monospace, monospace",
  gutterReservePx: 56,
} satisfies Required<CanvasTextSurfaceStyleOptions>;
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

export async function loadEditorFont(styleOptions: CanvasTextSurfaceStyleOptions = {}): Promise<void> {
  if (!("fonts" in document)) return;
  const style = resolveTextSurfaceStyle(styleOptions);
  await document.fonts.load(fontForStyle(style), "mmmmmmmmmmmmmmmm");
  await document.fonts.ready;
}

export function configureCanvasTextSurface(
  canvas: HTMLCanvasElement,
  styleOptions: CanvasTextSurfaceStyleOptions = {},
): CanvasTextSurfaceConfiguration {
  const rect = (canvas.parentElement ?? canvas).getBoundingClientRect();
  const cssWidth = Math.max(1, Math.floor(rect.width));
  const cssHeight = Math.max(1, Math.floor(rect.height));
  const dpr = window.devicePixelRatio || 1;

  canvas.style.width = `${cssWidth}px`;
  canvas.style.height = `${cssHeight}px`;
  canvas.width = Math.max(1, Math.floor(cssWidth * dpr));
  canvas.height = Math.max(1, Math.floor(cssHeight * dpr));

  const ctx = canvas.getContext("2d");
  const style = resolveTextSurfaceStyle(styleOptions);
  const cellWidthPx = ctx ? measureCellWidth(ctx, style) : 8;
  const contentWidth = Math.max(1, cssWidth - style.paddingPx * 2 - (style.gutterReservePx ?? 56));
  const contentHeight = Math.max(1, cssHeight - style.paddingPx * 2);

  return {
    metrics: {
      cellWidthPx: Math.ceil(cellWidthPx),
      rowHeightPx: style.lineHeightPx,
    },
    viewport: {
      firstRow: 0,
      rowCount: Math.max(1, Math.floor(contentHeight / style.lineHeightPx)),
      firstCol: 0,
      wrapWidth: Math.max(1, Math.floor(contentWidth / cellWidthPx)),
    },
  };
}

export function clearCanvasTextSurface(
  canvas: HTMLCanvasElement,
  themeSnapshot?: ThemeSnapshot,
  styleOptions: CanvasTextSurfaceStyleOptions = {},
): void {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  const dpr = window.devicePixelRatio || 1;
  const cssWidth = canvas.width / dpr;
  const cssHeight = canvas.height / dpr;
  const style = resolveTextSurfaceStyle(styleOptions);
  const theme = resolveTextSurfaceTheme(themeSnapshot?.active, {});

  prepareContext(ctx, dpr, style);
  ctx.fillStyle = theme.background;
  ctx.fillRect(0, 0, cssWidth, cssHeight);
}

export function paintCanvasTextSurface(
  canvas: HTMLCanvasElement,
  frame: RenderFrame,
  decorations: BufferDecoration[] = [],
  themeSnapshot?: ThemeSnapshot,
  syntaxLegend: Record<number, string> = {},
  options: CanvasTextSurfacePaintOptions = {},
  styleOptions: CanvasTextSurfaceStyleOptions = {},
): void {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  const dpr = window.devicePixelRatio || 1;
  const cssWidth = canvas.width / dpr;
  const cssHeight = canvas.height / dpr;
  const style = resolveTextSurfaceStyle(styleOptions);
  const metrics = rendererMetrics(ctx, style, options);
  const theme = resolveTextSurfaceTheme(themeSnapshot?.active, syntaxLegend);

  prepareContext(ctx, dpr, style);
  ctx.fillStyle = theme.background;
  ctx.fillRect(0, 0, cssWidth, cssHeight);

  const rows = new Map<number, RenderRow>();
  for (const row of frame.rows) rows.set(row.surfaceRow, row);

  paintGutters(ctx, options.gutters ?? [], metrics, theme);
  paintSelections(ctx, frame, metrics, theme);
  paintDecorationRanges(ctx, decorations, metrics);
  paintRows(ctx, frame, rows, metrics, theme);
  paintInlineTextDecorations(ctx, decorations, metrics, theme);
  paintDecorationUnderlines(ctx, decorations, metrics);
  if (options.showCursors ?? true) {
    paintCursors(ctx, frame, rows, metrics, theme, options.selectionPickerActive ?? false);
  }
}

function prepareContext(
  ctx: CanvasRenderingContext2D,
  dpr: number,
  style: Required<CanvasTextSurfaceStyleOptions>,
): void {
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.imageSmoothingEnabled = false;
  ctx.font = fontForStyle(style);
  ctx.textAlign = "left";
  ctx.textBaseline = "alphabetic";
  ctx.fontKerning = "normal";
}

interface RendererMetrics {
  cellWidth: number;
  rowHeight: number;
  baselineOffset: number;
  padding: number;
  offsetX: number;
  offsetY: number;
  gutterWidth: number;
}

function rendererMetrics(
  ctx: CanvasRenderingContext2D,
  style: Required<CanvasTextSurfaceStyleOptions>,
  options: CanvasTextSurfacePaintOptions = {},
): RendererMetrics {
  return {
    cellWidth: measureCellWidth(ctx, style),
    rowHeight: style.lineHeightPx,
    baselineOffset: Math.round((style.lineHeightPx - style.fontSizePx) / 2 + style.fontSizePx * 0.82),
    padding: style.paddingPx,
    offsetX: options.offsetXPx ?? 0,
    offsetY: options.offsetYPx ?? 0,
    gutterWidth: gutterWidthPx(options.gutters ?? [], measureCellWidth(ctx, style)),
  };
}

function measureCellWidth(ctx: CanvasRenderingContext2D, style: Required<CanvasTextSurfaceStyleOptions>): number {
  ctx.font = fontForStyle(style);
  ctx.fontKerning = "normal";
  const sample = "mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm";
  return ctx.measureText(sample).width / sample.length;
}

function paintGutters(
  ctx: CanvasRenderingContext2D,
  gutters: GutterSnapshot[],
  metrics: RendererMetrics,
  theme: TextSurfaceTheme,
): void {
  const lanes = gutterLaneGroups(gutters);
  let x = metrics.padding;
  for (const lane of lanes) {
    const laneWidth = gutterLaneGroupWidthPx(lane, metrics.cellWidth);
    const laneSpec = lane[0]!.lane;
    const contentX = x + laneSpec.paddingLeftPx;
    const contentWidth = Math.max(0, laneWidth - laneSpec.paddingLeftPx - laneSpec.paddingRightPx);
    for (const gutter of lane) {
      for (const line of gutter.lines) {
        const y = yForRow(line.surfaceRow, { ...metrics, gutterWidth: 0 });
        if (line.background) {
          ctx.fillStyle = line.background;
          ctx.fillRect(x, y, laneWidth, metrics.rowHeight);
        }
        const endRow = line.surfaceRowEnd ?? line.surfaceRow;
        const height = Math.max(metrics.rowHeight, (endRow - line.surfaceRow + 1) * metrics.rowHeight);
        if (line.kind === "bar") {
          ctx.save();
          ctx.globalAlpha = line.opacity ?? 0.9;
          ctx.fillStyle = line.color ?? line.foreground ?? theme.gutterForeground;
          const barWidth = Math.min(3, Math.max(2, contentWidth || laneWidth));
          ctx.fillRect(contentX, y + 2, barWidth, Math.max(2, height - 4));
          ctx.restore();
          continue;
        }
        if (line.kind === "marker") {
          ctx.save();
          ctx.globalAlpha = line.opacity ?? 0.9;
          ctx.fillStyle = line.color ?? line.foreground ?? theme.gutterForeground;
          const radius = Math.min(4, metrics.rowHeight / 4);
          ctx.beginPath();
          ctx.arc(contentX + contentWidth / 2, y + metrics.rowHeight / 2, radius, 0, Math.PI * 2);
          ctx.fill();
          ctx.restore();
          continue;
        }
        const text = line.text ?? line.icon;
        if (!text) continue;
        ctx.save();
        ctx.globalAlpha = line.opacity ?? 0.6;
        ctx.fillStyle = line.foreground ?? line.color ?? theme.gutterForeground;
        const align = line.align ?? "right";
        ctx.textAlign = align;
        const textX = align === "left" ? contentX : align === "center" ? contentX + contentWidth / 2 : contentX + contentWidth;
        ctx.fillText(text, textX, y + metrics.baselineOffset);
        ctx.restore();
      }
    }
    x += laneWidth;
  }
  ctx.textAlign = "left";
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
  selectionPickerActive: boolean,
): void {
  for (const cursor of frame.cursors) {
    ctx.save();
    if (selectionPickerActive && !cursor.active) ctx.globalAlpha = 0.25;
    if (cursor.semantics === "InsertionPoint") paintBarCursor(ctx, cursor, metrics, theme);
    else paintBlockCursor(ctx, cursor, rows.get(cursor.surfaceRow), metrics, theme);
    ctx.restore();
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

export function measureTextSurfaceColumns(text: string): number {
  return cellsForText(text).length;
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
  const gutterForeground = resolveThemeStyle(activeTheme ?? { styles: {} }, "ui.gutter", ["ui.text"])?.fg ?? "rgba(220, 220, 220, 0.58)";
  const cursorStyle = resolveThemeStyle(activeTheme ?? { styles: {} }, "ui.cursor", ["ui.cursor.insert", "ui.cursor.normal"]);
  const cursorBackground = cursorStyle?.bg ?? cursorStyle?.fg ?? "#e6e6e6";
  const cursorForeground = cursorStyle?.fg ?? background;

  return {
    background,
    foreground,
    selectionBackground,
    gutterForeground,
    cursorBackground,
    cursorForeground,
    syntaxColor(kind) {
      const scope = syntaxLegend[kind] ?? fallbackSyntaxScopes[kind];
      if (!scope) return foreground;
      return resolveThemeStyleLoose(activeTheme ?? { styles: {} }, scope, "ui.text")?.fg ?? foreground;
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

function resolveTextSurfaceStyle(styleOptions: CanvasTextSurfaceStyleOptions): Required<CanvasTextSurfaceStyleOptions> {
  return {
    paddingPx: styleOptions.paddingPx ?? DEFAULT_TEXT_SURFACE_STYLE.paddingPx,
    fontSizePx: styleOptions.fontSizePx ?? DEFAULT_TEXT_SURFACE_STYLE.fontSizePx,
    lineHeightPx: styleOptions.lineHeightPx ?? DEFAULT_TEXT_SURFACE_STYLE.lineHeightPx,
    fontFamily: styleOptions.fontFamily ?? DEFAULT_TEXT_SURFACE_STYLE.fontFamily,
    gutterReservePx: styleOptions.gutterReservePx ?? DEFAULT_TEXT_SURFACE_STYLE.gutterReservePx,
  };
}

function fontForStyle(style: Required<CanvasTextSurfaceStyleOptions>): string {
  return `${style.fontSizePx}px ${style.fontFamily}`;
}

function gutterWidthPx(gutters: GutterSnapshot[], cellWidth: number): number {
  return gutterLaneGroups(gutters)
    .reduce((sum, lane) => sum + gutterLaneGroupWidthPx(lane, cellWidth), 0);
}

function gutterLaneGroups(gutters: GutterSnapshot[]): GutterSnapshot[][] {
  const groups = new Map<string, GutterSnapshot[]>();
  for (const gutter of gutters) {
    if (gutter.lane.side !== "left") continue;
    const group = groups.get(gutter.lane.id);
    if (group) group.push(gutter);
    else groups.set(gutter.lane.id, [gutter]);
  }
  return [...groups.values()].sort((a, b) => {
    const left = a[0]!.lane;
    const right = b[0]!.lane;
    return left.order - right.order || left.id.localeCompare(right.id);
  });
}

function gutterLaneGroupWidthPx(lane: GutterSnapshot[], cellWidth: number): number {
  const spec = lane[0]!.lane;
  const contentWidth = Math.max(0, ...lane.map((gutter) => gutter.lane.widthPx ?? (gutter.lane.widthCells ?? 1) * cellWidth));
  return Math.ceil(contentWidth + spec.paddingLeftPx + spec.paddingRightPx);
}

function xForCol(col: number, metrics: RendererMetrics): number {
  return metrics.padding + metrics.gutterWidth - metrics.offsetX + col * metrics.cellWidth;
}

function yForRow(row: number, metrics: RendererMetrics): number {
  return metrics.padding - metrics.offsetY + row * metrics.rowHeight;
}
