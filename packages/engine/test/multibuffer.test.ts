import { Effect } from "effect";
import { describe, expect, test } from "vitest";
import {
  ChangeSet,
  expandExcerptRange,
  MultiBufferSnapshot,
  NaiveMultiBufferSnapshot,
  TextBuffer,
  type Excerpt,
  type TextEdit,
} from "../src/index";

const doc = (text: string) => TextBuffer.fromString(text);
const span = (start: number, end: number) => ({ start, end });

const planEntries = (edits: ReadonlyMap<string, readonly TextEdit[]>) =>
  [...edits].map(([id, list]) => [id, list] as const);

describe("MultiBufferSnapshot", () => {
  test("projects excerpts and maps offsets, source positions, lines, and boundaries", () => {
    const a = doc("alpha\nbeta\ngamma\n");
    const b = doc("one\ntwo\nthree");
    const snapshot = MultiBufferSnapshot.fromExcerpts(
      new Map([
        ["a", a],
        ["b", b],
      ]),
      [
        { id: "a1", documentId: "a", sourceRange: span(0, 10), primaryRange: span(6, 10) },
        { id: "b1", documentId: "b", sourceRange: span(4, 13), primaryRange: span(4, 7) },
      ],
    );

    expect(snapshot.excerptCount).toBe(2);
    expect(snapshot.length).toBe(20);
    expect(snapshot.lineCount).toBe(4);
    expect(snapshot.textBetween(0, snapshot.length)).toBe("alpha\nbeta\ntwo\nthree");
    expect(snapshot.textBetween(3, 14)).toBe("ha\nbeta\ntwo");

    expect(snapshot.offsetToSource(0)).toMatchObject({ documentId: "a", offset: 0 });
    expect(snapshot.offsetToSource(6)).toMatchObject({ documentId: "a", offset: 6 });
    // Offset 10 is a1's synthetic separator; the b excerpt begins at 11.
    expect(snapshot.offsetToSource(10)).toMatchObject({ documentId: "a", offset: 10 });
    expect(snapshot.offsetToSource(11)).toMatchObject({ documentId: "b", offset: 4 });
    expect(snapshot.offsetToSource(11, "before")).toMatchObject({ documentId: "a", offset: 10 });
    expect(snapshot.sourceToOffset("a", 6)).toBe(6);
    expect(snapshot.sourceToOffset("b", 4)).toBe(11);

    expect(snapshot.lineToOffset(0)).toBe(0);
    expect(snapshot.lineToOffset(1)).toBe(6);
    expect(snapshot.lineToOffset(2)).toBe(11);
    expect(snapshot.lineToOffset(3)).toBe(15);
    expect(snapshot.offsetToLine(0)).toBe(0);
    expect(snapshot.offsetToLine(10)).toBe(1);
    expect(snapshot.offsetToLine(13)).toBe(2);
    expect(snapshot.offsetToLine(15)).toBe(3);

    expect(snapshot.excerptAtOffset(11)?.id).toBe("b1");
    expect(snapshot.excerptAtOffset(11, "before")?.id).toBe("a1");
    expect(snapshot.excerptBoundariesBetween(0, snapshot.length)).toEqual([
      {
        offset: 0,
        next: { id: "a1", documentId: "a", sourceRange: span(0, 10), primaryRange: span(6, 10) },
      },
      {
        offset: 11,
        previous: { id: "a1", documentId: "a", sourceRange: span(0, 10), primaryRange: span(6, 10) },
        next: { id: "b1", documentId: "b", sourceRange: span(4, 13), primaryRange: span(4, 7) },
      },
    ]);
  });

  test("iterates visible chunks without concatenating whole excerpts", () => {
    const buffer = doc("0123456789\nabcdefghij\n");
    const snapshot = MultiBufferSnapshot.fromExcerpts(new Map([["doc", buffer]]), [
      { id: "e1", documentId: "doc", sourceRange: span(2, 8), primaryRange: span(2, 8) },
      { id: "e2", documentId: "doc", sourceRange: span(11, 16), primaryRange: span(12, 15) },
    ]);
    const chunks: string[] = [];
    snapshot.chunksBetween(1, 10, (chunk) => chunks.push(`${chunk.excerpt.id}:${chunk.from}-${chunk.to}:${chunk.text}`));
    expect(chunks).toEqual(["e1:1-6:34567", "e1:6-7:\n", "e2:7-10:abc"]);
  });

  test("plans multibuffer edits as per-document edits", () => {
    const a = doc("alpha\nbeta\n");
    const b = doc("one\ntwo\nthree");
    const snapshot = MultiBufferSnapshot.fromExcerpts(
      new Map([
        ["a", a],
        ["b", b],
      ]),
      [
        { id: "a1", documentId: "a", sourceRange: span(0, 10), primaryRange: span(6, 10) },
        { id: "b1", documentId: "b", sourceRange: span(4, 13), primaryRange: span(4, 7) },
      ],
    );

    expect(planEntries(snapshot.editPlan([{ span: span(1, 1), text: "!" }]).documentEdits)).toEqual([
      ["a", [{ span: span(1, 1), text: "!" }]],
    ]);

    expect(planEntries(snapshot.editPlan([{ span: span(6, 14), text: "X" }]).documentEdits)).toEqual([
      ["a", [{ span: span(6, 10), text: "X" }]],
      ["b", [{ span: span(4, 7), text: "" }]],
    ]);
  });

  test("remaps excerpts through source document changes", () => {
    const before = doc("abc\ndef");
    const change = Effect.runSync(ChangeSet.fromReplacements(before.length, [{ from: 1, to: 1, text: "X" }]));
    const after = change.apply(before);
    const snapshot = MultiBufferSnapshot.fromExcerpts(new Map([["doc", before]]), [
      { id: "e", documentId: "doc", sourceRange: span(0, before.length), primaryRange: span(4, 7) },
    ]);

    const mapped = snapshot.mapDocumentChanges(new Map([["doc", change.desc()]]), new Map([["doc", after]]));
    expect(mapped.textBetween(0, mapped.length)).toBe("aXbc\ndef");
    expect(mapped.excerpts()[0]).toEqual({
      id: "e",
      documentId: "doc",
      sourceRange: span(0, 8),
      primaryRange: span(5, 8),
    });
  });

  test("large trees agree with a string oracle", () => {
    const buffer = doc(Array.from({ length: 2_500 }, (_, i) => `line-${i}\n`).join(""));
    const excerpts: Excerpt[] = [];
    let oracle = "";
    for (let i = 0; i < 2_000; i++) {
      const line = buffer.lineRange(i);
      excerpts.push({ id: `e${i}`, documentId: "doc", sourceRange: span(line.from, line.to), primaryRange: span(line.from, line.to) });
      oracle += (i > 0 ? "\n" : "") + buffer.sliceString(line.from, line.to);
    }
    const snapshot = MultiBufferSnapshot.fromExcerpts(new Map([["doc", buffer]]), excerpts);
    expect(snapshot.length).toBe(oracle.length);
    expect(snapshot.textBetween(0, snapshot.length)).toBe(oracle);
    expect(snapshot.offsetToSource(1234)?.documentId).toBe("doc");
    expect(snapshot.sourceToOffset("doc", buffer.lineRange(42).from)).toBe(326);
  });

  test("line mapping matches the naive string oracle with separators", () => {
    // Excerpts that do NOT end on a newline: the synthetic separators are the
    // only thing keeping each excerpt on its own row, so the tree's line math
    // must agree with a naive scan of the (separator-joined) flattened text.
    const a = doc("alpha beta\ngamma delta\nepsilon\n");
    const b = doc("one two three\nfour five\nsix seven eight\n");
    const snapshot = MultiBufferSnapshot.fromExcerpts(
      new Map([
        ["a", a],
        ["b", b],
      ]),
      [
        { id: "a1", documentId: "a", sourceRange: span(0, 10), primaryRange: span(0, 5) },
        { id: "b1", documentId: "b", sourceRange: span(0, 13), primaryRange: span(0, 3) },
        { id: "a2", documentId: "a", sourceRange: span(11, 29), primaryRange: span(11, 16) },
      ],
    );
    const naive = new NaiveMultiBufferSnapshot(snapshot);
    expect(snapshot.length).toBe(naive.length);
    for (let line = 0; line < snapshot.lineCount; line++)
      expect(snapshot.lineToOffset(line)).toBe(naive.lineToOffset(line));
    for (let offset = 0; offset <= snapshot.length; offset++)
      expect(snapshot.offsetToLine(offset)).toBe(naive.offsetToLine(offset));
    // RenderableText surface: each line slices to its content (separator excluded).
    const lines = naive.textBetween(0, snapshot.length).split("\n");
    expect(snapshot.lineCount).toBe(lines.length);
    for (let line = 0; line < snapshot.lineCount; line++) {
      const range = snapshot.lineRange(line);
      expect(snapshot.sliceString(range.from, range.to)).toBe(lines[line]);
      expect(snapshot.lineRangeAt(range.from)).toEqual(range);
    }
  });
});

describe("expandExcerptRange", () => {
  const buffer = doc("l0\nl1\nl2\nl3\nl4\nl5\n");
  const lineSpan = (line: number) => ({
    start: buffer.lineRange(line).from,
    end: buffer.lineRange(line).to,
  });

  test("grows line-aligned in each direction", () => {
    expect(expandExcerptRange(buffer, lineSpan(2), "both", 1)).toEqual({
      start: buffer.lineRange(1).from,
      end: buffer.lineRange(3).to,
    });
    expect(expandExcerptRange(buffer, lineSpan(2), "up", 1)).toEqual({
      start: buffer.lineRange(1).from,
      end: buffer.lineRange(2).to,
    });
    expect(expandExcerptRange(buffer, lineSpan(2), "down", 1)).toEqual({
      start: buffer.lineRange(2).from,
      end: buffer.lineRange(3).to,
    });
  });

  test("clamps to the buffer bounds", () => {
    expect(expandExcerptRange(buffer, lineSpan(0), "up", 5).start).toBe(0);
    expect(expandExcerptRange(buffer, lineSpan(5), "down", 5).end).toBe(buffer.lineRange(6).to);
  });
});
