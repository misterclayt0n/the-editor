import { TextBuffer, type LineRange } from "./text/buffer";
import type { ChangeDesc, RangeMapOptions, Span } from "./text/transaction";

export type DocumentId = string;
export type ExcerptId = string;

export type Excerpt = {
  readonly id: ExcerptId;
  readonly documentId: DocumentId;
  readonly sourceRange: Span;
  readonly primaryRange: Span;
};

export type MultiBufferSummary = {
  readonly length: number;
  readonly lineBreaks: number;
};

export type MultiBufferSourcePoint = {
  readonly excerpt: Excerpt;
  readonly documentId: DocumentId;
  readonly offset: number;
};

export type MultiBufferBoundary = {
  readonly offset: number;
  readonly previous?: Excerpt;
  readonly next: Excerpt;
};

export type MultiBufferChunk = {
  readonly from: number;
  readonly to: number;
  readonly text: string;
  readonly excerpt: Excerpt;
};

export type TextEdit = {
  readonly span: Span;
  readonly text: string;
};

export type MultiBufferEditPlan = {
  readonly documentEdits: ReadonlyMap<DocumentId, readonly TextEdit[]>;
};

type ExcerptEntry = Excerpt & {
  readonly buffer: TextBuffer;
  readonly multibufferStart: number;
  /** Length of the excerpt's own source text (excludes the trailing separator). */
  readonly textLength: number;
  /** Line breaks inside the excerpt's own source text (excludes the separator). */
  readonly textLineBreaks: number;
  /** Whether a synthetic "\n" separator follows this excerpt in unified space. */
  readonly hasTrailingNewline: boolean;
  /** Effective summary used by the tree: source text PLUS the separator. */
  readonly summary: MultiBufferSummary;
};

const publicExcerpt = ({ id, documentId, sourceRange, primaryRange }: Excerpt): Excerpt => ({
  id,
  documentId,
  sourceRange,
  primaryRange,
});

type Leaf = {
  readonly kind: "leaf";
  readonly height: 0;
  readonly entries: readonly ExcerptEntry[];
  readonly summary: MultiBufferSummary;
};

type Branch = {
  readonly kind: "branch";
  readonly height: number;
  readonly children: readonly Node[];
  readonly summary: MultiBufferSummary;
};

type Node = Leaf | Branch;

type SourceIndexEntry = {
  readonly sourceStart: number;
  readonly sourceEnd: number;
  readonly multibufferStart: number;
  readonly entry: ExcerptEntry;
};

const LeafMaxExcerpts = 64;
const BranchMaxChildren = 32;
const emptySummary: MultiBufferSummary = { length: 0, lineBreaks: 0 };
const addSummary = (a: MultiBufferSummary, b: MultiBufferSummary): MultiBufferSummary => ({
  length: a.length + b.length,
  lineBreaks: a.lineBreaks + b.lineBreaks,
});

const summarizeEntries = (entries: readonly ExcerptEntry[]): MultiBufferSummary => {
  let summary = emptySummary;
  for (const entry of entries) summary = addSummary(summary, entry.summary);
  return summary;
};

const summarizeNodes = (nodes: readonly Node[]): MultiBufferSummary => {
  let summary = emptySummary;
  for (const node of nodes) summary = addSummary(summary, node.summary);
  return summary;
};

const leaf = (entries: readonly ExcerptEntry[]): Leaf => ({
  kind: "leaf",
  height: 0,
  entries,
  summary: summarizeEntries(entries),
});

const branch = (children: readonly Node[]): Branch => ({
  kind: "branch",
  height: children[0]!.height + 1,
  children,
  summary: summarizeNodes(children),
});

const checkSpan = (span: Span, length: number, name: string): void => {
  if (
    !Number.isInteger(span.start) ||
    !Number.isInteger(span.end) ||
    span.start < 0 ||
    span.end < span.start ||
    span.end > length
  )
    throw new RangeError(`invalid ${name} ${span.start}..${span.end} for length ${length}`);
};

const checkOffset = (offset: number, length: number): void => {
  if (!Number.isInteger(offset) || offset < 0 || offset > length)
    throw new RangeError(`invalid offset ${offset} for length ${length}`);
};

