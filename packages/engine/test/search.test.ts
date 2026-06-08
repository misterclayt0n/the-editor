import { describe, expect, test } from "vitest";
import {
  EmptySearchQueryError,
  InvalidSearchCountError,
  InvalidSearchMatcherError,
  InvalidSearchOffsetError,
  TextBuffer,
  buildSearchRegex,
  escapeRegexLiteral,
  findNextRegexMatch,
  findNthNext,
  findNthPrevious,
  findPreviousRegexMatch,
  findRegexMatch,
  iterateRegexMatches,
  searchMatchToSelectionRange,
  SelectionRange,
} from "../src/index";

describe("search", () => {
  test("char search forward/backward and validation", () => {
    expect(findNthNext("ababa", "a", 0, 1)).toBe(0);
    expect(findNthNext("ababa", "a", 0, 2)).toBe(2);
    expect(findNthNext("ababa", "a", 0, 1, "exclusive")).toBe(2);
    expect(findNthPrevious("ababa", "a", 4, 1)).toBe(4);
    expect(findNthPrevious("ababa", "a", 4, 1, "exclusive")).toBe(2);
    expect(findNthNext("ababa", "a", 0, 0)).toBeUndefined();
    expect(() => findNthNext("abc", "a", 0, -1)).toThrow(InvalidSearchCountError);
    expect(() => findNthNext("abc", "a", 4, 1)).toThrow(InvalidSearchOffsetError);
    expect(() => findNthNext("abc", "ab", 0, 1)).toThrow(InvalidSearchMatcherError);
  });

  test("char search is code point safe and predicate receives char and offset", () => {
    const text = "a😊b😊";
    expect(findNthNext(text, "😊", 0, 1)).toBe(1);
    expect(findNthNext(text, "😊", 0, 2)).toBe(4);
    const seen: Array<[string, number]> = [];
    expect(
      findNthNext(
        text,
        (char, offset) => {
          seen.push([char, offset]);
          return char === "b";
        },
        0,
        1,
      ),
    ).toBe(3);
    expect(seen).toContainEqual(["😊", 1]);
  });

  test("regex builder", () => {
    expect(escapeRegexLiteral("a+b.c")).toBe("a\\+b\\.c");
    expect(buildSearchRegex("abc", { smartCase: true }).test("ABC")).toBe(true);
    expect(buildSearchRegex("Abc", { smartCase: true }).test("abc")).toBe(false);
    expect(buildSearchRegex("abc", { smartCase: true, caseSensitive: true }).test("ABC")).toBe(
      false,
    );
    expect(buildSearchRegex("a.b", { literal: true }).test("a.b")).toBe(true);
    const re = buildSearchRegex("abc");
    expect(re.flags).toContain("g");
    expect(re.flags).toContain("u");
    expect(re.flags).toContain("m");
    expect(() => buildSearchRegex("")).toThrow(EmptySearchQueryError);
    expect(() => buildSearchRegex("[")).toThrow(SyntaxError);
  });

  test("regex iteration", () => {
    const regex = /a/g;
    regex.lastIndex = 2;
    expect([...iterateRegexMatches("ababa", regex)]).toEqual([
      { from: 0, to: 1 },
      { from: 2, to: 3 },
      { from: 4, to: 5 },
    ]);
    expect(regex.lastIndex).toBe(2);
    expect([...iterateRegexMatches("xxababa", /a/g, { from: 2, to: 6 })]).toEqual([
      { from: 2, to: 3 },
      { from: 4, to: 5 },
    ]);
    expect([...iterateRegexMatches("a😊b", /(?:)/gu)].map((m) => m.from)).toEqual([0, 1, 3, 4]);
  });

  test("regex search forward/backward and wrap", () => {
    const text = "one two one";
    const re = /one/g;
    expect(findNextRegexMatch(text, re, 0)).toEqual({ from: 0, to: 3 });
    expect(findNextRegexMatch(text, re, 0, { start: "exclusive" })).toEqual({ from: 8, to: 11 });
    expect(findNextRegexMatch(text, re, 9, { wrap: true })).toEqual({ from: 0, to: 3 });
    expect(findPreviousRegexMatch(text, re, 10)).toEqual({ from: 8, to: 11 });
    expect(findPreviousRegexMatch(text, re, 8, { start: "exclusive" })).toEqual({ from: 0, to: 3 });
    expect(findPreviousRegexMatch(text, re, 0, { start: "exclusive", wrap: true })).toEqual({
      from: 8,
      to: 11,
    });
    expect(findRegexMatch(text, re, 0, { direction: "next" })).toEqual({ from: 0, to: 3 });
  });

  test("regex search zero-length and TextBuffer input", () => {
    expect(findNextRegexMatch("ab", /(?:)/g, 0)).toEqual({ from: 0, to: 0 });
    expect(findNextRegexMatch(TextBuffer.fromString("abc abc"), /abc/g, 1, { wrap: true })).toEqual(
      { from: 4, to: 7 },
    );
  });

  test("conversion helper", () => {
    expect(searchMatchToSelectionRange({ from: 1, to: 3 })).toEqual(SelectionRange.create(1, 3));
  });
});
