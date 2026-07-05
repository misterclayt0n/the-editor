import { Effect, Fiber, Stream } from "effect";
import { RangeSet, type RenderableText, type Selection } from "@the-editor/engine";
import { definePackage } from "@the-editor/extensions";
import {
  DecorationProviders,
  DocumentInvalidated,
  SelectionInvalidated,
  hasInvalidation,
  type DecorationViewTarget,
  type EditorVisualSet,
} from "@the-editor/editor";

const OWNER = "the-editor/matching-brackets";
const DEBOUNCE_MS = 25;
const MAX_SCAN_CHARS = 200_000;

export type OffsetRange = { readonly from: number; readonly to: number };
export type MatchingBracketRanges = {
  readonly open: OffsetRange;
  readonly close: OffsetRange;
};

const emptySet: EditorVisualSet = { decorations: RangeSet.empty() };

const OPEN_TO_CLOSE = new Map([
  ["(", ")"],
  ["[", "]"],
  ["{", "}"],
]);
const CLOSES = new Set(OPEN_TO_CLOSE.values());

type OpenBracket = {
  readonly char: string;
  readonly close: string;
  readonly from: number;
};

const charLengthAt = (text: RenderableText, offset: number): number => {
  const sample = text.sliceString(offset, Math.min(text.length, offset + 2));
  const char = Array.from(sample)[0];
  return char === undefined ? 0 : char.length;
};

const scanWindow = (text: RenderableText, around: number): { readonly from: number; readonly to: number } => {
  if (text.length <= MAX_SCAN_CHARS) return { from: 0, to: text.length };
  const half = Math.floor(MAX_SCAN_CHARS / 2);
  const from = Math.max(0, around - half);
  return { from, to: Math.min(text.length, from + MAX_SCAN_CHARS) };
};

const contains = (pair: MatchingBracketRanges, from: number, to: number): boolean =>
  pair.open.from <= from && pair.close.to >= to;

const isSmaller = (
  candidate: MatchingBracketRanges,
  current: MatchingBracketRanges | null,
): boolean => {
  if (current === null) return true;
  const candidateLength = candidate.close.to - candidate.open.from;
  const currentLength = current.close.to - current.open.from;
  return candidateLength < currentLength;
};

/**
 * Zed-style query: find the innermost bracket pair enclosing the cursor range.
 *
 * Zed gets these bracket ranges from tree-sitter bracket queries. Until this
 * product has native tree-sitter bracket ranges, this is the same editor-facing
 * behavior over a conservative text fallback: structural (), [], and {} pairs.
 */
export function innermostEnclosingBracketRanges(
  text: RenderableText,
  from: number,
  to: number,
): MatchingBracketRanges | null {
  if (from < 0 || to < from || to > text.length) return null;

  const window = scanWindow(text, from);
  if (from < window.from || to > window.to) return null;

  const source = text.sliceString(window.from, window.to);
  const stack: OpenBracket[] = [];
  let best: MatchingBracketRanges | null = null;

  for (let i = 0; i < source.length; i++) {
    const char = source[i]!;
    const absolute = window.from + i;
    const close = OPEN_TO_CLOSE.get(char);
    if (close !== undefined) {
      stack.push({ char, close, from: absolute });
      continue;
    }
    if (!CLOSES.has(char)) continue;

    const top = stack[stack.length - 1];
    if (top?.close !== char) continue;
    stack.pop();

    const pair: MatchingBracketRanges = {
      open: { from: top.from, to: top.from + top.char.length },
      close: { from: absolute, to: absolute + char.length },
    };
    if (contains(pair, from, to) && isSmaller(pair, best)) best = pair;
  }

  return best;
}

export function matchingBracketRangesForSelection(
  text: RenderableText,
  selection: Selection,
  cursorShape: "bar" | "block" | "underline" | "hidden" = "bar",
): MatchingBracketRanges | null {
  const range = selection.primary;
  if (!range.isCollapsed) return null;

  const head = Math.min(range.head, text.length);
  let tail = head;
  if (cursorShape === "block" && head < text.length) tail += charLengthAt(text, head);
  return innermostEnclosingBracketRanges(text, head, tail);
}

export const decoratedRangesForMatch = (
  match: MatchingBracketRanges | null,
): readonly OffsetRange[] => {
  if (match === null) return [];
  return match.open.to === match.close.from
    ? [{ from: match.open.from, to: match.close.to }]
    : [match.open, match.close];
};

const visualSetFor = (match: MatchingBracketRanges | null): EditorVisualSet => ({
  decorations: RangeSet.of(
    decoratedRangesForMatch(match).map((range) => ({
      ...range,
      value: {
        layer: "outline" as const,
        role: "matching-bracket",
        className: "editor-matching-bracket",
        priority: 30,
      },
    })),
  ),
});

const rangesOf = (match: MatchingBracketRanges | null): readonly OffsetRange[] =>
  match === null ? [] : [match.open, match.close];

const invalidationFor = (
  before: readonly OffsetRange[],
  after: readonly OffsetRange[],
): readonly OffsetRange[] | undefined => {
  if (before.length === 0 && after.length === 0) return undefined;
  return [...before, ...after];
};

const attachMatchingBrackets = (target: DecorationViewTarget) =>
  Effect.gen(function* () {
    const handle = yield* target.viewVisuals.contribute(OWNER, emptySet);
    let current: readonly OffsetRange[] = [];
    let latestToken = 0;
    let pending: Fiber.RuntimeFiber<void, never> | null = null;

    const readMatch = Effect.gen(function* () {
      const snapshot = yield* target.text.snapshot;
      const selection = yield* target.selection.get;
      const shape = target.cursor.getStyle().shape;
      const match = matchingBracketRangesForSelection(snapshot.buffer, selection, shape);
      return { version: snapshot.version, selection, shape, match };
    });

    const publish = (match: MatchingBracketRanges | null) => {
      const next = rangesOf(match);
      return handle.update(visualSetFor(match), invalidationFor(current, next)).pipe(
        Effect.tap(() =>
          Effect.sync(() => {
            current = next;
          }),
        ),
      );
    };

    const cancelPending = Effect.gen(function* () {
      if (pending !== null) {
        yield* Fiber.interruptFork(pending);
        pending = null;
      }
    });

    const schedule = Effect.gen(function* () {
      const token = ++latestToken;
      yield* cancelPending;
      pending = Effect.runFork(
        Effect.gen(function* () {
          yield* Effect.sleep(DEBOUNCE_MS);
          const state = yield* readMatch;
          if (token !== latestToken) return;
          yield* publish(state.match);
        }),
      );
    });

    yield* Effect.addFinalizer(() => cancelPending);
    yield* schedule;
    const unsubscribeCursor = target.cursor.subscribe(() => Effect.runFork(schedule));
    yield* Effect.addFinalizer(() => Effect.sync(unsubscribeCursor));
    yield* Stream.runForEach(
      Stream.merge(target.viewNotifications.notifications, target.notifications.notifications),
      (notification) =>
        hasInvalidation(notification, SelectionInvalidated) ||
        hasInvalidation(notification, DocumentInvalidated)
          ? schedule
          : Effect.void,
    ).pipe(Effect.forkScoped);
  });

export const matchingBrackets = definePackage(OWNER, () =>
  Effect.gen(function* () {
    const decorations = yield* DecorationProviders;
    yield* decorations.register({ id: OWNER, attachView: attachMatchingBrackets });
  }),
);
