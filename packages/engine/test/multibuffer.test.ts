import { Effect } from "effect";
import { describe, expect, test } from "vitest";
import { ChangeSet, MultiBufferSnapshot, TextBuffer, type Excerpt, type TextEdit } from "../src/index";

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
    expect(snapshot.length).toBe(19);
    expect(snapshot.lineCount).toBe(3);
    expect(snapshot.textBetween(0, snapshot.length)).toBe("alpha\nbetatwo\nthree");
    expect(snapshot.textBetween(3, 14)).toBe("ha\nbetatwo\n");

    expect(snapshot.offsetToSource(0)).toMatchObject({ documentId: "a", offset: 0 });
    expect(snapshot.offsetToSource(6)).toMatchObject({ documentId: "a", offset: 6 });
    expect(snapshot.offsetToSource(10)).toMatchObject({ documentId: "b", offset: 4 });
    expect(snapshot.offsetToSource(10, "before")).toMatchObject({ documentId: "a", offset: 10 });
    expect(snapshot.sourceToOffset("a", 6)).toBe(6);
    expect(snapshot.sourceToOffset("b", 4)).toBe(10);

    expect(snapshot.lineToOffset(0)).toBe(0);
    expect(snapshot.lineToOffset(1)).toBe(6);
    expect(snapshot.lineToOffset(2)).toBe(14);
    expect(snapshot.offsetToLine(0)).toBe(0);
    expect(snapshot.offsetToLine(13)).toBe(1);
    expect(snapshot.offsetToLine(14)).toBe(2);

    expect(snapshot.excerptAtOffset(10)?.id).toBe("b1");
    expect(snapshot.excerptAtOffset(10, "before")?.id).toBe("a1");
    expect(snapshot.excerptBoundariesBetween(0, snapshot.length)).toEqual([
      {
        offset: 0,
        next: { id: "a1", documentId: "a", sourceRange: span(0, 10), primaryRange: span(6, 10) },
      },
      {
        offset: 10,
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
    expect(chunks).toEqual(["e1:1-6:34567", "e2:6-10:abcd"]);
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

    expect(planEntries(snapshot.editPlan([{ span: span(6, 13), text: "X" }]).documentEdits)).toEqual([
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
      oracle += buffer.sliceString(line.from, line.to);
    }
    const snapshot = MultiBufferSnapshot.fromExcerpts(new Map([["doc", buffer]]), excerpts);
    expect(snapshot.length).toBe(oracle.length);
    expect(snapshot.textBetween(0, snapshot.length)).toBe(oracle);
    expect(snapshot.offsetToSource(1234)?.documentId).toBe("doc");
    expect(snapshot.sourceToOffset("doc", buffer.lineRange(42).from)).toBe(284);
  });
});
