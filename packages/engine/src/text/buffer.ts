import type { Block } from "./block";
import { addSummary, countLineBreaks, emptySummary, summarizeText } from "./summary";
import {
  addMutation,
  collectTextStats,
  emptyMutationStats,
  type MutableMutationStats,
  type MutationStats,
  type TextStats,
} from "./stats";
import {
  blockPiece,
  countBlockLineBreaks,
  inlinePiece,
  pieceText,
  slicePiece,
  type Piece,
} from "./piece";
import { BranchMaxChildren, LeafMaxPieces, branch, leaf, type Node } from "./node";
import {
  buildPiecesFromChunks,
  buildTree,
  coalescePieces,
  flattenPieces,
  piecesToString,
} from "./tree";

export type LineRange = {
  readonly index: number;
  readonly from: number;
  readonly to: number;
  readonly breakFrom: number;
  readonly breakTo: number;
};
export type { MutationStats, TextStats } from "./stats";
export type Replacement = { readonly from: number; readonly to: number; readonly text: string };
export type ChangeSet = { readonly replacements: readonly Replacement[] };
export type TextCursor = {
  readonly offset: number;
  readonly chunk: string;
  readonly chunkFrom: number;
  readonly chunkTo: number;
};
export type VisibleLine = LineRange & { readonly text: string };
export type LineChunk = { readonly from: number; readonly to: number; readonly text: string };
export type VisibleChunk = LineChunk & { readonly line: number };
export type LineColumn = { readonly line: number; readonly column: number };

const checkRange = (from: number, to: number, length: number): void => {
  if (!Number.isInteger(from) || !Number.isInteger(to) || from < 0 || to < from || to > length)
    throw new RangeError(`invalid range ${from}..${to} for length ${length}`);
};
const checkLineRange = (fromLine: number, toLine: number, lineCount: number): void => {
  if (
    !Number.isInteger(fromLine) ||
    !Number.isInteger(toLine) ||
    fromLine < 0 ||
    toLine < fromLine ||
    toLine > lineCount
  )
    throw new RangeError(`invalid line range ${fromLine}..${toLine}`);
};

export type TextBufferDebugState = {
  readonly blocks: readonly Block[];
  readonly root: Node | undefined;
  readonly mutation: MutationStats;
};

