import { describe, expect, it } from "vitest";
import { Selection, SelectionRange, TextBuffer } from "@the-editor/engine";
import { findSelectionMatches, selectionMatchQuery } from "./selectionMatches";

describe("selectionMatches", () => {
  it("derives a query from one non-empty single-line selection", () => {
    const text = TextBuffer.fromString("alpha beta alpha");
    expect(selectionMatchQuery(text, Selection.single(0, 5))).toEqual({
      text: "alpha",
      range: { from: 0, to: 5 },
    });
  });

  it("rejects selections that should not drive occurrence highlighting", () => {
    const text = TextBuffer.fromString("alpha\nalpha\n   ");
    expect(selectionMatchQuery(text, Selection.point(0))).toBeNull();
    expect(selectionMatchQuery(text, Selection.single(0, 7))).toBeNull();
    expect(selectionMatchQuery(text, Selection.single(12, 15))).toBeNull();
    expect(
      selectionMatchQuery(
        text,
        Selection.create([SelectionRange.create(0, 5), SelectionRange.create(6, 11)]),
      ),
    ).toBeNull();
    expect(selectionMatchQuery(text, Selection.single(0, 5), { maxLength: 4 })).toBeNull();
  });

  it("finds case-insensitive non-overlapping matches and excludes the selection", () => {
    const text = TextBuffer.fromString("Alpha alpha ALPHA alphabet alpha");
    const query = selectionMatchQuery(text, Selection.single(0, 5));
    expect(query).not.toBeNull();
    expect(findSelectionMatches(text, query!)).toEqual([
      { from: 6, to: 11 },
      { from: 12, to: 17 },
      { from: 18, to: 23 },
      { from: 27, to: 32 },
    ]);
  });

  it("can search just a visible range", () => {
    const text = TextBuffer.fromString("foo one foo two foo three foo");
    const query = selectionMatchQuery(text, Selection.single(0, 3));
    expect(query).not.toBeNull();
    expect(findSelectionMatches(text, query!, { from: 8, to: 22 })).toEqual([
      { from: 8, to: 11 },
      { from: 16, to: 19 },
    ]);
  });
});
