/**
 * App runtime: boots the kernel package runtime, workbench, commands, and
 * keymaps; opens one editor panel; activates the default product package; and
 * exposes a synchronous facade for the host.
 *
 * The keystroke pipeline is:
 *
 *   keydown -> chord -> Keymaps.handleKey -> Commands.execute -> transaction
 *           -> notification publish -> renderer repaint
 *
 * Every hop is synchronous under Effect.runSync; only the repaint rides a
 * subscriber fiber behind the keystroke.
 */

import { Effect, Exit, Fiber, Layer, Scope, Stream } from "effect";
import { makePackageRuntime } from "@the-editor/kernel";
import { toPackageDescriptor } from "@the-editor/extensions";
import { Commands, makeCommands } from "@the-editor/commands";
import {
  chordFromKeyboardEvent,
  isTextInputChord,
  Keymaps,
  makeKeymaps,
  type Chord,
  type KeyboardEventLike,
} from "@the-editor/keymaps";
import {
  collectSlots,
  makeStack,
  makeWorkbench,
  Workbench,
  type Panel,
} from "@the-editor/workbench";
import {
  closeEditorScope,
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
import { SelectionState, type Selection } from "@the-editor/engine";
import { editorDefaults, EditorSurface } from "./product/editorDefaults";

export type KeyOutcome = "handled" | "pending" | "passthrough";

export class AppRuntime {
  private constructor(
    private readonly scope: Scope.CloseableScope,
    private readonly editor: EditorScope,
    readonly panel: Panel,
    readonly workbench: Workbench.Service,
    readonly commands: Commands.Service,
    readonly keymaps: Keymaps.Service,
  ) {}

  static boot(initialText: string): AppRuntime {
    return Effect.runSync(
      Effect.gen(function* () {
        const scope = yield* Scope.make();
        const runtime = yield* makePackageRuntime;
        const workbench = yield* Scope.extend(makeWorkbench(), scope);
        const commands = yield* makeCommands(runtime.contributionIndex, { workbench });
        const keymaps = yield* makeKeymaps(runtime.contributionIndex, { workbench });

        const editor = yield* makeEditorScope({ text: initialText });
        const panel = yield* workbench.createPanel({
          kind: EditorSurface,
          context: editor.context,
        });
        yield* workbench
          .updateArrangement((a) => ({ ...a, tiles: makeStack([panel.id]) }))
          .pipe(Effect.orDie);
        const arrangement = yield* workbench.arrangement;
        yield* workbench.setFocus(collectSlots(arrangement)[0]!.id).pipe(Effect.orDie);

        const services = Layer.mergeAll(
          Layer.succeed(Commands, commands),
          Layer.succeed(Keymaps, keymaps),
          Layer.succeed(Workbench, workbench),
        );
        yield* runtime
          .activateGraph([toPackageDescriptor(editorDefaults, services)])
          .pipe(Effect.orDie);

        return new AppRuntime(scope, editor, panel, workbench, commands, keymaps);
      }),
    );
  }

  dispose(): void {
    Effect.runSync(Scope.close(this.scope, Exit.void));
    Effect.runSync(closeEditorScope(this.editor));
  }

  snapshot(): EditorSnapshot {
    return this.runInEditor(editorSnapshot());
  }

  /**
   * The host's single keyboard entry point. Returns what the host should do:
   * handled/pending mean preventDefault; passthrough means the key is not
   * ours (browser shortcuts, IME, etc.).
   */
  handleKeyboardEvent(event: KeyboardEventLike): KeyOutcome {
    const chord = chordFromKeyboardEvent(event);
    if (chord === undefined) return "passthrough";

    const resolution = Effect.runSync(this.keymaps.handleKey(chord));
    switch (resolution._tag) {
      case "match":
        Effect.runSync(
          this.commands
            .execute(resolution.binding.command, resolution.binding.args)
            .pipe(Effect.orDie),
        );
        return "handled";
      case "pending":
        return "pending";
      case "none":
        if (isTextInputChord(chord) && event.key.length === 1) {
          Effect.runSync(
            this.commands
              .execute("editor.insertText", { text: event.key })
              .pipe(Effect.orDie),
          );
          return "handled";
        }
        return "passthrough";
    }
  }

  pendingChords(): ReadonlyArray<Chord> {
    return Effect.runSync(this.keymaps.pending);
  }

  /** Host pointer gesture: place the caret. Not a command (yet). */
  setSelection(selection: Selection): void {
    this.runInEditor(
      Effect.gen(function* () {
        const selectionState = yield* SelectionState;
        const notifications = yield* EditorNotifications;
        yield* selectionState.set(selection);
        yield* notifications.publish([invalidate(SelectionInvalidated)]);
      }),
    );
  }

  /** Consume the editor's notification stream; returns unsubscribe. */
  subscribe(listener: (notification: EditorNotification) => void): () => void {
    const consume = Effect.flatMap(EditorNotifications, (notifications) =>
      Stream.runForEach(notifications.notifications, (notification) =>
        Effect.sync(() => listener(notification)),
      ),
    );
    const fiber = Effect.runFork(provideEditorScope(this.editor, consume));
    return () => {
      Effect.runFork(Fiber.interrupt(fiber));
    };
  }

  private runInEditor<A, E>(effect: Effect.Effect<A, E, any>): A {
    return Effect.runSync(provideEditorScope(this.editor, effect));
  }
}
