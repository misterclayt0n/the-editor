/**
 * Pane interaction as a FAN-OUT hub: many subscribers, not one slot. Lives in
 * its own module (no runtime imports) so panes can construct a hub without
 * dragging the whole AppRuntime — and its heavy LSP/process deps — into their
 * bundle. `runtime.ts` re-exports these for the rest of the app.
 *
 * `IntelController` is one subscriber; an extension reacting to hover/scroll is
 * another; a pane's own behavior (multibuffer nav) is another. Boolean events
 * (`keyIntercept`, `gutterClick`) try subscribers newest-first — later
 * registration wins, matching the keymap resolver — and stop at the first that
 * returns true; void events notify every subscriber. Each `on*` returns an
 * unsubscribe.
 */

import type { KeyboardEventLike } from "@the-editor/keymaps";

export interface PaneInteraction {
  onKeyIntercept(fn: (event: KeyboardEventLike) => boolean): () => void;
  onPointerHover(fn: (offset: number | null) => void): () => void;
  onPointerScrolled(fn: () => void): () => void;
  onViewportChanged(fn: (topLine: number, startLine: number, endLine: number) => void): () => void;
  onGutterClick(fn: (line: number) => boolean): () => void;
}

/** The producer half: the renderer emits into the hub. */
export interface InteractionDispatch {
  keyIntercept(event: KeyboardEventLike): boolean;
  pointerHover(offset: number | null): void;
  pointerScrolled(): void;
  viewportChanged(topLine: number, startLine: number, endLine: number): void;
  gutterClick(line: number): boolean;
}

export function makeInteractionHub(): {
  readonly interaction: PaneInteraction;
  readonly dispatch: InteractionDispatch;
} {
  const key = new Set<(event: KeyboardEventLike) => boolean>();
  const hover = new Set<(offset: number | null) => void>();
  const scrolled = new Set<() => void>();
  const viewport = new Set<(top: number, start: number, end: number) => void>();
  const gutter = new Set<(line: number) => boolean>();
  const add = <T>(set: Set<T>, fn: T): (() => void) => {
    set.add(fn);
    return () => void set.delete(fn);
  };
  return {
    interaction: {
      onKeyIntercept: (fn) => add(key, fn),
      onPointerHover: (fn) => add(hover, fn),
      onPointerScrolled: (fn) => add(scrolled, fn),
      onViewportChanged: (fn) => add(viewport, fn),
      onGutterClick: (fn) => add(gutter, fn),
    },
    dispatch: {
      keyIntercept: (event) => {
        for (const fn of [...key].reverse()) if (fn(event)) return true;
        return false;
      },
      pointerHover: (offset) => {
        for (const fn of hover) fn(offset);
      },
      pointerScrolled: () => {
        for (const fn of scrolled) fn();
      },
      viewportChanged: (top, start, end) => {
        for (const fn of viewport) fn(top, start, end);
      },
      gutterClick: (line) => {
        for (const fn of [...gutter].reverse()) if (fn(line)) return true;
        return false;
      },
    },
  };
}
