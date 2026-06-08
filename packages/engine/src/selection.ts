import { Context, Effect, Layer, Ref } from "effect";
import type { Assoc, ChangeDesc } from "./text/transaction";

export type SelectionDirection = "forward" | "backward";

export class SelectionError extends Error {}
export class EmptySelectionError extends SelectionError {
  constructor() {
    super("selection must contain at least one range");
  }
}
export class InvalidSelectionOffsetError extends SelectionError {
  constructor(readonly offset: number) {
    super(`invalid selection offset ${offset}`);
  }
}
export class PrimaryIndexOutOfBoundsError extends SelectionError {
  constructor(
    readonly primaryIndex: number,
    readonly rangeCount: number,
  ) {
    super(`primary index ${primaryIndex} out of bounds for ${rangeCount} ranges`);
  }
}
export class RangeIndexOutOfBoundsError extends SelectionError {
  constructor(
    readonly index: number,
    readonly rangeCount: number,
  ) {
    super(`range index ${index} out of bounds for ${rangeCount} ranges`);
  }
}
export class RemoveLastSelectionRangeError extends SelectionError {
  constructor() {
    super("cannot remove the last selection range");
  }
}
export class SelectionOutOfBoundsError extends SelectionError {
  constructor(
    readonly offset: number,
    readonly documentLength: number,
  ) {
    super(`selection offset ${offset} out of bounds for document length ${documentLength}`);
  }
}

const assertOffset = (offset: number): void => {
  if (!Number.isFinite(offset) || !Number.isInteger(offset) || offset < 0)
    throw new InvalidSelectionOffsetError(offset);
};
const assertDocumentLength = (length: number): void => {
  if (!Number.isFinite(length) || !Number.isInteger(length) || length < 0)
    throw new InvalidSelectionOffsetError(length);
};
const assertRangeIndex = (index: number, count: number): void => {
  if (!Number.isInteger(index) || index < 0 || index >= count)
    throw new RangeIndexOutOfBoundsError(index, count);
};

export class SelectionRange {
  readonly anchor: number;
  readonly head: number;
  constructor(anchor: number, head: number) {
    assertOffset(anchor);
    assertOffset(head);
    this.anchor = anchor;
    this.head = head;
  }
  static create(anchor: number, head: number): SelectionRange {
    return new SelectionRange(anchor, head);
  }
  static point(offset: number): SelectionRange {
    return new SelectionRange(offset, offset);
  }
  get from(): number {
    return Math.min(this.anchor, this.head);
  }
  get to(): number {
    return Math.max(this.anchor, this.head);
  }
  get length(): number {
    return this.to - this.from;
  }
  get isCollapsed(): boolean {
    return this.anchor === this.head;
  }
  get direction(): SelectionDirection {
    return this.head >= this.anchor ? "forward" : "backward";
  }
  contains(offset: number): boolean {
    assertOffset(offset);
    return this.from <= offset && offset < this.to;
  }
  containsRange(other: SelectionRange): boolean {
    return this.from <= other.from && this.to >= other.to;
  }
  overlaps(other: SelectionRange): boolean {
    return this.from === other.from || (this.to > other.from && other.to > this.from);
  }
  touches(other: SelectionRange): boolean {
    return this.to === other.from || other.to === this.from;
  }
  flip(): SelectionRange {
    return new SelectionRange(this.head, this.anchor);
  }
  withDirection(direction: SelectionDirection): SelectionRange {
    return direction === "forward"
      ? new SelectionRange(this.from, this.to)
      : new SelectionRange(this.to, this.from);
  }
  collapseToHead(): SelectionRange {
    return SelectionRange.point(this.head);
  }
  collapseToAnchor(): SelectionRange {
    return SelectionRange.point(this.anchor);
  }
  extendTo(from: number, to: number): SelectionRange {
    assertOffset(from);
    assertOffset(to);
    const nextFrom = Math.min(this.from, from, to);
    const nextTo = Math.max(this.to, from, to);
    return this.direction === "forward"
      ? new SelectionRange(nextFrom, nextTo)
      : new SelectionRange(nextTo, nextFrom);
  }
  merge(other: SelectionRange): SelectionRange {
    const from = Math.min(this.from, other.from);
    const to = Math.max(this.to, other.to);
    return this.direction === "backward" && other.direction === "backward"
      ? new SelectionRange(to, from)
      : new SelectionRange(from, to);
  }
  map(changeDesc: ChangeDesc): SelectionRange {
    let anchorAssoc: Assoc;
    let headAssoc: Assoc;
    if (this.isCollapsed) anchorAssoc = headAssoc = "afterSticky";
    else if (this.direction === "forward") {
      anchorAssoc = "afterSticky";
      headAssoc = "beforeSticky";
    } else {
      anchorAssoc = "beforeSticky";
      headAssoc = "afterSticky";
    }
    return new SelectionRange(
      changeDesc.mapPosition(this.anchor, anchorAssoc),
      changeDesc.mapPosition(this.head, headAssoc),
    );
  }
}

