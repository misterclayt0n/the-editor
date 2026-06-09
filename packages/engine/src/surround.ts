import type { Syntax, SyntaxText } from "@the-editor/syntax";
import type { Selection, SelectionRange } from "./selection";
import { codePointStringAt, nextCodePointBoundary } from "./chars";
import {
  DEFAULT_BRACKET_PAIRS,
  bracketPair,
  isOpenBracket,
  isCloseBracket,
  findMatchingBracket,
  findMatchingBracketFuzzy,
  type BracketPair,
} from "./match-brackets";

export type SurroundPair = { readonly open: number; readonly close: number };
export type SurroundOptions = {
  readonly syntax?: Syntax;
  readonly pairs?: readonly BracketPair[];
  readonly maxScan?: number;
  readonly syntaxSiblingMatchLimit?: number;
};

export class SurroundError extends Error {}
export class SurroundPairNotFoundError extends SurroundError {
  constructor() {
    super("surround pair not found");
  }
}

export class SurroundCursorOverlapError extends SurroundError {
  constructor() {
    super("surround cursors overlap");
  }
}

export class SurroundRangeExceedsTextError extends SurroundError {
  constructor(
    readonly to: number,
    readonly length: number,
  ) {
    super(`surround range ${to} exceeds text length ${length}`);
  }
}

export class CursorOnAmbiguousSurroundPairError extends SurroundError {
  constructor(
    readonly char: string,
    readonly pos: number,
  ) {
    super(`cursor on ambiguous surround pair ${char} at ${pos}`);
  }
}

export class InvalidSurroundCountError extends SurroundError {
  constructor(readonly count: number) {
    super(`invalid surround count ${count}`);
  }
}

export class InvalidSurroundPairCharError extends SurroundError {
  constructor(readonly char: string) {
    super(`invalid surround pair char ${char}`);
  }
}

function charAt(text: SyntaxText, offset: number): string | undefined {
  if (offset < 0 || offset >= text.length) return undefined;
  return codePointStringAt(text.sliceString(offset, Math.min(text.length, offset + 2)), 0);
}

function nextBoundaryText(text: SyntaxText, offset: number): number {
  return Math.min(text.length, offset + (charAt(text, offset)?.length ?? 1));
}

function previousBoundaryText(text: SyntaxText, offset: number): number {
  if (offset <= 0) return 0;
  const s = text.sliceString(Math.max(0, offset - 2), offset);
  const last = s.charCodeAt(s.length - 1);
  const before = s.length >= 2 ? s.charCodeAt(s.length - 2) : undefined;
  return last >= 0xdc00 &&
    last <= 0xdfff &&
    before !== undefined &&
    before >= 0xd800 &&
    before <= 0xdbff
    ? offset - 2
    : offset - 1;
}

const oneCp = (s: string) => s.length > 0 && nextCodePointBoundary(s, 0) === s.length;
const validateCount = (n: number) => {
  if (!Number.isInteger(n) || n < 0) throw new InvalidSurroundCountError(n);
};

const validateRange = (text: SyntaxText, range: SelectionRange) => {
  if (range.to > text.length) throw new SurroundRangeExceedsTextError(range.to, text.length);
};

function rangeCursor(text: SyntaxText, range: SelectionRange): number {
  return range.head > range.anchor ? previousBoundaryText(text, range.head) : range.head;
}

function normalizeSurroundPair(a: number, b: number): SurroundPair {
  return a <= b ? { open: a, close: b } : { open: b, close: a };
}

export function surroundPairChars(char: string, pairs?: readonly BracketPair[]): BracketPair {
  if (!oneCp(char)) throw new InvalidSurroundPairCharError(char);
  return bracketPair(char, pairs) ?? { open: char, close: char };
}

function findNthPreviousChar(
  text: SyntaxText,
  ch: string,
  pos: number,
  n: number,
): number | undefined {
  let count = 0;
  for (let i = Math.min(pos, text.length - 1); i >= 0; ) {
    if (charAt(text, i) === ch && ++count === n) return i;
    if (i === 0) break;
    i = previousBoundaryText(text, i);
  }
  return undefined;
}

function findNthNextChar(text: SyntaxText, ch: string, pos: number, n: number): number | undefined {
  let count = 0;
  for (let i = pos; i < text.length; i = nextBoundaryText(text, i))
    if (charAt(text, i) === ch && ++count === n) return i;
  return undefined;
}

function findNthOpenPair(
  text: SyntaxText,
  open: string,
  close: string,
  pos: number,
  n: number,
): number | undefined {
  if (pos >= text.length) return undefined;
  if (charAt(text, pos) === open) return pos;
  let i = previousBoundaryText(text, pos);
  for (let found = 0; found < n; found++) {
    let stepOver = 0;
    for (;;) {
      const ch = charAt(text, i);
      if (ch === undefined) return undefined;
      if (ch === close) stepOver++;
      else if (ch === open) {
        if (stepOver === 0) break;
        stepOver--;
      }
      if (i === 0) return undefined;
      i = previousBoundaryText(text, i);
    }
    if (found + 1 === n) return i;
    if (i === 0) return undefined;
    i = previousBoundaryText(text, i);
  }
  return undefined;
}