export class TextBuffer {
  static readonly empty = new TextBuffer([], undefined);
  readonly length: number;
  readonly lineCount: number;
  private constructor(
    private readonly blocks: readonly Block[],
    private readonly root: Node | undefined,
    private readonly mutation: MutationStats = emptyMutationStats,
  ) {
    this.length = root?.summary.length ?? 0;
    this.lineCount = (root?.summary.lineBreaks ?? 0) + 1;
  }
  /** @internal Unsafe test/debug construction hook. Do not use in product code. */
  static unsafeFromParts(
    blocks: readonly Block[],
    root: Node | undefined,
    mutation: MutationStats = emptyMutationStats,
  ): TextBuffer {
    return new TextBuffer(blocks, root, mutation);
  }
  /** @internal Unsafe test/debug state hook. Do not use in product code. */
  static unsafeDebugState(buffer: TextBuffer): TextBufferDebugState {
    return { blocks: buffer.blocks, root: buffer.root, mutation: buffer.mutation };
  }
  static fromString(text: string): TextBuffer {
    return TextBuffer.fromChunks([text]);
  }
  static fromChunks(chunks: readonly string[]): TextBuffer {
    const blocks: Block[] = [];
    const pieces = buildPiecesFromChunks(chunks, blocks);
    return new TextBuffer(blocks, buildTree(pieces), {
      ...emptyMutationStats,
      piecesCreated: pieces.length,
      blockAppends: blocks.length,
    });
  }
  private pieces(): readonly Piece[] {
    return flattenPieces(this.root);
  }
  toString(): string {
    return piecesToString(this.pieces(), this.blocks);
  }
  chunksBetween(from: number, to: number, emit: (chunk: string) => void): void {
    checkRange(from, to, this.length);
    if (from === to || !this.root) return;
    emitChunksBetween(this.root, this.blocks, from, to, 0, emit);
  }
  sliceString(from: number, to = this.length): string {
    let s = "";
    this.chunksBetween(from, to, (c) => {
      s += c;
    });
    return s;
  }
  lineRange(index: number): LineRange {
    if (!Number.isInteger(index) || index < 0 || index >= this.lineCount)
      throw new RangeError(`invalid line ${index}`);
    const from = lineStartOffset(this.root, this.blocks, index);
    const nextBreak =
      this.root && index < this.root.summary.lineBreaks
        ? lineBreakOffset(this.root, this.blocks, index)
        : undefined;
    const breakTo = nextBreak === undefined ? this.length : nextBreak + 1;
    const lineEnd = nextBreak === undefined ? this.length : nextBreak;
    const breakFrom =
      lineEnd > from && charCodeAtNode(this.root, this.blocks, lineEnd - 1) === 13
        ? lineEnd - 1
        : lineEnd;
    return { index, from, to: breakFrom, breakFrom, breakTo };
  }
  lineRangeAt(offset: number): LineRange {
    if (!Number.isInteger(offset) || offset < 0 || offset > this.length)
      throw new RangeError(`invalid offset ${offset}`);
    const index = lineIndexAtOffset(this.root, this.blocks, offset);
    const prevBreak = previousLineBreakBefore(this.root, this.blocks, offset);
    const from = prevBreak === undefined ? 0 : prevBreak + 1;
    const nextBreak = nextLineBreakAtOrAfter(this.root, this.blocks, offset);
    const breakTo = nextBreak === undefined ? this.length : nextBreak + 1;
    const lineEnd = nextBreak === undefined ? this.length : nextBreak;
    const breakFrom =
      lineEnd > from && charCodeAtNode(this.root, this.blocks, lineEnd - 1) === 13
        ? lineEnd - 1
        : lineEnd;
    return { index, from, to: breakFrom, breakFrom, breakTo };
  }
  lineText(index: number): string {
    const r = this.lineRange(index);
    return this.sliceString(r.from, r.to);
  }
  cursorAt(offset: number): TextCursor | undefined {
    return this.chunkAt(offset);
  }
  chunkAt(offset: number): TextCursor | undefined {
    if (!Number.isInteger(offset) || offset < 0 || offset > this.length)
      throw new RangeError(`invalid offset ${offset}`);
    const c =
      offset === this.length && offset > 0
        ? chunkAtOffset(this.root, this.blocks, offset - 1)
        : chunkAtOffset(this.root, this.blocks, offset);
    return c ? { offset, chunk: c.chunk, chunkFrom: c.from, chunkTo: c.to } : undefined;
  }
  visibleLineRanges(startLine: number, count: number, emit: (line: LineRange) => void): void {
    if (!Number.isInteger(startLine) || !Number.isInteger(count) || startLine < 0 || count < 0)
      throw new RangeError("invalid visible line range");
    this.lineRangesBetween(startLine, Math.min(this.lineCount, startLine + count), emit);
  }
  lineRangesBetween(fromLine: number, toLine: number, emit: (line: LineRange) => void): void {
    checkLineRange(fromLine, toLine, this.lineCount);
    for (let i = fromLine; i < toLine; i++) emit(this.lineRange(i));
  }
  chunksForLine(line: number, emit: (chunk: LineChunk) => void): void {
    const r = this.lineRange(line);
    let pos = r.from;
    this.chunksBetween(r.from, r.to, (text) => {
      const from = pos;
      const to = pos + text.length;
      pos = to;
      if (text.length > 0) emit({ from, to, text });
    });
  }
  visibleChunks(startLine: number, count: number, emit: (chunk: VisibleChunk) => void): void {
    if (!Number.isInteger(startLine) || !Number.isInteger(count) || startLine < 0 || count < 0)
      throw new RangeError("invalid visible line range");
    this.chunksForLines(startLine, Math.min(this.lineCount, startLine + count), emit);
  }
  chunksForLines(fromLine: number, toLine: number, emit: (chunk: VisibleChunk) => void): void {
    checkLineRange(fromLine, toLine, this.lineCount);
    for (let line = fromLine; line < toLine; line++)
      this.chunksForLine(line, (chunk) => emit({ line, ...chunk }));
  }
  visibleLines(startLine: number, count: number, emit: (line: VisibleLine) => void): void {
    this.linesBetween(startLine, Math.min(this.lineCount, startLine + count), emit);
  }
  linesBetween(fromLine: number, toLine: number, emit: (line: VisibleLine) => void): void {
    checkLineRange(fromLine, toLine, this.lineCount);
    if (fromLine === toLine) return;
    let index = fromLine;
    let from = lineStartOffset(this.root, this.blocks, fromLine);
    let pos = from;
    let text = "";
    this.chunksBetween(from, this.length, (chunk) => {
      if (index >= toLine) return;
      for (let i = 0; i < chunk.length && index < toLine; i++) {
        const code = chunk.charCodeAt(i);
        if (code === 10) {
          const breakTo = pos + 1;
          const breakFrom = text.endsWith("\r") ? pos - 1 : pos;
          const lineText = text.endsWith("\r") ? text.slice(0, -1) : text;
          emit({ index, from, to: breakFrom, breakFrom, breakTo, text: lineText });
          index++;
          from = breakTo;
          text = "";
        } else text += chunk[i];
        pos++;
      }
    });
    if (index < toLine)
      emit({ index, from, to: this.length, breakFrom: this.length, breakTo: this.length, text });
  }
  offsetToLineColumn(offset: number): LineColumn {
    const r = this.lineRangeAt(offset);
    return { line: r.index, column: Math.max(0, Math.min(offset, r.to) - r.from) };
  }
  lineColumnToOffset(line: number, column: number, options?: { readonly clamp?: boolean }): number {
    if (!Number.isInteger(line) || line < 0 || line >= this.lineCount)
      throw new RangeError(`invalid line ${line}`);
    const from = lineStartOffset(this.root, this.blocks, line);
    const nextBreak =
      this.root && line < this.root.summary.lineBreaks
        ? lineBreakOffset(this.root, this.blocks, line)
        : undefined;
    const lineEnd = nextBreak === undefined ? this.length : nextBreak;
    const to =
      lineEnd > from && charCodeAtNode(this.root, this.blocks, lineEnd - 1) === 13
        ? lineEnd - 1
        : lineEnd;
    if (!Number.isInteger(column) || column < 0) throw new RangeError(`invalid column ${column}`);
    const len = to - from;
    if (column > len) {
      if (options?.clamp) return to;
      throw new RangeError(`invalid column ${column}`);
    }
    return from + column;
  }
  replace(from: number, to: number, text: string): TextBuffer {
    checkRange(from, to, this.length);
    if (from === to && text.length === 0) return this;
    return replaceMany(this, [{ from, to, text }]);
  }
  apply(changes: ChangeSet): TextBuffer {
    return replaceMany(this, changes.replacements);
  }
  stats(): TextStats {
    return collectTextStats(this.root, this.blocks, this.length, this.lineCount, this.mutation);
  }
  assert(): void {
    for (let bi = 0; bi < this.blocks.length; bi++) {
      const block = this.blocks[bi]!;
      let prev = -1;
      let count = 0;
      for (let i = 0; i < block.text.length; i++)
        if (block.text.charCodeAt(i) === 10) {
          if (block.lineBreaks[count] !== i || i <= prev)
            throw new Error(`block ${bi} lineBreaks invalid`);
          prev = i;
          count++;
        }
      if (count !== block.lineBreaks.length) throw new Error(`block ${bi} lineBreak count invalid`);
    }
    const checkNode = (
      node: Node | undefined,
      depth: number,
      leafDepth: { value: number | undefined },
    ): { length: number; lineBreaks: number } => {
      if (!node) return { length: 0, lineBreaks: 0 };
      if (node.kind === "leaf") {
        if (leafDepth.value === undefined) leafDepth.value = depth;
        else if (leafDepth.value !== depth) throw new Error("leaves have different depths");
        if (node.pieces.length === 0) throw new Error("empty leaf");
        if (node.pieces.length > LeafMaxPieces) throw new Error("leaf too large");
        let summary = emptySummary;
        let length = 0,
          lineBreaks = 0;
        for (const p of node.pieces) {
          if (p.length <= 0) throw new Error("zero-length piece");
          if (p.kind === "block") {
            const b = this.blocks[p.block];
            if (!b || p.start < 0 || p.length < 0 || p.start + p.length > b.text.length)
              throw new Error("invalid block piece bounds");
            if (p.lineBreaks !== countBlockLineBreaks(b, p.start, p.length))
              throw new Error("invalid block piece lineBreaks");
          } else if (p.length !== p.text.length || p.lineBreaks !== countLineBreaks(p.text))
            throw new Error("invalid inline piece");
          length += p.length;
          lineBreaks += p.lineBreaks;
          summary = addSummary(summary, {
            length: p.length,
            lineBreaks: p.lineBreaks,
            firstLineLength: p.firstLineLength,
            lastLineLength: p.lastLineLength,
            maxLineLength: p.maxLineLength,
            startsWithLF: p.startsWithLF,
            endsWithCR: p.endsWithCR,
            maxLineLengthBeforeLast: p.maxLineLengthBeforeLast,
          });
        }
        if (
          node.summary.length !== length ||
          node.summary.lineBreaks !== lineBreaks ||
          node.summary.maxLineLength !== summary.maxLineLength ||
          node.summary.firstLineLength !== summary.firstLineLength ||
          node.summary.lastLineLength !== summary.lastLineLength
        )
          throw new Error("leaf summary invalid");
        return { length, lineBreaks };
      }
      if (node.children.length === 0) throw new Error("empty branch");
      if (node.children.length > BranchMaxChildren) throw new Error("branch too large");
      let summary = emptySummary;
      let length = 0,
        lineBreaks = 0;
      for (const child of node.children) {
        if (child.height !== node.height - 1) throw new Error("branch children height invalid");
        const s = checkNode(child, depth + 1, leafDepth);
        length += s.length;
        lineBreaks += s.lineBreaks;
        summary = addSummary(summary, child.summary);
      }
      if (
        node.summary.length !== length ||
        node.summary.lineBreaks !== lineBreaks ||
        node.summary.maxLineLength !== summary.maxLineLength ||
        node.summary.firstLineLength !== summary.firstLineLength ||
        node.summary.lastLineLength !== summary.lastLineLength
      )
        throw new Error("branch summary invalid");
      return { length, lineBreaks };
    };
    if (this.root?.kind === "branch" && this.root.children.length === 1)
      throw new Error("root not collapsed");
    const summary = checkNode(this.root, 0, { value: undefined });
    if (summary.length !== this.length) throw new Error("length invariant failed");
    if (summary.lineBreaks + 1 !== this.lineCount) throw new Error("line count invariant failed");
    const exact = summarizeText(this.toString());
    if (
      this.root &&
      (this.root.summary.length !== exact.length ||
        this.root.summary.lineBreaks !== exact.lineBreaks ||
        this.root.summary.firstLineLength !== exact.firstLineLength ||
        this.root.summary.lastLineLength !== exact.lastLineLength ||
        this.root.summary.maxLineLength !== exact.maxLineLength)
    )
      throw new Error("root rich summary invalid");
  }
}
export type TextSnapshot = TextBuffer;
/** @internal Unsafe test/debug state hook. Do not use in product code. */
export const unsafeTextBufferDebugState = (buffer: TextBuffer): TextBufferDebugState =>
  TextBuffer.unsafeDebugState(buffer);

