import { describe, expect, test } from "vitest";
import { syntaxTextFromString } from "@the-editor/syntax";
import {
  columnAtOffset,
  displayGrapheme,
  graphemeWidth,
  offsetAtColumn,
  tabWidthAt,
  TextBuffer,
} from "../src/index";

describe("columns", () => {
  test("tabWidthAt", () => {
    expect(tabWidthAt(0, 4)).toBe(4);
    expect(tabWidthAt(1, 4)).toBe(3);
    expect(tabWidthAt(2, 4)).toBe(2);
    expect(tabWidthAt(3, 4)).toBe(1);
    expect(tabWidthAt(4, 4)).toBe(4);
  });

  test("graphemeWidth", () => {
    expect(graphemeWidth("")).toBe(0);
    expect(graphemeWidth("a")).toBe(1);
    expect(graphemeWidth("\u0000")).toBe(2);
    expect(graphemeWidth("\u007f")).toBe(2);
    expect(graphemeWidth("\u200B")).toBe(0);
    expect(graphemeWidth("😀")).toBe(2);
    expect(graphemeWidth("漢")).toBe(2);
    expect(graphemeWidth("a\u0301")).toBe(1);
    expect(graphemeWidth("\u0301")).toBe(0);
  });

  test("displayGrapheme", () => {
    expect(displayGrapheme("\n", 0, 4)).toEqual({
      kind: "newline",
      text: "\n",
      width: 1,
      displayText: " ",
    });
    expect(displayGrapheme("\r\n", 0, 4)).toEqual({
      kind: "newline",
      text: "\r\n",
      width: 1,
      displayText: " ",
    });
    expect(displayGrapheme("\t", 2, 4)).toEqual({
      kind: "tab",
      text: "\t",
      width: 2,
      displayText: "  ",
    });
    expect(displayGrapheme("a", 0, 4)).toEqual({
      kind: "other",
      text: "a",
      width: 1,
      displayText: "a",
    });
  });

  test("invalid tab args", () => {
    expect(() => tabWidthAt(-1, 4)).toThrow(RangeError);
    expect(() => tabWidthAt(0.5, 4)).toThrow(RangeError);
    expect(() => tabWidthAt(0, 0)).toThrow(RangeError);
    expect(() => tabWidthAt(0, 1.5)).toThrow(RangeError);
  });

  test("columnAtOffset uses grapheme and tab widths", () => {
    const text = TextBuffer.fromString("a\t😀b\nxx");
    expect(columnAtOffset({ text, offset: 0, tabWidth: 4 })).toBe(0);
    expect(columnAtOffset({ text, offset: 1, tabWidth: 4 })).toBe(1);
    expect(columnAtOffset({ text, offset: 2, tabWidth: 4 })).toBe(4);
    expect(columnAtOffset({ text, offset: 4, tabWidth: 4 })).toBe(6);
    expect(columnAtOffset({ text, offset: 5, tabWidth: 4 })).toBe(7);
    expect(columnAtOffset({ text, offset: 7, tabWidth: 4 })).toBe(1);
  });

  test("offsetAtColumn finds offsets with before and after bias", () => {
    const text = TextBuffer.fromString("a\t😀b");
    expect(offsetAtColumn({ text, line: 0, column: 0, tabWidth: 4 })).toBe(0);
    expect(offsetAtColumn({ text, line: 0, column: 1, tabWidth: 4 })).toBe(1);
    expect(offsetAtColumn({ text, line: 0, column: 2, tabWidth: 4, bias: "before" })).toBe(1);
    expect(offsetAtColumn({ text, line: 0, column: 2, tabWidth: 4, bias: "after" })).toBe(2);
    expect(offsetAtColumn({ text, line: 0, column: 5, tabWidth: 4, bias: "before" })).toBe(2);
    expect(offsetAtColumn({ text, line: 0, column: 5, tabWidth: 4, bias: "after" })).toBe(4);
    expect(offsetAtColumn({ text, line: 0, column: 99, tabWidth: 4 })).toBe(5);
  });

  test("column primitives also accept SyntaxText", () => {
    const text = syntaxTextFromString("ab\n\tcd");
    expect(columnAtOffset({ text, offset: 4, tabWidth: 4 })).toBe(4);
    expect(offsetAtColumn({ text, line: 1, column: 5, tabWidth: 4 })).toBe(5);
  });
});
