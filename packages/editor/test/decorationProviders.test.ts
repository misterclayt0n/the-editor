import { Context, Effect, Exit, Ref, Scope } from "effect";
import { describe, expect, it } from "vitest";
import {
  DocumentRegistry,
  WorkspaceNotifications,
  makeDecorationProviders,
  makeViewScope,
  makeWorkspaceScope,
  closeViewScope,
  ViewFloatHub,
  ViewMountHub,
  ViewInteraction,
  type DecorationTarget,
  type DecorationViewTarget,
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

const countingViewProvider = (live: Ref.Ref<number>, seen?: Ref.Ref<string[]>) => ({
  id: "view-counter",
  attachView: (target: DecorationViewTarget) =>
    Effect.acquireRelease(
      Effect.gen(function* () {
        yield* Ref.update(live, (n) => n + 1);
        if (seen !== undefined) yield* Ref.update(seen, (list) => [...list, target.viewId]);
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

  it("attaches view providers to connected views and retracts on view close", () =>
    Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const ws = yield* makeWorkspaceScope();
          const registry = Context.get(ws.context, DocumentRegistry);
          const bus = Context.get(ws.context, WorkspaceNotifications);
          const providers = yield* makeDecorationProviders(registry, bus);
          const document = yield* registry.open({ text: "a" });
          const view = yield* makeViewScope(document);

          const live = yield* Ref.make(0);
          const seen = yield* Ref.make<string[]>([]);
          yield* providers.register(countingViewProvider(live, seen));
          yield* providers.connectView(view);

          expect(yield* Ref.get(live)).toBe(1);
          expect(yield* Ref.get(seen)).toEqual([view.id]);

          yield* closeViewScope(view);
          expect(yield* Ref.get(live)).toBe(0);
        }),
      ),
    ));

  it("gives view providers the interaction hub to subscribe to (verb 14)", () =>
    Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const ws = yield* makeWorkspaceScope();
          const registry = Context.get(ws.context, DocumentRegistry);
          const bus = Context.get(ws.context, WorkspaceNotifications);
          const providers = yield* makeDecorationProviders(registry, bus);
          const document = yield* registry.open({ text: "a" });
          const view = yield* makeViewScope(document);

          const hovers = yield* Ref.make<Array<number | null>>([]);
          yield* providers.register({
            id: "hover-watcher",
            attachView: (target: DecorationViewTarget) =>
              Effect.acquireRelease(
                Effect.sync(() =>
                  target.interaction.onPointerHover((offset) => {
                    void Effect.runSync(Ref.update(hovers, (list) => [...list, offset]));
                  }),
                ),
                (unsubscribe) => Effect.sync(unsubscribe),
              ).pipe(Effect.asVoid),
          });
          yield* providers.connectView(view);

          // The pane dispatches through the same view-context hub.
          const hub = Context.get(view.context, ViewInteraction);
          hub.dispatch.pointerHover(3);
          hub.dispatch.pointerHover(null);
          expect(yield* Ref.get(hovers)).toEqual([3, null]);

          // Subscription retracts with the view.
          yield* closeViewScope(view);
          hub.dispatch.pointerHover(9);
          expect(yield* Ref.get(hovers)).toEqual([3, null]);
        }),
      ),
    ));

  it("attaches a view provider registered after the view is connected", () =>
    Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const ws = yield* makeWorkspaceScope();
          const registry = Context.get(ws.context, DocumentRegistry);
          const bus = Context.get(ws.context, WorkspaceNotifications);
          const providers = yield* makeDecorationProviders(registry, bus);
          const document = yield* registry.open({ text: "a" });
          const view = yield* makeViewScope(document);
          yield* providers.connectView(view);

          const live = yield* Ref.make(0);
          yield* providers.register(countingViewProvider(live));

          expect(yield* Ref.get(live)).toBe(1);
        }),
      ),
    ));

  it("retracts view providers when the document closes", () =>
    Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const ws = yield* makeWorkspaceScope();
          const registry = Context.get(ws.context, DocumentRegistry);
          const bus = Context.get(ws.context, WorkspaceNotifications);
          const providers = yield* makeDecorationProviders(registry, bus);
          const document = yield* registry.open({ text: "a" });
          const view = yield* makeViewScope(document);
          yield* providers.connectView(view);

          const live = yield* Ref.make(0);
          yield* providers.register(countingViewProvider(live));
          expect(yield* Ref.get(live)).toBe(1);

          yield* registry.close(document.id);
          expect(yield* Ref.get(live)).toBe(0);
        }),
      ),
    ));

  it("exposes live mount and float hubs through the view context and target", () =>
    Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const ws = yield* makeWorkspaceScope();
          const registry = Context.get(ws.context, DocumentRegistry);
          const bus = Context.get(ws.context, WorkspaceNotifications);
          const providers = yield* makeDecorationProviders(registry, bus);
          const document = yield* registry.open({ text: "a" });
          const view = yield* makeViewScope(document);
          const viewMounts = Context.get(view.context, ViewMountHub);
          const viewFloats = Context.get(view.context, ViewFloatHub);
          const mounted: string[] = [];
          let targetFloats: unknown;

          yield* providers.register({
            id: "mount-consumer",
            attachView: (target) =>
              Effect.acquireRelease(
                Effect.sync(() => {
                  expect(target.mounts).toBe(viewMounts);
                  expect(target.floats).toBe(viewFloats);
                  targetFloats = target.floats;
                  return target.mounts.subscribe((event) => {
                    if (event.type === "mounted") mounted.push(event.id);
                  });
                }),
                (unsubscribe) => Effect.sync(unsubscribe),
              ).pipe(Effect.asVoid),
          });
          yield* providers.connectView(view);

          viewMounts.mounted("widget:1", {} as HTMLElement);
          expect(mounted).toEqual(["widget:1"]);
          expect(targetFloats).toBe(viewFloats);

          yield* closeViewScope(view);
          viewMounts.mounted("widget:2", {} as HTMLElement);
          expect(mounted).toEqual(["widget:1"]);
        }),
      ),
    ));
});
