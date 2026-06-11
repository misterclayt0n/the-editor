/**
 * The single mutation entry point. EVERY writer — typing, file reload, LSP
 * workspace edits, formatters, VCS checkouts — commits through here, so the
 * sacred sequence holds for all of them:
 *
 *   1. apply the transaction to the document
 *   2. slide every contributed visual through the edit (owners never run)
 *   3. publish DocumentInvalidated{version, changes} on the document bus
 *      (every view of the document re-renders its dirty rows)
 *   4. optionally move THIS view's selection and publish on the view bus
 *
 * Selection is view-scoped: a commit only ever touches the committing
 * view's selection. Other views of the same document keep theirs anchored
 * via the per-view sync fiber (see view.ts), which maps selections through
 * the published change ranges. ViewDocumentSync is the handshake: setting a
 * selection marks this view current for the document version it was set
 * against, so the sync fiber never double-maps it.
 */

import { Context, Effect, Ref } from "effect";
import {
  SelectionState,
  TextDocument,
  type Selection,
  type TextError,
  type Transaction,
} from "@the-editor/engine";
import {
  DocumentInvalidated,
  DocumentNotifications,
  invalidate,
  SelectionInvalidated,
  ViewNotifications,
  type DocumentChangeRange,
} from "./notifications";
import { EditorVisuals } from "./visuals";

/** Last document version a view's selection accounts for (see view.ts). */
export class ViewDocumentSync extends Context.Tag("@the-editor/editor/ViewDocumentSync")<
  ViewDocumentSync,
  Ref.Ref<number>
>() {}

/** Map an offset forward through one transaction's change ranges. */
export function mapOffsetThroughChanges(
  changes: ReadonlyArray<DocumentChangeRange>,
  pos: number,
): number {
  let diff = 0;
  for (const range of changes) {
    if (pos < range.fromA) break;
    if (pos >= range.toA) {
      diff += range.toB - range.fromB - (range.toA - range.fromA);
      continue;
    }
    return range.fromB;
  }
  return pos + diff;
}

/**
 * Apply a transaction to the current document, optionally moving the
 * committing view's selection in the same logical step.
 */
export const commitTransaction = (
  transaction: Transaction,
  selection?: Selection,
): Effect.Effect<
  void,
  TextError,
  | TextDocument
  | EditorVisuals
  | DocumentNotifications
  | SelectionState
  | ViewNotifications
  | ViewDocumentSync
> =>
  Effect.gen(function* () {
    const document = yield* TextDocument;
    const visuals = yield* EditorVisuals;
    const documentBus = yield* DocumentNotifications;
    yield* document.transact(transaction);
    yield* visuals.mapChanges(transaction.changes.desc());
    const version = yield* document.version;
    if (selection !== undefined) {
      // Mark this view current BEFORE publishing so its sync fiber skips
      // the batch; the explicit selection is already in new coordinates.
      const sync = yield* ViewDocumentSync;
      yield* Ref.set(sync, version);
    }
    yield* documentBus.publish([
      invalidate(DocumentInvalidated, {
        version,
        changes: transaction.changes.desc().ranges,
      }),
    ]);
    if (selection !== undefined) {
      yield* setSelection(selection);
    }
  });

/** Move this view's selection (current document coordinates) and notify. */
export const setSelection = (
  selection: Selection,
): Effect.Effect<
  void,
  never,
  SelectionState | ViewNotifications | TextDocument | ViewDocumentSync
> =>
  Effect.gen(function* () {
    const selectionState = yield* SelectionState;
    const viewBus = yield* ViewNotifications;
    const document = yield* TextDocument;
    const sync = yield* ViewDocumentSync;
    yield* Ref.set(sync, yield* document.version);
    yield* selectionState.set(selection);
    yield* viewBus.publish([invalidate(SelectionInvalidated)]);
  });
