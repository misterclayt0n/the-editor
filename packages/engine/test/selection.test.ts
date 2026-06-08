import { Effect } from "effect";
import { describe, expect, test } from "vitest";
import {
  EmptySelectionError,
  InvalidSelectionOffsetError,
  RemoveLastSelectionRangeError,
  Selection,
  SelectionOutOfBoundsError,
  SelectionRange,
  SelectionState,
  SelectionStateLive,
  Transaction,
  makeSelectionState,
} from "../src/index";

const run = <A, E = never>(effect: Effect.Effect<A, E>) => Effect.runSync(effect);

describe("SelectionRange", () => {
  test("point and accessors", () => {
    const point = SelectionRange.point(3);
    expect(point.from).toBe(3);
    expect(point.to).toBe(3);
    expect(point.length).toBe(0);
    expect(point.isCollapsed).toBe(true);
    expect(point.direction).toBe("forward");
    const backward = SelectionRange.create(5, 2);
    expect(backward.from).toBe(2);
    expect(backward.to).toBe(5);
    expect(backward.length).toBe(3);
    expect(backward.direction).toBe("backward");
  });

  test("contains, overlaps, touches", () => {
    const a = SelectionRange.create(2, 5);
    expect(a.contains(2)).toBe(true);
    expect(a.contains(5)).toBe(false);
    expect(a.containsRange(SelectionRange.create(3, 4))).toBe(true);
    expect(a.overlaps(SelectionRange.create(4, 7))).toBe(true);
    expect(a.overlaps(SelectionRange.create(5, 7))).toBe(false);
    expect(SelectionRange.point(2).overlaps(SelectionRange.point(2))).toBe(true);
    expect(a.touches(SelectionRange.create(5, 7))).toBe(true);
  });

  test("directional transforms and merge", () => {
    const r = SelectionRange.create(2, 5);
    expect(r.flip()).toEqual(SelectionRange.create(5, 2));
    expect(r.withDirection("backward")).toEqual(SelectionRange.create(5, 2));
    expect(r.extendTo(1, 6)).toEqual(SelectionRange.create(1, 6));
    expect(SelectionRange.create(5, 2).extendTo(1, 6)).toEqual(SelectionRange.create(6, 1));
    expect(r.merge(SelectionRange.create(4, 8))).toEqual(SelectionRange.create(2, 8));
    expect(SelectionRange.create(5, 2).merge(SelectionRange.create(8, 4))).toEqual(
      SelectionRange.create(8, 2),
    );
    expect(SelectionRange.create(5, 2).merge(SelectionRange.create(4, 8))).toEqual(
      SelectionRange.create(2, 8),
    );
  });

  test("invalid offsets rejected", () => {
    expect(() => SelectionRange.point(-1)).toThrow(InvalidSelectionOffsetError);
    expect(() => SelectionRange.create(1.5, 2)).toThrow(InvalidSelectionOffsetError);
  });
});

