/**
 * Pure span arithmetic for detached highlighting: slicing a highlighted
 * string's spans to a sub-range (signature help wraps the active parameter
 * around the colored spans, so the slice must preserve colors and cut
 * spans mid-way without drift).
 */

import { describe, expect, test } from "vitest";
import { runsToSpans, sliceCodeSpans, type CodeSpan } from "./highlightSpans";

// "printf(const char *fmt) -> int" colored in three runs.
const spans: CodeSpan[] = [
  { text: "printf", color: "#f00" },
  { text: "(" },
  { text: "const char *fmt", color: "#0f0" },
  { text: ") -> " },
  { text: "int", color: "#00f" },
];
const full = spans.map((s) => s.text).join("");

describe("sliceCodeSpans", () => {
  test("the full range reproduces the spans", () => {
    expect(sliceCodeSpans(spans, 0, full.length)).toEqual(spans);
  });

  test("a mid-span slice cuts text but keeps the color", () => {
    // "const char *fmt" occupies [7, 22); slice out "char *".
    const sliced = sliceCodeSpans(spans, 13, 19);
    expect(sliced).toEqual([{ text: "char *", color: "#0f0" }]);
  });

  test("a slice across span boundaries keeps each piece's color", () => {
    // "f(const" spans the end of "printf", "(" and the start of the type.
    expect(sliceCodeSpans(spans, 5, 12)).toEqual([
      { text: "f", color: "#f00" },
      { text: "(" },
      { text: "const", color: "#0f0" },
    ]);
  });

  test("empty and out-of-range slices are empty", () => {
    expect(sliceCodeSpans(spans, 4, 4)).toEqual([]);
    expect(sliceCodeSpans(spans, full.length, full.length + 5)).toEqual([]);
  });

  test("concatenating adjacent slices loses nothing", () => {
    const cut = 11;
    const joined = [...sliceCodeSpans(spans, 0, cut), ...sliceCodeSpans(spans, cut, full.length)]
      .map((s) => s.text)
      .join("");
    expect(joined).toBe(full);
  });
});

describe("runsToSpans", () => {
  test("covers gaps between runs with uncolored spans", () => {
    const text = "ab cd ef";
    const result = runsToSpans(
      text,
      [
        { from: 0, to: 2, kind: "keyword" },
        { from: 6, to: 8, kind: "type" },
      ],
      (kind) => (kind === "keyword" ? "#f00" : undefined),
    );
    expect(result).toEqual([
      { text: "ab", color: "#f00" },
      { text: " cd " },
      { text: "ef" },
    ]);
  });
});
