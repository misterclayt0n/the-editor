/**
 * User configuration as a package — the "manifest phase" of config-as-code.
 *
 * `~/.the-editor/config.ts` (and a trust-gated project `.the-editor/config.ts`)
 * `export default defineConfig({...})`. Unlike an extension — which ADDS
 * contributions and loads LAST — config is read FIRST, because it decides
 * *which default packages activate at all*. That's how "the-editor is just a
 * default set" gets its other half: a user drops or replaces a first-party
 * package by name instead of only stacking on top.
 *
 * The transform is package-granular and subtractive by omission: `disable` names
 * default package ids that should never load (their contributions never
 * register — not suppressed, never activated). "Replace X with mine" = disable X
 * + ship your own extension. There is deliberately no per-contribution
 * suppression or command advice: a contribution has one owner, and reload
 * disposes exactly what a package registered — letting a second party retract a
 * first party's contribution would make reload undefined and stop composing.
 */

import { Effect } from "effect";
import {
  evaluateModuleDefault,
  ExtensionLoadError,
  type LoadExtensionPackageOptions,
} from "./loadModule.js";

/** Terminal emulator engine for the terminal panel. */
export type TerminalEngineSetting = "xterm" | "ghostty";

export interface EditorConfig {
  /**
   * First-party default package ids to NOT activate. Only swappable defaults
   * can be dropped; naming a `required` package (kernel/editor foundation) is
   * ignored by the host with a notice. Order-independent.
   */
  readonly disable?: readonly string[];
  /** Terminal panel settings. */
  readonly terminal?: {
    /**
     * Emulator engine: `"xterm"` (xterm.js — the default) or `"ghostty"`
     * (Ghostty's VT emulator in WASM). Project config overrides global.
     * Applies to terminals opened after the config is read.
     */
    readonly engine?: TerminalEngineSetting;
  };
}

/** Identity + a typed authoring surface for `~/.the-editor/config.ts`. */
export function defineConfig(config: EditorConfig): EditorConfig {
  return config;
}

const isEditorConfig = (value: unknown): value is EditorConfig => {
  if (typeof value !== "object" || value === null) return false;
  const disable = (value as { disable?: unknown }).disable;
  if (disable !== undefined) {
    if (!Array.isArray(disable) || !disable.every((entry) => typeof entry === "string")) {
      return false;
    }
  }
  const terminal = (value as { terminal?: unknown }).terminal;
  if (terminal !== undefined) {
    if (typeof terminal !== "object" || terminal === null) return false;
    const engine = (terminal as { engine?: unknown }).engine;
    if (engine !== undefined && engine !== "xterm" && engine !== "ghostty") return false;
  }
  return true;
};

/**
 * Compile + evaluate one config source and validate its default export is an
 * `EditorConfig` (`export default defineConfig(...)`). Same failure discipline
 * as `loadExtensionPackage`: authored problems surface as `ExtensionLoadError`,
 * never a throw, so a bad config isolates instead of aborting boot.
 */
export function loadConfig(
  source: string,
  path: string,
  options: LoadExtensionPackageOptions,
): Effect.Effect<EditorConfig, ExtensionLoadError> {
  return Effect.gen(function* () {
    const candidate = yield* evaluateModuleDefault(source, path, options);
    if (!isEditorConfig(candidate)) {
      return yield* Effect.fail(
        new ExtensionLoadError(path, 'must "export default" the result of defineConfig(...)'),
      );
    }
    return candidate;
  });
}
