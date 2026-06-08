import { describe, expect, test } from "vitest";
import {
  categorizeChar,
  categorizeCodePoint,
  charIsBreakingWhitespace,
  charIsLineEnding,
  charIsNonBreakingWhitespace,
  charIsWhitespace,
  charIsZeroWidthWhitespace,
  codePointAt,
  codePointBefore,
  codePointIsPunctuation,
  codePointIsWord,
  codePointStringAt,
  codePointStringBefore,
  computeCodePoint,
  isCodePointBoundary,
  isHighSurrogateCodeUnit,
  isLowSurrogateCodeUnit,
  isValidCodePoint,
  isSurrogateCodeUnit,
  nextCodePointBoundary,
  previousCodePointBoundary,
  snapToCodePointBoundaryAfter,
  snapToCodePointBoundaryBefore,
  whitespaceProperties,
} from "../src/index";

describe("chars", () => {
  test("surrogate helpers", () => {
    expect(isHighSurrogateCodeUnit(0xd83d)).toBe(true);
    expect(isLowSurrogateCodeUnit(0xde0a)).toBe(true);
    expect(isSurrogateCodeUnit(0xd83d)).toBe(true);
    expect(isSurrogateCodeUnit(0xde0a)).toBe(true);
    expect(isSurrogateCodeUnit("a".charCodeAt(0))).toBe(false);
    expect(computeCodePoint(0xd83d, 0xde0a)).toBe("😊".codePointAt(0));
  });

  test("UTF-16 boundaries", () => {
    const text = "a😊b";
    expect(isCodePointBoundary(text, 0)).toBe(true);
    expect(isCodePointBoundary(text, 1)).toBe(true);
    expect(isCodePointBoundary(text, 2)).toBe(false);
    expect(isCodePointBoundary(text, 3)).toBe(true);
    expect(isCodePointBoundary(text, 4)).toBe(true);

    expect(snapToCodePointBoundaryBefore(text, 2)).toBe(1);
    expect(snapToCodePointBoundaryAfter(text, 2)).toBe(3);
    expect(snapToCodePointBoundaryBefore(text, 3)).toBe(3);
    expect(snapToCodePointBoundaryAfter(text, 1)).toBe(1);

    expect(previousCodePointBoundary(text, 0)).toBe(0);
    expect(previousCodePointBoundary(text, 1)).toBe(0);
    expect(previousCodePointBoundary(text, 2)).toBe(1);
    expect(previousCodePointBoundary(text, 3)).toBe(1);
    expect(previousCodePointBoundary(text, 4)).toBe(3);

    expect(nextCodePointBoundary(text, 0)).toBe(1);
    expect(nextCodePointBoundary(text, 1)).toBe(3);
    expect(nextCodePointBoundary(text, 2)).toBe(3);
    expect(nextCodePointBoundary(text, 3)).toBe(4);
    expect(nextCodePointBoundary(text, 4)).toBe(4);

    expect(codePointAt(text, 0)).toBe("a".codePointAt(0));
    expect(codePointAt(text, 1)).toBe("😊".codePointAt(0));
    expect(codePointAt(text, 2)).toBe("😊".codePointAt(0));
    expect(codePointAt(text, 3)).toBe("b".codePointAt(0));
    expect(codePointAt(text, 4)).toBeUndefined();

    expect(codePointBefore(text, 0)).toBeUndefined();
    expect(codePointBefore(text, 1)).toBe("a".codePointAt(0));
    expect(codePointBefore(text, 2)).toBe("😊".codePointAt(0));
    expect(codePointBefore(text, 3)).toBe("😊".codePointAt(0));
    expect(codePointBefore(text, 4)).toBe("b".codePointAt(0));

    expect(codePointStringAt(text, 1)).toBe("😊");
    expect(codePointStringBefore(text, 3)).toBe("😊");
  });

  test("invalid code point policy", () => {
    expect(isValidCodePoint(0)).toBe(true);
    expect(isValidCodePoint(0x10ffff)).toBe(true);
    expect(isValidCodePoint(-1)).toBe(false);
    expect(isValidCodePoint(1.5)).toBe(false);
    expect(isValidCodePoint(Number.NaN)).toBe(false);
    expect(isValidCodePoint(0x110000)).toBe(false);

    expect(codePointIsWord(-1)).toBe(false);
    expect(codePointIsPunctuation(Number.NaN)).toBe(false);
    expect(categorizeCodePoint(0x110000)).toBe("unknown");
  });

  test("malformed UTF-16 strings are traversed as code units", () => {
    const loneLow = "\ude0a";
    const loneHigh = "\ud83d";
    const highThenAscii = "\ud83da";
    const asciiThenLow = "a\ude0a";

    expect(isCodePointBoundary(loneLow, 0)).toBe(true);
    expect(isCodePointBoundary(loneLow, 1)).toBe(true);
    expect(nextCodePointBoundary(loneLow, 0)).toBe(1);
    expect(previousCodePointBoundary(loneLow, 1)).toBe(0);
    expect(codePointAt(loneLow, 0)).toBe(loneLow.charCodeAt(0));
    expect(codePointBefore(loneLow, 1)).toBe(loneLow.charCodeAt(0));

    expect(isCodePointBoundary(loneHigh, 0)).toBe(true);
    expect(isCodePointBoundary(loneHigh, 1)).toBe(true);
    expect(nextCodePointBoundary(loneHigh, 0)).toBe(1);
    expect(previousCodePointBoundary(loneHigh, 1)).toBe(0);
    expect(codePointAt(loneHigh, 0)).toBe(loneHigh.charCodeAt(0));
    expect(codePointBefore(loneHigh, 1)).toBe(loneHigh.charCodeAt(0));

    expect(isCodePointBoundary(highThenAscii, 0)).toBe(true);
    expect(isCodePointBoundary(highThenAscii, 1)).toBe(true);
    expect(isCodePointBoundary(highThenAscii, 2)).toBe(true);
    expect(nextCodePointBoundary(highThenAscii, 0)).toBe(1);
    expect(nextCodePointBoundary(highThenAscii, 1)).toBe(2);
    expect(codePointAt(highThenAscii, 0)).toBe(highThenAscii.charCodeAt(0));
    expect(codePointAt(highThenAscii, 1)).toBe("a".codePointAt(0));

    expect(isCodePointBoundary(asciiThenLow, 0)).toBe(true);
    expect(isCodePointBoundary(asciiThenLow, 1)).toBe(true);
    expect(isCodePointBoundary(asciiThenLow, 2)).toBe(true);
    expect(nextCodePointBoundary(asciiThenLow, 0)).toBe(1);
    expect(nextCodePointBoundary(asciiThenLow, 1)).toBe(2);
    expect(codePointAt(asciiThenLow, 0)).toBe("a".codePointAt(0));
    expect(codePointAt(asciiThenLow, 1)).toBe(asciiThenLow.charCodeAt(1));
  });

  test("invalid offsets throw RangeError", () => {
    expect(() => isCodePointBoundary("abc", -1)).toThrow(RangeError);
    expect(() => isCodePointBoundary("abc", 4)).toThrow(RangeError);
    expect(() => isCodePointBoundary("abc", 1.5)).toThrow(RangeError);
    expect(() => nextCodePointBoundary("abc", Number.NaN)).toThrow(RangeError);
  });

  test("classification", () => {
    expect(categorizeChar("\n")).toBe("eol");
    expect(categorizeChar("\r")).toBe("eol");
    expect(categorizeChar(" ")).toBe("whitespace");
    expect(categorizeChar("\t")).toBe("whitespace");
    expect(categorizeChar("a")).toBe("word");
    expect(categorizeChar("é")).toBe("word");
    expect(categorizeChar("9")).toBe("word");
    expect(categorizeChar("_")).toBe("word");
    expect(categorizeChar(".")).toBe("punctuation");
    expect(categorizeChar("+")).toBe("punctuation");
    expect(categorizeChar("$")).toBe("punctuation");
    expect(categorizeChar("😊")).toBe("unknown");
    expect(categorizeChar("")).toBe("unknown");
  });

  test("whitespace properties", () => {
    expect(whitespaceProperties(" ")?.breaking).toBe("breaking");
    expect(whitespaceProperties(" ")?.width).toBe("single");
    expect(whitespaceProperties("\u00A0")?.breaking).toBe("nonBreaking");
    expect(whitespaceProperties("\u200B")?.width).toBe("zero");
    expect(whitespaceProperties("\t")?.width).toBe("variable");

    expect(charIsBreakingWhitespace(" ")).toBe(true);
    expect(charIsNonBreakingWhitespace("\u00A0")).toBe(true);
    expect(charIsZeroWidthWhitespace("\u200B")).toBe(true);
    expect(charIsWhitespace("\n")).toBe(false);
    expect(charIsLineEnding("\n")).toBe(true);
  });
});
