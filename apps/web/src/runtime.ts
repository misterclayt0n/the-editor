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
import { makeThemes, Themes, themeDefaults, type ResolvedTheme } from "@the-editor/theme";
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
  dockEdge,
  findSlot,
  makeStack,
  makeWorkbench,
  moveTab,
  placeInStack,
  removePanel,
  slotOfPanel,
  splitBeside,
  toggleStackTabs,
  Workbench,
  type Arrangement,
  type Panel,
  type SurfaceKind,
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
import { Selection } from "@the-editor/engine";
import { editorDefaults, EditorSurface } from "./product/editorDefaults";
import { lineNumbers } from "./product/lineNumbers";
import { filesPackage, HostFileActions } from "./product/files";
import { palettePackage, HostPaletteActions } from "./product/palette";
import { viewPackage, HostViewActions } from "./product/view";
import { themeCommands, HostThemeActions } from "./product/themes";
import { createFileSystemPort } from "./platform/fs";
import { createFileFinderPort, type FileFinderPort } from "./platform/fileFinder";
import { createProcessPort } from "./platform/process";
import { createTerminalPort, type TerminalPort } from "./platform/terminal";
import { createSyntaxPort, type HighlightRun, type SyntaxPort } from "./platform/syntax";
import { DocumentHighlights } from "./editor/highlights";
import { FloatHub } from "./editor/floats";
import { LspManager, type DiagnosticCounts } from "./lsp/manager";
import { lspDefaults, LanguageServers, makeLanguageServers } from "./product/lsp";
import { lspActions, HostLspActions } from "./product/lspActions";
import { lspIntel, HostLspIntel } from "./product/lspIntel";
import { intelActions, HostIntelActions } from "./product/intelActions";
import { wordCompletions } from "./product/wordCompletions";
import { makePanelRenderers, PanelRenderers } from "./workbench/panelRenderers";
import { fileTreePackage, FileTreeSurface, HostFileTree } from "./product/fileTree";
import { createFileTreeRenderer } from "./filetree/mount";
import { terminalPackage, TerminalSurface, HostTerminal } from "./product/terminal";
import { createTerminalRenderer } from "./terminal/mount";
import { HostVcsDefaults, makeVcsProviders, vcsDefaults, VcsProviders } from "./product/vcs";
import { createGitProvider } from "./vcs/git";
import { VcsManager } from "./vcs/manager";
import {
  makeCompletionProviders,
  makeHoverProviders,
  makeSignatureProviders,
  CompletionProviders,
  HoverProviders,
  SignatureProviders,
} from "./intel/types";
import { uriToPath, type lsp } from "@the-editor/lsp";
import { MountHub } from "./editor/mounts";
import { applyThemeToDocument } from "./theme/dom";

export type KeyOutcome = "handled" | "pending" | "passthrough";

/** A surface that owns its own typing (form fields, contenteditable). */
function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || target.isContentEditable;
}

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
  readonly floats: FloatHub;
  readonly highlights?: PaneHighlights | undefined;
  /** Intercepts keydown BEFORE keymaps (completion navigation). */
  keyInterceptor?: ((event: KeyboardEventLike) => boolean) | undefined;
  /** Pointer hover over text: offset (null = left the text). */
  pointerHover?: ((offset: number | null) => void) | undefined;
  /** Scroll position changed: hover must hide NOW (Zed semantics). */
  pointerScrolled?: (() => void) | undefined;
  snapshot(): EditorSnapshot;
  visualsSnapshot(): VisualsSnapshot;
  setSelection(selection: Selection): void;
  subscribeTheme(listener: () => void): () => void;
  subscribe(
    listener: (notification: EditorNotification, source: "document" | "view") => void,
  ): () => void;
}

export type PaneFileInfo = { readonly path: string; readonly status: FileSyncStatus };

/**
 * One open editor: a view scope onto a document, displayed by a panel.
 * Panes are content; the arrangement decides where (and whether) they show.
 */
export class EditorPane implements PaneHost {
  readonly mounts = new MountHub();
  readonly floats = new FloatHub();
  keyInterceptor?: ((event: KeyboardEventLike) => boolean) | undefined;
  pointerHover?: ((offset: number | null) => void) | undefined;
  pointerScrolled?: (() => void) | undefined;

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

  subscribeTheme(listener: () => void): () => void {
    return this.runtime.subscribeTheme(listener);
  }

