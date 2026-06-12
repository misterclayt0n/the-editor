/**
 * App runtime: boots the kernel package runtime, workbench, commands, and
 * keymaps; manages one EditorPane per panel (a view scope onto a document);
 * and exposes a synchronous facade for the host chrome.
 *
 * The keystroke pipeline is:
 *
 *   keydown -> chord -> Keymaps.handleKey -> Commands.execute (at the
 *   focused panel's view context) -> transaction -> notification publish
 *   -> renderer repaint
 *
 * Every hop is synchronous under Effect.runSync; only the repaint rides a
 * subscriber fiber behind the keystroke.
 *
 * Layout is the workbench's: panes never know where they sit. The chrome
 * reads the arrangement through a mirrored snapshot (useSyncExternalStore
 * shape) and commits gestures through the ops below, all of which are
 * arrangement surgery + pane lifecycle in one place.
 */

import { Context, Effect, Exit, Fiber, Layer, Option, Scope, Stream } from "effect";
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
  activateTab,
  collectSlots,
  findSlot,
  makeStack,
  makeWorkbench,
  moveTab,
  placeInStack,
  removePanel,
  slotOfPanel,
  splitBeside,
  Workbench,
  type Arrangement,
  type Panel,
  type PanelId,
  type SlotId,
  type SplitDirection,
  type Stack,
  type Tile,
} from "@the-editor/workbench";
import {
  closeViewScope,
  closeWorkspaceScope,
  DocumentNotifications,
  DocumentRegistry,
  editorSnapshot,
  EditorVisuals,
  Files,
  FileSystemPort,
  makeViewScope,
  makeWorkspaceScope,
  setSelection,
  ViewNotifications,
  WorkspaceNotifications,
  type DocumentChangeRange,
  type DocumentScope,
  type EditorNotification,
  type EditorSnapshot,
  type FileSyncStatus,
  type NotificationBus,
  type ViewScope,
  type VisualsSnapshot,
  type WorkspaceScope,
} from "@the-editor/editor";
import { type Selection } from "@the-editor/engine";
import { editorDefaults, EditorSurface } from "./product/editorDefaults";
import { lineNumbers } from "./product/lineNumbers";
import { filesPackage, HostFileActions } from "./product/files";
import { palettePackage, HostPaletteActions } from "./product/palette";
import { viewPackage, HostViewActions } from "./product/view";
import { createFileSystemPort } from "./platform/fs";
import { createSyntaxPort, type HighlightRun, type SyntaxPort } from "./platform/syntax";
import { DocumentHighlights } from "./editor/highlights";
import { MountHub } from "./editor/mounts";

export type KeyOutcome = "handled" | "pending" | "passthrough";

/** Synchronous highlight source for the renderer's layout pass. */
export interface PaneHighlights {
  /** Bring the engine to `version` before painting it (null = resync). */
  ensureSynced(
    version: number,
    changeSets: ReadonlyArray<ReadonlyArray<DocumentChangeRange>> | null,
  ): void;
  runsBetween(from: number, to: number): readonly HighlightRun[];
  subscribe(listener: () => void): () => void;
}

/** What the per-pane renderer consumes. */
export interface PaneHost {
  readonly mounts: MountHub;
  readonly highlights?: PaneHighlights | undefined;
  snapshot(): EditorSnapshot;
  visualsSnapshot(): VisualsSnapshot;
  setSelection(selection: Selection): void;
  subscribe(
    listener: (notification: EditorNotification, source: "document" | "view") => void,
  ): () => void;
  handleKeyboardEvent(event: KeyboardEventLike): KeyOutcome;
  pendingChords(): ReadonlyArray<Chord>;
}

export type PaneFileInfo = { readonly path: string; readonly status: FileSyncStatus };

/**
 * One open editor: a view scope onto a document, displayed by a panel.
 * Panes are content; the arrangement decides where (and whether) they show.
 */
export class EditorPane implements PaneHost {
  readonly mounts = new MountHub();

  constructor(
    readonly panelId: PanelId,
    readonly document: DocumentScope,
    readonly view: ViewScope,
    private readonly runtime: AppRuntime,
  ) {}

