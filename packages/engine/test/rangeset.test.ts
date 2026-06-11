import { describe, expect, test } from "vitest";
import { Effect } from "effect";
import { RangeSet, Transaction } from "../src/index";

const changes = (length: number, replacements: Array<{ from: number; to: number; text: string }>) =>
  Effect.runSync(Transaction.fromReplacements(length, replacements)).changes.desc();

describe("RangeSet", () => {
  test("between returns sorted overlapping ranges (closed interval)", () => {
    const set = RangeSet.of([
      { from: 0, to: 5, value: "a" },
      { from: 10, to: 20, value: "b" },
      { from: 15, to: 16, value: "c" },
      { from: 30, to: 30, value: "point" },
    ]);
    expect(set.between(12, 14).map((r) => r.value)).toEqual(["b"]);
    expect(set.between(5, 10).map((r) => r.value)).toEqual(["a", "b"]);
    expect(set.between(30, 40).map((r) => r.value)).toEqual(["point"]);
    expect(set.between(21, 29)).toEqual([]);
    expect(set.size).toBe(4);
  });

  test("windowed queries scale past chunk boundaries", () => {
    const ranges = Array.from({ length: 1000 }, (_, i) => ({
      from: i * 10,
      to: i * 10 + 4,
      value: i,
    }));
    const set = RangeSet.of(ranges);
    const window = set.between(5000, 5050);
    expect(window.map((r) => r.value)).toEqual([500, 501, 502, 503, 504, 505]);
  });

  test("map shifts ranges after an edit and preserves earlier ones", () => {
    const set = RangeSet.of([
      { from: 10, to: 20, value: "before" },
      { from: 100, to: 110, value: "after" },
    ]);
    const mapped = set.map(changes(200, [{ from: 50, to: 50, text: "xxxxx" }]));
    expect(mapped.all()).toEqual([
      { from: 10, to: 20, value: "before" },
      { from: 105, to: 115, value: "after" },
    ]);
  });

  test("insertion at boundaries stays outside the range by default", () => {
    const set = RangeSet.of([{ from: 10, to: 20, value: "x" }]);
    const atStart = set.map(changes(100, [{ from: 10, to: 10, text: "ab" }]));
    expect(atStart.all()).toEqual([{ from: 12, to: 22, value: "x" }]);
    const atEnd = set.map(changes(100, [{ from: 20, to: 20, text: "ab" }]));
    expect(atEnd.all()).toEqual([{ from: 10, to: 20, value: "x" }]);
  });

  test("deletion spanning a range drops it unless keepCollapsed", () => {
    const set = RangeSet.of([{ from: 10, to: 20, value: "x" }]);
    const desc = changes(100, [{ from: 5, to: 25, text: "" }]);
    expect(set.map(desc).all()).toEqual([]);
    expect(set.map(desc, { keepCollapsed: true }).all()).toEqual([{ from: 5, to: 5, value: "x" }]);
  });

  test("partial deletion clips the range", () => {
    const set = RangeSet.of([{ from: 10, to: 20, value: "x" }]);
    const mapped = set.map(changes(100, [{ from: 15, to: 30, text: "" }]));
    expect(mapped.all()).toEqual([{ from: 10, to: 15, value: "x" }]);
  });

  test("points survive edits and respect pointAssoc", () => {
    const set = RangeSet.of([{ from: 10, to: 10, value: "pt" }]);
    const ins = changes(100, [{ from: 10, to: 10, text: "abc" }]);
    expect(set.map(ins).all()).toEqual([{ from: 10, to: 10, value: "pt" }]);
    expect(set.map(ins, { pointAssoc: "after" }).all()).toEqual([
      { from: 13, to: 13, value: "pt" },
    ]);
    const del = changes(100, [{ from: 5, to: 15, text: "" }]);
    expect(set.map(del).all()).toEqual([{ from: 5, to: 5, value: "pt" }]);
  });

  test("replacement inside a range keeps the range covering it", () => {
    const set = RangeSet.of([{ from: 10, to: 30, value: "x" }]);
    const mapped = set.map(changes(100, [{ from: 15, to: 20, text: "longer-text" }]));
    expect(mapped.all()).toEqual([{ from: 10, to: 36, value: "x" }]);
  });

  test("untouched chunks share storage across map (structural sharing)", () => {
    const ranges = Array.from({ length: 600 }, (_, i) => ({
      from: i * 10,
      to: i * 10 + 4,
      value: i,
    }));
    const set = RangeSet.of(ranges);
    // Edit near the very end: earlier chunks must be reused by identity.
    const mapped = set.map(changes(10_000, [{ from: 5990, to: 5990, text: "zz" }]));
    const before = (set as unknown as { chunks: Array<{ ranges: unknown }> }).chunks;
    const after = (mapped as unknown as { chunks: Array<{ ranges: unknown }> }).chunks;
    expect(after[0]!.ranges).toBe(before[0]!.ranges);
    expect(after[1]!.ranges).toBe(before[1]!.ranges);
    expect(mapped.size).toBe(600);
    // And positions after the edit shifted.
    expect(mapped.between(5990, 6010).map((r) => r.from)).toEqual([5992]);
  });

  test("map with no changes returns the same set", () => {
    const set = RangeSet.of([{ from: 1, to: 2, value: "x" }]);
    expect(set.map(changes(100, []))).toBe(set);
  });
});