export type OffsetSeek = {
  readonly path: readonly {
    readonly node: Node;
    readonly childIndex: number;
    readonly base: number;
  }[];
  readonly leaf: Extract<Node, { kind: "leaf" }>;
  readonly leafIndex: number;
  readonly pieceIndex: number;
  readonly pieceOffset: number;
  readonly absoluteOffset: number;
};
export const seekOffset = (
  root: Node | undefined,
  offset: number,
  assoc: -1 | 1 = 1,
): OffsetSeek | undefined => {
  if (!root || offset < 0 || offset > root.summary.length) return undefined;
  let node = root;
  let base = 0;
  const path: { node: Node; childIndex: number; base: number }[] = [];
  while (node.kind === "branch") {
    let pos = base;
    let chosen = node.children.length - 1;
    for (let i = 0; i < node.children.length; i++) {
      const child = node.children[i]!;
      const end = pos + child.summary.length;
      if (offset < end || (offset === end && assoc < 0) || i === node.children.length - 1) {
        chosen = i;
        break;
      }
      pos = end;
    }
    path.push({ node, childIndex: chosen, base });
    node = node.children[chosen]!;
    base = pos;
  }
  let pos = base;
  for (let i = 0; i < node.pieces.length; i++) {
    const p = node.pieces[i]!;
    const end = pos + p.length;
    if (offset < end || (offset === end && assoc < 0) || i === node.pieces.length - 1)
      return {
        path,
        leaf: node,
        leafIndex: path[path.length - 1]?.childIndex ?? 0,
        pieceIndex: i,
        pieceOffset: Math.max(0, Math.min(p.length, offset - pos)),
        absoluteOffset: pos,
      };
    pos = end;
  }
  return {
    path,
    leaf: node,
    leafIndex: path[path.length - 1]?.childIndex ?? 0,
    pieceIndex: 0,
    pieceOffset: 0,
    absoluteOffset: base,
  };
};