function findNthClosePair(
  text: SyntaxText,
  open: string,
  close: string,
  pos: number,
  n: number,
): number | undefined {
  if (pos >= text.length) return undefined;
  if (charAt(text, pos) === close) return pos;
  let i = nextBoundaryText(text, pos);
  for (let found = 0; found < n; found++) {
    let stepOver = 0;
    for (;;) {
      const ch = charAt(text, i);
      if (ch === undefined) return undefined;
      if (ch === open) stepOver++;
      else if (ch === close) {
        if (stepOver === 0) break;
        stepOver--;
      }
      i = nextBoundaryText(text, i);
    }
    if (found + 1 === n) return i;
    i = nextBoundaryText(text, i);
  }
  return undefined;
}

export function findNthSurroundPair(
  text: SyntaxText,
  char: string,
  range: SelectionRange,
  n: number,
  options: SurroundOptions = {},
): SurroundPair {
  validateRange(text, range);
  validateCount(n);
  if (text.length < 2 || n === 0) throw new SurroundPairNotFoundError();
  const pair = surroundPairChars(char, options.pairs);
  const pos = rangeCursor(text, range);
  let openPos: number | undefined, closePos: number | undefined;
  if (pair.open === pair.close) {
    if (charAt(text, pos) === pair.open)
      throw new CursorOnAmbiguousSurroundPairError(pair.open, pos);
    openPos = findNthPreviousChar(text, pair.open, pos, n);
    closePos = findNthNextChar(text, pair.close, pos, n);
  } else {
    openPos = findNthOpenPair(text, pair.open, pair.close, pos, n);
    closePos = findNthClosePair(text, pair.open, pair.close, pos, n);
  }
  if (openPos === undefined || closePos === undefined) throw new SurroundPairNotFoundError();
  return normalizeSurroundPair(openPos, closePos);
}

export function findNthClosestSurroundPair(
  text: SyntaxText,
  range: SelectionRange,
  skip: number,
  options: SurroundOptions = {},
): SurroundPair {
  validateRange(text, range);
  validateCount(skip);
  if (text.length < 2 || skip === 0) throw new SurroundPairNotFoundError();
  return options.syntax
    ? closestWithSyntax(options.syntax, text, range, skip, options)
    : closestPlain(text, range, skip, options);
}

function closestWithSyntax(
  syntax: Syntax,
  text: SyntaxText,
  range: SelectionRange,
  skip: number,
  options: SurroundOptions,
): SurroundPair {
  let opening = range.from,
    closing = range.to;
  const matchOptions = {
    ...(options.pairs !== undefined ? { pairs: options.pairs } : {}),
    ...(options.maxScan !== undefined ? { maxPlaintextScan: options.maxScan } : {}),
    ...(options.syntaxSiblingMatchLimit !== undefined
      ? { syntaxSiblingMatchLimit: options.syntaxSiblingMatchLimit }
      : {}),
  };
  while (skip > 0) {
    const fuzzy = findMatchingBracketFuzzy(syntax, text, closing, matchOptions);
    if (fuzzy === undefined) throw new SurroundPairNotFoundError();
    closing = fuzzy;
    const strict = findMatchingBracket(syntax, text, closing, matchOptions);
    if (strict === undefined) throw new SurroundPairNotFoundError();
    opening = strict;
    const pair = normalizeSurroundPair(opening, closing);
    opening = pair.open;
    closing = pair.close;
    if (range.from < opening || closing < range.to - 1) {
      closing = nextBoundaryText(text, closing);
      continue;
    }
    skip--;
    if (skip !== 0) closing = nextBoundaryText(text, closing);
  }
  return { open: opening, close: closing };
}

function closestPlain(
  text: SyntaxText,
  range: SelectionRange,
  skip: number,
  options: SurroundOptions,
): SurroundPair {
  const pairs = options.pairs ?? DEFAULT_BRACKET_PAIRS;
  const stack: string[] = [];
  const pos = range.from;
  const maxScan = options.maxScan;
  let scanned = 0;
  for (let closePos = pos; closePos < text.length; closePos = nextBoundaryText(text, closePos)) {
    if (maxScan !== undefined && scanned++ >= maxScan) break;
    const ch = charAt(text, closePos);
    if (!ch) continue;
    if (isOpenBracket(ch, pairs)) {
      stack.push(ch);
      continue;
    }
    if (!isCloseBracket(ch, pairs)) continue;
    const pair = surroundPairChars(ch, pairs);
    if (stack[stack.length - 1] === pair.open) {
      stack.pop();
      continue;
    }
    const openPos = findNthOpenPair(text, pair.open, pair.close, closePos, 1);
    if (openPos === undefined) continue;
    if (openPos > pos + 1 || closePos < range.to - 1) continue;
    if (skip > 1) {
      skip--;
      continue;
    }
    return normalizeSurroundPair(openPos, closePos);
  }
  throw new SurroundPairNotFoundError();
}

const overlaps = (a: SurroundPair, b: SurroundPair) => a.open <= b.close && b.open <= a.close;

export function getSurroundPairs(
  text: SyntaxText,
  selection: Selection,
  char: string | undefined,
  skip: number,
  options: SurroundOptions = {},
): readonly SurroundPair[] {
  const out: SurroundPair[] = [];
  for (const range of selection.ranges) {
    const pair =
      char === undefined
        ? findNthClosestSurroundPair(text, range, skip, options)
        : findNthSurroundPair(text, char, range, skip, options);
    if (out.some((p) => overlaps(p, pair))) throw new SurroundCursorOverlapError();
    out.push(pair);
  }
  return out;
}
