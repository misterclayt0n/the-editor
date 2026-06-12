/**
 * Line diff for gutter indicators: Myers O(ND) over line arrays, reduced
 * to what a gutter can show — per NEW-document line, is it added or
 * modified, and does a deletion sit immediately above it.
 */

export type LineDiff = {
  /** New-document line index → marker. */
  readonly lines: ReadonlyMap<number, "added" | "modified">;
  /** New-document line indices with deleted content directly above. */
  readonly deletedAbove: ReadonlySet<number>;
};

export type DiffHunk = {
  /** [oldStart, oldEnd) replaced by [newStart, newEnd). */
  readonly oldStart: number;
  readonly oldEnd: number;
  readonly newStart: number;
  readonly newEnd: number;
};

const MAX_DIFF_LINES = 50_000;

type EditOp = "keep" | "del" | "ins";

/** Myers shortest edit script, ops in forward order. */
function editScript(a: readonly string[], b: readonly string[]): EditOp[] {
  const n = a.length;
  const m = b.length;
  const max = n + m;
  const offset = max;
  const v = new Int32Array(2 * max + 1);
  const traces: Int32Array[] = [];
  let dFound = -1;
  outer: for (let d = 0; d <= max; d++) {
    traces.push(v.slice());
    for (let k = -d; k <= d; k += 2) {
      let x =
        k === -d || (k !== d && v[offset + k - 1]! < v[offset + k + 1]!)
          ? v[offset + k + 1]!
          : v[offset + k - 1]! + 1;
      let y = x - k;
      while (x < n && y < m && a[x] === b[y]) {
        x++;
        y++;
      }
      v[offset + k] = x;
      if (x >= n && y >= m) {
        dFound = d;
        break outer;
      }
    }
  }
  const ops: EditOp[] = [];
  let x = n;
  let y = m;
  for (let d = dFound; d > 0; d--) {
    const prev = traces[d]!;
    const k = x - y;
    const fromDown = k === -d || (k !== d && prev[offset + k - 1]! < prev[offset + k + 1]!);
    const prevK = fromDown ? k + 1 : k - 1;
    const prevX = prev[offset + prevK]!;
    const prevY = prevX - prevK;
    while (x > prevX && y > prevY) {
      ops.push("keep");
      x--;
      y--;
    }
    if (fromDown) {
      ops.push("ins");
      y--;
    } else {
      ops.push("del");
      x--;
    }
  }
  while (x > 0 && y > 0) {
    ops.push("keep");
    x--;
    y--;
  }
  ops.reverse();
  return ops;
}

/** Replacement hunks between two line arrays. */
export function diffHunks(
  oldLines: readonly string[],
  newLines: readonly string[],
): DiffHunk[] {
  // Trim the common prefix/suffix — typical edits are local.
  let prefix = 0;
  while (
    prefix < oldLines.length &&
    prefix < newLines.length &&
    oldLines[prefix] === newLines[prefix]
  ) {
    prefix++;
  }
  let suffix = 0;
  while (
    suffix < oldLines.length - prefix &&
    suffix < newLines.length - prefix &&
    oldLines[oldLines.length - 1 - suffix] === newLines[newLines.length - 1 - suffix]
  ) {
    suffix++;
  }
  const a = oldLines.slice(prefix, oldLines.length - suffix);
  const b = newLines.slice(prefix, newLines.length - suffix);
  if (a.length === 0 && b.length === 0) return [];

  const hunks: DiffHunk[] = [];
  let oldIndex = 0;
  let newIndex = 0;
  let current: { oldStart: number; oldEnd: number; newStart: number; newEnd: number } | null =
    null;
  const flush = () => {
    if (current !== null) {
      hunks.push({ ...current });
      current = null;
    }
  };
  for (const op of editScript(a, b)) {
    if (op === "keep") {
      flush();
      oldIndex++;
      newIndex++;
      continue;
    }
    if (current === null) {
      current = { oldStart: oldIndex, oldEnd: oldIndex, newStart: newIndex, newEnd: newIndex };
    }
    if (op === "del") {
      current.oldEnd = ++oldIndex;
    } else {
      current.newEnd = ++newIndex;
    }
  }
  flush();
  return hunks.map((hunk) => ({
    oldStart: hunk.oldStart + prefix,
    oldEnd: hunk.oldEnd + prefix,
    newStart: hunk.newStart + prefix,
    newEnd: hunk.newEnd + prefix,
  }));
}

/** Per-new-line gutter markers for `newText` against baseline `oldText`. */
export function diffLines(oldText: string, newText: string): LineDiff {
  const empty: LineDiff = { lines: new Map(), deletedAbove: new Set() };
  if (oldText === newText) return empty;
  // A brand-new file (no baseline content) is all additions.
  if (oldText.length === 0) {
    const added = new Map<number, "added" | "modified">();
    const count = newText.split("\n").length;
    for (let line = 0; line < count; line++) added.set(line, "added");
    return { lines: added, deletedAbove: new Set() };
  }
  const oldLines = oldText.split("\n");
  const newLines = newText.split("\n");
  if (oldLines.length > MAX_DIFF_LINES || newLines.length > MAX_DIFF_LINES) return empty;

  const lines = new Map<number, "added" | "modified">();
  const deletedAbove = new Set<number>();
  for (const hunk of diffHunks(oldLines, newLines)) {
    const deleted = hunk.oldEnd - hunk.oldStart;
    const inserted = hunk.newEnd - hunk.newStart;
    if (inserted === 0 && deleted > 0) {
      deletedAbove.add(Math.max(0, Math.min(hunk.newStart, newLines.length - 1)));
      continue;
    }
    const kind = deleted > 0 ? "modified" : "added";
    for (let line = hunk.newStart; line < hunk.newEnd; line++) lines.set(line, kind);
  }
  return { lines, deletedAbove };
}
