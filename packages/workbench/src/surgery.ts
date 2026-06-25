/**
 * Pure arrangement surgery.
 *
 * These are library functions, not API surface: product gestures ("split
 * right", "open as tab", drag-and-drop commits) are compositions of these, and
 * user packages can ignore them and write their own — the Workbench only
 * enforces validity at commit time. None of these normalize; the commit
 * boundary does.
 */

import type { Arrangement, DockEdge, Overlay, Stack, SplitDirection, Tile } from "./arrangement.js";
import { SlotNotFoundError } from "./errors.js";
import { SlotId, type PanelId, type SlotId as SlotIdType } from "./ids.js";

let nextSlotId = 1;

export function freshSlotId(): SlotIdType {
  return SlotId(`slot:${nextSlotId++}`);
}

export function makeStack(panels: ReadonlyArray<PanelId>, active = 0): Stack {
  return { kind: "stack", id: freshSlotId(), panels, active };
}

function mapStacks(tile: Tile, f: (stack: Stack) => Tile): Tile {
  if (tile.kind === "stack") return f(tile);
  return {
    ...tile,
    children: tile.children.map((child) => ({ ...child, tile: mapStacks(child.tile, f) })),
  };
}

function requireStack(arrangement: Arrangement, slotId: SlotIdType): void {
  const found = (function find(tile: Tile | null): boolean {
    if (tile === null) return false;
    if (tile.kind === "stack") return tile.id === slotId;
    return tile.children.some((child) => find(child.tile));
  })(arrangement.tiles);
  if (!found) throw new SlotNotFoundError({ slotId });
}

/** Insert a panel into a stack (as a "tab"), optionally at an index, activating it by default. */
export function placeInStack(
  arrangement: Arrangement,
  slotId: SlotIdType,
  panelId: PanelId,
  options?: { readonly index?: number; readonly activate?: boolean },
): Arrangement {
  requireStack(arrangement, slotId);
  return {
    ...arrangement,
    tiles: mapStacks(arrangement.tiles!, (stack) => {
      if (stack.id !== slotId) return stack;
      const index = Math.max(
        0,
        Math.min(stack.panels.length, options?.index ?? stack.panels.length),
      );
      const panels = [...stack.panels.slice(0, index), panelId, ...stack.panels.slice(index)];
      const activate = options?.activate ?? true;
      const active = activate ? index : stack.active >= index ? stack.active + 1 : stack.active;
      return { ...stack, panels, active };
    }),
  };
}

/** Split the root in two, placing `stack` as a new pane at `edge`; `ratio` is
 *  its share of the new split, the existing tiles take the rest. */
function attachStackAtEdge(
  arrangement: Arrangement,
  stack: Stack,
  edge: DockEdge,
  ratio: number,
): Arrangement {
  if (arrangement.tiles === null) return { ...arrangement, tiles: stack };
  const direction: SplitDirection = edge === "left" || edge === "right" ? "row" : "column";
  const first = edge === "left" || edge === "top";
  const children = first
    ? [
        { tile: stack as Tile, weight: ratio },
        { tile: arrangement.tiles, weight: 1 - ratio },
      ]
    : [
        { tile: arrangement.tiles, weight: 1 - ratio },
        { tile: stack as Tile, weight: ratio },
      ];
  return { ...arrangement, tiles: { kind: "split", direction, children } };
}

/** Dock a new singleton stack at an edge of the whole tiled area. */
export function dockEdge(
  arrangement: Arrangement,
  panelId: PanelId,
  edge: DockEdge,
  options?: { readonly ratio?: number },
): Arrangement {
  return attachStackAtEdge(arrangement, makeStack([panelId]), edge, options?.ratio ?? 0.5);
}

/** Find a region's stack plus its parent-split share and the edge it sits on,
 *  so a collapse can be restored to the same place/width. Depth-first. */
