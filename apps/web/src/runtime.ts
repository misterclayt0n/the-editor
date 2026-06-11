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
  closeViewScope,
  DocumentNotifications,
  DocumentRegistry,
  editorSnapshot,
  EditorVisuals,
  Files,
  FileSystemPort,
  makeEditorScope,
  makeViewScope,
  provideEditorScope,
  setSelection,
  ViewNotifications,
  WorkspaceNotifications,
  type EditorNotification,
  type EditorScope,
  type EditorSnapshot,
  type FileSyncStatus,
  type NotificationBus,
  type VisualsSnapshot,
} from "@the-editor/editor";
import { Option } from "effect";
import { type Selection } from "@the-editor/engine";
import { editorDefaults, EditorSurface } from "./product/editorDefaults";
import { lineNumbers } from "./product/lineNumbers";
import { filesPackage, HostFileActions } from "./product/files";
import { createFileSystemPort } from "./platform/fs";
import { MountHub } from "./editor/mounts";

export type KeyOutcome = "handled" | "pending" | "passthrough";

export class AppRuntime {
  /** Widget mount contract: renderer announces containers, products portal in. */
  readonly mounts = new MountHub();
  /** Host UI hooks: a different document is now active (remount the view). */
  onDocumentSwitched?: () => void;
  /** Host UI hooks: file sync state changed (refresh status surfaces). */
  onFileStateChanged?: () => void;

  private constructor(
    private readonly scope: Scope.CloseableScope,
    private editor: EditorScope,
    private currentPanel: Panel,
    readonly workbench: Workbench.Service,
    readonly commands: Commands.Service,
    readonly keymaps: Keymaps.Service,
  ) {}

  get panel(): Panel {
    return this.currentPanel;
  }

  static boot(initialText: string): AppRuntime {
    const fileSystem = createFileSystemPort();
    let instance!: AppRuntime;
    instance = Effect.runSync(
      Effect.gen(function* () {
        const scope = yield* Scope.make();
        const runtime = yield* makePackageRuntime;
        const workbench = yield* Scope.extend(makeWorkbench(), scope);
        const commands = yield* makeCommands(runtime.contributionIndex, { workbench });
        const keymaps = yield* makeKeymaps(runtime.contributionIndex, { workbench });

        const editor = yield* makeEditorScope({
          text: initialText,
          ...(fileSystem ? { fileSystem } : {}),
        });
        const panel = yield* workbench.createPanel({
          kind: EditorSurface,
          context: editor.context,
        });
        yield* workbench
          .updateArrangement((a) => ({ ...a, tiles: makeStack([panel.id]) }))
          .pipe(Effect.orDie);
        const arrangement = yield* workbench.arrangement;
        yield* workbench.setFocus(collectSlots(arrangement)[0]!.id).pipe(Effect.orDie);

        // Packages get WORKSPACE-level services and react to the document
        // set themselves (see lineNumbers). Per-document services flow to
        // commands through the focused panel's view context.
        const workspaceContext = editor.workspace.context;
        const services = Layer.mergeAll(
          Layer.succeed(Commands, commands),
          Layer.succeed(Keymaps, keymaps),
          Layer.succeed(Workbench, workbench),
          Layer.succeed(DocumentRegistry, Context.get(workspaceContext, DocumentRegistry)),
          Layer.succeed(
            WorkspaceNotifications,
            Context.get(workspaceContext, WorkspaceNotifications),
          ),
          Layer.succeed(HostFileActions, {
            save: () => void instance.saveActiveFile(),
            open: () => void instance.openFileViaDialog(),
          }),
        );
        yield* runtime
          .activateGraph([
            toPackageDescriptor(editorDefaults, services),
            toPackageDescriptor(lineNumbers, services),
            toPackageDescriptor(filesPackage, services),
          ])
          .pipe(Effect.orDie);

        return new AppRuntime(scope, editor, panel, workbench, commands, keymaps);
      }),
    );
    return instance;
  }

  /** Path + sync status of the active document, when file-bound. */
  fileInfo(): { path: string; status: FileSyncStatus } | null {
    const files = Context.getOption(this.editor.workspace.context, Files);
    if (Option.isNone(files)) return null;
    const binding = Effect.runSync(files.value.bindingFor(this.editor.document.id));
    return Option.isNone(binding)
      ? null
      : { path: binding.value.path, status: binding.value.status };
  }

  /** Open a path into the active editor area (replacing the current view). */
  async openPath(path: string): Promise<void> {
    const files = Context.getOption(this.editor.workspace.context, Files);
    if (Option.isNone(files)) return;
    const document = await Effect.runPromise(files.value.openPath(path));
    if (document.id === this.editor.document.id) return;
    const previous = {
      document: this.editor.document,
      view: this.editor.view,
      panel: this.currentPanel,
    };
    const view = await Effect.runPromise(makeViewScope(document));
    const workbench = this.workbench;
    const panel = Effect.runSync(
      Effect.gen(function* () {
        const created = yield* workbench.createPanel({
          kind: EditorSurface,
          context: view.context,
        });
        yield* workbench
          .updateArrangement((a) => ({ ...a, tiles: makeStack([created.id]) }))
          .pipe(Effect.orDie);
        const arrangement = yield* workbench.arrangement;
        yield* workbench.setFocus(collectSlots(arrangement)[0]!.id).pipe(Effect.orDie);
        yield* workbench.closePanel(previous.panel.id).pipe(Effect.ignore);
        return created;
      }),
    );
    this.editor = {
      ...this.editor,
      id: view.id,
      context: view.context,
      document,
      view,
    };
    this.currentPanel = panel;
    Effect.runSync(closeViewScope(previous.view));
    // Drop the unbound scratch document the app booted with.
    const previousBinding = Effect.runSync(files.value.bindingFor(previous.document.id));
    if (Option.isNone(previousBinding)) {
      const registry = Context.get(this.editor.workspace.context, DocumentRegistry);
      Effect.runSync(registry.close(previous.document.id));
    }
    this.onDocumentSwitched?.();
  }

  async openFileViaDialog(): Promise<void> {
    const port = Context.getOption(this.editor.workspace.context, FileSystemPort);
    if (Option.isNone(port)) return;
    const picked = await Effect.runPromise(port.value.pickFile());
    if (Option.isSome(picked)) await this.openPath(picked.value);
  }

  async saveActiveFile(): Promise<void> {
    const files = Context.getOption(this.editor.workspace.context, Files);
    if (Option.isNone(files)) return;
    await Effect.runPromise(
      files.value.save(this.editor.document.id).pipe(Effect.catchAll(() => Effect.void)),
    );
    this.onFileStateChanged?.();
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
