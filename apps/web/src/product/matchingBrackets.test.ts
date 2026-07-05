import { describe, expect, it } from "vitest";
import { Selection } from "@the-editor/engine";
import { TextBuffer } from "@the-editor/engine";
import {
  decoratedRangesForMatch,
  innermostEnclosingBracketRanges,
  matchingBracketRangesForSelection,
} from "./matchingBrackets";

const text = (source: string) => TextBuffer.fromString(source);

describe("matchingBrackets", () => {
  it("finds the innermost enclosing structural bracket pair", () => {
    const buffer = text("fn main() { call(1); }");
    const cursor = buffer.sliceString(0).indexOf("1");
    expect(matchingBracketRangesForSelection(buffer, Selection.point(cursor))).toEqual({
      open: { from: 16, to: 17 },
      close: { from: 18, to: 19 },
    });
  });

  it("matches the bracket under a block cursor", () => {
    const buffer = text("{}");
    expect(matchingBracketRangesForSelection(buffer, Selection.point(0), "block")).toEqual({
      open: { from: 0, to: 1 },
      close: { from: 1, to: 2 },
    });
  });

  it("returns the outer pair when the cursor is in body text", () => {
    const buffer = text("if (ok) { call(); }");
    const cursor = buffer.sliceString(0).indexOf("call");
    expect(matchingBracketRangesForSelection(buffer, Selection.point(cursor))).toEqual({
      open: { from: 8, to: 9 },
      close: { from: 18, to: 19 },
    });
  });

  it("uses one outline rectangle for adjacent matching brackets", () => {
    expect(
      decoratedRangesForMatch({ open: { from: 0, to: 1 }, close: { from: 1, to: 2 } }),
    ).toEqual([{ from: 0, to: 2 }]);
  });

  it("uses separate outline rectangles for non-adjacent matching brackets", () => {
    expect(
      decoratedRangesForMatch({ open: { from: 0, to: 1 }, close: { from: 4, to: 5 } }),
    ).toEqual([
      { from: 0, to: 1 },
      { from: 4, to: 5 },
    ]);
  });

  it("clears for non-empty selections", () => {
    const buffer = text("(abc)");
    expect(matchingBracketRangesForSelection(buffer, Selection.single(1, 3))).toBeNull();
  });

  it("returns null for unmatched or mismatched brackets", () => {
    expect(innermostEnclosingBracketRanges(text("([)"), 2, 2)).toBeNull();
    expect(innermostEnclosingBracketRanges(text("abc"), 1, 1)).toBeNull();
  });
});
