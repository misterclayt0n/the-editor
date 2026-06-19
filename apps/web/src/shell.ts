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

import { Effect, Fiber, Layer, Stream } from "effect";
import { makePackageRuntime, type PackageRuntime } from "@the-editor/kernel";
import { toPackageDescriptor } from "@the-editor/extensions";
import { makeThemes, Themes, themeDefaults, type ResolvedTheme } from "@the-editor/theme";
import { Fonts, fontDefaults, makeFonts, type ResolvedFontSystem } from "@the-editor/fonts";
import { applyThemeToDocument } from "./theme/dom";
import { createFontDomDriver } from "./fonts/dom";
import { createFontFileLoader } from "./platform/fonts";

export class Shell {
  private themeFiber: Fiber.RuntimeFiber<unknown, unknown> | undefined;
  private fontFiber: Fiber.RuntimeFiber<unknown, unknown> | undefined;
  private readonly fontDriver = createFontDomDriver(createFontFileLoader());
  private systemAppearanceMedia: MediaQueryList | undefined;
  private systemAppearanceListener: (() => void) | undefined;
  private readonly themeListeners = new Set<() => void>();
  private readonly fontListeners = new Set<() => void>();

  private constructor(
    private readonly packages: PackageRuntime,
    /** The shared theme service injected into every session's package graph. */
    readonly themes: Themes.Service,
    /** The shared font service injected into every session's package graph. */
    readonly fonts: Fonts.Service,
  ) {}

  static boot(): Shell {
    const instance = Effect.runSync(
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
        return new Shell(packages, themes, fonts);
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
    Effect.runFork(this.packages.disposePackage(themeDefaults.id).pipe(Effect.ignore));
    Effect.runFork(this.packages.disposePackage(fontDefaults.id).pipe(Effect.ignore));
  }
}
