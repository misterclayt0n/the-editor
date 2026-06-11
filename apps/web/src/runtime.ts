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

import { Context, Effect, Exit, Fiber, Layer, Scope, Stream } from "effect";
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
  DocumentNotifications,
  editorSnapshot,
  EditorVisuals,
  makeEditorScope,
  provideEditorScope,
  setSelection,
  ViewNotifications,
  type EditorNotification,
  type EditorScope,
  type EditorSnapshot,
  type NotificationBus,
  type VisualsSnapshot,
} from "@the-editor/editor";
import { TextDocument, type Selection } from "@the-editor/engine";
import { editorDefaults, EditorSurface } from "./product/editorDefaults";
import { lineNumbers } from "./product/lineNumbers";
import { MountHub } from "./editor/mounts";

export type KeyOutcome = "handled" | "pending" | "passthrough";

export class AppRuntime {
  /** Widget mount contract: renderer announces containers, products portal in. */
  readonly mounts = new MountHub();

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

        // Single-document world: the initial document's services are handed
        // to packages directly. Multi-document routing (via the registry +
        // ActiveEditor) comes when products need it.
        const documentContext = editor.document.context;
        const services = Layer.mergeAll(
          Layer.succeed(Commands, commands),
          Layer.succeed(Keymaps, keymaps),
          Layer.succeed(Workbench, workbench),
          Layer.succeed(EditorVisuals, Context.get(documentContext, EditorVisuals)),
          Layer.succeed(DocumentNotifications, Context.get(documentContext, DocumentNotifications)),
          Layer.succeed(TextDocument, Context.get(documentContext, TextDocument)),
        );
        yield* runtime
          .activateGraph([
            toPackageDescriptor(editorDefaults, services),
            toPackageDescriptor(lineNumbers, services),
          ])
          .pipe(Effect.orDie);

        return new AppRuntime(scope, editor, panel, workbench, commands, keymaps);
      }),
    );
  }

  dispose(): void {
    // Closing the package scope interrupts forked package fibers (an async
    // boundary), so disposal cannot be runSync.
    Effect.runFork(
      Scope.close(this.scope, Exit.void).pipe(Effect.andThen(closeEditorScope(this.editor))),
    );
  }

  snapshot(): EditorSnapshot {
    return this.runInEditor(editorSnapshot());
  }

  /** Merged visuals snapshot (cached inside the service per generation). */
  visualsSnapshot(): VisualsSnapshot {
    return this.runInEditor(Effect.flatMap(EditorVisuals, (visuals) => visuals.snapshot));
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
            this.commands.execute("editor.insertText", { text: event.key }).pipe(Effect.orDie),
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
    this.runInEditor(setSelection(selection));
  }

  /**
   * Consume this editor's notification streams; returns unsubscribe.
   * Sources are tagged because serials are per bus: the document bus is
   * shared by every view of the document, the view bus is this view's own.
   */
  subscribe(
    listener: (notification: EditorNotification, source: "document" | "view") => void,
  ): () => void {
    const consume = (bus: NotificationBus, source: "document" | "view") =>
      Stream.runForEach(bus.notifications, (notification) =>
        Effect.sync(() => listener(notification, source)),
      );
    const documentBus = Context.get(this.editor.context, DocumentNotifications);
    const viewBus = Context.get(this.editor.context, ViewNotifications);
    const documentFiber = Effect.runFork(consume(documentBus, "document"));
    const viewFiber = Effect.runFork(consume(viewBus, "view"));
    return () => {
      Effect.runFork(Fiber.interrupt(documentFiber));
      Effect.runFork(Fiber.interrupt(viewFiber));
    };
  }

  private runInEditor<A, E>(effect: Effect.Effect<A, E, any>): A {
    return Effect.runSync(provideEditorScope(this.editor, effect));
  }
}