type RangeEntry = { readonly range: SelectionRange; readonly originalIndex: number };

const normalizeEntries = (
  ranges: readonly SelectionRange[],
  primaryIndex: number,
): { ranges: SelectionRange[]; primaryIndex: number } => {
  const entries = ranges
    .map((range, originalIndex): RangeEntry => ({ range, originalIndex }))
    .sort((a, b) => a.range.from - b.range.from || a.range.to - b.range.to);
  const normalized: SelectionRange[] = [];
  let primary = -1;
  for (let i = 0; i < entries.length; ) {
    let merged = entries[i]!.range;
    let containsPrimary = entries[i]!.originalIndex === primaryIndex;
    i++;
    while (i < entries.length && merged.overlaps(entries[i]!.range)) {
      containsPrimary ||= entries[i]!.originalIndex === primaryIndex;
      merged = merged.merge(entries[i]!.range);
      i++;
    }
    if (containsPrimary) primary = normalized.length;
    normalized.push(merged);
  }
  if (primary < 0) {
    const sortedIndex = entries.findIndex((entry) => entry.originalIndex === primaryIndex);
    const primaryRange = entries[sortedIndex]!.range;
    primary = normalized.findIndex((range) => range.containsRange(primaryRange));
  }
  return { ranges: normalized, primaryIndex: Math.max(0, primary) };
};

