import { describe, expect, test } from "vitest";
import {
  collectSlots,
  DuplicatePanelInStackError,
  DuplicateSlotIdError,
  emptyArrangement,
  InvalidWeightError,
  makeStack,
  normalizeArrangement,
  PanelId,
  slotOfPanel,
  slotPanel,
  UnknownPanelInArrangementError,
  validateArrangement,
  type Arrangement,
  type Stack,
  type Tile,
} from "../src/index";

const p = (n: number) => PanelId(`p${n}`);
const live = (...ns: number[]) => new Set(ns.map(p));

const stack = (id: string, panels: number[], active = 0): Stack => ({
  kind: "stack",
  id: id as Stack["id"],
  panels: panels.map(p),
  active,
});

const split = (direction: "row" | "column", ...children: [Tile, number][]): Tile => ({
  kind: "split",
  direction,
  children: children.map(([tile, weight]) => ({ tile, weight })),
});

describe("normalization", () => {
  test("prunes empty stacks and collapses single-child splits", () => {
    const a: Arrangement = {
      tiles: split("row", [stack("a", [1]), 1], [stack("b", [], 0), 1]),
      overlays: [],
    };
    const normalized = normalizeArrangement(a);
    expect(normalized.tiles).toEqual(stack("a", [1]));
  });

  test("flattens same-direction nested splits with weight distribution", () => {
    const inner = split("row", [stack("b", [2]), 1], [stack("c", [3]), 1]);
    const a: Arrangement = {
      tiles: split("row", [stack("a", [1]), 0.5], [inner, 0.5]),
      overlays: [],
    };
    const normalized = normalizeArrangement(a);
    expect(normalized.tiles).toMatchObject({
      kind: "split",
      direction: "row",
      children: [
        { tile: stack("a", [1]), weight: 0.5 },
        { tile: stack("b", [2]), weight: 0.25 },
        { tile: stack("c", [3]), weight: 0.25 },
      ],
    });
  });

  test("keeps different-direction nesting and clamps active", () => {
    const a: Arrangement = {
      tiles: split(
        "row",
        [stack("a", [1, 2], 9), 1],
        [split("column", [stack("b", [3]), 1], [stack("c", [4]), 1]), 1],
      ),
      overlays: [],
    };
    const normalized = normalizeArrangement(a);
    expect(normalized.tiles).toMatchObject({
      kind: "split",
      children: [
        { tile: stack("a", [1, 2], 1), weight: 1 },
        { tile: { kind: "split", direction: "column" }, weight: 1 },
      ],
    });
  });

  test("empty everything normalizes to null tiles", () => {
    const a: Arrangement = { tiles: stack("a", []), overlays: [] };
    expect(normalizeArrangement(a).tiles).toBeNull();
  });
});

describe("validation", () => {
  test("accepts a valid arrangement, including a panel shown in two slots", () => {
    const a: Arrangement = {
      tiles: split("row", [stack("a", [1, 2]), 1], [stack("b", [1]), 1]),
      overlays: [{ id: "o" as never, panelId: p(3) }],
    };
    expect(validateArrangement(a, live(1, 2, 3))).toBeUndefined();
  });

  test("rejects unknown panels", () => {
    const a: Arrangement = { tiles: stack("a", [7]), overlays: [] };
    expect(validateArrangement(a, live(1))).toBeInstanceOf(UnknownPanelInArrangementError);
  });

  test("rejects duplicate panel within one stack", () => {
    const a: Arrangement = { tiles: stack("a", [1, 1]), overlays: [] };
    expect(validateArrangement(a, live(1))).toBeInstanceOf(DuplicatePanelInStackError);
  });

  test("rejects duplicate slot ids", () => {
    const a: Arrangement = {
      tiles: split("row", [stack("dup", [1]), 1], [stack("dup", [2]), 1]),
      overlays: [],
    };
    expect(validateArrangement(a, live(1, 2))).toBeInstanceOf(DuplicateSlotIdError);
  });

  test("rejects non-positive weights", () => {
    const a: Arrangement = {
      tiles: split("row", [stack("a", [1]), 0], [stack("b", [2]), 1]),
      overlays: [],
    };
    expect(validateArrangement(a, live(1, 2))).toBeInstanceOf(InvalidWeightError);
  });
});

describe("queries", () => {
  test("collectSlots, slotPanel, slotOfPanel", () => {
    const a: Arrangement = {
      tiles: split("row", [stack("a", [1, 2], 1), 1], [stack("b", [3]), 1]),
      overlays: [{ id: "o" as never, panelId: p(4) }],
    };
    expect(collectSlots(a).map((slot) => slot.id)).toEqual(["a", "b", "o"]);
    expect(slotPanel(a, "a" as never)).toBe(p(2)); // active tab
    expect(slotPanel(a, "o" as never)).toBe(p(4));
    expect(slotOfPanel(a, p(3))).toBe("b");
    expect(slotOfPanel(a, p(9))).toBeUndefined();
  });

  test("makeStack mints unique slot ids", () => {
    expect(makeStack([p(1)]).id).not.toBe(makeStack([p(1)]).id);
  });

  test("empty arrangement has no slots", () => {
    expect(collectSlots(emptyArrangement)).toEqual([]);
  });
});
