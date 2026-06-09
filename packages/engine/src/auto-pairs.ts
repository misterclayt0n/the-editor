import { Effect } from "effect";
import { nextGraphemeBoundary, previousGraphemeBoundary } from "./grapheme";
import { Selection, SelectionRange, type SelectionError } from "./selection";
import { Replacement, Transaction, type Span, type TextError } from "./text/transaction";
import type { TextBuffer } from "./text/buffer";

export const DEFAULT_AUTO_PAIRS = [
  ["(", ")"],
  ["{", "}"],
  ["[", "]"],
  ["'", "'"],
  ['"', '"'],
  ["`", "`"],
] as const;

export type AutoPairInput = readonly [string, string] | { readonly open: string; readonly close: string };
export type AutoPair = { readonly open: string; readonly close: string };
export type AutoPairsEdit = { readonly transaction: Transaction; readonly selection: Selection };

export class AutoPairsError extends Error {}
export class EmptyOpenPairError extends AutoPairsError {
  constructor() {
    super("auto-pair open string must not be empty");
  }
}
export class EmptyClosePairError extends AutoPairsError {
  constructor() {
    super("auto-pair close string must not be empty");
  }
}
export class InvalidTypedCharacterError extends AutoPairsError {
  constructor(readonly value: string) {
    super(`typed character must be exactly one Unicode code point: ${JSON.stringify(value)}`);
  }
}

export class AutoPairs {
  readonly pairs: readonly AutoPair[];
  private constructor(pairs: readonly AutoPair[]) {
    this.pairs = pairs;
  }
  static create(pairs: Iterable<AutoPairInput>): Effect.Effect<AutoPairs, AutoPairsError> {
    const out: AutoPair[] = [];
    for (const input of pairs) {
      const pair: AutoPair = "open" in input ? input : { open: input[0], close: input[1] };
      if (pair.open.length === 0) return Effect.fail(new EmptyOpenPairError());
      if (pair.close.length === 0) return Effect.fail(new EmptyClosePairError());
      out.push(pair);
    }
    return Effect.succeed(new AutoPairs(out));
  }
  static default(): AutoPairs {
    return new AutoPairs(DEFAULT_AUTO_PAIRS.map(([open, close]) => ({ open, close })));
  }
  matchesChar(ch: string): boolean {
    return this.pairs.some((pair) => openLastChar(pair) === ch || closeFirstChar(pair) === ch);
  }
}

type CursorPlan =
  | { readonly type: "replace"; readonly replacement: Replacement; readonly range: SelectionRange; readonly delta: number }
  | { readonly type: "move"; readonly range: SelectionRange };

