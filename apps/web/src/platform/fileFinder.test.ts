import { describe, expect, test } from "vitest";
import { byteToUtf16Offset } from "./fileFinder";

describe("byteToUtf16Offset", () => {
  test("ascii is identity", () => {
    expect(byteToUtf16Offset("abcdef", 0)).toBe(0);
    expect(byteToUtf16Offset("abcdef", 3)).toBe(3);
    expect(byteToUtf16Offset("abcdef", 6)).toBe(6);
  });

  test("clamps non-positive and out-of-range offsets", () => {
    expect(byteToUtf16Offset("abc", -5)).toBe(0);
    expect(byteToUtf16Offset("abc", 99)).toBe(3);
    expect(byteToUtf16Offset("", 4)).toBe(0);
  });

  test("two-byte char (é) maps past it correctly", () => {
    // "éfoo": é = U+00E9 (2 bytes, 1 UTF-16 unit). Match "foo" at bytes [2,5].
    const text = "éfoo";
    expect(byteToUtf16Offset(text, 2)).toBe(1);
    expect(byteToUtf16Offset(text, 5)).toBe(4);
    expect(text.slice(byteToUtf16Offset(text, 2), byteToUtf16Offset(text, 5))).toBe("foo");
  });

  test("three-byte char (→) advances one UTF-16 unit per char", () => {
    const text = "→x"; // → = U+2192 (3 bytes, 1 unit)
    expect(byteToUtf16Offset(text, 3)).toBe(1);
    expect(byteToUtf16Offset(text, 4)).toBe(2);
  });

  test("four-byte char (emoji) advances two UTF-16 units", () => {
    const text = "😀x"; // U+1F600 (4 bytes, surrogate pair = 2 units)
    expect(byteToUtf16Offset(text, 4)).toBe(2);
    expect(text.slice(byteToUtf16Offset(text, 4))).toBe("x");
  });
});
