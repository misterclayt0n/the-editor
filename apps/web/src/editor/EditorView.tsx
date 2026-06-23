import { useLayoutEffect, useRef } from "react";
import type { EditorSnapshot } from "@the-editor/editor";
import type { EditorPane } from "../runtime";
import type { WorkbenchHost } from "../workbench/host";
import { EditorRenderer } from "./renderer";

type EditorViewProps = {
  readonly runtime: WorkbenchHost;
  readonly pane: EditorPane;
  readonly focused: boolean;
  readonly onSnapshot?: (snapshot: EditorSnapshot) => void;
};

/**
 * Thin React shell around EditorRenderer, one per visible pane. React owns
 * the mount point and nothing else: scrolling, typing, and painting all
 * happen in the renderer's own frame loop, outside React reconciliation.
 * Keyboard input is NOT handled here — one document-level dispatcher in the
 * runtime routes every keystroke (so every surface is keyboard-driven).
 */
export function EditorView({ runtime, pane, focused, onSnapshot }: EditorViewProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const callbacks = useRef({ onSnapshot });
  const rendererRef = useRef<EditorRenderer | null>(null);
  callbacks.current = { onSnapshot };

  useLayoutEffect(() => {
    const root = rootRef.current!;
    const renderer = new EditorRenderer(root, pane, {
      focused,
      onSnapshot: (snapshot) => callbacks.current.onSnapshot?.(snapshot),
    });
    rendererRef.current = renderer;
    const unregister = runtime.registerPanelFocuser(pane.panelId, () =>
      root.focus({ preventScroll: true }),
    );
    return () => {
      unregister();
      rendererRef.current = null;
      renderer.dispose();
    };
  }, [runtime, pane]);

  useLayoutEffect(() => {
    rendererRef.current?.setFocused(focused);
  }, [focused]);

  return <div ref={rootRef} className="editor" tabIndex={0} />;
}
