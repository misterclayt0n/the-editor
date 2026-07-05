import { describe, expect, it } from "vitest";
import { Selection, TextBuffer } from "@the-editor/engine";
import { activeIndentGuide, indentationColumns, indentDepthsForLines, indentGuideRanges } from "./indentGuides";

describe("indentGuides", () => {
  it("measures indentation columns with tabs", () => {
    expect(indentationColumns("\t  x", 4)).toBe(6);
    expect(indentationColumns("    x", 4)).toBe(4);
    expect(indentationColumns("   ", 4)).toBeNull();
  });

  it("derives blank line depth from surrounding non-empty lines", () => {
    expect(indentDepthsForLines(["if x {", "    a", "", "    b", "}"], 4)).toEqual([
      0,
      1,
      1,
      1,
      0,
    ]);
  });

  it("builds spanning guide ranges", () => {
    const text = TextBuffer.fromString("fn main() {\n    one();\n    two();\n}\n");
    expect(indentGuideRanges(text, Selection.point(16), 4)).toEqual([
      { startLine: 1, endLine: 2, depth: 0, active: true },
    ]);
  });

  it("selects the child guide when cursor is on the parent opener line", () => {
    expect(activeIndentGuide([0, 1, 1, 0], 0)).toEqual({ line: 0, depth: 0 });
  });
});
