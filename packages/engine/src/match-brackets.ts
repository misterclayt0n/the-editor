import type { Syntax, SyntaxText } from "@the-editor/syntax";
import { codePointStringAt, nextCodePointBoundary } from "./chars";

export type BracketPair = { readonly open: string; readonly close: string };
export type MatchBracketOptions = {
  readonly pairs?: readonly BracketPair[];
  readonly maxPlaintextScan?: number;
  readonly syntaxSiblingMatchLimit?: number;
};
export const DEFAULT_BRACKET_PAIRS: readonly BracketPair[] = [
  { open: "(", close: ")" },
  { open: "{", close: "}" },
  { open: "[", close: "]" },
  { open: "<", close: ">" },
  { open: "‘", close: "’" },
  { open: "“", close: "”" },
  { open: "«", close: "»" },
  { open: "「", close: "」" },
  { open: "（", close: "）" },
];
const DEFAULT_MAX_PLAINTEXT_SCAN = 10_000;
const DEFAULT_SYNTAX_SIBLING_MATCH_LIMIT = 16;
export class MatchBracketError extends Error {}
export class InvalidBracketOffsetError extends MatchBracketError {
  constructor(readonly pos: number) {
    super(`invalid bracket offset ${pos}`);
  }
}
export class InvalidBracketPairError extends MatchBracketError {
  constructor(readonly pair: BracketPair) {
    super("invalid bracket pair");
  }
}
export class InvalidBracketScanLimitError extends MatchBracketError {
  constructor(readonly limit: number) {
    super(`invalid bracket scan limit ${limit}`);
  }
}
type Direction = "forward" | "backward";
type MatchMode = "strict" | "fuzzy";

const assertPos = (pos: number) => {
  if (!Number.isInteger(pos) || pos < 0) throw new InvalidBracketOffsetError(pos);
};
const assertLimit = (limit: number) => {
  if (!Number.isInteger(limit) || limit < 0) throw new InvalidBracketScanLimitError(limit);
};
const oneCp = (s: string) => s.length > 0 && nextCodePointBoundary(s, 0) === s.length;
const pairsFor = (pairs = DEFAULT_BRACKET_PAIRS) => {
  for (const p of pairs)
    if (!oneCp(p.open) || !oneCp(p.close) || p.open === p.close)
      throw new InvalidBracketPairError(p);
  return pairs;
};
export const bracketPair = (char: string, pairs?: readonly BracketPair[]) =>
  pairsFor(pairs).find((p) => p.open === char || p.close === char);
export const isOpenBracket = (char: string, pairs?: readonly BracketPair[]) =>
  pairsFor(pairs).some((p) => p.open === char);
export const isCloseBracket = (char: string, pairs?: readonly BracketPair[]) =>
  pairsFor(pairs).some((p) => p.close === char);
export const isBracket = (char: string, pairs?: readonly BracketPair[]) =>
  bracketPair(char, pairs) !== undefined;

function charAt(text: SyntaxText, pos: number): string | undefined {
  return codePointStringAt(text.sliceString(pos, Math.min(text.length, pos + 2)), 0);
}
function charEnd(text: SyntaxText, pos: number): number {
  return pos + (charAt(text, pos)?.length ?? 0);
}
function nextBoundaryText(text: SyntaxText, pos: number): number {
  return Math.min(text.length, charEnd(text, pos));
}
function previousBoundaryText(text: SyntaxText, pos: number): number {
  if (pos <= 0) return 0;
  const prev = text.sliceString(Math.max(0, pos - 2), pos);
  const last = prev.charCodeAt(prev.length - 1);
  const before = prev.length >= 2 ? prev.charCodeAt(prev.length - 2) : undefined;
  return last >= 0xdc00 &&
    last <= 0xdfff &&
    before !== undefined &&
    before >= 0xd800 &&
    before <= 0xdbff
    ? pos - 2
    : pos - 1;
}
function singleChar(text: SyntaxText, node: any): { pos: number; char: string } | undefined {
  const pos = node.startIndex;
  const end = node.endIndex;
  if (end !== charEnd(text, pos)) return undefined;
  const char = charAt(text, pos);
  return char ? { pos, char } : undefined;
}
function sibling(node: any, dir: Direction): any | null {
  return dir === "forward" ? node.nextSibling : node.previousSibling;
}
function findSiblingMatch(
  text: SyntaxText,
  node: any,
  pair: BracketPair,
  dir: Direction,
  limit: number,
): number | undefined {
  let depth = 1,
    seen = 0;
  const start = dir === "forward" ? pair.open : pair.close;
  const needle = dir === "forward" ? pair.close : pair.open;
  for (let n = sibling(node, dir); n && seen++ < limit; n = sibling(n, dir)) {
    const c = singleChar(text, n);
    if (!c) continue;
    if (c.char === start) depth++;
    else if (c.char === needle && --depth === 0) return c.pos;
  }
  return undefined;
}
function matchBracketNode(
  text: SyntaxText,
  node: any,
  pos: number,
  pairs: readonly BracketPair[],
  limit: number,
) {
  const c = singleChar(text, node);
  if (!c || c.pos !== pos) return undefined;
  const p = bracketPair(c.char, pairs);
  if (!p) return undefined;
  return c.char === p.open
    ? findSiblingMatch(text, node, p, "forward", limit)
    : findSiblingMatch(text, node, p, "backward", limit);
}
function matchWrappingNode(
  text: SyntaxText,
  node: any,
  pos: number,
  mode: MatchMode,
  pairs: readonly BracketPair[],
) {
  if (!node.isNamed || node.childCount < 2) return undefined;
  const first = singleChar(text, node.firstChild);
  const last = singleChar(text, node.lastChild);
  if (!first || !last) return undefined;
  const p = pairs.find((x) => x.open === first.char && x.close === last.char);
  if (!p) return undefined;
  if (pos === last.pos) return first.pos;
  if (mode === "fuzzy" || pos === first.pos) return last.pos;
  return undefined;
}
function findMatchInSyntax(
  syntax: Syntax,
  text: SyntaxText,
  pos: number,
  mode: MatchMode,
  options: MatchBracketOptions,
) {
  try {
    // node-tree-sitter callback input and SyntaxNode indices use JS string
    // offsets, which are the engine's public UTF-16 offsets. The deprecated
    // method name is retained by @the-editor/syntax for compatibility.
    const index = pos;
    let node: any = syntax.descendantForByteRange(index, index);
    const pairs = pairsFor(options.pairs);
    const limit = options.syntaxSiblingMatchLimit ?? DEFAULT_SYNTAX_SIBLING_MATCH_LIMIT;
    assertLimit(limit);
    while (node) {
      const w = matchWrappingNode(text, node, pos, mode, pairs);
      if (w !== undefined) return w;
      const b = matchBracketNode(text, node, pos, pairs, limit);
      if (b !== undefined) return b;
      if (mode !== "fuzzy" && node.isNamed) break;
      node = node.parent;
    }
  } catch {
    return undefined;
  }
  return undefined;
}

