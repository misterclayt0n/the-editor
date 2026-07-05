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

const OWNER = "the-editor/selection-matches";
const ROLE = "selection-match";
const MAX_SELECTION_LENGTH = 200;
const DEBOUNCE_MS = 150;

export type OffsetRange = { readonly from: number; readonly to: number };
export type SelectionMatchQuery = {
  readonly text: string;
  readonly range: OffsetRange;
};

const emptySet: EditorVisualSet = { decorations: RangeSet.empty() };

const matchSet = (ranges: readonly OffsetRange[]): EditorVisualSet => ({
  decorations: RangeSet.of(
    ranges.map((range) => ({
      ...range,
      value: {
        layer: "background" as const,
        role: ROLE,
        className: "editor-selection-match",
        priority: 20,
      },
    })),
  ),
});

const invalidationFor = (
  before: readonly OffsetRange[],
  after: readonly OffsetRange[],
): readonly OffsetRange[] | undefined => {
  if (before.length === 0 && after.length === 0) return undefined;
  return [...before, ...after];
};

export function selectionMatchQuery(
  text: RenderableText,
  selection: Selection,
  options: { readonly maxLength?: number; readonly multiline?: boolean } = {},
): SelectionMatchQuery | null {
  if (selection.rangeCount !== 1) return null;
  const range = selection.primary;
  if (range.isCollapsed) return null;
  if (range.length > (options.maxLength ?? MAX_SELECTION_LENGTH)) return null;

  const startLine = text.lineRangeAt(range.from).index;
  const endLine = text.lineRangeAt(range.to).index;
  if (startLine !== endLine && options.multiline !== true) return null;

  const query = text.sliceString(range.from, range.to);
  if (query.trim().length === 0) return null;
  return { text: query, range: { from: range.from, to: range.to } };
}

const intersects = (a: OffsetRange, b: OffsetRange): boolean =>
  a.from === b.from || (a.to > b.from && b.to > a.from);

export function findSelectionMatches(
  text: RenderableText,
  query: SelectionMatchQuery,
  searchRange: OffsetRange = { from: 0, to: text.length },
): readonly OffsetRange[] {
  if (query.text.length === 0) return [];
  const from = Math.max(0, Math.min(searchRange.from, text.length));
  const to = Math.max(from, Math.min(searchRange.to, text.length));
  const haystack = text.sliceString(from, to);
  const haystackFolded = haystack.toLowerCase();
  const needleFolded = query.text.toLowerCase();
  const out: OffsetRange[] = [];
  for (let at = haystackFolded.indexOf(needleFolded); at >= 0; ) {
    const match = { from: from + at, to: from + at + query.text.length };
    if (!intersects(match, query.range)) out.push(match);
    at = haystackFolded.indexOf(needleFolded, at + query.text.length);
  }
  return out;
}

const linesToRange = (text: RenderableText, startLine: number, endLine: number): OffsetRange => {
  const start = Math.max(0, Math.min(startLine, text.lineCount - 1));
  const end = Math.max(start + 1, Math.min(endLine, text.lineCount));
  return {
    from: text.lineRange(start).from,
    to: text.lineRange(end - 1).to,
  };
};

const keyOf = (version: number, query: SelectionMatchQuery): string =>
  `${version}:${query.range.from}:${query.range.to}:${query.text}`;

const attachSelectionMatches = (target: DecorationViewTarget) =>
  Effect.gen(function* () {
    const handle = yield* target.viewVisuals.contribute(OWNER, emptySet);
    let currentRanges: readonly OffsetRange[] = [];
    let latestToken = 0;
    let fullCompleteFor: string | null = null;
    let visibleRange: OffsetRange | null = null;
    let debouncedFull: Fiber.RuntimeFiber<void, never> | null = null;

    const publish = (ranges: readonly OffsetRange[]) =>
      handle.update(matchSet(ranges), invalidationFor(currentRanges, ranges)).pipe(
        Effect.tap(() =>
          Effect.sync(() => {
            currentRanges = ranges;
          }),
        ),
      );

    const clear = () =>
      currentRanges.length === 0
        ? Effect.void
        : handle.update(emptySet, invalidationFor(currentRanges, [])).pipe(
            Effect.tap(() =>
              Effect.sync(() => {
                currentRanges = [];
              }),
            ),
          );

    const cancelDebounce = Effect.gen(function* () {
      if (debouncedFull !== null) {
        yield* Fiber.interruptFork(debouncedFull);
        debouncedFull = null;
      }
    });

    const readQuery = Effect.gen(function* () {
      const snapshot = yield* target.text.snapshot;
      const selection = yield* target.selection.get;
      const query = selectionMatchQuery(snapshot.buffer, selection);
      return query === null ? null : { snapshot, query, key: keyOf(snapshot.version, query) };
    });

    const quickUpdate = Effect.gen(function* () {
      const state = yield* readQuery;
      if (state === null) {
        latestToken++;
        fullCompleteFor = null;
        yield* cancelDebounce;
        yield* clear();
        return;
      }
      if (fullCompleteFor === state.key) return;
      yield* publish(
        visibleRange === null
          ? []
          : findSelectionMatches(state.snapshot.buffer, state.query, visibleRange),
      );
    });

    const scheduleFull = Effect.gen(function* () {
      const token = ++latestToken;
      yield* cancelDebounce;
      const state = yield* readQuery;
      if (state === null) {
        fullCompleteFor = null;
        yield* clear();
        return;
      }
      fullCompleteFor = null;
      yield* quickUpdate;
      debouncedFull = Effect.runFork(
        Effect.gen(function* () {
          yield* Effect.sleep(DEBOUNCE_MS);
          const latest = yield* readQuery;
          if (latest === null || latest.key !== state.key || token !== latestToken) return;
          const ranges = findSelectionMatches(latest.snapshot.buffer, latest.query);
          if (token !== latestToken) return;
          fullCompleteFor = latest.key;
          yield* publish(ranges);
        }),
      );
    });

    yield* Effect.acquireRelease(
      Effect.sync(() =>
        target.interaction.onViewportChanged((_top, start, end) => {
          const buffer = Effect.runSync(target.text.get);
          visibleRange = linesToRange(buffer, start, end);
          Effect.runSync(quickUpdate);
        }),
      ),
      (unsubscribe) => Effect.sync(unsubscribe),
    );

    yield* Effect.addFinalizer(() => cancelDebounce);

    yield* scheduleFull;
    yield* Stream.runForEach(
      Stream.merge(target.viewNotifications.notifications, target.notifications.notifications),
      (notification) =>
        hasInvalidation(notification, SelectionInvalidated) ||
        hasInvalidation(notification, DocumentInvalidated)
          ? scheduleFull
          : Effect.void,
    ).pipe(Effect.forkScoped);
  });

export const selectionMatches = definePackage(OWNER, () =>
  Effect.gen(function* () {
    const decorations = yield* DecorationProviders;
    yield* decorations.register({ id: OWNER, attachView: attachSelectionMatches });
  }),
);
