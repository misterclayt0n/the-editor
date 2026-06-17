/**
 * The line-number gutter band — the single owner of the gutter's SHAPE: its id,
 * side, render priority, width formula, and cell styling.
 *
 * The editor's `lineNumbers` product package and the multibuffer both build
 * their line-number gutter through this one factory, so a change to the
 * gutter's width or styling mirrors to plain editor views AND multibuffers.
 * Only the LABEL differs — a plain view shows its own line index; a multibuffer
 * maps each row to its source document's line number — so the label is the one
 * thing callers supply.
 */

import type { GutterBandSpec } from "@the-editor/editor";

/** Digit count of the largest line number a gutter must fit. */
export const digits = (n: number): number => String(Math.max(1, n)).length;

export const lineNumberBand = (
  /** Largest line number the band must fit (sets the column width). */
  maxLine: number,
  /** Per display row → the number to show, or null for non-text rows. */
  label: (line: number) => number | null,
): GutterBandSpec => ({
  id: "line-numbers",
  side: "left",
  widthCh: digits(maxLine) + 1,
  priority: 0,
  cell: (line) => {
    const value = label(line);
    return value === null ? null : { text: String(value), className: "gutter-line-number" };
  },
});
