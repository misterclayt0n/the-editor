/**
 * Coordinate discipline for the LSP manager's cached visual ranges.
 *
 * One principle, applied everywhere: NEVER apply positions computed for
 * document version V to a buffer at version W — map them forward through
 * the edits, or discard and re-request. (Zed enforces the same rule with
 * buffer anchors plus `buffer_version.changed_since` gates in its inlay
 * hint cache.)
 *
 * The EditorVisuals service already maps every CONTRIBUTED set through
 * edits, but the manager keeps its own plain copies — diagnostics for the
 * hover card and the cross-server merge, hint ranges as the diff base for
 * localized pushes. Those copies must slide through the same edits
 * (`mapRangedValues`), or a later push re-applies pre-edit offsets and the
 * hints visibly teleport while typing.
 *
 * `changedSpans` is the other half: a push tells the renderer exactly which
 * offset ranges changed, so an identical diagnostics republish repaints
 * nothing and a one-hint change rewrites one row — instead of the whole
 * window.
 */

import type { RangedValue } from "@the-editor/engine";

export type ChangeRangeLike = {
  readonly fromA: number;
  readonly toA: number;
  readonly fromB: number;
  readonly toB: number;
};

/** Map an offset forward through one transaction's change ranges (same
 * semantics as the renderer's scroll anchor and the commit path: positions
 * inside a replaced span collapse to its start; positions at or after a
 * change shift by its length delta). */
export function mapOffsetThrough(changes: ReadonlyArray<ChangeRangeLike>, pos: number): number {
  let diff = 0;
  for (const range of changes) {
    if (pos < range.fromA) break;
    if (pos >= range.toA) {
      diff += range.toB - range.fromB - (range.toA - range.fromA);
      continue;
    }
    return range.fromB;
  }
  return pos + diff;
}

/** Slide cached ranged values through a committed change set. Point ranges
 * (from === to, inlay hints) stay points; spans never invert. */
export function mapRangedValues<T>(
  ranges: ReadonlyArray<RangedValue<T>>,
  changes: ReadonlyArray<ChangeRangeLike>,
): RangedValue<T>[] {
  return ranges.map((range) => {
    const from = mapOffsetThrough(changes, range.from);
    const to =
      range.to === range.from ? from : Math.max(from, mapOffsetThrough(changes, range.to));
    return { from, to, value: range.value };
  });
}

/**
 * The offset spans on which two SORTED range lists differ — the localized
 * invalidation for a visuals push. Entries are equal when their range and
 * value match; every entry present on only one side (or with a changed
 * value) contributes its span. An identical republish yields [] — the
 * caller can skip the push entirely.
 */
export function changedSpans<T>(
  before: ReadonlyArray<RangedValue<T>>,
  after: ReadonlyArray<RangedValue<T>>,
  valueEquals: (a: T, b: T) => boolean,
): Array<{ from: number; to: number }> {
  const spans: Array<{ from: number; to: number }> = [];
  const push = (range: RangedValue<T>) =>
    spans.push({ from: range.from, to: Math.max(range.from, range.to) });
  let i = 0;
  let j = 0;
  while (i < before.length && j < after.length) {
    const a = before[i]!;
    const b = after[j]!;
    if (a.from === b.from && a.to === b.to) {
      if (!valueEquals(a.value, b.value)) push(a);
      i++;
      j++;
      continue;
    }
    if (a.from < b.from || (a.from === b.from && a.to < b.to)) {
      push(a);
      i++;
    } else {
      push(b);
      j++;
    }
  }
  for (; i < before.length; i++) push(before[i]!);
  for (; j < after.length; j++) push(after[j]!);
  return spans;
}

/** Sort ranges the way changedSpans requires (and RangeSet.of expects). */
export function sortRanges<T>(ranges: RangedValue<T>[]): RangedValue<T>[] {
  return ranges.sort((a, b) => a.from - b.from || a.to - b.to);
}
