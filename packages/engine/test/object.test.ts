import { describe, expect, test } from "vitest";
import Json from "tree-sitter-json";
import { LanguageRegistry, Syntax, type QueryKind, type SyntaxResources } from "@the-editor/syntax";
import { TextBuffer, Selection, SelectionRange, syntaxTextFromTextBuffer, expandSelection, shrinkSelection, selectNextSibling, selectPrevSibling, selectAllChildren, selectAllSiblings, moveParentEdge } from "../src/index";

const registry = () => LanguageRegistry.create(
  { language: [{ name: "json", scope: "source.json", fileTypes: ["json"], grammar: "json" }] },
  { language: (name) => name === "json" ? Json : undefined, query: (_language, _kind: QueryKind) => undefined } satisfies SyntaxResources,
);
function setup(source: string) {
  const text = TextBuffer.fromString(source); const r = registry(); const id = r.languageForName("json")!;
  return { text, syntax: Syntax.parse({ text: syntaxTextFromTextBuffer(text), languageId: id, registry: r }) };
}
const ranges = (s: Selection) => s.ranges.map((r) => [r.anchor, r.head]);

describe("syntax object selection", () => {
  test("expand selection expands exact node to parent and preserves direction", () => {
    const s = setup("[1,22,333]");
    expect(ranges(expandSelection({ ...s, selection: Selection.single(5, 3) }))).toEqual([[10, 0]]);
  });

  test("shrink selection is no-op when no direct named child contains the range", () => {
    const s = setup("[1,22,333]");
    expect(ranges(shrinkSelection({ ...s, selection: Selection.single(0, 10) }))).toEqual([[0, 10]]);
  });

  test("sibling navigation", () => {
    const s = setup("[1,22,333]");
    expect(ranges(selectNextSibling({ ...s, selection: Selection.single(2, 1) }))).toEqual([[3, 5]]);
    expect(ranges(selectPrevSibling({ ...s, selection: Selection.single(6, 9) }))).toEqual([[5, 3]]);
  });

  test("sibling navigation climbs to parent", () => {
    const s = setup("[[1],[2]]");
    expect(ranges(selectNextSibling({ ...s, selection: Selection.single(2, 3) }))).toEqual([[5, 8]]);
  });

  test("select all children fan-out", () => {
    const s = setup("[1,22,333]");
    const selection = selectAllChildren({ ...s, selection: Selection.single(0, 10) });
    expect(ranges(selection)).toEqual([[1, 2], [3, 5], [6, 9]]);
    expect(selection.primaryIndex).toBe(0);
  });

  test("select all siblings climbs past single-child parents", () => {
    const s = setup("[[1],[2]]");
    expect(ranges(selectAllSiblings({ ...s, selection: Selection.single(2, 3) }))).toEqual([[1, 4], [5, 8]]);
  });

  test("select all children no-op for leaf", () => {
    const s = setup("[1,22,333]");
    expect(ranges(selectAllChildren({ ...s, selection: Selection.single(3, 5) }))).toEqual([[3, 5]]);
  });

  test("move parent edge forward move/extend", () => {
    const s = setup("[1,22,333]");
    expect(ranges(moveParentEdge({ ...s, selection: Selection.point(1), direction: "forward", mode: "move" }))).toEqual([[2, 3]]);
    expect(ranges(moveParentEdge({ ...s, selection: Selection.point(0), direction: "forward", mode: "move" }))).toEqual([[10, 10]]);
    expect(ranges(moveParentEdge({ ...s, selection: Selection.point(1), direction: "forward", mode: "extend" }))).toEqual([[1, 3]]);
  });

  test("move parent edge backward", () => {
    const s = setup("[1,22,333]");
    expect(ranges(moveParentEdge({ ...s, selection: Selection.point(4), direction: "backward", mode: "move" }))).toEqual([[3, 4]]);
    expect(ranges(moveParentEdge({ ...s, selection: Selection.point(3), direction: "backward", mode: "move" }))).toEqual([[0, 1]]);
  });

  test("empty syntax no-op-ish", () => {
    const s = setup("");
    expect(ranges(expandSelection({ ...s, selection: Selection.point(0) }))).toEqual([[0, 0]]);
  });
});