const countLineBreaks = (buffer: TextBuffer, from: number, to: number): number => {
  let lineBreaks = 0;
  buffer.chunksBetween(from, to, (chunk) => {
    for (let i = 0; i < chunk.length; i++) if (chunk.charCodeAt(i) === 10) lineBreaks++;
  });
  return lineBreaks;
};

const summarizeExcerpt = (buffer: TextBuffer, range: Span): MultiBufferSummary => ({
  length: range.end - range.start,
  lineBreaks: countLineBreaks(buffer, range.start, range.end),
});

/**
 * Grow a line-aligned excerpt source range by `lines` rows in `direction`,
 * clamped to the buffer. The returned range is line-aligned and excludes the
 * last line's trailing newline (the multibuffer's synthetic separator supplies
 * the row break). This is the engine analog of Zed's expand_excerpt.
 */
export const expandExcerptRange = (
  buffer: TextBuffer,
  range: Span,
  direction: "up" | "down" | "both",
  lines: number,
): Span => {
  const grow = Math.max(0, Math.trunc(lines));
  const startLine = buffer.lineRangeAt(range.start).index;
  const endLine = buffer.lineRangeAt(Math.max(range.start, range.end - 1)).index;
  const newStartLine = direction === "down" ? startLine : Math.max(0, startLine - grow);
  const newEndLine = direction === "up" ? endLine : Math.min(buffer.lineCount - 1, endLine + grow);
  return { start: buffer.lineRange(newStartLine).from, end: buffer.lineRange(newEndLine).to };
};

const buildTree = (entries: readonly ExcerptEntry[]): Node | undefined => {
  if (entries.length === 0) return undefined;
  let level: Node[] = [];
  for (let i = 0; i < entries.length; i += LeafMaxExcerpts)
    level.push(leaf(entries.slice(i, i + LeafMaxExcerpts)));
  while (level.length > 1) {
    const next: Node[] = [];
    for (let i = 0; i < level.length; i += BranchMaxChildren)
      next.push(branch(level.slice(i, i + BranchMaxChildren)));
    level = next;
  }
  return level[0];
};

const findLineBreakInRange = (
  buffer: TextBuffer,
  from: number,
  to: number,
  breakIndex: number,
): number => {
  let seen = 0;
  let pos = from;
  let found = -1;
  buffer.chunksBetween(from, to, (chunk) => {
    if (found >= 0) return;
    for (let i = 0; i < chunk.length; i++) {
      if (chunk.charCodeAt(i) === 10) {
        if (seen === breakIndex) {
          found = pos + i;
          return;
        }
        seen++;
      }
    }
    pos += chunk.length;
  });
  if (found < 0) throw new RangeError(`invalid line break ${breakIndex}`);
  return found;
};

const positionInRange = (offset: number, entry: ExcerptEntry): boolean =>
  offset >= entry.sourceRange.start && offset <= entry.sourceRange.end;

const rangeMapOptions: RangeMapOptions = { startAssoc: "before", endAssoc: "after" };

export class MultiBufferSnapshot {
  readonly length: number;
  readonly lineCount: number;
  readonly excerptCount: number;

  private constructor(
    private readonly documents: ReadonlyMap<DocumentId, TextBuffer>,
    private readonly entries: readonly ExcerptEntry[],
    private readonly root: Node | undefined,
    private readonly byDocument: ReadonlyMap<DocumentId, readonly SourceIndexEntry[]>,
  ) {
    this.length = root?.summary.length ?? 0;
    this.lineCount = (root?.summary.lineBreaks ?? 0) + 1;
    this.excerptCount = entries.length;
  }

