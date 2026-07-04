/**
 * Reverse portals: keep a panel's live DOM (and all its state — a terminal's
 * PTY + screen buffer, an editor's scroll offset, a diff view's NATIVE scroll
 * position) alive while it moves between stacks.
 *
 * The problem: panel content rendered inside a stack's React subtree is
 * destroyed and rebuilt when the tab moves to another stack (React reparents
 * across subtrees by unmount + remount). For a terminal that kills the process.
 *
 * Three classes of panel state, three mechanisms:
 *
 * 1. JS-owned state (the editor's scroll model, a terminal's screen buffer)
 *    survives anything as long as the React tree lives — so each panel owns
 *    ONE persistent container element, content rendered into it via
 *    `createPortal` from a stable spot at the workbench root; a stack renders
 *    a lightweight `OutPortal` placeholder that relocates the container.
 *    (The `react-reverse-portal` technique, minimized.)
 *
 * 2. Browser state that survives a CONNECTED atomic move (focus, iframe
 *    documents, CSS animations): parked panels live in a hidden but connected
 *    lot and re-homing uses `Element.moveBefore()` (Chromium 133+), with an
 *    appendChild fallback.
 *
 * 3. NATIVE SCROLL OFFSETS — the hard one. A scroll position lives on the
 *    element's box, and the box is destroyed while parked (hidden or
 *    detached alike — `display: none` does NOT preserve it), so no DOM
 *    technique can carry it across a tab switch. This is why the diff view
 *    (a real native scroller + virtualizer, unlike the editor) reset to the
 *    top: the state must be CAPTURED before parking and RESTORED after
 *    homing. Restoration retries over a few frames because a virtualized
 *    panel re-grows its scrollable content after re-attach — each write
 *    clamps to what exists, which also walks the virtualizer toward the
 *    target; once the full offset is reachable it is written once and the
 *    loop stops, so it can never fight the user.
 */

import { useLayoutEffect, useRef } from "react";

/** A panel's persistent container. Lives parked in the connected lot until an
 * OutPortal homes it into a visible stack. */
export type PortalNode = { readonly element: HTMLDivElement };

/** Chromium 133+ atomic move; absent from lib.dom until TS catches up. */
type MovableParent = HTMLElement & {
  readonly moveBefore?: (node: Node, child: Node | null) => void;
};

/**
 * The parking lot: a hidden, document-CONNECTED holding area for panels not
 * currently displayed. `display: none` (not detachment) is the load-bearing
 * choice — a connected element keeps its saved scroll offsets across box
 * destruction, and `moveBefore` requires both endpoints connected.
 */
let lot: HTMLDivElement | null = null;
const parkingLot = (): HTMLDivElement => {
  if (lot === null || !lot.isConnected) {
    lot = document.createElement("div");
    lot.className = "wb-panel-lot";
    lot.style.display = "none";
    lot.setAttribute("aria-hidden", "true");
    document.body.appendChild(lot);
  }
  return lot;
};

/** Re-home `element` under `parent`, preserving browser-owned state (scroll
 * positions, iframes, focus, animations) when the platform can. */
const home = (parent: HTMLElement, element: HTMLElement): void => {
  const movable = parent as MovableParent;
  if (
    typeof movable.moveBefore === "function" &&
    parent.isConnected &&
    element.isConnected &&
    element.ownerDocument === parent.ownerDocument
  ) {
    try {
      movable.moveBefore(element, null);
      return;
    } catch {
      // Invalid hierarchy or transient state: the plain append below still works.
    }
  }
  parent.appendChild(element);
};

export function createPortalNode(className: string): PortalNode {
  const element = document.createElement("div");
  element.className = className;
  // Born connected: state preservation holds from the very first move.
  parkingLot().appendChild(element);
  return { element };
}

/** Remove a closed panel's container from the lot (the lot holds a real DOM
 * reference now, so forgetting this would leak the subtree). */
export function disposePortalNode(node: PortalNode): void {
  cancelRestore.get(node)?.();
  node.element.remove();
}

// ------------------------------------------------- native scroll state (3.)

type ScrollEntry = {
  readonly element: Element;
  readonly top: number;
  readonly left: number;
};

const savedScroll = new WeakMap<PortalNode, ScrollEntry[]>();
const cancelRestore = new WeakMap<PortalNode, () => void>();

/** Every scrolled element under `root`, OPEN SHADOW ROOTS included (the diff
 * view's scroller lives inside @pierre/diffs' shadow DOM). Virtualized
 * panels only keep the visible window in the DOM, so this walk stays small. */
const collectScrolled = (root: ParentNode, out: ScrollEntry[]): void => {
  for (const element of root.querySelectorAll("*")) {
    if (element.scrollTop > 0 || element.scrollLeft > 0) {
      out.push({ element, top: element.scrollTop, left: element.scrollLeft });
    }
    if (element.shadowRoot !== null) collectScrolled(element.shadowRoot, out);
  }
};

/** Snapshot scroll offsets while the panel is still displayed (boxes alive). */
const captureScroll = (node: PortalNode): void => {
  const entries: ScrollEntry[] = [];
  collectScrolled(node.element, entries);
  if (node.element.scrollTop > 0 || node.element.scrollLeft > 0) {
    entries.push({
      element: node.element,
      top: node.element.scrollTop,
      left: node.element.scrollLeft,
    });
  }
  savedScroll.set(node, entries);
};

/** Re-apply captured offsets after homing; see class (3.) in the header. */
const restoreScroll = (node: PortalNode): void => {
  cancelRestore.get(node)?.();
  let pending = (savedScroll.get(node) ?? []).filter((entry) => entry.element.isConnected);
  if (pending.length === 0) return;
  // ~2s of frames: a diff that shrank while hidden stops at its new bottom.
  let attempts = 120;
  let raf = 0;
  const tick = (): void => {
    pending = pending.filter((entry) => {
      const el = entry.element;
      if (!el.isConnected) return false;
      const reachable =
        el.scrollHeight - el.clientHeight + 1 >= entry.top &&
        el.scrollWidth - el.clientWidth + 1 >= entry.left;
      el.scrollTop = entry.top;
      el.scrollLeft = entry.left;
      // Done once the FULL offset was applicable; a clamped write still
      // advanced the virtualizer, so the next frame reaches further.
      return !reachable;
    });
    if (pending.length > 0 && --attempts > 0) {
      raf = requestAnimationFrame(tick);
    }
  };
  tick();
  cancelRestore.set(node, () => cancelAnimationFrame(raf));
};

/**
 * Homes the node's element into this placeholder. On unmount or when `node`
 * changes, the element is PARKED back into the connected lot — never
 * disconnected; its content stays alive (rendered elsewhere) until the panel
 * itself closes.
 */
export function OutPortal({
  node,
  onAttached,
}: {
  readonly node: PortalNode;
  readonly onAttached?: () => void;
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const onAttachedRef = useRef(onAttached);
  onAttachedRef.current = onAttached;

  useLayoutEffect(() => {
    const host = hostRef.current;
    if (host === null) return;
    home(host, node.element);
    restoreScroll(node);
    onAttachedRef.current?.();
    return () => {
      cancelRestore.get(node)?.();
      if (node.element.parentNode === host) {
        // Capture BEFORE parking: offsets are readable only while displayed.
        captureScroll(node);
        home(parkingLot(), node.element);
      }
    };
  }, [node]);
  return <div ref={hostRef} className="wb-panel-outlet" />;
}
