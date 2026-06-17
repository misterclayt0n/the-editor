/**
 * The shell: process-global state shared by every workspace session.
 *
 * A workspace is one AppRuntime — its own documents, layout, panes, LSP, VCS,
 * and terminals, independently disposable. The theme is the exception: it is
 * NOT per-workspace. It paints the one window's chrome and writes
 * process-global state — the `:root` CSS variables and the `activeTheme`
 * singleton every editor renderer reads (see theme/dom.ts). If each mounted
 * session drove that, hidden-but-alive sessions would stomp the same DOM and
 * singleton, and the second session activating the bundled theme family would
 * hit ThemeDuplicateError.
 *
 * So the shell owns ONE Themes service, activates the theme family exactly
 * once, and is the sole author of the DOM theme + OS-appearance tracking.
 * Sessions receive this service as a consumer: their package graphs read it
 * (theme.select etc.) and their renderers repaint off their own subscription
 * to `themes.changes`, but they never touch the DOM or register a family.
 */

import { Effect, Fiber, Layer, Stream } from "effect";
import { makePackageRuntime, type PackageRuntime } from "@the-editor/kernel";
import { toPackageDescriptor } from "@the-editor/extensions";
import { makeThemes, Themes, themeDefaults, type ResolvedTheme } from "@the-editor/theme";
import { applyThemeToDocument } from "./theme/dom";

export class Shell {
  private themeFiber: Fiber.RuntimeFiber<unknown, unknown> | undefined;
  private systemAppearanceMedia: MediaQueryList | undefined;
  private systemAppearanceListener: (() => void) | undefined;
  private readonly themeListeners = new Set<() => void>();

  private constructor(
    private readonly packages: PackageRuntime,
    /** The shared theme service injected into every session's package graph. */
    readonly themes: Themes.Service,
  ) {}

  static boot(): Shell {
    const instance = Effect.runSync(
      Effect.gen(function* () {
        const packages = yield* makePackageRuntime;
        const themes = yield* makeThemes();
        // Register the bundled theme family ONCE, here — not per session.
        yield* packages
          .activateGraph([toPackageDescriptor(themeDefaults, Layer.succeed(Themes, themes))])
          .pipe(Effect.orDie);
        return new Shell(packages, themes);
      }),
    );
    instance.startThemeDriver();
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

  /** Re-render hook for chrome that reflects the theme (the App status bar). */
  subscribeTheme(listener: () => void): () => void {
    this.themeListeners.add(listener);
    return () => void this.themeListeners.delete(listener);
  }

  dispose(): void {
    if (this.themeFiber !== undefined) Effect.runFork(Fiber.interrupt(this.themeFiber));
    if (this.systemAppearanceMedia !== undefined && this.systemAppearanceListener !== undefined) {
      this.systemAppearanceMedia.removeEventListener("change", this.systemAppearanceListener);
    }
    this.themeListeners.clear();
    Effect.runFork(this.packages.disposePackage(themeDefaults.id).pipe(Effect.ignore));
  }
}