export function findMatchingBracket(
  syntax: Syntax,
  text: SyntaxText,
  pos: number,
  options: MatchBracketOptions = {},
) {
  assertPos(pos);
  if (pos >= text.length) return undefined;
  const ch = charAt(text, pos);
  if (!ch || !isBracket(ch, options.pairs)) return undefined;
  return (
    findMatchInSyntax(syntax, text, pos, "strict", options) ??
    findMatchInPlaintextWindow(text, pos, "strict", options)
  );
}
export function findMatchingBracketFuzzy(
  syntax: Syntax,
  text: SyntaxText,
  pos: number,
  options: MatchBracketOptions = {},
) {
  assertPos(pos);
  if (pos >= text.length) return undefined;
  return (
    findMatchInSyntax(syntax, text, pos, "fuzzy", options) ??
    findMatchInPlaintextWindow(text, pos, "fuzzy", options)
  );
}
function findMatchInPlaintextWindow(
  text: SyntaxText,
  pos: number,
  mode: MatchMode,
  options: MatchBracketOptions,
) {
  const limit = options.maxPlaintextScan ?? DEFAULT_MAX_PLAINTEXT_SCAN;
  assertLimit(limit);
  const start = Math.max(0, pos - limit),
    end = Math.min(text.length, pos + limit);
  const window = {
    length: end - start,
    sliceString: (from: number, to: number) => text.sliceString(start + from, start + to),
  };
  const local = pos - start;
  const found =
    mode === "strict"
      ? findMatchingBracketPlaintext(window, local, options)
      : findEnclosingBracketPlaintext(window, local, options);
  return found === undefined ? undefined : found + start;
}
export function findMatchingBracketPlaintext(
  text: SyntaxText,
  pos: number,
  options: MatchBracketOptions = {},
) {
  assertPos(pos);
  if (pos >= text.length) return undefined;
  const limit = options.maxPlaintextScan ?? DEFAULT_MAX_PLAINTEXT_SCAN;
  assertLimit(limit);
  const pairs = pairsFor(options.pairs);
  const ch = charAt(text, pos);
  const p = ch && bracketPair(ch, pairs);
  if (!ch || !p) return undefined;
  const forward = ch === p.open;
  const start = forward ? p.open : p.close,
    needle = forward ? p.close : p.open;
  let depth = 1,
    scanned = 0;
  let i = forward ? nextBoundaryText(text, pos) : previousBoundaryText(text, pos);
  while (i >= 0 && i < text.length && scanned++ < limit) {
    const c = charAt(text, i);
    if (c === start) depth++;
    else if (c === needle && --depth === 0) return i;
    if (forward) i = nextBoundaryText(text, i);
    else {
      if (i === 0) break;
      i = previousBoundaryText(text, i);
    }
  }
  return undefined;
}
export function findEnclosingBracketPlaintext(
  text: SyntaxText,
  pos: number,
  options: MatchBracketOptions = {},
) {
  assertPos(pos);
  if (pos >= text.length) return undefined;
  const strict = findMatchingBracketPlaintext(text, pos, options);
  if (strict !== undefined) return strict;
  const limit = options.maxPlaintextScan ?? DEFAULT_MAX_PLAINTEXT_SCAN;
  assertLimit(limit);
  let scanned = 0;
  for (let i = pos; i >= 0 && scanned++ < limit; ) {
    const c = charAt(text, i);
    if (c && isOpenBracket(c, options.pairs)) {
      const close = findMatchingBracketPlaintext(text, i, options);
      if (close !== undefined && close >= pos) return close;
    }
    if (i === 0) break;
    i = previousBoundaryText(text, i);
  }
  return undefined;
}