  setSelection(selection: Selection): void {
    this.runIn(setSelection(selection));
  }

  /** Path + sync status of this pane's document, when file-bound. */
  fileInfo(): PaneFileInfo | null {
    return this.runtime.fileInfoFor(this.document);
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
  /** Every live panel (placed AND alive-but-undisplayed), so the panel pool
   *  keeps a hidden panel's content mounted instead of disposing it. */
  readonly panelIds: ReadonlyArray<PanelId>;
};

export class AppRuntime {
  /** Host UI hooks: file sync state changed (refresh status surfaces). */
  onFileStateChanged?: () => void;
  /** Host UI hooks: the palette.open command fired (show the dialog). */
  onPaletteOpenRequested?: () => void;
  /** Host UI hooks: file.find command fired (show the fff picker). */
  onFileFinderOpenRequested?: () => void;
  /** Host UI hooks: theme selector command fired (show themed palette mode). */
  onThemeSelectorOpenRequested?: () => void;
  /** Host UI hooks: diagnostics changed for a document (status surfaces). */
  onDiagnosticsChanged?: (docId: string) => void;
  /** Host UI hooks: rename was requested at the caret (show the prompt). */
  onRenameRequested?: (currentName: string) => void;
  /** Host UI hooks: explicit completion trigger (ctrl+space). */
  onTriggerCompletion?: () => void;
  /** Host UI hooks: explicit signature-help trigger (ctrl+shift+space). */
  onTriggerSignatureHelp?: () => void;
  /** Host UI hooks: the pending chord prefix changed (status surfaces). */
  onPendingChords?: (chords: ReadonlyArray<Chord>) => void;
  /** Host UI hook: ask the user for a string (resolves null on cancel). */
  onPrompt?: (options: {
    readonly initial?: string;
    readonly placeholder?: string;
  }) => Promise<string | null>;
  /** Panel renderers for non-editor surface kinds (file tree, custom UIs). */
  panelRenderers!: PanelRenderers.Service;
  /** Version control (undefined without a process host). */
  vcs: VcsManager | undefined;
  /** Intel provider registries (App hands them to the IntelController). */
  intel!: {
    readonly completions: CompletionProviders.Service;
    readonly hovers: HoverProviders.Service;
    readonly signatures: SignatureProviders.Service;
  };

  private readonly panes = new Map<PanelId, EditorPane>();
  /**
   * The "active editor group": the last stack the user focused that hosts an
   * editor pane. Opening a file targets THIS, not the raw focused slot — so
   * clicking a file in the tree (which moves focus to the tree) still opens in
   * the editor you were last working in, and a focused terminal/tool panel
   * never hijacks file opens. Focusing a tool panel leaves it unchanged.
   */
  private lastEditorSlot: SlotId | null = null;
  /** Re-emit the workbench snapshot (set by the mirror); see closePanel paths. */
  private republishWorkbench: () => void = () => {};
  private syntaxPort: SyntaxPort | undefined;
  private lsp: LspManager | undefined;
  private terminal: TerminalPort | undefined;
  private fileFinder: FileFinderPort | undefined | null = undefined;
  private pendingRename: { docId: string; offset: number } | null = null;
  private readonly highlightsByDoc = new Map<string, DocumentHighlights>();
  private workbenchSnapshot: WorkbenchSnapshot;
  private readonly workbenchListeners = new Set<() => void>();
  private mirrorFibers: ReadonlyArray<Fiber.RuntimeFiber<unknown, unknown>> = [];
  private themeFiber: Fiber.RuntimeFiber<unknown, unknown> | undefined;
  private readonly themeListeners = new Set<() => void>();
  private systemAppearanceMedia: MediaQueryList | undefined;
  private systemAppearanceListener: (() => void) | undefined;
  /** The panel each toggle (terminal, file tree) currently owns as its dock.
   *  Cleared when the user moves or closes it, so a manually-placed panel is
   *  released from the toggle (Zed: a terminal dragged out of the dock is no
   *  longer "the dock terminal"; the next toggle creates a fresh one). */
  private readonly dockedPanels = new Map<SurfaceKind, PanelId>();
  /**
   * How to focus each live panel's surface, registered by its renderer (the
   * editor focuses its `.editor`; the terminal calls `term.focus()`; the file
   * tree focuses its rows). The single focus authority: `focusActivePanel`
   * invokes the focused panel's entry so every focus change — tab switch, ⌘J
   * toggle, split, dialog dismissal — lands the keyboard in its surface.
   */
  private readonly panelFocusers = new Map<PanelId, () => void>();

