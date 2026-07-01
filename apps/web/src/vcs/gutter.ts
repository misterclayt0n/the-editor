/**
 * The VCS diff gutter, as a DecorationProvider.
 *
 * The seam drives the per-document lifecycle: for every document, this attaches
 * a thin colored strip beside the line numbers (added / modified / deleted vs.
 * HEAD). It resolves the document's file path from `Files`, pulls the baseline
 * from the `VcsManager`, and republishes on two signals — the document changing
 * (debounced) and the repo state changing (checkout, commit, save). A document
 * with no path or no repo simply shows nothing. All resources are scoped: they
 * retract when the document closes or the provider is unregistered.
 */

import { Effect, Option, Stream } from "effect";
import {
  DocumentInvalidated,
  FileSyncInvalidated,
  hasInvalidation,
  payloadsOf,
  type DecorationProvider,
  type DecorationTarget,
  type EditorVisualSet,
  type Files,
} from "@the-editor/editor";
import { diffLines, type LineDiff } from "./diff";
import type { VcsManager } from "./manager";

/**
 * Recompute is pure memory (the HEAD baseline is cached); the only reason to
 * wait is coalescing keystroke bursts within a frame.
 */
const GUTTER_DEBOUNCE_MS = 16;

const gutterSet = (diff: LineDiff): EditorVisualSet => ({
  gutterBands: [
    {
      id: "vcs-diff",
      side: "left",
      widthCh: 0,
      // A thin colored strip beside the line numbers (Zed/VSCode), not a
      // padded text column.
      widthPx: 3,
      priority: 1,
      cell: (line) => {
        const mark = diff.lines.get(line);
        if (mark === "added") return { text: "", className: "vcs-gutter-added" };
        if (mark === "modified") return { text: "", className: "vcs-gutter-modified" };
        if (diff.deletedAbove.has(line)) return { text: "", className: "vcs-gutter-deleted" };
        return null;
      },
    },
  ],
});

const EMPTY_DIFF: LineDiff = {
  lines: new Map<number, "added" | "modified">(),
  deletedAbove: new Set<number>(),
};

export const vcsGutterProvider = (
  vcs: VcsManager,
  files: Files.Service | undefined,
): DecorationProvider => ({
  id: "the-editor/vcs-gutter",
  attach: (target: DecorationTarget) =>
    Effect.gen(function* () {
      const handle = yield* target.visuals.contribute("vcs-diff", {});
      let baseline: string | null = null;
      let inRepo = false;

      // Resolve the path fresh each time: a document bound after open (save-as)
      // then picks up its gutter on the next repo-state signal.
      const pathOf = (): string | null => {
        if (files === undefined) return null;
        const binding = Effect.runSync(files.bindingFor(target.id));
        return Option.isNone(binding) ? null : binding.value.path;
      };

      const publish = Effect.gen(function* () {
        const buffer = yield* target.text.get;
        const text = buffer.sliceString(0, buffer.length);
        const diff =
          baseline !== null
            ? diffLines(baseline, text)
            : inRepo
              ? diffLines("", text) // tracked but new: all additions
              : EMPTY_DIFF;
        yield* handle.update(gutterSet(diff));
      });

      const reloadBaseline = Effect.gen(function* () {
        const path = pathOf();
        if (path === null) {
          baseline = null;
          inRepo = false;
        } else {
          const result = yield* Effect.promise(() => vcs.baselineFor(path));
          baseline = result.baseline;
          inRepo = result.inRepo;
        }
        yield* publish;
      });

      yield* reloadBaseline;

      // Republish on document edits, debounced to a frame.
      let timer: number | null = null;
      yield* Effect.addFinalizer(() =>
        Effect.sync(() => {
          if (timer !== null) window.clearTimeout(timer);
        }),
      );
      yield* Stream.runForEach(target.notifications.notifications, (notification) =>
        // The document became file-bound (or its sync state changed): re-resolve
        // the path and baseline. This is what makes the gutter appear on open —
        // `openPath` binds the path a beat after the document opens.
        payloadsOf(notification, FileSyncInvalidated).length > 0
          ? reloadBaseline
          : Effect.sync(() => {
              if (!hasInvalidation(notification, DocumentInvalidated)) return;
              if (timer !== null) window.clearTimeout(timer);
              timer = window.setTimeout(() => {
                timer = null;
                Effect.runFork(publish);
              }, GUTTER_DEBOUNCE_MS);
            }),
      ).pipe(Effect.forkScoped);

      // Re-baseline when the repo moves (checkout, commit, save). `subscribe` is
      // a plain callback; bridge its unsubscribe into the attachment scope.
      const unsubscribe = vcs.subscribe(() => void Effect.runFork(reloadBaseline));
      yield* Effect.addFinalizer(() => Effect.sync(unsubscribe));
    }),
});
