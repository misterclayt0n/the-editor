import { describe, expect, test } from "vitest";
import {
  highlightSegments,
  jumpTarget,
  styleSegments,
  matchKey,
  previewLines,
  trimLeading,
  type GrepMatchLike,
} from "./grepResults";

const match = (over: Partial<GrepMatchLike>): GrepMatchLike => ({
  relativePath: "src/a.ts",
  lineNumber: 10,
  byteOffset: 1234,
  lineContent: "  import { x } from 'y'",
  matchRanges: [[9, 15]],
  ...over,
});

describe("matchKey", () => {
  test("keys per occurrence by byte offset", () => {
    expect(matchKey(match({}))).toBe("src/a.ts#1234");
    expect(matchKey(match({ byteOffset: 5 }))).toBe("src/a.ts#5");
  });
});

describe("jumpTarget", () => {
  test("0-based line, caret at first match start", () => {
    expect(jumpTarget({ lineNumber: 10, matchRanges: [[9, 15]] })).toEqual({
      line: 9,
      character: 9,
    });
  });
  test("falls back to column 0 with no ranges", () => {
    expect(jumpTarget({ lineNumber: 1, matchRanges: [] })).toEqual({ line: 0, character: 0 });
  });
});

describe("highlightSegments", () => {
  test("splits around a single match", () => {
    expect(highlightSegments("abcdef", [[2, 4]])).toEqual([
      { text: "ab", match: false },
      { text: "cd", match: true },
      { text: "ef", match: false },
    ]);
  });
  test("no ranges yields one plain segment", () => {
    expect(highlightSegments("abc", [])).toEqual([{ text: "abc", match: false }]);
  });
  test("empty text yields nothing", () => {
    expect(highlightSegments("", [])).toEqual([]);
  });
  test("clamps out-of-bounds ranges and merges order", () => {
    expect(highlightSegments("abc", [[1, 99]])).toEqual([
      { text: "a", match: false },
      { text: "bc", match: true },
    ]);
  });
  test("handles multiple ranges in any order", () => {
    expect(
      highlightSegments("0123456789", [
        [6, 8],
        [1, 3],
      ]),
    ).toEqual([
      { text: "0", match: false },
      { text: "12", match: true },
      { text: "345", match: false },
      { text: "67", match: true },
      { text: "89", match: false },
    ]);
  });
});

describe("styleSegments", () => {
  test("keeps syntax colors while overlaying matches", () => {
    expect(
      styleSegments("export type", [[0, 6]], [
        { text: "export", color: "purple" },
        { text: " type", color: "blue" },
      ]),
    ).toEqual([
      { text: "export", match: true, color: "purple" },
      { text: " type", match: false, color: "blue" },
    ]);
  });

  test("splits a syntax span when a match starts inside it", () => {
    expect(styleSegments("abcdef", [[2, 4]], [{ text: "abcdef", color: "green" }])).toEqual([
      { text: "ab", match: false, color: "green" },
      { text: "cd", match: true, color: "green" },
      { text: "ef", match: false, color: "green" },
    ]);
  });
});

describe("trimLeading", () => {
  test("shifts ranges by the trimmed whitespace", () => {
    const { text, ranges } = trimLeading("  import x", [[2, 8]]);
    expect(text).toBe("import x");
    expect(ranges).toEqual([[0, 6]]);
  });
  test("no-op when there is no leading whitespace", () => {
    expect(trimLeading("import", [[0, 6]])).toEqual({ text: "import", ranges: [[0, 6]] });
  });
  test("drops ranges fully inside the trimmed prefix", () => {
    expect(trimLeading("    x", [[0, 2]]).ranges).toEqual([]);
  });
});

describe("previewLines", () => {
  test("numbers context lines around the match", () => {
    const lines = previewLines(
      match({
        lineNumber: 10,
        lineContent: "MATCH",
        matchRanges: [[0, 5]],
        contextBefore: ["b8", "b9"],
        contextAfter: ["a11"],
      }),
    );
    expect(lines).toEqual([
      { number: 8, text: "b8", match: false, ranges: [] },
      { number: 9, text: "b9", match: false, ranges: [] },
      { number: 10, text: "MATCH", match: true, ranges: [[0, 5]] },
      { number: 11, text: "a11", match: false, ranges: [] },
    ]);
  });
  test("works with no context", () => {
    const lines = previewLines(match({ lineNumber: 3, lineContent: "x", matchRanges: [[0, 1]] }));
    expect(lines).toEqual([{ number: 3, text: "x", match: true, ranges: [[0, 1]] }]);
  });
});
