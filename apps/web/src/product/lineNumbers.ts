/**
 * The line-number gutter, shipped as an ordinary product package.
 *
 * The dogfood proof of the decoration seam: this package registers a single
 * `DecorationProvider`, and the host drives it across every document. It never
 * lists documents, never subscribes to `DocumentsChanged`, never touches a
 * document's `Context` or scope — it just contributes a gutter BAND through the
 * curated `attach` target, exactly as an external extension would. Disable the
 * package and contribute your own band to replace the gutter entirely.
 */

import { Effect, Stream } from "effect";
import { definePackage } from "@the-editor/extensions";
import {
  DecorationProviders,
  DocumentInvalidated,
  hasInvalidation,
  type DecorationTarget,
  type EditorVisualSet,
} from "@the-editor/editor";
import { digits, lineNumberBand } from "../editor/gutter";

const band = (lineCount: number): EditorVisualSet => ({
  gutterBands: [lineNumberBand(lineCount, (line) => line + 1)],
});

/** Per document: contribute the band, then keep its width in step with the
 *  document's digit count. Everything here retracts when the attachment scope
 *  closes (document close or package deactivation) — no manual teardown. */
const attach = (target: DecorationTarget) =>
  Effect.gen(function* () {
    const initial = yield* target.text.snapshot;
    let width = digits(initial.buffer.lineCount);
    const handle = yield* target.visuals.contribute(
      "the-editor/line-numbers",
      band(initial.buffer.lineCount),
    );
    yield* Stream.runForEach(target.notifications.notifications, (notification) =>
      Effect.gen(function* () {
        if (!hasInvalidation(notification, DocumentInvalidated)) return;
        const snapshot = yield* target.text.snapshot;
        const next = digits(snapshot.buffer.lineCount);
        if (next !== width) {
          width = next;
          yield* handle.update(band(snapshot.buffer.lineCount));
        }
      }),
    ).pipe(Effect.forkScoped);
  });

export const lineNumbers = definePackage("the-editor/line-numbers", () =>
  Effect.gen(function* () {
    const decorations = yield* DecorationProviders;
    yield* decorations.register({ id: "the-editor/line-numbers", attach });
  }),
);