  static fromExcerpts(
    documents: ReadonlyMap<DocumentId, TextBuffer>,
    excerpts: readonly Excerpt[],
  ): MultiBufferSnapshot {
    let multibufferStart = 0;
    const entries: ExcerptEntry[] = [];
    const byDocument = new Map<DocumentId, SourceIndexEntry[]>();
    for (let i = 0; i < excerpts.length; i++) {
      const excerpt = excerpts[i]!;
      const buffer = documents.get(excerpt.documentId);
      if (buffer === undefined) throw new RangeError(`unknown document ${excerpt.documentId}`);
      checkSpan(excerpt.sourceRange, buffer.length, "source range");
      checkSpan(excerpt.primaryRange, buffer.length, "primary range");
      if (
        excerpt.primaryRange.start < excerpt.sourceRange.start ||
        excerpt.primaryRange.end > excerpt.sourceRange.end
      )
        throw new RangeError(`primary range must be inside source range for excerpt ${excerpt.id}`);
      if (excerpt.sourceRange.start === excerpt.sourceRange.end)
        throw new RangeError(`empty source range for excerpt ${excerpt.id}`);
      const text = summarizeExcerpt(buffer, excerpt.sourceRange);
      // Every excerpt but the last is followed by a synthetic "\n" so each
      // excerpt always begins on its own row, regardless of whether its
      // source slice ends in a newline. The separator belongs to no document.
      const hasTrailingNewline = i < excerpts.length - 1;
      const summary: MultiBufferSummary = hasTrailingNewline
        ? { length: text.length + 1, lineBreaks: text.lineBreaks + 1 }
        : text;
      const entry: ExcerptEntry = {
        ...excerpt,
        buffer,
        multibufferStart,
        textLength: text.length,
        textLineBreaks: text.lineBreaks,
        hasTrailingNewline,
        summary,
      };
      entries.push(entry);
      const indexed: SourceIndexEntry = {
        sourceStart: excerpt.sourceRange.start,
        sourceEnd: excerpt.sourceRange.end,
        multibufferStart,
        entry,
      };
      const list = byDocument.get(excerpt.documentId);
      if (list) list.push(indexed);
      else byDocument.set(excerpt.documentId, [indexed]);
      multibufferStart += summary.length;
    }
    for (const list of byDocument.values()) {
      list.sort(
        (a, b) =>
          a.sourceStart - b.sourceStart ||
          a.sourceEnd - b.sourceEnd ||
          a.multibufferStart - b.multibufferStart,
      );
    }
    return new MultiBufferSnapshot(documents, entries, buildTree(entries), byDocument);
  }

  excerpts(): readonly Excerpt[] {
    return this.entries.map(publicExcerpt);
  }

  document(id: DocumentId): TextBuffer | undefined {
    return this.documents.get(id);
  }

  excerptAtOffset(offset: number, assoc: "before" | "after" = "after"): Excerpt | undefined {
    return this.entryAtOffset(offset, assoc);
  }

  offsetToSource(offset: number, assoc: "before" | "after" = "after"): MultiBufferSourcePoint | undefined {
    const entry = this.entryAtOffset(offset, assoc);
    if (entry === undefined) return undefined;
    const local = Math.max(0, Math.min(entry.textLength, offset - entry.multibufferStart));
    return { excerpt: entry, documentId: entry.documentId, offset: entry.sourceRange.start + local };
  }

  sourceToOffset(
    documentId: DocumentId,
    offset: number,
    assoc: "before" | "after" = "after",
  ): number | undefined {
    return this.sourceToOffsets(documentId, offset, assoc)[0];
  }