const piecesForText = (
  text: string,
  blocks: Block[],
  stats: { piecesCreated: number; blockAppends: number },
): Piece[] => {
  const beforeBlocks = blocks.length;
  const pieces = buildPiecesFromChunks([text], blocks);
  stats.piecesCreated += pieces.length;
  stats.blockAppends += blocks.length - beforeBlocks;
  return pieces;
};

const nodesFromPieces = (pieces: readonly Piece[]): Node[] => {
  if (pieces.length === 0) return [];
  const leaves = [] as Node[];
  for (let i = 0; i < pieces.length; i += LeafMaxPieces)
    leaves.push(leaf(pieces.slice(i, i + LeafMaxPieces)));
  return leaves;
};
const collapseRoot = (root: Node | undefined): Node | undefined => {
  let node = root;
  while (node?.kind === "branch" && node.children.length === 1) node = node.children[0];
  return node;
};
const normalizeForest = (
  nodes: readonly Node[],
  counters: Partial<MutableMutationStats>,
): Node | undefined => {
  if (nodes.length === 0) return undefined;
  let level = [...nodes];
  if (!level.every((n) => n.height === level[0]!.height)) {
    const pieces = flattenForest(level);
    counters.subtreeRebuilds = (counters.subtreeRebuilds ?? 0) + 1;
    counters.fullRebuilds = (counters.fullRebuilds ?? 0) + 1;
    counters.flattenedPieces = (counters.flattenedPieces ?? 0) + pieces.length;
    return buildTree(pieces);
  }
  while (level.length > 1) {
    const next: Node[] = [];
    for (let i = 0; i < level.length; i += BranchMaxChildren)
      next.push(branch(level.slice(i, i + BranchMaxChildren)));
    level = next;
  }
  return collapseRoot(level[0]);
};
const flattenForest = (nodes: readonly Node[]): Piece[] => {
  const out: Piece[] = [];
  for (const n of nodes) out.push(...flattenPieces(n));
  return out;
};

