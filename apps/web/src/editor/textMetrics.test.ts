/**
 * The measured grapheme-width oracle: real font advances classified into
 * grid cells (what the column math uses) and off-grid "fractional" glyphs
 * (what the row renderer pins in the DOM).
 */

import { describe, expect, test } from "vitest";
import { graphemeMetricsFrom } from "./textMetrics";

// A fake font: cell 8px; em dash exactly 2 cells (Iosevka); CJK from a
// fallback face at 1.7 cells; a zero-width joiner leftover at ~0.
const advances: Record<string, number> = {
  "0": 8,
  "—": 16,
  "→": 16,
  "世": 13.6,
  "\u200d": 0,
};
const measure = (text: string) => advances[text] ?? text.length * 8;

describe("graphemeMetricsFrom", () => {
  const metrics = graphemeMetricsFrom(measure)!;

  test("plain ASCII is one cell and never fractional", () => {
    expect(metrics.width("a")).toBe(1);
    expect(metrics.fractional("a")).toBe(false);
  });

  test("an exactly-two-cell glyph (Iosevka em dash) is wide but on-grid", () => {
    expect(metrics.width("—")).toBe(2);
    expect(metrics.fractional("—")).toBe(false);
    expect(metrics.width("→")).toBe(2);
  });

  test("an off-grid fallback glyph rounds to its nearest cells and pins", () => {
    expect(metrics.width("世")).toBe(2); // 1.7 → 2
    expect(metrics.fractional("世")).toBe(true);
  });

  test("zero-advance clusters stay zero width", () => {
    expect(metrics.width("\u200d")).toBe(0);
  });

  test("a broken measure yields no oracle", () => {
    expect(graphemeMetricsFrom(() => 0)).toBeNull();
  });
});