  sourceToOffsets(
    documentId: DocumentId,
    offset: number,
    assoc: "before" | "after" = "after",
  ): readonly number[] {
    const list = this.byDocument.get(documentId);
    const buffer = this.documents.get(documentId);
    if (list === undefined || buffer === undefined) return [];
    checkOffset(offset, buffer.length);
    let lo = 0;
    let hi = list.length;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (list[mid]!.sourceStart <= offset) lo = mid + 1;
      else hi = mid;
    }
    const out: number[] = [];
    for (let i = lo - 1; i >= 0; i--) {
      const item = list[i]!;
      if (item.sourceEnd < offset || (item.sourceEnd === offset && assoc === "after")) break;
      if (positionInRange(offset, item.entry))
        out.push(item.multibufferStart + offset - item.sourceStart);
    }
    out.sort((a, b) => a - b);
    return out;
  }

  excerptBoundariesBetween(from: number, to: number): readonly MultiBufferBoundary[] {
    checkSpan({ start: from, end: to }, this.length, "multibuffer range");
    const out: MultiBufferBoundary[] = [];
    for (let i = 0; i < this.entries.length; i++) {
      const entry = this.entries[i]!;
      if (entry.multibufferStart > to) break;
      if (entry.multibufferStart >= from) {
        const previous = this.entries[i - 1];
        out.push(
          previous === undefined
            ? { offset: entry.multibufferStart, next: publicExcerpt(entry) }
            : {
                offset: entry.multibufferStart,
                previous: publicExcerpt(previous),
                next: publicExcerpt(entry),
              },
        );
      }
    }
    return out;
  }

  chunksBetween(from: number, to: number, emit: (chunk: MultiBufferChunk) => void): void {
    checkSpan({ start: from, end: to }, this.length, "multibuffer range");
    if (from === to || this.root === undefined) return;
    this.emitChunks(this.root, 0, from, to, emit);
  }

  textBetween(from: number, to: number): string {
    let text = "";
    this.chunksBetween(from, to, (chunk) => {
      text += chunk.text;
    });
    return text;
  }

  lineToOffset(line: number): number {
    if (!Number.isInteger(line) || line < 0 || line >= this.lineCount)
      throw new RangeError(`invalid line ${line}`);
    if (line === 0) return 0;
    return this.lineBreakOffset(this.root, line - 1, 0) + 1;
  }

  offsetToLine(offset: number): number {
    checkOffset(offset, this.length);
    if (offset === 0 || this.root === undefined) return 0;
    return this.lineBreaksBefore(this.root, 0, offset);
  }

  /** Alias of {@link textBetween}; part of the RenderableText surface. */
  sliceString(from: number, to: number = this.length): string {
    return this.textBetween(from, to);
  }

  lineRange(index: number): LineRange {
    if (!Number.isInteger(index) || index < 0 || index >= this.lineCount)
      throw new RangeError(`invalid line ${index}`);
    return this.lineRangeFrom(index, this.lineToOffset(index));
  }

  lineRangeAt(offset: number): LineRange {
    checkOffset(offset, this.length);
    const index = this.offsetToLine(offset);
    return this.lineRangeFrom(index, this.lineToOffset(index));
  }

  private lineRangeFrom(index: number, from: number): LineRange {
    if (index >= this.lineCount - 1)
      return { index, from, to: this.length, breakFrom: this.length, breakTo: this.length };
    // lineToOffset(index + 1) sits one past the line break that ends this line.
    const breakTo = this.lineToOffset(index + 1);
    const lineEnd = breakTo - 1;
    const breakFrom =
      lineEnd > from && this.textBetween(lineEnd - 1, lineEnd) === "\r" ? lineEnd - 1 : lineEnd;
    return { index, from, to: breakFrom, breakFrom, breakTo };
  }

  editPlan(edits: readonly TextEdit[]): MultiBufferEditPlan {
    const normalized = [...edits].sort((a, b) => a.span.start - b.span.start || a.span.end - b.span.end);
    let previousEnd = 0;
    const byDocument = new Map<DocumentId, TextEdit[]>();
    for (const edit of normalized) {
      checkSpan(edit.span, this.length, "edit span");
      if (edit.span.start < previousEnd) throw new RangeError("overlapping multibuffer edits");
      previousEnd = edit.span.end;
      if (edit.span.start === edit.span.end) {
        const source = this.offsetToSource(
          edit.span.start,
          edit.span.start === this.length ? "before" : "after",
        );
        if (source === undefined) throw new RangeError("cannot edit an empty multibuffer");
        this.pushDocumentEdit(byDocument, source.documentId, {
          span: { start: source.offset, end: source.offset },
          text: edit.text,
        });
      } else {
        let inserted = false;
        this.entriesBetween(edit.span.start, edit.span.end, (entry, from, to) => {
          this.pushDocumentEdit(byDocument, entry.documentId, {
            span: {
              start: entry.sourceRange.start + from - entry.multibufferStart,
              end: entry.sourceRange.start + to - entry.multibufferStart,
            },
            text: inserted ? "" : edit.text,
          });
          inserted = true;
        });
      }
    }
    for (const [documentId, documentEdits] of byDocument) {
      documentEdits.sort((a, b) => a.span.start - b.span.start || a.span.end - b.span.end);
      let previous: TextEdit | undefined;
      for (const edit of documentEdits) {
        if (previous && edit.span.start < previous.span.end)
          throw new RangeError(`overlapping edits for document ${documentId}`);
        previous = edit;
      }
    }
    return { documentEdits: byDocument };
  }

  mapDocumentChanges(
    changes: ReadonlyMap<DocumentId, ChangeDesc>,
    documents: ReadonlyMap<DocumentId, TextBuffer> = this.documents,
  ): MultiBufferSnapshot {
    const mapped: Excerpt[] = [];
    for (const entry of this.entries) {
      const change = changes.get(entry.documentId);
      if (change === undefined) {
        mapped.push(entry);
        continue;
      }
      if (change.length !== entry.buffer.length)
        throw new RangeError(`change length mismatch for document ${entry.documentId}`);
      const sourceRange = change.mapRange(entry.sourceRange, rangeMapOptions);
      if (sourceRange.start === sourceRange.end) continue;
      const primaryRange = change.mapRange(entry.primaryRange, rangeMapOptions);
      mapped.push({
        id: entry.id,
        documentId: entry.documentId,
        sourceRange,
        primaryRange: {
          start: Math.max(sourceRange.start, Math.min(sourceRange.end, primaryRange.start)),
          end: Math.max(sourceRange.start, Math.min(sourceRange.end, primaryRange.end)),
        },
      });
    }
    return MultiBufferSnapshot.fromExcerpts(documents, mapped);
  }

  private pushDocumentEdit(byDocument: Map<DocumentId, TextEdit[]>, documentId: DocumentId, edit: TextEdit): void {
    const list = byDocument.get(documentId);
    if (list) list.push(edit);
    else byDocument.set(documentId, [edit]);
  }

  private entryAtOffset(offset: number, assoc: "before" | "after"): ExcerptEntry | undefined {
    checkOffset(offset, this.length);
    if (this.root === undefined) return undefined;
    return this.seekEntry(this.root, 0, offset, assoc);
  }

  private seekEntry(
    node: Node,
    base: number,
    offset: number,
    assoc: "before" | "after",
  ): ExcerptEntry {
    if (node.kind === "branch") {
      let pos = base;
      for (let i = 0; i < node.children.length; i++) {
        const child = node.children[i]!;
        const end = pos + child.summary.length;
        if (offset < end || (offset === end && assoc === "before") || i === node.children.length - 1)
          return this.seekEntry(child, pos, offset, assoc);
        pos = end;
      }
      throw new RangeError(`invalid offset ${offset}`);
    }
    let pos = base;
    for (let i = 0; i < node.entries.length; i++) {
      const entry = node.entries[i]!;
      const end = pos + entry.summary.length;
      if (offset < end || (offset === end && assoc === "before") || i === node.entries.length - 1)
        return entry;
      pos = end;
    }
    return node.entries[node.entries.length - 1]!;
  }

  private entriesBetween(
    from: number,
    to: number,
    emit: (entry: ExcerptEntry, from: number, to: number) => void,
  ): void {
    for (const entry of this.entries) {
      const entryFrom = entry.multibufferStart;
      const textTo = entryFrom + entry.textLength;
      if (textTo <= from) continue;
      if (entryFrom >= to) break;
      emit(entry, Math.max(from, entryFrom), Math.min(to, textTo));
    }
  }

  private emitChunks(
    node: Node,
    base: number,
    from: number,
    to: number,
    emit: (chunk: MultiBufferChunk) => void,
  ): void {
    const nodeTo = base + node.summary.length;
    if (nodeTo <= from || base >= to) return;
    if (node.kind === "branch") {
      let pos = base;
      for (const child of node.children) {
        this.emitChunks(child, pos, from, to, emit);
        pos += child.summary.length;
        if (pos >= to) break;
      }
      return;
    }
    for (const entry of node.entries) {
      const entryFrom = entry.multibufferStart;
      const textTo = entryFrom + entry.textLength;
      const entryTo = entryFrom + entry.summary.length;
      if (entryTo > from && entryFrom < to) {
        const fromClamped = Math.max(from, entryFrom);
        const textToClamped = Math.min(to, textTo);
        if (textToClamped > fromClamped) {
          let pos = fromClamped;
          entry.buffer.chunksBetween(
            entry.sourceRange.start + (fromClamped - entryFrom),
            entry.sourceRange.start + (textToClamped - entryFrom),
            (text) => {
              const chunkFrom = pos;
              pos += text.length;
              emit({ from: chunkFrom, to: pos, text, excerpt: entry });
            },
          );
        }
        // The synthetic separator is a single "\n" at [textTo, entryTo).
        if (entry.hasTrailingNewline && from < entryTo && to > textTo)
          emit({ from: textTo, to: entryTo, text: "\n", excerpt: entry });
      }
      if (entryTo >= to) break;
    }
  }

  private lineBreakOffset(node: Node | undefined, breakIndex: number, base: number): number {
    if (node === undefined || breakIndex < 0 || breakIndex >= node.summary.lineBreaks)
      throw new RangeError(`invalid line break ${breakIndex}`);
    if (node.kind === "branch") {
      let pos = base;
      let remaining = breakIndex;
      for (const child of node.children) {
        if (remaining < child.summary.lineBreaks) return this.lineBreakOffset(child, remaining, pos);
        remaining -= child.summary.lineBreaks;
        pos += child.summary.length;
      }
      throw new RangeError(`invalid line break ${breakIndex}`);
    }
    let pos = base;
    let remaining = breakIndex;
    for (const entry of node.entries) {
      if (remaining < entry.summary.lineBreaks) {
        if (remaining < entry.textLineBreaks) {
          const sourceBreak = findLineBreakInRange(
            entry.buffer,
            entry.sourceRange.start,
            entry.sourceRange.end,
            remaining,
          );
          return pos + sourceBreak - entry.sourceRange.start;
        }
        // The remaining break is the synthetic separator at the text end.
        return pos + entry.textLength;
      }
      remaining -= entry.summary.lineBreaks;
      pos += entry.summary.length;
    }
    throw new RangeError(`invalid line break ${breakIndex}`);
  }

  private lineBreaksBefore(node: Node, base: number, offset: number): number {
    const nodeTo = base + node.summary.length;
    if (offset >= nodeTo) return node.summary.lineBreaks;
    if (offset <= base) return 0;
    if (node.kind === "branch") {
      let total = 0;
      let pos = base;
      for (const child of node.children) {
        const childTo = pos + child.summary.length;
        if (offset >= childTo) total += child.summary.lineBreaks;
        else return total + this.lineBreaksBefore(child, pos, offset);
        pos = childTo;
      }
      return total;
    }
    let total = 0;
    for (const entry of node.entries) {
      const entryFrom = entry.multibufferStart;
      const textTo = entryFrom + entry.textLength;
      const entryTo = entryFrom + entry.summary.length;
      if (offset >= entryTo) total += entry.summary.lineBreaks;
      else if (offset <= entryFrom) return total;
      else if (offset <= textTo)
        return total + countLineBreaks(entry.buffer, entry.sourceRange.start, entry.sourceRange.start + offset - entryFrom);
      // offset sits on the synthetic separator: all text breaks precede it.
      else return total + entry.textLineBreaks;
    }
    return total;
  }
}

