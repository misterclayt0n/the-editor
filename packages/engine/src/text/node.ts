import type { Piece } from "./piece";
import { emptySummary, type TextSummary } from "./summary";

export type Leaf = {
  readonly kind: "leaf";
  readonly height: 0;
  readonly pieces: readonly Piece[];
  readonly summary: TextSummary;
};
export type Branch = {
  readonly kind: "branch";
  readonly height: number;
  readonly children: readonly Node[];
  readonly summary: TextSummary;
};
export type Node = Leaf | Branch;
export const LeafMaxPieces = 64;
export const BranchMaxChildren = 32;

type SummaryLike = TextSummary;
const summarize = (items: readonly SummaryLike[]): TextSummary => {
  let length = 0;
  let lineBreaks = 0;
  let firstLineLength = 0;
  let lastLineLength = 0;
  let maxLineLength = 0;
  let startsWithLF = false;
  let endsWithCR = false;
  let maxLineLengthBeforeLast = 0;
  for (const item of items) {
    if (item.length === 0) continue;
    if (length === 0) {
      length = item.length;
      lineBreaks = item.lineBreaks;
      firstLineLength = item.firstLineLength;
      lastLineLength = item.lastLineLength;
      maxLineLength = item.maxLineLength;
      startsWithLF = item.startsWithLF;
      endsWithCR = item.endsWithCR;
      maxLineLengthBeforeLast = item.maxLineLengthBeforeLast;
      continue;
    }
    const crlfJoin = endsWithCR && item.startsWithLF;
    const joinedLine = lastLineLength + item.firstLineLength - (crlfJoin ? 1 : 0);
    const nextLineBreaks = lineBreaks + item.lineBreaks;
    const nextFirstLineLength =
      lineBreaks === 0 ? length + item.firstLineLength - (crlfJoin ? 1 : 0) : firstLineLength;
    const nextLastLineLength =
      item.lineBreaks === 0 ? lastLineLength + item.length : item.lastLineLength;
    const nextMaxBeforeLast =
      item.lineBreaks === 0
        ? maxLineLengthBeforeLast
        : Math.max(maxLineLengthBeforeLast, joinedLine, item.maxLineLengthBeforeLast);
    length += item.length;
    lineBreaks = nextLineBreaks;
    firstLineLength = nextFirstLineLength;
    lastLineLength = nextLastLineLength;
    maxLineLengthBeforeLast = nextMaxBeforeLast;
    maxLineLength = Math.max(
      nextMaxBeforeLast,
      nextLastLineLength,
      item.lineBreaks === 0 ? joinedLine : 0,
    );
    endsWithCR = item.endsWithCR;
  }
  return length === 0
    ? emptySummary
    : {
        length,
        lineBreaks,
        firstLineLength,
        lastLineLength,
        maxLineLength,
        startsWithLF,
        endsWithCR,
        maxLineLengthBeforeLast,
      };
};
export const summarizePieces = (pieces: readonly Piece[]): TextSummary => summarize(pieces);
export const summarizeNodes = (nodes: readonly Node[]): TextSummary => {
  let length = 0,
    lineBreaks = 0,
    firstLineLength = 0,
    lastLineLength = 0,
    maxLineLength = 0,
    startsWithLF = false,
    endsWithCR = false,
    maxLineLengthBeforeLast = 0;
  for (const node of nodes) {
    const item = node.summary;
    if (item.length === 0) continue;
    if (length === 0) {
      length = item.length;
      lineBreaks = item.lineBreaks;
      firstLineLength = item.firstLineLength;
      lastLineLength = item.lastLineLength;
      maxLineLength = item.maxLineLength;
      startsWithLF = item.startsWithLF;
      endsWithCR = item.endsWithCR;
      maxLineLengthBeforeLast = item.maxLineLengthBeforeLast;
      continue;
    }
    const crlfJoin = endsWithCR && item.startsWithLF;
    const joinedLine = lastLineLength + item.firstLineLength - (crlfJoin ? 1 : 0);
    const nextMaxBeforeLast =
      item.lineBreaks === 0
        ? maxLineLengthBeforeLast
        : Math.max(maxLineLengthBeforeLast, joinedLine, item.maxLineLengthBeforeLast);
    firstLineLength =
      lineBreaks === 0 ? length + item.firstLineLength - (crlfJoin ? 1 : 0) : firstLineLength;
    lastLineLength = item.lineBreaks === 0 ? lastLineLength + item.length : item.lastLineLength;
    length += item.length;
    lineBreaks += item.lineBreaks;
    maxLineLengthBeforeLast = nextMaxBeforeLast;
    maxLineLength = Math.max(
      nextMaxBeforeLast,
      lastLineLength,
      item.lineBreaks === 0 ? joinedLine : 0,
    );
    endsWithCR = item.endsWithCR;
  }
  return length === 0
    ? emptySummary
    : {
        length,
        lineBreaks,
        firstLineLength,
        lastLineLength,
        maxLineLength,
        startsWithLF,
        endsWithCR,
        maxLineLengthBeforeLast,
      };
};
export const leaf = (pieces: readonly Piece[]): Leaf => ({
  kind: "leaf",
  height: 0,
  pieces,
  summary: summarizePieces(pieces),
});
export const branch = (children: readonly Node[]): Branch => ({
  kind: "branch",
  height: children[0]!.height + 1,
  children,
  summary: summarizeNodes(children),
});