describe("Selection", () => {
  test("constructors and basic accessors", () => {
    expect(Selection.point(2).primary).toEqual(SelectionRange.point(2));
    const s = Selection.single(1, 4);
    expect(s.rangeCount).toBe(1);
    expect(s.length).toBe(3);
    expect(s.isCollapsed).toBe(false);
    expect(() => Selection.create([])).toThrow(EmptySelectionError);
  });

  test("normalization sorts, merges overlaps, not adjacent", () => {
    const s = Selection.create(
      [
        SelectionRange.create(10, 12),
        SelectionRange.create(1, 3),
        SelectionRange.create(2, 5),
        SelectionRange.create(5, 7),
      ],
      { primaryIndex: 2 },
    );
    expect(s.ranges).toEqual([
      SelectionRange.create(1, 5),
      SelectionRange.create(5, 7),
      SelectionRange.create(10, 12),
    ]);
    expect(s.primary).toEqual(SelectionRange.create(1, 5));
  });

  test("range replacement/add/remove/collapse/transform", () => {
    let s = Selection.create([SelectionRange.point(1), SelectionRange.point(5)], {
      primaryIndex: 1,
    });
    expect(s.range(1)).toEqual(SelectionRange.point(5));
    s = s.replacePrimary(SelectionRange.create(4, 8));
    expect(s.primary).toEqual(SelectionRange.create(4, 8));
    s = s.addRange(SelectionRange.create(10, 11), { makePrimary: true });
    expect(s.primary).toEqual(SelectionRange.create(10, 11));
    const removed = s.removeRange(s.primaryIndex);
    expect(removed.rangeCount).toBe(2);
    expect(() => Selection.point(0).removeRange(0)).toThrow(RemoveLastSelectionRangeError);
    expect(s.collapseToPrimary().ranges).toEqual([SelectionRange.point(11)]);
    expect(s.collapseAllToHeads().ranges.every((r) => r.isCollapsed)).toBe(true);
    expect(s.transform((range) => range.flip()).primary.direction).toBe("backward");
  });

  test("validate document length", () => {
    Selection.single(0, 3).validate(3);
    expect(() => Selection.single(0, 4).validate(3)).toThrow(SelectionOutOfBoundsError);
  });
});

describe("selection mapping", () => {
  test("collapsed cursor around insertions", () => {
    const before = run(Transaction.insertAt(10, 2, "XX")).desc();
    expect(Selection.point(5).map(before).primary).toEqual(SelectionRange.point(7));
    expect(Selection.point(1).map(before).primary).toEqual(SelectionRange.point(1));
    expect(Selection.point(2).map(before).primary).toEqual(SelectionRange.point(4));
  });

  test("forward and backward selections preserve directional sticky behavior", () => {
    const insertAtStart = run(Transaction.insertAt(10, 2, "XX")).desc();
    expect(Selection.single(2, 5).map(insertAtStart).primary).toEqual(SelectionRange.create(4, 7));
    const backward = Selection.single(5, 2).map(insertAtStart).primary;
    expect(backward.direction).toBe("backward");
    expect(backward).toEqual(SelectionRange.create(7, 4));
  });

  test("deletions shift and contract selections", () => {
    const delBefore = run(Transaction.delete(10, { start: 1, end: 3 })).desc();
    expect(Selection.single(5, 8).map(delBefore).primary).toEqual(SelectionRange.create(3, 6));
    const overlap = run(Transaction.delete(10, { start: 3, end: 6 })).desc();
    expect(Selection.single(2, 8).map(overlap).primary).toEqual(SelectionRange.create(2, 5));
  });

  test("multiple ranges can merge after mapping", () => {
    const tx = run(Transaction.delete(10, { start: 3, end: 5 })).desc();
    const s = Selection.create([SelectionRange.point(3), SelectionRange.point(5)], {
      primaryIndex: 1,
    });
    const mapped = s.map(tx);
    expect(mapped.ranges).toEqual([SelectionRange.point(3)]);
    expect(mapped.primaryIndex).toBe(0);
  });
});

describe("SelectionState", () => {
  test("default, custom, get/set/update/snapshot", () => {
    const state = run(makeSelectionState());
    expect(run(state.get)).toEqual(Selection.point(0));
    run(state.set(Selection.point(4)));
    expect(run(state.get)).toEqual(Selection.point(4));
    expect(
      run(
        state.update((selection) =>
          selection.replacePrimary(SelectionRange.point(selection.primary.head + 1)),
        ),
      ),
    ).toEqual(Selection.point(5));
    expect(run(state.snapshot)).toEqual({ selection: Selection.point(5) });

    const custom = run(makeSelectionState({ selection: Selection.single(1, 2) }));
    expect(run(custom.get)).toEqual(Selection.single(1, 2));
  });

  test("Layer usage", () => {
    const program = Effect.gen(function* () {
      const state = yield* SelectionState;
      yield* state.set(Selection.point(3));
      return yield* state.get;
    }).pipe(Effect.provide(SelectionStateLive()));
    expect(Effect.runSync(program)).toEqual(Selection.point(3));
  });
});
