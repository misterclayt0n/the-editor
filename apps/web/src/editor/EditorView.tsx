import { useLayoutEffect, useRef } from "react";
import type { EditorSnapshot } from "@the-editor/editor";
import type { EditorPane } from "../runtime";
import { EditorRenderer } from "./renderer";

type EditorViewProps = {
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
export function EditorView({ pane, onSnapshot }: EditorViewProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const callbacks = useRef({ onSnapshot });
  callbacks.current = { onSnapshot };

  useLayoutEffect(() => {
    const renderer = new EditorRenderer(rootRef.current!, pane, {
      onSnapshot: (snapshot) => callbacks.current.onSnapshot?.(snapshot),
    });
    return () => renderer.dispose();
  }, [pane]);

  return <div ref={rootRef} className="editor" tabIndex={0} />;
}
