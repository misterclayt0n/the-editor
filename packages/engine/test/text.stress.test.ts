import { describe, expect, test } from "vitest";
import { TextBuffer, unsafeTextBufferDebugState } from "../src/index";
import type { Replacement } from "../src/text/buffer";

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
  for (const p of [0, Math.floor(ref.length / 2), ref.length])
    expect(doc.lineRangeAt(p)).toEqual(refLine(ref, ref.slice(0, p).split("\n").length - 1));
};
const fragmentDoc = (edits: number): [TextBuffer, string] => {
  let doc = TextBuffer.fromString(Array.from({ length: 2000 }, (_, i) => `line${i}\n`).join(""));
  let ref = doc.toString();
  let seed = 11;
  const rnd = () => (seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32;
  for (let i = 0; i < edits; i++) {
    const from = Math.floor(rnd() * (ref.length + 1));
    const to = from + Math.min(ref.length - from, Math.floor(rnd() * 4));
    const text = i % 5 === 0 ? `\n${i}` : "x";
    doc = doc.replace(from, to, text);
    ref = ref.slice(0, from) + text + ref.slice(to);
  }
  return [doc, ref];
};
const sampleBoundaries = (xs: readonly number[], count: number, length: number): number[] => {
  const clamped = xs.map((x) => Math.max(0, Math.min(length, x))).sort((a, b) => a - b);
  const n = Math.max(1, Math.floor(count / 3));
  const mid = Math.max(0, Math.floor(clamped.length / 2) - n);
  return [
    ...new Set([...clamped.slice(0, n), ...clamped.slice(mid, mid + 2 * n), ...clamped.slice(-n)]),
  ].sort((a, b) => a - b);
};
const applyOracle = (text: string, replacements: readonly Replacement[]): string =>
  replacements.reduceRight((s, r) => s.slice(0, r.from) + r.text + s.slice(r.to), text);
const assertNoFallback = (doc: TextBuffer): void => {
  const m = doc.stats().mutation;
  expect(m.fullRebuilds).toBe(0);
  expect(m.subtreeRebuilds).toBe(0);
  expect(m.flattenedPieces).toBe(0);
};
const collectBoundaries = (doc: TextBuffer) => {
  const piece = new Set<number>([0, doc.length]);
  const leaf = new Set<number>([0, doc.length]);
  const branch = new Set<number>([0, doc.length]);
  const walk = (
    node: ReturnType<typeof unsafeTextBufferDebugState>["root"],
    base: number,
  ): void => {
    if (!node) return;
    if (node.kind === "leaf") {
      leaf.add(base);
      let p = base;
      for (const item of node.pieces) {
        piece.add(p);
        p += item.length;
        piece.add(p);
      }
      leaf.add(p);
    } else {
      branch.add(base);
      let p = base;
      for (const child of node.children) {
        walk(child, p);
        p += child.summary.length;
        branch.add(p);
      }
    }
  };
  walk(unsafeTextBufferDebugState(doc).root, 0);
  return {
    piece: [...piece].sort((a, b) => a - b),
    leaf: [...leaf].sort((a, b) => a - b),
    branch: [...branch].sort((a, b) => a - b),
  };
};

describe("TextBuffer stress", () => {
  test("boundary edits on height-2 and height-3 trees", () => {
    for (const chunks of [
      Array.from({ length: 2100 }, (_, i) => `${i}\n`),
      Array.from({ length: 70_000 }, (_, i) => `${i % 10}`),
    ]) {
      const original = chunks.join("");
      const base = TextBuffer.fromChunks(chunks);
      expect(base.stats().height).toBeGreaterThanOrEqual(chunks.length > 10_000 ? 3 : 2);
      for (const p of [
        0,
        1,
        63,
        64,
        65,
        Math.floor(base.length / 2),
        base.length - 1,
        base.length,
      ]) {
        const at = Math.max(0, Math.min(base.length, p));
        assertDoc(
          base.replace(at, at, "Z\r\n"),
          original.slice(0, at) + "Z\r\n" + original.slice(at),
        );
      }
      assertDoc(base, original);
    }
  });
  test("real piece/leaf/branch boundary offsets", () => {
    const base = TextBuffer.fromChunks(Array.from({ length: 3000 }, (_, i) => `${i}\n`));
    const ref = base.toString();
    const b = collectBoundaries(base);
    const offsets = [
      ...sampleBoundaries(b.piece, 9, ref.length),
      ...sampleBoundaries(b.leaf, 9, ref.length),
      ...sampleBoundaries(b.branch, 9, ref.length),
      Math.floor(ref.length / 2),
      ref.length,
    ];
    for (const at of offsets) {
      const pos = Math.max(0, Math.min(ref.length, at));
      assertDoc(base.replace(pos, pos, "I"), ref.slice(0, pos) + "I" + ref.slice(pos));
      const delTo = Math.min(ref.length, pos + 2);
      assertDoc(base.replace(pos, delTo, ""), ref.slice(0, pos) + ref.slice(delTo));
      if (pos < ref.length)
        assertDoc(base.replace(pos, delTo, "R\n"), ref.slice(0, pos) + "R\n" + ref.slice(delTo));
    }
    const rangeCases: Array<[number, number, string]> = [
      [b.piece[1] ?? 0, b.piece[3] ?? 0, "two-pieces"],
      [b.leaf[1] ?? 0, b.leaf[2] ?? ref.length, "two-leaves"],
      [b.leaf[1] ?? 0, b.leaf[5] ?? ref.length, "many-leaves"],
      [b.branch[1] ?? 0, b.branch[2] ?? ref.length, "branch-boundary"],
    ];
    for (const [from, to, label] of rangeCases) {
      if (from > to) continue;
      assertDoc(base.replace(from, to, ""), ref.slice(0, from) + ref.slice(to));
      assertDoc(
        base.replace(from, to, `${label}\r\n`),
        ref.slice(0, from) + `${label}\r\n` + ref.slice(to),
      );
    }
    assertDoc(base, ref);
  });
  test("fragmented density after 10k and 100k edits", () => {
    for (const edits of [10_000, 100_000]) {
      const [doc, ref] = fragmentDoc(edits);
      assertDoc(doc, ref);
      const stats = doc.stats();
      expect(stats.mutation.fullRebuilds).toBe(0);
      expect(stats.mutation.subtreeRebuilds).toBe(0);
      expect(stats.mutation.flattenedPieces).toBe(0);
      expect(stats.avgLeafPieces).toBeGreaterThanOrEqual(20);
      expect(stats.mutation.localRepackedPieces).toBeLessThan(
        edits === 10_000 ? 5_000_000 : 50_000_000,
      );
      expect(stats.mutation.localRepackedChildren).toBeLessThan(
        edits === 10_000 ? 3_000_000 : 40_000_000,
      );
    }
  }, 20_000);
  test("same-position inserts and exact leaf/branch boundary edits", () => {
    const base = TextBuffer.fromChunks(Array.from({ length: 4096 }, (_, i) => `${i}\n`));
    const ref = base.toString();
    const b = collectBoundaries(base);
    for (const at of [...new Set([...b.leaf, ...b.branch])].slice(0, 40)) {
      const changes = {
        replacements: [
          { from: at, to: at, text: "A" },
          { from: at, to: at, text: "B" },
          { from: at, to: at, text: "C" },
        ],
      };
      assertDoc(base.apply(changes), ref.slice(0, at) + "ABC" + ref.slice(at));
      const to = Math.min(ref.length, at + 1);
      assertDoc(base.replace(at, to, ""), ref.slice(0, at) + ref.slice(to));
      if (at < ref.length)
        assertDoc(base.replace(at, to, "Z"), ref.slice(0, at) + "Z" + ref.slice(to));
    }
    assertDoc(base, ref);
  }, 20_000);
  test("CRLF edge fuzz", () => {
    const parts = ["\r\n", "\n", "\r", "", "abc", "😀"];
    for (let seed = 0; seed < 40; seed++) {
      let text = "";
      for (let i = 0; i < 20; i++) text += parts[(seed * 7 + i * 3) % parts.length];
      const offsets: number[] = [];
      for (let i = 0; i < text.length - 1; i++)
        if (text[i] === "\r" && text[i + 1] === "\n") offsets.push(i, i + 1, i + 2);
      for (const at of [...new Set(offsets)].slice(0, 12)) {
        const base = TextBuffer.fromString(text);
        const cases: Array<[number, number, string]> = [
          [at, at, "X"],
          [at, at, "\r\n"],
          [at, at, "\n"],
          [at, Math.min(text.length, at + 1), ""],
          [at, Math.min(text.length, at + 2), ""],
          [at, Math.min(text.length, at + 1), "R"],
          [at, Math.min(text.length, at + 2), "\n"],
        ];
        for (const [from, to, insert] of cases)
          assertDoc(base.replace(from, to, insert), text.slice(0, from) + insert + text.slice(to));
      }
    }
  });
  test("bulk far-apart and dense distributed replacements", () => {
    const [fragmented, ref] = fragmentDoc(10_000);
    const far = {
      replacements: [
        { from: 5, to: 8, text: "S\r\n" },
        { from: ref.length >> 1, to: (ref.length >> 1) + 3, text: "M" },
        { from: ref.length - 8, to: ref.length - 5, text: "E\n" },
      ],
    };
    const farExpected = far.replacements.reduceRight(
      (s, r) => s.slice(0, r.from) + r.text + s.slice(r.to),
      ref,
    );
    const farDoc = fragmented.apply(far);
    assertDoc(farDoc, farExpected);
    expect(farDoc.stats().mutation.fullRebuilds).toBe(0);
    expect(farDoc.stats().mutation.flattenedPieces).toBe(0);
    const step = Math.max(2, Math.floor(ref.length / 1001));
    const replacements = Array.from({ length: 1000 }, (_, i) => ({
      from: i * step,
      to: Math.min(ref.length, i * step + 1),
      text: i % 10 === 0 ? "\r\n" : "x",
    }));
    const expected = replacements.reduceRight(
      (s, r) => s.slice(0, r.from) + r.text + s.slice(r.to),
      ref,
    );
    const doc = fragmented.apply({ replacements });
    assertDoc(doc, expected);
    expect(doc.stats().mutation.fullRebuilds).toBe(0);
    expect(doc.stats().mutation.flattenedPieces).toBe(0);
  });
  test("bulk edge cases: adjacent replacements and same-position inserts", () => {
    const ref = "0123456789\r\nabcdefghij\nklmnopqrst";
    const base = TextBuffer.fromString(ref);
    const cases: Replacement[][] = [
      [
        { from: 2, to: 5, text: "A" },
        { from: 5, to: 8, text: "B" },
      ],
      [
        { from: 2, to: 5, text: "" },
        { from: 5, to: 5, text: "I" },
      ],
      [
        { from: 2, to: 5, text: "RR" },
        { from: 5, to: 8, text: "" },
      ],
      [
        { from: 10, to: 10, text: "A" },
        { from: 10, to: 10, text: "B" },
        { from: 10, to: 10, text: "C" },
      ],
      [
        { from: 9, to: 10, text: "" },
        { from: 10, to: 10, text: "A" },
        { from: 10, to: 10, text: "B" },
        { from: 10, to: 11, text: "R" },
      ],
      [{ from: 8, to: 13, text: "X\r\nY" }],
    ];
    for (const replacements of cases) {
      const doc = base.apply({ replacements });
      assertDoc(doc, applyOracle(ref, replacements));
      assertDoc(base, ref);
      assertNoFallback(doc);
    }
  });
  test("branch-boundary replacement and insert edge cases", () => {
    const base = TextBuffer.fromChunks(Array.from({ length: 5000 }, (_, i) => `${i}\n`));
    const ref = base.toString();
    const boundaries = sampleBoundaries(collectBoundaries(base).branch, 12, ref.length).filter(
      (x) => x > 0 && x < ref.length,
    );
    for (const at of boundaries) {
      const cases: Replacement[][] = [
        [{ from: at, to: at, text: "I\n" }],
        [{ from: Math.max(0, at - 3), to: Math.min(ref.length, at + 3), text: "R" }],
        [{ from: Math.max(0, at - 3), to: at, text: "" }],
        [{ from: at, to: Math.min(ref.length, at + 3), text: "" }],
      ];
      for (const replacements of cases) {
        const doc = base.apply({ replacements });
        assertDoc(doc, applyOracle(ref, replacements));
        assertNoFallback(doc);
      }
    }
    assertDoc(base, ref);
  });
  test("whole-document replace on fragmented doc", () => {
    const [base, ref] = fragmentDoc(2_000);
    for (const text of ["", "small", "a\r\nb\r\nc", "L".repeat(80_000)]) {
      const doc = base.replace(0, base.length, text);
      assertDoc(doc, text);
      assertDoc(base, ref);
      assertNoFallback(doc);
    }
  });
  test("empty document apply with multiple inserts", () => {
    const replacements = [
      { from: 0, to: 0, text: "A" },
      { from: 0, to: 0, text: "B" },
      { from: 0, to: 0, text: "C\r\n" },
      { from: 0, to: 0, text: "D" },
    ];
    const doc = TextBuffer.empty.apply({ replacements });
    assertDoc(doc, "ABC\r\nD");
    assertNoFallback(doc);
  });
  test("random edit property with snapshots and visible/chunk oracles", () => {
    const atoms = ["a", "bc", "\n", "\r\n", "\r", "😀", "é", "👨‍👩‍👧‍👦", "\uD800", "\uDC00"];
    let seed = 202405;
    const rnd = () => (seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32;
    const checkRich = (doc: TextBuffer, ref: string): void => {
      assertDoc(doc, ref);
      for (let k = 0; k < 12; k++) {
        const a = Math.floor(rnd() * (ref.length + 1));
        const b = a + Math.floor(rnd() * (ref.length - a + 1));
        const chunks: string[] = [];
        doc.chunksBetween(a, b, (c) => chunks.push(c));
        expect(doc.sliceString(a, b)).toBe(ref.slice(a, b));
        expect(chunks.join("")).toBe(ref.slice(a, b));
        const pos = Math.floor(rnd() * (ref.length + 1));
        const line = ref.slice(0, pos).split("\n").length - 1;
        expect(doc.lineRangeAt(pos)).toEqual(refLine(ref, line));
        const r = doc.lineRange(line);
        expect(doc.lineText(line)).toBe(ref.slice(r.from, r.to));
        const lc = doc.offsetToLineColumn(pos);
        expect(lc.line).toBe(line);
        expect(doc.lineColumnToOffset(lc.line, lc.column, { clamp: true })).toBe(
          Math.min(pos, r.to),
        );
      }
      const start = Math.floor(rnd() * doc.lineCount);
      const count = Math.min(10, doc.lineCount - start);
      const ranges: ReturnType<typeof doc.lineRange>[] = [];
      doc.visibleLineRanges(start, count, (r) => ranges.push(r));
      expect(ranges).toEqual(Array.from({ length: count }, (_, i) => doc.lineRange(start + i)));
      const byLine = new Map<number, string>();
      doc.visibleChunks(start, count, (c) =>
        byLine.set(c.line, (byLine.get(c.line) ?? "") + c.text),
      );
      for (let i = start; i < start + count; i++) expect(byLine.get(i) ?? "").toBe(doc.lineText(i));
    };
    let doc = TextBuffer.empty;
    let ref = "";
    const snapshots: Array<[TextBuffer, string]> = [];
    for (let i = 0; i < 300; i++) {
      const from = Math.floor(rnd() * (ref.length + 1));
      const to = from + Math.floor(rnd() * (ref.length - from + 1));
      let text = "";
      for (let j = 0, n = Math.floor(rnd() * 5); j < n; j++)
        text += atoms[Math.floor(rnd() * atoms.length)];
      if (i % 25 === 0) snapshots.push([doc, ref]);
      doc = doc.replace(from, to, text);
      ref = ref.slice(0, from) + text + ref.slice(to);
      if (i % 25 === 0) checkRich(doc, ref);
    }
    checkRich(doc, ref);
    for (const [snap, text] of snapshots) checkRich(snap, text);
  });
  test("bulk replacement property on fragmented CRLF doc", () => {
    const [base, ref] = fragmentDoc(2_000);
    let seed = 777;
    const rnd = () => (seed = (seed * 1103515245 + 12345) >>> 0) / 2 ** 32;
    for (let round = 0; round < 50; round++) {
      const replacements: Replacement[] = [];
      let pos = round % 3;
      while (replacements.length < 80 && pos <= ref.length) {
        pos += Math.floor(rnd() * 20);
        if (pos > ref.length) break;
        const to = Math.min(ref.length, pos + Math.floor(rnd() * 12));
        const text =
          replacements.length % 13 === 0
            ? "BIG\r\n" + "x".repeat(500)
            : replacements.length % 5 === 0
              ? "\r\n"
              : replacements.length % 3 === 0
                ? ""
                : "z";
        replacements.push({ from: pos, to, text });
        pos = to;
      }
      replacements.push({ from: 0, to: Math.min(3, ref.length), text: "S" });
      replacements.sort((a, b) => a.from - b.from || a.to - b.to);
      const filtered: Replacement[] = [];
      let last = 0;
      for (const r of replacements)
        if (r.from >= last) {
          filtered.push(r);
          last = r.to;
        }
      const expected = applyOracle(ref, filtered);
      const doc = base.apply({ replacements: filtered });
      assertDoc(doc, expected);
      assertNoFallback(doc);
    }
  });
  test("streaming visible APIs on pathological docs", () => {
    const docs = [
      TextBuffer.fromString("x".repeat(100_000)),
      TextBuffer.fromString("\r\n".repeat(2_000)),
      TextBuffer.fromString("a\r\nb\n\r\n".repeat(500)),
      fragmentDoc(1_000)[0],
    ];
    for (const doc of docs) {
      const start = Math.floor(doc.lineCount / 2);
      const count = Math.min(50, doc.lineCount - start);
      const ranges: ReturnType<typeof doc.lineRange>[] = [];
      doc.lineRangesBetween(start, start + count, (r) => ranges.push(r));
      expect(ranges).toEqual(Array.from({ length: count }, (_, i) => doc.lineRange(start + i)));
      for (let line = start; line < start + count; line++) {
        const chunks: string[] = [];
        doc.chunksForLine(line, (c) => chunks.push(c.text));
        expect(chunks.join("")).toBe(doc.lineText(line));
      }
      const byLine = new Map<number, string>();
      doc.chunksForLines(start, start + count, (c) =>
        byLine.set(c.line, (byLine.get(c.line) ?? "") + c.text),
      );
      for (let line = start; line < start + count; line++)
        expect(byLine.get(line) ?? "").toBe(doc.lineText(line));
      doc.assert();
    }
  });
  test("progressive random sorted apply oracle", () => {
    let seed = 123;
    const rnd = () => (seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32;
    let ref = Array.from({ length: 500 }, (_, i) => `${i}\r\n`).join("");
    let doc = TextBuffer.fromString(ref);
    for (let round = 0; round < 100; round++) {
      const replacements: Replacement[] = [];
      let pos = 0;
      for (let i = 0; i < 20 && pos <= ref.length; i++) {
        pos += Math.floor(rnd() * 20);
        if (pos > ref.length) break;
        const to = Math.min(ref.length, pos + Math.floor(rnd() * 5));
        replacements.push({ from: pos, to, text: i % 2 ? "" : `P${round}_${i}\n` });
        pos = to;
      }
      const expected = replacements.reduceRight(
        (s, r) => s.slice(0, r.from) + r.text + s.slice(r.to),
        ref,
      );
      doc = doc.apply({ replacements });
      ref = expected;
      assertDoc(doc, ref);
    }
  });
});
