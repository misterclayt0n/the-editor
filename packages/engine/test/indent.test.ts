import { describe, expect, test } from "vitest";
import Json from "tree-sitter-json";
import {
  LanguageRegistry,
  Syntax,
  syntaxTextFromString,
  type QueryKind,
  type SyntaxResources,
} from "@the-editor/syntax";
import {
  IndentStyle,
  Indentation,
  autoDetectIndentStyle,
  indentForNewline,
  indentLevelForLine,
  normalizeIndentation,
  treeSitterIndentForPos,
  whitespacePrefixForWidth,
  whitespaceWidth,
} from "../src/index";

const registryWith = (indents: string) =>
  LanguageRegistry.create(
    { language: [{ name: "json", scope: "source.json", fileTypes: ["json"], grammar: "json" }] },
    {
      language: (name) => (name === "json" ? Json : undefined),
      query: (_language, kind: QueryKind) => (kind === "indents" ? indents : undefined),
    } satisfies SyntaxResources,
  );
const syntaxSetup = (source: string, query: string) => {
  const text = syntaxTextFromString(source);
  const r = registryWith(query);
  const id = r.languageForName("json")!;
  return {
    text,
    syntax: Syntax.parse({ text, languageId: id, registry: r }),
    query: r.language(id)!.indentQuery(r)!,
  };
};

