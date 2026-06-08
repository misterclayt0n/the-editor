import { performance } from "node:perf_hooks";
import { Effect } from "effect";
import { makeTextDocument, TextBuffer, Transaction } from "./index";

type Case = {
  name: string;
  run: () => number;
  stats?: () => unknown;
  samples?: number;
  warmup?: number;
};
const mutationDelta = (before: TextBuffer, after: TextBuffer) => {
  const a = after.stats().mutation,
    b = before.stats().mutation;
  return {
    pathCopies: a.pathCopies - b.pathCopies,
    leafRebuilds: a.leafRebuilds - b.leafRebuilds,
    branchRebuilds: a.branchRebuilds - b.branchRebuilds,
    subtreeRebuilds: a.subtreeRebuilds - b.subtreeRebuilds,
    fullRebuilds: a.fullRebuilds - b.fullRebuilds,
    flattenedPieces: a.flattenedPieces - b.flattenedPieces,
    localRepackedPieces: a.localRepackedPieces - b.localRepackedPieces,
    localRepackedChildren: a.localRepackedChildren - b.localRepackedChildren,
    piecesCreated: a.piecesCreated - b.piecesCreated,
    piecesCoalesced: a.piecesCoalesced - b.piecesCoalesced,
    blockAppends: a.blockAppends - b.blockAppends,
    leafMerges: a.leafMerges - b.leafMerges,
    branchMerges: a.branchMerges - b.branchMerges,
  };
};
const checksum = (s: string): number => {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619);
  return h >>> 0;
};
const time = (c: Case): void => {
  const warmup = c.warmup ?? 5;
  const sampleCount = c.samples ?? 31;
  for (let i = 0; i < warmup; i++) c.run();
  const samples: number[] = [];
  let sum = 0;
  for (let i = 0; i < sampleCount; i++) {
    const t = performance.now();
    const value = c.run();
    sum = (sum + Math.imul(value | 0, i + 1)) >>> 0;
    samples.push(performance.now() - t);
  }
  samples.sort((a, b) => a - b);
  const p50 = samples[Math.floor((sampleCount - 1) * 0.5)]!;
  const p95 = samples[Math.floor((sampleCount - 1) * 0.95)]!;
  console.log(
    `${c.name.padEnd(42)} n=${String(sampleCount).padStart(2)} p50=${p50.toFixed(3)}ms p95=${p95.toFixed(3)}ms checksum=${sum}`,
    c.stats?.() ?? "",
  );
};
const medium = Array.from(
  { length: 20_000 },
  (_, i) => `line ${i} abcdefghijklmnopqrstuvwxyz\n`,
).join("");
const large = medium.repeat(10);
const visible = (doc: TextBuffer, startLine: number): number => {
  let h = 0;
  doc.visibleLines(startLine, 300, (line) => {
    h =
      (h + Math.imul(checksum(line.text) ^ line.from ^ line.to ^ line.index, line.index + 1)) >>> 0;
  });
  return h >>> 0;
};
const visibleRanges = (doc: TextBuffer, startLine: number): number => {
  let h = 0;
  doc.visibleLineRanges(startLine, 300, (line) => {
    h =
      (h +
        Math.imul(
          line.from ^ line.to ^ line.index ^ line.breakFrom ^ line.breakTo,
          line.index + 1,
        )) >>>
      0;
  });
  return h;
};
const visibleChunked = (doc: TextBuffer, startLine: number): number => {
  let h = 0;
  doc.visibleChunks(startLine, 300, (chunk) => {
    h =
      (h + Math.imul(checksum(chunk.text) ^ chunk.from ^ chunk.to ^ chunk.line, chunk.line + 1)) >>>
      0;
  });
  return h;
};
const cases: Case[] = [
  { name: "from_str/small", run: () => TextBuffer.fromString("hello\nworld").length },
  { name: "from_str/medium", run: () => TextBuffer.fromString(medium).length },
  { name: "from_str/large", run: () => TextBuffer.fromString(large).length },
  { name: "from_str/linefeeds", run: () => TextBuffer.fromString("\n".repeat(100_000)).lineCount },
];
const doc = TextBuffer.fromString(large);
const hugeLineDoc = TextBuffer.fromString("x".repeat(500_000));
const manyTinyLinesDoc = TextBuffer.fromString("x\n".repeat(100_000));
const fragment = (edits: number): TextBuffer => {
  let d = TextBuffer.fromString(medium);
  let fseed = 3;
  const frnd = () => (fseed = (fseed * 1664525 + 1013904223) >>> 0) / 2 ** 32;
  for (let i = 0; i < edits; i++) {
    const at = Math.floor(frnd() * (d.length + 1));
    const del = Math.min(d.length - at, Math.floor(frnd() * 4));
    d = d.replace(at, at + del, i % 5 === 0 ? `\n${i}` : "x");
  }
  return d;
};
const tFrag10 = performance.now();
const fragmented = fragment(10_000);
const tFrag10End = performance.now();
const tFrag100 = performance.now();
const fragmented100k = fragment(100_000);
const tFrag100End = performance.now();
console.log("fragmented/10k build ms", (tFrag10End - tFrag10).toFixed(3));
console.log("fragmented/10k initial stats", fragmented.stats());
console.log("fragmented/100k build ms", (tFrag100End - tFrag100).toFixed(3));
console.log("fragmented/100k initial stats", fragmented100k.stats());
const randomEditWorkload = (edits: number): number => {
  let d = TextBuffer.fromString("abcdefghij\n".repeat(100));
  let seed = 42;
  const rnd = () => (seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32;
  for (let i = 0; i < edits; i++) {
    const at = Math.floor(rnd() * (d.length + 1));
    const del = Math.min(d.length - at, Math.floor(rnd() * 3));
    d = d.replace(at, at + del, i % 17 === 0 ? "\n" : "x");
  }
  return checksum(d.sliceString(0, Math.min(4096, d.length))) ^ d.length ^ d.lineCount;
};
const typingWithDeletes = (edits: number): number => {
  let d = TextBuffer.empty;
  for (let i = 0; i < edits; i++) {
    if (i % 7 === 6 && d.length > 0) d = d.replace(d.length - 1, d.length, "");
    else d = d.replace(d.length, d.length, i % 29 === 0 ? "\n" : "x");
  }
  return checksum(d.toString()) ^ d.length;
};
const alternatingStartEnd = (edits: number): number => {
  let d = TextBuffer.empty;
  for (let i = 0; i < edits; i++)
    d = i % 2 === 0 ? d.replace(0, 0, "a") : d.replace(d.length, d.length, "z");
  return checksum(d.sliceString(0, Math.min(2048, d.length))) ^ d.length;
};
const transactionBase = TextBuffer.fromString("abcdefghij\n".repeat(1_000));
const transactionInsert = Effect.runSync(
  Transaction.insertAt(transactionBase.length, transactionBase.length >> 1, "INSERT\n"),
);
const transactionDelete = Effect.runSync(
  Transaction.delete(transactionBase.length, { start: 100, end: 200 }),
);
const transactionApplied = transactionInsert.apply(transactionBase);
const transactionSecond = Effect.runSync(Transaction.insertAt(transactionApplied.length, 10, "B"));
const documentTx = Effect.runSync(Transaction.insertAt(3, 1, "X"));

const transactionMap1000 = (): number => {
  let h = 0;
  for (let i = 0; i < 1_000; i++)
    h =
      (h +
        transactionInsert.mapPosition(
          i % transactionBase.length,
          i % 2 === 0 ? "before" : "after",
        )) >>>
      0;
  return h;
};

const documentTransact = (): number => {
  const doc = Effect.runSync(makeTextDocument("abc"));
  const next = Effect.runSync(doc.transact(documentTx));
  return checksum(next.toString()) ^ Effect.runSync(doc.version);
};

const documentUpdate = (): number => {
  const doc = Effect.runSync(makeTextDocument("abc"));
  const next = Effect.runSync(
    doc.update((buffer) => Transaction.insertAt(buffer.length, buffer.length, "!")),
  );
  return checksum(next.toString()) ^ Effect.runSync(doc.version);
};

const documentSnapshot = (): number => {
  const doc = Effect.runSync(makeTextDocument("abc"));
  let h = 0;
  for (let i = 0; i < 100; i++) {
    const snap = Effect.runSync(doc.snapshot);
    h = (h + snap.buffer.length + snap.version + (snap.dirty ? 1 : 0)) >>> 0;
  }
  return h;
};

const distributedReplacements = (base: TextBuffer, count: number): number => {
  const step = Math.max(2, Math.floor(base.length / (count + 1)));
  const replacements = Array.from({ length: count }, (_, i) => ({
    from: i * step,
    to: Math.min(base.length, i * step + 1),
    text: i % 11 === 0 ? "\n" : "r",
  }));
  const d = base.apply({ replacements });
  return checksum(d.sliceString(0, 4096)) ^ d.length ^ d.lineCount;
};
const hugeDeleteAcrossLeaves = (base: TextBuffer): number => {
  const d = base.replace(base.length >> 4, (base.length * 15) >> 4, "");
  return (
    checksum(
      d.sliceString(
        Math.max(0, (base.length >> 4) - 128),
        Math.min(d.length, (base.length >> 4) + 128),
      ),
    ) ^ d.length
  );
};
cases.push(
  { name: "get/offset", run: () => doc.sliceString(12345, 12346).charCodeAt(0) },
  { name: "get/chunk_at_offset_slice", run: () => checksum(doc.sliceString(12345, 12445)) },
  { name: "get/line", run: () => checksum(doc.lineText(1234)) },
  { name: "get/lineRange", run: () => doc.lineRange(1234).from },
  { name: "get/lineRangeAt", run: () => doc.lineRangeAt(123456).index },
  { name: "index_convert/offset_to_line", run: () => doc.lineRangeAt(654321).index },
  { name: "index_convert/line_to_offset", run: () => doc.lineColumnToOffset(6543, 0) },
  {
    name: "index_convert/offset_to_line_col",
    run: () => doc.offsetToLineColumn(654321).line + doc.offsetToLineColumn(654321).column,
  },
  {
    name: "index_convert/line_col_to_offset",
    run: () => doc.lineColumnToOffset(6543, 3, { clamp: true }),
  },
  {
    name: "insert_char/start",
    run: () => checksum(doc.replace(0, 0, "x").sliceString(0, 10)),
    stats: () => mutationDelta(doc, doc.replace(0, 0, "x")),
  },
  {
    name: "insert_char/middle",
    run: () =>
      checksum(
        doc
          .replace(doc.length >> 1, doc.length >> 1, "x")
          .sliceString((doc.length >> 1) - 8, (doc.length >> 1) + 9),
      ),
    stats: () => mutationDelta(doc, doc.replace(doc.length >> 1, doc.length >> 1, "x")),
  },
  {
    name: "insert_char/end",
    run: () => checksum(doc.replace(doc.length, doc.length, "x").sliceString(doc.length - 8)),
    stats: () => mutationDelta(doc, doc.replace(doc.length, doc.length, "x")),
  },
  { name: "insert_small/random", run: () => doc.replace(9999, 9999, "small text").length },
  {
    name: "insert_large/middle",
    run: () =>
      checksum(
        doc
          .replace(doc.length >> 1, doc.length >> 1, medium)
          .sliceString(doc.length >> 1, (doc.length >> 1) + 256),
      ),
    stats: () => mutationDelta(doc, doc.replace(doc.length >> 1, doc.length >> 1, medium)),
  },
  {
    name: "remove_small/start",
    run: () => checksum(doc.replace(0, 10, "").sliceString(0, 64)),
    stats: () => mutationDelta(doc, doc.replace(0, 10, "")),
  },
  {
    name: "remove_medium/middle",
    run: () => checksum(doc.replace(10000, 20000, "").sliceString(9900, 10100)),
    stats: () => mutationDelta(doc, doc.replace(10000, 20000, "")),
  },
  {
    name: "remove_large/random",
    run: () => checksum(doc.replace(50000, 500000, "").sliceString(49900, 50100)),
    stats: () => mutationDelta(doc, doc.replace(50000, 500000, "")),
  },
  { name: "slice/small", run: () => checksum(doc.sliceString(10, 50)) },
  { name: "slice/medium", run: () => checksum(doc.sliceString(1000, 20000)) },
  { name: "slice/whole", run: () => checksum(doc.toString()) },
  {
    name: "editor/10k sequential typing",
    run: () => {
      let d = TextBuffer.empty;
      for (let i = 0; i < 10_000; i++) d = d.replace(d.length, d.length, "x");
      return d.length;
    },
  },
  {
    name: "stress/1M random edits",
    run: () => randomEditWorkload(1_000_000),
    samples: 3,
    warmup: 0,
  },
  {
    name: "stress/100k typing with deletes",
    run: () => typingWithDeletes(100_000),
    samples: 5,
    warmup: 1,
  },
  {
    name: "stress/100k alternating start/end",
    run: () => alternatingStartEnd(100_000),
    samples: 5,
    warmup: 1,
  },
  {
    name: "stress/10k distributed replacements",
    run: () => distributedReplacements(fragmented100k, 10_000),
    samples: 9,
    warmup: 1,
  },
  {
    name: "stress/huge delete across leaves",
    run: () => hugeDeleteAcrossLeaves(fragmented100k),
    samples: 9,
    warmup: 1,
  },
  { name: "editor/visible 300 top", run: () => visible(doc, 0) },
  { name: "editor/visible 300 middle", run: () => visible(doc, doc.lineCount >> 1) },
  { name: "editor/visible 300 end", run: () => visible(doc, doc.lineCount - 301) },
  { name: "editor/visibleLineRanges 300 top", run: () => visibleRanges(doc, 0) },
  {
    name: "editor/visibleLineRanges 300 middle",
    run: () => visibleRanges(doc, doc.lineCount >> 1),
  },
  { name: "editor/visibleLineRanges 300 end", run: () => visibleRanges(doc, doc.lineCount - 301) },
  { name: "editor/visibleChunks 300 top", run: () => visibleChunked(doc, 0) },
  { name: "editor/visibleChunks 300 middle", run: () => visibleChunked(doc, doc.lineCount >> 1) },
  { name: "editor/visibleChunks 300 end", run: () => visibleChunked(doc, doc.lineCount - 301) },
  { name: "visible/one huge line", run: () => visible(hugeLineDoc, 0) },
  { name: "visible/one huge line ranges", run: () => visibleRanges(hugeLineDoc, 0) },
  { name: "visible/one huge line chunks", run: () => visibleChunked(hugeLineDoc, 0) },
  { name: "visible/many tiny lines", run: () => visible(manyTinyLinesDoc, 50_000) },
  {
    name: "fragmented10k/middle_insert",
    run: () =>
      checksum(
        fragmented
          .replace(fragmented.length >> 1, fragmented.length >> 1, "x")
          .sliceString((fragmented.length >> 1) - 20, (fragmented.length >> 1) + 21),
      ),
  },
  {
    name: "fragmented10k/start_insert",
    run: () => checksum(fragmented.replace(0, 0, "x").sliceString(0, 64)),
  },
  {
    name: "fragmented10k/end_insert",
    run: () =>
      checksum(
        fragmented
          .replace(fragmented.length, fragmented.length, "x")
          .sliceString(fragmented.length - 63),
      ),
  },
  {
    name: "fragmented10k/large_delete",
    run: () =>
      checksum(
        fragmented
          .replace(fragmented.length >> 2, (fragmented.length * 3) >> 2, "")
          .sliceString(0, 256),
      ),
  },
  {
    name: "fragmented10k/1000_sorted_replacements",
    run: () => {
      const step = Math.max(2, Math.floor(fragmented.length / 1001));
      const replacements = Array.from({ length: 1000 }, (_, i) => ({
        from: i * step,
        to: Math.min(fragmented.length, i * step + 1),
        text: "r",
      }));
      return checksum(fragmented.apply({ replacements }).sliceString(0, 4096));
    },
    stats: () => {
      const step = Math.max(2, Math.floor(fragmented.length / 1001));
      const replacements = Array.from({ length: 1000 }, (_, i) => ({
        from: i * step,
        to: Math.min(fragmented.length, i * step + 1),
        text: "r",
      }));
      return mutationDelta(fragmented, fragmented.apply({ replacements }));
    },
  },
  {
    name: "fragmented10k/lineRangeAt",
    run: () => fragmented.lineRangeAt(fragmented.length >> 1).from,
  },
  { name: "fragmented10k/visible", run: () => visible(fragmented, fragmented.lineCount >> 1) },
  { name: "fragmented10k/slice_small", run: () => checksum(fragmented.sliceString(100, 180)) },
  { name: "fragmented10k/slice_medium", run: () => checksum(fragmented.sliceString(1000, 20000)) },
  {
    name: "fragmented100k/middle_insert",
    run: () =>
      checksum(
        fragmented100k
          .replace(fragmented100k.length >> 1, fragmented100k.length >> 1, "x")
          .sliceString((fragmented100k.length >> 1) - 20, (fragmented100k.length >> 1) + 21),
      ),
    stats: () =>
      mutationDelta(
        fragmented100k,
        fragmented100k.replace(fragmented100k.length >> 1, fragmented100k.length >> 1, "x"),
      ),
  },
  {
    name: "fragmented100k/far_apart_replacements",
    run: () => {
      const changes = {
        replacements: [
          { from: 5, to: 8, text: "S" },
          { from: fragmented100k.length >> 1, to: (fragmented100k.length >> 1) + 3, text: "M" },
          { from: fragmented100k.length - 8, to: fragmented100k.length - 5, text: "E" },
        ],
      };
      return checksum(fragmented100k.apply(changes).sliceString(0, 2048));
    },
    stats: () => {
      const changes = {
        replacements: [
          { from: 5, to: 8, text: "S" },
          { from: fragmented100k.length >> 1, to: (fragmented100k.length >> 1) + 3, text: "M" },
          { from: fragmented100k.length - 8, to: fragmented100k.length - 5, text: "E" },
        ],
      };
      return mutationDelta(fragmented100k, fragmented100k.apply(changes));
    },
  },
  {
    name: "fragmented100k/lineRangeAt",
    run: () => fragmented100k.lineRangeAt(fragmented100k.length >> 1).from,
  },
  {
    name: "fragmented100k/visible",
    run: () => visible(fragmented100k, fragmented100k.lineCount >> 1),
  },
  {
    name: "transaction/fromReplacements",
    run: () =>
      Effect.runSync(
        Transaction.fromReplacements(transactionBase.length, [
          { from: 10, to: 20, text: "A" },
          { from: 100, to: 100, text: "B" },
          { from: 500, to: 520, text: "" },
        ]),
      ).lengthAfter,
  },
  {
    name: "transaction/apply",
    run: () => checksum(transactionInsert.apply(transactionBase).sliceString(0, 1024)),
  },
  {
    name: "transaction/invert",
    run: () => Effect.runSync(transactionInsert.invert(transactionBase)).lengthAfter,
  },
  {
    name: "transaction/compose",
    run: () => Effect.runSync(transactionInsert.compose(transactionSecond)).lengthAfter,
  },
  { name: "transaction/mapPosition x1000", run: transactionMap1000 },
  {
    name: "document/make",
    run: () =>
      Effect.runSync(makeTextDocument("abc").pipe(Effect.flatMap((doc) => doc.version))) + 3,
  },
  { name: "document/transact", run: documentTransact, samples: 15 },
  { name: "document/update", run: documentUpdate, samples: 15 },
  { name: "document/snapshot x100", run: documentSnapshot, samples: 15 },
  { name: "stats", run: () => doc.length, stats: () => doc.stats() },
  { name: "fragmented10k/stats", run: () => fragmented.length, stats: () => fragmented.stats() },
  {
    name: "fragmented100k/stats",
    run: () => fragmented100k.length,
    stats: () => fragmented100k.stats(),
  },
);
console.log("@the-editor/engine bench");
for (const c of cases) time(c);
