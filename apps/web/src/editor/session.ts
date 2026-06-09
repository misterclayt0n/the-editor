import { Effect } from "effect";
import {
  closeEditorScope,
  editorSnapshot,
  makeEditorScope,
  provideEditorScope,
  type EditorScope,
  type EditorSnapshot,
} from "@the-editor/editor";
import {
  Selection,
  SelectionState,
  TextDocument,
  type Transaction,
} from "@the-editor/engine";

/**
 * Synchronous facade over an `@the-editor/editor` scope.
 *
 * The kernel models document/selection state as Effect services so lifecycle
 * and ownership stay principled, but the keystroke hot path must remain
 * synchronous. `Effect.runSync` bridges the two: every method here completes
 * before the keydown handler returns.
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
    this.run(Effect.flatMap(SelectionState, (state) => state.set(selection)));
  }

  apply(transaction: Transaction, selection: Selection): void {
    this.run(
      Effect.gen(function* () {
        const document = yield* TextDocument;
        const selectionState = yield* SelectionState;
        yield* document.transact(transaction).pipe(Effect.orDie);
        yield* selectionState.set(selection);
      }),
    );
  }

  private run<A, E>(effect: Effect.Effect<A, E, any>): A {
    return Effect.runSync(provideEditorScope(this.scope, effect));
  }
}
