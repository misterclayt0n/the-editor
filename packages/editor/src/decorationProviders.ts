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
 * View-local element capabilities (`MountHub`/`FloatHub`) are PER-VIEW, not
 * per-document: split panes of the same buffer each have their own renderer-owned
 * DOM containers. Providers that need those element seams implement
 * `attachView`; the host connects every editor view to this registry so the
 * provider can subscribe to the pane's mounts/floats in a scope bounded by the
 * document, view, and provider lifetimes.
 *
 * This is exactly the attach loop `lineNumbers` used to hand-roll (list current
 * documents + subscribe to `DocumentsChanged`), promoted to a primitive so an
 * external package can decorate buffers without touching document internals or
 * re-implementing the plumbing. The curated attach surfaces (`DecorationTarget`
 * and `DecorationViewTarget`) are the stable contract — a package never sees the
 * raw document/view `Context`.
 */

import { Context, Effect, ExecutionStrategy, Exit, Option, Ref, Scope, Stream } from "effect";
import { TextDocument } from "@the-editor/engine";
import { EditorVisuals } from "./visuals";
import {
  DocumentNotifications,
  payloadsOf,
  ViewNotifications,
  type NotificationBus,
} from "./notifications";
import { DocumentRegistry, DocumentsChanged, type DocumentScope } from "./workspace";
import type { ViewScope } from "./view";
import type { DocumentId, EditorScopeId } from "./services";
import { ViewMountHub, type MountHub } from "./mounts";
import { ViewFloatHub, type FloatHub } from "./floats";

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

/**
 * The per-view extension of `DecorationTarget`: one live pane/rendered view onto
 * the document, with element-based block-widget mounts and anchored floats.
 */
export interface DecorationViewTarget extends DecorationTarget {
  readonly viewId: EditorScopeId;
  readonly viewNotifications: NotificationBus;
  readonly mounts: MountHub;
  readonly floats: FloatHub;
}

export interface DecorationProvider {
  readonly id: string;
  /** Only attach to documents/views this accepts. Omit = every document/view. */
  readonly appliesTo?: (target: DecorationTarget) => boolean;
  /**
   * Runs once per matching document, in a scope tied to that document's life
   * AND this provider's registration. Contribute via `target.visuals` and fork
   * with `Effect.forkScoped`; both retract automatically — no manual scoping.
   */
  readonly attach?: (target: DecorationTarget) => Effect.Effect<void, never, Scope.Scope>;
  /**
   * Runs once per matching editor view. Subscribe to `target.mounts` for block
   * widget containers and create `target.floats` for popovers. The scope is tied
   * to this document/view AND this provider's registration.
   */
  readonly attachView?: (target: DecorationViewTarget) => Effect.Effect<void, never, Scope.Scope>;
}

export namespace DecorationProviders {
  export interface Service {
    /**
     * Register a provider. Attaches to all open documents/views now and to every
     * future one until the surrounding scope (package activation) closes.
     */
    readonly register: (provider: DecorationProvider) => Effect.Effect<void, never, Scope.Scope>;
    /** Host side: connect one editor view so view-local providers can attach. */
    readonly connectView: (view: ViewScope) => Effect.Effect<void>;
    /** Currently registered providers (introspection/testing). */
    readonly list: Effect.Effect<ReadonlyArray<DecorationProvider>>;
  }
}

export class DecorationProviders extends Context.Tag("@the-editor/editor/DecorationProviders")<
  DecorationProviders,
  DecorationProviders.Service
>() {}

type ProviderRecord = {
  readonly provider: DecorationProvider;
  readonly scope: Scope.Scope;
};

const targetOfContext = (
  id: DocumentId,
  ctx: Context.Context<TextDocument | DocumentNotifications | EditorVisuals>,
): DecorationTarget => ({
  id,
  text: Context.get(ctx, TextDocument),
  notifications: Context.get(ctx, DocumentNotifications),
  visuals: Context.get(ctx, EditorVisuals),
});

/** Pull the curated target out of a document's context (the one place that
 *  touches the raw `Context`, so providers never have to). */
const targetOf = (document: DocumentScope): DecorationTarget =>
  targetOfContext(
    document.id,
    document.context as Context.Context<TextDocument | DocumentNotifications | EditorVisuals>,
  );

const viewTargetOf = (view: ViewScope): DecorationViewTarget => {
  const ctx = view.context as Context.Context<
    | TextDocument
    | DocumentNotifications
    | EditorVisuals
    | ViewNotifications
    | ViewMountHub
    | ViewFloatHub
  >;
  return {
    ...targetOfContext(view.documentId, ctx),
    viewId: view.id,
    viewNotifications: Context.get(ctx, ViewNotifications),
    mounts: Context.get(ctx, ViewMountHub),
    floats: Context.get(ctx, ViewFloatHub),
  };
};

