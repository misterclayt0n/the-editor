import { describe, expect, test } from "vitest";
import { fuzzyScore } from "./fuzzy";

const rank = (pattern: string, words: string[]): string[] =>
  words
    .map((word) => ({ word, score: fuzzyScore(pattern, word) }))
    .filter((entry) => entry.score !== null)
    .sort((a, b) => b.score! - a.score!)
    .map((entry) => entry.word);

describe("fuzzyScore", () => {
  test("non-subsequences are filtered out", () => {
    expect(fuzzyScore("xyz", "hello")).toBeNull();
    expect(fuzzyScore("ba", "ab")).toBeNull();
  });

  test("empty pattern matches everything neutrally", () => {
    expect(fuzzyScore("", "anything")).toBe(0);
  });

  test("exact prefix beats scattered subsequence", () => {
    expect(rank("for", ["performance", "forEach", "format"])).toEqual([
      "format",
      "forEach",
      "performance",
    ]);
  });

  test("case-exact prefix beats case-folded prefix", () => {
    const ranked = rank("For", ["format", "Format"]);
    expect(ranked[0]).toBe("Format");
  });

  test("camelCase initials match well", () => {
    expect(fuzzyScore("gcp", "getCompletionProvider")).not.toBeNull();
    const ranked = rank("gcp", ["getCompletionProvider", "garbageheapcompact"]);
    expect(ranked[0]).toBe("getCompletionProvider");
  });

  test("word boundaries via separators score above mid-word hits", () => {
    const ranked = rank("fb", ["foo_bar", "ofxxb"]);
    expect(ranked[0]).toBe("foo_bar");
  });

  test("shorter word wins on equal prefix", () => {
    expect(rank("ma", ["mapForever", "map"])[0]).toBe("map");
  });
});
