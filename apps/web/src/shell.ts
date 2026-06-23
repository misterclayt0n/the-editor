/**
 * The shell: process-global state shared by every workspace session.
 *
 * A workspace is one AppRuntime — its own documents, layout, panes, LSP, VCS,
 * and terminals, independently disposable. Theme and fonts are the exceptions:
 * they are NOT per-workspace. They paint the one window's chrome, `:root`
 * CSS variables, DOM FontFace set, and the `activeTheme` singleton every
 * editor renderer reads (see theme/dom.ts). If each mounted session drove
 * that, hidden-but-alive sessions would stomp the same DOM; the second
 * session activating bundled defaults would also hit duplicate contribution
 * errors.
 *
 * So the shell owns ONE Themes service and ONE Fonts service, activates their
 * defaults exactly once, and is the sole author of DOM theme/font state.
 * Sessions receive these services as consumers: package graphs read them and
 * renderers repaint off their own subscriptions, but sessions never touch
 * global DOM appearance state.
 */

import { Effect, Exit, Fiber, Layer, Scope, Stream } from "effect";
import { makePackageRuntime, type PackageRuntime } from "@the-editor/kernel";
import { toPackageDescriptor } from "@the-editor/extensions";
import { makeThemes, Themes, themeDefaults, type ResolvedTheme } from "@the-editor/theme";
import { Fonts, fontDefaults, makeFonts, type ResolvedFontSystem } from "@the-editor/fonts";
import {
  dockEdge,
  makeWorkbench,
  removePanel,
  slotOfPanel,
  SurfaceKind,
  toggleStackTabs,
  type PanelId,
  type Workbench,
} from "@the-editor/workbench";
import { applyThemeToDocument } from "./theme/dom";
import { createFontDomDriver } from "./fonts/dom";
import { createFontFileLoader } from "./platform/fonts";
import { makePanelRenderers, type PanelRenderer } from "./workbench/panelRenderers";
import { ChromeSlot, ShellWorkbenchHost } from "./workbench/shellHost";

/** The shell chrome's two panel kinds: the session sidebar and the viewport
 *  that hosts the active session's workbench. */
const SessionSidebarSurface = SurfaceKind("the-editor/session-sidebar");
const SessionViewportSurface = SurfaceKind("the-editor/session-viewport");

/** A chrome panel renderer that simply hands its mount element to a ChromeSlot;
 *  App portals the real React (sidebar / session views) into that element. */
const chromeRenderer = (title: string, slot: ChromeSlot, className: string): PanelRenderer => ({
  title: () => title,
  chrome: { className },
  mount: (element) => {
    slot.set(element);
    return { dispose: () => slot.set(null) };
  },
});

export class Shell {
  private themeFiber: Fiber.RuntimeFiber<unknown, unknown> | undefined;
  private fontFiber: Fiber.RuntimeFiber<unknown, unknown> | undefined;
  private readonly fontDriver = createFontDomDriver(createFontFileLoader());
  private systemAppearanceMedia: MediaQueryList | undefined;
  private systemAppearanceListener: (() => void) | undefined;
  private readonly themeListeners = new Set<() => void>();
  private readonly fontListeners = new Set<() => void>();

  /** Set by App: focuses the active session's surface, so the chrome host's
   *  focusActivePanel delegates here and a sidebar click keeps the editor hot. */
  onFocusActiveSession?: () => void;

  /** Remembered sidebar width fraction, so re-showing restores the user's size. */
  private sidebarRatio = 0.18;

  private constructor(
    private readonly packages: PackageRuntime,
    /** The shared theme service injected into every session's package graph. */
    readonly themes: Themes.Service,
    /** The shared font service injected into every session's package graph. */
    readonly fonts: Fonts.Service,
    /** The outer chrome workbench host: sidebar + viewport panels. */
    readonly chromeHost: ShellWorkbenchHost,
    /** Mount points the chrome renderers publish; App portals content into them. */
    readonly sidebarSlot: ChromeSlot,
    readonly viewportSlot: ChromeSlot,
    private readonly chromeScope: Scope.CloseableScope,
    /** The chrome workbench service, for sidebar show/hide surgery. */
    private readonly chromeWorkbench: Workbench.Service,
    /** The sidebar panel id, toggled in/out of the chrome arrangement. */
    private readonly sidebarPanelId: PanelId,
  ) {}

  /** Whether the session sidebar is currently placed in the chrome layout. */
  isSidebarVisible(): boolean {
    return (
      slotOfPanel(Effect.runSync(this.chromeWorkbench.arrangement), this.sidebarPanelId) !== undefined
    );
  }

  /** Show/hide the session sidebar (Zed dock model): hiding remembers its width,
   *  showing restores it. The panel stays alive either way. */
  toggleSidebar(): void {
    Effect.runSync(
      this.chromeWorkbench
        .updateArrangement((a) => {
          const slot = slotOfPanel(a, this.sidebarPanelId);
          if (slot !== undefined) {
            const tiles = a.tiles;
            if (tiles !== null && tiles.kind === "split") {
              const total = tiles.children.reduce((sum, child) => sum + child.weight, 0);
              const child = tiles.children.find(
                (c) => c.tile.kind === "stack" && c.tile.id === slot,
              );
              if (child !== undefined && total > 0) this.sidebarRatio = child.weight / total;
            }
            return removePanel(a, this.sidebarPanelId);
          }
          let next = dockEdge(a, this.sidebarPanelId, "left", { ratio: this.sidebarRatio });
          const reSlot = slotOfPanel(next, this.sidebarPanelId);
          if (reSlot !== undefined) next = toggleStackTabs(next, reSlot);
          return next;
        })
        .pipe(Effect.orDie),
    );
  }