export const makeDecorationProviders = (
  registry: DocumentRegistry.Service,
  workspaceBus: NotificationBus,
): Effect.Effect<DecorationProviders.Service> =>
  Effect.gen(function* () {
    const providers = yield* Ref.make<ReadonlyArray<ProviderRecord>>([]);
    const views = yield* Ref.make<ReadonlyArray<ViewScope>>([]);

    const attachToDocument = (
      record: ProviderRecord,
      document: DocumentScope,
    ): Effect.Effect<void> =>
      Effect.gen(function* () {
        if (record.provider.attach === undefined) return;
        const target = targetOf(document);
        if (record.provider.appliesTo !== undefined && !record.provider.appliesTo(target)) return;

        // Attachment lifetime = (provider registered) ∩ (document open):
        // a child of the provider scope dies on unregister; a finalizer on
        // the document scope dies on close. Whichever fires first wins;
        // closing an already-closed scope is a no-op.
        const attachment = yield* Scope.fork(record.scope, ExecutionStrategy.sequential);
        yield* Scope.addFinalizer(document.scope, Scope.close(attachment, Exit.void));
        yield* Scope.extend(record.provider.attach(target), attachment);
      });

    const attachToView = (record: ProviderRecord, view: ViewScope): Effect.Effect<void> =>
      Effect.gen(function* () {
        if (record.provider.attachView === undefined) return;
        const document = yield* registry.get(view.documentId);
        if (Option.isNone(document)) return;
        const target = viewTargetOf(view);
        if (record.provider.appliesTo !== undefined && !record.provider.appliesTo(target)) return;

        // Same intersection as document attachments, but also bounded by the
        // view scope. Split panes of one document therefore get independent
        // mount/float subscriptions and tear down independently.
        const attachment = yield* Scope.fork(record.scope, ExecutionStrategy.sequential);
        yield* Scope.addFinalizer(document.value.scope, Scope.close(attachment, Exit.void));
        yield* Scope.addFinalizer(view.scope, Scope.close(attachment, Exit.void));
        yield* Scope.extend(record.provider.attachView(target), attachment);
      });

    const connectView = (view: ViewScope): Effect.Effect<void> =>
      Effect.gen(function* () {
        const added = yield* Ref.modify(views, (current) =>
          current.includes(view) ? [false, current] : [true, [...current, view]],
        );
        if (!added) return;
        yield* Scope.addFinalizer(
          view.scope,
          Ref.update(views, (current) => current.filter((entry) => entry !== view)),
        );
        yield* Effect.forEach(yield* Ref.get(providers), (record) => attachToView(record, view), {
          discard: true,
        });
      });

    const register = (provider: DecorationProvider): Effect.Effect<void, never, Scope.Scope> =>
      Effect.gen(function* () {
        // The provider's registration scope = the package activation scope.
        // When the package deactivates this closes, killing every attachment
        // and the subscription fiber below.
        const providerScope = yield* Effect.scope;
        const record: ProviderRecord = { provider, scope: providerScope };

        // Introspection only; retracts with the provider.
        yield* Effect.acquireRelease(
          Ref.update(providers, (list) => [...list, record]),
          () => Ref.update(providers, (list) => list.filter((entry) => entry !== record)),
        );

        // Attach to every document and view already open …
        yield* Effect.forEach(
          yield* registry.list,
          (document) => attachToDocument(record, document),
          {
            discard: true,
          },
        );
        yield* Effect.forEach(yield* Ref.get(views), (view) => attachToView(record, view), {
          discard: true,
        });

        // … and to every future document, until this provider is unregistered.
        yield* Stream.runForEach(workspaceBus.notifications, (notification) =>
          Effect.forEach(
            payloadsOf(notification, DocumentsChanged),
            (change) =>
              change.opened === undefined
                ? Effect.void
                : registry.get(change.opened).pipe(
                    Effect.flatMap(
                      Option.match({
                        onNone: () => Effect.void,
                        onSome: (document) => attachToDocument(record, document),
                      }),
                    ),
                  ),
            { discard: true },
          ),
        ).pipe(Effect.forkScoped, Effect.asVoid);
      });

    return {
      register,
      connectView,
      list: Ref.get(providers).pipe(Effect.map((list) => list.map((record) => record.provider))),
    } satisfies DecorationProviders.Service;
  });
