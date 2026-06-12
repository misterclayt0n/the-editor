import { describe, expect, test } from "vitest";
import { createRequire } from "node:module";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import {
  createNodeSyntaxResources,
  generatedLanguages,
  generateSyntaxEdits,
  LanguageRegistry,
  syntaxTextFromString,
} from "../src/index";

const require_ = createRequire(import.meta.url);
const runtimeDir = join(homedir(), ".config", "the-editor", "runtime");
const haveRuntime = existsSync(join(runtimeDir, "grammars"));

const { loadLanguage } = require_("@the-editor/grammar-loader") as {
  loadLanguage: (path: string, symbol: string) => { language: unknown; abiVersion: number };
};

const registry = LanguageRegistry.create(
  generatedLanguages,
  createNodeSyntaxResources({ runtimeDir, loadLanguage }),
);

const highlightNames = (code: string, path: string): ReadonlyArray<[string, string]> => {
  const text = syntaxTextFromString(code);
  const assignment = registry.detectLanguageForDocument({ path, text });
  expect(assignment).toBeDefined();
  const doc = registry.activateDocumentSyntax(text, assignment!);
  expect(doc.type).toBe("assigned");
  if (doc.type !== "assigned" || doc.state.type !== "active") throw new Error("not active");
  return doc.state.state
    .syntax()
    .highlightRunsInByteRange({ text, registry, byteRange: { start: 0, end: code.length } })
    .map((run) => [
      code.slice(run.byteRange.start, run.byteRange.end),
      registry.highlightName(run.kind)!,
    ]);
};

describe("generated registry + dlopen grammar store", () => {
  test("full helix configuration constructs and detects languages", () => {
    expect(registry.size).toBeGreaterThan(300);
    for (const [path, name] of [
      ["a.ts", "typescript"],
      ["a.tsx", "tsx"],
      ["a.rs", "rust"],
      ["a.json", "json"],
      ["Cargo.toml", "toml"],
    ] as const) {
      const id = registry.languageForFilename(path);
      expect(id, path).toBeDefined();
      expect(registry.language(id!)!.config.name).toBe(name);
    }
  });

  test.runIf(haveRuntime)("typescript highlights through .so grammar + helix queries", () => {
    const runs = highlightNames(`const x: string = "hi"; // note\n`, "/p/a.ts");
    const byText = new Map(runs);
    expect(byText.get("const")).toMatch(/^keyword/);
    expect(byText.get('"hi"')).toBe("string");
    expect(byText.get("// note")).toBe("comment");
    expect(byText.get("string")).toBe("type.builtin");
  });

  test.runIf(haveRuntime)("rust highlights through .so grammar", () => {
    const runs = highlightNames(`fn main() { let s = "x"; }\n`, "/p/a.rs");
    const byText = new Map(runs);
    expect(byText.get("fn")).toMatch(/^keyword/);
    expect(byText.get('"x"')).toBe("string");
  });

  test.runIf(haveRuntime)("windowed query stays windowed on a large rust file", () => {
    const big = Array.from(
      { length: 5000 },
      (_, i) => `fn item_${i}() -> u32 { let v = "s${i}"; ${i} }`,
    ).join("\n");
    const text = syntaxTextFromString(big);
    const assignment = registry.detectLanguageForDocument({ path: "/p/big.rs", text })!;
    const doc = registry.activateDocumentSyntax(text, assignment);
    if (doc.type !== "assigned" || doc.state.type !== "active") throw new Error("inactive");
    const mid = Math.floor(big.length / 2);
    const start = performance.now();
    const runs = doc.state.state
      .syntax()
      .highlightRunsInByteRange({ text, registry, byteRange: { start: mid, end: mid + 2000 } });
    const elapsed = performance.now() - start;
    expect(runs.length).toBeGreaterThan(10);
    for (const run of runs) {
      expect(run.byteRange.start).toBeGreaterThanOrEqual(mid);
      expect(run.byteRange.end).toBeLessThanOrEqual(mid + 2000);
    }
    // Whole-tree captures on 5k lines costs hundreds of ms; windowed must
    // stay far under that.
    expect(elapsed).toBeLessThan(50);
  });

  test.runIf(haveRuntime)("interpolate + budgeted sync reparse (Zed loop)", () => {
    const before = `const a = 1;\n`;
    const after = `const ab = 1;\n`;
    const text = syntaxTextFromString(before);
    const assignment = registry.detectLanguageForDocument({ path: "/p/a.ts", text })!;
    const doc = registry.activateDocumentSyntax(text, assignment);
    if (doc.type !== "assigned" || doc.state.type !== "active") throw new Error("inactive");
    const state = doc.state.state;
    state.interpolate(generateSyntaxEdits(text, [{ start: 7, end: 7, text: "b" }]));
    expect(state.needsReparse).toBe(true);
    const snapshot = state.snapshot(syntaxTextFromString(after));
    const reparsed = snapshot.syntax.tryReparse({
      text: snapshot.source,
      registry,
      timeoutMs: 3,
    })!;
    expect(reparsed).toBeDefined();
    expect(state.applyParsed({ ...snapshot, syntax: reparsed })).toBe(true);
    expect(state.isAccurate).toBe(true);
  });

  test.runIf(haveRuntime)("unknown grammar stays gracefully unavailable", () => {
    const text = syntaxTextFromString("whatever\n");
    const id = registry.languageForName("wren");
    if (id === undefined) return;
    const activated = registry.activateSyntax(text, { languageId: id, source: "name" });
    expect(activated.type).toBe("unavailable");
  });
});

describe("document language detection", () => {
  test("plain text never claims a language from content words", () => {
    // Regression: elixir's injection-regex "(elixir|ex)" matched the word
    // "Etext" in prose and highlighted a .txt file as elixir. Injection
    // regexes identify injection sites, never documents.
    const prose = syntaxTextFromString(
      "This is the 100th Etext file presented by Project Gutenberg.\nfor while and\n",
    );
    expect(registry.detectLanguageForDocument({ path: "/p/t8.shakespeare.txt", text: prose })).toBeUndefined();
    expect(registry.detectLanguageForDocument({ text: prose })).toBeUndefined();
  });

  test("shebang detection still works", () => {
    const script = syntaxTextFromString("#!/usr/bin/env bash\necho hi\n");
    const assignment = registry.detectLanguageForDocument({ path: "/p/run", text: script });
    expect(assignment?.source).toBe("shebang");
    expect(registry.language(assignment!.languageId)!.config.name).toBe("bash");
  });
});
