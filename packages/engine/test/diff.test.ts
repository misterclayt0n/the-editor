import { describe, expect, test } from "vitest";
import { Effect } from "effect";
import { diffReplacements, diffText, TextBuffer } from "../src/index";

const apply = (before: string, after: string): string => {
  const transaction = Effect.runSync(diffText(before, after));
  const buffer = Effect.runSync(transaction.applyTo(TextBuffer.fromString(before)));
  return buffer.sliceString(0, buffer.length);
};

describe("diffText", () => {
  test("identical inputs produce an empty transaction", () => {
    expect(diffReplacements("hello\nworld\n", "hello\nworld\n")).toEqual([]);
  });

  test("single in-line edit yields a single small replacement", () => {
    const before = "const a = 1;\nconst b = 2;\nconst c = 3;\n";
    const after = "const a = 1;\nconst b = 42;\nconst c = 3;\n";
    const replacements = diffReplacements(before, after);
    expect(replacements).toHaveLength(1);
    // Minimal: the edit touches at most the changed digits.
    expect(replacements[0]!.to - replacements[0]!.from).toBeLessThanOrEqual(2);
    expect(replacements[0]!.text.length).toBeLessThanOrEqual(2);
    expect(apply(before, after)).toBe(after);
  });

  test("line insertion and deletion produce line-sized hunks", () => {
    const before = "one\ntwo\nthree\nfour\n";
    const afterInsert = "one\ntwo\nNEW\nthree\nfour\n";
    const inserted = diffReplacements(before, afterInsert);
    expect(inserted).toHaveLength(1);
    expect(inserted[0]!.to - inserted[0]!.from).toBe(0);
    expect(inserted[0]!.text.length).toBe(4);
    const afterDelete = "one\nthree\nfour\n";
    const deleted = diffReplacements(before, afterDelete);
    expect(deleted).toHaveLength(1);
    expect(deleted[0]!.to - deleted[0]!.from).toBe(4);
    expect(deleted[0]!.text).toBe("");
    expect(apply(before, afterInsert)).toBe(afterInsert);
    expect(apply(before, afterDelete)).toBe(afterDelete);
  });

  test("multiple distant edits stay separate hunks", () => {
    const lines = Array.from({ length: 100 }, (_, i) => `line ${i}`);
    const before = lines.join("\n");
    const changed = [...lines];
    changed[10] = "line TEN";
    changed[90] = "line NINETY";
    const after = changed.join("\n");
    const replacements = diffReplacements(before, after);
    expect(replacements).toHaveLength(2);
    expect(apply(before, after)).toBe(after);
  });

  test("never splits surrogate pairs", () => {
    const before = "ab😀cd";
    const after = "ab😁cd";
    const replacements = diffReplacements(before, after);
    expect(apply(before, after)).toBe(after);
    for (const r of replacements) {
      // boundaries must not land between a surrogate pair of the source
      expect(before.charCodeAt(r.from)).not.toSatisfy(
        (c: number) => c >= 0xdc00 && c <= 0xdfff,
      );
      expect(before.charCodeAt(r.to)).not.toSatisfy((c: number) => c >= 0xdc00 && c <= 0xdfff);
    }
  });

  test("full rewrite degrades to one replacement, still correct", () => {
    const before = "completely\ndifferent\ncontent\n";
    const after = "nothing\nin\ncommon\nat\nall\n";
    expect(apply(before, after)).toBe(after);
  });

  test("accepts TextBuffer inputs", () => {
    const before = TextBuffer.fromString("alpha\nbeta\n");
    const transaction = Effect.runSync(diffText(before, "alpha\nBETA\n"));
    const result = Effect.runSync(transaction.applyTo(before));
    expect(result.sliceString(0, result.length)).toBe("alpha\nBETA\n");
  });

  test("localized edit in a huge document yields a tiny transaction", () => {
    const lines = Array.from({ length: 100_000 }, (_, i) => `const value_${i} = ${i};`);
    const before = lines.join("\n");
    const changed = [...lines];
    changed[50_000] = "const value_50000 = CHANGED;";
    const after = changed.join("\n");
    const start = performance.now();
    const replacements = diffReplacements(before, after);
    const elapsed = performance.now() - start;
    expect(replacements).toHaveLength(1);
    expect(replacements[0]!.to - replacements[0]!.from).toBeLessThan(30);
    expect(elapsed).toBeLessThan(500);
    expect(apply(before, after)).toBe(after);
  });

  test("property: applying the diff always reproduces the target", () => {
    // Deterministic pseudo-random edits over a base document.
    let seed = 42;
    const rand = (n: number) => {
      seed = (seed * 1103515245 + 12345) & 0x7fffffff;
      return seed % n;
    };
    const alphabet = "abc\nde\nf😀\n";
    for (let round = 0; round < 50; round++) {
      const length = rand(60);
      let before = "";
      for (let i = 0; i < length; i++) before += alphabet[rand(alphabet.length)];
      let after = before;
      const edits = rand(4) + 1;
      for (let e = 0; e < edits; e++) {
        const at = rand(after.length + 1);
        const del = rand(Math.max(1, after.length - at + 1));
        let insert = "";
        const insertLength = rand(6);
        for (let i = 0; i < insertLength; i++) insert += alphabet[rand(alphabet.length)];
        after = after.slice(0, at) + insert + after.slice(at + del);
      }
      expect(apply(before, after), `round ${round}`).toBe(after);
    }
  });
});