  private constructor(
    private readonly scope: Scope.CloseableScope,
    readonly workspace: WorkspaceScope,
    readonly workbench: Workbench.Service,
    readonly commands: Commands.Service,
    readonly keymaps: Keymaps.Service,
    readonly themes: Themes.Service,
  ) {
    this.workbenchSnapshot = {
      arrangement: Effect.runSync(workbench.arrangement),
      focusedSlot: Option.getOrNull(Effect.runSync(workbench.focusedSlot)),
      panelIds: Effect.runSync(workbench.panels).map((panel) => panel.id),
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
        const languageServers = yield* makeLanguageServers;
        const panelRenderers = makePanelRenderers();
        const vcsProviders = yield* makeVcsProviders;
        const vcsProcessPort = createProcessPort();
        const completionProviders = yield* makeCompletionProviders;
        const hoverProviders = yield* makeHoverProviders;
        const signatureProviders = yield* makeSignatureProviders;
        const themes = yield* makeThemes();
        const services = Layer.mergeAll(
          Layer.succeed(Themes, themes),
          Layer.succeed(PanelRenderers, panelRenderers),
          Layer.succeed(VcsProviders, vcsProviders),
          Layer.succeed(HostVcsDefaults, {
            providers: vcsProcessPort !== undefined ? [createGitProvider(vcsProcessPort)] : [],
          }),
          Layer.succeed(HostFileTree, {
            // `instance` is assigned at the end of boot; defer every deref.
            renderer: {
              title: (panel) => createFileTreeRenderer(instance).title(panel),
              mount: (element, panel) => createFileTreeRenderer(instance).mount(element, panel),
            },
            toggle: () => instance.togglePanel(FileTreeSurface, { dock: "left" }),
          }),
          Layer.succeed(HostTerminal, {
            renderer: {
              title: (panel) => createTerminalRenderer(instance).title(panel),
              mount: (element, panel) => createTerminalRenderer(instance).mount(element, panel),
            },
            toggle: () => instance.togglePanel(TerminalSurface, { dock: "bottom", ratio: 0.32 }),
          }),
          Layer.succeed(CompletionProviders, completionProviders),
          Layer.succeed(HoverProviders, hoverProviders),
          Layer.succeed(SignatureProviders, signatureProviders),
          Layer.succeed(HostIntelActions, {
            triggerCompletion: () => instance.onTriggerCompletion?.(),
            triggerSignatureHelp: () => instance.onTriggerSignatureHelp?.(),
          }),
          Layer.succeed(HostLspIntel, {
            completion: (context) => instance.lsp?.completionAt(context) ?? Promise.resolve(null),
            hover: (context) => instance.lsp?.hoverAt(context) ?? Promise.resolve(null),
            diagnosticsHover: (context) => instance.lsp?.diagnosticsHoverAt(context) ?? null,
            triggerCharacters: (docId) => instance.lsp?.completionTriggerCharacters(docId) ?? [],
            signature: (context) => instance.lsp?.signatureHelpAt(context) ?? Promise.resolve(null),
            signatureTriggerCharacters: (docId) =>
              instance.lsp?.signatureTriggerCharacters(docId) ?? [],
            signatureRetriggerCharacters: (docId) =>
              instance.lsp?.signatureRetriggerCharacters(docId) ?? [],
          }),
          Layer.succeed(LanguageServers, languageServers),
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
            openFolder: () => void instance.openFolderViaDialog(),
            find: () => instance.onFileFinderOpenRequested?.(),
          }),
          Layer.succeed(HostPaletteActions, {
            open: () => instance.onPaletteOpenRequested?.(),
          }),
          Layer.succeed(HostThemeActions, {
            openSelector: () => instance.onThemeSelectorOpenRequested?.(),
          }),
          Layer.succeed(HostLspActions, {
            goToDefinition: () => void instance.goToDefinition(),
            formatDocument: () => void instance.formatActiveDocument(),
            renameSymbol: () => instance.startRename(),
            toggleInlayHints: () => instance.toggleInlayHints(),
            toggleLsp: () => instance.toggleLsp(),
          }),
          Layer.succeed(HostViewActions, {
            splitRight: () => instance.splitFocused("row"),
            splitDown: () => instance.splitFocused("column"),
            closeTab: () => instance.closeFocusedTab(),
            nextTab: () => instance.cycleTab(1),
            previousTab: () => instance.cycleTab(-1),
            toggleTabs: () => instance.toggleTabs(),
          }),
        );
        yield* runtime
          .activateGraph([
            toPackageDescriptor(themeDefaults, services),
            toPackageDescriptor(editorDefaults, services),
            toPackageDescriptor(lineNumbers, services),
            toPackageDescriptor(filesPackage, services),
            toPackageDescriptor(palettePackage, services),
            toPackageDescriptor(themeCommands, services),
            toPackageDescriptor(viewPackage, services),
            toPackageDescriptor(lspDefaults, services),
            toPackageDescriptor(lspActions, services),
            toPackageDescriptor(lspIntel, services),
            toPackageDescriptor(wordCompletions, services),
            toPackageDescriptor(intelActions, services),
            toPackageDescriptor(vcsDefaults, services),
            toPackageDescriptor(fileTreePackage, services),
            toPackageDescriptor(terminalPackage, services),
          ])
          .pipe(Effect.orDie);