const replaceMany = (buffer: TextBuffer, replacements: readonly Replacement[]): TextBuffer => {
  if (replacements.length === 0) return buffer;
  let last = 0;
  for (const r of replacements) {
    checkRange(r.from, r.to, buffer.length);
    if (r.from < last) throw new RangeError("overlapping replacements");
    last = r.to;
  }
  const state = TextBuffer.unsafeDebugState(buffer);
  const blocks = [...state.blocks];
  const counters: MutableMutationStats = {
    pathCopies: 0,
    leafRebuilds: 0,
    branchRebuilds: 0,
    subtreeRebuilds: 0,
    fullRebuilds: 0,
    flattenedPieces: 0,
    localRepackedPieces: 0,
    localRepackedChildren: 0,
    piecesCreated: 0,
    piecesCoalesced: 0,
    blockAppends: 0,
    leafMerges: 0,
    leafRedistributions: 0,
    branchMerges: 0,
    branchRedistributions: 0,
  };
  const inserts = replacements.map((r) => piecesForText(r.text, blocks, counters));
  type EditResult = {
    readonly nodes: readonly Node[];
    readonly affectedFrom: number;
    readonly affectedTo: number;
    readonly changed: boolean;
  };
  const replacementRelevantToRange = (r: Replacement, from: number, to: number): boolean =>
    r.from === r.to
      ? r.from === 0
        ? from === 0
        : r.from > from && r.from <= to
      : r.from < to && r.to > from;
  const relevantRangeForNode = (
    from: number,
    to: number,
    replacementFrom: number,
    replacementTo: number,
  ): readonly [number, number] => {
    let start = replacementTo;
    let end = replacementFrom;
    for (let i = replacementFrom; i < replacementTo; i++)
      if (replacementRelevantToRange(replacements[i]!, from, to)) {
        if (start === replacementTo) start = i;
        end = i + 1;
      }
    return start === replacementTo ? [replacementTo, replacementTo] : [start, end];
  };
  const editNode = (
    node: Node,
    base: number,
    replacementFrom: number,
    replacementTo: number,
  ): EditResult => {
    const end = base + node.summary.length;
    const [first, last] = relevantRangeForNode(base, end, replacementFrom, replacementTo);
    if (first === last) return { nodes: [node], affectedFrom: -1, affectedTo: -1, changed: false };
    if (node.kind === "leaf") {
      counters.leafRebuilds++;
      counters.pathCopies++;
      const out: Piece[] = [];
      let pos = base;
      let localRi = first;
      for (const p of node.pieces) {
        const pEnd = pos + p.length;
        let cursor = pos;
        while (localRi < last && replacements[localRi]!.from < pEnd) {
          const r = replacements[localRi]!;
          if (r.from > cursor) {
            const q = slicePiece(p, cursor - pos, Math.min(r.from, pEnd) - pos, blocks);
            if (q) out.push(q);
          }
          if (r.from >= pos && r.from <= pEnd) out.push(...inserts[localRi]!);
          cursor = Math.max(cursor, Math.min(r.to, pEnd));
          if (r.to <= pEnd) localRi++;
          else break;
        }
        if (cursor < pEnd) {
          const q = slicePiece(p, cursor - pos, p.length, blocks);
          if (q) out.push(q);
        }
        pos = pEnd;
      }
      while (
        localRi < last &&
        replacements[localRi]!.from === end &&
        replacements[localRi]!.to === end
      ) {
        out.push(...inserts[localRi]!);
        localRi++;
      }
      const before = out.length;
      const compact = coalescePieces(out, blocks);
      counters.piecesCoalesced += before - compact.length;
      const nodes = nodesFromPieces(compact);
      return {
        nodes,
        affectedFrom: nodes.length > 0 ? 0 : -1,
        affectedTo: nodes.length - 1,
        changed: true,
      };
    }
    counters.branchRebuilds++;
    counters.pathCopies++;
    const children: Node[] = [];
    let pos = base;
    let firstAffected = -1;
    let lastAffected = -1;
    for (const child of node.children) {
      const childEnd = pos + child.summary.length;
      const [childFrom, childTo] = relevantRangeForNode(pos, childEnd, first, last);
      const beforeCount = children.length;
      if (childFrom === childTo) children.push(child);
      else {
        const result = editNode(child, pos, childFrom, childTo);
        children.push(...result.nodes);
        if (result.changed) {
          if (firstAffected < 0) firstAffected = beforeCount + Math.max(0, result.affectedFrom);
          lastAffected = beforeCount + Math.max(0, result.affectedTo);
        }
      }
      pos = childEnd;
    }
    if (firstAffected < 0)
      return { nodes: [node], affectedFrom: -1, affectedTo: -1, changed: false };
    const repaired = repairLocalWindow(children, firstAffected, lastAffected, counters);
    return { nodes: repaired, affectedFrom: 0, affectedTo: repaired.length - 1, changed: true };
  };
  const forest = state.root
    ? editNode(state.root, 0, 0, replacements.length).nodes
    : nodesFromPieces(coalescePieces(inserts.flat(), blocks));
  const root = normalizeForest(forest, counters);
  return TextBuffer.unsafeFromParts(
    blocks,
    root,
    addMutation(TextBuffer.unsafeDebugState(buffer).mutation, counters),
  );
};

