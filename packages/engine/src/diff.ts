/**
 * diffText: compute a minimal-ish Transaction that turns one text into
 * another.
 *
 * This is the primitive that lets EXTERNAL content enter the editor as an
 * ordinary edit: file reloads (watcher fires → diff disk against buffer →
 * commit), formatter output, VCS checkouts. Because the result is a
 * Transaction, everything downstream is automatic — undo history stays
 * coherent, selections and every extension's RangeSets map through the
 * change ranges, language servers receive exact deltas.
 *
 * Algorithm (the classic editor recipe, tuned for code):
 *   1. trim the common prefix/suffix (never splitting a surrogate pair),
 *   2. patience diff on lines in the changed middle: lines unique to both
 *      sides anchor the alignment (longest increasing subsequence), recurse
 *      between anchors; regions without anchors become replacement hunks,
 *   3. per-hunk character prefix/suffix refinement, so a one-character edit
 *      inside a line yields a one-character replacement.
 *
 * Quality note: output is correct by construction (applying it always
 * yields `after`) and minimal in the cases that matter (localized edits);
 * pathological inputs degrade to coarser hunks, never to wrong results.
 */

import { Effect } from "effect";
import type { TextBuffer } from "./text";
import { Transaction, type TextError } from "./text";

type Replacement = { readonly from: number; readonly to: number; readonly text: string };

const isLowSurrogate = (code: number) => code >= 0xdc00 && code <= 0xdfff;

/** Length of the common prefix, never ending between a surrogate pair. */
function commonPrefix(a: string, b: string): number {
  const max = Math.min(a.length, b.length);
  let i = 0;
  while (i < max && a.charCodeAt(i) === b.charCodeAt(i)) i++;
  if (i > 0 && i < max && isLowSurrogate(a.charCodeAt(i))) i--;
  return i;
}

/** Length of the common suffix within the unprefixed remainder. */
function commonSuffix(a: string, b: string, prefix: number): number {
  const max = Math.min(a.length, b.length) - prefix;
  let i = 0;
  while (i < max && a.charCodeAt(a.length - 1 - i) === b.charCodeAt(b.length - 1 - i)) i++;
  if (i > 0 && i < max && isLowSurrogate(a.charCodeAt(a.length - i))) i--;
  return i;
}

function splitLines(text: string): string[] {
  // Keep terminators attached so offsets reconstruct exactly.
  const lines: string[] = [];
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    if (text.charCodeAt(i) === 10 /* \n */) {
      lines.push(text.slice(start, i + 1));
      start = i + 1;
    }
  }
  if (start < text.length) lines.push(text.slice(start));
  return lines;
}

type LineRange = { aStart: number; aEnd: number; bStart: number; bEnd: number };

/**
 * Patience alignment: match lines unique in both sides, take the longest
 * increasing subsequence of those matches as anchors, recurse between them.
 * Pushes unmatched regions into `out`.
 */
