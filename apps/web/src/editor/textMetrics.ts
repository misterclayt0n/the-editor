/**
 * Measured grapheme grid widths — the renderer-side width oracle the engine
 * deliberately does not provide (columns.ts: "This is not font measurement.
 * … That belongs in a renderer/host layer.").
 *
 * The editor's geometry is an integer grid (columns × charWidth), but the
 * DOM renders REAL font advances: the user's Iosevka draws an em dash, an
 * arrow, or an ellipsis TWO cells wide while the engine's static table says
 * one — so every rect, caret, and hit test right of such a glyph drifted a
 * column per glyph. The oracle measures each grapheme cluster once against
 * the actual buffer font (canvas measureText shares the browser's shaping
 * and fallback) and answers in grid cells:
 *
 * - `width`: rounded cells — what the column math uses everywhere.
 * - `fractional`: the natural advance is NOT an integer cell multiple
 *   (a fallback-font CJK glyph at 1.7ch); the row renderer pins those to
 *   their grid width in the DOM so geometry and pixels cannot disagree.
 *
 * ASCII never touches the canvas (monospace premise), so code-only rows
 * cost nothing.
 */

import type { GraphemeWidthFn } from "@the-editor/engine";

export type GraphemeMetrics = {
  /** Grid cells for one grapheme cluster (measured, rounded). */
  readonly width: GraphemeWidthFn;
  /** True when the natural advance is off-grid and needs DOM pinning. */
  readonly fractional: (grapheme: string) => boolean;
};

const isPlainAscii = (grapheme: string): boolean => {
  if (grapheme.length !== 1) return false;
  const code = grapheme.charCodeAt(0);
  return code >= 0x20 && code <= 0x7e;
};

/** Build metrics from any string→px measure (the canvas one in production;
 * tests inject a fake). Returns null when the measure can't size a cell. */
export function graphemeMetricsFrom(
  measure: (text: string) => number,
): GraphemeMetrics | null {
  const cell = measure("0");
  if (!(cell > 0)) return null;
  const cache = new Map<string, { readonly cells: number; readonly fractional: boolean }>();
  const entry = (grapheme: string) => {
    let cached = cache.get(grapheme);
    if (cached === undefined) {
      const ratio = measure(grapheme) / cell;
      // Zero-width (combining marks, ZWJ leftovers) stays zero; anything
      // visible occupies at least one cell; sub-pixel noise snaps to grid.
      const cells = ratio <= 0.1 ? 0 : Math.max(1, Math.round(ratio));
      cached = { cells, fractional: Math.abs(ratio - cells) > 0.05 };
      cache.set(grapheme, cached);
    }
    return cached;
  };
  return {
    width: (grapheme) => (isPlainAscii(grapheme) ? 1 : entry(grapheme).cells),
    fractional: (grapheme) => (isPlainAscii(grapheme) ? false : entry(grapheme).fractional),
  };
}

/** The production oracle: canvas measureText with the buffer font. Null in
 * environments without canvas (tests, SSR) — callers fall back to the
 * engine's static widths. */
export function createGraphemeMetrics(font: string): GraphemeMetrics | null {
  if (typeof document === "undefined") return null;
  const context = document.createElement("canvas").getContext("2d");
  if (context === null) return null;
  context.font = font;
  if (context.font === "") return null;
  return graphemeMetricsFrom((text) => context.measureText(text).width);
}
