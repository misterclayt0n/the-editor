/**
 * ViewCursor: the cursor contract between editor behavior and the renderer.
 *
 * The renderer owns geometry/lifecycle: where the cursor is, how it scrolls,
 * when it is visible, and its z-order relative to selections/decorations.
 * Packages/extensions own policy/flair: bar/block/underline, blink, classes,
 * color, and transient effects mounted inside the renderer-owned cursor box.
 *
 * This deliberately is NOT a generic cursor package and it does NOT expose the
 * renderer's internal DOM node. One owner for geometry; one seam for styling.
 */

import { Context } from "effect";

export type CursorShape = "bar" | "block" | "underline" | "hidden";

export type CursorStyle = {
  readonly shape: CursorShape;
  /** Defaults to true for the active cursor; idle focus still disables blink. */
  readonly blink?: boolean | undefined;
  /** CSS color/token for the cursor core, e.g. "var(--theme-text-accent)". */
  readonly color?: string | undefined;
  /** Extra class on the renderer-owned cursor wrapper. */
  readonly className?: string | undefined;
  /** Bar/underline thickness in CSS pixels. */
  readonly widthPx?: number | undefined;
  /** Block/underline width in editor cells. Defaults to 1. */
  readonly widthColumns?: number | undefined;
  /**
   * Optional persistent custom renderer for the cursor core. The element is
   * inside the renderer-positioned cursor wrapper and is re-rendered when the
   * caret/style changes. Keep it idempotent; throw-safe rendering is enforced
   * by the web renderer.
   */
  readonly render?: ((element: HTMLElement, placement: CursorPlacement) => void) | undefined;
};

export const DEFAULT_CURSOR_STYLE: CursorStyle = { shape: "bar", blink: true };

export type CursorPlacement = {
  readonly offset: number;
  /** Window-local rectangle of the renderer-owned cursor wrapper. */
  readonly rect: {
    readonly top: number;
    readonly left: number;
    readonly width: number;
    readonly height: number;
  };
  readonly lineHeight: number;
  readonly charWidth: number;
  readonly focused: boolean;
  readonly style: CursorStyle;
};

export type CursorEffect = {
  /** Added to the transient effect element before render() runs. */
  readonly className?: string | undefined;
  /** Element lifetime. Defaults to 500ms. */
  readonly ttlMs?: number | undefined;
  /** Optional custom DOM fill. The element is inside the cursor box. */
  readonly render?: ((element: HTMLElement, placement: CursorPlacement) => void) | undefined;
};

export type CursorListener = () => void;

export class CursorController {
  private style: CursorStyle = DEFAULT_CURSOR_STYLE;
  private readonly effects: CursorEffect[] = [];
  private readonly listeners = new Set<CursorListener>();

  getStyle(): CursorStyle {
    return this.style;
  }

  setStyle(style: CursorStyle): void {
    this.style = normalizeStyle(style);
    this.notify();
  }

  updateStyle(fn: (prev: CursorStyle) => CursorStyle): void {
    this.setStyle(fn(this.style));
  }

  resetStyle(): void {
    this.setStyle(DEFAULT_CURSOR_STYLE);
  }

  /** Extension/product side: enqueue a one-shot visual effect. */
  spawnEffect(effect: CursorEffect): void {
    this.effects.push(effect);
    this.notify();
  }

  /** Renderer side: drain pending effects exactly once. */
  drainEffects(): readonly CursorEffect[] {
    if (this.effects.length === 0) return [];
    return this.effects.splice(0, this.effects.length);
  }

  subscribe(listener: CursorListener): () => void {
    this.listeners.add(listener);
    return () => void this.listeners.delete(listener);
  }

  private notify(): void {
    for (const listener of this.listeners) listener();
  }
}

function normalizeStyle(style: CursorStyle): CursorStyle {
  return {
    ...style,
    shape: style.shape,
    ...(style.blink === undefined ? { blink: true } : {}),
  };
}

export class ViewCursor extends Context.Tag("@the-editor/editor/ViewCursor")<
  ViewCursor,
  CursorController
>() {}

export function makeViewCursor(): CursorController {
  return new CursorController();
}
