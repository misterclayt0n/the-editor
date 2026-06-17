import { Context, Effect, Option, Scope } from "effect";
import { describe, expect, test } from "vitest";
import {
  activateTab,
  collectSlots,
  dockEdge,
  floatPanel,
  makeStack,
  makeWorkbench,
  moveTab,
  PanelId,
  placeInStack,
  registerSurfaceFallback,
  removeSlot,
  scopeApplies,
  SlotNotFoundError,
  slotOfPanel,
  splitBeside,
  SurfaceKind,
  SurfaceScope,
  UnknownPanelInArrangementError,
  type Panel,
  type SlotId,
  type Workbench,
} from "../src/index";

const Editor = SurfaceKind("test/editor");
const Tree = SurfaceKind("test/tree");

class Marker extends Context.Tag("test/Marker")<Marker, string>() {}

/** Build a workbench inside a scope we control, run the body synchronously. */
const withWorkbench = <A>(
  body: (wb: Workbench.Service) => Effect.Effect<A, unknown>,
  options?: Parameters<typeof makeWorkbench>[0],
): A =>
  Effect.runSync(Effect.scoped(Effect.flatMap(makeWorkbench(options), body)) as Effect.Effect<A>);

const openTiled = (wb: Workbench.Service, kind = Editor) =>
  Effect.gen(function* () {
    const panel = yield* wb.createPanel({ kind });
    yield* wb.updateArrangement((a) =>
      a.tiles === null
        ? { ...a, tiles: makeStack([panel.id]) }
        : placeInStack(a, collectSlots(a)[0]!.id, panel.id),
    );
    return panel;
  });

const slotOf = (wb: Workbench.Service, panel: Panel) =>
  Effect.map(wb.arrangement, (a) => slotOfPanel(a, panel.id)!);

