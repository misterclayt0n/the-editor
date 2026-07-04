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

export interface KeyBindingDefinition {
  /** Key sequence, e.g. "ctrl+s" or "ctrl+w h". */
  readonly keys: string;
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
}