function pairSame(pair: AutoPair): boolean {
  return pair.open === pair.close;
}
function openLastChar(pair: AutoPair): string {
  return Array.from(pair.open).at(-1)!;
}
function closeFirstChar(pair: AutoPair): string {
  return Array.from(pair.close)[0]!;
}
function isAlphaNumeric(ch: string): boolean {
  return /^[\p{L}\p{N}]$/u.test(ch);
}
function nextCodePointAt(buffer: TextBuffer, offset: number): string | undefined {
  if (offset >= buffer.length) return undefined;
  return Array.from(buffer.sliceString(offset, Math.min(buffer.length, offset + 2)))[0];
}
function previousCodePointBefore(buffer: TextBuffer, offset: number): string | undefined {
  if (offset <= 0) return undefined;
  const start = Math.max(0, offset - 2);
  return Array.from(buffer.sliceString(start, offset)).at(-1);
}
function nextIsNotAlpha(buffer: TextBuffer, cursor: number): boolean {
  const ch = nextCodePointAt(buffer, cursor);
  return ch === undefined || !isAlphaNumeric(ch);
}
function prevIsNotAlpha(buffer: TextBuffer, cursor: number): boolean {
  const ch = previousCodePointBefore(buffer, cursor);
  return ch === undefined || !isAlphaNumeric(ch);
}
function shouldClose(buffer: TextBuffer, range: SelectionRange, pair: AutoPair): boolean {
  const cursor = range.head;
  return nextIsNotAlpha(buffer, cursor) && (!pairSame(pair) || prevIsNotAlpha(buffer, cursor));
}
function matchesOpenPrefix(buffer: TextBuffer, cursor: number, pair: AutoPair): boolean {
  const last = openLastChar(pair);
  const prefix = pair.open.slice(0, pair.open.length - last.length);
  if (prefix.length === 0) return true;
  const start = cursor - prefix.length;
  return start >= 0 && buffer.sliceString(start, cursor) === prefix;
}
function matchesCloseAt(buffer: TextBuffer, cursor: number, pair: AutoPair): boolean {
  return cursor + pair.close.length <= buffer.length && buffer.sliceString(cursor, cursor + pair.close.length) === pair.close;
}
function betterPair(current: { readonly idx: number; readonly pair: AutoPair } | undefined, next: { readonly idx: number; readonly pair: AutoPair }, len: (p: AutoPair) => number) {
  if (!current) return next;
  const a = len(current.pair), b = len(next.pair);
  return b > a || (b === a && next.idx < current.idx) ? next : current;
}
function matchOpenPair(buffer: TextBuffer, cursor: number, ch: string, pairs: AutoPairs): AutoPair | undefined {
  let best: { idx: number; pair: AutoPair } | undefined;
  pairs.pairs.forEach((pair, idx) => {
    if (openLastChar(pair) === ch && matchesOpenPrefix(buffer, cursor, pair)) best = betterPair(best, { idx, pair }, (p) => p.open.length);
  });
  return best?.pair;
}
function matchClosePair(buffer: TextBuffer, cursor: number, ch: string, pairs: AutoPairs): AutoPair | undefined {
  let best: { idx: number; pair: AutoPair } | undefined;
  pairs.pairs.forEach((pair, idx) => {
    if (closeFirstChar(pair) === ch && matchesCloseAt(buffer, cursor, pair)) best = betterPair(best, { idx, pair }, (p) => p.close.length);
  });
  return best?.pair;
}
function deletePairSpan(buffer: TextBuffer, cursor: number, pairs: AutoPairs): Span | undefined {
  let best: { idx: number; span: Span } | undefined;
  pairs.pairs.forEach((pair, idx) => {
    const start = cursor - pair.open.length;
    const end = cursor + pair.close.length;
    if (start < 0 || end > buffer.length) return;
    if (buffer.sliceString(start, cursor) === pair.open && buffer.sliceString(cursor, end) === pair.close) {
      const span = { start, end };
      if (!best || end - start > best.span.end - best.span.start || (end - start === best.span.end - best.span.start && idx < best.idx)) best = { idx, span };
    }
  });
  return best?.span;
}
function isSingleGrapheme(text: string, range: SelectionRange): boolean {
  return !range.isCollapsed && nextGraphemeBoundary(text, range.from) === range.to;
}
function advanceGraphemes(text: string, pos: number, count: number): number {
  let p = pos;
  for (let i = 0; i < count; i++) p = nextGraphemeBoundary(text, p);
  return p;
}
function nextRangeAfterTypedEdit(buffer: TextBuffer, startRange: SelectionRange, offset: number, selectionLen: number, advance: number): SelectionRange {
  const docLen = buffer.length;
  if (startRange.head === docLen && startRange.anchor === docLen) return SelectionRange.create(startRange.anchor + offset + 1, startRange.head + offset + 1);
  const text = buffer.toString();
  const single = isSingleGrapheme(text, startRange);
  if (selectionLen === 0) return SelectionRange.point(advanceGraphemes(text, startRange.head, advance) + offset);
  if (selectionLen === 1) {
    if (startRange.isCollapsed) return SelectionRange.point(startRange.head + offset + 1);
    const anchor = single || startRange.direction === "backward" ? startRange.anchor + offset + 1 : startRange.anchor + offset;
    return SelectionRange.create(anchor, startRange.head + offset + 1);
  }
  const head = startRange.head === 0 || startRange.direction === "backward"
    ? startRange.head + offset + 1
    : previousGraphemeBoundary(text, startRange.head) + offset + selectionLen;
  let anchor: number;
  if (startRange.length === 0) anchor = head;
  else if (startRange.length === 1 && startRange.direction === "forward") anchor = head - 1;
  else if (startRange.length === 1 && startRange.direction === "backward") anchor = head + 1;
  else if (startRange.direction === "forward") anchor = single ? previousGraphemeBoundary(text, startRange.head) + 1 : startRange.anchor + offset;
  else anchor = single ? previousGraphemeBoundary(text, startRange.anchor) + selectionLen + offset : startRange.anchor + offset + selectionLen;
  return SelectionRange.create(anchor, head);
}
function planTypedCharForRange(buffer: TextBuffer, range: SelectionRange, ch: string, pairs: AutoPairs, offset: number): CursorPlan | undefined {
  const cursor = range.head;
  const closePair = matchClosePair(buffer, cursor, ch, pairs);
  if (closePair) return { type: "move", range: nextRangeAfterTypedEdit(buffer, range, offset, 0, Array.from(closePair.close).length) };
  const openPair = matchOpenPair(buffer, cursor, ch, pairs);
  if (!openPair) return undefined;
  const close = shouldClose(buffer, range, openPair);
  const text = ch + (close ? openPair.close : "");
  return {
    type: "replace",
    replacement: Replacement.insert(cursor, text),
    range: nextRangeAfterTypedEdit(buffer, range, offset, text.length, 0),
    delta: text.length,
  };
}
function createSelection(ranges: readonly SelectionRange[], primaryIndex: number): Effect.Effect<Selection, SelectionError> {
  return Effect.try({ try: () => Selection.create(ranges, { primaryIndex }), catch: (e) => e as SelectionError });
}

