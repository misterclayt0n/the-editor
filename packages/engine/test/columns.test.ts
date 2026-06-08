import { describe, expect, test } from "vitest";
import { displayGrapheme, graphemeWidth, tabWidthAt } from "../src/index";

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
});
