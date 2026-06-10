import type { IdentifiedContribution } from "@the-editor/kernel";
import type { SurfaceScope } from "@the-editor/workbench";
import type { Chord } from "./chord.js";

/**
 * A keybinding maps a key sequence to a command within a scope. Bindings
 * attach to scopes (Emacs keymap model), not to predicate expressions:
 * locality comes from the workbench focus, never from a when-clause DSL.
 */
export interface KeyBindingDefinition {
  /** Key sequence, e.g. "ctrl+s" or "ctrl+w h". */
  readonly keys: string;
  readonly command: string;
  readonly args?: unknown;
  /** Defaults to global. */
  readonly scope?: SurfaceScope;
  readonly description?: string;
}

export interface KeyBindingContribution extends IdentifiedContribution {
  readonly chords: ReadonlyArray<Chord>;
  readonly command: string;
  readonly args?: unknown;
  readonly scope: SurfaceScope;
  readonly description?: string;
}
