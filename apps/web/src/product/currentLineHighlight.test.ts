import { describe, expect, it } from "vitest";
import { Selection, SelectionRange } from "@the-editor/engine";
import { currentLineOffsets } from "./currentLineHighlight";

describe("currentLineHighlight", () => {
  it("returns one sorted offset per collapsed cursor", () => {
    const selection = Selection.create([
      SelectionRange.point(8),
      SelectionRange.point(2),
      SelectionRange.point(8),
    ]);
    expect(currentLineOffsets(selection)).toEqual([2, 8]);
  });

  it("does not render while any selection is non-empty", () => {
    const selection = Selection.create([SelectionRange.point(2), SelectionRange.create(4, 7)]);
    expect(currentLineOffsets(selection)).toEqual([]);
  });
});
