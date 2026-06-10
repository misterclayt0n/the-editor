import { Brand } from "effect";

/**
 * A *type* of surface — "editor", "file-tree", "terminal". Vocabulary, minted
 * once by the package that defines the surface. The Emacs analogue is a major
 * mode. The workbench never interprets kinds; it only matches on them.
 */
export type SurfaceKind = string & Brand.Brand<"SurfaceKind">;
export const SurfaceKind = Brand.nominal<SurfaceKind>();

/** One live instance of a surface kind. The Emacs analogue is a buffer view. */
export type PanelId = string & Brand.Brand<"PanelId">;
export const PanelId = Brand.nominal<PanelId>();

/**
 * A display slot in the arrangement: a stack (tiled leaf) or an overlay
 * (floating). Slots have identity separate from panels because the same panel
 * may appear in several slots, and focus addresses slots, not panels.
 */
export type SlotId = string & Brand.Brand<"SlotId">;
export const SlotId = Brand.nominal<SlotId>();

/**
 * Where a contribution (command, keybinding, ...) is in effect. Shared
 * vocabulary for every system that scopes behavior to surfaces:
 *
 * - global: everywhere
 * - surface: wherever any panel of this kind is focused (Emacs major-mode map)
 * - panel: in one specific panel instance only (Emacs buffer-local map)
 */
export type SurfaceScope =
  | { readonly _tag: "global" }
  | { readonly _tag: "surface"; readonly kind: SurfaceKind }
  | { readonly _tag: "panel"; readonly id: PanelId };

export const SurfaceScope = {
  global: { _tag: "global" } as SurfaceScope,
  surface: (kind: SurfaceKind): SurfaceScope => ({ _tag: "surface", kind }),
  panel: (id: PanelId): SurfaceScope => ({ _tag: "panel", id }),
};

/**
 * Does a contribution with this scope apply when the given panel is focused?
 * `undefined` means nothing is focused: only global contributions apply.
 */
export function scopeApplies(
  scope: SurfaceScope,
  focused: { readonly id: PanelId; readonly kind: SurfaceKind } | undefined,
): boolean {
  switch (scope._tag) {
    case "global":
      return true;
    case "surface":
      return focused !== undefined && focused.kind === scope.kind;
    case "panel":
      return focused !== undefined && focused.id === scope.id;
  }
}

/**
 * Precedence of a scope when several contributions match: panel-local beats
 * surface-kind beats global. Higher wins.
 */
export function scopeSpecificity(scope: SurfaceScope): number {
  switch (scope._tag) {
    case "global":
      return 0;
    case "surface":
      return 1;
    case "panel":
      return 2;
  }
}
