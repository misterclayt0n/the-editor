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
import { graphemeSegments } from "./grapheme";
import type { SyntaxText } from "@the-editor/syntax";
import type { TextBuffer } from "./text/buffer";

export type ColumnText = SyntaxText | TextBuffer;

export type OffsetBias = "before" | "after";

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

function assertColumn(column: number): void {
  if (!Number.isInteger(column) || column < 0) {
    throw new RangeError("column must be an integer >= 0");
  }
}

function isTextBuffer(text: ColumnText): text is TextBuffer {
  return "lineCount" in text && typeof (text as TextBuffer).lineRange === "function";
}

function lineCount(text: ColumnText): number {
  if (isTextBuffer(text)) return text.lineCount;
  let count = 1;
  for (let i = 0; i < text.length; i++) if (text.sliceString(i, i + 1) === "\n") count++;
  return count;
}

function lineRange(text: ColumnText, line: number): { readonly from: number; readonly to: number } {
  const count = lineCount(text);
  if (!Number.isInteger(line) || line < 0 || line >= count) {
    throw new RangeError(`line ${line} is out of bounds for ${count} lines`);
  }
  if (isTextBuffer(text)) {
    const range = text.lineRange(line);
    return { from: range.from, to: range.to };
  }
  let current = 0;
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    if (text.sliceString(i, i + 1) !== "\n") continue;
    if (current === line) {
      const to = i > start && text.sliceString(i - 1, i) === "\r" ? i - 1 : i;
      return { from: start, to };
    }
    current++;
    start = i + 1;
  }
  return { from: start, to: text.length };
}

function lineIndexAtOffset(text: ColumnText, offset: number): number {
  if (!Number.isInteger(offset) || offset < 0 || offset > text.length) {
    throw new RangeError(`offset ${offset} is out of bounds for text length ${text.length}`);
  }
  if (isTextBuffer(text)) return text.lineRangeAt(offset).index;
  let line = 0;
  for (let i = 0; i < offset; i++) if (text.sliceString(i, i + 1) === "\n") line++;
  return line;
}

export function columnAtOffset(args: {
  readonly text: ColumnText;
  readonly offset: number;
  readonly tabWidth: number;
  readonly line?: number;
}): number {
  assertTabArgs(0, args.tabWidth);
  const line = args.line ?? lineIndexAtOffset(args.text, args.offset);
  const range = lineRange(args.text, line);
  const end = Math.max(range.from, Math.min(args.offset, range.to));
  const lineText = args.text.sliceString(range.from, end);
  let column = 0;
  for (const segment of graphemeSegments(lineText)) {
    column += displayGrapheme(segment.text, column, args.tabWidth).width;
  }
  return column;
}

export function offsetAtColumn(args: {
  readonly text: ColumnText;
  readonly line: number;
  readonly column: number;
  readonly tabWidth: number;
  readonly bias?: OffsetBias;
}): number {
  assertTabArgs(0, args.tabWidth);
  assertColumn(args.column);
  const bias = args.bias ?? "before";
  const range = lineRange(args.text, args.line);
  const lineText = args.text.sliceString(range.from, range.to);
  let visualColumn = 0;
  for (const segment of graphemeSegments(lineText)) {
    const width = displayGrapheme(segment.text, visualColumn, args.tabWidth).width;
    const nextColumn = visualColumn + width;
    if (args.column < nextColumn) {
      if (args.column === visualColumn) return range.from + segment.from;
      return bias === "after" ? range.from + segment.to : range.from + segment.from;
    }
    if (args.column === nextColumn) return range.from + segment.to;
    visualColumn = nextColumn;
  }
  return range.to;
}