const repairLocalWindow = (
  children: readonly Node[],
  firstAffected: number,
  lastAffected: number,
  counters: MutableMutationStats,
): Node[] => {
  if (children.length === 0) return [];
  if (firstAffected < 0) return [...children];
  const start = Math.max(0, firstAffected - 2);
  const end = Math.min(children.length, lastAffected + 3);
  const prefix = children.slice(0, start);
  const window = children.slice(start, end);
  const suffix = children.slice(end);
  const repaired = repairDensity(window, counters);
  const joined = [...prefix, ...repaired, ...suffix];
  if (!joined.every((n) => n.height === joined[0]!.height)) {
    const pieces = flattenForest(joined);
    counters.subtreeRebuilds++;
    counters.fullRebuilds++;
    counters.flattenedPieces += pieces.length;
    return nodesFromPieces(pieces);
  }
  const out: Node[] = [];
  for (let i = 0; i < joined.length; i += BranchMaxChildren)
    out.push(branch(joined.slice(i, i + BranchMaxChildren)));
  return out;
};

const repairDensity = (nodes: readonly Node[], counters: MutableMutationStats): Node[] => {
  if (nodes.length <= 1) return [...nodes];
  if (!nodes.every((n) => n.height === nodes[0]!.height)) {
    const pieces = flattenForest(nodes);
    counters.subtreeRebuilds++;
    counters.fullRebuilds++;
    counters.flattenedPieces += pieces.length;
    return nodesFromPieces(pieces);
  }
  if (nodes[0]!.kind === "leaf") {
    let totalPieces = 0;
    for (const n of nodes) totalPieces += n.kind === "leaf" ? n.pieces.length : 0;
    const target = totalPieces <= LeafMaxPieces ? LeafMaxPieces : 48;
    if (nodes.length > 1) counters.leafRedistributions++;
    counters.localRepackedPieces += totalPieces;
    counters.leafMerges += Math.max(0, nodes.length - Math.ceil(totalPieces / target));
    const out: Node[] = [];
    let batch: Piece[] = [];
    const flush = () => {
      if (batch.length > 0) {
        out.push(leaf(batch));
        batch = [];
      }
    };
    for (const n of nodes)
      if (n.kind === "leaf")
        for (const p of n.pieces) {
          batch.push(p);
          if (batch.length >= target) flush();
        }
    flush();
    return out;
  }
  const height = nodes[0]!.height;
  const grandchildren: Node[] = [];
  for (const n of nodes) if (n.kind === "branch") grandchildren.push(...n.children);
  if (!grandchildren.every((n) => n.height === height - 1)) {
    const pieces = flattenForest(nodes);
    counters.subtreeRebuilds++;
    counters.fullRebuilds++;
    counters.flattenedPieces += pieces.length;
    return nodesFromPieces(pieces);
  }
  if (nodes.length > 1) counters.branchRedistributions++;
  counters.localRepackedChildren += grandchildren.length;
  counters.branchMerges += Math.max(
    0,
    nodes.length - Math.ceil(grandchildren.length / BranchMaxChildren),
  );
  const out: Node[] = [];
  for (let i = 0; i < grandchildren.length; i += BranchMaxChildren)
    out.push(branch(grandchildren.slice(i, i + BranchMaxChildren)));
  return out;
};

