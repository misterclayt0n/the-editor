import { describe, expect, test } from "vitest";
import {
  detectLineEnding,
  getLineEndingOfString,
  isLineEndingString,
  lineEndWithoutLineEnding,
  lineEndingFromString,
  lineEndingLength,
  lineEndingString,
  normalizeLineEndings,
  scanLineEndings,
  stripLineEnding,
} from "../src/index";

describe("line-ending", () => {
  test("basic conversion", () => {
    expect(lineEndingString("lf")).toBe("\n");
    expect(lineEndingString("crlf")).toBe("\r\n");
    expect(lineEndingLength("lf")).toBe(1);
    expect(lineEndingLength("crlf")).toBe(2);
  });

  test("from string and predicate", () => {
    expect(lineEndingFromString("\n")).toBe("lf");
    expect(lineEndingFromString("\r\n")).toBe("crlf");
    expect(lineEndingFromString("\r")).toBeUndefined();
    expect(lineEndingFromString("hello\n")).toBeUndefined();
    expect(isLineEndingString("\n")).toBe(true);
    expect(isLineEndingString("\r\n")).toBe(true);
    expect(isLineEndingString("\r")).toBe(false);
  });

  test("line suffix helpers", () => {
    expect(getLineEndingOfString("hello\r\n")).toBe("crlf");
    expect(getLineEndingOfString("hello\n")).toBe("lf");
    expect(getLineEndingOfString("hello\r")).toBeUndefined();
    expect(getLineEndingOfString("hello")).toBeUndefined();

    expect(lineEndWithoutLineEnding("hello\r\n")).toBe(5);
    expect(lineEndWithoutLineEnding("hello\n")).toBe(5);
    expect(lineEndWithoutLineEnding("hello\r")).toBe(6);
    expect(lineEndWithoutLineEnding("hello")).toBe(5);

    expect(stripLineEnding("hello\r\n")).toBe("hello");
    expect(stripLineEnding("hello\n")).toBe("hello");
    expect(stripLineEnding("hello\r")).toBe("hello\r");
    expect(stripLineEnding("hello")).toBe("hello");
  });

  test("scanLineEndings", () => {
    expect(scanLineEndings("abc")).toEqual({
      eolCount: 0,
      firstLineLength: 3,
      lastLineLength: 3,
      kind: "unknown",
      hasBareCarriageReturn: false,
    });
    expect(scanLineEndings("a\nb\n")).toEqual({
      eolCount: 2,
      firstLineLength: 1,
      lastLineLength: 0,
      kind: "lf",
      hasBareCarriageReturn: false,
    });
    expect(scanLineEndings("a\r\nb\r\n")).toEqual({
      eolCount: 2,
      firstLineLength: 1,
      lastLineLength: 0,
      kind: "crlf",
      hasBareCarriageReturn: false,
    });
    expect(scanLineEndings("a\r\nb\n")).toMatchObject({
      eolCount: 2,
      firstLineLength: 1,
      lastLineLength: 0,
      kind: "mixed",
      hasBareCarriageReturn: false,
    });
    expect(scanLineEndings("a\rb")).toEqual({
      eolCount: 1,
      firstLineLength: 1,
      lastLineLength: 1,
      kind: "invalid",
      hasBareCarriageReturn: true,
    });
    expect(scanLineEndings("a\rb\n")).toEqual({
      eolCount: 2,
      firstLineLength: 1,
      lastLineLength: 0,
      kind: "invalid",
      hasBareCarriageReturn: true,
    });
  });

  test("detectLineEnding", () => {
    expect(detectLineEnding("abc")).toBeUndefined();
    expect(detectLineEnding("a\r\nb\n")).toBe("crlf");
    expect(detectLineEnding("a\nb\r\n")).toBe("lf");
    expect(detectLineEnding("a\rb")).toBeUndefined();
    expect(detectLineEnding("a\rb\n")).toBe("lf");
  });

  test("normalizeLineEndings", () => {
    expect(normalizeLineEndings("a\r\nb\rc\n", "lf")).toBe("a\nb\nc\n");
    expect(normalizeLineEndings("a\nb", "crlf")).toBe("a\r\nb");
    expect(normalizeLineEndings("abc", "lf")).toBe("abc");
  });
});