function patience(
  a: ReadonlyArray<string>,
  b: ReadonlyArray<string>,
  range: LineRange,
  out: LineRange[],
): void {
  let { aStart, aEnd, bStart, bEnd } = range;
  // Trim common lines at both edges.
  while (aStart < aEnd && bStart < bEnd && a[aStart] === b[bStart]) {
    aStart++;
    bStart++;
  }
  while (aEnd > aStart && bEnd > bStart && a[aEnd - 1] === b[bEnd - 1]) {
    aEnd--;
    bEnd--;
  }
  if (aStart === aEnd && bStart === bEnd) return;
  if (aStart === aEnd || bStart === bEnd) {
    out.push({ aStart, aEnd, bStart, bEnd });
    return;
  }

  // Lines occurring exactly once on each side.
  const counts = new Map<string, { a: number; b: number; aIndex: number; bIndex: number }>();
  for (let i = aStart; i < aEnd; i++) {
    const line = a[i]!;
    const entry = counts.get(line) ?? { a: 0, b: 0, aIndex: -1, bIndex: -1 };
    entry.a++;
    entry.aIndex = i;
    counts.set(line, entry);
  }
  for (let i = bStart; i < bEnd; i++) {
    const line = b[i]!;
    const entry = counts.get(line);
    if (entry === undefined) continue;
    entry.b++;
    entry.bIndex = i;
  }
  const matches: Array<{ aIndex: number; bIndex: number }> = [];
  for (let i = aStart; i < aEnd; i++) {
    const entry = counts.get(a[i]!)!;
    if (entry.a === 1 && entry.b === 1) matches.push({ aIndex: i, bIndex: entry.bIndex });
  }

  if (matches.length === 0) {
    out.push({ aStart, aEnd, bStart, bEnd });
    return;
  }

  // Longest increasing subsequence over bIndex (matches are sorted by aIndex).
  const tailIndices: number[] = [];
  const predecessors = new Array<number>(matches.length).fill(-1);
  for (let i = 0; i < matches.length; i++) {
    const bIndex = matches[i]!.bIndex;
    let lo = 0;
    let hi = tailIndices.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (matches[tailIndices[mid]!]!.bIndex < bIndex) lo = mid + 1;
      else hi = mid;
    }
    if (lo > 0) predecessors[i] = tailIndices[lo - 1]!;
    tailIndices[lo] = i;
  }
  const anchors: Array<{ aIndex: number; bIndex: number }> = [];
  for (let i = tailIndices.length > 0 ? tailIndices[tailIndices.length - 1]! : -1; i >= 0; ) {
    anchors.push(matches[i]!);
    i = predecessors[i]!;
  }
  anchors.reverse();

  let prevA = aStart;
  let prevB = bStart;
  for (const anchor of anchors) {
    patience(a, b, { aStart: prevA, aEnd: anchor.aIndex, bStart: prevB, bEnd: anchor.bIndex }, out);
    prevA = anchor.aIndex + 1;
    prevB = anchor.bIndex + 1;
  }
  patience(a, b, { aStart: prevA, aEnd: aEnd, bStart: prevB, bEnd: bEnd }, out);
}

function materialize(input: string | TextBuffer): string {
  return typeof input === "string" ? input : input.sliceString(0, input.length);
}

/**
 * Replacements (sorted, non-overlapping) turning `before` into `after`.
 * Exposed for consumers that want the raw hunks (e.g. VCS gutters).
 */
export function diffReplacements(
  before: string | TextBuffer,
  after: string | TextBuffer,
): Replacement[] {
  const a = materialize(before);
  const b = materialize(after);
  if (a === b) return [];

  const prefix = commonPrefix(a, b);
  const suffix = commonSuffix(a, b, prefix);
  const aMid = a.slice(prefix, a.length - suffix);
  const bMid = b.slice(prefix, b.length - suffix);
  if (aMid.length === 0 || bMid.length === 0) {
    return [{ from: prefix, to: a.length - suffix, text: bMid }];
  }

  const aLines = splitLines(aMid);
  const bLines = splitLines(bMid);
  const hunks: LineRange[] = [];
  patience(aLines, bLines, { aStart: 0, aEnd: aLines.length, bStart: 0, bEnd: bLines.length }, hunks);

  // Line index → offset within the middle.
  const aOffsets = new Array<number>(aLines.length + 1);
  aOffsets[0] = 0;
  for (let i = 0; i < aLines.length; i++) aOffsets[i + 1] = aOffsets[i]! + aLines[i]!.length;
  const bOffsets = new Array<number>(bLines.length + 1);
  bOffsets[0] = 0;
  for (let i = 0; i < bLines.length; i++) bOffsets[i + 1] = bOffsets[i]! + bLines[i]!.length;

  const replacements: Replacement[] = [];
  for (const hunk of hunks) {
    let from = aOffsets[hunk.aStart]!;
    let to = aOffsets[hunk.aEnd]!;
    let text = bMid.slice(bOffsets[hunk.bStart]!, bOffsets[hunk.bEnd]!);
    // Character-level refinement inside the hunk.
    const aChunk = aMid.slice(from, to);
    const innerPrefix = commonPrefix(aChunk, text);
    const innerSuffix = commonSuffix(aChunk, text, innerPrefix);
    from += innerPrefix;
    to -= innerSuffix;
    text = text.slice(innerPrefix, text.length - innerSuffix);
    if (from === to && text.length === 0) continue;
    replacements.push({ from: prefix + from, to: prefix + to, text });
  }
  return replacements;
}

/** A Transaction turning `before` into `after`; commit it like any edit. */
export function diffText(
  before: string | TextBuffer,
  after: string | TextBuffer,
): Effect.Effect<Transaction, TextError> {
  const a = materialize(before);
  return Transaction.fromReplacements(a.length, diffReplacements(a, after));
}
