import type { IdentifiedContribution } from "@the-editor/kernel";
import type { SurfaceScope } from "@the-editor/workbench";
import type { Chord } from "./chord.js";

/**
 * A keybinding maps a key sequence to a command within a scope. Bindings
 * attach to scopes (Emacs keymap model), not to predicate expressions:
 * locality comes from the workbench focus, never from a when-clause DSL.
 */
export type KeyBindingFactValue = string | number | boolean;
export type KeyBindingFacts = Readonly<Record<string, KeyBindingFactValue | undefined>>;

export type KeybindingPlatform = "mac" | "win" | "linux";
export type PlatformKeyOverride = string | { readonly primary: string };
export type PlatformKeys = {
  /** Fallback key sequence when no current-platform override is present. */
  readonly primary: string;
  readonly mac?: PlatformKeyOverride;
  readonly win?: PlatformKeyOverride;
  readonly linux?: PlatformKeyOverride;
};
export type KeyBindingKeys = string | PlatformKeys;

export interface KeyBindingDefinition {
  /** Key sequence, e.g. "ctrl+s" or "ctrl+w h", or a platform-aware map. */
  readonly keys: KeyBindingKeys;
  readonly command: string;
  readonly args?: unknown;
  /** Defaults to global. */
  readonly scope?: SurfaceScope;
  /**
   * Exact-match facts that must be present on the focused input context for
   * this binding to participate. Facts are a FILTER ONLY — they never increase
   * precedence. Among surviving bindings, scope specificity and registration
   * order keep their existing meaning.
   */
  readonly when?: KeyBindingFacts;
  readonly description?: string;
}

export interface KeyBindingContribution extends IdentifiedContribution {
  readonly chords: ReadonlyArray<Chord>;
  readonly command: string;
  readonly args?: unknown;
  readonly scope: SurfaceScope;
  readonly when?: KeyBindingFacts;
  readonly description?: string;
  /** Resolved source keys used to create this contribution (for introspection). */
  readonly sourceKeys?: KeyBindingKeys;
}
