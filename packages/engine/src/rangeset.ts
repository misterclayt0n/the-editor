/**
 * RangeSet: a persistent, position-anchored interval collection.
 *
 * This is the engine primitive that makes visual extensibility scale: owners
 * (decorations, text styles, virtual text, widgets) keep their data in
 * RangeSets, the renderer queries only the viewport window, and edits slide
 * everything through `map` without the owner running at all.
 *
 * The performance contract everything above this file relies on:
 * - `between(from, to)` is O(log chunks + matches): viewport queries never
 *   touch the whole set.
 * - `map(changes)` is O(chunks + ranges-near-the-edit): chunks untouched by
 *   an edit are shifted by adjusting one base offset and SHARE their range
 *   storage with the previous set (structural sharing), so a keystroke costs
 *   the same whether a set holds ten ranges or a hundred thousand.
 *
 * Storage: ranges sorted by `from`, grouped into fixed-size chunks; each
 * chunk stores positions relative to its `base`. Sets are immutable; `map`
 * returns a new set.
 */

import type { ChangeDesc } from "./text";

export type RangedValue<T> = {
  readonly from: number;
  readonly to: number;
  readonly value: T;
};

export type RangeSetMapOptions = {
  /**
   * Where a range START lands when text is inserted exactly at it.
   * "after" (default): the insertion stays OUTSIDE the range.
   */
  readonly startAssoc?: "before" | "after";
  /**
   * Where a range END lands when text is inserted exactly at it.
   * "before" (default): the insertion stays OUTSIDE the range.
   */
  readonly endAssoc?: "before" | "after";
  /** Where points (from === to) land on insertion at them (default "before"). */
  readonly pointAssoc?: "before" | "after";
  /**
   * Keep spans whose text was entirely deleted (they collapse to points).
   * Default false: collapsed spans are dropped. Points always survive.
   */
  readonly keepCollapsed?: boolean;
};

const CHUNK_SIZE = 128;

type Chunk<T> = {
  /** Absolute offset added to every relative position in this chunk. */
  readonly base: number;
  /** Ranges relative to base, sorted by from (ties by to). */
  readonly ranges: ReadonlyArray<RangedValue<T>>;
  /** Max relative `to` in this chunk (chunk's reach past its base). */
  readonly maxEnd: number;
};

type ChangeRangeLike = {
  readonly fromA: number;
  readonly toA: number;
  readonly fromB: number;
  readonly toB: number;
};

function compareRanges<T>(a: RangedValue<T>, b: RangedValue<T>): number {
  return a.from - b.from || a.to - b.to;
}

/**
 * Map one position through sorted change ranges. `role` decides where a
 * position INSIDE replaced text lands (start → replacement start, end →
 * replacement end); `boundary` decides which side of an insertion exactly AT
 * the position it lands on.
 */
function mapPoint(
  pos: number,
  changes: ReadonlyArray<ChangeRangeLike>,
  role: "start" | "end",
  boundary: "before" | "after",
): number {
  let diff = 0;
  for (const change of changes) {
    if (pos < change.fromA) break;
    const inserted = change.toB - change.fromB;
    if (pos === change.fromA) {
      return boundary === "before" ? change.fromB : change.fromB + inserted;
    }
    if (pos < change.toA) {
      return role === "start" ? change.fromB : change.fromB + inserted;
    }
    diff += inserted - (change.toA - change.fromA);
  }
  return pos + diff;
}

export class RangeSet<T> {
  private constructor(
    private readonly chunks: ReadonlyArray<Chunk<T>>,
    readonly size: number,
  ) {}

  static empty<T>(): RangeSet<T> {
    return new RangeSet<T>([], 0);
  }

  /** Build a set from ranges (sorted internally; from must be <= to). */
  static of<T>(ranges: Iterable<RangedValue<T>>): RangeSet<T> {
    const sorted = [...ranges].sort(compareRanges);
    for (const range of sorted) {
      if (!(range.from <= range.to)) {
        throw new RangeError(`invalid range ${range.from}..${range.to}`);
      }
    }
    const chunks: Chunk<T>[] = [];
    for (let i = 0; i < sorted.length; i += CHUNK_SIZE) {
      const slice = sorted.slice(i, i + CHUNK_SIZE);
      const base = slice[0]!.from;
      let maxEnd = 0;
      const relative = slice.map((range) => {
        const to = range.to - base;
        if (to > maxEnd) maxEnd = to;
        return { from: range.from - base, to, value: range.value };
      });
      chunks.push({ base, ranges: relative, maxEnd });
    }
    return new RangeSet(chunks, sorted.length);
  }

