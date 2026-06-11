/**
 * The line-number gutter, shipped as an ordinary product package.
 *
 * This is the dogfood proof of the visuals primitives: the package
 * contributes a gutter BAND (a width reservation with a per-line cell)
 * through the same public EditorVisuals API any extension would use. The
 * engine and renderer never learn what a "line number" is — disable this
 * package and contribute your own band to replace the gutter entirely.
 *
 * It also exercises the lifecycle: the band's width follows the document's
 * digit count, updated through the contribution handle when the line count
 * crosses a power of ten.
 */

import { Effect, Stream } from "effect";
import { definePackage } from "@the-editor/extensions";
import {
  DocumentInvalidated,
  EditorNotifications,
  EditorVisuals,
  hasInvalidation,
  type EditorVisualSet,
} from "@the-editor/editor";
import { TextDocument } from "@the-editor/engine";

const digits = (n: number): number => String(Math.max(1, n)).length;

const band = (lineCount: number): EditorVisualSet => ({
  gutterBands: [
    {
      id: "line-numbers",
      side: "left",
      widthCh: digits(lineCount) + 1,
      priority: 0,
      cell: (line) => ({ text: String(line + 1), className: "gutter-line-number" }),
    },
  ],
});

export const lineNumbers = definePackage("the-editor/line-numbers", () =>
  Effect.gen(function* () {
    const visuals = yield* EditorVisuals;
    const notifications = yield* EditorNotifications;
    const document = yield* TextDocument;

    const initial = yield* document.snapshot;
    let width = digits(initial.buffer.lineCount);
    const handle = yield* visuals.contribute(
      "the-editor/line-numbers",
      band(initial.buffer.lineCount),
    );

    // Track digit-count changes; the handle update flows through the same
    // invalidation pipeline as any extension's.
    yield* Stream.runForEach(notifications.notifications, (notification) =>
      Effect.gen(function* () {
        if (!hasInvalidation(notification, DocumentInvalidated)) return;
        const snapshot = yield* document.snapshot;
        const next = digits(snapshot.buffer.lineCount);
        if (next !== width) {
          width = next;
          yield* handle.update(band(snapshot.buffer.lineCount));
        }
      }),
    ).pipe(Effect.forkScoped);
  }),
);
