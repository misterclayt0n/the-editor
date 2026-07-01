import { Context, Effect, Exit, Ref, Scope } from "effect";
import { describe, expect, it } from "vitest";
import {
  DocumentRegistry,
  WorkspaceNotifications,
  makeDecorationProviders,
  makeWorkspaceScope,
  type DecorationTarget,
} from "../src";

/** A provider whose attach bumps a live counter on acquire and drops it on
 *  release — so the counter is exactly the number of live attachments. */
const countingProvider = (live: Ref.Ref<number>, seen?: Ref.Ref<string[]>) => ({
  id: "counter",
  attach: (target: DecorationTarget) =>
    Effect.acquireRelease(
      Effect.gen(function* () {
        yield* Ref.update(live, (n) => n + 1);
        if (seen !== undefined) yield* Ref.update(seen, (list) => [...list, target.id]);
      }),
      () => Ref.update(live, (n) => n - 1),
    ).pipe(Effect.asVoid),
});

describe("DecorationProviders", () => {
  it("attaches to already-open documents and retracts on document close", () =>
    Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const ws = yield* makeWorkspaceScope();
          const registry = Context.get(ws.context, DocumentRegistry);
          const bus = Context.get(ws.context, WorkspaceNotifications);
          const providers = yield* makeDecorationProviders(registry, bus);

          const d1 = yield* registry.open({ text: "a" });
          const d2 = yield* registry.open({ text: "b" });

          const live = yield* Ref.make(0);
          yield* providers.register(countingProvider(live));
          // Two documents already open → two live attachments, synchronously.
          expect(yield* Ref.get(live)).toBe(2);

          // Closing one document retracts exactly its attachment.
          yield* registry.close(d1.id);
          expect(yield* Ref.get(live)).toBe(1);

          yield* registry.close(d2.id);
          expect(yield* Ref.get(live)).toBe(0);
        }),
      ),
    ));

  it("retracts every attachment when the provider is unregistered", () =>
    Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const ws = yield* makeWorkspaceScope();
          const registry = Context.get(ws.context, DocumentRegistry);
          const bus = Context.get(ws.context, WorkspaceNotifications);
          const providers = yield* makeDecorationProviders(registry, bus);

          yield* registry.open({ text: "a" });
          yield* registry.open({ text: "b" });

          const live = yield* Ref.make(0);
          // Register inside a dedicated scope so we can unregister on demand.
          const registration = yield* Scope.make();
          yield* Scope.extend(providers.register(countingProvider(live)), registration);
          expect(yield* Ref.get(live)).toBe(2);

          // Unregister → every attachment tears down; the documents live on.
          yield* Scope.close(registration, Exit.void);
          expect(yield* Ref.get(live)).toBe(0);
          expect(yield* providers.list).toHaveLength(0);
        }),
      ),
    ));

  it("attaches to documents opened after registration", () =>
    Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const ws = yield* makeWorkspaceScope();
          const registry = Context.get(ws.context, DocumentRegistry);
          const bus = Context.get(ws.context, WorkspaceNotifications);
          const providers = yield* makeDecorationProviders(registry, bus);

          const live = yield* Ref.make(0);
          const seen = yield* Ref.make<string[]>([]);
          yield* providers.register(countingProvider(live, seen));
          // Let the DocumentsChanged subscription come online (async PubSub).
          yield* Effect.sleep("20 millis");

          const future = yield* registry.open({ text: "later" });
          yield* Effect.sleep("20 millis");

          expect(yield* Ref.get(live)).toBe(1);
          expect(yield* Ref.get(seen)).toEqual([future.id]);
        }),
      ),
    ));
});