  /** Highlight source for this pane's document (absent without a host). */
  get highlights(): PaneHighlights | undefined {
    return this.runtime.highlightsFor(this.document.id);
  }

  snapshot(): EditorSnapshot {
    return this.runIn(editorSnapshot());
  }

  visualsSnapshot(): VisualsSnapshot {
    return this.runIn(Effect.flatMap(EditorVisuals, (visuals) => visuals.snapshot));
  }

  setSelection(selection: Selection): void {
    this.runIn(setSelection(selection));
  }

  /** Path + sync status of this pane's document, when file-bound. */
  fileInfo(): PaneFileInfo | null {
    return this.runtime.fileInfoFor(this.document);
  }

  handleKeyboardEvent(event: KeyboardEventLike): KeyOutcome {
    return this.runtime.handleKeyboardEvent(event);
  }

  pendingChords(): ReadonlyArray<Chord> {
    return this.runtime.pendingChords();
  }

  /**
   * Consume this pane's notification streams; returns unsubscribe.
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
    const documentBus = Context.get(
      this.view.context as Context.Context<DocumentNotifications>,
      DocumentNotifications,
    );
    const viewBus = Context.get(
      this.view.context as Context.Context<ViewNotifications>,
      ViewNotifications,
    );
    const documentFiber = Effect.runFork(consume(documentBus, "document"));
    const viewFiber = Effect.runFork(consume(viewBus, "view"));
    return () => {
      Effect.runFork(Fiber.interrupt(documentFiber));
      Effect.runFork(Fiber.interrupt(viewFiber));
    };
  }

  private runIn<A, E>(effect: Effect.Effect<A, E, any>): A {
    return Effect.runSync(
      (effect as Effect.Effect<A, E, never>).pipe(
        Effect.provide(this.view.context as Context.Context<any>),
      ),
    );
  }
}

/** Mirrored layout state for the React chrome (stable identity per change). */
export type WorkbenchSnapshot = {
  readonly arrangement: Arrangement;
  readonly focusedSlot: SlotId | null;
};

export class AppRuntime {
  /** Host UI hooks: file sync state changed (refresh status surfaces). */
  onFileStateChanged?: () => void;
  /** Host UI hooks: the palette.open command fired (show the dialog). */
  onPaletteOpenRequested?: () => void;

  private readonly panes = new Map<PanelId, EditorPane>();
  private syntaxPort: SyntaxPort | undefined;
  private readonly highlightsByDoc = new Map<string, DocumentHighlights>();
  private workbenchSnapshot: WorkbenchSnapshot;
  private readonly workbenchListeners = new Set<() => void>();
  private mirrorFibers: ReadonlyArray<Fiber.RuntimeFiber<unknown, unknown>> = [];

  private constructor(
    private readonly scope: Scope.CloseableScope,
    readonly workspace: WorkspaceScope,
    readonly workbench: Workbench.Service,
    readonly commands: Commands.Service,
    readonly keymaps: Keymaps.Service,
  ) {
    this.workbenchSnapshot = {
      arrangement: Effect.runSync(workbench.arrangement),
      focusedSlot: Option.getOrNull(Effect.runSync(workbench.focusedSlot)),
    };
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

        const workspace = yield* makeWorkspaceScope({
          ...(fileSystem ? { fileSystem } : {}),
        });

        // Packages get WORKSPACE-level services and react to the document
        // set themselves (see lineNumbers). Per-document services flow to
        // commands through the focused panel's view context.
        const workspaceContext = workspace.context;
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
          Layer.succeed(HostPaletteActions, {
            open: () => instance.onPaletteOpenRequested?.(),
          }),
          Layer.succeed(HostViewActions, {
            splitRight: () => instance.splitFocused("row"),
            splitDown: () => instance.splitFocused("column"),
            closeTab: () => instance.closeFocusedTab(),
            nextTab: () => instance.cycleTab(1),
            previousTab: () => instance.cycleTab(-1),
          }),
        );
        yield* runtime
          .activateGraph([
            toPackageDescriptor(editorDefaults, services),
            toPackageDescriptor(lineNumbers, services),
            toPackageDescriptor(filesPackage, services),
            toPackageDescriptor(palettePackage, services),
            toPackageDescriptor(viewPackage, services),
          ])
          .pipe(Effect.orDie);

