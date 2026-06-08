import { describe, expect, test } from "vitest";
import {
  graphemeAt,
  graphemeBefore,
  graphemeSegments,
  isGraphemeBoundary,
  nextGraphemeBoundary,
  nthNextGraphemeBoundary,
  nthPreviousGraphemeBoundary,
  previousGraphemeBoundary,
  snapToGraphemeBoundaryAfter,
  snapToGraphemeBoundaryBefore,
} from "../src/index";

describe("grapheme", () => {
  test("basic segmentation", () => {
    expect(graphemeSegments("abc")).toEqual([
      { from: 0, to: 1, text: "a" },
      { from: 1, to: 2, text: "b" },
      { from: 2, to: 3, text: "c" },
    ]);
  });

  test("combining marks", () => {
    const accented = "a\u0301b";
    expect(graphemeSegments(accented)).toEqual([
      { from: 0, to: 2, text: "a\u0301" },
      { from: 2, to: 3, text: "b" },
    ]);
    expect(isGraphemeBoundary(accented, 0)).toBe(true);
    expect(isGraphemeBoundary(accented, 1)).toBe(false);
    expect(isGraphemeBoundary(accented, 2)).toBe(true);
    expect(isGraphemeBoundary(accented, 3)).toBe(true);
    expect(snapToGraphemeBoundaryBefore(accented, 1)).toBe(0);
    expect(snapToGraphemeBoundaryAfter(accented, 1)).toBe(2);
    expect(nextGraphemeBoundary(accented, 0)).toBe(2);
    expect(previousGraphemeBoundary(accented, 3)).toBe(2);
  });

  test("emoji and surrogate pairs", () => {
    const text = "a😊b";
    expect(graphemeSegments(text)).toEqual([
      { from: 0, to: 1, text: "a" },
      { from: 1, to: 3, text: "😊" },
      { from: 3, to: 4, text: "b" },
    ]);
    expect(nextGraphemeBoundary(text, 1)).toBe(3);
    expect(nextGraphemeBoundary(text, 2)).toBe(3);
    expect(previousGraphemeBoundary(text, 3)).toBe(1);
    expect(previousGraphemeBoundary(text, 2)).toBe(1);
  });

  test("ZWJ emoji", () => {
    const familyOnly = "👨‍👩‍👧‍👦";
    const family = `${familyOnly}x`;
    expect(graphemeSegments(family)[0]).toEqual({
      from: 0,
      to: familyOnly.length,
      text: familyOnly,
    });
    expect(nextGraphemeBoundary(family, 0)).toBe(familyOnly.length);
  });

  test("CRLF", () => {
    const text = "x\r\ny";
    expect(graphemeSegments(text)).toEqual([
      { from: 0, to: 1, text: "x" },
      { from: 1, to: 3, text: "\r\n" },
      { from: 3, to: 4, text: "y" },
    ]);
    expect(nextGraphemeBoundary(text, 1)).toBe(3);
    expect(previousGraphemeBoundary(text, 4)).toBe(3);
    expect(nthPreviousGraphemeBoundary(text, 4, 2)).toBe(1);
  });

  test("nth helpers", () => {
    const text = "a😊b";
    expect(nthNextGraphemeBoundary(text, 0, 0)).toBe(0);
    expect(nthNextGraphemeBoundary(text, 0, 1)).toBe(1);
    expect(nthNextGraphemeBoundary(text, 0, 2)).toBe(3);
    expect(nthNextGraphemeBoundary(text, 0, 3)).toBe(4);
    expect(nthNextGraphemeBoundary(text, 0, 4)).toBe(4);
    expect(nthPreviousGraphemeBoundary(text, 4, 0)).toBe(4);
    expect(nthPreviousGraphemeBoundary(text, 4, 1)).toBe(3);
    expect(nthPreviousGraphemeBoundary(text, 4, 2)).toBe(1);
    expect(nthPreviousGraphemeBoundary(text, 4, 3)).toBe(0);
    expect(nthPreviousGraphemeBoundary(text, 4, 4)).toBe(0);
  });

  test("graphemeAt and graphemeBefore", () => {
    const text = "a😊b";
    expect(graphemeAt(text, 1)).toEqual({ from: 1, to: 3, text: "😊" });
    expect(graphemeAt(text, 2)).toEqual({ from: 1, to: 3, text: "😊" });
    expect(graphemeAt(text, 4)).toBeUndefined();
    expect(graphemeBefore(text, 3)).toEqual({ from: 1, to: 3, text: "😊" });
    expect(graphemeBefore(text, 2)).toEqual({ from: 1, to: 3, text: "😊" });
    expect(graphemeBefore(text, 0)).toBeUndefined();
  });

  test("invalid offsets and counts", () => {
    expect(() => isGraphemeBoundary("abc", -1)).toThrow(RangeError);
    expect(() => isGraphemeBoundary("abc", 4)).toThrow(RangeError);
    expect(() => isGraphemeBoundary("abc", 1.5)).toThrow(RangeError);
    expect(() => nthNextGraphemeBoundary("abc", 0, -1)).toThrow(RangeError);
    expect(() => nthPreviousGraphemeBoundary("abc", 0, 1.5)).toThrow(RangeError);
  });

  test("malformed UTF-16 covers full string", () => {
    for (const text of ["\ude0a", "\ud83d"]) {
      expect(() => graphemeSegments(text)).not.toThrow();
      const segments = graphemeSegments(text);
      expect(segments[0]?.from).toBe(0);
      expect(segments.map((s) => s.text).join("")).toBe(text);
      expect(segments.at(-1)?.to).toBe(text.length);
    }
  });
});
