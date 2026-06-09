import { describe, expect, test } from "vitest";
import Json from "tree-sitter-json";
import {
  LanguageRegistry,
  QueryCompileError,
  type QueryKind,
  type SyntaxResources,
} from "../src/index";

const config = {
  language: [{ name: "json", scope: "source.json", fileTypes: ["json"], grammar: "json" }],
};
const registryWith = (queries: Partial<Record<QueryKind, string>>) =>
  LanguageRegistry.create(config, {
    language: (name) => (name === "json" ? Json : undefined),
    query: (_language, kind) => queries[kind],
  } satisfies SyntaxResources);
const def = (r: LanguageRegistry) => r.language(r.languageForName("json")!)!;

describe("query wrappers", () => {
  test("indent metadata and custom predicates", () => {
    const r = registryWith({
      indents: `
      ((object) @indent (#set! scope "all") (#not-kind-eq? @indent "pair") (#same-line? @indent @anchor))
      ((array) @indent.always (#set! scope "tail") (#not-same-line? @indent.always @anchor))
      ((pair) @outdent (#one-line? @outdent))
      ((string) @outdent.always (#not-one-line? @outdent.always))
      ((number) @align)
      ((true) @anchor)
      ((false) @extend)
      ((null) @extend.prevent-once)
    `,
    });
    const q = def(r).indentQuery(r)!;
    expect(q.indentCapture).toBe("indent");
    expect(q.indentAlwaysCapture).toBe("indent.always");
    expect(q.outdentCapture).toBe("outdent");
    expect(q.outdentAlwaysCapture).toBe("outdent.always");
    expect(q.alignCapture).toBe("align");
    expect(q.anchorCapture).toBe("anchor");
    expect(q.extendCapture).toBe("extend");
    expect(q.extendPreventOnceCapture).toBe("extend.prevent-once");
    expect(q.properties.get(0)).toBe("all");
    expect(q.properties.get(1)).toBe("tail");
    expect(q.predicates.get(0)?.notKindEq).toEqual([{ capture: "indent", kind: "pair" }]);
    expect(q.predicates.get(0)?.sameLine).toEqual({
      leftCapture: "indent",
      rightCapture: "anchor",
      negated: false,
    });
    expect(q.predicates.get(1)?.sameLine?.negated).toBe(true);
    expect(q.predicates.get(2)?.oneLine).toEqual({ capture: "outdent", negated: false });
    expect(q.predicates.get(3)?.oneLine).toEqual({ capture: "outdent.always", negated: true });
  });

  test("indent adjacent, bracket, multiple pattern metadata", () => {
    const adjacent = registryWith({
      indents: `(object) @indent
      (#set! scope "all")
      (#same-line? @indent @indent)`,
    });
    const aq = def(adjacent).indentQuery(adjacent)!;
    expect(aq.properties.get(0)).toBe("all");
    expect(aq.predicates.get(0)?.sameLine).toEqual({
      leftCapture: "indent",
      rightCapture: "indent",
      negated: false,
    });

    const bracket = registryWith({
      indents: `[
      (object)
      (array)
    ] @indent
    (#set! scope "all")
    (#not-kind-eq? @indent "ERROR")`,
    });
    const bq = def(bracket).indentQuery(bracket)!;
    expect(bq.properties.get(0)).toBe("all");
    expect(bq.predicates.get(0)?.notKindEq).toEqual([{ capture: "indent", kind: "ERROR" }]);

    const multi = registryWith({
      indents: `(object) @indent
      (#set! scope "all")

      (array) @outdent
      (#set! scope "tail")`,
    });
    const mq = def(multi).indentQuery(multi)!;
    expect(mq.properties.get(0)).toBe("all");
    expect(mq.properties.get(1)).toBe("tail");
  });

  test("indent JS-supported predicate variants are allowed", () => {
    const r = registryWith({
      indents: `(object) @indent
      (#any-eq? @indent "{}")
      (#any-not-match? @indent "x")`,
    });
    expect(def(r).indentQuery(r)).toBeTruthy();
  });

  test("indent invalid metadata", () => {
    expect(() => {
      const r = registryWith({ indents: `((object) @indent (#set! scope "bad"))` });
      def(r).indentQuery(r);
    }).toThrow(QueryCompileError);
    expect(() => {
      const r = registryWith({ indents: `((object) @indent (#set! scope))` });
      def(r).indentQuery(r);
    }).toThrow(QueryCompileError);
    expect(() => {
      const r = registryWith({ indents: `((object) @indent (#same-line? @a))` });
      def(r).indentQuery(r);
    }).toThrow(QueryCompileError);
    expect(() => {
      const r = registryWith({ indents: `((object) @indent (#not-kind-eq? @a @b))` });
      def(r).indentQuery(r);
    }).toThrow(QueryCompileError);
    expect(() => {
      const r = registryWith({ indents: `((object) @indent (#unknown? @a))` });
      def(r).indentQuery(r);
    }).toThrow(QueryCompileError);
  });

  test("rainbow metadata", () => {
    const r = registryWith({
      rainbows: `
      ((object) @rainbow.scope (#set! rainbow.include-children))
      ((array) @rainbow.bracket)
    `,
    });
    const q = def(r).rainbowQuery(r)!;
    expect(q.scopeCapture).toBe("rainbow.scope");
    expect(q.bracketCapture).toBe("rainbow.bracket");
    expect(q.includeChildrenPatterns.has(0)).toBe(true);
    const adjacent = registryWith({
      rainbows: `[
      (object)
      (array)
    ] @rainbow.scope
    (#set! rainbow.include-children)`,
    });
    expect(def(adjacent).rainbowQuery(adjacent)?.includeChildrenPatterns.has(0)).toBe(true);
    expect(() => {
      const bad = registryWith({
        rainbows: `((object) @rainbow.scope (#set! rainbow.include-children "yes"))`,
      });
      def(bad).rainbowQuery(bad);
    }).toThrow(QueryCompileError);
    expect(() => {
      const bad = registryWith({
        rainbows: `(object) @rainbow.scope (#rainbow-weird? @rainbow.scope)`,
      });
      def(bad).rainbowQuery(bad);
    }).toThrow(QueryCompileError);
    expect(() => {
      const bad = registryWith({ rainbows: `(object) @rainbow.scope (#set! rainbow.unknown)` });
      def(bad).rainbowQuery(bad);
    }).toThrow(QueryCompileError);
  });

  test("tag predicate compatibility", () => {
    const r = registryWith({
      tags: `
      ((pair key: (string) @name) @definition
        (#strip! @name "^\\\"|\\\"$")
        (#select-adjacent! @doc @definition)
        (#is? local))
    `,
    });
    const q = def(r).tagQuery(r)!;
    expect(q.stripPredicates.get(0)).toEqual([{ capture: "name", pattern: '^"|"$' }]);
    expect(q.selectAdjacentPredicates.get(0)).toEqual([
      { leftCapture: "doc", rightCapture: "definition" },
    ]);
    expect(q.localPatterns.has(0)).toBe(true);
    const adjacent = registryWith({
      tags: `(pair key: (string) @name) @definition
      (#strip! @name "^\\\"|\\\"$")
      (#select-adjacent! @doc @definition)
      (#is? local)

      (pair) @reference
      (#is? local)`,
    });
    const aq = def(adjacent).tagQuery(adjacent)!;
    expect(aq.stripPredicates.get(0)).toEqual([{ capture: "name", pattern: '^"|"$' }]);
    expect(aq.selectAdjacentPredicates.get(0)).toEqual([
      { leftCapture: "doc", rightCapture: "definition" },
    ]);
    expect(aq.localPatterns.has(0)).toBe(true);
    expect(aq.localPatterns.has(1)).toBe(true);
    expect(() => {
      const bad = registryWith({ tags: `((pair) @definition (#strip! @name))` });
      def(bad).tagQuery(bad);
    }).toThrow(QueryCompileError);
    expect(() => {
      const bad = registryWith({ tags: `((pair) @definition (#select-adjacent! @name))` });
      def(bad).tagQuery(bad);
    }).toThrow(QueryCompileError);
    expect(() => {
      const bad = registryWith({ tags: `((pair) @definition (#custom! @name))` });
      def(bad).tagQuery(bad);
    }).toThrow(QueryCompileError);
  });

  test("query wrappers are cached", () => {
    const r = registryWith({
      highlights: `((object) @object)`,
      indents: `((object) @indent)`,
      textobjects: `((pair) @key)`,
      tags: `((pair) @definition)`,
      rainbows: `((object) @rainbow.scope)`,
    });
    const d = def(r);
    expect(d.highlightQuery(r)).toBe(d.highlightQuery(r));
    expect(d.indentQuery(r)).toBe(d.indentQuery(r));
    expect(d.textObjectQuery(r)).toBe(d.textObjectQuery(r));
    expect(d.tagQuery(r)).toBe(d.tagQuery(r));
    expect(d.rainbowQuery(r)).toBe(d.rainbowQuery(r));
  });
});
