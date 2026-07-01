/**
 * DecorationProviders: the extension seam over `EditorVisuals`.
 *
 * `EditorVisuals` is PER-DOCUMENT — created when a buffer opens, living in that
 * buffer's context. A package activates once, before any document exists, so it
 * has nothing to reach. This registry closes that gap: a package registers ONE
 * provider; the host drives it across every document (current AND future),
 * handing each a curated `DecorationTarget` and running the provider's `attach`
 * inside a scope bounded by BOTH the document's lifetime and the provider's
 * registration. Teardown is automatic from either side — close the document or
 * unregister the provider and the contributions retract.
 *
 * This is exactly the attach loop `lineNumbers` used to hand-roll (list current
 * documents + subscribe to `DocumentsChanged`), promoted to a primitive so an
 * external package can decorate buffers without touching document internals or
 * re-implementing the plumbing. The curated `attach` surface (`{ id, text,
 * notifications, visuals }`) is the stable contract — a package never sees the
 * raw document `Context`.
 */

import { Context, Effect, ExecutionStrategy, Exit, Option, Ref, Scope, Stream } from "effect";
import { TextDocument } from "@the-editor/engine";
import { EditorVisuals } from "./visuals";
import { DocumentNotifications, payloadsOf, type NotificationBus } from "./notifications";
import { DocumentRegistry, DocumentsChanged, type DocumentScope } from "./workspace";
import type { DocumentId } from "./services";

/**
 * What a decoration provider gets for one document: its identity, a live text
 * handle (read content, react to changes), the document's notification bus, and
 * the `EditorVisuals` handle to contribute overlays into. This named surface —
 * not the document's internal `Context` — is the extension contract.
 */
export interface DecorationTarget {
  readonly id: DocumentId;
  readonly text: TextDocument.Service;
  readonly notifications: NotificationBus;
  readonly visuals: EditorVisuals.Service;
}

export interface DecorationProvider {
  readonly id: string;
  /** Only attach to documents this accepts. Omit = every document. */
  readonly appliesTo?: (target: DecorationTarget) => boolean;
  /**
   * Runs once per matching document, in a scope tied to that document's life
   * AND this provider's registration. Contribute via `target.visuals` and fork
   * with `Effect.forkScoped`; both retract automatically — no manual scoping.
   */
  readonly attach: (target: DecorationTarget) => Effect.Effect<void, never, Scope.Scope>;
}

export namespace DecorationProviders {
  export interface Service {
    /**
     * Register a provider. Attaches to all open documents now and to every
     * future one until the surrounding scope (package activation) closes.
     */
    readonly register: (provider: DecorationProvider) => Effect.Effect<void, never, Scope.Scope>;
    /** Currently registered providers (introspection/testing). */
    readonly list: Effect.Effect<ReadonlyArray<DecorationProvider>>;
  }
}

export class DecorationProviders extends Context.Tag("@the-editor/editor/DecorationProviders")<
  DecorationProviders,
  DecorationProviders.Service
>() {}

/** Pull the curated target out of a document's context (the one place that
 *  touches the raw `Context`, so providers never have to). */
const targetOf = (document: DocumentScope): DecorationTarget => {
  const ctx = document.context as Context.Context<
    TextDocument | DocumentNotifications | EditorVisuals
  >;
  return {
    id: document.id,
    text: Context.get(ctx, TextDocument),
    notifications: Context.get(ctx, DocumentNotifications),
    visuals: Context.get(ctx, EditorVisuals),
  };
};

export const makeDecorationProviders = (
  registry: DocumentRegistry.Service,
  workspaceBus: NotificationBus,
): Effect.Effect<DecorationProviders.Service> =>
  Effect.gen(function* () {
    const providers = yield* Ref.make<ReadonlyArray<DecorationProvider>>([]);

    const register = (provider: DecorationProvider): Effect.Effect<void, never, Scope.Scope> =>
      Effect.gen(function* () {
        // The provider's registration scope = the package activation scope.
        // When the package deactivates this closes, killing every attachment
        // and the subscription fiber below.
        const providerScope = yield* Effect.scope;

        // Introspection only; retracts with the provider.
        yield* Effect.acquireRelease(
          Ref.update(providers, (list) => [...list, provider]),
          () => Ref.update(providers, (list) => list.filter((entry) => entry !== provider)),
        );

        const attachTo = (document: DocumentScope): Effect.Effect<void> =>
          Effect.gen(function* () {
            const target = targetOf(document);
            if (provider.appliesTo !== undefined && !provider.appliesTo(target)) return;

            // Attachment lifetime = (provider registered) ∩ (document open):
            // a child of the provider scope dies on unregister; a finalizer on
            // the document scope dies on close. Whichever fires first wins;
            // closing an already-closed scope is a no-op.
            const attachment = yield* Scope.fork(providerScope, ExecutionStrategy.sequential);
            yield* Scope.addFinalizer(document.scope, Scope.close(attachment, Exit.void));
            yield* Scope.extend(provider.attach(target), attachment);
          });

        // Attach to every document already open …
        yield* Effect.forEach(yield* registry.list, attachTo, { discard: true });

        // … and to every future one, until this provider is unregistered.
        yield* Stream.runForEach(workspaceBus.notifications, (notification) =>
          Effect.forEach(
            payloadsOf(notification, DocumentsChanged),
            (change) =>
              change.opened === undefined
                ? Effect.void
                : registry
                    .get(change.opened)
                    .pipe(Effect.flatMap(Option.match({ onNone: () => Effect.void, onSome: attachTo }))),
            { discard: true },
          ),
        ).pipe(Effect.forkScoped, Effect.asVoid);
      });

    return { register, list: Ref.get(providers) };
  });