const emitChunksBetween = (
  node: Node,
  blocks: readonly Block[],
  from: number,
  to: number,
  base: number,
  emit: (chunk: string) => void,
): void => {
  const nodeTo = base + node.summary.length;
  if (nodeTo <= from || base >= to) return;
  if (node.kind === "branch") {
    let pos = base;
    for (const child of node.children) {
      emitChunksBetween(child, blocks, from, to, pos, emit);
      pos += child.summary.length;
      if (pos >= to) break;
    }
    return;
  }
  let pos = base;
  for (const piece of node.pieces) {
    const end = pos + piece.length;
    if (end > from && pos < to)
      emit(
        pieceText(piece, blocks).slice(Math.max(0, from - pos), Math.min(piece.length, to - pos)),
      );
    pos = end;
    if (pos >= to) break;
  }
};

export type ChunkAtOffset = { readonly chunk: string; readonly from: number; readonly to: number };
export const lineBreakOffset = (
  root: Node | undefined,
  blocks: readonly Block[],
  breakIndex: number,
): number => {
  if (
    !root ||
    !Number.isInteger(breakIndex) ||
    breakIndex < 0 ||
    breakIndex >= root.summary.lineBreaks
  )
    throw new RangeError(`invalid line break ${breakIndex}`);
  return seekLineBreak(root, blocks, breakIndex, 0);
};
export const lineStartOffset = (
  root: Node | undefined,
  blocks: readonly Block[],
  lineIndex: number,
): number => {
  const lineCount = (root?.summary.lineBreaks ?? 0) + 1;
  if (!Number.isInteger(lineIndex) || lineIndex < 0 || lineIndex >= lineCount)
    throw new RangeError(`invalid line ${lineIndex}`);
  return lineIndex === 0 ? 0 : lineBreakOffset(root, blocks, lineIndex - 1) + 1;
};
const chunkAtOffset = (
  root: Node | undefined,
  blocks: readonly Block[],
  offset: number,
): ChunkAtOffset | undefined => {
  if (!root || offset < 0 || offset >= root.summary.length) return undefined;
  const seek = seekOffset(root, offset, 1);
  if (!seek) return undefined;
  const p = seek.leaf.pieces[seek.pieceIndex];
  if (!p) return undefined;
  const from = seek.absoluteOffset;
  return { chunk: pieceText(p, blocks), from, to: from + p.length };
};
const charCodeAtNode = (
  root: Node | undefined,
  blocks: readonly Block[],
  offset: number,
): number | undefined => {
  const c = chunkAtOffset(root, blocks, offset);
  if (!c) return undefined;
  const code = c.chunk.charCodeAt(offset - c.from);
  return Number.isNaN(code) ? undefined : code;
};
const nextLineBreakAtOrAfter = (
  root: Node | undefined,
  blocks: readonly Block[],
  offset: number,
): number | undefined => {
  if (!root || offset >= root.summary.length) return undefined;
  const countBefore = lineIndexAtOffset(root, blocks, offset);
  if (countBefore >= root.summary.lineBreaks) return undefined;
  const candidate = lineBreakOffset(root, blocks, countBefore);
  return candidate >= offset ? candidate : undefined;
};
const previousLineBreakBefore = (
  root: Node | undefined,
  blocks: readonly Block[],
  offset: number,
): number | undefined => {
  if (!root || offset <= 0) return undefined;
  const countBefore = lineIndexAtOffset(root, blocks, offset);
  return countBefore === 0 ? undefined : lineBreakOffset(root, blocks, countBefore - 1);
};

