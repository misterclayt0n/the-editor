import type { Block } from "./block";
import type { Node } from "./node";
import { flattenPieces } from "./tree";

export type MutationStats = {
  readonly pathCopies: number;
  readonly leafRebuilds: number;
  readonly branchRebuilds: number;
  readonly subtreeRebuilds: number;
  readonly fullRebuilds: number;
  readonly flattenedPieces: number;
  readonly localRepackedPieces: number;
  readonly localRepackedChildren: number;
  readonly piecesCreated: number;
  readonly piecesCoalesced: number;
  readonly blockAppends: number;
  readonly leafMerges: number;
  readonly leafRedistributions: number;
  readonly branchMerges: number;
  readonly branchRedistributions: number;
};
export type MutableMutationStats = { -readonly [K in keyof MutationStats]: MutationStats[K] };
export type TextStats = {
  readonly length: number;
  readonly lineCount: number;
  readonly pieces: number;
  readonly inlinePieces: number;
  readonly blockPieces: number;
  readonly leaves: number;
  readonly branches: number;
  readonly height: number;
  readonly blocks: number;
  readonly maxLeafPieces: number;
  readonly avgLeafPieces: number;
  readonly avgBranchChildren: number;
  readonly estimatedPieceObjects: number;
  readonly estimatedNodeObjects: number;
  readonly estimatedBlockChars: number;
  readonly estimatedInlineChars: number;
  readonly estimatedRetainedChars: number;
  readonly estimatedUnreferencedBlockChars: number;
  readonly mutation: MutationStats;
};
export const emptyMutationStats: MutationStats = {
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
export const addMutation = (a: MutationStats, b: Partial<MutationStats>): MutationStats => ({
  pathCopies: a.pathCopies + (b.pathCopies ?? 0),
  leafRebuilds: a.leafRebuilds + (b.leafRebuilds ?? 0),
  branchRebuilds: a.branchRebuilds + (b.branchRebuilds ?? 0),
  subtreeRebuilds: a.subtreeRebuilds + (b.subtreeRebuilds ?? 0),
  fullRebuilds: a.fullRebuilds + (b.fullRebuilds ?? 0),
  flattenedPieces: a.flattenedPieces + (b.flattenedPieces ?? 0),
  localRepackedPieces: a.localRepackedPieces + (b.localRepackedPieces ?? 0),
  localRepackedChildren: a.localRepackedChildren + (b.localRepackedChildren ?? 0),
  piecesCreated: a.piecesCreated + (b.piecesCreated ?? 0),
  piecesCoalesced: a.piecesCoalesced + (b.piecesCoalesced ?? 0),
  blockAppends: a.blockAppends + (b.blockAppends ?? 0),
  leafMerges: a.leafMerges + (b.leafMerges ?? 0),
  leafRedistributions: a.leafRedistributions + (b.leafRedistributions ?? 0),
  branchMerges: a.branchMerges + (b.branchMerges ?? 0),
  branchRedistributions: a.branchRedistributions + (b.branchRedistributions ?? 0),
});
export const collectTextStats = (
  root: Node | undefined,
  blocks: readonly Block[],
  length: number,
  lineCount: number,
  mutation: MutationStats,
): TextStats => {
  const pieces = flattenPieces(root);
  let leaves = 0,
    branches = 0,
    maxLeafPieces = 0,
    branchChildren = 0;
  const walk = (n: Node | undefined): void => {
    if (!n) return;
    if (n.kind === "leaf") {
      leaves++;
      maxLeafPieces = Math.max(maxLeafPieces, n.pieces.length);
    } else {
      branches++;
      branchChildren += n.children.length;
      for (const c of n.children) walk(c);
    }
  };
  walk(root);
  let estimatedInlineChars = 0,
    estimatedRetainedChars = 0;
  for (const p of pieces)
    p.kind === "inline" ? (estimatedInlineChars += p.length) : (estimatedRetainedChars += p.length);
  const estimatedBlockChars = blocks.reduce((n, b) => n + b.text.length, 0);
  return {
    length,
    lineCount,
    pieces: pieces.length,
    inlinePieces: pieces.filter((p) => p.kind === "inline").length,
    blockPieces: pieces.filter((p) => p.kind === "block").length,
    leaves,
    branches,
    height: root?.height ?? 0,
    blocks: blocks.length,
    maxLeafPieces,
    avgLeafPieces: leaves ? pieces.length / leaves : 0,
    avgBranchChildren: branches ? branchChildren / branches : 0,
    estimatedPieceObjects: pieces.length,
    estimatedNodeObjects: leaves + branches,
    estimatedBlockChars,
    estimatedInlineChars,
    estimatedRetainedChars,
    estimatedUnreferencedBlockChars: Math.max(0, estimatedBlockChars - estimatedRetainedChars),
    mutation,
  };
};
