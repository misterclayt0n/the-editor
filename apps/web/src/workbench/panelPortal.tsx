/**
 * Reverse portals: keep a panel's live DOM (and all its state — a terminal's
 * PTY + screen buffer, an editor's scroll offset) alive while it moves between
 * stacks.
 *
 * The problem: panel content rendered inside a stack's React subtree is
 * destroyed and rebuilt when the tab moves to another stack (React reparents
 * across subtrees by unmount + remount). For a terminal that kills the process.
 *
 * The fix: each panel owns ONE persistent container element. Its content is
 * rendered into that detached element (via `createPortal`) from a stable spot
 * at the workbench root, so it never unmounts. A stack renders a lightweight
 * `OutPortal` placeholder that relocates the container into itself with
 * `appendChild` — which MOVES a DOM node, preserving it and everything inside.
 * Switching tabs and moving stacks become the same cheap operation: re-home a
 * live node, never dispose it.
 *
 * This is the `react-reverse-portal` technique, minimized to exactly what the
 * workbench needs (one detached host div per panel, HTML only).
 */

import { useLayoutEffect, useRef } from "react";

/** A panel's persistent container. Lives detached until an OutPortal homes it. */
export type PortalNode = { readonly element: HTMLDivElement };

export function createPortalNode(className: string): PortalNode {
  const element = document.createElement("div");
  element.className = className;
  return { element };
}

/**
 * Homes the node's element into this placeholder. On unmount or when `node`
 * changes, the element is detached (parked) — never destroyed; its content
 * stays alive (rendered elsewhere) until the panel itself closes.
 */
export function OutPortal({ node }: { node: PortalNode }) {
  const hostRef = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const host = hostRef.current;
    if (host === null) return;
    host.appendChild(node.element);
    return () => {
      if (node.element.parentNode === host) host.removeChild(node.element);
    };
  }, [node]);
  return <div ref={hostRef} className="wb-panel-outlet" />;
}
