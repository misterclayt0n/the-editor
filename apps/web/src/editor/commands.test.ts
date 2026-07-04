/**
 * Wrap-aware vertical movement: cursor.up/down move by VISUAL rows using
 * the renderer-published wrap display, computing the exact breaks the
 * renderer paints.
 */

import { describe, expect, test } from "vitest";
import { Selection, TextBuffer } from "@the-editor/engine";
import type { EditorSnapshot } from "@the-editor/editor";
import { moveVertical, type VerticalWrapContext } from "./commands";

// "aaa bbb ccc" wraps at 6 into rows [0,4) "aaa " | [4,8) "bbb " | [8,11) "ccc".
const text = TextBuffer.fromString("aaa bbb ccc\nend\n");
const wrap: VerticalWrapContext = {
  columns: 6,
  tabWidth: 4,
  virtualTextBetween: () => [],
};
const snap = (head: number): EditorSnapshot =>
  ({
    id: "test-view",
    text,
    version: 0,
    dirty: false,
    selection: Selection.point(head),
  }) as unknown as EditorSnapshot;

const headAfter = (from: number, direction: -1 | 1) =>
  moveVertical(snap(from), direction, false, wrap).selection.primary.head;

describe("moveVertical under soft wrap", () => {
  test("down moves one VISUAL row within the wrapped line", () => {
    expect(headAfter(1, 1)).toBe(5); // row 0 col 1 → row 1 col 1
    expect(headAfter(5, 1)).toBe(9); // row 1 col 1 → row 2 col 1
  });

  test("down from the last visual row crosses to the next line", () => {
    expect(headAfter(9, 1)).toBe(13); // "ccc" col 1 → "end" col 1
  });

  test("up from a line's first row enters the previous line's LAST row", () => {
    expect(headAfter(13, -1)).toBe(9); // "end" col 1 → "ccc" col 1
  });

  test("up within the wrapped line", () => {
    expect(headAfter(9, -1)).toBe(5);
    expect(headAfter(5, -1)).toBe(1);
  });

  test("document edges clamp", () => {
    expect(headAfter(1, -1)).toBe(0);
    expect(headAfter(13, 1)).toBe(text.length);
  });

  test("a goal past a shorter row holds one grapheme short of the break", () => {
    // "aaa bbbbbb" at width 6: "aaa " | "bbbbbb". Moving UP from column 5
    // of the long row aims at column 5 of "aaa " — offset 4 is the break
    // (displays on the row below), so the caret holds at offset 3.
    const t = TextBuffer.fromString("aaa bbbbbb\n");
    const s = {
      id: "t2",
      text: t,
      version: 0,
      dirty: false,
      selection: Selection.point(9),
    } as unknown as EditorSnapshot;
    expect(moveVertical(s, -1, false, wrap).selection.primary.head).toBe(3);
  });

  test("without wrap context movement stays buffer-line based", () => {
    expect(moveVertical(snap(1), 1, false, null).selection.primary.head).toBe(13);
  });
});
