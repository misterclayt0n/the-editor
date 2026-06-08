import type { TextSummary } from "./summary";

export type Block = {
  readonly text: string;
  readonly lineBreaks: readonly number[];
  readonly summary: TextSummary;
  /** Content length for each line segment. Includes the final unterminated line. CR before LF is excluded. */
  readonly lineLengths: readonly number[];
  /** Inclusive prefix max over lineLengths: prefixMaxLineLengths[i] = max(lineLengths[0..i]). */
  readonly prefixMaxLineLengths: readonly number[];
};
const lineLen = (text: string, from: number, to: number): number =>
  to > from && text.charCodeAt(to - 1) === 13 ? to - from - 1 : to - from;
export const makeBlock = (text: string): Block => {
  const lineBreaks: number[] = [];
  const lineLengths: number[] = [];
  const prefixMaxLineLengths: number[] = [];
  let firstLineLength = text.length,
    lastStart = 0,
    max = 0;
  const pushLine = (length: number): void => {
    lineLengths.push(length);
    max = Math.max(max, length);
    prefixMaxLineLengths.push(max);
  };
  for (let i = 0; i < text.length; i++)
    if (text.charCodeAt(i) === 10) {
      const len = lineLen(text, lastStart, i);
      if (lineBreaks.length === 0) firstLineLength = len;
      pushLine(len);
      lineBreaks.push(i);
      lastStart = i + 1;
    }
  const lastLineLength = text.length - lastStart;
  pushLine(lastLineLength);
  const summary: TextSummary = {
    length: text.length,
    lineBreaks: lineBreaks.length,
    firstLineLength: lineBreaks.length === 0 ? text.length : firstLineLength,
    lastLineLength,
    maxLineLength: max,
    startsWithLF: text.length > 0 && text.charCodeAt(0) === 10,
    endsWithCR: text.length > 0 && text.charCodeAt(text.length - 1) === 13,
    maxLineLengthBeforeLast:
      lineBreaks.length === 0 ? 0 : (prefixMaxLineLengths[prefixMaxLineLengths.length - 2] ?? 0),
  };
  return Object.freeze({ text, lineBreaks, summary, lineLengths, prefixMaxLineLengths });
};
