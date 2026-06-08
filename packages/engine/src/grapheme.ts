/**
 * Grapheme cluster helpers for JavaScript strings.
 *
 * Offsets are UTF-16 code unit offsets, matching JavaScript strings and
 * TextBuffer offsets.
 *
 * These helpers operate on user-visible grapheme cluster boundaries. They are
 * higher-level than `chars.ts`, which only handles Unicode code points and
 * surrogate pairs.
 *
 * Visual width, tab stops, cursor columns, wrapping, and rendering are not
 * handled here. See `columns.ts` for simple editor-cell approximations.
 */

import Graphemer from "graphemer";

export type GraphemeSegment = {
  readonly from: number;
  readonly to: number;
  readonly text: string;
};

const splitter = new Graphemer();

function assertOffset(text: string, offset: number): void {
  if (!Number.isInteger(offset) || offset < 0 || offset > text.length) {
    throw new RangeError(`offset ${offset} is out of bounds for string length ${text.length}`);
  }
}

function assertCount(count: number): void {
  if (!Number.isInteger(count) || count < 0) {
    throw new RangeError(`count ${count} must be a non-negative integer`);
  }
}

// These helpers operate on strings and intentionally keep no cache.
// Hot movement over TextBuffer lines can later cache per-line grapheme
// boundaries if profiling shows a need.
export function graphemeSegments(text: string): readonly GraphemeSegment[] {
  const segments: GraphemeSegment[] = [];
  let offset = 0;
  for (const segment of splitter.iterateGraphemes(text)) {
    const from = offset;
    const to = from + segment.length;
    segments.push({ from, to, text: segment });
    offset = to;
  }
  return segments;
}

export function* iterateGraphemes(text: string): Iterable<GraphemeSegment> {
  yield* graphemeSegments(text);
}

function containingOrStartingSegment(text: string, offset: number): GraphemeSegment | undefined {
  for (const segment of graphemeSegments(text)) {
    if (offset >= segment.from && offset < segment.to) return segment;
  }
  return undefined;
}

export function isGraphemeBoundary(text: string, offset: number): boolean {
  assertOffset(text, offset);
  if (offset === 0 || offset === text.length) return true;
  return graphemeSegments(text).some((segment) => segment.from === offset || segment.to === offset);
}

export function snapToGraphemeBoundaryBefore(text: string, offset: number): number {
  assertOffset(text, offset);
  if (offset === 0 || (offset === text.length && isGraphemeBoundary(text, offset))) return offset;
  if (isGraphemeBoundary(text, offset)) return offset;
  return containingOrStartingSegment(text, offset)?.from ?? offset;
}

export function snapToGraphemeBoundaryAfter(text: string, offset: number): number {
  assertOffset(text, offset);
  if (isGraphemeBoundary(text, offset)) return offset;
  return containingOrStartingSegment(text, offset)?.to ?? offset;
}

export function previousGraphemeBoundary(text: string, offset: number): number {
  assertOffset(text, offset);
  if (!isGraphemeBoundary(text, offset)) return snapToGraphemeBoundaryBefore(text, offset);
  let previous = 0;
  for (const segment of graphemeSegments(text)) {
    if (segment.to === offset) return segment.from;
    if (segment.from >= offset) return previous;
    previous = segment.from;
  }
  return previous;
}

export function nextGraphemeBoundary(text: string, offset: number): number {
  assertOffset(text, offset);
  if (!isGraphemeBoundary(text, offset)) return snapToGraphemeBoundaryAfter(text, offset);
  for (const segment of graphemeSegments(text)) {
    if (segment.from >= offset) return segment.to;
  }
  return text.length;
}

export function nthPreviousGraphemeBoundary(text: string, offset: number, count: number): number {
  assertCount(count);
  let current = offset;
  assertOffset(text, current);
  for (let i = 0; i < count; i++) current = previousGraphemeBoundary(text, current);
  return current;
}

export function nthNextGraphemeBoundary(text: string, offset: number, count: number): number {
  assertCount(count);
  let current = offset;
  assertOffset(text, current);
  for (let i = 0; i < count; i++) current = nextGraphemeBoundary(text, current);
  return current;
}

export function graphemeAt(text: string, offset: number): GraphemeSegment | undefined {
  assertOffset(text, offset);
  if (offset === text.length) return undefined;
  return containingOrStartingSegment(text, offset);
}

export function graphemeBefore(text: string, offset: number): GraphemeSegment | undefined {
  assertOffset(text, offset);
  if (offset === 0) return undefined;
  if (!isGraphemeBoundary(text, offset)) return containingOrStartingSegment(text, offset);
  let previous: GraphemeSegment | undefined;
  for (const segment of graphemeSegments(text)) {
    if (segment.to > offset) break;
    if (segment.to === offset) return segment;
    previous = segment;
  }
  return previous;
}
