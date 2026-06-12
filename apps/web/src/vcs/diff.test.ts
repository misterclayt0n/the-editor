import { describe, expect, test } from "vitest";
import { diffHunks, diffLines } from "./diff";

describe("diffLines", () => {
  test("identical texts produce nothing", () => {
    const diff = diffLines("a\nb\nc", "a\nb\nc");
    expect(diff.lines.size).toBe(0);
    expect(diff.deletedAbove.size).toBe(0);
  });

  test("pure insertion marks added lines", () => {
    const diff = diffLines("a\nc", "a\nb1\nb2\nc");
    expect([...diff.lines.entries()]).toEqual([
      [1, "added"],
      [2, "added"],
    ]);
    expect(diff.deletedAbove.size).toBe(0);
  });

  test("replacement marks modified lines", () => {
    const diff = diffLines("a\nold\nc", "a\nnew\nc");
    expect([...diff.lines.entries()]).toEqual([[1, "modified"]]);
  });

  test("pure deletion marks the boundary line", () => {
    const diff = diffLines("a\ngone\nc", "a\nc");
    expect(diff.lines.size).toBe(0);
    expect([...diff.deletedAbove]).toEqual([1]);
  });

  test("deletion at end of file clamps to last line", () => {
    const diff = diffLines("a\nb\ngone", "a\nb");
    expect([...diff.deletedAbove]).toEqual([1]);
  });

  test("uneven replacement marks all new lines modified", () => {
    const diff = diffLines("a\nx\nz", "a\np\nq\nr\nz");
    expect([...diff.lines.values()]).toEqual(["modified", "modified", "modified"]);
  });

  test("disjoint hunks are independent", () => {
    const diff = diffLines("a\nb\nc\nd\ne", "a\nB\nc\nd\ne\nf");
    expect(diff.lines.get(1)).toBe("modified");
    expect(diff.lines.get(5)).toBe("added");
    expect(diff.lines.size).toBe(2);
  });

  test("everything-new file is all added", () => {
    const diff = diffLines("", "x\ny");
    expect([...diff.lines.values()]).toEqual(["added", "added"]);
  });
});

describe("diffHunks", () => {
  test("hunk boundaries are exact replacement ranges", () => {
    const hunks = diffHunks(["a", "b", "c"], ["a", "B1", "B2", "c"]);
    expect(hunks).toEqual([{ oldStart: 1, oldEnd: 2, newStart: 1, newEnd: 3 }]);
  });

  test("interleaved keeps split hunks", () => {
    const hunks = diffHunks(["k1", "x", "k2", "y", "k3"], ["k1", "X", "k2", "k3"]);
    expect(hunks).toEqual([
      { oldStart: 1, oldEnd: 2, newStart: 1, newEnd: 2 },
      { oldStart: 3, oldEnd: 4, newStart: 3, newEnd: 3 },
    ]);
  });
});
