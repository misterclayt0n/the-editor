import type { Block } from "./block";
import { summarizeText, type TextSummary } from "./summary";

type PieceSummaryFields = {
  readonly length: number;
  readonly lineBreaks: number;
  readonly firstLineLength: number;
  readonly lastLineLength: number;
  readonly maxLineLength: number;
  readonly startsWithLF: boolean;
  readonly endsWithCR: boolean;
  readonly maxLineLengthBeforeLast: number;
};
export type BlockPiece = PieceSummaryFields & {
  readonly kind: "block";
  readonly block: number;
  readonly start: number;
  readonly lineBreakStart: number;
};
export type InlinePiece = PieceSummaryFields & { readonly kind: "inline"; readonly text: string };
export type Piece = BlockPiece | InlinePiece;
export const InlinePieceMax = 256;
export const AddBlockSize = 64 * 1024;
const singleLineSummary = (text: string): TextSummary => ({
  length: text.length,
  lineBreaks: 0,
  firstLineLength: text.length,
  lastLineLength: text.length,
  maxLineLength: text.length,
  startsWithLF: false,
  endsWithCR: text.length > 0 && text.charCodeAt(text.length - 1) === 13,
  maxLineLengthBeforeLast: 0,
});
export const inlinePieceWithSummary = (text: string, summary: TextSummary): InlinePiece => ({
  kind: "inline",
  text,
  ...summary,
});
export const inlinePiece = (text: string): InlinePiece =>
  inlinePieceWithSummary(
    text,
    text.indexOf("\n") < 0 ? singleLineSummary(text) : summarizeText(text),
  );
const lowerBound = (xs: readonly number[], value: number): number => {
  let lo = 0,
    hi = xs.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (xs[mid]! < value) lo = mid + 1;
    else hi = mid;
  }
  return lo;
};
const prefixMax = (block: Block, fromLine: number, toLineExclusive: number): number => {
  if (toLineExclusive <= fromLine) return 0;
  const end = block.prefixMaxLineLengths[toLineExclusive - 1] ?? 0;
  if (fromLine <= 0) return end;
  // Rare fallback: prefix maxima are not invertible. This scans only fully contained line records, not text.
  let max = 0;
  for (let i = fromLine; i < toLineExclusive; i++) max = Math.max(max, block.lineLengths[i] ?? 0);
  return max;
};
export const countBlockLineBreaks = (block: Block, start: number, length: number): number =>
  lowerBound(block.lineBreaks, start + length) - lowerBound(block.lineBreaks, start);
export const summarizeBlockRange = (block: Block, start: number, length: number): TextSummary => {
  const end = start + length;
  if (start === 0 && end === block.text.length) return block.summary;
  const lineBreakStart = lowerBound(block.lineBreaks, start);
  const lineBreakEnd = lowerBound(block.lineBreaks, end);
  const lineBreaks = lineBreakEnd - lineBreakStart;
  const startsWithLF = length > 0 && block.text.charCodeAt(start) === 10;
  const endsWithCR = length > 0 && block.text.charCodeAt(end - 1) === 13;
  if (lineBreaks === 0)
    return {
      length,
      lineBreaks,
      firstLineLength: length,
      lastLineLength: length,
      maxLineLength: length,
      startsWithLF,
      endsWithCR,
      maxLineLengthBeforeLast: 0,
    };
  const firstLf = block.lineBreaks[lineBreakStart]!;
  const firstLineLength =
    firstLf > start && block.text.charCodeAt(firstLf - 1) === 13
      ? firstLf - start - 1
      : firstLf - start;
  const lastLf = block.lineBreaks[lineBreakEnd - 1]!;
  const lastLineLength = end - (lastLf + 1);
  const fullLineStart = lineBreakStart + 1;
  const fullLineEnd = lineBreakEnd;
  const middleMax = prefixMax(block, fullLineStart, fullLineEnd);
  const maxLineLengthBeforeLast = Math.max(firstLineLength, middleMax);
  return {
    length,
    lineBreaks,
    firstLineLength,
    lastLineLength,
    maxLineLength: Math.max(maxLineLengthBeforeLast, lastLineLength),
    startsWithLF,
    endsWithCR,
    maxLineLengthBeforeLast,
  };
};
export const blockPiece = (
  block: number,
  start: number,
  length: number,
  blocks: readonly Block[],
): BlockPiece => ({
  kind: "block",
  block,
  start,
  lineBreakStart: lowerBound(blocks[block]!.lineBreaks, start),
  ...summarizeBlockRange(blocks[block]!, start, length),
});
export const pieceSummary = (p: Piece): TextSummary => ({
  length: p.length,
  lineBreaks: p.lineBreaks,
  firstLineLength: p.firstLineLength,
  lastLineLength: p.lastLineLength,
  maxLineLength: p.maxLineLength,
  startsWithLF: p.startsWithLF,
  endsWithCR: p.endsWithCR,
  maxLineLengthBeforeLast: p.maxLineLengthBeforeLast,
});
export const pieceText = (p: Piece, blocks: readonly Block[]): string =>
  p.kind === "inline" ? p.text : blocks[p.block]!.text.slice(p.start, p.start + p.length);
export const slicePiece = (
  p: Piece,
  from: number,
  to: number,
  blocks: readonly Block[],
): Piece | undefined => {
  if (from === 0 && to === p.length) return p;
  const len = to - from;
  if (len <= 0) return undefined;
  if (p.kind === "inline") return inlinePiece(p.text.slice(from, to));
  return blockPiece(p.block, p.start + from, len, blocks);
};
