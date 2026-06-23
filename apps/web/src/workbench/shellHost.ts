/**
 * The shell's chrome workbench host: a WorkbenchHost over a bare, session-less
 * Workbench.Service. Its panels are the session sidebar and the session
 * viewport, so it owns no editor panes and no workspace — the editor lookups
 * return undefined and `workspaceRoot` is null. WorkbenchView renders it the
 * same way it renders a session; only the panel CONTENT (portaled in by App)
 * differs.
 *
 * Keyboard focus belongs to the active session, not the chrome: a sidebar
 * click switches sessions but the editor keeps the keyboard, so
 * `focusActivePanel` delegates to the active session's focuser rather than
 * focusing a chrome panel.
 */

import { Effect, Fiber, Option, Stream } from "effect";
import {
  activateTab as activateTabSurgery,
  moveTab,
  removePanel,
  slotOfPanel,
  splitBeside,
  Workbench,
  type Panel,
  type PanelId,
  type SlotId,
  type SplitDirection,
  type Tile,
} from "@the-editor/workbench";
import type { EditorPane, WorkbenchSnapshot } from "../runtime";
import type { MultiBufferPane } from "../multibuffer/pane";
import type { PanelRenderers } from "./panelRenderers";
import type { WorkbenchHost } from "./host";

/**
 * A handoff point between a chrome panel's renderer (which owns a DOM element)
 * and App (which portals React content into it). The renderer publishes its
 * mount element here; App subscribes via useSyncExternalStore and renders into
 * whatever element is current.
 */
export class ChromeSlot {
  private element: HTMLElement | null = null;
  private readonly listeners = new Set<() => void>();

  set(element: HTMLElement | null): void {
    if (this.element === element) return;
    this.element = element;
    for (const listener of this.listeners) listener();
  }

  readonly getElement = (): HTMLElement | null => this.element;

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => void this.listeners.delete(listener);
  };
}

export class ShellWorkbenchHost implements WorkbenchHost {
  readonly workspaceRoot = null;
  private snapshot!: WorkbenchSnapshot;
  private readonly listeners = new Set<() => void>();
  private readonly focusers = new Map<PanelId, () => void>();
  private readonly mirrorFibers: Fiber.RuntimeFiber<unknown, unknown>[];

  constructor(
    private readonly workbench: Workbench.Service,
    readonly panelRenderers: PanelRenderers.Service,
    /** Focus the active session's surface; the chrome is not a keyboard surface. */
    private readonly focusActiveSession: () => void,
  ) {
    this.publish();
    this.mirrorFibers = [
      Effect.runFork(
        Stream.runForEach(workbench.arrangementChanges, () => Effect.sync(() => this.publish())),
      ),
      Effect.runFork(
        Stream.runForEach(workbench.focusChanges, () => Effect.sync(() => this.publish())),
      ),
    ];
  }

  private publish(): void {
    this.snapshot = {
      arrangement: Effect.runSync(this.workbench.arrangement),
      focusedSlot: Option.getOrNull(Effect.runSync(this.workbench.focusedSlot)),
      panelIds: Effect.runSync(this.workbench.panels).map((panel) => panel.id),
    };
    for (const listener of this.listeners) listener();
  }

  getWorkbenchSnapshot(): WorkbenchSnapshot {
    return this.snapshot;
  }

  subscribeWorkbench(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => void this.listeners.delete(listener);
  }

  focusActivePanel(): void {
    this.focusActiveSession();
  }

  focusSlotOfPanel(panelId: PanelId): void {
    const slot = slotOfPanel(this.snapshot.arrangement, panelId);
    if (slot !== undefined && this.snapshot.focusedSlot !== slot) {
      Effect.runSync(this.workbench.setFocus(slot).pipe(Effect.ignore));
    }
  }

  activateTab(slotId: SlotId, panelId: PanelId): void {
    Effect.runSync(
      this.workbench
        .updateArrangement((a) => activateTabSurgery(a, slotId, panelId))
        .pipe(Effect.orDie),
    );
  }

  closeTab(panelId: PanelId): void {
    // Chrome panels are permanent; this only un-places (keeps alive).
    Effect.runSync(
      this.workbench.updateArrangement((a) => removePanel(a, panelId)).pipe(Effect.orDie),
    );
  }

  moveTabToStack(panelId: PanelId, fromSlot: SlotId, toSlot: SlotId, index?: number): void {
    Effect.runSync(
      this.workbench
        .updateArrangement((a) =>
          moveTab(a, panelId, fromSlot, toSlot, index !== undefined ? { index } : undefined),
        )
        .pipe(Effect.orDie),
    );
  }

  moveTabToSplit(
    panelId: PanelId,
    targetSlot: SlotId,
    direction: SplitDirection,
    before: boolean,
  ): void {
    Effect.runSync(
      this.workbench
        .updateArrangement((a) => splitBeside(a, targetSlot, panelId, direction, { before }))
        .pipe(Effect.orDie),
    );
  }

  setSplitWeights(path: ReadonlyArray<number>, weights: ReadonlyArray<number>): void {
    const rewrite = (tile: Tile, depth: number): Tile => {
      if (tile.kind !== "split") return tile;
      if (depth === path.length) {
        if (weights.length !== tile.children.length) return tile;
        return {
          ...tile,
          children: tile.children.map((child, i) => ({ ...child, weight: weights[i]! })),
        };
      }
      const index = path[depth]!;
      const child = tile.children[index];
      if (child === undefined) return tile;
      return {
        ...tile,
        children: tile.children.map((c, i) =>
          i === index ? { ...c, tile: rewrite(c.tile, depth + 1) } : c,
        ),
      };
    };
    Effect.runSync(
      this.workbench
        .updateArrangement((a) => (a.tiles === null ? a : { ...a, tiles: rewrite(a.tiles, 0) }))
        .pipe(Effect.ignore),
    );
  }

  panelById(panelId: PanelId): Panel | undefined {
    return Option.getOrUndefined(Effect.runSync(this.workbench.panel(panelId)));
  }

  registerPanelFocuser(panelId: PanelId, focus: () => void): () => void {
    this.focusers.set(panelId, focus);
    return () => {
      if (this.focusers.get(panelId) === focus) this.focusers.delete(panelId);
    };
  }

  pane(): EditorPane | undefined {
    return undefined;
  }

  multiBufferPane(): MultiBufferPane | undefined {
    return undefined;
  }

  dispose(): void {
    for (const fiber of this.mirrorFibers) Effect.runFork(Fiber.interrupt(fiber));
    this.listeners.clear();
    this.focusers.clear();
  }
}