export class Selection {
  readonly ranges: readonly SelectionRange[];
  readonly primaryIndex: number;
  private constructor(ranges: readonly SelectionRange[], primaryIndex: number) {
    if (ranges.length === 0) throw new EmptySelectionError();
    if (!Number.isInteger(primaryIndex) || primaryIndex < 0 || primaryIndex >= ranges.length)
      throw new PrimaryIndexOutOfBoundsError(primaryIndex, ranges.length);
    this.ranges = ranges;
    this.primaryIndex = primaryIndex;
  }
  static point(offset: number): Selection {
    return Selection.create([SelectionRange.point(offset)]);
  }
  static single(anchor: number, head: number): Selection {
    return Selection.create([SelectionRange.create(anchor, head)]);
  }
  static create(
    ranges: readonly SelectionRange[],
    options?: { readonly primaryIndex?: number; readonly normalize?: boolean },
  ): Selection {
    if (ranges.length === 0) throw new EmptySelectionError();
    const primaryIndex = options?.primaryIndex ?? ranges.length - 1;
    if (!Number.isInteger(primaryIndex) || primaryIndex < 0 || primaryIndex >= ranges.length)
      throw new PrimaryIndexOutOfBoundsError(primaryIndex, ranges.length);
    if (options?.normalize === false) return new Selection([...ranges], primaryIndex);
    const normalized = normalizeEntries(ranges, primaryIndex);
    return new Selection(normalized.ranges, normalized.primaryIndex);
  }
  get primary(): SelectionRange {
    return this.ranges[this.primaryIndex]!;
  }
  get rangeCount(): number {
    return this.ranges.length;
  }
  get length(): number {
    return this.ranges.reduce((sum, range) => sum + range.length, 0);
  }
  get isCollapsed(): boolean {
    return this.ranges.every((range) => range.isCollapsed);
  }
  range(index: number): SelectionRange {
    assertRangeIndex(index, this.ranges.length);
    return this.ranges[index]!;
  }
  normalize(): Selection {
    return Selection.create(this.ranges, { primaryIndex: this.primaryIndex });
  }
  map(changeDesc: ChangeDesc): Selection {
    return Selection.create(
      this.ranges.map((range) => range.map(changeDesc)),
      { primaryIndex: this.primaryIndex },
    );
  }
  validate(documentLength: number): void {
    assertDocumentLength(documentLength);
    for (const range of this.ranges) {
      for (const offset of [range.anchor, range.head])
        if (offset > documentLength) throw new SelectionOutOfBoundsError(offset, documentLength);
    }
  }
  replaceRange(index: number, range: SelectionRange): Selection {
    assertRangeIndex(index, this.ranges.length);
    const ranges = [...this.ranges];
    ranges[index] = range;
    return Selection.create(ranges, { primaryIndex: this.primaryIndex });
  }
  replacePrimary(range: SelectionRange): Selection {
    return this.replaceRange(this.primaryIndex, range);
  }
  addRange(range: SelectionRange, options?: { readonly makePrimary?: boolean }): Selection {
    const ranges = [...this.ranges, range];
    return Selection.create(ranges, {
      primaryIndex: options?.makePrimary ? ranges.length - 1 : this.primaryIndex,
    });
  }
  removeRange(index: number): Selection {
    assertRangeIndex(index, this.ranges.length);
    if (this.ranges.length === 1) throw new RemoveLastSelectionRangeError();
    const ranges = this.ranges.filter((_, i) => i !== index);
    let primaryIndex = this.primaryIndex;
    if (index === this.primaryIndex) primaryIndex = Math.min(index, ranges.length - 1);
    else if (index < this.primaryIndex) primaryIndex--;
    return Selection.create(ranges, { primaryIndex, normalize: false }).normalize();
  }
  collapseToPrimary(): Selection {
    return Selection.create([this.primary.collapseToHead()]);
  }
  collapseAllToHeads(): Selection {
    return Selection.create(
      this.ranges.map((range) => range.collapseToHead()),
      { primaryIndex: this.primaryIndex },
    );
  }
  transform(f: (range: SelectionRange, index: number) => SelectionRange): Selection {
    return Selection.create(this.ranges.map(f), { primaryIndex: this.primaryIndex });
  }
}

export interface SelectionStateSnapshot {
  readonly selection: Selection;
}
export interface SelectionStateInput {
  readonly selection?: Selection;
}

export class SelectionState extends Context.Tag("@the-editor/engine/SelectionState")<
  SelectionState,
  SelectionState.Service
>() {}
export namespace SelectionState {
  export interface Service {
    readonly get: Effect.Effect<Selection>;
    readonly set: (selection: Selection) => Effect.Effect<void>;
    readonly update: (f: (selection: Selection) => Selection) => Effect.Effect<Selection>;
    readonly snapshot: Effect.Effect<SelectionStateSnapshot>;
  }
}

export const makeSelectionState = (
  input?: SelectionStateInput,
): Effect.Effect<SelectionState.Service> =>
  Effect.gen(function* () {
    const ref = yield* Ref.make(input?.selection ?? Selection.point(0));
    const get = Ref.get(ref);
    return {
      get,
      set: (selection) => Ref.set(ref, selection),
      update: (f) =>
        Ref.modify(ref, (selection) => {
          const next = f(selection);
          return [next, next] as const;
        }),
      snapshot: get.pipe(Effect.map((selection) => ({ selection }))),
    } satisfies SelectionState.Service;
  });

export const SelectionStateLive = (input?: SelectionStateInput): Layer.Layer<SelectionState> =>
  Layer.effect(SelectionState, makeSelectionState(input));
