/**
 * The workbench chrome: renders the arrangement tree (splits → stacks →
 * tabs), and turns pointer gestures into arrangement surgery commits.
 *
 * React renders FROM the workbench, never owns layout truth: the only state
 * here is the in-flight drag. Gestures:
 *   - click tab            activate + focus
 *   - middle-click / ×     close tab
 *   - drag tab             move into another stack (tab bar = positioned
 *                          insert; pane center = append) or tear into a new
 *                          split (pane edges)
 *   - drag divider         reweight the split
 */

import {
  Fragment,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { SearchIcon } from "lucide-react";
import type { EditorSnapshot } from "@the-editor/editor";
import type { PanelId, SlotId, Stack, Tile } from "@the-editor/workbench";
import type { EditorPane } from "../runtime";
import type { WorkbenchHost } from "./host";
import { EditorView } from "../editor/EditorView";
import { MultiBufferView } from "../multibuffer/MultiBufferView";
import { FileIcon } from "./fileIcons";
import type { PanelMount } from "./panelRenderers";
import { createPortalNode, OutPortal, type PortalNode } from "./panelPortal";

type WorkbenchViewProps = {
  readonly runtime: WorkbenchHost;
  /** Only the active workspace shows and drives focus. Inactive sessions stay
   *  mounted — their terminals, editor scroll, and portal DOM survive — but are
   *  hidden and never grab the keyboard. */
  readonly active: boolean;
  /** Snapshot stream of the FOCUSED pane (status bar fodder). */
  readonly onActiveSnapshot?: (snapshot: EditorSnapshot) => void;
  /** Left inset (px) applied to ONLY the top-left stack's tab strip, so its
   *  first tab clears the window controls (traffic lights + sidebar toggle)
   *  when this workbench sits flush against the window's top-left. */
  readonly firstTabInset?: number;
  /** A real action (the file-tree toggle) rendered in the top-RIGHT stack's tab
   *  strip as a trailing element — it reserves layout space, never overlays. */
  readonly trailingAction?: ReactNode;
};

type DropZone = "center" | "left" | "right" | "top" | "bottom";

type DropTarget =
  | { readonly kind: "tabbar"; readonly slotId: SlotId; readonly index: number }
  | { readonly kind: "zone"; readonly slotId: SlotId; readonly zone: DropZone };

type DragState = {
  readonly panelId: PanelId;
  readonly fromSlot: SlotId;
  readonly title: string;
  readonly x: number;
  readonly y: number;
  readonly target: DropTarget | null;
};

const DRAG_THRESHOLD = 5;
/** Edge bands of the pane body that mean "split here" instead of "move here". */
const EDGE_RATIO = 0.27;

function hitTest(x: number, y: number): DropTarget | null {
  const el = document.elementFromPoint(x, y);
  if (el === null) return null;
  const tabbar = el.closest<HTMLElement>("[data-wb-tabbar]");
  if (tabbar !== null) {
    const slotId = tabbar.dataset["slotId"] as SlotId;
    const tabs = [...tabbar.querySelectorAll<HTMLElement>("[data-wb-tab]")];
    let index = tabs.length;
    for (let i = 0; i < tabs.length; i++) {
      const rect = tabs[i]!.getBoundingClientRect();
      if (x < rect.left + rect.width / 2) {
        index = i;
        break;
      }
    }
    return { kind: "tabbar", slotId, index };
  }
  const body = el.closest<HTMLElement>("[data-wb-body]");
  if (body !== null) {
    const slotId = body.dataset["slotId"] as SlotId;
    const rect = body.getBoundingClientRect();
    const rx = (x - rect.left) / rect.width;
    const ry = (y - rect.top) / rect.height;
    const zone: DropZone =
      rx < EDGE_RATIO
        ? "left"
        : rx > 1 - EDGE_RATIO
          ? "right"
          : ry < EDGE_RATIO
            ? "top"
            : ry > 1 - EDGE_RATIO
              ? "bottom"
              : "center";
    return { kind: "zone", slotId, zone };
  }
  return null;
}

function collectStacks(tile: Tile, out: Stack[]): void {
  if (tile.kind === "stack") {
    out.push(tile);
    return;
  }
  for (const child of tile.children) collectStacks(child.tile, out);
}

/** The top-left stack: follow the first child down the split tree. Its tab
 *  strip is the one that sits under the window controls. */
function firstStackId(tile: Tile | null): SlotId | undefined {
  let node = tile;
  while (node !== null && node.kind === "split") node = node.children[0]?.tile ?? null;
  return node !== null && node.kind === "stack" ? node.id : undefined;
}

/** The top-right stack: follow the last child down the split tree. Its tab
 *  strip is the one under the file-tree toggle at the window's top-right. */
function lastStackId(tile: Tile | null): SlotId | undefined {
  let node = tile;
  while (node !== null && node.kind === "split") node = node.children[node.children.length - 1]?.tile ?? null;
  return node !== null && node.kind === "stack" ? node.id : undefined;
}

export function WorkbenchView({ runtime, active, onActiveSnapshot, firstTabInset, trailingAction }: WorkbenchViewProps) {
  const wb = useSyncExternalStore(
    useCallback((cb) => runtime.subscribeWorkbench(cb), [runtime]),
    () => runtime.getWorkbenchSnapshot(),
  );
  const [drag, setDrag] = useState<DragState | null>(null);
  const leftStackId = firstTabInset ? firstStackId(wb.arrangement.tiles) : undefined;
  const rightStackId = trailingAction !== undefined ? lastStackId(wb.arrangement.tiles) : undefined;

  // One persistent container per panel, created lazily. Panel content renders
  // into these (see PanelInPortals); an OutPortal re-homes the live element on
  // a tab switch or stack move, so the content (a terminal's PTY + screen, an
  // editor's scroll) is never disposed and rebuilt — only when the panel
  // leaves the arrangement (closed).
  const nodesRef = useRef<Map<PanelId, PortalNode> | null>(null);
  const nodeFor = useCallback((panelId: PanelId): PortalNode => {
    const nodes = (nodesRef.current ??= new Map<PanelId, PortalNode>());
    let node = nodes.get(panelId);
    if (node === undefined) {
      node = createPortalNode("wb-panel-host");
      nodes.set(panelId, node);
    }
    return node;
  }, []);

  // Keep every visited panel mounted (lazy: a never-shown panel isn't built).
  // Liveness follows the panel LIFECYCLE, not placement: a panel hidden from
  // the arrangement (e.g. a toggled-off terminal) stays alive — its content is
  // disposed only when the panel closes and leaves `wb.panelIds`.
  const activatedRef = useRef<Set<PanelId> | null>(null);
  const activated = (activatedRef.current ??= new Set<PanelId>());
  const nodes = (nodesRef.current ??= new Map<PanelId, PortalNode>());
  const stacks: Stack[] = [];
  if (wb.arrangement.tiles !== null) collectStacks(wb.arrangement.tiles, stacks);
  const allPanels = new Set<PanelId>(wb.panelIds);
  for (const id of activated) if (!allPanels.has(id)) activated.delete(id);
  for (const id of nodes.keys()) if (!allPanels.has(id)) nodes.delete(id);
  for (const s of stacks) {
    const active = s.panels[s.active];
    if (active !== undefined) activated.add(active);
  }
  const livePanels = [...activated];
  const focusedStack = stacks.find((s) => s.id === wb.focusedSlot);
  const focusedActivePanelId =
    focusedStack !== undefined ? focusedStack.panels[focusedStack.active] : undefined;

  // Single focus authority: whenever the focused pane or its active panel
  // changes, hand the keyboard to that surface (editor, terminal, file tree).
  // useLayoutEffect so the focus lands before the browser paints, which matters
  // when a reverse-portal host is re-attached (terminal toggle on/off).
  useLayoutEffect(() => {
    if (active) runtime.focusActivePanel();
  }, [runtime, active, wb.focusedSlot, focusedActivePanelId]);

  const beginTabDrag = useCallback(
    (
      event: React.PointerEvent,
      panelId: PanelId,
      fromSlot: SlotId,
      title: string,
    ) => {
      if (event.button !== 0) return;
      const startX = event.clientX;
      const startY = event.clientY;
      let active = false;

      const onMove = (move: PointerEvent) => {
        if (!active) {
          const dx = move.clientX - startX;
          const dy = move.clientY - startY;
          if (dx * dx + dy * dy < DRAG_THRESHOLD * DRAG_THRESHOLD) return;
          active = true;
        }
        setDrag({
          panelId,
          fromSlot,
          title,
          x: move.clientX,
          y: move.clientY,
          target: hitTest(move.clientX, move.clientY),
        });
      };

      const onUp = (up: PointerEvent) => {
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("pointerup", onUp);
        setDrag(null);
        commit(up);
        // A tab gesture lands DOM focus on inert chrome; hand it back to the
        // active surface of whichever pane ended up focused.
        runtime.focusActivePanel();
      };

      const commit = (up: PointerEvent) => {
        if (!active) {
          // A plain click: activate the tab.
          runtime.activateTab(fromSlot, panelId);
          return;
        }
        const target = hitTest(up.clientX, up.clientY);
        if (target === null) return;
        if (target.kind === "tabbar") {
          runtime.moveTabToStack(panelId, fromSlot, target.slotId, target.index);
          return;
        }
        if (target.zone === "center") {
          if (target.slotId !== fromSlot) {
            runtime.moveTabToStack(panelId, fromSlot, target.slotId);
          }
          return;
        }
        const direction = target.zone === "left" || target.zone === "right" ? "row" : "column";
        const before = target.zone === "left" || target.zone === "top";
        runtime.moveTabToSplit(panelId, target.slotId, direction, before);
      };

      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
    },
    [runtime],
  );

  if (wb.arrangement.tiles === null) {
    // No visible tiles, but hidden-yet-alive panels (a toggled-off terminal)
    // still render into their parked containers so their state survives.
    return (
      <div className="workbench" style={active ? undefined : { display: "none" }}>
        <div className="wb-empty">no open files — ⌘O to open, ⇧⌘P for commands</div>
        <PanelInPortals
          runtime={runtime}
          nodeFor={nodeFor}
          livePanels={livePanels}
          focusedActivePanelId={focusedActivePanelId}
          {...(onActiveSnapshot ? { onActiveSnapshot } : {})}
        />
      </div>
    );
  }

  return (
    <div className="workbench" style={active ? undefined : { display: "none" }}>
      <TileView
        tile={wb.arrangement.tiles}
        path={[]}
        runtime={runtime}
        focusedSlot={wb.focusedSlot}
        drag={drag}
        beginTabDrag={beginTabDrag}
        nodeFor={nodeFor}
        {...(leftStackId !== undefined && firstTabInset ? { leftStackId, firstTabInset } : {})}
        {...(rightStackId !== undefined && trailingAction !== undefined ? { rightStackId, trailingAction } : {})}
      />
      <PanelInPortals
        runtime={runtime}
        nodeFor={nodeFor}
        livePanels={livePanels}
        focusedActivePanelId={focusedActivePanelId}
        {...(onActiveSnapshot ? { onActiveSnapshot } : {})}
      />
      {drag !== null && (
        <div className="wb-drag-ghost" style={{ left: drag.x + 10, top: drag.y + 8 }}>
          {drag.title}
        </div>
      )}
    </div>
  );
}

/**
 * Renders every live panel's content into its persistent container. Mounted
 * once at the workbench root, so a panel's content survives tab switches and
 * stack moves — only the container is re-homed (by an OutPortal), never the
 * React subtree. The focused stack's active panel drives focus and the live
 * editor snapshot.
 */
function PanelInPortals({
  runtime,
  nodeFor,
  livePanels,
  focusedActivePanelId,
  onActiveSnapshot,
}: {
  readonly runtime: WorkbenchHost;
  readonly nodeFor: (panelId: PanelId) => PortalNode;
  readonly livePanels: ReadonlyArray<PanelId>;
  readonly focusedActivePanelId: PanelId | undefined;
  readonly onActiveSnapshot?: (snapshot: EditorSnapshot) => void;
}): ReactNode {
  return livePanels.map((panelId) => (
    <PortalPanel
      key={panelId}
      runtime={runtime}
      panelId={panelId}
      node={nodeFor(panelId)}
      focused={panelId === focusedActivePanelId}
      {...(onActiveSnapshot ? { onSnapshot: onActiveSnapshot } : {})}
    />
  ));
}

/**
 * One live panel rendered into its persistent host. A native `focusin` listener
 * on the host updates the workbench's focused slot whenever the keyboard lands
 * inside this panel — React synthetic events do NOT bubble through the reverse
 * portal, so the host must observe focus directly.
 */
function PortalPanel({
  runtime,
  panelId,
  node,
  focused,
  onSnapshot,
}: {
  readonly runtime: WorkbenchHost;
  readonly panelId: PanelId;
  readonly node: PortalNode;
  readonly focused: boolean;
  readonly onSnapshot?: (snapshot: EditorSnapshot) => void;
}) {
  useEffect(() => {
    const element = node.element;
    const onFocusIn = () => runtime.focusSlotOfPanel(panelId);
    element.addEventListener("focusin", onFocusIn);
    return () => element.removeEventListener("focusin", onFocusIn);
  }, [runtime, panelId, node]);

  return createPortal(
    <PanelBody
      runtime={runtime}
      panelId={panelId}
      focused={focused}
      {...(onSnapshot ? { onSnapshot } : {})}
    />,
    node.element,
    panelId,
  );
}

function PanelBody({
  runtime,
  panelId,
  focused,
  onSnapshot,
}: {
  readonly runtime: WorkbenchHost;
  readonly panelId: PanelId;
  readonly focused: boolean;
  readonly onSnapshot?: (snapshot: EditorSnapshot) => void;
}) {
  const pane = runtime.pane(panelId);
  if (pane !== undefined) {
    return (
      <EditorView
        runtime={runtime}
        pane={pane}
        focused={focused}
        {...(focused && onSnapshot ? { onSnapshot } : {})}
      />
    );
  }
  const multiBuffer = runtime.multiBufferPane(panelId);
  if (multiBuffer !== undefined) {
    return <MultiBufferView runtime={runtime} pane={multiBuffer} focused={focused} />;
  }
  return <CustomPanelView runtime={runtime} panelId={panelId} />;
}

type TreeProps = {
  readonly runtime: WorkbenchHost;
  readonly focusedSlot: SlotId | null;
  readonly drag: DragState | null;
  readonly beginTabDrag: (
    event: React.PointerEvent,
    panelId: PanelId,
    fromSlot: SlotId,
    title: string,
  ) => void;
  readonly nodeFor: (panelId: PanelId) => PortalNode;
  /** The top-left stack id and the inset to apply to its tab strip (see
   *  WorkbenchViewProps.firstTabInset). Absent when no inset is requested. */
  readonly leftStackId?: SlotId;
  readonly firstTabInset?: number;
  /** The top-right stack id and the trailing action to render in its tab strip
   *  (see WorkbenchViewProps.trailingAction). Absent when none is requested. */
  readonly rightStackId?: SlotId;
  readonly trailingAction?: ReactNode;
};

function TileView({ tile, path, ...tree }: TreeProps & { tile: Tile; path: ReadonlyArray<number> }) {
  if (tile.kind === "stack") return <StackView stack={tile} {...tree} />;
  return <SplitView split={tile} path={path} {...tree} />;
}

function SplitView({
  split: tile,
  path,
  ...tree
}: TreeProps & { split: Extract<Tile, { kind: "split" }>; path: ReadonlyArray<number> }) {
  const container = useRef<HTMLDivElement>(null);
  // flex-grow only consumes 100% of free space when the grow factors sum to >=1;
  // a removed sibling can leave the raw weights summing to <1 (e.g. one cell of a
  // former 3-way split), which would leave the rest of the row blank. Normalize so
  // the cells always tile the full axis regardless of the stored weights.
  const totalWeight = tile.children.reduce((sum, child) => sum + child.weight, 0);
  return (
    <div ref={container} className={`wb-split wb-split-${tile.direction}`}>
      {tile.children.map((child, i) => (
        <Fragment key={child.tile.kind === "stack" ? child.tile.id : `split:${i}`}>
          {i > 0 && (
            <SplitDivider
              runtime={tree.runtime}
              direction={tile.direction}
              path={path}
              index={i}
              container={container}
            />
          )}
          <div
            className="wb-cell"
            style={{ flexGrow: totalWeight > 0 ? child.weight / totalWeight : 1 }}
          >
            <TileView tile={child.tile} path={[...path, i]} {...tree} />
          </div>
        </Fragment>
      ))}
    </div>
  );
}

/**
 * Reweights the two children adjacent to the divider; the rest keep theirs.
 * Weights are committed live (the arrangement is the single layout truth, so
 * dragging is just a high-frequency series of small commits).
 */
function SplitDivider({
  runtime,
  direction,
  path,
  index,
  container,
}: {
  readonly runtime: WorkbenchHost;
  readonly direction: "row" | "column";
  readonly path: ReadonlyArray<number>;
  readonly index: number;
  readonly container: React.RefObject<HTMLDivElement | null>;
}) {
  const onPointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    const containerEl = container.current;
    if (containerEl === null) return;
    const horizontal = direction === "row";
    const sizePx = horizontal
      ? containerEl.getBoundingClientRect().width
      : containerEl.getBoundingClientRect().height;
    const start = horizontal ? event.clientX : event.clientY;

    const tiles = runtime.getWorkbenchSnapshot().arrangement.tiles;
    let node: Tile | null = tiles;
    for (const step of path) {
      if (node === null || node.kind !== "split") return;
      node = node.children[step]?.tile ?? null;
    }
    if (node === null || node.kind !== "split") return;
    const startWeights = node.children.map((child) => child.weight);
    const total = startWeights.reduce((sum, w) => sum + w, 0);
    const minWeight = total * 0.1;

    const target = event.currentTarget;
    target.setPointerCapture(event.pointerId);
    let frame = 0;

    const onMove = (move: PointerEvent) => {
      const deltaWeight = (((horizontal ? move.clientX : move.clientY) - start) / sizePx) * total;
      const left = Math.max(
        minWeight,
        Math.min(startWeights[index - 1]! + startWeights[index]! - minWeight, startWeights[index - 1]! + deltaWeight),
      );
      const right = startWeights[index - 1]! + startWeights[index]! - left;
      const weights = [...startWeights];
      weights[index - 1] = left;
      weights[index] = right;
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => runtime.setSplitWeights(path, weights));
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  return (
    <div
      className={`wb-divider wb-divider-${direction}`}
      onPointerDown={onPointerDown}
    />
  );
}

function StackView({ stack, ...tree }: TreeProps & { stack: Stack }) {
  const { runtime, focusedSlot, drag } = tree;
  const activePanelId = stack.panels[stack.active];
  // Content keep-alive (across tab switches AND stack moves) is owned by the
  // panel pool at the workbench root; this stack only homes its active panel's
  // live container via an OutPortal.
  const focused = focusedSlot === stack.id;
  // Chrome styling for the pane container: a dock REGION owns its look via
  // stack.className (the right/left sidebars); otherwise a panel kind may
  // declare it via renderer.chrome. Region wins so every panel in a region
  // shares one look regardless of which tab is active.
  const activePanel = activePanelId !== undefined ? runtime.panelById(activePanelId) : undefined;
  const chromeClass =
    stack.className ??
    (activePanel !== undefined ? runtime.panelRenderers.get(activePanel.kind)?.chrome?.className : undefined);

  const dropZone =
    drag !== null && drag.target?.kind === "zone" && drag.target.slotId === stack.id
      ? drag.target.zone
      : null;
  const dropTabIndex =
    drag !== null && drag.target?.kind === "tabbar" && drag.target.slotId === stack.id
      ? drag.target.index
      : null;
  const tabbarRef = useRef<HTMLDivElement | null>(null);
  // Follow the active tab: when it changes, scroll the strip so the tab clears
  // the (sticky) traffic-light reserve on the left and the right edge.
  useLayoutEffect(() => {
    const bar = tabbarRef.current;
    if (bar === null) return;
    const active = bar.querySelector<HTMLElement>(".wb-tab--active");
    if (active === null) return;
    const leftReserve = bar.querySelector<HTMLElement>(".wb-tabbar-inset")?.offsetWidth ?? 0;
    const rightReserve = bar.querySelector<HTMLElement>(".wb-tabbar-end")?.offsetWidth ?? 0;
    const barRect = bar.getBoundingClientRect();
    const tabRect = active.getBoundingClientRect();
    const visibleLeft = barRect.left + leftReserve;
    const visibleRight = barRect.right - rightReserve;
    if (tabRect.left < visibleLeft) bar.scrollLeft -= visibleLeft - tabRect.left;
    else if (tabRect.right > visibleRight) bar.scrollLeft += tabRect.right - visibleRight;
  }, [stack.active, activePanelId]);

  return (
    <section className={`wb-pane${focused ? " wb-pane--focused" : ""}${chromeClass ? ` ${chromeClass}` : ""}`}>
      {(stack.panels.length > 1 || stack.tabsHidden !== true) && (
      <div className="wb-tabbar" data-wb-tabbar data-slot-id={stack.id} ref={tabbarRef}>
        {tree.firstTabInset && tree.leftStackId === stack.id && (
          <div className="wb-tabbar-inset" style={{ width: tree.firstTabInset }} aria-hidden="true" />
        )}
        {stack.panels.map((panelId, i) => {
          const tabPane = runtime.pane(panelId);
          if (tabPane === undefined) {
            // Non-editor panel: title comes from the renderer registry.
            const panel = runtime.panelById(panelId);
            const multiBuffer = runtime.multiBufferPane(panelId);
            const renderer = panel !== undefined ? runtime.panelRenderers.get(panel.kind) : undefined;
            const title =
              multiBuffer !== undefined
                ? multiBuffer.title
                : panel !== undefined && renderer !== undefined
                  ? renderer.title(panel)
                  : "panel";
            const icon =
              multiBuffer !== undefined ? (
                <SearchIcon className="wb-tab-search-icon" aria-hidden="true" />
              ) : panel !== undefined && renderer !== undefined ? (
                renderer.icon?.(panel)
              ) : null;
            return (
              <Fragment key={panelId}>
                {dropTabIndex === i && <div className="wb-tab-insert" />}
                <div
                  className={`wb-tab${i === stack.active ? " wb-tab--active" : ""}`}
                  onPointerDown={(event) => tree.beginTabDrag(event, panelId, stack.id, title)}
                >
                  {icon}
                  <span className="wb-tab-name">{title}</span>
                  <button
                    type="button"
                    className="wb-tab-close"
                    onClick={(event) => {
                      event.stopPropagation();
                      runtime.closeTab(panelId);
                    }}
                  >
                    ×
                  </button>
                </div>
              </Fragment>
            );
          }
          return (
            <Fragment key={panelId}>
              {dropTabIndex === i && <div className="wb-tab-insert" />}
              <Tab
                pane={tabPane}
                active={i === stack.active}
                dragging={drag?.panelId === panelId}
                onPointerDown={(event, title) =>
                  tree.beginTabDrag(event, panelId, stack.id, title)
                }
                onClose={() => runtime.closeTab(panelId)}
              />
            </Fragment>
          );
        })}
        {dropTabIndex === stack.panels.length && <div className="wb-tab-insert" />}
        {tree.trailingAction !== undefined && tree.rightStackId === stack.id && (
          <div className="wb-tabbar-end">{tree.trailingAction}</div>
        )}
      </div>
      )}
      <div
        className="wb-body"
        data-wb-body
        data-slot-id={stack.id}
      >
        {activePanelId !== undefined && <OutPortal node={tree.nodeFor(activePanelId)} />}
        {dropZone !== null && <div className={`wb-drop wb-drop-${dropZone}`} />}
      </div>
    </section>
  );
}

/** Mounts a registered panel renderer for non-editor panels. */
function CustomPanelView({
  runtime,
  panelId,
}: {
  readonly runtime: WorkbenchHost;
  readonly panelId: PanelId;
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const mountRef = useRef<PanelMount | null>(null);
  const [epoch, setEpoch] = useState(0);

  useEffect(() => runtime.panelRenderers.onChanged(() => setEpoch((e) => e + 1)), [runtime]);

  useEffect(() => {
    const element = hostRef.current;
    const panel = runtime.panelById(panelId);
    if (element === null || panel === undefined) return;
    const renderer = runtime.panelRenderers.get(panel.kind);
    if (renderer === undefined) return;
    const handle = renderer.mount(element, panel);
    mountRef.current = handle;
    return () => {
      mountRef.current = null;
      handle.dispose();
    };
  }, [runtime, panelId, epoch]);

  // Register how this panel focuses itself; the runtime's focus authority
  // invokes it whenever this panel becomes the focused pane's active surface,
  // retrying for content (e.g. @pierre/trees rows) that mounts async.
  useEffect(
    () => runtime.registerPanelFocuser(panelId, () => mountRef.current?.focus?.()),
    [runtime, panelId],
  );

  return (
    <div
      ref={hostRef}
      className="wb-custom-panel"
      data-panel-id={panelId}
    />
  );
}

type TabInfo = { readonly name: string; readonly dirty: boolean; readonly conflict: boolean };

function readTabInfo(pane: EditorPane): TabInfo {
  const file = pane.fileInfo();
  const snapshot = pane.snapshot();
  const name =
    file === null ? "untitled" : (file.path.split("/").pop() ?? file.path);
  return {
    name,
    dirty: snapshot.dirty,
    conflict: file !== null && (file.status === "conflict" || file.status === "missing"),
  };
}

function Tab({
  pane,
  active,
  dragging,
  onPointerDown,
  onClose,
}: {
  readonly pane: EditorPane;
  readonly active: boolean;
  readonly dragging: boolean;
  readonly onPointerDown: (event: React.PointerEvent, title: string) => void;
  readonly onClose: () => void;
}) {
  const [info, setInfo] = useState<TabInfo>(() => readTabInfo(pane));

  useEffect(() => {
    setInfo(readTabInfo(pane));
    return pane.subscribe((_notification, source) => {
      if (source !== "document") return;
      const next = readTabInfo(pane);
      setInfo((prev) =>
        prev.name === next.name && prev.dirty === next.dirty && prev.conflict === next.conflict
          ? prev
          : next,
      );
    });
  }, [pane]);

  return (
    <div
      className={`wb-tab${active ? " wb-tab--active" : ""}${dragging ? " wb-tab--dragging" : ""}`}
      data-wb-tab
      onPointerDown={(event) => onPointerDown(event, info.name)}
      onAuxClick={(event) => {
        if (event.button === 1) onClose();
      }}
    >
      {info.name !== "untitled" && <FileIcon fileName={info.name} />}
      <span className={`wb-tab-name${info.conflict ? " wb-tab-name--conflict" : ""}`}>
        {info.name}
      </span>
      {info.dirty && <span className="wb-tab-dot" />}
      <button
        className="wb-tab-close"
        aria-label={`Close ${info.name}`}
        onPointerDown={(event) => event.stopPropagation()}
        onClick={(event) => {
          event.stopPropagation();
          onClose();
        }}
      >
        ×
      </button>
    </div>
  );
}