describe("indent primitives", () => {
  test("indent levels", () => {
    expect(indentLevelForLine("        fn new", 4, 4)).toBe(2);
    expect(indentLevelForLine("\t\t\tfn new", 4, 4)).toBe(3);
    expect(indentLevelForLine("\t    \tfn new", 4, 4)).toBe(3);
    expect(indentLevelForLine("                fn new", 16, 16)).toBe(1);
    expect(indentLevelForLine("                                fn new", 16, 16)).toBe(2);
  });

  test("indentation addCapture semantics", () => {
    expect(Indentation.empty().addCapture({ type: "indent" }).netIndent()).toBe(1);
    expect(
      Indentation.empty().addCapture({ type: "indent" }).addCapture({ type: "indent" }).netIndent(),
    ).toBe(1);
    expect(
      Indentation.empty()
        .addCapture({ type: "indentAlways" })
        .addCapture({ type: "indentAlways" })
        .netIndent(),
    ).toBe(2);
    expect(
      Indentation.empty()
        .addCapture({ type: "indent" })
        .addCapture({ type: "indentAlways" })
        .netIndent(),
    ).toBe(1);
    expect(Indentation.empty().addCapture({ type: "outdent" }).netIndent()).toBe(-1);
    expect(
      Indentation.empty()
        .addCapture({ type: "outdentAlways" })
        .addCapture({ type: "outdentAlways" })
        .netIndent(),
    ).toBe(-2);
  });

  test("relative indentation", () => {
    const style = IndentStyle.spaces(2);
    expect(
      new Indentation({ indent: 2 }).relativeIndent({
        otherComputedIndent: new Indentation({ indent: 1 }),
        otherLeadingWhitespace: "  ",
        indentStyle: style,
        tabWidth: 4,
      }),
    ).toBe("    ");
    expect(
      new Indentation({ indent: 0 }).relativeIndent({
        otherComputedIndent: new Indentation({ indent: 1 }),
        otherLeadingWhitespace: "    ",
        indentStyle: style,
        tabWidth: 4,
      }),
    ).toBe("  ");
    expect(
      new Indentation({ align: "  " }).relativeIndent({
        otherComputedIndent: new Indentation({ align: "\t" }),
        otherLeadingWhitespace: "\t",
        indentStyle: style,
        tabWidth: 4,
      }),
    ).toBeUndefined();
  });

  test("indent style and whitespace helpers", () => {
    expect(IndentStyle.fromString("\t")).toEqual({ type: "tabs" });
    expect(IndentStyle.fromString("    ")).toEqual({ type: "spaces", size: 4 });
    expect(IndentStyle.asString({ type: "spaces", size: 20 })).toBe("                ");
    expect(IndentStyle.width({ type: "tabs" }, 8)).toBe(8);
    expect(whitespaceWidth(" \t", 4)).toBe(4);
    expect(whitespacePrefixForWidth(" \tx", 3, 4)).toBe(1);
  });

  test("auto detect indent style", () => {
    expect(autoDetectIndentStyle(syntaxTextFromString("a\n\tb\n\t\tc"))).toEqual({ type: "tabs" });
    expect(autoDetectIndentStyle(syntaxTextFromString("a\n  b\n    c"))).toEqual({
      type: "spaces",
      size: 2,
    });
    expect(autoDetectIndentStyle(syntaxTextFromString("a\n    b\n        c"))).toEqual({
      type: "spaces",
      size: 4,
    });
    expect(autoDetectIndentStyle(syntaxTextFromString("a\nb\nc"))).toBeUndefined();
  });

  test("normalize indentation", () => {
    expect(
      normalizeIndentation({
        prefix: "",
        line: "    x",
        indentStyle: IndentStyle.tabs(),
        tabWidth: 4,
      }),
    ).toEqual({ text: "\t", consumed: 4 });
    expect(
      normalizeIndentation({
        prefix: "",
        line: "\tx",
        indentStyle: IndentStyle.spaces(2),
        tabWidth: 4,
      }),
    ).toEqual({ text: "    ", consumed: 1 });
    expect(
      normalizeIndentation({
        prefix: " ",
        line: "\tx",
        indentStyle: IndentStyle.tabs(),
        tabWidth: 4,
      }),
    ).toEqual({ text: "   ", consumed: 1 });
  });

  test("simple fallback newline indentation", () => {
    const text = syntaxTextFromString("    x");
    expect(
      indentForNewline({
        heuristic: "simple",
        indentStyle: IndentStyle.spaces(2),
        tabWidth: 4,
        text,
        lineBefore: 0,
        lineBeforeEndPos: text.length,
        currentLine: 0,
      }),
    ).toBe("    ");
  });

  test("tree-sitter indent basic and predicate filtering", () => {
    const s = syntaxSetup('{\n  "a": 1\n}', `(object) @indent`);
    expect(
      indentForNewline({
        query: s.query,
        syntax: s.syntax,
        heuristic: "treeSitter",
        indentStyle: IndentStyle.spaces(2),
        tabWidth: 4,
        text: s.text,
        lineBefore: 0,
        lineBeforeEndPos: 1,
        currentLine: 0,
      }),
    ).toBe("  ");
    const no = syntaxSetup("{\n}", `(object) @indent (#not-kind-eq? @indent "object")`);
    expect(
      treeSitterIndentForPos({
        query: no.query,
        syntax: no.syntax,
        text: no.text,
        tabWidth: 4,
        indentWidth: 2,
        line: 1,
        pos: 1,
        newLine: true,
      })?.netIndent(),
    ).toBe(0);
    const yes = syntaxSetup("{\n}", `(object) @indent (#not-kind-eq? @indent "array")`);
    expect(
      treeSitterIndentForPos({
        query: yes.query,
        syntax: yes.syntax,
        text: yes.text,
        tabWidth: 4,
        indentWidth: 2,
        line: 1,
        pos: 1,
        newLine: true,
      })?.netIndent(),
    ).toBe(1);
  });

  test("same-line regular indents coalesce", () => {
    const s = syntaxSetup('{"a": {"b": 1}}', `(object) @indent (pair) @indent`);
    expect(
      treeSitterIndentForPos({
        query: s.query,
        syntax: s.syntax,
        text: s.text,
        tabWidth: 4,
        indentWidth: 2,
        line: 0,
        pos: s.text.length - 2,
        newLine: true,
      })?.netIndent(),
    ).toBe(1);
  });

  test("different-line regular indents stack", () => {
    const s = syntaxSetup('{\n  "a": {\n    "b": 1\n  }\n}', `(object) @indent (pair) @indent`);
    expect(
      treeSitterIndentForPos({
        query: s.query,
        syntax: s.syntax,
        text: s.text,
        tabWidth: 4,
        indentWidth: 2,
        line: 2,
        pos: s.text.sliceString(0, s.text.length).indexOf('"b"'),
        newLine: false,
      })?.netIndent(),
    ).toBe(2);
  });

  test("align uses anchor whitespace", () => {
    const s = syntaxSetup('{\n  "long": 1\n}', `(pair key: (_) @anchor) @align`);
    expect(
      treeSitterIndentForPos({
        query: s.query,
        syntax: s.syntax,
        text: s.text,
        tabWidth: 4,
        indentWidth: 2,
        line: 1,
        pos: s.text.sliceString(0, s.text.length).indexOf("1"),
        newLine: false,
      })?.toString({ indentStyle: IndentStyle.spaces(2), tabWidth: 4 }),
    ).toBe("  ");
  });

  test("hybrid uses nearby actual indentation baseline", () => {
    const s = syntaxSetup('{\n    "a": {\n      "b": 1\n    }\n}', `(object) @indent`);
    expect(
      indentForNewline({
        query: s.query,
        syntax: s.syntax,
        heuristic: "hybrid",
        indentStyle: IndentStyle.spaces(2),
        tabWidth: 4,
        text: s.text,
        lineBefore: 1,
        lineBeforeEndPos: 11,
        currentLine: 1,
      }),
    ).toBe("    ");
  });
});