  get isEmpty(): boolean {
    return this.size === 0;
  }

  /**
   * Ranges intersecting the CLOSED interval [from, to], sorted by from.
   * (Closed so zero-length points sitting exactly on a boundary are
   * returned; callers with half-open needs filter the result.)
   */
  between(from: number, to: number): RangedValue<T>[] {
    const out: RangedValue<T>[] = [];
    const chunks = this.chunks;
    // First chunk that could reach `from`.
    let lo = 0;
    let hi = chunks.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (chunks[mid]!.base + chunks[mid]!.maxEnd < from) lo = mid + 1;
      else hi = mid;
    }
    for (let i = lo; i < chunks.length && chunks[i]!.base <= to; i++) {
      const chunk = chunks[i]!;
      for (const range of chunk.ranges) {
        const absFrom = chunk.base + range.from;
        if (absFrom > to) break;
        const absTo = chunk.base + range.to;
        if (absTo >= from) out.push({ from: absFrom, to: absTo, value: range.value });
      }
    }
    return out;
  }

  /** All ranges, sorted (mainly for tests/debugging). */
  all(): RangedValue<T>[] {
    return this.between(-Infinity, Infinity);
  }

  /**
   * Slide every range through a transaction's changes. Chunks the edit does
   * not touch are shifted in O(1) and share their storage with this set.
   */
  map(changes: ChangeDesc, options: RangeSetMapOptions = {}): RangeSet<T> {
    const changeRanges = changes.ranges as ReadonlyArray<ChangeRangeLike>;
    if (changeRanges.length === 0 || this.chunks.length === 0) return this;
    const startAssoc = options.startAssoc ?? "after";
    const endAssoc = options.endAssoc ?? "before";
    const pointAssoc = options.pointAssoc ?? "before";
    const keepCollapsed = options.keepCollapsed ?? false;

    const nextChunks: Chunk<T>[] = [];
    let size = 0;
    let changeIndex = 0;
    let diffBefore = 0;
    for (const chunk of this.chunks) {
      // Accumulate deltas of changes entirely before this chunk.
      while (
        changeIndex < changeRanges.length &&
        changeRanges[changeIndex]!.toA <= chunk.base &&
        // an insertion exactly at base still affects boundary mapping
        !(changeRanges[changeIndex]!.fromA === chunk.base)
      ) {
        const c = changeRanges[changeIndex]!;
        diffBefore += c.toB - c.fromB - (c.toA - c.fromA);
        changeIndex++;
      }
      const next = changeRanges[changeIndex];
      const chunkEnd = chunk.base + chunk.maxEnd;
      if (next === undefined || next.fromA > chunkEnd) {
        // Fast path: untouched chunk, shift base, share storage.
        nextChunks.push(
          diffBefore === 0
            ? chunk
            : { base: chunk.base + diffBefore, ranges: chunk.ranges, maxEnd: chunk.maxEnd },
        );
        size += chunk.ranges.length;
        continue;
      }
      // Slow path: remap this chunk's ranges individually.
      const remapped: RangedValue<T>[] = [];
      for (const range of chunk.ranges) {
        const absFrom = chunk.base + range.from;
        const absTo = chunk.base + range.to;
        const isPoint = absFrom === absTo;
        const from = mapPoint(absFrom, changeRanges, "start", isPoint ? pointAssoc : startAssoc);
        const to = isPoint ? from : mapPoint(absTo, changeRanges, "end", endAssoc);
        if (to < from) continue;
        if (!isPoint && from === to && !keepCollapsed) continue;
        remapped.push({ from, to, value: range.value });
      }
      if (remapped.length > 0) {
        remapped.sort(compareRanges);
        const base = remapped[0]!.from;
        let maxEnd = 0;
        const relative = remapped.map((range) => {
          const to = range.to - base;
          if (to > maxEnd) maxEnd = to;
          return { from: range.from - base, to, value: range.value };
        });
        nextChunks.push({ base, ranges: relative, maxEnd });
        size += remapped.length;
      }
    }
    return new RangeSet(nextChunks, size);
  }
}
