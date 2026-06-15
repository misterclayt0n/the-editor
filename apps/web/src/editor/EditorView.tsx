import { useLayoutEffect, useRef } from "react";
import type { EditorSnapshot } from "@the-editor/editor";
import type { AppRuntime, EditorPane } from "../runtime";
import { EditorRenderer } from "./renderer";

type EditorViewProps = {
  readonly runtime: AppRuntime;
  readonly pane: EditorPane;
  readonly onSnapshot?: (snapshot: EditorSnapshot) => void;
};

/**
 * Thin React shell around EditorRenderer, one per visible pane. React owns
 * the mount point and nothing else: scrolling, typing, and painting all
 * happen in the renderer's own frame loop, outside React reconciliation.
 * Keyboard input is NOT handled here — one document-level dispatcher in the
 * runtime routes every keystroke (so every surface is keyboard-driven).
 */
export function EditorView({ runtime, pane, onSnapshot }: EditorViewProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const callbacks = useRef({ onSnapshot });
  callbacks.current = { onSnapshot };

  useLayoutEffect(() => {
    const root = rootRef.current!;
    const renderer = new EditorRenderer(root, pane, {
      onSnapshot: (snapshot) => callbacks.current.onSnapshot?.(snapshot),
    });
    const unregister = runtime.registerPanelFocuser(pane.panelId, () =>
      root.focus({ preventScroll: true }),
    );
    return () => {
      unregister();
      renderer.dispose();
    };
  }, [runtime, pane]);

  return <div ref={rootRef} className="editor" tabIndex={0} />;
}
