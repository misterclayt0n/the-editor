/**
 * Unicode code point helpers for JavaScript strings.
 *
 * Offsets are UTF-16 code unit offsets, matching JavaScript strings and
 * TextBuffer offsets.
 *
 * These helpers avoid splitting surrogate pairs, but they do not implement
 * grapheme cluster traversal. A future grapheme module should handle
 * user-visible character movement.
 */

export type CharCategory = "whitespace" | "eol" | "word" | "punctuation" | "unknown";
export type BreakingBehavior = "breaking" | "nonBreaking";
export type WhitespaceWidth = "zero" | "single" | "wide" | "variable";
export type WhitespaceProperties = {
  readonly breaking: BreakingBehavior;
  readonly width: WhitespaceWidth;
};

const HIGH_SURROGATE_START = 0xd800;
const HIGH_SURROGATE_END = 0xdbff;
const LOW_SURROGATE_START = 0xdc00;
const LOW_SURROGATE_END = 0xdfff;
const LF = 0x0a;
const CR = 0x0d;
const UNDERSCORE = 0x5f;

const letterOrNumberRe = /^[\p{L}\p{N}]$/u;
const punctuationRe = /^\p{P}$/u;
const punctuationSymbolRe = /^[\p{Sm}\p{Sc}\p{Sk}]$/u;

function assertOffset(text: string, offset: number): void {
  if (!Number.isInteger(offset) || offset < 0 || offset > text.length) {
    throw new RangeError(`offset ${offset} is out of bounds for string length ${text.length}`);
  }
}

export function isHighSurrogateCodeUnit(codeUnit: number): boolean {
  return codeUnit >= HIGH_SURROGATE_START && codeUnit <= HIGH_SURROGATE_END;
}

export function isLowSurrogateCodeUnit(codeUnit: number): boolean {
  return codeUnit >= LOW_SURROGATE_START && codeUnit <= LOW_SURROGATE_END;
}

export function isSurrogateCodeUnit(codeUnit: number): boolean {
  return isHighSurrogateCodeUnit(codeUnit) || isLowSurrogateCodeUnit(codeUnit);
}

/**
 * Computes a Unicode code point from a UTF-16 surrogate pair.
 *
 * This is a low-level helper and does not validate that the inputs are
 * actually high/low surrogate code units. Use `isHighSurrogateCodeUnit` and
 * `isLowSurrogateCodeUnit` when validating external input.
 */
export function computeCodePoint(highSurrogate: number, lowSurrogate: number): number {
  return (
    ((highSurrogate - HIGH_SURROGATE_START) << 10) + (lowSurrogate - LOW_SURROGATE_START) + 0x10000
  );
}

export function isValidCodePoint(codePoint: number): boolean {
  return Number.isInteger(codePoint) && codePoint >= 0 && codePoint <= 0x10ffff;
}

function isCodePointBoundaryUnchecked(text: string, offset: number): boolean {
  return !(
    offset > 0 &&
    offset < text.length &&
    isLowSurrogateCodeUnit(text.charCodeAt(offset)) &&
    isHighSurrogateCodeUnit(text.charCodeAt(offset - 1))
  );
}

export function isCodePointBoundary(text: string, offset: number): boolean {
  assertOffset(text, offset);
  return isCodePointBoundaryUnchecked(text, offset);
}

export function snapToCodePointBoundaryBefore(text: string, offset: number): number {
  assertOffset(text, offset);
  return isCodePointBoundaryUnchecked(text, offset) ? offset : offset - 1;
}

export function snapToCodePointBoundaryAfter(text: string, offset: number): number {
  assertOffset(text, offset);
  return isCodePointBoundaryUnchecked(text, offset) ? offset : offset + 1;
}

export function previousCodePointBoundary(text: string, offset: number): number {
  assertOffset(text, offset);
  if (!isCodePointBoundaryUnchecked(text, offset))
    return snapToCodePointBoundaryBefore(text, offset);
  if (offset === 0) return 0;
  return snapToCodePointBoundaryBefore(text, offset - 1);
}

export function nextCodePointBoundary(text: string, offset: number): number {
  assertOffset(text, offset);
  if (!isCodePointBoundaryUnchecked(text, offset))
    return snapToCodePointBoundaryAfter(text, offset);
  if (offset === text.length) return text.length;
  const next =
    offset +
    (isHighSurrogateCodeUnit(text.charCodeAt(offset)) &&
    offset + 1 < text.length &&
    isLowSurrogateCodeUnit(text.charCodeAt(offset + 1))
      ? 2
      : 1);
  return Math.min(next, text.length);
}

export function codePointAt(text: string, offset: number): number | undefined {
  const boundary = snapToCodePointBoundaryBefore(text, offset);
  if (boundary === text.length) return undefined;
  const first = text.charCodeAt(boundary);
  if (isHighSurrogateCodeUnit(first) && boundary + 1 < text.length) {
    const second = text.charCodeAt(boundary + 1);
    if (isLowSurrogateCodeUnit(second)) return computeCodePoint(first, second);
  }
  return first;
}

export function codePointBefore(text: string, offset: number): number | undefined {
  assertOffset(text, offset);
  if (offset === 0) return undefined;
  const boundary = snapToCodePointBoundaryAfter(text, offset);
  const start = previousCodePointBoundary(text, boundary);
  return codePointAt(text, start);
}

