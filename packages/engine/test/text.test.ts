import { Effect } from "effect";
import { describe, expect, test } from "vitest";
import { TextBuffer, Transaction, unsafeTextBufferDebugState } from "../src/index";
import type { Replacement } from "../src/text/buffer";
import { makeBlock } from "../src/text/block";
import { blockPiece, inlinePiece } from "../src/text/piece";
import { addSummary, summarizeText } from "../src/text/summary";
import { coalescePieces } from "../src/text/tree";
import { seekOffset } from "../src/text/buffer";

const countLF = (s: string) => [...s].reduce((n, c) => n + (c === "\n" ? 1 : 0), 0);
const refLine = (text: string, index: number) => {
  let line = 0,
    start = 0;
  for (let i = 0; ; i++)
    if (i === text.length || text.charCodeAt(i) === 10) {
      if (line === index) {
        const breakTo = i < text.length ? i + 1 : i;
        const breakFrom = i > start && text.charCodeAt(i - 1) === 13 ? i - 1 : i;
        return { index, from: start, to: breakFrom, breakFrom, breakTo };
      }
      start = i + 1;
      line++;
    }
};
const assertDoc = (doc: TextBuffer, ref: string) => {
  expect(doc.length).toBe(ref.length);
  expect(doc.toString()).toBe(ref);
  expect(doc.lineCount).toBe(countLF(ref) + 1);
  doc.assert();
  for (let i = 0; i < doc.lineCount; i++) {
    const r = refLine(ref, i);
    expect(doc.lineRange(i)).toEqual(r);
    expect(doc.lineText(i)).toBe(ref.slice(r.from, r.to));
  }
  for (const p of [0, Math.floor(ref.length / 2), ref.length])
    expect(doc.lineRangeAt(p)).toEqual(refLine(ref, ref.slice(0, p).split("\n").length - 1));
};
const statDelta = (after: TextBuffer, before: TextBuffer) => {
  const a = after.stats().mutation;
  const b = before.stats().mutation;
  return {
    pathCopies: a.pathCopies - b.pathCopies,
    leafRebuilds: a.leafRebuilds - b.leafRebuilds,
    branchRebuilds: a.branchRebuilds - b.branchRebuilds,
    localRepackedPieces: a.localRepackedPieces - b.localRepackedPieces,
    localRepackedChildren: a.localRepackedChildren - b.localRepackedChildren,
    fullRebuilds: a.fullRebuilds - b.fullRebuilds,
    subtreeRebuilds: a.subtreeRebuilds - b.subtreeRebuilds,
    flattenedPieces: a.flattenedPieces - b.flattenedPieces,
  };
};

