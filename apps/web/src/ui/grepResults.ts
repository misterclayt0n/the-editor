/**
 * Pure shaping for the global-search (grep) picker: row/preview models derived
 * from a grep match. The preview is built entirely from the match's own
 * context lines (which the backend already returns) — it never reads files, so
 * scrolling the result list stays cheap and synchronous.
 */

import type { CodeSpan } from "../editor/highlightSpans";
import type { FileFinderGrepMatch } from "../platform/fileFinder";

export type MatchRange = readonly [number, number];

/** The subset of a grep match this module needs (keeps helpers test-friendly). */
export type GrepMatchLike = Pick<
  FileFinderGrepMatch,
  | "relativePath"
  | "lineNumber"
  | "byteOffset"
  | "lineContent"
  | "matchRanges"
  | "contextBefore"
  | "contextAfter"
>;

/** Stable per-occurrence key (byteOffset is unique within a file). */
export const matchKey = (match: Pick<GrepMatchLike, "relativePath" | "byteOffset">): string =>
  `${match.relativePath}#${match.byteOffset}`;

/** 0-based caret target: the start of the first match on the line. */
export const jumpTarget = (
  match: Pick<GrepMatchLike, "lineNumber" | "matchRanges">,
): { readonly line: number; readonly character: number } => ({
  line: Math.max(0, match.lineNumber - 1),
  character: match.matchRanges[0]?.[0] ?? 0,
});

export type Segment = { readonly text: string; readonly match: boolean };
export type StyledSegment = Segment & { readonly color?: string };

const normalizedRanges = (textLength: number, ranges: readonly MatchRange[]): MatchRange[] => {
  const sorted = ranges
    .map(([from, to]): MatchRange => [Math.max(0, Math.min(from, textLength)), Math.max(0, Math.min(to, textLength))])
    .filter(([from, to]) => to > from)
    .toSorted((a, b) => a[0] - b[0]);
  const normalized: MatchRange[] = [];
  for (const [from, to] of sorted) {
    const previous = normalized[normalized.length - 1];
    if (previous !== undefined && from <= previous[1]) {
      normalized[normalized.length - 1] = [previous[0], Math.max(previous[1], to)];
    } else {
      normalized.push([from, to]);
    }
  }
  return normalized;
};

const pushStyledSegment = (segments: StyledSegment[], segment: StyledSegment) => {
  if (segment.text.length === 0) return;
  const previous = segments[segments.length - 1];
  if (previous?.match === segment.match && previous.color === segment.color) {
    segments[segments.length - 1] = { ...previous, text: previous.text + segment.text };
  } else {
    segments.push(segment);
  }
};

/** Split `text` into matched / unmatched segments from clamped, sorted ranges. */
export function highlightSegments(text: string, ranges: readonly MatchRange[]): Segment[] {
  const sorted = normalizedRanges(text.length, ranges);
  if (sorted.length === 0) return text.length > 0 ? [{ text, match: false }] : [];
  const segments: Segment[] = [];
  let pos = 0;
  for (const [from, to] of sorted) {
    if (from > pos) segments.push({ text: text.slice(pos, from), match: false });
    segments.push({ text: text.slice(from, to), match: true });
    pos = to;
  }
  if (pos < text.length) segments.push({ text: text.slice(pos), match: false });
  return segments;
}

/** Overlay search-hit ranges on syntax-colored spans for rendering. */
export function styleSegments(
  text: string,
  ranges: readonly MatchRange[],
  codeSpans: readonly CodeSpan[] | null | undefined,
): StyledSegment[] {
  if (codeSpans === null || codeSpans === undefined || codeSpans.length === 0) {
    return highlightSegments(text, ranges);
  }

  const sorted = normalizedRanges(text.length, ranges);
  const segments: StyledSegment[] = [];
  let position = 0;
  let rangeIndex = 0;

  for (const span of codeSpans) {
    const spanStart = position;
    const spanEnd = Math.min(text.length, spanStart + span.text.length);
    let cursor = spanStart;

    while (rangeIndex < sorted.length && sorted[rangeIndex]![1] <= spanStart) rangeIndex++;

    for (let i = rangeIndex; i < sorted.length && sorted[i]![0] < spanEnd; i++) {
      const [from, to] = sorted[i]!;
      const plainEnd = Math.min(Math.max(from, cursor), spanEnd);
      if (plainEnd > cursor) {
        pushStyledSegment(segments, {
          text: text.slice(cursor, plainEnd),
          match: false,
          ...(span.color !== undefined ? { color: span.color } : {}),
        });
      }

      const matchStart = Math.max(from, cursor, spanStart);
      const matchEnd = Math.min(to, spanEnd);
      if (matchEnd > matchStart) {
        pushStyledSegment(segments, {
          text: text.slice(matchStart, matchEnd),
          match: true,
          ...(span.color !== undefined ? { color: span.color } : {}),
        });
        cursor = matchEnd;
      }
    }

    if (cursor < spanEnd) {
      pushStyledSegment(segments, {
        text: text.slice(cursor, spanEnd),
        match: false,
        ...(span.color !== undefined ? { color: span.color } : {}),
      });
    }
    position += span.text.length;
  }

  if (position < text.length) {
    const remainingRanges = sorted
      .filter(([, to]) => to > position)
      .map(([from, to]): MatchRange => [from - position, to - position]);
    for (const segment of highlightSegments(text.slice(position), remainingRanges)) {
      pushStyledSegment(segments, segment);
    }
  }

  return segments;
}

/** Drop leading whitespace from a row line, shifting match ranges to match. */
export function trimLeading(
  text: string,
  ranges: readonly MatchRange[],
): { readonly text: string; readonly ranges: MatchRange[] } {
  const trimmed = text.length - text.trimStart().length;
  if (trimmed === 0) return { text, ranges: [...ranges] };
  const shifted = ranges
    .map(([from, to]): MatchRange => [Math.max(0, from - trimmed), Math.max(0, to - trimmed)])
    .filter(([from, to]) => to > from);
  return { text: text.slice(trimmed), ranges: shifted };
}

export type PreviewLine = {
  readonly number: number;
  readonly text: string;
  readonly match: boolean;
  readonly ranges: readonly MatchRange[];
};

/**
 * The preview window for a match: its context-before lines, the matched line
 * (carrying its highlight ranges), then context-after — with correct 1-based
 * line numbers. Built purely from data the grep call already returned.
 */
export function previewLines(match: GrepMatchLike): PreviewLine[] {
  const before = match.contextBefore ?? [];
  const after = match.contextAfter ?? [];
  const lines: PreviewLine[] = [];
  before.forEach((text, i) =>
    lines.push({ number: match.lineNumber - before.length + i, text, match: false, ranges: [] }),
  );
  lines.push({
    number: match.lineNumber,
    text: match.lineContent,
    match: true,
    ranges: match.matchRanges,
  });
  after.forEach((text, i) =>
    lines.push({ number: match.lineNumber + 1 + i, text, match: false, ranges: [] }),
  );
  return lines;
}