export function codePointStringAt(text: string, offset: number): string | undefined {
  const cp = codePointAt(text, offset);
  return cp === undefined ? undefined : String.fromCodePoint(cp);
}

export function codePointStringBefore(text: string, offset: number): string | undefined {
  const cp = codePointBefore(text, offset);
  return cp === undefined ? undefined : String.fromCodePoint(cp);
}

export function codePointIsLineEnding(codePoint: number): boolean {
  return isValidCodePoint(codePoint) && (codePoint === LF || codePoint === CR);
}

export function codePointWhitespaceProperties(codePoint: number): WhitespaceProperties | undefined {
  if (!isValidCodePoint(codePoint)) return undefined;
  switch (codePoint) {
    case 0x0009:
      return { breaking: "breaking", width: "variable" };
    case 0x0020:
      return { breaking: "breaking", width: "single" };
    case 0x00a0:
      return { breaking: "nonBreaking", width: "single" };
    case 0x180e:
      return { breaking: "nonBreaking", width: "zero" };
    case 0x2000:
    case 0x2001:
    case 0x2002:
    case 0x2003:
      return { breaking: "breaking", width: "wide" };
    case 0x2004:
    case 0x2005:
    case 0x2006:
      return { breaking: "breaking", width: "single" };
    case 0x2007:
      return { breaking: "nonBreaking", width: "single" };
    case 0x2008:
    case 0x2009:
    case 0x200a:
      return { breaking: "breaking", width: "single" };
    case 0x200b:
      return { breaking: "breaking", width: "zero" };
    case 0x202f:
      return { breaking: "nonBreaking", width: "single" };
    case 0x205f:
      return { breaking: "breaking", width: "single" };
    case 0x3000:
      return { breaking: "breaking", width: "wide" };
    case 0xfeff:
      return { breaking: "nonBreaking", width: "zero" };
    default:
      return undefined;
  }
}

export function codePointIsWhitespace(codePoint: number): boolean {
  return codePointWhitespaceProperties(codePoint) !== undefined;
}

export function codePointIsWord(codePoint: number): boolean {
  return (
    isValidCodePoint(codePoint) &&
    (codePoint === UNDERSCORE || letterOrNumberRe.test(String.fromCodePoint(codePoint)))
  );
}

export function codePointIsPunctuation(codePoint: number): boolean {
  if (!isValidCodePoint(codePoint)) return false;
  const ch = String.fromCodePoint(codePoint);
  return punctuationRe.test(ch) || punctuationSymbolRe.test(ch);
}

export function categorizeCodePoint(codePoint: number): CharCategory {
  if (!isValidCodePoint(codePoint)) return "unknown";
  if (codePointIsLineEnding(codePoint)) return "eol";
  if (codePointIsWhitespace(codePoint)) return "whitespace";
  if (codePointIsWord(codePoint)) return "word";
  if (codePointIsPunctuation(codePoint)) return "punctuation";
  return "unknown";
}

const firstCodePoint = (ch: string): number | undefined =>
  ch.length === 0 ? undefined : ch.codePointAt(0);

export function categorizeChar(ch: string): CharCategory {
  const cp = firstCodePoint(ch);
  return cp === undefined ? "unknown" : categorizeCodePoint(cp);
}

export function charIsLineEnding(ch: string): boolean {
  const cp = firstCodePoint(ch);
  return cp !== undefined && codePointIsLineEnding(cp);
}

export function charIsWord(ch: string): boolean {
  const cp = firstCodePoint(ch);
  return cp !== undefined && codePointIsWord(cp);
}

export function charIsWhitespace(ch: string): boolean {
  const cp = firstCodePoint(ch);
  return cp !== undefined && codePointIsWhitespace(cp);
}

export function charIsPunctuation(ch: string): boolean {
  const cp = firstCodePoint(ch);
  return cp !== undefined && codePointIsPunctuation(cp);
}

export function whitespaceProperties(ch: string): WhitespaceProperties | undefined {
  const cp = firstCodePoint(ch);
  return cp === undefined ? undefined : codePointWhitespaceProperties(cp);
}

export function codePointIsBreakingWhitespace(codePoint: number): boolean {
  return codePointWhitespaceProperties(codePoint)?.breaking === "breaking";
}

export function codePointIsNonBreakingWhitespace(codePoint: number): boolean {
  return codePointWhitespaceProperties(codePoint)?.breaking === "nonBreaking";
}

export function codePointIsZeroWidthWhitespace(codePoint: number): boolean {
  return codePointWhitespaceProperties(codePoint)?.width === "zero";
}

export function charIsBreakingWhitespace(ch: string): boolean {
  const cp = firstCodePoint(ch);
  return cp !== undefined && codePointIsBreakingWhitespace(cp);
}

export function charIsNonBreakingWhitespace(ch: string): boolean {
  const cp = firstCodePoint(ch);
  return cp !== undefined && codePointIsNonBreakingWhitespace(cp);
}

export function charIsZeroWidthWhitespace(ch: string): boolean {
  const cp = firstCodePoint(ch);
  return cp !== undefined && codePointIsZeroWidthWhitespace(cp);
}