const seekLineBreak = (
  node: Node,
  blocks: readonly Block[],
  lineBreakIndex: number,
  base: number,
): number => {
  if (node.kind === "branch") {
    let pos = base;
    let remaining = lineBreakIndex;
    for (const child of node.children) {
      if (remaining < child.summary.lineBreaks) return seekLineBreak(child, blocks, remaining, pos);
      remaining -= child.summary.lineBreaks;
      pos += child.summary.length;
    }
    return base + node.summary.length;
  }
  let pos = base;
  let remaining = lineBreakIndex;
  for (const piece of node.pieces) {
    if (remaining < piece.lineBreaks) {
      if (piece.kind === "block") {
        const b = blocks[piece.block]!;
        const absolute =
          b.lineBreaks[piece.lineBreakStart + remaining] ?? piece.start + piece.length;
        return pos + absolute - piece.start;
      }
      for (let i = 0; i < piece.text.length; i++)
        if (piece.text.charCodeAt(i) === 10 && remaining-- === 0) return pos + i;
    }
    remaining -= piece.lineBreaks;
    pos += piece.length;
  }
  return base + node.summary.length;
};

const lineIndexAtOffset = (
  root: Node | undefined,
  blocks: readonly Block[],
  offset: number,
): number => (root ? countBreaksBeforeOffset(root, blocks, offset, 0) : 0);

const countBreaksBeforeOffset = (
  node: Node,
  blocks: readonly Block[],
  offset: number,
  base: number,
): number => {
  if (offset <= base) return 0;
  if (offset >= base + node.summary.length) return node.summary.lineBreaks;
  if (node.kind === "branch") {
    let pos = base;
    let count = 0;
    for (const child of node.children) {
      const end = pos + child.summary.length;
      if (offset >= end) count += child.summary.lineBreaks;
      else return count + countBreaksBeforeOffset(child, blocks, offset, pos);
      pos = end;
    }
    return count;
  }
  let pos = base;
  let count = 0;
  for (const piece of node.pieces) {
    const end = pos + piece.length;
    if (offset >= end) count += piece.lineBreaks;
    else {
      const n = offset - pos;
      if (piece.kind === "block") {
        const b = blocks[piece.block]!;
        let lo = piece.lineBreakStart,
          hi = piece.lineBreakStart + piece.lineBreaks;
        const value = piece.start + n;
        while (lo < hi) {
          const mid = (lo + hi) >>> 1;
          if (b.lineBreaks[mid]! < value) lo = mid + 1;
          else hi = mid;
        }
        return count + lo - piece.lineBreakStart;
      }
      for (let i = 0; i < n; i++) if (piece.text.charCodeAt(i) === 10) count++;
      return count;
    }
    pos = end;
  }
  return count;
};

export const referenceLineRange = (text: string, index: number): LineRange => {
  let line = 0,
    start = 0;
  for (let i = 0; ; i++) {
    if (i === text.length || text.charCodeAt(i) === 10) {
      if (line === index) {
        const breakTo = i < text.length ? i + 1 : i;
        const breakFrom = i > start && text.charCodeAt(i - 1) === 13 ? i - 1 : i;
        return { index, from: start, to: breakFrom, breakFrom, breakTo };
      }
      start = i + 1;
      line++;
    }
  }
};
