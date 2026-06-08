/**
 * Simple editor-cell column helpers.
 *
 * These functions provide a deterministic monospace/editor-cell approximation
 * for tabs, graphemes, and control characters.
 *
 * This is not font measurement. Real pixel measurement depends on renderer
 * technology, font loading, OS/browser shaping, device pixel ratio, ligatures,
 * and font fallback. That belongs in a renderer/host layer.
 */

import { codePointAt, codePointIsZeroWidthWhitespace } from "./chars";

export type DisplayGrapheme =
  | {
      readonly kind: "newline";
      readonly text: string;
      readonly width: 1;
      readonly displayText: " ";
    }
  | {
      readonly kind: "tab";
      readonly text: "\t";
      readonly width: number;
      readonly displayText: string;
    }
  | {
      readonly kind: "other";
      readonly text: string;
      readonly width: number;
      readonly displayText: string;
    };

const assertTabArgs = (visualColumn: number, tabSize: number): void => {
  if (!Number.isInteger(visualColumn) || visualColumn < 0) {
    throw new RangeError("visualColumn must be an integer >= 0");
  }
  if (!Number.isInteger(tabSize) || tabSize <= 0) {
    throw new RangeError("tabSize must be an integer > 0");
  }
};

export function tabWidthAt(visualColumn: number, tabSize: number): number {
  assertTabArgs(visualColumn, tabSize);
  return tabSize - (visualColumn % tabSize);
}

function isAsciiControl(codePoint: number): boolean {
  return (codePoint >= 0x00 && codePoint <= 0x1f) || codePoint === 0x7f;
}

function isCombiningMark(codePoint: number): boolean {
  return (
    (codePoint >= 0x0300 && codePoint <= 0x036f) ||
    (codePoint >= 0x1ab0 && codePoint <= 0x1aff) ||
    (codePoint >= 0x1dc0 && codePoint <= 0x1dff) ||
    (codePoint >= 0x20d0 && codePoint <= 0x20ff) ||
    (codePoint >= 0xfe20 && codePoint <= 0xfe2f)
  );
}

function isEmojiLikeCodePoint(codePoint: number): boolean {
  return (
    (codePoint >= 0x1f000 && codePoint <= 0x1faff) || (codePoint >= 0x2600 && codePoint <= 0x27bf)
  );
}

function isFullWidthCodePoint(codePoint: number): boolean {
  return (
    codePoint >= 0x1100 &&
    (codePoint <= 0x115f ||
      codePoint === 0x2329 ||
      codePoint === 0x232a ||
      (codePoint >= 0x2e80 && codePoint <= 0xa4cf && codePoint !== 0x303f) ||
      (codePoint >= 0xac00 && codePoint <= 0xd7a3) ||
      (codePoint >= 0xf900 && codePoint <= 0xfaff) ||
      (codePoint >= 0xfe10 && codePoint <= 0xfe19) ||
      (codePoint >= 0xfe30 && codePoint <= 0xfe6f) ||
      (codePoint >= 0xff00 && codePoint <= 0xff60) ||
      (codePoint >= 0xffe0 && codePoint <= 0xffe6) ||
      (codePoint >= 0x20000 && codePoint <= 0x3fffd))
  );
}

export function graphemeWidth(grapheme: string): number {
  if (grapheme.length === 0) return 0;
  const cp = codePointAt(grapheme, 0);
  if (cp === undefined) return 0;
  if (codePointIsZeroWidthWhitespace(cp)) return 0;
  if (isCombiningMark(cp)) return 0;
  if (isAsciiControl(cp)) return 2;
  if (isEmojiLikeCodePoint(cp)) return 2;
  if (isFullWidthCodePoint(cp)) return 2;
  return 1;
}

export function displayGrapheme(
  grapheme: string,
  visualColumn: number,
  tabSize: number,
): DisplayGrapheme {
  if (grapheme === "\n" || grapheme === "\r" || grapheme === "\r\n") {
    return { kind: "newline", text: grapheme, width: 1, displayText: " " };
  }
  if (grapheme === "\t") {
    const width = tabWidthAt(visualColumn, tabSize);
    return { kind: "tab", text: "\t", width, displayText: " ".repeat(width) };
  }
  return { kind: "other", text: grapheme, width: graphemeWidth(grapheme), displayText: grapheme };
}
