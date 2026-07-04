/**
 * Soft-wrap break computation: the ONE function every consumer (geometry
 * row counts, layout row emission, hit testing) derives from — these tests
 * pin its word-boundary semantics.
 */

import { describe, expect, test } from "vitest";
import { wrapIndentColumns, wrapLineBreaks, wrapRowOfOffset, wrapSegmentBounds } from "../src/wrap";

const breaks = (lineText: string, wrapColumns: number) =>
  wrapLineBreaks({ lineText, wrapColumns, tabWidth: 4 });

describe("wrapLineBreaks", () => {
  test("a fitting line never wraps", () => {
    expect(breaks("hello world", 20)).toEqual([]);
    expect(breaks("", 10)).toEqual([]);
  });

  test("breaks at word boundaries; the space stays on the previous row", () => {
    // "aaa bbb ccc" at width 6: "aaa " | "bbb " | "ccc"
    expect(breaks("aaa bbb ccc", 6)).toEqual([4, 8]);
    // At width 7 the tail "bbb ccc" (7 cols) fits on one row.
    expect(breaks("aaa bbb ccc", 7)).toEqual([4]);
  });

  test("a word longer than the width hard-breaks mid-word", () => {
    expect(breaks("abcdefghij", 4)).toEqual([4, 8]);
  });

  test("a long word after a short one moves whole, then hard-breaks", () => {
    // "ab cdefgh" at width 5: "ab " | "cdefg" | "h"
    expect(breaks("ab cdefgh", 5)).toEqual([3, 8]);
  });

  test("every row keeps at least one grapheme even at width 1", () => {
    expect(breaks("abc", 1)).toEqual([1, 2]);
  });

  test("tabs count toward the width with per-row tab stops", () => {
    // "\ta" at width 4: the tab fills the row (4 cols), "a" wraps.
    expect(breaks("\ta", 4)).toEqual([1]);
  });

  test("wide glyphs (measured oracle) consume their cells", () => {
    const width = (g: string) => (g === "—" ? 2 : 1);
    // "a—b—c" at width 4 with 2-cell dashes: "a—b" (4 cols) | "—c".
    expect(
      wrapLineBreaks({ lineText: "a—b—c", wrapColumns: 4, tabWidth: 4, graphemeWidth: width }),
    ).toEqual([3]);
  });
});

describe("wrapRowOfOffset", () => {
  test("an offset AT a break belongs to the continuation row", () => {
    const b = [4, 8];
    expect(wrapRowOfOffset(b, 0)).toBe(0);
    expect(wrapRowOfOffset(b, 3)).toBe(0);
    expect(wrapRowOfOffset(b, 4)).toBe(1);
    expect(wrapRowOfOffset(b, 7)).toBe(1);
    expect(wrapRowOfOffset(b, 8)).toBe(2);
    expect(wrapRowOfOffset(b, 11)).toBe(2);
  });
});

describe("wrapSegmentBounds", () => {
  test("absolute bounds bracket every segment", () => {
    expect(wrapSegmentBounds(100, 111, [4, 8])).toEqual([100, 104, 108, 111]);
    expect(wrapSegmentBounds(5, 8, [])).toEqual([5, 8]);
  });
});

describe("hanging indent", () => {
  test("continuation rows reduce capacity by the line's indent", () => {
    // 4-space indent, width 10: "    aaaa " then "bbbb" hangs (cap 6).
    expect(breaks("    aaaa bbbb", 10)).toEqual([9]);
    // A long word: the indent-only first row, then 4-col rows.
    expect(breaks("    abcdefghij", 8)).toEqual([4, 8, 12]);
  });

  test("indent wider than half the width is ignored", () => {
    expect(wrapIndentColumns("      x", 4, 10)).toBe(0); // 6 > 10/2
    expect(wrapIndentColumns("    x", 4, 10)).toBe(4);
    expect(wrapIndentColumns("\tx", 4, 12)).toBe(4); // tab expands
  });
});

describe("inlay hints occupy wrap width", () => {
  test("a hint pushes the row over and wraps earlier", () => {
    expect(
      wrapLineBreaks({
        lineText: "abcdef",
        wrapColumns: 6,
        tabWidth: 4,
        lineFrom: 100,
        virtualText: [{ at: 103, text: ":hi" }],
      }),
    ).toEqual([3]);
    // Without the hint the same line fits.
    expect(wrapLineBreaks({ lineText: "abcdef", wrapColumns: 6, tabWidth: 4 })).toEqual([]);
  });

  test("hints outside the line are ignored", () => {
    expect(
      wrapLineBreaks({
        lineText: "abcdef",
        wrapColumns: 6,
        tabWidth: 4,
        lineFrom: 100,
        virtualText: [{ at: 50, text: "elsewhere" }],
      }),
    ).toEqual([]);
  });
});
