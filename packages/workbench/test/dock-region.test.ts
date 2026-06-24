import { describe, expect, test } from "vitest";
import {
  dockIntoRegion,
  emptyArrangement,
  normalizeArrangement,
  PanelId,
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
const regionStack = (a: Arrangement, region: string): Stack | undefined =>
  stacks(a.tiles).find((s) => s.region === region);

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
