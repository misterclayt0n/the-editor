import { describe, expect, it } from "vitest";
import { Selection, TextBuffer } from "@the-editor/engine";
import { documentHighlightQuery, surroundingWord } from "./documentHighlights";

describe("documentHighlights", () => {
  it("finds the surrounding word at either side of the caret", () => {
    const text = TextBuffer.fromString("int factorial(int x)");
    expect(surroundingWord(text, 4)).toEqual({ from: 4, to: 13 });
    expect(surroundingWord(text, 13)).toEqual({ from: 4, to: 13 });
    expect(surroundingWord(text, 3)).toEqual({ from: 0, to: 3 });
    expect(surroundingWord(TextBuffer.fromString("int factorial(+ x)"), 14)).toBeNull();
  });

  it("derives a query only when head and anchor are in the same word", () => {
    const text = TextBuffer.fromString("int factorial(int x)");
    expect(documentHighlightQuery(text, Selection.point(7))).toEqual({
      offset: 7,
      word: { from: 4, to: 13 },
    });
    expect(documentHighlightQuery(text, Selection.single(4, 8))).toEqual({
      offset: 8,
      word: { from: 4, to: 13 },
    });
    expect(documentHighlightQuery(text, Selection.single(0, 8))).toBeNull();
    expect(documentHighlightQuery(TextBuffer.fromString("int factorial(+ x)"), Selection.point(14))).toBeNull();
  });
});
