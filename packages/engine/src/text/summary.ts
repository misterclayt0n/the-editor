export type TextSummary = {
  readonly length: number;
  readonly lineBreaks: number;
  readonly firstLineLength: number;
  readonly lastLineLength: number;
  readonly maxLineLength: number;
  readonly startsWithLF: boolean;
  readonly endsWithCR: boolean;
  readonly maxLineLengthBeforeLast: number;
};
export const emptySummary: TextSummary = {
  length: 0,
  lineBreaks: 0,
  firstLineLength: 0,
  lastLineLength: 0,
  maxLineLength: 0,
  startsWithLF: false,
  endsWithCR: false,
  maxLineLengthBeforeLast: 0,
};
const lineLen = (text: string, from: number, to: number): number =>
  to > from && text.charCodeAt(to - 1) === 13 ? to - from - 1 : to - from;
export const summarizeTextRange = (text: string, start: number, end: number): TextSummary => {
  let lineBreaks = 0,
    firstLineLength = end - start,
    lastStart = start,
    maxLineLengthBeforeLast = 0;
  for (let i = start; i < end; i++)
    if (text.charCodeAt(i) === 10) {
      const len = lineLen(text, lastStart, i);
      if (lineBreaks === 0) firstLineLength = len;
      maxLineLengthBeforeLast = Math.max(maxLineLengthBeforeLast, len);
      lineBreaks++;
      lastStart = i + 1;
    }
  const lastLineLength = end - lastStart;
  const maxLineLength = Math.max(maxLineLengthBeforeLast, lastLineLength);
  return {
    length: end - start,
    lineBreaks,
    firstLineLength: lineBreaks === 0 ? end - start : firstLineLength,
    lastLineLength,
    maxLineLength,
    startsWithLF: start < end && text.charCodeAt(start) === 10,
    endsWithCR: start < end && text.charCodeAt(end - 1) === 13,
    maxLineLengthBeforeLast,
  };
};
export const summarizeText = (text: string): TextSummary =>
  summarizeTextRange(text, 0, text.length);
export const addSummary = (a: TextSummary, b: TextSummary): TextSummary => {
  if (a.length === 0)
    return {
      length: b.length,
      lineBreaks: b.lineBreaks,
      firstLineLength: b.firstLineLength,
      lastLineLength: b.lastLineLength,
      maxLineLength: b.maxLineLength,
      startsWithLF: b.startsWithLF,
      endsWithCR: b.endsWithCR,
      maxLineLengthBeforeLast: b.maxLineLengthBeforeLast,
    };
  if (b.length === 0)
    return {
      length: a.length,
      lineBreaks: a.lineBreaks,
      firstLineLength: a.firstLineLength,
      lastLineLength: a.lastLineLength,
      maxLineLength: a.maxLineLength,
      startsWithLF: a.startsWithLF,
      endsWithCR: a.endsWithCR,
      maxLineLengthBeforeLast: a.maxLineLengthBeforeLast,
    };
  const crlfJoin = a.endsWithCR && b.startsWithLF;
  const joinedLine = a.lastLineLength + b.firstLineLength - (crlfJoin ? 1 : 0);
  const lineBreaks = a.lineBreaks + b.lineBreaks;
  const firstLineLength =
    a.lineBreaks === 0 ? a.length + b.firstLineLength - (crlfJoin ? 1 : 0) : a.firstLineLength;
  const lastLineLength = b.lineBreaks === 0 ? a.lastLineLength + b.length : b.lastLineLength;
  const maxLineLengthBeforeLast =
    b.lineBreaks === 0
      ? a.maxLineLengthBeforeLast
      : Math.max(a.maxLineLengthBeforeLast, joinedLine, b.maxLineLengthBeforeLast);
  const maxLineLength = Math.max(
    maxLineLengthBeforeLast,
    lastLineLength,
    b.lineBreaks === 0 ? joinedLine : 0,
  );
  return {
    length: a.length + b.length,
    lineBreaks,
    firstLineLength,
    lastLineLength,
    maxLineLength,
    startsWithLF: a.startsWithLF,
    endsWithCR: b.endsWithCR,
    maxLineLengthBeforeLast,
  };
};
export const countLineBreaks = (text: string): number => summarizeText(text).lineBreaks;
export const lineBreakOffsets = (text: string): readonly number[] => {
  const out: number[] = [];
  for (let i = 0; i < text.length; i++) if (text.charCodeAt(i) === 10) out.push(i);
  return out;
};
