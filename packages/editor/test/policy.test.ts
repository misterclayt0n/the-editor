import { Effect, Context } from "effect";
import { describe, expect, test } from "vitest";
import { SelectionState, TextDocument, Transaction } from "@the-editor/engine";
import {
  commitDocumentTransaction,
  commitTransaction,
  DocumentNotifications,
  DocumentRegistry,
  EditorVisuals,
  makeViewScope,
  makeWorkspaceScope,
  provideViewScope,
  type DocumentScope,
} from "../src/index";

const run = <A, E>(effect: Effect.Effect<A, E>) => Effect.runSync(effect);

const textOf = (document: DocumentScope): string =>
  run(
    Effect.gen(function* () {
      const doc = Context.get(document.context as Context.Context<TextDocument>, TextDocument);
      const text = yield* doc.get;
      return text.sliceString(0, text.length);
    }),
  );

const replaceAll = (document: DocumentScope, text: string) =>
  Effect.gen(function* () {
    const doc = Context.get(document.context as Context.Context<TextDocument>, TextDocument);
    const length = (yield* doc.get).length;
    return yield* Transaction.fromReplacements(length, [{ from: 0, to: length, text }]).pipe(
      Effect.orDie,
    );
  });

describe("document policy (virtual buffers)", () => {
  test("a read-only document rejects a view commit but a forced write lands", () => {
    run(
      Effect.gen(function* () {
        const ws = yield* makeWorkspaceScope();
        const registry = Context.get(
          ws.context as Context.Context<DocumentRegistry>,
          DocumentRegistry,
        );
        const document = yield* registry.open({ text: "readonly", readOnly: true });
        const view = yield* makeViewScope(document);

        // A view-level commit (what typing does) is dropped whole.
        const typed = yield* replaceAll(document, "typed!");
        yield* provideViewScope(view, commitTransaction(typed)).pipe(Effect.orDie);
        expect(textOf(document)).toBe("readonly");

        // The provider's forced write lands.
        const forced = yield* replaceAll(document, "provider-set");
        yield* commitDocumentTransaction(forced, { force: true }).pipe(
          Effect.provide(
            document.context as Context.Context<
              TextDocument | EditorVisuals | DocumentNotifications
            >,
          ),
          Effect.orDie,
        );
        expect(textOf(document)).toBe("provider-set");
      }),
    );
  });

  test("an editable document commits normally (default)", () => {
    run(
      Effect.gen(function* () {
        const ws = yield* makeWorkspaceScope();
        const registry = Context.get(
          ws.context as Context.Context<DocumentRegistry>,
          DocumentRegistry,
        );
        const document = yield* registry.open({ text: "" });
        const view = yield* makeViewScope(document);
        const typed = yield* replaceAll(document, "hello");
        yield* provideViewScope(view, commitTransaction(typed)).pipe(Effect.orDie);
        expect(textOf(document)).toBe("hello");
      }),
    );
  });
});