function findRegion(
  tile: Tile | null,
  region: string,
): { readonly stack: Stack; readonly ratio: number; readonly edge: DockEdge } | undefined {
  if (tile === null || tile.kind === "stack") {
    return tile?.kind === "stack" && tile.region === region
      ? { stack: tile, ratio: 1, edge: "left" }
      : undefined;
  }
  const total = tile.children.reduce((sum, child) => sum + child.weight, 0);
  for (let i = 0; i < tile.children.length; i++) {
    const child = tile.children[i]!;
    if (child.tile.kind === "stack" && child.tile.region === region) {
      const last = i === tile.children.length - 1;
      const edge: DockEdge =
        tile.direction === "row" ? (last ? "right" : "left") : last ? "bottom" : "top";
      return { stack: child.tile, ratio: total > 0 ? child.weight / total : 0.2, edge };
    }
    const nested = findRegion(child.tile, region);
    if (nested !== undefined) return nested;
  }
  return undefined;
}

/**
 * Dock a panel into a named region: if the region's stack already exists, the
 * panel joins it as a tab; otherwise a fresh region stack is created at `edge`,
 * carrying the region id, chrome class, and tab-strip preference. This is what
 * makes a sidebar a real dock zone — many contributed panels tab together.
 */
export function dockIntoRegion(
  arrangement: Arrangement,
  panelId: PanelId,
  options: {
    readonly region: string;
    readonly edge: DockEdge;
    readonly className?: string;
    readonly tabsHidden?: boolean;
    readonly ratio?: number;
  },
): Arrangement {
  const existing = findRegion(arrangement.tiles, options.region);
  if (existing !== undefined) return placeInStack(arrangement, existing.stack.id, panelId);
  const stack: Stack = {
    kind: "stack",
    id: freshSlotId(),
    panels: [panelId],
    active: 0,
    region: options.region,
    ...(options.className !== undefined ? { className: options.className } : {}),
    ...(options.tabsHidden !== undefined ? { tabsHidden: options.tabsHidden } : {}),
  };
  return attachStackAtEdge(arrangement, stack, options.edge, options.ratio ?? 0.2);
}

/** Split a stack in two: the target keeps its place, the new panel gets a sibling stack. */
export function splitBeside(
  arrangement: Arrangement,
  targetSlot: SlotIdType,
  panelId: PanelId,
  direction: SplitDirection,
  options?: { readonly ratio?: number; readonly before?: boolean },
): Arrangement {
  requireStack(arrangement, targetSlot);
  const ratio = options?.ratio ?? 0.5;
  return {
    ...arrangement,
    tiles: mapStacks(arrangement.tiles!, (stack) => {
      if (stack.id !== targetSlot) return stack;
      const fresh = makeStack([panelId]);
      const children = options?.before
        ? [
            { tile: fresh as Tile, weight: ratio },
            { tile: stack as Tile, weight: 1 - ratio },
          ]
        : [
            { tile: stack as Tile, weight: 1 - ratio },
            { tile: fresh as Tile, weight: ratio },
          ];
      return { kind: "split", direction, children };
    }),
  };
}

function removeFromStack(stack: Stack, panelId: PanelId): Stack {
  const index = stack.panels.indexOf(panelId);
  if (index === -1) return stack;
  const panels = stack.panels.filter((id) => id !== panelId);
  const active = stack.active > index ? stack.active - 1 : stack.active;
  return { ...stack, panels, active };
}

/** Move a panel between stacks (drag a tab). Empty source stacks are pruned at commit. */
export function moveTab(
  arrangement: Arrangement,
  panelId: PanelId,
  fromSlot: SlotIdType,
  toSlot: SlotIdType,
  options?: { readonly index?: number },
): Arrangement {
  requireStack(arrangement, fromSlot);
  requireStack(arrangement, toSlot);
  const removed: Arrangement = {
    ...arrangement,
    tiles: mapStacks(arrangement.tiles!, (stack) =>
      stack.id === fromSlot ? removeFromStack(stack, panelId) : stack,
    ),
  };
  return placeInStack(removed, toSlot, panelId, {
    ...(options?.index !== undefined ? { index: options.index } : {}),
    activate: true,
  });
}

