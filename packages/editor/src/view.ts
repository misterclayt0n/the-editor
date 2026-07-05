/**
 * View scopes: one per pane. A view is a window onto a document — it owns
 * selection, the view notification bus, and renderer-local element hubs.
 * Everything document-shaped (text, history, document-anchored visuals) comes from the
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
  type ChangeDesc,
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
import { MountHub, ViewMountHub } from "./mounts";
import { FloatHub, ViewFloatHub } from "./floats";
import { makeInteractionHub, ViewInteraction } from "./interaction";
import { makeViewCursor, ViewCursor } from "./cursor";
import { makeViewVisuals, ViewVisuals, VisualsInvalidated } from "./visuals";
import type { WrapDisplay } from "./wrap";

/**
 * The view's soft-wrap display state: WRITTEN by the renderer every time
 * its wrap width changes, READ by wrap-aware commands (vertical caret
 * movement moves by VISUAL rows). The renderer owns the truth; this cell
 * is its per-view mirror — null while wrap is off or nothing rendered yet.
 */
export class ViewWrapDisplay extends Context.Tag("@the-editor/editor/ViewWrapDisplay")<
  ViewWrapDisplay,
  {
    readonly get: () => WrapDisplay | null;
    readonly set: (display: WrapDisplay | null) => void;
  }
>() {}

let nextViewId = 1;

export type CreateViewInput = {
  readonly id?: string | EditorScopeId;
  readonly selection?: Selection;
  /** Existing hub instances to expose through the view context; defaults create fresh ones. */
  readonly mounts?: MountHub;
  readonly floats?: FloatHub;
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
    const mounts = input.mounts ?? new MountHub();
    const floats = input.floats ?? new FloatHub();
    const hub = makeInteractionHub();
    const cursor = makeViewCursor();
    const viewVisuals = yield* makeViewVisuals(bus);
    let wrapDisplay: WrapDisplay | null = null;
    const wrapDisplayCell = {
      get: () => wrapDisplay,
      set: (display: WrapDisplay | null) => {
        wrapDisplay = display;
      },
    };

    const context = documentContext.pipe(
      Context.add(ActiveEditorScopeId, id),
      Context.add(ViewWrapDisplay, wrapDisplayCell),
      Context.add(ViewCursor, cursor),
      Context.add(ViewVisuals, viewVisuals),
      Context.add(ViewMountHub, mounts),
      Context.add(ViewFloatHub, floats),
      Context.add(ViewInteraction, hub),
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
          yield* viewVisuals.mapChanges({ ranges: payload.changes } as ChangeDesc);
          const current = yield* selectionState.get;
          const mapped = mapSelection(current, payload.changes);
          yield* selectionState.set(mapped);
          const visuals = yield* viewVisuals.snapshot;
          yield* bus.publish([
            invalidate(SelectionInvalidated),
            invalidate(VisualsInvalidated, {
              generation: visuals.generation,
              changes: payload.changes.map((change) => ({ from: change.fromB, to: change.toB })),
            }),
          ]);
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
