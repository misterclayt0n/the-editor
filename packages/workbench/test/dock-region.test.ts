import { describe, expect, test } from "vitest";
import {
  collapseRegion,
  dockIntoRegion,
  emptyArrangement,
  normalizeArrangement,
  PanelId,
  regionStack,
  restoreRegion,
  type Arrangement,
  type Stack,
  type Tile,
} from "../src/index";

const p = (n: number) => PanelId(`p${n}`);

function stacks(tile: Tile | null): Stack[] {
  if (tile === null) return [];
  if (tile.kind === "stack") return [tile];
  return tile.children.flatMap((child) => stacks(child.tile));
}

describe("dockIntoRegion", () => {
  test("first dock creates a region stack carrying its id, class, and tabs pref", () => {
    const a = dockIntoRegion(emptyArrangement, p(1), {
      region: "right-sidebar",
      edge: "right",
      className: "wb-pane--rightbar",
      tabsHidden: false,
    });
    const stack = regionStack(a, "right-sidebar");
    expect(stack?.panels).toEqual([p(1)]);
    expect(stack?.region).toBe("right-sidebar");
    expect(stack?.className).toBe("wb-pane--rightbar");
  });

  test("a second panel docked into the same region joins ONE stack (tabs together)", () => {
    let a = dockIntoRegion(emptyArrangement, p(1), { region: "right-sidebar", edge: "right" });
    a = dockIntoRegion(a, p(2), { region: "right-sidebar", edge: "right" });
    const inRegion = stacks(a.tiles).filter((s) => s.region === "right-sidebar");
    expect(inRegion).toHaveLength(1);
    expect(inRegion[0]?.panels).toEqual([p(1), p(2)]);
    // The newly docked panel becomes active.
    expect(inRegion[0]?.active).toBe(1);
  });

  test("different regions get separate stacks beside each other", () => {
    let a = dockIntoRegion(emptyArrangement, p(0), { region: "viewport", edge: "right" });
    a = dockIntoRegion(a, p(1), { region: "left-sidebar", edge: "left" });
    a = dockIntoRegion(a, p(2), { region: "right-sidebar", edge: "right" });
    expect(regionStack(a, "viewport")?.panels).toEqual([p(0)]);
    expect(regionStack(a, "left-sidebar")?.panels).toEqual([p(1)]);
    expect(regionStack(a, "right-sidebar")?.panels).toEqual([p(2)]);
  });

  test("region id and chrome class survive normalization", () => {
    let a = dockIntoRegion(emptyArrangement, p(0), { region: "viewport", edge: "right" });
    a = dockIntoRegion(a, p(1), {
      region: "left-sidebar",
      edge: "left",
      className: "wb-pane--sidebar",
      tabsHidden: true,
    });
    const normalized = normalizeArrangement(a);
    const stack = regionStack(normalized, "left-sidebar");
    expect(stack?.className).toBe("wb-pane--sidebar");
    expect(stack?.tabsHidden).toBe(true);
  });
});

/** Build: editor p0 + a right "rail" region holding [p1, p2] with p2 active. */
function withRail(): Arrangement {
  let a = dockIntoRegion(emptyArrangement, p(0), { region: "editor", edge: "left" });
  a = dockIntoRegion(a, p(1), {
    region: "rail",
    edge: "right",
    className: "wb-pane--rail",
    ratio: 0.25,
  });
  a = dockIntoRegion(a, p(2), { region: "rail", edge: "right" });
  return normalizeArrangement(a);
}

describe("collapseRegion / restoreRegion", () => {
  test("collapse drops the region; restore brings back tabs, active, class, and width", () => {
    const a = withRail();
    const collapsed = collapseRegion(a, "rail");
    expect(collapsed).toBeDefined();
    // The snapshot captures the whole layout.
    expect(collapsed!.snapshot.stack.panels).toEqual([p(1), p(2)]);
    expect(collapsed!.snapshot.stack.active).toBe(1);
    expect(collapsed!.snapshot.stack.className).toBe("wb-pane--rail");
    expect(collapsed!.snapshot.edge).toBe("right");
    expect(collapsed!.snapshot.ratio).toBeCloseTo(0.25, 5);

    // After commit (normalize) the region is gone entirely.
    const hidden = normalizeArrangement(collapsed!.arrangement);
    expect(regionStack(hidden, "rail")).toBeUndefined();

    // Restore reattaches the exact stack at the same edge/width.
    const back = normalizeArrangement(restoreRegion(hidden, collapsed!.snapshot, new Set([p(0), p(1), p(2)])));
    const stack = regionStack(back, "rail");
    expect(stack?.panels).toEqual([p(1), p(2)]);
    expect(stack?.active).toBe(1);
    expect(stack?.className).toBe("wb-pane--rail");
    expect(collapseRegion(back, "rail")!.snapshot.ratio).toBeCloseTo(0.25, 5);
  });

  test("restore prunes panels that died while collapsed (and clamps active)", () => {
    const collapsed = collapseRegion(withRail(), "rail")!;
    const hidden = normalizeArrangement(collapsed.arrangement);
    // p2 (the active tab) died → only p1 returns, active clamped to 0.
    const back = normalizeArrangement(restoreRegion(hidden, collapsed.snapshot, new Set([p(0), p(1)])));
    const stack = regionStack(back, "rail");
    expect(stack?.panels).toEqual([p(1)]);
    expect(stack?.active).toBe(0);
  });

  test("restore is a no-op when every stashed panel is gone", () => {
    const collapsed = collapseRegion(withRail(), "rail")!;
    const hidden = normalizeArrangement(collapsed.arrangement);
    const back = restoreRegion(hidden, collapsed.snapshot, new Set([p(0)]));
    expect(back).toBe(hidden);
  });

  test("collapse returns undefined for an absent region", () => {
    expect(collapseRegion(withRail(), "nope")).toBeUndefined();
  });
});
