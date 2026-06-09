import { describe, expect, test } from "vitest";
import Json from "tree-sitter-json";
import {
  LanguageRegistry,
  Syntax,
  syntaxTextFromString,
  type SyntaxResources,
} from "@the-editor/syntax";
import {
  TextBuffer,
  findEnclosingBracketPlaintext,
  findMatchingBracket,
  findMatchingBracketFuzzy,
  findMatchingBracketPlaintext,
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
const syntaxFor = (text: string) => {
  const t = syntaxTextFromString(text);
  return {
    text: t,
    syntax: Syntax.parse({ text: t, languageId: registry.languageForName("json")!, registry }),
  };
};

describe("match brackets", () => {
  test("strict matching symmetric nested plaintext brackets", () => {
    const text = syntaxTextFromString("(a[b]c)");
    expect(findMatchingBracketPlaintext(text, 0)).toBe(6);
    expect(findMatchingBracketPlaintext(text, 6)).toBe(0);
    expect(findMatchingBracketPlaintext(text, 2)).toBe(4);
    expect(findMatchingBracketPlaintext(text, 4)).toBe(2);
  });

  test("strict undefined cases", () => {
    const text = syntaxTextFromString("hi");
    expect(findMatchingBracketPlaintext(text, 0)).toBeUndefined();
    expect(findMatchingBracketPlaintext(syntaxTextFromString("["), 0)).toBeUndefined();
    expect(findMatchingBracketPlaintext(text, text.length)).toBeUndefined();
  });

  test("fuzzy nearest enclosing", () => {
    const text = syntaxTextFromString("[1,[2,3]]");
    expect(findEnclosingBracketPlaintext(text, 4)).toBe(7);
    expect(findEnclosingBracketPlaintext(text, 2)).toBe(8);
    expect(findEnclosingBracketPlaintext(text, 0)).toBe(8);
    expect(findEnclosingBracketPlaintext(text, 7)).toBe(3);
  });

  test("syntax backed with plaintext fallback", () => {
    const { text, syntax } = syntaxFor("[1,[2,3]]");
    expect(findMatchingBracket(syntax, text, 0)).toBe(8);
    expect(findMatchingBracketFuzzy(syntax, text, 4)).toBe(7);
    const unicode = syntaxFor('{"😊":[1]}');
    expect(findMatchingBracket(unicode.syntax, unicode.text, 6)).toBe(8);
    const plain = syntaxTextFromString("prefix (middle [inner]) suffix");
    const fake = syntaxFor("{}").syntax;
    expect(findMatchingBracketFuzzy(fake, plain, 16)).toBe(21);
  });

  test("unicode before brackets keeps UTF-16 offsets", () => {
    const text = syntaxTextFromString("😊(x[ｙ])");
    expect(findMatchingBracketPlaintext(text, 2)).toBe(7);
    expect(findMatchingBracketPlaintext(text, 4)).toBe(6);
  });

  test("custom pairs and validation", () => {
    const text = syntaxTextFromString("⟨x⟩");
    expect(findMatchingBracketPlaintext(text, 0, { pairs: [{ open: "⟨", close: "⟩" }] })).toBe(2);
    expect(() =>
      findMatchingBracketPlaintext(text, 0, { pairs: [{ open: "", close: ")" }] }),
    ).toThrow();
    expect(() =>
      findMatchingBracketPlaintext(text, 0, { pairs: [{ open: "ab", close: ")" }] }),
    ).toThrow();
    expect(() =>
      findMatchingBracketPlaintext(text, 0, { pairs: [{ open: "(", close: "(" }] }),
    ).toThrow();
  });

  test("scan limit, bounded slices, and TextBuffer adapter", () => {
    expect(
      findMatchingBracketPlaintext(syntaxTextFromString("(xxxx)"), 0, { maxPlaintextScan: 2 }),
    ).toBeUndefined();
    const source = "(x)";
    const bounded = {
      length: source.length,
      sliceString: (from: number, to: number) => {
        if (from === 0 && to === source.length) throw new Error("materialized full text");
        return source.slice(from, to);
      },
    };
    expect(findMatchingBracketPlaintext(bounded, 0)).toBe(2);
    const text = syntaxTextFromTextBuffer(TextBuffer.fromString("(x)"));
    expect(findMatchingBracketPlaintext(text, 0)).toBe(2);
  });
});
