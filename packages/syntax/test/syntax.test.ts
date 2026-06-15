import { describe, expect, test } from "vitest";
import Json from "tree-sitter-json";
import {
  DuplicateExtensionError,
  HighlightKindId,
  InvalidSyntaxEditError,
  LanguageId,
  LanguageRegistry,
  NullSyntaxResources,
  RuntimeSyntax,
  Syntax,
  SyntaxState,
  childForByteRange,
  documentSyntaxAssignment,
  documentSyntaxState,
  generateSyntaxEdits,
  grammarName,
  pointToUtf16Offset,
  normalizeHighlightRuns,
  prettyPrintTree,
  queryKindFilename,
  readQueryWithInherits,
  syntaxTextFromString,
  utf16OffsetToPoint,
  utf16OffsetToUtf8ByteOffset,
  utf8ByteOffsetToUtf16Offset,
  type SyntaxResources,
} from "../src/index";

const resources: SyntaxResources = {
  language: (name) => (name === "json" ? (Json as unknown) : undefined),
  query: (languageId, kind) => {
    if (languageId === "json" && kind === "highlights") return `(string) @string\n(number) @number`;
    if (languageId === "json" && kind === "textobjects")
      return `(pair key: (string) @key value: (_) @value)`;
    return undefined;
  },
};
const config = {
  language: [
    {
      name: "json",
      scope: "source.json",
      fileTypes: ["json", { glob: "*.jsonc" }],
      shebangs: ["json"],
      grammar: "json",
      injectionRegex: /\{\s*"/,
    },
  ],
} as const;

describe("syntax text and offsets", () => {
  test("string/TextBuffer adapters and offset conversion", () => {
    const text = syntaxTextFromString("aé\n😊b");
    expect(text.length).toBe("aé\n😊b".length);
    expect(text.sliceString(1, 2)).toBe("é");
    expect(utf16OffsetToUtf8ByteOffset(text, 2)).toBe(3);
    expect(utf8ByteOffsetToUtf16Offset(text, 3)).toBe(2);
    expect(utf16OffsetToPoint(text, 4)).toEqual({ row: 1, column: 4 });
    expect(pointToUtf16Offset(text, { row: 1, column: 4 })).toBe(5);
    const multibyte = syntaxTextFromString("éx");
    expect(utf8ByteOffsetToUtf16Offset(multibyte, 1)).toBe(0);
    expect(pointToUtf16Offset(multibyte, { row: 0, column: 1 })).toBe(0);
  });
});

describe("registry and resources", () => {
  test("lookups, query names, inheritance, missing grammar", () => {
    const registry = LanguageRegistry.create(config, resources);
    expect(registry.size).toBe(1);
    expect(grammarName(config.language[0])).toBe("json");
    expect(queryKindFilename("highlights")).toBe("highlights.scm");
    expect(registry.languageForName("json")).toBeDefined();
    expect(registry.assignByFilename("foo.json")?.source).toBe("filename");
    expect(registry.assignByFilename("foo.jsonc")?.source).toBe("filename");
    expect(registry.assignByShebang(syntaxTextFromString("#!/usr/bin/env json\n{}"))?.source).toBe(
      "shebang",
    );
    expect(registry.assignByMatch(syntaxTextFromString('{"a":1}'))?.source).toBe("injectionRegex");
    expect(
      readQueryWithInherits("tsx", (id) => (id === "tsx" ? "; inherits: js\n(a) @a" : "(b) @b")),
    ).toContain("@b");
    const unavailable = LanguageRegistry.create(config, NullSyntaxResources).activateDocumentSyntax(
      syntaxTextFromString("{}"),
      { languageId: registry.languageForName("json")!, source: "name" },
    );
    expect(unavailable.type === "assigned" && unavailable.state.type === "unavailable").toBe(true);
    expect(() =>
      LanguageRegistry.create(
        {
          language: [
            config.language[0],
            { ...config.language[0], name: "json2", scope: "source.json2" },
          ],
        },
        resources,
      ),
    ).toThrow(DuplicateExtensionError);
  });
});

describe("parse, state, edits, highlights", () => {
  test("parse JSON, tree helpers, scopes", () => {
    const registry = LanguageRegistry.create(config, resources);
    const id = registry.languageForName("json")!;
    const text = syntaxTextFromString('{"é": "😊"}');
    const syntax = Syntax.parse({ text, languageId: id, registry });
    expect(syntax.rootNode().type).toBe("document");
    expect(prettyPrintTree(syntax.rootNode())).toContain("object");
    expect(syntax.descendantForByteRange(1, 2)?.type).toBeTruthy();
    expect(syntax.namedDescendantForByteRange(1, 2)?.type).toBeTruthy();
    expect(syntax.treeForByteRange(0, 1)).toBe(syntax.tree);
    const keyStart = utf16OffsetToUtf8ByteOffset(text, 2);
    expect(syntax.descendantForByteRange(keyStart, keyStart + 2)?.type).toBeTruthy();
    expect(syntax.scopesAt(text, text.length - 3)).toContain("document");
    expect(
      childForByteRange(syntax.rootNode(), { start: 1, end: syntax.rootNode().endIndex - 1 })?.type,
    ).toBeTruthy();
  });

  test("state generations and edits", () => {
    const registry = LanguageRegistry.create(config, resources);
    const id = registry.languageForName("json")!;
    const text = syntaxTextFromString('{"a":1}');
    const syntax = Syntax.parse({ text, languageId: id, registry });
    const state = SyntaxState.create(id, syntax);
    expect(state.editGeneration).toBe(0);
    const edits = generateSyntaxEdits(text, [{ start: 5, end: 6, text: "22" }]);
    expect(edits[0]!.newEndIndex).toBe(edits[0]!.startIndex + 2);
    state.interpolate(edits);
    expect(state.editGeneration).toBe(1);
    expect(state.isParsePending).toBe(true);
    expect(state.invalidation.tree.values()).toEqual(state.invalidation.baseline.values());
    expect(state.invalidation.tree.values()).toEqual(state.invalidation.enrichment.values());
    expect(state.invalidation.tree.values()[0]?.end).toBe(
      Math.max(edits[0]!.oldEndIndex, edits[0]!.newEndIndex),
    );
    const edits2 = generateSyntaxEdits(text, [{ start: 4, end: 6, text: "" }]);
    state.interpolate(edits2);
    expect(state.editGeneration).toBe(2);
    expect(state.invalidation.tree.length).toBe(1);
    const stale = state.snapshot(syntaxTextFromString('{"a":22}'));
    expect(state.applyParsed({ ...stale, editGeneration: 1 })).toBe(false);
    expect(state.applyParsed({ ...stale, languageId: LanguageId.of(99) })).toBe(false);
    const parsed = stale.syntax.reparse({ text: stale.source, registry });
    expect(state.applyParsed({ ...stale, syntax: parsed })).toBe(true);
    expect(state.treeGeneration).toBe(2);
    expect(state.baselineGeneration).toBe(2);
    expect(state.enrichmentGeneration).toBe(2);
    expect(state.isAccurate).toBe(true);
    expect(() =>
      generateSyntaxEdits(text, [
        { start: 4, end: 6, text: "" },
        { start: 5, end: 6, text: "" },
      ]),
    ).toThrow(InvalidSyntaxEditError);
  });

  test("highlight normalization and textobject queries", () => {
    const outer = HighlightKindId.of(0);
    const inner = HighlightKindId.of(1);
    expect(
      normalizeHighlightRuns([
        { byteRange: { start: 0, end: 10 }, kind: outer },
        { byteRange: { start: 3, end: 6 }, kind: inner },
      ]),
    ).toEqual([
      { byteRange: { start: 0, end: 3 }, kind: outer },
      { byteRange: { start: 3, end: 6 }, kind: inner },
      { byteRange: { start: 6, end: 10 }, kind: outer },
    ]);
    expect(
      normalizeHighlightRuns([
        { byteRange: { start: 0, end: 2 }, kind: outer },
        { byteRange: { start: 2, end: 4 }, kind: outer },
      ]),
    ).toEqual([{ byteRange: { start: 0, end: 4 }, kind: outer }]);
    expect(
      normalizeHighlightRuns([
        { byteRange: { start: 0, end: 4 }, kind: outer },
        { byteRange: { start: 0, end: 4 }, kind: inner },
      ]),
    ).toEqual([{ byteRange: { start: 0, end: 4 }, kind: inner }]);
    const registry = LanguageRegistry.create(config, resources);
    const id = registry.languageForName("json")!;
    const text = syntaxTextFromString('{"a":1}');
    const syntax = Syntax.parse({ text, languageId: id, registry });
    const runs = syntax.highlightRunsInByteRange({
      text,
      registry,
      byteRange: { start: 0, end: utf16OffsetToUtf8ByteOffset(text, text.length) },
    });
    expect(runs.length).toBeGreaterThan(0);
    expect(registry.highlightNames()).toContain("string");
    const tq = registry.language(id)!.textObjectQuery(registry)!;
    const captured = tq.captureNodes("key", syntax.rootNode(), text)[0];
    expect(captured?.type === "single" && captured.kind === "key").toBe(true);
  });
});

describe("runtime", () => {
  test("facade detects and activates documents", () => {
    expect(RuntimeSyntax.disabled().registry.isEmpty).toBe(true);
    const runtime = RuntimeSyntax.create(config, resources);
    const text = syntaxTextFromString('{"a":1}');
    const assignment = runtime.detectLanguageForPath({ path: "x.json", text });
    expect(assignment?.source).toBe("filename");
    const doc = runtime.activateDocument({ path: "x.json", text });
    expect(documentSyntaxAssignment(doc)).toBeTruthy();
    expect(documentSyntaxState(doc)?.type).toBe("active");
    if (doc.type === "assigned" && doc.state.type === "active") {
      const state = doc.state.state;
      state.interpolate(generateSyntaxEdits(text, [{ start: 5, end: 6, text: "2" }]));
      expect(state.isParsePending).toBe(true);
      expect(runtime.reparseDocument({ syntax: doc, text: syntaxTextFromString('{"a":2}') })).toBe(
        true,
      );
      expect(state.isParsePending).toBe(false);
      expect(state.invalidation.tree.isEmpty).toBe(true);
      expect(state.parsedVersion).toBe(state.editGeneration);
      expect(state.isAccurate).toBe(true);
    }
    expect(runtime.advanceDocumentSyntax({ syntax: doc, text })).toBe(true);
  });
});