describe("TextBuffer", () => {
  test("fixtures", () => {
    for (const s of ["", "abc", "a\nb", "a\r\nb", "\n\n", "😀👨‍👩‍👧‍👦é", "x".repeat(70_000)])
      assertDoc(TextBuffer.fromString(s), s);
  });
  test("edits and snapshots", () => {
    let ref = "hello\r\nworld\n";
    let doc = TextBuffer.fromString(ref);
    const old = doc;
    doc = doc.replace(5, 7, "\n");
    ref = ref.slice(0, 5) + "\n" + ref.slice(7);
    doc = doc.replace(doc.length, doc.length, "!");
    ref += "!";
    assertDoc(doc, ref);
    assertDoc(old, "hello\r\nworld\n");
  });
  test("deterministic fuzz against string", () => {
    let seed = 1;
    const rnd = () => (seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32;
    let doc = TextBuffer.empty;
    let ref = "";
    const alphabet = ["a", "b", "\n", "\r\n", "😀", "́", "👨‍👩‍👧‍👦"];
    for (let i = 0; i < 200; i++) {
      const from = Math.floor(rnd() * (ref.length + 1));
      const to = from + Math.floor(rnd() * (ref.length - from + 1));
      let text = "";
      for (let j = 0, n = Math.floor(rnd() * 8); j < n; j++)
        text += alphabet[Math.floor(rnd() * alphabet.length)];
      doc = doc.replace(from, to, text);
      ref = ref.slice(0, from) + text + ref.slice(to);
      assertDoc(doc, ref);
    }
  });
  test("CRLF edge edits on a CRLF boundary", () => {
    const ref = "a\r\nb";
    const base = TextBuffer.fromString(ref);
    assertDoc(base.replace(2, 2, "X"), "a\rX\nb");
    assertDoc(base.replace(1, 2, ""), "a\nb");
    assertDoc(base.replace(2, 3, ""), "a\rb");
  });
  test("seek and line helpers at boundaries", () => {
    const doc = TextBuffer.fromChunks(
      Array.from({ length: 130 }, (_, i) => (i % 2 ? `b${i}\r\n` : `a${i}\n`)),
    );
    const ref = doc.toString();
    for (const offset of [
      0,
      1,
      ref.indexOf("\n"),
      ref.indexOf("\n") + 1,
      ref.length - 1,
      ref.length,
    ]) {
      const root = unsafeTextBufferDebugState(doc).root;
      expect(seekOffset(root, offset, -1)?.pieceOffset).toBeGreaterThanOrEqual(0);
      expect(seekOffset(root, offset, 1)?.pieceOffset).toBeGreaterThanOrEqual(0);
      expect(doc.lineRangeAt(offset)).toEqual(
        refLine(ref, ref.slice(0, offset).split("\n").length - 1),
      );
    }
  });
  test("single insert mutation stats are bounded", () => {
    const base = TextBuffer.fromChunks(Array.from({ length: 5000 }, (_, i) => `${i}\n`));
    const edited = base.replace(base.length >> 1, base.length >> 1, "x");
    assertDoc(
      edited,
      base.toString().slice(0, base.length >> 1) + "x" + base.toString().slice(base.length >> 1),
    );
    const d = statDelta(edited, base);
    expect(d.fullRebuilds).toBe(0);
    expect(d.subtreeRebuilds).toBe(0);
    expect(d.flattenedPieces).toBe(0);
    expect(d.leafRebuilds).toBeLessThanOrEqual(2);
    expect(d.localRepackedPieces).toBeLessThan(512);
    expect(d.localRepackedChildren).toBeLessThan(512);
  });
  test("two far-apart replacements do not repack broad spans", () => {
    const base = TextBuffer.fromChunks(Array.from({ length: 8000 }, (_, i) => `${i}\n`));
    const ref = base.toString();
    const changes = {
      replacements: [
        { from: 20, to: 21, text: "A" },
        { from: ref.length - 20, to: ref.length - 19, text: "B" },
      ],
    };
    const edited = base.apply(changes);
    const expected = changes.replacements.reduceRight(
      (s, r) => s.slice(0, r.from) + r.text + s.slice(r.to),
      ref,
    );
    assertDoc(edited, expected);
    const d = statDelta(edited, base);
    expect(d.fullRebuilds).toBe(0);
    expect(d.subtreeRebuilds).toBe(0);
    expect(d.flattenedPieces).toBe(0);
    expect(d.localRepackedPieces).toBeLessThan(3_000);
    expect(d.localRepackedChildren).toBeLessThan(3_000);
  });
  test("summary properties", () => {
    const atoms = [
      "",
      "\r",
      "\n",
      "\r\n",
      "abc\r",
      "\ndef",
      "😀",
      "\uD800",
      "\uDC00",
      "é",
      "👨‍👩‍👧‍👦",
      "x".repeat(300),
    ];
    for (let i = 0; i < 300; i++) {
      const a = atoms[i % atoms.length]! + atoms[(i * 3) % atoms.length]!;
      const b = atoms[(i * 5) % atoms.length]! + atoms[(i * 7) % atoms.length]!;
      const c = atoms[(i * 11) % atoms.length]!;
      const sa = summarizeText(a),
        sb = summarizeText(b),
        sc = summarizeText(c);
      expect(addSummary(sa, sb)).toEqual(summarizeText(a + b));
      expect(addSummary(addSummary(sa, sb), sc)).toEqual(addSummary(sa, addSummary(sb, sc)));
      expect(addSummary(addSummary(sa, sb), sc)).toEqual(summarizeText(a + b + c));
    }
    const texts = [
      "abc\r\ndef\n",
      "\r\n\r\n",
      "x".repeat(1000),
      Array.from({ length: 100 }, (_, i) => (i % 3 ? "a\r\n" : "😀\n")).join(""),
    ];
    for (const text of texts) {
      const blocks = [makeBlock(text)];
      for (let start = 0; start < text.length; start += Math.max(1, Math.floor(text.length / 13)))
        for (
          let end = start + 1;
          end <= text.length;
          end += Math.max(1, Math.floor(text.length / 11))
        ) {
          const p = blockPiece(0, start, end - start, blocks);
          expect({
            length: p.length,
            lineBreaks: p.lineBreaks,
            firstLineLength: p.firstLineLength,
            lastLineLength: p.lastLineLength,
            maxLineLength: p.maxLineLength,
            startsWithLF: p.startsWithLF,
            endsWithCR: p.endsWithCR,
            maxLineLengthBeforeLast: p.maxLineLengthBeforeLast,
          }).toEqual(summarizeText(text.slice(start, end)));
        }
    }
  });
  test("coalescing preserves CRLF split rich summary", () => {
    const pieces = coalescePieces([inlinePiece("abc\r"), inlinePiece("\ndef")], []);
    expect(pieces).toHaveLength(1);
    expect(pieces[0]).toMatchObject(summarizeText("abc\r\ndef"));
  });
  test("rich summaries match exact text across split CRLF", () => {
    const docs = [
      TextBuffer.fromChunks(["abc\r", "\ndef"]),
      TextBuffer.fromChunks(["\r", "\n"]),
      TextBuffer.fromChunks(["a\r", "\nb\r", "\n"]),
      TextBuffer.fromString("abc").replace(1, 1, "\r").replace(2, 2, "\n"),
      TextBuffer.fromString("a\r\nb\n").replace(1, 2, ""),
      TextBuffer.fromString("a\r\nb\n").replace(1, 3, "\n"),
    ];
    for (const doc of docs) {
      expect(unsafeTextBufferDebugState(doc).root?.summary).toEqual(summarizeText(doc.toString()));
      doc.assert();
      assertDoc(doc, doc.toString());
    }
  });
  test("line start offsets via lineColumnToOffset", () => {
    for (const s of ["", "a\r\nb", "a\n\n", "\n\n", "a\nb\n", "x\r\ny\n\nz"]) {
      const doc = TextBuffer.fromString(s);
      const starts = [0];
      for (let i = 0; i < s.length; i++) if (s.charCodeAt(i) === 10) starts.push(i + 1);
      for (let line = 0; line < starts.length; line++)
        expect(doc.lineColumnToOffset(line, 0)).toBe(starts[line]);
    }
    let frag = TextBuffer.fromString("a\nb\nc\n");
    for (let i = 0; i < 200; i++)
      frag = frag.replace(Math.floor(frag.length / 2), Math.floor(frag.length / 2), `x${i}\n`);
    const text = frag.toString();
    const starts = [0];
    for (let i = 0; i < text.length; i++) if (text.charCodeAt(i) === 10) starts.push(i + 1);
    for (let line = 0; line < starts.length; line += Math.max(1, Math.floor(starts.length / 20)))
      expect(frag.lineColumnToOffset(line, 0)).toBe(starts[line]);
  });
  test("non-materializing visible range and chunk APIs", () => {
    const docs = ["", "a\nb", "a\r\nb", "a\nb\n", "\n\n", "😀é\nabc", "x".repeat(10_000)];
    for (const s of docs) {
      const doc = TextBuffer.fromString(s);
      for (const start of [0, Math.max(0, doc.lineCount - 1), Math.floor(doc.lineCount / 2)]) {
        const ranges: ReturnType<typeof doc.lineRange>[] = [];
        doc.visibleLineRanges(start, 3, (r) => ranges.push(r));
        expect(ranges).toEqual(
          Array.from({ length: Math.min(3, doc.lineCount - start) }, (_, i) =>
            doc.lineRange(start + i),
          ),
        );
      }
      for (let i = 0; i < doc.lineCount; i++) {
        const chunks: string[] = [];
        const chunkRanges: Array<[number, number]> = [];
        doc.chunksForLine(i, (c) => {
          chunks.push(c.text);
          chunkRanges.push([c.from, c.to]);
          expect(c.from).toBeLessThanOrEqual(c.to);
        });
        const r = doc.lineRange(i);
        expect(chunks.join("")).toBe(doc.lineText(i));
        if (chunkRanges.length > 0) {
          expect(chunkRanges[0]![0]).toBe(r.from);
          expect(chunkRanges[chunkRanges.length - 1]![1]).toBe(r.to);
        }
        for (const [from, to] of chunkRanges) {
          expect(from).toBeGreaterThanOrEqual(r.from);
          expect(to).toBeLessThanOrEqual(r.to);
        }
      }
      const byLine = new Map<number, string>();
      doc.visibleChunks(0, doc.lineCount, (c) =>
        byLine.set(c.line, (byLine.get(c.line) ?? "") + c.text),
      );
      for (let i = 0; i < doc.lineCount; i++) expect(byLine.get(i) ?? "").toBe(doc.lineText(i));
    }
    let frag = TextBuffer.fromString("a\nb\nc\n");
    for (let i = 0; i < 100; i++)
      frag = frag.replace(Math.floor(frag.length / 2), Math.floor(frag.length / 2), `x${i}\n`);
    const mid = Math.floor(frag.lineCount / 2);
    const got = new Map<number, string>();
    frag.visibleChunks(mid, 20, (c) => got.set(c.line, (got.get(c.line) ?? "") + c.text));
    for (let i = mid; i < Math.min(frag.lineCount, mid + 20); i++)
      expect(got.get(i) ?? "").toBe(frag.lineText(i));
  });
  test("summary cursor visible and line-column APIs", () => {
    for (const s of ["", "abc", "a\nb", "a\r\nb", "\n\n", "x".repeat(1000), "😀\n😀"])
      TextBuffer.fromString(s).assert();
    const doc = TextBuffer.fromString("a\r\nb\n😀x");
    const c = doc.cursorAt(0)!;
    expect(c.chunk).toBe(doc.sliceString(c.chunkFrom, c.chunkTo));
    expect(doc.cursorAt(doc.length)?.chunkTo).toBe(doc.length);
    const lines: string[] = [];
    doc.visibleLines(0, doc.lineCount, (l) => lines.push(l.text));
    expect(lines).toEqual(["a", "b", "😀x"]);
    for (let off = 0; off <= doc.length; off++) {
      const lc = doc.offsetToLineColumn(off);
      expect(doc.lineColumnToOffset(lc.line, lc.column, { clamp: true })).toBe(
        Math.min(off, doc.lineRangeAt(off).to),
      );
    }
    expect(() => doc.lineColumnToOffset(0, 99)).toThrow(RangeError);
    expect(doc.lineColumnToOffset(0, 99, { clamp: true })).toBe(1);
  });
  test("apply changes", () => {
    const changes = {
      replacements: [
        { from: 1, to: 2, text: "XX" },
        { from: 4, to: 5, text: "" },
      ] satisfies Replacement[],
    };
    expect(TextBuffer.fromString("abcde").apply(changes).toString()).toBe("aXXcd");
  });
});

describe("Transaction", () => {
  test("invert restores", () => {
    const before = TextBuffer.fromString("abcdef");
    const tx = Effect.runSync(Transaction.replace(before.length, { start: 2, end: 4 }, "XYZ"));
    const after = tx.apply(before);
    const inv = Effect.runSync(tx.invert(before));
    expect(inv.apply(after).toString()).toBe("abcdef");
  });
  test("map position", () => {
    const tx = Effect.runSync(Transaction.replace(6, { start: 2, end: 5 }, "X"));
    expect(tx.mapPosition(6)).toBe(4);
    expect(tx.mapPosition(3)).toBe(3);
  });
});