        return new AppRuntime(scope, workspace, workbench, commands, keymaps);
      }),
    );
    // Syntax host (absent in plain browsers: capability gating).
    instance.syntaxPort = createSyntaxPort();
    // First pane: a scratch document in a single stack.
    const registry = Context.get(instance.workspace.context, DocumentRegistry);
    const scratch = Effect.runSync(registry.open({ text: initialText }));
    instance.attachHighlights(scratch, undefined);
    const pane = instance.createPane(scratch);
    Effect.runSync(
      instance.workbench
        .updateArrangement((a) => ({ ...a, tiles: makeStack([pane.panelId]) }))
        .pipe(Effect.orDie),
    );
    instance.focusSlotOfPanel(pane.panelId);
    instance.startWorkbenchMirror();
    return instance;
  }

  // ---------------------------------------------------------------- layout

  /** Stable layout snapshot for useSyncExternalStore. */
  getWorkbenchSnapshot(): WorkbenchSnapshot {
    return this.workbenchSnapshot;
  }

  subscribeWorkbench(listener: () => void): () => void {
    this.workbenchListeners.add(listener);
    return () => this.workbenchListeners.delete(listener);
  }

  pane(panelId: PanelId): EditorPane | undefined {
    return this.panes.get(panelId);
  }

  focusedPane(): EditorPane | undefined {
    const focused = Effect.runSync(this.workbench.focusedPanel);
    return Option.isNone(focused) ? undefined : this.panes.get(focused.value.id);
  }

  private startWorkbenchMirror(): void {
    const publish = () => {
      this.workbenchSnapshot = {
        arrangement: Effect.runSync(this.workbench.arrangement),
        focusedSlot: Option.getOrNull(Effect.runSync(this.workbench.focusedSlot)),
      };
      for (const listener of this.workbenchListeners) listener();
    };
    publish();
    this.mirrorFibers = [
      Effect.runFork(
        Stream.runForEach(this.workbench.arrangementChanges, () => Effect.sync(publish)),
      ),
      Effect.runFork(Stream.runForEach(this.workbench.focusChanges, () => Effect.sync(publish))),
    ];
  }

  // ------------------------------------------------------------- gestures

  focusSlot(slotId: SlotId): void {
    const current = this.workbenchSnapshot.focusedSlot;
    if (current === slotId) return;
    Effect.runSync(this.workbench.setFocus(slotId).pipe(Effect.ignore));
  }

  activateTab(slotId: SlotId, panelId: PanelId): void {
    Effect.runSync(
      this.workbench.updateArrangement((a) => activateTab(a, slotId, panelId)).pipe(Effect.orDie),
    );
    this.focusSlot(slotId);
  }

  /** Close a tab: the panel dies; the document follows when unreferenced. */
  closeTab(panelId: PanelId): void {
    const pane = this.panes.get(panelId);
    if (pane === undefined) return;
    this.panes.delete(panelId);
    Effect.runSync(this.workbench.closePanel(panelId));
    Effect.runSync(closeViewScope(pane.view));
    const documentStillShown = [...this.panes.values()].some(
      (other) => other.document.id === pane.document.id,
    );
    if (!documentStillShown) {
      this.highlightsByDoc.get(String(pane.document.id))?.close();
      this.highlightsByDoc.delete(String(pane.document.id));
      const files = Context.getOption(this.workspace.context, Files);
      if (Option.isSome(files)) Effect.runSync(files.value.unbind(pane.document.id));
      const registry = Context.get(this.workspace.context, DocumentRegistry);
      Effect.runSync(registry.close(pane.document.id));
    }
  }

  closeFocusedTab(): void {
    const focused = Effect.runSync(this.workbench.focusedPanel);
    if (Option.isSome(focused)) this.closeTab(focused.value.id);
  }

  /** Split the focused stack: a new view of the same document beside it. */
  splitFocused(direction: SplitDirection): void {
    const { focusedSlot } = this.workbenchSnapshot;
    const source = this.focusedPane();
    if (focusedSlot === null || source === undefined) return;
    const pane = this.createPane(source.document, {
      selection: source.snapshot().selection,
    });
    Effect.runSync(
      this.workbench
        .updateArrangement((a) => splitBeside(a, focusedSlot, pane.panelId, direction))
        .pipe(Effect.orDie),
    );
    this.focusSlotOfPanel(pane.panelId);
  }

  /** Cycle the active tab within the focused stack. */
  cycleTab(delta: 1 | -1): void {
    const { arrangement, focusedSlot } = this.workbenchSnapshot;
    if (focusedSlot === null) return;
    const slot = findSlot(arrangement, focusedSlot);
    if (slot === undefined || slot.kind !== "stack" || slot.stack.panels.length < 2) return;
    const stack = slot.stack;
    const next = (stack.active + delta + stack.panels.length) % stack.panels.length;
    this.activateTab(focusedSlot, stack.panels[next]!);
  }

  /** Drag commit: move a tab into another stack (or reorder within one). */
  moveTabToStack(panelId: PanelId, fromSlot: SlotId, toSlot: SlotId, index?: number): void {
    Effect.runSync(
      this.workbench
        .updateArrangement((a) => {
          // Same-stack reorder: the insert index was computed against the
          // list that still contains the tab; account for its removal.
          let insertAt = index;
          if (insertAt !== undefined && fromSlot === toSlot) {
            const slot = findSlot(a, fromSlot);
            if (slot?.kind === "stack") {
              const from = slot.stack.panels.indexOf(panelId);
              if (from !== -1 && from < insertAt) insertAt -= 1;
            }
          }
          return moveTab(a, panelId, fromSlot, toSlot, insertAt === undefined ? {} : { index: insertAt });
        })
        .pipe(Effect.orDie),
    );
    this.focusSlot(toSlot);
  }

  /** Drag commit: tear a tab out into a new split beside the target stack. */
  moveTabToSplit(
    panelId: PanelId,
    targetSlot: SlotId,
    direction: SplitDirection,
    before: boolean,
  ): void {
    const { arrangement } = this.workbenchSnapshot;
    const target = findSlot(arrangement, targetSlot);
    if (target === undefined || target.kind !== "stack") return;
    // Tearing the only tab of a stack into a split of that same stack is a
    // no-op shuffle; skip it.
    if (target.stack.panels.length === 1 && target.stack.panels[0] === panelId) return;
    Effect.runSync(
      this.workbench
        .updateArrangement((a) =>
          splitBeside(removePanel(a, panelId), targetSlot, panelId, direction, { before }),
        )
        .pipe(Effect.orDie),
    );
    this.focusSlotOfPanel(panelId);
  }

  /** Resize commit: replace the weights of a split node (path from root). */
  setSplitWeights(path: ReadonlyArray<number>, weights: ReadonlyArray<number>): void {
    const rewrite = (tile: Tile, depth: number): Tile => {
      if (tile.kind !== "split") return tile;
      if (depth === path.length) {
        if (weights.length !== tile.children.length) return tile;
        return {
          ...tile,
          children: tile.children.map((child, i) => ({ ...child, weight: weights[i]! })),
        };
      }
      const index = path[depth]!;
      const child = tile.children[index];
      if (child === undefined) return tile;
      return {
        ...tile,
        children: tile.children.map((c, i) =>
          i === index ? { ...c, tile: rewrite(c.tile, depth + 1) } : c,
        ),
      };
    };
    Effect.runSync(
      this.workbench
        .updateArrangement((a) =>
          a.tiles === null ? a : { ...a, tiles: rewrite(a.tiles, 0) },
        )
        .pipe(Effect.ignore),
    );
  }

  private focusSlotOfPanel(panelId: PanelId): void {
    const arrangement = Effect.runSync(this.workbench.arrangement);
    const slot = slotOfPanel(arrangement, panelId);
    if (slot !== undefined) this.focusSlot(slot);
  }

  // ---------------------------------------------------------------- syntax

  highlightsFor(docId: unknown): PaneHighlights | undefined {
    return this.highlightsByDoc.get(String(docId));
  }

  private attachHighlights(document: DocumentScope, path: string | undefined): void {
    if (this.syntaxPort === undefined) return;
    const key = String(document.id);
    if (this.highlightsByDoc.has(key)) return;
    this.highlightsByDoc.set(
      key,
      new DocumentHighlights(this.syntaxPort, key, document, path),
    );
  }

  // ----------------------------------------------------------------- files

  fileInfoFor(document: DocumentScope): PaneFileInfo | null {
    const files = Context.getOption(this.workspace.context, Files);
    if (Option.isNone(files)) return null;
    const binding = Effect.runSync(files.value.bindingFor(document.id));
    return Option.isNone(binding)
      ? null
      : { path: binding.value.path, status: binding.value.status };
  }

  /** Open a path as a tab in the focused stack (activating an existing one). */
  async openPath(path: string): Promise<void> {
    const files = Context.getOption(this.workspace.context, Files);
    if (Option.isNone(files)) return;
    const document = await Effect.runPromise(files.value.openPath(path));
    this.attachHighlights(document, path);

    const { arrangement, focusedSlot } = this.workbenchSnapshot;
    const slot = focusedSlot === null ? undefined : findSlot(arrangement, focusedSlot);
    const stack: Stack | undefined = slot?.kind === "stack" ? slot.stack : undefined;

    if (stack !== undefined) {
      for (const panelId of stack.panels) {
        if (this.panes.get(panelId)?.document.id === document.id) {
          this.activateTab(stack.id, panelId);
          return;
        }
      }
    }

    const pane = this.createPane(document);
    Effect.runSync(
      this.workbench
        .updateArrangement((a) => {
          if (a.tiles === null) return { ...a, tiles: makeStack([pane.panelId]) };
          if (stack !== undefined) return placeInStack(a, stack.id, pane.panelId);
          // Focused slot is an overlay (or nothing): fall back to the first stack.
          const first = collectSlots(a).find((s) => s.kind === "stack");
          return first === undefined
            ? { ...a, tiles: makeStack([pane.panelId]) }
            : placeInStack(a, first.id, pane.panelId);
        })
        .pipe(Effect.orDie),
    );
    this.focusSlotOfPanel(pane.panelId);
  }

  async openFileViaDialog(): Promise<void> {
    const port = Context.getOption(this.workspace.context, FileSystemPort);
    if (Option.isNone(port)) return;
    const picked = await Effect.runPromise(port.value.pickFile());
    if (Option.isSome(picked)) await this.openPath(picked.value);
  }

  async saveActiveFile(): Promise<void> {
    const pane = this.focusedPane();
    if (pane === undefined) return;
    const files = Context.getOption(this.workspace.context, Files);
    if (Option.isNone(files)) return;
    await Effect.runPromise(
      files.value.save(pane.document.id).pipe(Effect.catchAll(() => Effect.void)),
    );
    this.onFileStateChanged?.();
  }

  // ------------------------------------------------------------- keyboard

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

  // ------------------------------------------------------------ lifecycle

  private createPane(document: DocumentScope, options?: { selection?: Selection }): EditorPane {
    const view = Effect.runSync(
      makeViewScope(document, options?.selection ? { selection: options.selection } : {}),
    );
    const panel: Panel = Effect.runSync(
      this.workbench.createPanel({ kind: EditorSurface, context: view.context }),
    );
    const pane = new EditorPane(panel.id, document, view, this);
    this.panes.set(panel.id, pane);
    return pane;
  }

  dispose(): void {
    // Closing the package scope interrupts forked package fibers (an async
    // boundary), so disposal cannot be runSync.
    for (const highlights of this.highlightsByDoc.values()) highlights.close();
    this.highlightsByDoc.clear();
    for (const fiber of this.mirrorFibers) Effect.runFork(Fiber.interrupt(fiber));
    const views = [...this.panes.values()].map((pane) => pane.view);
    this.panes.clear();
    Effect.runFork(
      Scope.close(this.scope, Exit.void).pipe(
        Effect.andThen(Effect.forEach(views, closeViewScope, { discard: true })),
        Effect.andThen(closeWorkspaceScope(this.workspace)),
      ),
    );
  }
}
