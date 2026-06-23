/**
 * React shell for a multibuffer pane: mounts the SAME EditorRenderer used for
 * single files (so the cards are a real editor), and portals each excerpt's
 * header card into the block-row container the renderer announces via MountHub.
 *
 * The renderer's imperative DOM lives in an INNER root (a ref'd leaf React never
 * reconciles); the sticky header is a sibling overlay in the outer container, so
 * React managing it never clobbers the renderer's nodes. Header cards re-derive
 * from `pane.metas` on every document notification (fold/expand/edit).
 */

import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { WorkbenchHost } from "../workbench/host";
import { EditorRenderer } from "../editor/renderer";
import type { ExcerptMeta } from "./builder";
import { HeaderCard, type HeaderActions } from "./HeaderCard";
import type { MultiBufferPane } from "./pane";
import { headerId } from "./visuals";

export function MultiBufferView({
  runtime,
  pane,
  focused,
}: {
  runtime: WorkbenchHost;
  pane: MultiBufferPane;
  focused: boolean;
}) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [mounts, setMounts] = useState<ReadonlyMap<string, HTMLElement>>(new Map());
  const [revision, setRevision] = useState(0);
  const [sticky, setSticky] = useState<ExcerptMeta | null>(null);
  const rendererRef = useRef<EditorRenderer | null>(null);

  useLayoutEffect(() => {
    const root = rootRef.current!;
    const renderer = new EditorRenderer(root, pane, { focused });
    rendererRef.current = renderer;
    const unregister = runtime.registerPanelFocuser(pane.panelId, () =>
      root.focus({ preventScroll: true }),
    );
    const unsubscribeMounts = pane.mounts.subscribe((event) => {
      setMounts((prev) => {
        const next = new Map(prev);
        if (event.type === "mounted") next.set(event.id, event.el);
        else next.delete(event.id);
        return next;
      });
    });
    const unsubscribeDoc = pane.subscribe((_notification, source) => {
      if (source === "document") setRevision((value) => value + 1);
    });
    const unsubscribeViewport = pane.onViewport((meta) => {
      setSticky((prev) =>
        prev?.excerpt.id === meta?.excerpt.id && prev?.folded === meta?.folded ? prev : meta,
      );
    });
    return () => {
      unsubscribeMounts();
      unsubscribeDoc();
      unsubscribeViewport();
      unregister();
      rendererRef.current = null;
      renderer.dispose();
    };
  }, [runtime, pane]);

  useLayoutEffect(() => {
    rendererRef.current?.setFocused(focused);
  }, [focused]);

  const actionsFor = (meta: ExcerptMeta): HeaderActions => ({
    onOpen: () => pane.openExcerpt(meta),
    onFold: () => pane.toggleFold(meta.excerpt.documentId),
  });

  const metaByHeader = useMemo(() => {
    const map = new Map<string, ExcerptMeta>();
    for (const meta of pane.metas) map.set(headerId(meta.excerpt.id), meta);
    return map;
    // revision changes whenever the pane rebuilds its metas.
  }, [pane, revision]);

  return (
    <div className="mb-container">
      <div ref={rootRef} className="editor multibuffer" tabIndex={0} />
      {sticky ? (
        <div className="mb-sticky">
          <HeaderCard meta={sticky} workspaceRoot={runtime.workspaceRoot} {...actionsFor(sticky)} />
        </div>
      ) : null}
      {[...mounts].flatMap(([id, el]) => {
        const meta = metaByHeader.get(id);
        if (meta === undefined) return [];
        return [createPortal(<HeaderCard meta={meta} workspaceRoot={runtime.workspaceRoot} {...actionsFor(meta)} />, el, id)];
      })}
    </div>
  );
}
