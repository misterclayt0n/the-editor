import { Effect, Fiber, Stream } from "effect";
import {
  closeEditorScope,
  DocumentInvalidated,
  EditorNotifications,
  editorSnapshot,
  invalidate,
  makeEditorScope,
  provideEditorScope,
  SelectionInvalidated,
  type EditorNotification,
  type EditorScope,
  type EditorSnapshot,
} from "@the-editor/editor";
import { Selection, SelectionState, TextDocument, type Transaction } from "@the-editor/engine";

/**
 * Synchronous facade over an `@the-editor/editor` scope.
 *
 * The kernel models document/selection state as Effect services so lifecycle
 * and ownership stay principled, but the keystroke hot path must remain
 * synchronous. `Effect.runSync` bridges the two: every mutation here completes
 * before the keydown handler returns. Publishing to the notification bus never
 * suspends, so it rides inside the same synchronous transaction; consumers
 * (the renderer) are woken asynchronously on their own fibers and never gate
 * input.
 */
export class EditorSession {
  private constructor(private readonly scope: EditorScope) {}

  static create(text: string): EditorSession {
    return new EditorSession(Effect.runSync(makeEditorScope({ text })));
  }

  dispose(): void {
    Effect.runSync(closeEditorScope(this.scope));
  }

  snapshot(): EditorSnapshot {
    return this.run(editorSnapshot());
  }

  setSelection(selection: Selection): void {
    this.run(
      Effect.gen(function* () {
        const selectionState = yield* SelectionState;
        const notifications = yield* EditorNotifications;
        yield* selectionState.set(selection);
        yield* notifications.publish([invalidate(SelectionInvalidated)]);
      }),
    );
  }

  apply(transaction: Transaction, selection: Selection): void {
    this.run(
      Effect.gen(function* () {
        const document = yield* TextDocument;
        const selectionState = yield* SelectionState;
        const notifications = yield* EditorNotifications;
        yield* document.transact(transaction).pipe(Effect.orDie);
        yield* selectionState.set(selection);
        const version = yield* document.version;
        // One batch per logical operation: consumers observe the document and
        // selection invalidations atomically.
        yield* notifications.publish([
          invalidate(DocumentInvalidated, { version }),
          invalidate(SelectionInvalidated),
        ]);
      }),
    );
  }

  /**
   * Consume this editor's notification stream. The listener fires once per
   * published batch; payloads are hints only, so listeners re-read state via
   * `snapshot()`. Returns an unsubscribe function. The stream also ends on
   * `dispose()`, when the scope shuts the bus down.
   */
  subscribe(listener: (notification: EditorNotification) => void): () => void {
    const consume = Effect.flatMap(EditorNotifications, (notifications) =>
      Stream.runForEach(notifications.notifications, (notification) =>
        Effect.sync(() => listener(notification)),
      ),
    );
    const fiber = Effect.runFork(provideEditorScope(this.scope, consume));
    return () => {
      Effect.runFork(Fiber.interrupt(fiber));
    };
  }

  private run<A, E>(effect: Effect.Effect<A, E, any>): A {
    return Effect.runSync(provideEditorScope(this.scope, effect));
  }
}
