import { useLayoutEffect, useRef } from "react";
import type { EditorSnapshot } from "@the-editor/editor";
import type { Chord } from "@the-editor/keymaps";
import type { AppRuntime } from "../runtime";
import { EditorRenderer } from "./renderer";

type EditorViewProps = {
  readonly runtime: AppRuntime;
  readonly onSnapshot?: (snapshot: EditorSnapshot) => void;
  readonly onPending?: (chords: ReadonlyArray<Chord>) => void;
};

/**
 * Thin React shell around EditorRenderer. React owns the mount point and
 * nothing else: scrolling, typing, and painting all happen in the renderer's
 * own frame loop, outside React reconciliation.
 */
export function EditorView({ runtime, onSnapshot, onPending }: EditorViewProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const callbacks = useRef({ onSnapshot, onPending });
  callbacks.current = { onSnapshot, onPending };

  useLayoutEffect(() => {
    const renderer = new EditorRenderer(rootRef.current!, runtime, {
      onSnapshot: (snapshot) => callbacks.current.onSnapshot?.(snapshot),
      onPending: (chords) => callbacks.current.onPending?.(chords),
    });
    return () => renderer.dispose();
  }, [runtime]);

  return <div ref={rootRef} className="editor" tabIndex={0} />;
}