export class NaiveMultiBufferSnapshot {
  readonly length: number;
  readonly lineCount: number;

  constructor(private readonly snapshot: MultiBufferSnapshot) {
    this.length = snapshot.length;
    this.lineCount = snapshot.lineCount;
  }

  offsetToSource(offset: number): MultiBufferSourcePoint | undefined {
    return this.snapshot.offsetToSource(offset);
  }

  sourceToOffset(documentId: DocumentId, offset: number): number | undefined {
    const offsets = this.snapshot.sourceToOffsets(documentId, offset);
    return offsets[0];
  }

  textBetween(from: number, to: number): string {
    return this.snapshot.textBetween(from, to);
  }

  lineToOffset(line: number): number {
    const text = this.snapshot.textBetween(0, this.snapshot.length);
    if (!Number.isInteger(line) || line < 0) throw new RangeError(`invalid line ${line}`);
    if (line === 0) return 0;
    let seen = 0;
    for (let i = 0; i < text.length; i++)
      if (text.charCodeAt(i) === 10 && ++seen === line) return i + 1;
    throw new RangeError(`invalid line ${line}`);
  }

  offsetToLine(offset: number): number {
    checkOffset(offset, this.snapshot.length);
    const text = this.snapshot.textBetween(0, offset);
    let line = 0;
    for (let i = 0; i < text.length; i++) if (text.charCodeAt(i) === 10) line++;
    return line;
  }

  editPlan(edits: readonly TextEdit[]): MultiBufferEditPlan {
    return this.snapshot.editPlan(edits);
  }
}