describe("workbench", () => {
  test("createPanel is detached; placing it makes it visible", () => {
    const result = withWorkbench((wb) =>
      Effect.gen(function* () {
        const panel = yield* wb.createPanel({ kind: Editor });
        const before = yield* wb.arrangement;
        yield* wb.updateArrangement((a) => ({ ...a, tiles: makeStack([panel.id]) }));
        const after = yield* wb.arrangement;
        return { before: collectSlots(before).length, after: collectSlots(after).length };
      }),
    );
    expect(result).toEqual({ before: 0, after: 1 });
  });

  test("updateArrangement rejects references to unknown panels and keeps state", () => {
    const result = withWorkbench((wb) =>
      Effect.gen(function* () {
        const error = yield* wb
          .updateArrangement((a) => ({ ...a, tiles: makeStack(["ghost" as never]) }))
          .pipe(Effect.flip);
        const arrangement = yield* wb.arrangement;
        return { error, tiles: arrangement.tiles };
      }),
    );
    expect(result.error).toBeInstanceOf(UnknownPanelInArrangementError);
    expect(result.tiles).toBeNull();
  });

  test("focus follows slots; closing a panel restores focus from history", () => {
    const result = withWorkbench((wb) =>
      Effect.gen(function* () {
        const first = yield* openTiled(wb);
        const firstSlot = yield* slotOf(wb, first);
        yield* wb.setFocus(firstSlot);

        const second = yield* wb.createPanel({ kind: Tree });
        yield* wb.updateArrangement((a) => splitBeside(a, firstSlot, second.id, "row"));
        const secondSlot = yield* slotOf(wb, second);
        yield* wb.setFocus(secondSlot);

        const focusedBefore = yield* wb.focusedPanel;
        yield* wb.closePanel(second.id);
        const focusedAfter = yield* wb.focusedSlot;
        return {
          before: Option.getOrThrow(focusedBefore).id,
          after: Option.getOrThrow(focusedAfter),
          firstSlot,
        };
      }),
    );
    expect(result.before).toBe("panel:" + result.before.split(":")[1]);
    expect(result.after).toBe(result.firstSlot);
  });

  test("setFocus on a nonexistent slot fails", () => {
    const error = withWorkbench((wb) => wb.setFocus("nope" as SlotId).pipe(Effect.flip));
    expect(error).toBeInstanceOf(SlotNotFoundError);
  });

  test("overlays float, take focus, and dismissal restores focus", () => {
    const result = withWorkbench((wb) =>
      Effect.gen(function* () {
        const editor = yield* openTiled(wb);
        const editorSlot = yield* slotOf(wb, editor);
        yield* wb.setFocus(editorSlot);

        const palette = yield* wb.createPanel({ kind: Tree });
        yield* wb.updateArrangement((a) => floatPanel(a, palette.id));
        const paletteSlot = yield* slotOf(wb, palette);
        yield* wb.setFocus(paletteSlot);
        const whileOpen = yield* wb.focusedPanel;

        // Dismiss: remove the overlay slot (panel stays alive), focus pops back.
        yield* wb.updateArrangement((a) => removeSlot(a, paletteSlot));
        const after = yield* wb.focusedSlot;
        const paletteAlive = yield* wb.panel(palette.id);
        return {
          whileOpen: Option.getOrThrow(whileOpen).id,
          restored: Option.getOrThrow(after) === editorSlot,
          paletteAlive: Option.isSome(paletteAlive),
        };
      }),
    );
    expect(result.whileOpen).toContain("panel:");
    expect(result.restored).toBe(true);
    expect(result.paletteAlive).toBe(true);
  });

  test("tabs: placeInStack, activateTab, moveTab across splits", () => {
    const result = withWorkbench((wb) =>
      Effect.gen(function* () {
        const a = yield* openTiled(wb);
        const b = yield* wb.createPanel({ kind: Editor });
        const slotA = yield* slotOf(wb, a);
        yield* wb.updateArrangement((arr) => placeInStack(arr, slotA, b.id));

        // b was activated on insert; switch back to a.
        yield* wb.updateArrangement((arr) => activateTab(arr, slotA, a.id));
        yield* wb.setFocus(slotA);
        const focusedA = yield* wb.focusedPanel;

        // Split c beside, then drag b over to c's stack.
        const c = yield* wb.createPanel({ kind: Editor });
        yield* wb.updateArrangement((arr) => splitBeside(arr, slotA, c.id, "row"));
        const slotC = (yield* Effect.map(wb.arrangement, (arr) => slotOfPanel(arr, c.id)))!;
        yield* wb.updateArrangement((arr) => moveTab(arr, b.id, slotA, slotC));
        const arrangement = yield* wb.arrangement;
        return {
          focusedA: Option.getOrThrow(focusedA).id === a.id,
          bNowInC: slotOfPanel(arrangement, b.id) === slotC,
        };
      }),
    );
    expect(result).toEqual({ focusedA: true, bNowInC: true });
  });

  test("same panel may sit in two slots (mirror door)", () => {
    const ok = withWorkbench((wb) =>
      Effect.gen(function* () {
        const panel = yield* openTiled(wb);
        const slot = yield* slotOf(wb, panel);
        yield* wb.updateArrangement((a) => splitBeside(a, slot, panel.id, "row"));
        const arrangement = yield* wb.arrangement;
        return collectSlots(arrangement).length;
      }),
    );
    expect(ok).toBe(2);
  });

  test("contextAt merges base under panel context, innermost wins", () => {
    const result = withWorkbench(
      (wb) =>
        Effect.gen(function* () {
          const plain = yield* wb.createPanel({ kind: Editor });
          const custom = yield* wb.createPanel({
            kind: Editor,
            context: Context.make(Marker, "panel") as Context.Context<never>,
          });
          const plainCtx = yield* wb.contextAt(plain.id);
          const customCtx = yield* wb.contextAt(custom.id);
          return {
            plain: Context.get(plainCtx as Context.Context<Marker>, Marker),
            custom: Context.get(customCtx as Context.Context<Marker>, Marker),
          };
        }),
      { baseContext: Context.make(Marker, "base") as Context.Context<never> },
    );
    expect(result).toEqual({ plain: "base", custom: "panel" });
  });

  test("focusedContext falls back to base when nothing is focused", () => {
    const value = withWorkbench(
      (wb) =>
        Effect.map(wb.focusedContext, (ctx) => Context.get(ctx as Context.Context<Marker>, Marker)),
      { baseContext: Context.make(Marker, "base") as Context.Context<never> },
    );
    expect(value).toBe("base");
  });

  test("dockEdge places at edges of the whole tiled area", () => {
    const tiles = withWorkbench((wb) =>
      Effect.gen(function* () {
        const a = yield* openTiled(wb);
        void a;
        const tree = yield* wb.createPanel({ kind: Tree });
        yield* wb.updateArrangement((arr) => dockEdge(arr, tree.id, "left", { ratio: 0.2 }));
        return yield* wb.arrangement;
      }),
    );
    expect(tiles.tiles).toMatchObject({
      kind: "split",
      direction: "row",
      children: [{ weight: 0.2 }, { weight: 0.8 }],
    });
  });

  test("closing the workbench scope closes panel scopes", async () => {
    let finalized = false;
    const scope = Effect.runSync(Scope.make());
    const wb = Effect.runSync(Scope.extend(makeWorkbench(), scope));
    const panel = Effect.runSync(wb.createPanel({ kind: Editor }));
    Effect.runSync(
      Scope.addFinalizer(
        panel.scope,
        Effect.sync(() => (finalized = true)),
      ),
    );
    await Effect.runPromise(Scope.close(scope, undefined as never));
    expect(finalized).toBe(true);
  });
});
describe("scopeApplies", () => {
  const Editor = SurfaceKind("test/scope/editor");
  const MultiBuffer = SurfaceKind("test/scope/multibuffer");
  const Tree = SurfaceKind("test/scope/tree");

  test("surface scope matches its own kind", () => {
    const scope = SurfaceScope.surface(Editor);
    expect(scopeApplies(scope, { id: "p1" as PanelId, kind: Editor })).toBe(true);
    expect(scopeApplies(scope, { id: "p2" as PanelId, kind: Tree })).toBe(false);
  });

  test("surface scope matches a registered fallback kind", () => {
    registerSurfaceFallback(MultiBuffer, Editor);
    const scope = SurfaceScope.surface(Editor);
    expect(scopeApplies(scope, { id: "p1" as PanelId, kind: MultiBuffer })).toBe(true);
  });

  test("fallback chains are followed", () => {
    const A = SurfaceKind("test/scope/a");
    const B = SurfaceKind("test/scope/b");
    const C = SurfaceKind("test/scope/c");
    registerSurfaceFallback(B, A);
    registerSurfaceFallback(C, B);
    expect(scopeApplies(SurfaceScope.surface(A), { id: "p" as PanelId, kind: C })).toBe(true);
  });

  test("global and panel scopes are unaffected by fallback", () => {
    registerSurfaceFallback(MultiBuffer, Editor);
    expect(scopeApplies(SurfaceScope.global, { id: "p" as PanelId, kind: Tree })).toBe(true);
    expect(scopeApplies(SurfaceScope.panel("p1" as PanelId), { id: "p1" as PanelId, kind: MultiBuffer })).toBe(true);
    expect(scopeApplies(SurfaceScope.panel("p1" as PanelId), { id: "p2" as PanelId, kind: MultiBuffer })).toBe(false);
  });
});
