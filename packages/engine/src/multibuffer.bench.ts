import { performance } from "node:perf_hooks";
import { Effect } from "effect";
import { ChangeSet, MultiBufferSnapshot, TextBuffer, type Excerpt, type Span, type TextEdit } from "./index";

type Case = {
  readonly name: string;
  readonly run: () => number;
  readonly samples?: number;
  readonly warmup?: number;
};

type FlatEntry = Excerpt & {
  readonly multibufferStart: number;
  readonly length: number;
  readonly lineBreaks: number;
  readonly buffer: TextBuffer;
};

const checksum = (s: string): number => {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619);
  return h >>> 0;
};

const time = (c: Case): void => {
  const warmup = c.warmup ?? 3;
  const sampleCount = c.samples ?? 11;
  for (let i = 0; i < warmup; i++) c.run();
  const samples: number[] = [];
  let sum = 0;
  for (let i = 0; i < sampleCount; i++) {
    const start = performance.now();
    const value = c.run();
    sum = (sum + Math.imul(value | 0, i + 1)) >>> 0;
    samples.push(performance.now() - start);
  }
  samples.sort((a, b) => a - b);
  const p50 = samples[Math.floor((sampleCount - 1) * 0.5)]!;
  const p95 = samples[Math.floor((sampleCount - 1) * 0.95)]!;
  console.log(`${c.name.padEnd(42)} n=${String(sampleCount).padStart(2)} p50=${p50.toFixed(3)}ms p95=${p95.toFixed(3)}ms checksum=${sum}`);
};

const span = (start: number, end: number): Span => ({ start, end });
const lineBreaks = (buffer: TextBuffer, from: number, to: number): number => {
  let n = 0;
  buffer.chunksBetween(from, to, (chunk) => {
    for (let i = 0; i < chunk.length; i++) if (chunk.charCodeAt(i) === 10) n++;
  });
  return n;
};

const makeFixture = (count: number, docs = 1) => {
  const documents = new Map<string, TextBuffer>();
  const excerpts: Excerpt[] = [];
  for (let d = 0; d < docs; d++) {
    const lines = Math.ceil(count / docs) + 8;
    const buffer = TextBuffer.fromString(Array.from({ length: lines }, (_, i) => `doc-${d}-line-${i}\n`).join(""));
    const documentId = `doc-${d}`;
    documents.set(documentId, buffer);
    for (let i = 0; i < lines && excerpts.length < count; i++) {
      const line = buffer.lineRange(i);
      excerpts.push({
        id: `${documentId}:${i}`,
        documentId,
        sourceRange: span(line.from, line.breakTo),
        primaryRange: span(line.from, line.to),
      });
    }
  }
  return { documents, excerpts };
};

const flatBuild = (documents: ReadonlyMap<string, TextBuffer>, excerpts: readonly Excerpt[]): readonly FlatEntry[] => {
  const entries: FlatEntry[] = [];
  let multibufferStart = 0;
  for (const excerpt of excerpts) {
    const buffer = documents.get(excerpt.documentId)!;
    const length = excerpt.sourceRange.end - excerpt.sourceRange.start;
    const entry = {
      ...excerpt,
      buffer,
      length,
      lineBreaks: lineBreaks(buffer, excerpt.sourceRange.start, excerpt.sourceRange.end),
      multibufferStart,
    } satisfies FlatEntry;
    entries.push(entry);
    multibufferStart += length;
  }
  return entries;
};

const flatOffsetToSource = (entries: readonly FlatEntry[], offset: number): number => {
  for (const entry of entries) {
    const end = entry.multibufferStart + entry.length;
    if (offset < end) return entry.sourceRange.start + offset - entry.multibufferStart;
  }
  const last = entries[entries.length - 1]!;
  return last.sourceRange.end;
};

const flatSourceToOffset = (entries: readonly FlatEntry[], documentId: string, offset: number): number => {
  for (const entry of entries)
    if (entry.documentId === documentId && offset >= entry.sourceRange.start && offset <= entry.sourceRange.end)
      return entry.multibufferStart + offset - entry.sourceRange.start;
  return -1;
};

const flatChunks = (entries: readonly FlatEntry[], from: number, to: number): number => {
  let h = 0;
  for (const entry of entries) {
    const entryFrom = entry.multibufferStart;
    const entryTo = entryFrom + entry.length;
    if (entryTo <= from) continue;
    if (entryFrom >= to) break;
    entry.buffer.chunksBetween(
      entry.sourceRange.start + Math.max(from, entryFrom) - entryFrom,
      entry.sourceRange.start + Math.min(to, entryTo) - entryFrom,
      (chunk) => {
        h = (h + checksum(chunk)) >>> 0;
      },
    );
  }
  return h;
};

const flatEditPlan = (entries: readonly FlatEntry[], edits: readonly TextEdit[]): number => {
  let h = 0;
  for (const edit of edits) {
    let inserted = false;
    for (const entry of entries) {
      const entryFrom = entry.multibufferStart;
      const entryTo = entryFrom + entry.length;
      if (entryTo <= edit.span.start) continue;
      if (entryFrom >= edit.span.end) break;
      const from = Math.max(edit.span.start, entryFrom);
      const to = Math.min(edit.span.end, entryTo);
      h = (h + checksum(entry.documentId) + entry.sourceRange.start + from - entryFrom + to - from) >>> 0;
      if (!inserted) h = (h + checksum(edit.text)) >>> 0;
      inserted = true;
    }
  }
  return h;
};