        const appRuntime = new AppRuntime(scope, workspace, workbench, commands, keymaps, themes);
        appRuntime.panelRenderers = panelRenderers;
        appRuntime.intel = {
          completions: completionProviders,
          hovers: hoverProviders,
          signatures: signatureProviders,
        };
        const processPort = createProcessPort();
        if (processPort !== undefined) {
          appRuntime.lsp = new LspManager(processPort, fileSystem, languageServers);
          appRuntime.lsp.onDiagnostics = (docId) => appRuntime.onDiagnosticsChanged?.(docId);
          appRuntime.vcs = new VcsManager(vcsProviders, fileSystem);
        }
        return appRuntime;
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
    instance.startThemeMirror();
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

  themeSnapshot(): ResolvedTheme {
    return Effect.runSync(this.themes.active);
  }

  subscribeTheme(listener: () => void): () => void {
    this.themeListeners.add(listener);
    return () => void this.themeListeners.delete(listener);
  }

  private startThemeMirror(): void {
    const publish = (theme: ResolvedTheme) => {
      applyThemeToDocument(theme);
      for (const listener of this.themeListeners) listener();
    };
    publish(Effect.runSync(this.themes.active));
    this.themeFiber = Effect.runFork(
      Stream.runForEach(this.themes.changes, (theme) => Effect.sync(() => publish(theme))),
    );
    this.systemAppearanceMedia = window.matchMedia("(prefers-color-scheme: light)");
    this.systemAppearanceListener = () => {
      const appearance = this.systemAppearanceMedia?.matches ? "light" : "dark";
      Effect.runFork(this.themes.setSystemAppearance(appearance).pipe(Effect.ignore));
    };
    this.systemAppearanceMedia.addEventListener("change", this.systemAppearanceListener);
    this.systemAppearanceListener();
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
        panelIds: Effect.runSync(this.workbench.panels).map((panel) => panel.id),
      };
      for (const listener of this.workbenchListeners) listener();
    };
    // Closing a panel mutates only the panel REGISTRY (no arrangement/focus
    // change), so it must force a republish — otherwise `panelIds` goes stale
    // and the chrome keeps a closed panel's portal mounted (leaking its PTY).
    this.republishWorkbench = publish;
    publish();
    this.mirrorFibers = [
      Effect.runFork(
        Stream.runForEach(this.workbench.arrangementChanges, () => Effect.sync(publish)),
      ),
      Effect.runFork(
        Stream.runForEach(this.workbench.focusChanges, (focused) =>
          Effect.sync(() => {
            // Remember the last editor group focused; tool panels (file tree,
            // terminal) don't count, so they never become the open target.
            if (Option.isSome(focused) && this.stackHostsEditors(focused.value)) {
              this.lastEditorSlot = focused.value;
            }
            publish();
          }),
        ),
      ),
    ];
  }

  /** Does this slot's stack currently host at least one editor pane? */
  private stackHostsEditors(slotId: SlotId): boolean {
    const slot = findSlot(this.workbenchSnapshot.arrangement, slotId);
    return slot?.kind === "stack" && slot.stack.panels.some((id) => this.panes.has(id));
  }

  // ------------------------------------------------------------- gestures

  focusSlot(slotId: SlotId): void {
    const current = this.workbenchSnapshot.focusedSlot;
    if (current === slotId) return;
    Effect.runSync(this.workbench.setFocus(slotId).pipe(Effect.ignore));
  }

  /** A surface registers how to focus itself; returns an unregister disposer. */
  registerPanelFocuser(panelId: PanelId, focus: () => void): () => void {
    this.panelFocusers.set(panelId, focus);
    return () => {
      if (this.panelFocusers.get(panelId) === focus) this.panelFocusers.delete(panelId);
    };
  }

  /**
   * Land the keyboard in the focused pane's surface. The single focus authority
   * delegates to the panel's own focuser, so every focus change — tab switch,
   * ⌘J toggle, split, dialog dismissal — puts keys in the right surface without
   * guessing at DOM state. A panel whose content mounts async (the terminal)
   * calls its own focuser once ready.
   */
  focusActivePanel(): void {
    const focused = Effect.runSync(this.workbench.focusedPanel);
    if (Option.isNone(focused)) return;
    this.panelFocusers.get(focused.value.id)?.();
  }

  activateTab(slotId: SlotId, panelId: PanelId): void {
    Effect.runSync(
      this.workbench.updateArrangement((a) => activateTab(a, slotId, panelId)).pipe(Effect.orDie),
    );
    this.focusSlot(slotId);
  }

  /** Close a tab: the panel dies; the document follows when unreferenced. */
  closeTab(panelId: PanelId): void {
    this.releaseDock(panelId);
    const pane = this.panes.get(panelId);
    if (pane === undefined) {
      // Custom panel (terminal, file tree): closing its scope disposes the
      // renderer (the pool unmounts it once it leaves the live panel set).
      Effect.runSync(this.workbench.closePanel(panelId));
      // closePanel mutates only the registry; force the snapshot to refresh so
      // the chrome prunes the panel's portal and unmounts it (its dispose kills
      // the PTY). Without this, panelIds stays stale and the content leaks.
      this.republishWorkbench();
      return;
    }
    this.panes.delete(panelId);
    Effect.runSync(this.workbench.closePanel(panelId));
    this.republishWorkbench();
    Effect.runSync(closeViewScope(pane.view));
    const documentStillShown = [...this.panes.values()].some(
      (other) => other.document.id === pane.document.id,
    );
    if (!documentStillShown) {
      this.lsp?.detach(String(pane.document.id));
      this.vcs?.detach(String(pane.document.id));
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

  /** Live panel by id (editor panes AND custom panels). */
  panelById(panelId: PanelId): Panel | undefined {
    return Option.getOrUndefined(Effect.runSync(this.workbench.panel(panelId)));
  }

  /**
   * Toggle a singleton panel of `kind` (terminal, file tree). The panel is
   * created once and shown/hidden thereafter (Zed's dock model): toggling a
   * visible panel un-places it but keeps it alive, so its content (a terminal's
   * PTY + screen) survives; toggling a hidden one re-docks it. Disposal is
   * closing its tab, never toggling.
   */
  togglePanel(
    kind: SurfaceKind,
    options?: { readonly dock?: "left" | "right" | "top" | "bottom"; readonly ratio?: number },
  ): void {
    const dock = options?.dock ?? "left";
    const ratio = options?.ratio ?? 0.22;
    const trackedId = this.dockedPanels.get(kind);
    const tracked = trackedId !== undefined ? this.panelById(trackedId) : undefined;
    if (tracked !== undefined) {
      const arrangement = Effect.runSync(this.workbench.arrangement);
      if (slotOfPanel(arrangement, tracked.id) !== undefined) {
        // Visible → hide it, kept alive (content survives in the panel pool).
        Effect.runSync(
          this.workbench.updateArrangement((a) => removePanel(a, tracked.id)).pipe(Effect.orDie),
        );
      } else {
        // Hidden → re-dock and focus.
        Effect.runSync(
          this.workbench.updateArrangement((a) => dockEdge(a, tracked.id, dock, { ratio })).pipe(Effect.orDie),
        );
        this.focusSlotOfPanel(tracked.id);
      }
      return;
    }
    // No dock panel (never created, or the previous one was moved/closed):
    // create a fresh one and remember it as this kind's dock.
    const panel = Effect.runSync(this.workbench.createPanel({ kind }));
    this.dockedPanels.set(kind, panel.id);
    Effect.runSync(
      this.workbench.updateArrangement((a) => dockEdge(a, panel.id, dock, { ratio })).pipe(Effect.orDie),
    );
    this.focusSlotOfPanel(panel.id);
  }

  /** Drop the dock association for a panel that was moved or closed, so the
   *  toggle stops managing it and the next toggle creates a fresh dock panel. */
  private releaseDock(panelId: PanelId): void {
    const panel = this.panelById(panelId);
    if (panel !== undefined && this.dockedPanels.get(panel.kind) === panelId) {
      this.dockedPanels.delete(panel.kind);
    }
  }

  /** Show/hide the focused stack's tab strip (e.g. a single-panel file tree).
   * Tabs can still be cycled by keyboard while hidden, so it's safe on any
   * stack. */
  toggleTabs(): void {
    const focusedSlot = this.workbenchSnapshot.focusedSlot;
    if (focusedSlot === null) return;
    Effect.runSync(
      this.workbench.updateArrangement((a) => toggleStackTabs(a, focusedSlot)).pipe(Effect.orDie),
    );
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
    this.releaseDock(panelId);
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
    this.releaseDock(panelId);
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

  focusSlotOfPanel(panelId: PanelId): void {
    const arrangement = Effect.runSync(this.workbench.arrangement);
    const slot = slotOfPanel(arrangement, panelId);
    if (slot !== undefined) this.focusSlot(slot);
  }

  // ---------------------------------------------------------------- syntax

  highlightsFor(docId: unknown): PaneHighlights | undefined {
    return this.highlightsByDoc.get(String(docId));
  }

  /** Diagnostic counts for a document (null without LSP or attachment). */
  diagnosticsFor(docId: unknown): DiagnosticCounts | null {
    return this.lsp?.counts(String(docId)) ?? null;
  }

  async goToDefinition(): Promise<void> {
    const pane = this.focusedPane();
    if (pane === undefined || this.lsp === undefined) return;
    const head = pane.snapshot().selection.primary.head;
    const locations = await this.lsp.definition(String(pane.document.id), head);
    const target = locations[0];
    if (target === undefined) return;
    const path = uriToPath(target.uri);
    if (path === null) return;
    await this.jumpToLocation(path, target.range.start);
  }

  /** Open (or focus) a file and place the caret at an LSP position. */
  async jumpToLocation(path: string, position: lsp.Position): Promise<void> {
    const current = this.focusedPane();
    const samePath = current !== undefined && current.fileInfo()?.path === path;
    if (!samePath) await this.openPath(path);
    const pane = this.focusedPane();
    if (pane === undefined) return;
    const text = pane.snapshot().text;
    const line = Math.min(Math.max(0, position.line), text.lineCount - 1);
    const range = text.lineRange(line);
    const offset = Math.min(range.from + Math.max(0, position.character), range.to);
    pane.setSelection(Selection.point(offset));
  }

  async formatActiveDocument(): Promise<void> {
    const pane = this.focusedPane();
    if (pane === undefined || this.lsp === undefined) return;
    await this.lsp.format(String(pane.document.id));
  }

  /** Phase 1 of rename: capture the caret target, ask the host for a name. */
  startRename(): void {
    const pane = this.focusedPane();
    if (pane === undefined || this.lsp === undefined) return;
    const snapshot = pane.snapshot();
    const head = snapshot.selection.primary.head;
    this.pendingRename = { docId: String(pane.document.id), offset: head };
    // Word under the caret as the prompt's initial value.
    const lineRange = snapshot.text.lineRangeAt(head);
    const lineText = snapshot.text.sliceString(lineRange.from, lineRange.to);
    const column = head - lineRange.from;
    let start = column;
    let end = column;
    while (start > 0 && /\w/.test(lineText[start - 1]!)) start--;
    while (end < lineText.length && /\w/.test(lineText[end]!)) end++;
    this.onRenameRequested?.(lineText.slice(start, end));
  }

  /** Phase 2 of rename: the host collected the new name. */
  async performRename(newName: string): Promise<void> {
    const pending = this.pendingRename;
    this.pendingRename = null;
    if (pending === null || this.lsp === undefined || newName.length === 0) return;
    await this.lsp.rename(pending.docId, pending.offset, newName);
  }

  toggleInlayHints(): void {
    this.lsp?.toggleInlayHints();
  }

  /**
   * Stop all language servers, or restart them and re-attach every open
   * document. The manager only owns the server side; re-attaching is the
   * runtime's job since it holds the open panes (attach dedupes by doc id).
   */
  toggleLsp(): void {
    if (this.lsp === undefined) return;
    if (this.lsp.isEnabled()) {
      this.lsp.stop();
    } else {
      this.lsp.start();
      for (const pane of this.panes.values()) {
        const path = pane.fileInfo()?.path;
        if (path !== undefined) void this.lsp.attach(pane.document, path);
      }
    }
    // Diagnostics counts vanished/returned — refresh the status surfaces.
    for (const pane of this.panes.values()) {
      this.onDiagnosticsChanged?.(String(pane.document.id));
    }
  }

  lspEnabled(): boolean {
    return this.lsp?.isEnabled() ?? false;
  }

  /** Ask the host for a string (new-file/new-folder names, etc.). */
  requestPrompt(options: { initial?: string; placeholder?: string }): Promise<string | null> {
    return this.onPrompt?.(options) ?? Promise.resolve(null);
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
    void this.lsp?.attach(document, path);
    this.vcs?.attach(document, path);

    const { arrangement, focusedSlot } = this.workbenchSnapshot;
    const hostsEditors = (s: Stack) => s.panels.some((id) => this.panes.has(id));
    const editorStacks = collectSlots(arrangement).flatMap((s) =>
      s.kind === "stack" && hostsEditors(s.stack) ? [s.stack] : [],
    );
    // Target the ACTIVE EDITOR GROUP, not the raw focused slot: the last editor
    // the user worked in (tool panels like the file tree don't count). Fall
    // back to the focused stack if it's an editor (first open in a fresh group),
    // then any editor group, then none → a new group is docked below.
    const focusedSlotStack =
      focusedSlot === null ? undefined : findSlot(arrangement, focusedSlot);
    const focusedEditor =
      focusedSlotStack?.kind === "stack" && hostsEditors(focusedSlotStack.stack)
        ? focusedSlotStack.stack
        : undefined;
    const stack: Stack | undefined =
      editorStacks.find((s) => s.id === this.lastEditorSlot) ??
      focusedEditor ??
      editorStacks[0];

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
          // No editor stack anywhere (only custom panels): dock one.
          return dockEdge(a, pane.panelId, "right", { ratio: 0.78 });
        })
        .pipe(Effect.orDie),
    );
    this.focusSlotOfPanel(pane.panelId);
  }

  /** The project root: the file tree's root and the VCS detect target. */
  workspaceRoot: string | null = null;
  private readonly workspaceRootListeners = new Set<(root: string | null) => void>();

  subscribeWorkspaceRoot(listener: (root: string | null) => void): () => void {
    this.workspaceRootListeners.add(listener);
    return () => void this.workspaceRootListeners.delete(listener);
  }

  /** The platform file system, when one exists. */
  fileSystem(): FileSystemPort.Service | undefined {
    return Option.getOrUndefined(Context.getOption(this.workspace.context, FileSystemPort));
  }

  /** The platform PTY transport for the terminal panel (Electron / dev echo). */
  terminalPort(): TerminalPort | undefined {
    return (this.terminal ??= createTerminalPort());
  }

  /** The native fff-backed finder, when the desktop bridge is present. */
  fileFinderPort(): FileFinderPort | undefined {
    if (this.fileFinder === undefined) this.fileFinder = createFileFinderPort() ?? null;
    return this.fileFinder ?? undefined;
  }

  openFolder(root: string): void {
    this.workspaceRoot = root.replace(/\/+$/, "") || "/";
    void this.vcs?.setRoot(this.workspaceRoot);
    for (const listener of this.workspaceRootListeners) listener(this.workspaceRoot);
  }

  async openFolderViaDialog(): Promise<void> {
    const port = Context.getOption(this.workspace.context, FileSystemPort);
    if (Option.isNone(port) || port.value.pickFolder === undefined) return;
    const picked = await Effect.runPromise(port.value.pickFolder());
    if (Option.isSome(picked)) this.openFolder(picked.value);
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
    this.lsp?.documentSaved(String(pane.document.id));
    this.vcs?.documentSaved();
    this.onFileStateChanged?.();
  }

  // ------------------------------------------------------------- keyboard

  /**
   * The app's SINGLE keyboard entry point. ONE document-level listener routes
   * every keystroke here, whatever surface is focused — keymaps are the app's,
   * not the editor's. Cooperative leaves (the owned editor, @pierre/trees)
   * claim the keys they use and let the rest bubble; this dispatches global +
   * focused-surface bindings against whatever reaches it. Editable surfaces
   * (the command palette, the rename prompt, the tree's search box, the
   * TERMINAL) keep their own typing — but GLOBAL bindings (the palette, toggle
   * terminal/tree) still fire over them, so an editable surface is never a
   * keyboard trap. Surface-scoped bindings (editor save, etc.) do NOT leak in.
   */
  dispatchKeyboardEvent(event: KeyboardEvent): void {
    const editable = isEditableTarget(event.target);

    // Native copy of a selection inside a float (hover-card text): the keymap's
    // copy would grab the EDITOR selection instead.
    if (!editable && (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "c") {
      const selection = window.getSelection();
      const node = selection?.anchorNode ?? null;
      const element = node instanceof Element ? node : node?.parentElement ?? null;
      if (selection !== null && !selection.isCollapsed && element?.closest(".editor-float") != null) {
        return;
      }
    }

    // The focused editor's completion/snippet interceptor runs before keymaps
    // (never over an editable surface — completion only lives in the editor).
    if (!editable && this.focusedPane()?.keyInterceptor?.(event) === true) {
      event.preventDefault();
      this.onPendingChords?.(this.pendingChords());
      return;
    }

    const outcome = this.resolveKeyboardEvent(event, editable);
    if (outcome !== "passthrough") event.preventDefault();
    this.onPendingChords?.(this.pendingChords());
  }

  /**
   * Resolves a key against the keymaps (scope-aware via the focused panel).
   * handled/pending mean the caller should preventDefault; passthrough means
   * the key is not ours (browser shortcuts, IME, a leaf surface's own key).
   * `editable`: the key originated in an input/terminal that owns its typing,
   * so only GLOBAL bindings may claim it — no surface bindings, no chord
   * buffering, no text insertion.
   */
  private resolveKeyboardEvent(event: KeyboardEventLike, editable: boolean): KeyOutcome {
    const chord = chordFromKeyboardEvent(event);
    if (chord === undefined) return "passthrough";

    const resolution = Effect.runSync(this.keymaps.handleKey(chord));
    switch (resolution._tag) {
      case "match":
        if (editable && resolution.binding.scope._tag !== "global") return "passthrough";
        Effect.runSync(
          this.commands
            .execute(resolution.binding.command, resolution.binding.args)
            .pipe(Effect.orDie),
        );
        return "handled";
      case "pending":
        // Don't hold a chord prefix over an editable surface's own typing.
        return editable ? "passthrough" : "pending";
      case "none":
        if (
          !editable &&
          isTextInputChord(chord) &&
          event.key.length === 1 &&
          this.focusedPane() !== undefined
        ) {
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

  /**
   * Run a key against ONLY the global keymap, returning whether it matched.
   * For surfaces that swallow their own keydowns before they can bubble to the
   * document dispatcher (the terminal: ghostty consumes everything) — they ask
   * this so app-global shortcuts (the palette, toggle terminal) still work
   * without the surface becoming a keyboard trap. Non-global keys are left for
   * the surface.
   */
  handleGlobalKey(event: KeyboardEventLike): boolean {
    const chord = chordFromKeyboardEvent(event);
    if (chord === undefined) return false;
    const resolution = Effect.runSync(this.keymaps.handleKey(chord));
    if (resolution._tag === "match" && resolution.binding.scope._tag === "global") {
      Effect.runSync(
        this.commands
          .execute(resolution.binding.command, resolution.binding.args)
          .pipe(Effect.orDie),
      );
      this.onPendingChords?.(this.pendingChords());
      return true;
    }
    this.onPendingChords?.(this.pendingChords());
    return false;
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
    this.lsp?.dispose();
    this.vcs?.dispose();
    for (const highlights of this.highlightsByDoc.values()) highlights.close();
    this.highlightsByDoc.clear();
    for (const fiber of this.mirrorFibers) Effect.runFork(Fiber.interrupt(fiber));
    if (this.themeFiber !== undefined) Effect.runFork(Fiber.interrupt(this.themeFiber));
    if (this.systemAppearanceMedia !== undefined && this.systemAppearanceListener !== undefined) {
      this.systemAppearanceMedia.removeEventListener("change", this.systemAppearanceListener);
    }
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
