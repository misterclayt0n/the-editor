/**
 * View scopes: one per pane. A view is a window onto a document — it owns
 * selection and the view notification bus, nothing else. Everything
 * document-shaped (text, history, document-anchored visuals) comes from the
 * document context the view extends, which is what makes two splits of one
 * document share edits for free.
 *
 * Selection coherence across views: every view runs a small sync fiber that
 * maps its OWN selection through document changes published by other
 * writers, so a caret in split B stays anchored to its text while split A
 * types. The committing view marks itself current (commitTransaction with an
 * explicit selection bumps ViewDocumentSync), so it is never double-mapped.
 */

import { Context, Effect, Exit, Fiber, Ref, Scope, Stream } from "effect";
import {
  makeSelectionState,
  Selection,
  SelectionRange,
  SelectionState,
  TextDocument,
} from "@the-editor/engine";
import { ActiveDocumentId, ActiveEditorScopeId, EditorScopeId, type DocumentId } from "./services";
import {
  DocumentInvalidated,
  DocumentNotifications,
  invalidate,
  makeNotificationBus,
  payloadsOf,
  SelectionInvalidated,
  ViewNotifications,
  type DocumentChangeRange,
} from "./notifications";
import { mapOffsetThroughChanges, ViewDocumentSync } from "./commit";

let nextViewId = 1;

export type CreateViewInput = {
  readonly id?: string | EditorScopeId;
  readonly selection?: Selection;
};

export interface ViewScope {
  readonly id: EditorScopeId;
  readonly documentId: DocumentId;
  readonly scope: Scope.CloseableScope;
  /** Document context + this view's services. */
  readonly context: Context.Context<any>;
}

function viewId(id: string | EditorScopeId | undefined): EditorScopeId {
  return typeof id === "string"
    ? EditorScopeId.make(id)
    : (id ?? EditorScopeId.make(`view:${nextViewId++}`));
}

function mapSelection(
  selection: Selection,
  changes: ReadonlyArray<DocumentChangeRange>,
): Selection {
  return Selection.create(
    selection.ranges.map((range) =>
      SelectionRange.create(
        mapOffsetThroughChanges(changes, range.anchor),
        mapOffsetThroughChanges(changes, range.head),
      ),
    ),
  );
}

export function makeViewScope(
  document: DocumentScopeLike,
  input: CreateViewInput = {},
): Effect.Effect<ViewScope> {
  return Effect.gen(function* () {
    const id = viewId(input.id);
    const scope = yield* Scope.make();
    const selectionState = yield* makeSelectionState({
      selection: input.selection ?? Selection.point(0),
    });
    const bus = yield* Scope.extend(makeNotificationBus(), scope);
    const documentContext = document.context as Context.Context<any>;
    const textDocument = Context.get(
      documentContext as Context.Context<TextDocument>,
      TextDocument,
    );
    const documentBus = Context.get(
      documentContext as Context.Context<DocumentNotifications>,
      DocumentNotifications,
    );
    const syncedVersion = yield* Ref.make(yield* textDocument.version);

    const context = documentContext.pipe(
      Context.add(ActiveEditorScopeId, id),
      Context.add(SelectionState, selectionState),
      Context.add(ViewNotifications, bus),
      Context.add(ViewDocumentSync, syncedVersion),
    ) as Context.Context<any>;

    // Selection sync: map this view's selection through document edits it
    // did not commit itself. Forked as a daemon with fire-and-forget
    // interruption so closing a view stays synchronous (runSync-safe).
    const syncFiber = yield* Stream.runForEach(documentBus.notifications, (notification) =>
      Effect.gen(function* () {
        for (const payload of payloadsOf(notification, DocumentInvalidated)) {
          const synced = yield* Ref.get(syncedVersion);
          if (payload.version <= synced) continue;
          yield* Ref.set(syncedVersion, payload.version);
          if (payload.changes === undefined || payload.changes.length === 0) continue;
          const current = yield* selectionState.get;
          const mapped = mapSelection(current, payload.changes);
          yield* selectionState.set(mapped);
          yield* bus.publish([invalidate(SelectionInvalidated)]);
        }
      }),
    ).pipe(Effect.forkDaemon);
    yield* Scope.addFinalizer(scope, Fiber.interruptFork(syncFiber));

    const documentId = Context.get(
      documentContext as Context.Context<ActiveDocumentId>,
      ActiveDocumentId,
    );
    return { id, documentId, scope, context } satisfies ViewScope;
  });
}

/** Structural subset of DocumentScope, avoiding an import cycle. */
export type DocumentScopeLike = {
  readonly context: Context.Context<any>;
};

export function closeViewScope(view: ViewScope): Effect.Effect<void> {
  return Scope.close(view.scope, Exit.void);
}

export function provideViewScope<A, E, R>(
  view: ViewScope,
  effect: Effect.Effect<A, E, R>,
): Effect.Effect<A, E> {
  return effect.pipe(Effect.provide(view.context as any)) as Effect.Effect<A, E>;
}
