import type { Block } from "./block";
import { makeBlock } from "./block";
import { addSummary } from "./summary";
import {
  AddBlockSize,
  InlinePieceMax,
  blockPiece,
  inlinePiece,
  inlinePieceWithSummary,
  pieceText,
  slicePiece,
  type Piece,
} from "./piece";
import { BranchMaxChildren, LeafMaxPieces, branch, leaf, type Node } from "./node";

export const buildPiecesFromChunks = (chunks: readonly string[], blocks: Block[]): Piece[] => {
  const pieces: Piece[] = [];
  for (const chunk of chunks) {
    if (chunk.length === 0) continue;
    if (chunk.length <= InlinePieceMax) pieces.push(inlinePiece(chunk));
    else {
      for (let start = 0; start < chunk.length; start += AddBlockSize) {
        const text = chunk.slice(start, start + AddBlockSize);
        const ix = blocks.push(makeBlock(text)) - 1;
        pieces.push(blockPiece(ix, 0, text.length, blocks));
      }
    }
  }
  return pieces;
};

export const buildTree = (pieces: readonly Piece[]): Node | undefined => {
  if (pieces.length === 0) return undefined;
  let level: Node[] = [];
  for (let i = 0; i < pieces.length; i += LeafMaxPieces)
    level.push(leaf(pieces.slice(i, i + LeafMaxPieces)));
  while (level.length > 1) {
    const next: Node[] = [];
    for (let i = 0; i < level.length; i += BranchMaxChildren)
      next.push(branch(level.slice(i, i + BranchMaxChildren)));
    level = next;
  }
  return level[0];
};

export const flattenPieces = (root: Node | undefined): readonly Piece[] => {
  if (!root) return [];
  const out: Piece[] = [];
  const walk = (n: Node): void => {
    if (n.kind === "leaf") out.push(...n.pieces);
    else for (const c of n.children) walk(c);
  };
  walk(root);
  return out;
};

export const coalescePieces = (pieces: readonly Piece[], blocks: readonly Block[]): Piece[] => {
  const out: Piece[] = [];
  for (const p of pieces) {
    if (p.length === 0) continue;
    const last = out[out.length - 1];
    if (last?.kind === "inline" && p.kind === "inline" && last.length + p.length <= InlinePieceMax)
      out[out.length - 1] = inlinePieceWithSummary(last.text + p.text, addSummary(last, p));
    else if (
      last?.kind === "block" &&
      p.kind === "block" &&
      last.block === p.block &&
      last.start + last.length === p.start
    ) {
      const summary = addSummary(last, p);
      out[out.length - 1] = {
        kind: "block",
        block: last.block,
        start: last.start,
        lineBreakStart: last.lineBreakStart,
        ...summary,
      };
    } else out.push(p);
  }
  return out;
};

export const piecesToString = (pieces: readonly Piece[], blocks: readonly Block[]): string =>
  pieces.map((p) => pieceText(p, blocks)).join("");

export const slicePieces = (
  pieces: readonly Piece[],
  from: number,
  to: number,
  blocks: readonly Block[],
): Piece[] => {
  const out: Piece[] = [];
  let pos = 0;
  for (const p of pieces) {
    const end = pos + p.length;
    if (end > from && pos < to) {
      const s = Math.max(from, pos) - pos;
      const e = Math.min(to, end) - pos;
      const q = slicePiece(p, s, e, blocks);
      if (q) out.push(q);
    }
    pos = end;
    if (pos >= to) break;
  }
  return out;
};
