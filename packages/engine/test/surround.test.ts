import { describe, expect, test } from "vitest";
import Json from "tree-sitter-json";
import {
  LanguageRegistry,
  Syntax,
  syntaxTextFromString,
  type SyntaxResources,
} from "@the-editor/syntax";
import {
  CursorOnAmbiguousSurroundPairError,
  InvalidSurroundPairCharError,
  Selection,
  SelectionRange,
  SurroundCursorOverlapError,
  SurroundPairNotFoundError,
  SurroundRangeExceedsTextError,
  TextBuffer,
  findNthClosestSurroundPair,
  findNthSurroundPair,
  getSurroundPairs,
  surroundPairChars,
  syntaxTextFromTextBuffer,
} from "../src/index";

const resources: SyntaxResources = {
  language: (name) => (name === "json" ? Json : undefined),
  query: () => undefined,
};
const registry = LanguageRegistry.create(
  { language: [{ name: "json", scope: "source.json", fileTypes: ["json"], grammar: "json" }] },
  resources,
);
const syntaxFor = (source: string) => {
  const text = syntaxTextFromString(source);
  return {
    text,
    syntax: Syntax.parse({ text, languageId: registry.languageForName("json")!, registry }),
  };
};

describe("surround", () => {
  test("explicit pair lookup returns normalized bounds for backward ranges", () => {
    expect(
      findNthSurroundPair(syntaxTextFromString("(abc)"), "(", SelectionRange.create(3, 1), 1),
    ).toEqual({ open: 0, close: 4 });
  });
  test("symmetric pair lookup supports skip and ambiguity", () => {
    const text = syntaxTextFromString("some 'nested 'quoted' text' on this line");
    expect(findNthSurroundPair(text, "'", SelectionRange.point(18), 2)).toEqual({
      open: 5,
      close: 26,
    });
    expect(() =>
      findNthSurroundPair(syntaxTextFromString("'quoted'"), "'", SelectionRange.point(0), 1),
    ).toThrow(CursorOnAmbiguousSurroundPairError);
  });
  test("closest pair lookup works without syntax", () => {
    const text = syntaxTextFromString("(outer [inner])");
    expect(findNthClosestSurroundPair(text, SelectionRange.point(8), 1)).toEqual({
      open: 7,
      close: 13,
    });
    expect(findNthClosestSurroundPair(text, SelectionRange.point(8), 2)).toEqual({
      open: 0,
      close: 14,
    });
  });
  test("closest plaintext lookup follows nested stack behavior", () => {
    const text = syntaxTextFromString("(a [b] c)");
    expect(findNthClosestSurroundPair(text, SelectionRange.point(2), 1)).toEqual({
      open: 0,
      close: 8,
    });
    expect(findNthClosestSurroundPair(text, SelectionRange.point(4), 1)).toEqual({
      open: 3,
      close: 5,
    });
    expect(findNthClosestSurroundPair(text, SelectionRange.point(4), 2)).toEqual({
      open: 0,
      close: 8,
    });
    const mixed = syntaxTextFromString("{ a (b [c]) d }");
    expect(findNthClosestSurroundPair(mixed, SelectionRange.point(8), 1)).toEqual({
      open: 7,
      close: 9,
    });
    expect(findNthClosestSurroundPair(mixed, SelectionRange.point(8), 2)).toEqual({
      open: 4,
      close: 10,
    });
    expect(findNthClosestSurroundPair(mixed, SelectionRange.point(8), 3)).toEqual({
      open: 0,
      close: 14,
    });
  });
  test("closest pair lookup can use syntax", () => {
    const { text, syntax } = syntaxFor("[1,[2,3]]");
    expect(findNthClosestSurroundPair(text, SelectionRange.point(5), 1, { syntax })).toEqual({
      open: 3,
      close: 7,
    });
  });
  test("getSurroundPairs is all-or-nothing and rejects overlaps", () => {
    const text = syntaxTextFromString("[some]\n(chars)xx\n(newline)");
    expect(() =>
      getSurroundPairs(
        text,
        Selection.create([SelectionRange.point(2), SelectionRange.point(10)], { normalize: false }),
        "(",
        1,
      ),
    ).toThrow(SurroundPairNotFoundError);
    expect(() =>
      getSurroundPairs(
        syntaxTextFromString("[some]"),
        Selection.create([SelectionRange.point(2), SelectionRange.point(3)], { normalize: false }),
        "[",
        1,
      ),
    ).toThrow(SurroundCursorOverlapError);
  });
  test("validation", () => {
    expect(() => surroundPairChars("")).toThrow(InvalidSurroundPairCharError);
    expect(() => surroundPairChars("ab")).toThrow(InvalidSurroundPairCharError);
    expect(surroundPairChars("😀")).toEqual({ open: "😀", close: "😀" });
    const text = syntaxTextFromString("(x)");
    expect(() =>
      findNthSurroundPair(text, "(", SelectionRange.create(0, text.length + 1), 1),
    ).toThrow(SurroundRangeExceedsTextError);
    expect(() => findNthSurroundPair(text, "(", SelectionRange.point(1), 0)).toThrow(
      SurroundPairNotFoundError,
    );
    expect(() => findNthClosestSurroundPair(text, SelectionRange.point(1), 0)).toThrow(
      SurroundPairNotFoundError,
    );
    expect(findNthSurroundPair(text, "(", SelectionRange.point(0), 1)).toEqual({
      open: 0,
      close: 2,
    });
    expect(findNthSurroundPair(text, ")", SelectionRange.point(2), 1)).toEqual({
      open: 0,
      close: 2,
    });
  });
  test("unicode offsets and TextBuffer adapter", () => {
    const text = syntaxTextFromString("😊(x[ｙ])");
    expect(findNthSurroundPair(text, "(", SelectionRange.point(3), 1)).toEqual({
      open: 2,
      close: 7,
    });
    expect(findNthSurroundPair(text, "[", SelectionRange.point(5), 1)).toEqual({
      open: 4,
      close: 6,
    });
    const bufferText = syntaxTextFromTextBuffer(TextBuffer.fromString("(x)"));
    expect(findNthSurroundPair(bufferText, "(", SelectionRange.point(1), 1)).toEqual({
      open: 0,
      close: 2,
    });
  });
  test("does not materialize full text in local scan loops", () => {
    const source = "(x)";
    const text = {
      length: source.length,
      sliceString: (from: number, to: number) => {
        if (from === 0 && to === source.length) throw new Error("full text");
        return source.slice(from, to);
      },
    };
    expect(findNthSurroundPair(text, "(", SelectionRange.point(1), 1)).toEqual({
      open: 0,
      close: 2,
    });
  });
});
