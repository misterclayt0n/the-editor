import { Effect, Fiber, Stream } from "effect";
import { RangeSet, type RenderableText, type Selection } from "@the-editor/engine";
import { definePackage } from "@the-editor/extensions";
import { Lsp, type DocumentHighlight } from "@the-editor/intel";
import {
  DecorationProviders,
  DocumentInvalidated,
  SelectionInvalidated,
  hasInvalidation,
  type DecorationViewTarget,
  type EditorVisualSet,
} from "@the-editor/editor";

const OWNER = "the-editor/document-highlights";
const DEBOUNCE_MS = 75;

export type OffsetRange = { readonly from: number; readonly to: number };
export type DocumentHighlightQuery = {
  readonly offset: number;
  readonly word: OffsetRange;
};

const emptySet: EditorVisualSet = { decorations: RangeSet.empty() };

const roleOf = (kind: DocumentHighlight["kind"]): string =>
  kind === "write" ? "document-highlight-write" : "document-highlight-read";

const classOf = (kind: DocumentHighlight["kind"]): string =>
  kind === "write" ? "editor-document-highlight-write" : "editor-document-highlight-read";

const highlightSet = (highlights: readonly DocumentHighlight[]): EditorVisualSet => ({
  decorations: RangeSet.of(
    highlights.map((highlight) => ({
      from: highlight.from,
      to: highlight.to,
      value: {
        layer: "background" as const,
        role: roleOf(highlight.kind),
        className: classOf(highlight.kind),
        priority: 18,
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

const WORD = /[\p{L}\p{N}_$]/u;
const isWordChar = (char: string): boolean => WORD.test(char);

export function surroundingWord(text: RenderableText, offset: number): OffsetRange | null {
  if (text.length === 0) return null;
  let at = Math.max(0, Math.min(offset, text.length));
  if (at === text.length || !isWordChar(text.sliceString(at, at + 1))) {
    if (at === 0 || !isWordChar(text.sliceString(at - 1, at))) return null;
    at--;
  }
  let from = at;
  let to = at + 1;
  while (from > 0 && isWordChar(text.sliceString(from - 1, from))) from--;
  while (to < text.length && isWordChar(text.sliceString(to, to + 1))) to++;
  return from < to ? { from, to } : null;
}

/** Zed-style guard: head and tail must be inside the same surrounding word. */
export function documentHighlightQuery(
  text: RenderableText,
  selection: Selection,
): DocumentHighlightQuery | null {
  if (selection.rangeCount !== 1) return null;
  const range = selection.primary;
  const headWord = surroundingWord(text, range.head);
  const tailWord = surroundingWord(text, range.anchor);
  if (headWord === null || tailWord === null) return null;
  if (headWord.from !== tailWord.from || headWord.to !== tailWord.to) return null;
  return { offset: range.head, word: headWord };
}

const keyOf = (version: number, query: DocumentHighlightQuery): string =>
  `${version}:${query.offset}:${query.word.from}:${query.word.to}`;

const rangesOf = (highlights: readonly DocumentHighlight[]): readonly OffsetRange[] =>
  highlights.map((highlight) => ({ from: highlight.from, to: highlight.to }));

const attachDocumentHighlights = (lsp: Lsp.Service, target: DecorationViewTarget) =>
  Effect.gen(function* () {
    const handle = yield* target.viewVisuals.contribute(OWNER, emptySet);
    let currentRanges: readonly OffsetRange[] = [];
    let latestToken = 0;
    let pending: Fiber.RuntimeFiber<void, never> | null = null;

    const publish = (highlights: readonly DocumentHighlight[]) => {
      const nextRanges = rangesOf(highlights);
      return handle.update(highlightSet(highlights), invalidationFor(currentRanges, nextRanges)).pipe(
        Effect.tap(() =>
          Effect.sync(() => {
            currentRanges = nextRanges;
          }),
        ),
      );
    };

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

    const cancelPending = Effect.gen(function* () {
      if (pending !== null) {
        yield* Fiber.interruptFork(pending);
        pending = null;
      }
    });

    const readQuery = Effect.gen(function* () {
      const snapshot = yield* target.text.snapshot;
      const selection = yield* target.selection.get;
      const query = documentHighlightQuery(snapshot.buffer, selection);
      return query === null ? null : { snapshot, query, key: keyOf(snapshot.version, query) };
    });

    const schedule = Effect.gen(function* () {
      const token = ++latestToken;
      yield* cancelPending;
      const state = yield* readQuery;
      if (state === null) {
        yield* clear();
        return;
      }
      pending = Effect.runFork(
        Effect.gen(function* () {
          yield* Effect.sleep(DEBOUNCE_MS);
          const latest = yield* readQuery;
          if (latest === null || latest.key !== state.key || token !== latestToken) return;
          const highlights = yield* Effect.tryPromise(() =>
            lsp.documentHighlights({
              docId: String(target.id),
              path: null,
              text: latest.snapshot.buffer,
              offset: latest.query.offset,
            }),
          ).pipe(Effect.catchAll(() => Effect.succeed([])));
          const after = yield* readQuery;
          if (after === null || after.key !== latest.key || token !== latestToken) return;
          yield* publish(highlights);
        }),
      );
    });

    yield* Effect.addFinalizer(() => cancelPending);
    yield* schedule;
    yield* Stream.runForEach(
      Stream.merge(target.viewNotifications.notifications, target.notifications.notifications),
      (notification) =>
        hasInvalidation(notification, SelectionInvalidated) ||
        hasInvalidation(notification, DocumentInvalidated)
          ? schedule
          : Effect.void,
    ).pipe(Effect.forkScoped);
  });

export const documentHighlights = definePackage(OWNER, () =>
  Effect.gen(function* () {
    const decorations = yield* DecorationProviders;
    const lsp = yield* Lsp;
    yield* decorations.register({
      id: OWNER,
      attachView: (target) => attachDocumentHighlights(lsp, target),
    });
  }),
);
