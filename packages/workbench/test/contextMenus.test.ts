import { describe, expect, it } from "vitest";
import { Effect, Scope, Exit } from "effect";
import {
  makeContextMenuProviders,
  isMenuSeparator,
  type ContextMenuEntry,
} from "../src/contextMenus.js";
import { makeStatuslineSegments } from "../src/statusline.js";

const run = <A>(effect: Effect.Effect<A>): A => Effect.runSync(effect);

describe("ContextMenuProviders", () => {
  it("collects entries per menu, joins groups with separators, retracts on scope close", () => {
    const registry = makeContextMenuProviders();
    const scope = run(Scope.make());
    run(
      Scope.extend(
        registry.register({
          id: "a",
          provide: (req) => (req.menu === "term" ? [{ id: "copy", label: "Copy", run: () => {} }] : []),
        }),
        scope,
      ),
    );
    run(
      Scope.extend(
        registry.register({
          id: "b",
          provide: (req) => (req.menu === "term" ? [{ id: "clear", label: "Clear", run: () => {} }] : []),
        }),
        scope,
      ),
    );

    const entries = registry.entriesFor({ menu: "term" });
    const labels = entries.map((e: ContextMenuEntry) => (isMenuSeparator(e) ? "|" : e.label));
    expect(labels).toEqual(["Copy", "|", "Clear"]);
    // A different menu gets nothing.
    expect(registry.entriesFor({ menu: "other" })).toEqual([]);

    run(Scope.close(scope, Exit.void));
    expect(registry.entriesFor({ menu: "term" })).toEqual([]);
  });

  it("passes request data through to providers", () => {
    const registry = makeContextMenuProviders();
    const scope = run(Scope.make());
    run(
      Scope.extend(
        registry.register({
          id: "x",
          provide: (req) => [{ id: "d", label: String((req.data as { n: number }).n), run: () => {} }],
        }),
        scope,
      ),
    );
    const entry = registry.entriesFor({ menu: "m", data: { n: 42 } })[0];
    expect(entry !== undefined && !isMenuSeparator(entry) && entry.label).toBe("42");
  });
});

describe("StatuslineSegments", () => {
  it("sorts by side + priority and retracts on scope close", () => {
    const registry = makeStatuslineSegments();
    const scope = run(Scope.make());
    const reg = (id: string, side: "left" | "right", priority: number) =>
      run(Scope.extend(registry.register({ id, side, priority, content: () => ({ text: id }) }), scope));
    reg("l2", "left", 2);
    reg("l1", "left", 1);
    reg("r1", "right", 0);

    expect(registry.forSide("left").map((s) => s.id)).toEqual(["l1", "l2"]);
    expect(registry.forSide("right").map((s) => s.id)).toEqual(["r1"]);

    run(Scope.close(scope, Exit.void));
    expect(registry.forSide("left")).toEqual([]);
  });
});
