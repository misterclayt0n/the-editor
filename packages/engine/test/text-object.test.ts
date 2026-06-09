import { describe, expect, test } from "vitest";
import Json from "tree-sitter-json";
import { LanguageRegistry, Syntax, type QueryKind, type SyntaxResources } from "@the-editor/syntax";
import {
  TextBuffer,
  Selection,
  SelectionRange,
  wordTextObject,
  paragraphTextObject,
  pairTextObject,
  closestPairTextObject,
  syntaxNamedTextObject,
  moveSyntaxNamedTextObject,
  syntaxTextFromTextBuffer,
} from "../src/index";

const doc = (s: string) => TextBuffer.fromString(s);
const ranges = (s: Selection) => s.ranges.map((r) => [r.anchor, r.head]);
const registryWith = (textobjects: string) =>
  LanguageRegistry.create(
    { language: [{ name: "json", scope: "source.json", fileTypes: ["json"], grammar: "json" }] },
    {
      language: (name) => (name === "json" ? Json : undefined),
      query: (_language, kind: QueryKind) => (kind === "textobjects" ? textobjects : undefined),
    } satisfies SyntaxResources,
  );
const syntaxSetup = (source: string, query: string) => {
  const text = doc(source);
  const registry = registryWith(query);
  const id = registry.languageForName("json")!;
  const syntax = Syntax.parse({ text: syntaxTextFromTextBuffer(text), languageId: id, registry });
  return { text, syntax, query: registry.language(id)!.textObjectQuery(registry)! };
};

describe("text objects", () => {
  test("word objects select inside/around and preserve primary/direction", () => {
    const text = doc("hello world again");
    const selection = Selection.create([SelectionRange.point(1), SelectionRange.create(8, 7)], {
      primaryIndex: 0,
      normalize: false,
    });
    const inside = wordTextObject({ text, selection, kind: "inside" });
    expect(ranges(inside)).toEqual([
      [0, 5],
      [11, 6],
    ]);
    expect(inside.primaryIndex).toBe(0);
    const around = wordTextObject({ text, selection, kind: "around" });
    expect(ranges(around)).toEqual([
      [0, 6],
      [12, 6],
    ]);
  });

  test("word count and long word behavior", () => {
    let text = doc("one two three");
    expect(
      ranges(wordTextObject({ text, selection: Selection.point(1), kind: "inside", count: 2 })),
    ).toEqual([[0, 7]]);
    expect(
      ranges(wordTextObject({ text, selection: Selection.point(1), kind: "around", count: 2 })),
    ).toEqual([[0, 8]]);
    text = doc("foo.bar baz");
    expect(ranges(wordTextObject({ text, selection: Selection.point(1), kind: "inside" }))).toEqual(
      [[0, 3]],
    );
    expect(
      ranges(wordTextObject({ text, selection: Selection.point(1), kind: "inside", long: true })),
    ).toEqual([[0, 7]]);
  });

  test("word on whitespace is point", () => {
    expect(
      ranges(
        wordTextObject({ text: doc("foo bar"), selection: Selection.point(3), kind: "inside" }),
      ),
    ).toEqual([[3, 3]]);
  });

  test("paragraph objects", () => {
    const text = doc("one\n\ntwo\n\nthree");
    expect(
      ranges(
        paragraphTextObject({ text, selection: Selection.point(1), kind: "inside", count: 2 }),
      ),
    ).toEqual([[0, 9]]);
    expect(
      ranges(
        paragraphTextObject({ text, selection: Selection.point(5), kind: "around", count: 1 }),
      ),
    ).toEqual([[5, 10]]);
  });

  test("paragraph from blank line", () => {
    expect(
      ranges(
        paragraphTextObject({
          text: doc("one\n\ntwo"),
          selection: Selection.point(4),
          kind: "inside",
        }),
      ),
    ).toEqual([[5, 8]]);
    expect(
      ranges(
        paragraphTextObject({
          text: doc("one\n\n"),
          selection: Selection.point(4),
          kind: "inside",
        }),
      ),
    ).toEqual([[0, 4]]);
    expect(
      ranges(
        paragraphTextObject({ text: doc("\n\n"), selection: Selection.point(0), kind: "inside" }),
      ),
    ).toEqual([[0, 0]]);
  });

  test("pair objects", () => {
    const text = doc("(alpha) beta");
    expect(
      ranges(pairTextObject({ text, selection: Selection.point(2), kind: "inside", ch: "(" })),
    ).toEqual([[1, 6]]);
    expect(
      ranges(pairTextObject({ text, selection: Selection.point(2), kind: "around", ch: "(" })),
    ).toEqual([[0, 7]]);
    const sel = Selection.point(9);
    expect(ranges(pairTextObject({ text, selection: sel, kind: "inside", ch: "[" }))).toEqual([
      [9, 9],
    ]);
  });

  test("closest pair", () => {
    const text = doc("(outer [inner])");
    expect(
      ranges(closestPairTextObject({ text, selection: Selection.point(8), kind: "inside" })),
    ).toEqual([[8, 13]]);
    expect(
      ranges(
        closestPairTextObject({ text, selection: Selection.point(8), kind: "around", count: 2 }),
      ),
    ).toEqual([[0, 15]]);
  });

  test("syntax named smallest containing capture", () => {
    const s = syntaxSetup("[1,[2,3]]", `(array) @array.around`);
    expect(
      ranges(
        syntaxNamedTextObject({
          ...s,
          selection: Selection.point(5),
          kind: "around",
          objectName: "array",
        }),
      ),
    ).toEqual([[3, 8]]);
  });

  test("syntax named inside capture", () => {
    const s = syntaxSetup("[1,[2,3]]", `(array) @array.around\n(number) @number.inside`);
    expect(
      ranges(
        syntaxNamedTextObject({
          ...s,
          selection: Selection.point(4),
          kind: "inside",
          objectName: "number",
        }),
      ),
    ).toEqual([[4, 5]]);
  });

  test("move syntax named forward/backward", () => {
    const s = syntaxSetup("[1,[2,3],[4,5]]", `(array) @array.around`);
    expect(
      ranges(
        moveSyntaxNamedTextObject({
          ...s,
          selection: Selection.point(1),
          objectName: "array",
          direction: "forward",
        }),
      ),
    ).toEqual([[3, 8]]);
    expect(
      ranges(
        moveSyntaxNamedTextObject({
          ...s,
          selection: Selection.point(15),
          objectName: "array",
          direction: "backward",
        }),
      ),
    ).toEqual([[9, 14]]);
  });

  test("syntax named no matching capture no-op", () => {
    const s = syntaxSetup("[1]", `(array) @array.around`);
    const sel = Selection.point(1);
    expect(
      ranges(syntaxNamedTextObject({ ...s, selection: sel, kind: "inside", objectName: "array" })),
    ).toEqual([[1, 1]]);
  });
});
