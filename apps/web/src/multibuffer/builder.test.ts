import { describe, expect, test } from "vitest";
import { TextBuffer, type Span } from "@the-editor/engine";
import { buildMultiBuffer, type MultiBufferMatchFile } from "./builder";

const lines = (n: number) => Array.from({ length: n }, (_, i) => `line ${i} target`).join("\n");

const file = (path: string, text: string, matchLines: number[]): MultiBufferMatchFile => {
  const buffer = TextBuffer.fromString(text);
  const matches: Span[] = matchLines.map((line) => {
    const lr = buffer.lineRange(line);
    const col = buffer.sliceString(lr.from, lr.to).indexOf("target");
    return { start: lr.from + col, end: lr.from + col + "target".length };
  });
  return { path, documentId: `file:${path}`, buffer, matches };
};

describe("buildMultiBuffer", () => {
  test("expands matches to context windows and keeps disjoint ones separate", () => {
    const build = buildMultiBuffer([file("src/a.ts", lines(20), [2, 15])], { contextLines: 2 });
    expect(build.snapshot.excerptCount).toBe(2);
    const [first, second] = build.metas;
    expect(first!.isFirstOfFile).toBe(true);
    expect(second!.isFirstOfFile).toBe(false); // same file -> divider, not a new card
    expect(first!.fileName).toBe("a.ts");
    expect(first!.dir).toBe("src");
    // Context 2 around line 2 -> lines 0..4; around line 15 -> lines 13..17.
    const buffer = TextBuffer.fromString(lines(20));
    expect(first!.excerpt.sourceRange).toEqual({ start: 0, end: buffer.lineRange(4).to });
    expect(second!.excerpt.sourceRange).toEqual({ start: buffer.lineRange(13).from, end: buffer.lineRange(17).to });
  });

  test("merges overlapping/adjacent windows into one excerpt", () => {
    const build = buildMultiBuffer([file("src/a.ts", lines(20), [2, 4])], { contextLines: 2 });
    // Windows [0..4] and [2..6] overlap -> a single merged excerpt with both matches.
    expect(build.snapshot.excerptCount).toBe(1);
    expect(build.metas[0]!.matches).toHaveLength(2);
    const buffer = TextBuffer.fromString(lines(20));
    expect(build.metas[0]!.excerpt.sourceRange).toEqual({ start: 0, end: buffer.lineRange(6).to });
  });

  test("each excerpt begins on its own row in the unified buffer", () => {
    const build = buildMultiBuffer(
      [file("src/a.ts", lines(20), [2, 15]), file("src/b.ts", lines(10), [5])],
      { contextLines: 1 },
    );
    // Two files -> both first excerpts start a new file card.
    expect(build.metas.filter((m) => m.isFirstOfFile)).toHaveLength(2);
    // The synthetic separator guarantees startOffset is a line start.
    for (const meta of build.metas) {
      const line = build.snapshot.offsetToLine(meta.startOffset);
      expect(build.snapshot.lineToOffset(line)).toBe(meta.startOffset);
    }
  });

  test("clamps matches into the excerpt source range", () => {
    const build = buildMultiBuffer([file("src/a.ts", lines(8), [4])], { contextLines: 1 });
    const meta = build.metas[0]!;
    for (const match of meta.matches) {
      expect(match.start).toBeGreaterThanOrEqual(meta.excerpt.sourceRange.start);
      expect(match.end).toBeLessThanOrEqual(meta.excerpt.sourceRange.end);
    }
  });
});
