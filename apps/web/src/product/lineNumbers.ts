/**
 * The line-number gutter, shipped as an ordinary product package.
 *
 * This is the dogfood proof of the visuals primitives: the package
 * contributes a gutter BAND (a width reservation with a per-line cell)
 * through the same public EditorVisuals API any extension would use. The
 * engine and renderer never learn what a "line number" is — disable this
 * package and contribute your own band to replace the gutter entirely.
 *
 * It is document-lifecycle aware: it attaches to every document in the
 * workspace (current and future, via DocumentsChanged) and each band's
 * width follows its document's digit count. Contributions are tied to the
 * DOCUMENT scope, so they retract naturally when the document closes —
 * before its bus shuts down (LIFO finalizers).
 */

import { Context, Effect, Option, Scope, Stream } from "effect";
import { definePackage } from "@the-editor/extensions";
import {
  DocumentInvalidated,
  DocumentNotifications,
  DocumentRegistry,
  DocumentsChanged,
  EditorVisuals,
  hasInvalidation,
  payloadsOf,
  WorkspaceNotifications,
  type DocumentScope,
  type EditorVisualSet,
} from "@the-editor/editor";
import { TextDocument } from "@the-editor/engine";
import { digits, lineNumberBand } from "../editor/gutter";

const band = (lineCount: number): EditorVisualSet => ({
  gutterBands: [lineNumberBand(lineCount, (line) => line + 1)],
});

const attach = (document: DocumentScope) =>
  Effect.gen(function* () {
    const visuals = Context.get(
      document.context as Context.Context<EditorVisuals>,
      EditorVisuals,
    );
    const text = Context.get(document.context as Context.Context<TextDocument>, TextDocument);
    const bus = Context.get(
      document.context as Context.Context<DocumentNotifications>,
      DocumentNotifications,
    );

    const initial = yield* text.snapshot;
    let width = digits(initial.buffer.lineCount);
    const handle = yield* visuals
      .contribute("the-editor/line-numbers", band(initial.buffer.lineCount))
      .pipe(Scope.extend(document.scope));

    // Width follows the document's digit count; the fiber ends with the
    // document's bus.
    yield* Effect.forkDaemon(
      Stream.runForEach(bus.notifications, (notification) =>
        Effect.gen(function* () {
          if (!hasInvalidation(notification, DocumentInvalidated)) return;
          const snapshot = yield* text.snapshot;
          const next = digits(snapshot.buffer.lineCount);
          if (next !== width) {
            width = next;
            yield* handle.update(band(snapshot.buffer.lineCount));
          }
        }),
      ),
    );
  });

export const lineNumbers = definePackage("the-editor/line-numbers", () =>
  Effect.gen(function* () {
    const registry = yield* DocumentRegistry;
    const workspaceBus = yield* WorkspaceNotifications;

    for (const document of yield* registry.list) {
      yield* attach(document);
    }
    yield* Stream.runForEach(workspaceBus.notifications, (notification) =>
      Effect.gen(function* () {
        for (const payload of payloadsOf(notification, DocumentsChanged)) {
          if (payload.opened === undefined) continue;
          const document = yield* registry.get(payload.opened);
          if (Option.isSome(document)) yield* attach(document.value);
        }
      }),
    ).pipe(Effect.forkScoped);
  }),
);
