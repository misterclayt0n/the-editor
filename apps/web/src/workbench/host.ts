/**
 * WorkbenchHost: everything WorkbenchView needs to render and drive ONE
 * workbench, with no assumption that the workbench belongs to a session.
 *
 * A session's AppRuntime satisfies this structurally; so does the shell's
 * chrome host (the outer workbench whose panels are the session sidebar and
 * the session viewport). The editor lookups (`pane`, `multiBufferPane`) return
 * undefined for a non-session host — the shell chrome has no editor panes — so
 * WorkbenchView's editor render branch is simply never reached for it.
 */

import type { Panel, PanelId, PanelRenderers, SlotId, SplitDirection } from "@the-editor/workbench";
import type { EditorPane, WorkbenchSnapshot } from "../runtime";
import type { MultiBufferPane } from "../multibuffer/pane";

export interface WorkbenchHost {
  /** Subscribe to layout changes (arrangement, focus, panel set). */
  subscribeWorkbench(listener: () => void): () => void;
  /** Stable layout snapshot for useSyncExternalStore. */
  getWorkbenchSnapshot(): WorkbenchSnapshot;

  /** Land the keyboard in the focused pane's own surface. */
  focusActivePanel(): void;
  /** Focus the slot that displays a panel (e.g. on `focusin`). */
  focusSlotOfPanel(panelId: PanelId): void;

  /** Activate (and focus) a tab within a stack. */
  activateTab(slotId: SlotId, panelId: PanelId): void;
  /** Close a tab; the panel dies. */
  closeTab(panelId: PanelId): void;
  /** Close a tab via user gesture: guards an unsaved editor pane with a prompt. */
  requestCloseTab(panelId: PanelId): void;
  /** Drag commit: move a tab into another stack (or reorder within one). */
  moveTabToStack(panelId: PanelId, fromSlot: SlotId, toSlot: SlotId, index?: number): void;
  /** Drag commit: tear a tab out into a new split beside the target stack. */
  moveTabToSplit(
    panelId: PanelId,
    targetSlot: SlotId,
    direction: SplitDirection,
    before: boolean,
  ): void;
  /** Resize commit: replace the weights of a split node (path from root). */
  setSplitWeights(path: ReadonlyArray<number>, weights: ReadonlyArray<number>): void;

  /** Live panel by id (editor panes AND custom panels). */
  panelById(panelId: PanelId): Panel | undefined;
  /** A surface registers how to focus itself; returns an unregister disposer. */
  registerPanelFocuser(panelId: PanelId, focus: () => void): () => void;
  /** Renderer registry for custom (non-editor) panel kinds. */
  readonly panelRenderers: PanelRenderers.Service;

  /** Editor pane for a panel, when this host hosts editors (else undefined). */
  pane(panelId: PanelId): EditorPane | undefined;
  /** Multibuffer pane for a panel, when this host hosts them (else undefined). */
  multiBufferPane(panelId: PanelId): MultiBufferPane | undefined;
  /** Project root of this workbench (null for chrome hosts with no workspace);
   *  multibuffer header cards render paths relative to it. */
  readonly workspaceRoot: string | null;
}
