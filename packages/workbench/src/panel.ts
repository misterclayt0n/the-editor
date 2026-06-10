import type { Context, Scope } from "effect";
import type { PanelId, SurfaceKind } from "./ids.js";

/**
 * One live instance of a surface. 100% generic: the workbench attaches no
 * meaning to `kind`, and `context` carries whatever services the surface owns
 * (an editor panel's context is its EditorScope context; a file tree's is its
 * tree state). Closing the panel closes `scope`, finalizing everything the
 * surface acquired.
 *
 * A panel is content + identity, not a place: it may sit in any number of
 * slots (or none — alive but undisplayed, like an Emacs background buffer).
 */
export interface Panel {
  readonly id: PanelId;
  readonly kind: SurfaceKind;
  readonly scope: Scope.CloseableScope;
  readonly context: Context.Context<never>;
}

export type CreatePanelInput = {
  readonly kind: SurfaceKind;
  readonly id?: string;
  readonly context?: Context.Context<never>;
};