/** Activate a tab within a stack. */
export function activateTab(
  arrangement: Arrangement,
  slotId: SlotIdType,
  panelId: PanelId,
): Arrangement {
  requireStack(arrangement, slotId);
  return {
    ...arrangement,
    tiles: mapStacks(arrangement.tiles!, (stack) => {
      if (stack.id !== slotId) return stack;
      const index = stack.panels.indexOf(panelId);
      return index === -1 ? stack : { ...stack, active: index };
    }),
  };
}

/** Show/hide a stack's tab strip (chrome only — panels are untouched). */
export function toggleStackTabs(arrangement: Arrangement, slotId: SlotIdType): Arrangement {
  requireStack(arrangement, slotId);
  return {
    ...arrangement,
    tiles: mapStacks(arrangement.tiles!, (stack) =>
      stack.id === slotId ? { ...stack, tabsHidden: stack.tabsHidden !== true } : stack,
    ),
  };
}

/** Float a panel above the tiles. Last overlay is topmost. */
export function floatPanel(arrangement: Arrangement, panelId: PanelId): Arrangement {
  const overlay: Overlay = { id: freshSlotId(), panelId };
  return { ...arrangement, overlays: [...arrangement.overlays, overlay] };
}

/** Remove every reference to a panel (it stays alive — just undisplayed). */
export function removePanel(arrangement: Arrangement, panelId: PanelId): Arrangement {
  return {
    tiles:
      arrangement.tiles === null
        ? null
        : mapStacks(arrangement.tiles, (stack) => removeFromStack(stack, panelId)),
    overlays: arrangement.overlays.filter((overlay) => overlay.panelId !== panelId),
  };
}

/** Remove one slot (close a split pane or dismiss an overlay). Panels stay alive. */
export function removeSlot(arrangement: Arrangement, slotId: SlotIdType): Arrangement {
  return {
    tiles:
      arrangement.tiles === null
        ? null
        : mapStacks(arrangement.tiles, (stack) =>
            stack.id === slotId ? { ...stack, panels: [], active: 0 } : stack,
          ),
    overlays: arrangement.overlays.filter((overlay) => overlay.id !== slotId),
  };
}

/** The visible stack of a region, if it is currently laid out. */
export function regionStack(arrangement: Arrangement, region: string): Stack | undefined {
  return findRegion(arrangement.tiles, region)?.stack;
}

/** A collapsed region's full layout — enough to restore it byte-for-byte. */
export type RegionSnapshot = {
  readonly stack: Stack;
  readonly ratio: number;
  readonly edge: DockEdge;
};

/** Collapse a whole dock region: remove its stack and return it (with its split
 *  share and edge) so {@link restoreRegion} can bring the exact layout back. The
 *  panels stay alive in the pool — only the placement is dropped. */
export function collapseRegion(
  arrangement: Arrangement,
  region: string,
): { readonly arrangement: Arrangement; readonly snapshot: RegionSnapshot } | undefined {
  const found = findRegion(arrangement.tiles, region);
  if (found === undefined) return undefined;
  return {
    arrangement: removeSlot(arrangement, found.stack.id),
    snapshot: { stack: found.stack, ratio: found.ratio, edge: found.edge },
  };
}

/** Re-dock a previously collapsed region, dropping any panels that died while it
 *  was hidden. A no-op (returns the input) when nothing remains to show. */
export function restoreRegion(
  arrangement: Arrangement,
  snapshot: RegionSnapshot,
  livePanels: ReadonlySet<PanelId>,
): Arrangement {
  const panels = snapshot.stack.panels.filter((id) => livePanels.has(id));
  if (panels.length === 0) return arrangement;
  const active = Math.min(snapshot.stack.active, panels.length - 1);
  return attachStackAtEdge(arrangement, { ...snapshot.stack, panels, active }, snapshot.edge, snapshot.ratio);
}
