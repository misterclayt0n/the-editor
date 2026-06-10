/**
 * The arrangement is a *value*: a split tree whose leaves are stacks (ordered
 * panels, one active — what a renderer would draw as tabs) plus a stack of
 * floating overlays. Mirrors the engine's Transaction/TextDocument split:
 * anyone constructs/transforms arrangements with pure functions; the Workbench
 * service validates, normalizes, and commits them.
 *
 * The workbench owns validity. Shape is the caller's business.
 */

import type { PanelId, SlotId } from "./ids.js";
import {
  DuplicatePanelInStackError,
  DuplicateSlotIdError,
  InvalidWeightError,
  UnknownPanelInArrangementError,
  type ArrangementError,
} from "./errors.js";

export type SplitDirection = "row" | "column";

export type Stack = {
  readonly kind: "stack";
  readonly id: SlotId;
  readonly panels: ReadonlyArray<PanelId>;
  readonly active: number;
};

export type Split = {
  readonly kind: "split";
  readonly direction: SplitDirection;
  readonly children: ReadonlyArray<TileChild>;
};

export type TileChild = { readonly tile: Tile; readonly weight: number };

export type Tile = Split | Stack;

export type Overlay = { readonly id: SlotId; readonly panelId: PanelId };

export type Arrangement = {
  readonly tiles: Tile | null;
  readonly overlays: ReadonlyArray<Overlay>;
};

export const emptyArrangement: Arrangement = { tiles: null, overlays: [] };

export type SlotView =
  | { readonly kind: "stack"; readonly id: SlotId; readonly stack: Stack }
  | { readonly kind: "overlay"; readonly id: SlotId; readonly overlay: Overlay };

export function collectSlots(arrangement: Arrangement): ReadonlyArray<SlotView> {
  const out: SlotView[] = [];
  const walk = (tile: Tile): void => {
    if (tile.kind === "stack") {
      out.push({ kind: "stack", id: tile.id, stack: tile });
      return;
    }
    for (const child of tile.children) walk(child.tile);
  };
  if (arrangement.tiles) walk(arrangement.tiles);
  for (const overlay of arrangement.overlays) {
    out.push({ kind: "overlay", id: overlay.id, overlay });
  }
  return out;
}

export function findSlot(arrangement: Arrangement, slotId: SlotId): SlotView | undefined {
  return collectSlots(arrangement).find((slot) => slot.id === slotId);
}

/** The panel a slot currently shows: a stack's active panel, or the overlay's panel. */
export function slotPanel(arrangement: Arrangement, slotId: SlotId): PanelId | undefined {
  const slot = findSlot(arrangement, slotId);
  if (!slot) return undefined;
  if (slot.kind === "overlay") return slot.overlay.panelId;
  return slot.stack.panels[slot.stack.active];
}

/** First slot whose panel list contains the panel (stacks first, then overlays). */
export function slotOfPanel(arrangement: Arrangement, panelId: PanelId): SlotId | undefined {
  for (const slot of collectSlots(arrangement)) {
    if (slot.kind === "stack" && slot.stack.panels.includes(panelId)) return slot.id;
    if (slot.kind === "overlay" && slot.overlay.panelId === panelId) return slot.id;
  }
  return undefined;
}

export function referencedPanels(arrangement: Arrangement): ReadonlySet<PanelId> {
  const out = new Set<PanelId>();
  for (const slot of collectSlots(arrangement)) {
    if (slot.kind === "stack") for (const id of slot.stack.panels) out.add(id);
    else out.add(slot.overlay.panelId);
  }
  return out;
}

const clampActive = (stack: Stack): Stack => {
  const active = Math.max(0, Math.min(stack.panels.length - 1, stack.active));
  return active === stack.active ? stack : { ...stack, active };
};

function normalizeTile(tile: Tile): Tile | null {
  if (tile.kind === "stack") {
    return tile.panels.length === 0 ? null : clampActive(tile);
  }
  const children: TileChild[] = [];
  for (const child of tile.children) {
    const normalized = normalizeTile(child.tile);
    if (normalized === null) continue;
    // Flatten same-direction nested splits, distributing the child's weight.
    if (normalized.kind === "split" && normalized.direction === tile.direction) {
      const total = normalized.children.reduce((sum, c) => sum + c.weight, 0);
      for (const inner of normalized.children) {
        children.push({ tile: inner.tile, weight: (inner.weight / total) * child.weight });
      }
    } else {
      children.push({ tile: normalized, weight: child.weight });
    }
  }
  if (children.length === 0) return null;
  if (children.length === 1) return children[0]!.tile;
  return { kind: "split", direction: tile.direction, children };
}

/**
 * Structural cleanup: prune empty stacks, collapse single-child splits, merge
 * same-direction nesting, clamp active indices. Pure; never fails.
 */
export function normalizeArrangement(arrangement: Arrangement): Arrangement {
  return {
    tiles: arrangement.tiles === null ? null : normalizeTile(arrangement.tiles),
    overlays: arrangement.overlays,
  };
}

/**
 * Reference and structural validity. Duplicate panels *across* slots are
 * allowed (one panel may be displayed in several places); duplicates within a
 * single stack are not.
 */
export function validateArrangement(
  arrangement: Arrangement,
  livePanels: ReadonlySet<PanelId>,
): ArrangementError | undefined {
  const seenSlots = new Set<string>();
  const checkSlotId = (id: SlotId): ArrangementError | undefined => {
    if (seenSlots.has(id)) return new DuplicateSlotIdError({ slotId: id });
    seenSlots.add(id);
    return undefined;
  };
  const checkPanel = (id: PanelId): ArrangementError | undefined =>
    livePanels.has(id) ? undefined : new UnknownPanelInArrangementError({ panelId: id });

  const walk = (tile: Tile): ArrangementError | undefined => {
    if (tile.kind === "stack") {
      const slotError = checkSlotId(tile.id);
      if (slotError) return slotError;
      const seenPanels = new Set<string>();
      for (const panelId of tile.panels) {
        if (seenPanels.has(panelId)) {
          return new DuplicatePanelInStackError({ slotId: tile.id, panelId });
        }
        seenPanels.add(panelId);
        const panelError = checkPanel(panelId);
        if (panelError) return panelError;
      }
      return undefined;
    }
    for (const child of tile.children) {
      if (!Number.isFinite(child.weight) || child.weight <= 0) {
        return new InvalidWeightError({ weight: child.weight });
      }
      const error = walk(child.tile);
      if (error) return error;
    }
    return undefined;
  };

  if (arrangement.tiles) {
    const error = walk(arrangement.tiles);
    if (error) return error;
  }
  for (const overlay of arrangement.overlays) {
    const error = checkSlotId(overlay.id) ?? checkPanel(overlay.panelId);
    if (error) return error;
  }
  return undefined;
}
