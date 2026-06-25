/**
 * Dock-region show/hide for workbench hosts.
 *
 * A sidebar is a dock region: a tabbed stack many panels share. Hiding one must
 * collapse the WHOLE region (every tab) and remember its layout, so re-showing
 * restores the exact tabs, active selection, and width — not just one panel.
 *
 * The pure surgery lives in `@the-editor/workbench`; this is the tiny glue that
 * reads/commits a live workbench and owns the per-region stash map (one map per
 * host: the session runtime and the shell each keep their own).
 */

import { Effect } from "effect";
import {
  collapseRegion,
  dockIntoRegion,
  regionStack,
  removePanel,
  restoreRegion,
  type DockEdge,
  type PanelId,
  type RegionSnapshot,
  type Workbench,
} from "@the-editor/workbench";

/** Outcome of a toggle. "empty" means there was nothing to hide and no prior
 *  layout to restore — the caller seeds the region (e.g. opens the file tree). */
export type RegionToggle = "collapsed" | "restored" | "empty";

/** Hide a region (stashing its full layout) or restore it from a prior stash. */
export function toggleRegion(
  workbench: Workbench.Service,
  stashes: Map<string, RegionSnapshot>,
  region: string,
): RegionToggle {
  const collapsed = collapseRegion(Effect.runSync(workbench.arrangement), region);
  if (collapsed !== undefined) {
    stashes.set(region, collapsed.snapshot);
    Effect.runSync(workbench.updateArrangement(() => collapsed.arrangement).pipe(Effect.orDie));
    return "collapsed";
  }
  const snapshot = stashes.get(region);
  if (snapshot === undefined) return "empty";
  stashes.delete(region);
  const live = new Set(Effect.runSync(workbench.panels).map((panel) => panel.id));
  Effect.runSync(
    workbench.updateArrangement((current) => restoreRegion(current, snapshot, live)).pipe(Effect.orDie),
  );
  return "restored";
}

/** Whether a region is currently laid out (vs collapsed or never created). */
export function isRegionVisible(workbench: Workbench.Service, region: string): boolean {
  return regionStack(Effect.runSync(workbench.arrangement), region) !== undefined;
}

/** Move `panelId` into region `to` (live, joining or creating it). Restores
 *  either `from` or `to` if it was collapsed first, so a region is never left
 *  both live and stashed and the other tabs on each side survive the move. The
 *  live-or-stashed invariant lives here, not in callers. */
export function moveToRegion(
  workbench: Workbench.Service,
  stashes: Map<string, RegionSnapshot>,
  panelId: PanelId,
  from: string,
  to: string,
  options: { readonly edge: DockEdge; readonly className?: string; readonly ratio?: number },
): void {
  if (stashes.has(from)) toggleRegion(workbench, stashes, from);
  if (to !== from && stashes.has(to)) toggleRegion(workbench, stashes, to);
  Effect.runSync(
    workbench
      .updateArrangement((a) => dockIntoRegion(removePanel(a, panelId), panelId, { region: to, ...options }))
      .pipe(Effect.orDie),
  );
}
