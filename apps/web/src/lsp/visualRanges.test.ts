/**
 * Coordinate discipline for the LSP manager's cached visual ranges: the
 * mapping that keeps cached diagnostics/hints glued to their text through
 * edits, and the diff that turns a visuals push into a localized
 * invalidation (identical republish → nothing to repaint).
 */

import { describe, expect, test } from "vitest";
import { changedSpans, mapOffsetThrough, mapRangedValues, sortRanges } from "./visualRanges";

const insertAt = (at: number, length: number) => [
  { fromA: at, toA: at, fromB: at, toB: at + length },
];
const deleteRange = (from: number, to: number) => [{ fromA: from, toA: to, fromB: from, toB: from }];

describe("mapOffsetThrough", () => {
  test("insertion shifts positions at and after it", () => {
    const changes = insertAt(5, 3);
    expect(mapOffsetThrough(changes, 3)).toBe(3);
    expect(mapOffsetThrough(changes, 5)).toBe(8); // at the point: pushed right
    expect(mapOffsetThrough(changes, 7)).toBe(10);
  });

  test("deletion collapses positions inside it and shifts the rest", () => {
    const changes = deleteRange(2, 5);
    expect(mapOffsetThrough(changes, 1)).toBe(1);
    expect(mapOffsetThrough(changes, 3)).toBe(2); // inside → change start
    expect(mapOffsetThrough(changes, 6)).toBe(3);
  });
});

describe("mapRangedValues", () => {
  test("point ranges (inlay hints) stay points", () => {
    const mapped = mapRangedValues([{ from: 5, to: 5, value: "h" }], deleteRange(4, 6));
    expect(mapped).toEqual([{ from: 4, to: 4, value: "h" }]);
  });

  test("spans truncated by a delete never invert", () => {
    const mapped = mapRangedValues([{ from: 3, to: 8, value: "d" }], deleteRange(5, 10));
    expect(mapped).toEqual([{ from: 3, to: 5, value: "d" }]);
  });

  test("spans after an insert slide right whole", () => {
    const mapped = mapRangedValues([{ from: 6, to: 9, value: "d" }], insertAt(2, 4));
    expect(mapped).toEqual([{ from: 10, to: 13, value: "d" }]);
  });
});

describe("changedSpans", () => {
  const eq = (a: string, b: string) => a === b;

  test("identical lists produce no spans (republish is a no-op)", () => {
    const list = [
      { from: 1, to: 4, value: "a" },
      { from: 8, to: 8, value: "b" },
    ];
    expect(changedSpans(list, [...list], eq)).toEqual([]);
  });

  test("a changed value at the same range invalidates just that span", () => {
    const before = [
      { from: 1, to: 4, value: "a" },
      { from: 8, to: 8, value: "b" },
    ];
    const after = [
      { from: 1, to: 4, value: "a" },
      { from: 8, to: 8, value: "B" },
    ];
    expect(changedSpans(before, after, eq)).toEqual([{ from: 8, to: 8 }]);
  });

  test("additions and removals each contribute their span", () => {
    const before = [
      { from: 1, to: 4, value: "a" },
      { from: 10, to: 12, value: "c" },
    ];
    const after = [
      { from: 1, to: 4, value: "a" },
      { from: 6, to: 6, value: "b" },
    ];
    expect(changedSpans(before, after, eq)).toEqual([
      { from: 6, to: 6 }, // added
      { from: 10, to: 12 }, // removed
    ]);
  });

  test("sortRanges orders by (from, to) for the diff walk", () => {
    const ranges = sortRanges([
      { from: 5, to: 5, value: "b" },
      { from: 1, to: 3, value: "a" },
      { from: 5, to: 7, value: "c" },
    ]);
    expect(ranges.map((r) => r.from)).toEqual([1, 5, 5]);
    expect(ranges[1]!.to).toBe(5);
  });
});