const fixture1k = makeFixture(1_000);
const fixture10k = makeFixture(10_000);
const fixture100k = makeFixture(100_000);
const fixtureManyDocs = makeFixture(10_000, 100);
const tree100k = MultiBufferSnapshot.fromExcerpts(fixture100k.documents, fixture100k.excerpts);
const flat100k = flatBuild(fixture100k.documents, fixture100k.excerpts);
const treeManyDocs = MultiBufferSnapshot.fromExcerpts(fixtureManyDocs.documents, fixtureManyDocs.excerpts);
const flatManyDocs = flatBuild(fixtureManyDocs.documents, fixtureManyDocs.excerpts);
const offsets = Array.from({ length: 256 }, (_, i) => (i * 9973) % tree100k.length);
const sourceOffsets = Array.from({ length: 256 }, (_, i) => {
  const line = fixture100k.documents.get("doc-0")!.lineRange((i * 389) % fixture100k.excerpts.length);
  return line.from;
});
const visibleFrom = tree100k.lineToOffset(40_000);
const visibleTo = tree100k.lineToOffset(40_600);
const manyExcerptEdit = [{ span: span(tree100k.lineToOffset(10_000), tree100k.lineToOffset(10_500)), text: "replacement\n" }];
const singleExcerptEdit = [{ span: span(5, 5), text: "x" }];
const manyDocEdit = [{ span: span(treeManyDocs.lineToOffset(2_000), treeManyDocs.lineToOffset(2_600)), text: "replacement\n" }];
const baseDoc = fixture100k.documents.get("doc-0")!;
const change = Effect.runSync(ChangeSet.fromReplacements(baseDoc.length, [{ from: 5, to: 5, text: "inserted\n" }]));
const changedDoc = change.apply(baseDoc);
const changedDocs = new Map(fixture100k.documents).set("doc-0", changedDoc);
const changeMap = new Map([["doc-0", change.desc()]]);

const cases: Case[] = [
  { name: "mb/build_flat/1k", run: () => flatBuild(fixture1k.documents, fixture1k.excerpts).length },
  { name: "mb/build_tree/1k", run: () => MultiBufferSnapshot.fromExcerpts(fixture1k.documents, fixture1k.excerpts).length },
  { name: "mb/build_flat/10k", run: () => flatBuild(fixture10k.documents, fixture10k.excerpts).length, samples: 7 },
  { name: "mb/build_tree/10k", run: () => MultiBufferSnapshot.fromExcerpts(fixture10k.documents, fixture10k.excerpts).length, samples: 7 },
  { name: "mb/build_flat/100k", run: () => flatBuild(fixture100k.documents, fixture100k.excerpts).length, samples: 3, warmup: 1 },
  { name: "mb/build_tree/100k", run: () => MultiBufferSnapshot.fromExcerpts(fixture100k.documents, fixture100k.excerpts).length, samples: 3, warmup: 1 },
  {
    name: "mb/offset_to_source/flat",
    run: () => offsets.reduce((h, offset) => (h + flatOffsetToSource(flat100k, offset)) >>> 0, 0),
  },
  {
    name: "mb/offset_to_source/tree",
    run: () => offsets.reduce((h, offset) => (h + (tree100k.offsetToSource(offset)?.offset ?? 0)) >>> 0, 0),
  },
  {
    name: "mb/source_to_offset/flat",
    run: () => sourceOffsets.reduce((h, offset) => (h + flatSourceToOffset(flat100k, "doc-0", offset)) >>> 0, 0),
  },
  {
    name: "mb/source_to_offset/tree",
    run: () => sourceOffsets.reduce((h, offset) => (h + (tree100k.sourceToOffset("doc-0", offset) ?? 0)) >>> 0, 0),
  },
  { name: "mb/visible_chunks/flat", run: () => flatChunks(flat100k, visibleFrom, visibleTo) },
  {
    name: "mb/visible_chunks/tree",
    run: () => {
      let h = 0;
      tree100k.chunksBetween(visibleFrom, visibleTo, (chunk) => {
        h = (h + checksum(chunk.text)) >>> 0;
      });
      return h;
    },
  },
  { name: "mb/edit_plan/single_flat", run: () => flatEditPlan(flat100k, singleExcerptEdit) },
  { name: "mb/edit_plan/single_tree", run: () => tree100k.editPlan(singleExcerptEdit).documentEdits.size },
  { name: "mb/edit_plan/many_excerpts_flat", run: () => flatEditPlan(flat100k, manyExcerptEdit) },
  { name: "mb/edit_plan/many_excerpts_tree", run: () => tree100k.editPlan(manyExcerptEdit).documentEdits.size },
  { name: "mb/edit_plan/many_docs_flat", run: () => flatEditPlan(flatManyDocs, manyDocEdit) },
  { name: "mb/edit_plan/many_docs_tree", run: () => treeManyDocs.editPlan(manyDocEdit).documentEdits.size },
  {
    name: "mb/update_remap/tree",
    run: () => tree100k.mapDocumentChanges(changeMap, changedDocs).length,
    samples: 3,
    warmup: 1,
  },
];

console.log("@the-editor/engine multibuffer bench");
for (const c of cases) time(c);