  static boot(): Shell {
    let instance!: Shell;
    instance = Effect.runSync(
      Effect.gen(function* () {
        const packages = yield* makePackageRuntime;
        const themes = yield* makeThemes();
        const fonts = yield* makeFonts;
        // Register bundled process-global appearance defaults ONCE, here — not per session.
        yield* packages
          .activateGraph([
            toPackageDescriptor(themeDefaults, Layer.succeed(Themes, themes)),
            toPackageDescriptor(fontDefaults, Layer.succeed(Fonts, fonts)),
          ])
          .pipe(Effect.orDie);

        // The chrome workbench: a session-less Workbench whose two panels are
        // the session sidebar and the session viewport. Process-global, scoped
        // for the shell's lifetime — like theme and fonts.
        const chromeScope = yield* Scope.make();
        const chromeWorkbench = yield* Scope.extend(makeWorkbench(), chromeScope);
        const chromeRenderers = makePanelRenderers();
        const sidebarSlot = new ChromeSlot();
        const viewportSlot = new ChromeSlot();
        yield* Scope.extend(
          chromeRenderers.register(
            SessionSidebarSurface,
            chromeRenderer("Sessions", sidebarSlot, "wb-pane--sidebar"),
          ),
          chromeScope,
        );
        yield* Scope.extend(
          chromeRenderers.register(
            SessionViewportSurface,
            chromeRenderer("Workspace", viewportSlot, "wb-pane--viewport"),
          ),
          chromeScope,
        );
        const sidebar = yield* chromeWorkbench.createPanel({ kind: SessionSidebarSurface });
        const viewport = yield* chromeWorkbench.createPanel({ kind: SessionViewportSurface });
        yield* chromeWorkbench
          .updateArrangement((a) => {
            let next = dockEdge(a, viewport.id, "left");
            next = dockEdge(next, sidebar.id, "left", { ratio: 0.18 });
            const sidebarSlotId = slotOfPanel(next, sidebar.id);
            const viewportSlotId = slotOfPanel(next, viewport.id);
            if (sidebarSlotId !== undefined) next = toggleStackTabs(next, sidebarSlotId);
            if (viewportSlotId !== undefined) next = toggleStackTabs(next, viewportSlotId);
            return next;
          })
          .pipe(Effect.orDie);
        const chromeHost = new ShellWorkbenchHost(
          chromeWorkbench,
          chromeRenderers,
          () => instance.onFocusActiveSession?.(),
        );

        return new Shell(
          packages,
          themes,
          fonts,
          chromeHost,
          sidebarSlot,
          viewportSlot,
          chromeScope,
          chromeWorkbench,
          sidebar.id,
        );
      }),
    );
    instance.startThemeDriver();
    instance.startFontDriver();
    return instance;
  }

  /**
   * Sole author of the window theme: applies the active theme and every change
   * to the DOM (CSS vars + the `activeTheme` singleton renderers read) and
   * tracks the OS light/dark preference. Sessions repaint independently off
   * their own `themes.changes` subscription; only the shell writes the DOM.
   */
  private startThemeDriver(): void {
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

  /**
   * Sole author of window font faces and CSS font variables. Sessions repaint
   * from `fonts.changes`; they do not register DOM FontFace objects themselves.
   */
  private startFontDriver(): void {
    const publish = (fonts: ResolvedFontSystem) => {
      this.fontDriver.apply(fonts);
      for (const listener of this.fontListeners) listener();
    };
    publish(Effect.runSync(this.fonts.resolved));
    this.fontFiber = Effect.runFork(
      Stream.runForEach(this.fonts.changes, (fonts) => Effect.sync(() => publish(fonts))),
    );
  }

  subscribeFont(listener: () => void): () => void {
    this.fontListeners.add(listener);
    return () => void this.fontListeners.delete(listener);
  }

  /** Re-render hook for chrome that reflects the theme (the App status bar). */
  subscribeTheme(listener: () => void): () => void {
    this.themeListeners.add(listener);
    return () => void this.themeListeners.delete(listener);
  }

  dispose(): void {
    if (this.themeFiber !== undefined) Effect.runFork(Fiber.interrupt(this.themeFiber));
    if (this.fontFiber !== undefined) Effect.runFork(Fiber.interrupt(this.fontFiber));
    if (this.systemAppearanceMedia !== undefined && this.systemAppearanceListener !== undefined) {
      this.systemAppearanceMedia.removeEventListener("change", this.systemAppearanceListener);
    }
    this.themeListeners.clear();
    this.fontListeners.clear();
    this.fontDriver.dispose();
    this.chromeHost.dispose();
    Effect.runSync(Scope.close(this.chromeScope, Exit.void));
    Effect.runFork(this.packages.disposePackage(themeDefaults.id).pipe(Effect.ignore));
    Effect.runFork(this.packages.disposePackage(fontDefaults.id).pipe(Effect.ignore));
  }
}