export function editForTypedChar(buffer: TextBuffer, selection: Selection, ch: string, pairs?: AutoPairs): Effect.Effect<AutoPairsEdit | undefined, AutoPairsError | TextError | SelectionError> {
  return Effect.gen(function* () {
    if (Array.from(ch).length !== 1) return yield* Effect.fail(new InvalidTypedCharacterError(ch));
    const autoPairs = pairs ?? AutoPairs.default();
    if (!autoPairs.matchesChar(ch)) return undefined;
    const plans: CursorPlan[] = [];
    let offset = 0;
    for (const range of selection.ranges) {
      const plan = planTypedCharForRange(buffer, range, ch, autoPairs, offset);
      if (!plan) return undefined;
      if (plan.type === "replace") offset += plan.delta;
      plans.push(plan);
    }
    const replacements = plans.flatMap((plan) => plan.type === "replace" ? [plan.replacement] : []);
    const transaction = replacements.length > 0 ? yield* Transaction.fromReplacements(buffer.length, replacements, { overlap: "reject" }) : Transaction.empty(buffer.length);
    const nextSelection = yield* createSelection(plans.map((p) => p.range), selection.primaryIndex);
    return { transaction, selection: nextSelection };
  });
}

export function editForBackspace(buffer: TextBuffer, selection: Selection, pairs?: AutoPairs): Effect.Effect<AutoPairsEdit | undefined, AutoPairsError | TextError | SelectionError> {
  return Effect.gen(function* () {
    const autoPairs = pairs ?? AutoPairs.default();
    const spans: Span[] = [];
    for (const range of selection.ranges) {
      if (!range.isCollapsed) return undefined;
      const span = deletePairSpan(buffer, range.head, autoPairs);
      if (!span) return undefined;
      spans.push(span);
    }
    if (spans.length === 0) return undefined;
    const transaction = yield* Transaction.deleteSpans(buffer.length, spans, { overlap: "reject" });
    const desc = transaction.desc();
    const nextSelection = yield* createSelection(spans.map((span) => SelectionRange.point(desc.mapPosition(span.start, "before"))), selection.primaryIndex);
    return { transaction, selection: nextSelection };
  });
}
