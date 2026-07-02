/**
 * Pane interaction as a FAN-OUT hub, exposed on the view context so it's a
 * public, view-scoped seam: many subscribers, not one slot. `IntelController`
 * is one subscriber; an extension reacting to hover/scroll (via a decoration
 * provider's `attachView`, which carries `interaction`) is another; a pane's
 * own behavior (multibuffer nav) is another.
 *
 * Boolean events (`keyIntercept`, `gutterClick`) try subscribers newest-first —
 * later registration wins, matching the keymap resolver — and stop at the first
 * that returns true; void events notify every subscriber. Each `on*` returns an
 * unsubscribe. `PaneKeyEvent` mirrors `@the-editor/keymaps`' `KeyboardEventLike`
 * structurally so the editor package needn't depend on keymaps.
 */

import { Context } from "effect";

export type PaneKeyEvent = {
  readonly key: string;
  readonly ctrlKey: boolean;
  readonly altKey: boolean;
  readonly shiftKey: boolean;
  readonly metaKey: boolean;
};

export interface PaneInteraction {
  onKeyIntercept(fn: (event: PaneKeyEvent) => boolean): () => void;
  onPointerHover(fn: (offset: number | null) => void): () => void;
  onPointerScrolled(fn: () => void): () => void;
  onViewportChanged(fn: (topLine: number, startLine: number, endLine: number) => void): () => void;
  onGutterClick(fn: (line: number) => boolean): () => void;
}

/** The producer half: the renderer emits into the hub. */
export interface InteractionDispatch {
  keyIntercept(event: PaneKeyEvent): boolean;
  pointerHover(offset: number | null): void;
  pointerScrolled(): void;
  viewportChanged(topLine: number, startLine: number, endLine: number): void;
  gutterClick(line: number): boolean;
}

export function makeInteractionHub(): {
  readonly interaction: PaneInteraction;
  readonly dispatch: InteractionDispatch;
} {
  const key = new Set<(event: PaneKeyEvent) => boolean>();
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

/** The view's interaction hub, on the view context — read by the pane (to
 *  dispatch) and by decoration providers' `attachView` (to subscribe). */
export class ViewInteraction extends Context.Tag("@the-editor/editor/ViewInteraction")<
  ViewInteraction,
  { readonly interaction: PaneInteraction; readonly dispatch: InteractionDispatch }
>() {}
