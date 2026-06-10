import { Chunk, Effect, Fiber, Option, Stream } from "effect";
import { describe, expect, test } from "vitest";
import { ActiveEditor, ActiveEditorLive, EditorScopeId, makeStateCell } from "../src/index";

const run = <A, E>(effect: Effect.Effect<A, E>) => Effect.runSync(effect);
const runAsync = <A, E>(effect: Effect.Effect<A, E>) => Effect.runPromise(effect);

describe("state cells", () => {
  test("get returns current value, set and update replace it", () => {
    const result = run(
      Effect.gen(function* () {
        const cell = yield* makeStateCell(1);
        const initial = yield* cell.get;
        yield* cell.set(5);
        const afterSet = yield* cell.get;
        const updated = yield* cell.update((n) => n * 2);
        return { initial, afterSet, updated, final: yield* cell.get };
      }),
    );
    expect(result).toEqual({ initial: 1, afterSet: 5, updated: 10, final: 10 });
  });

  test("changes emits the current value immediately, then every change", async () => {
    const values = await runAsync(
      Effect.gen(function* () {
        const cell = yield* makeStateCell("a");
        const consumer = yield* cell.changes.pipe(Stream.take(3), Stream.runCollect, Effect.fork);
        yield* Effect.yieldNow();
        yield* cell.set("b");
        yield* cell.set("c");
        return Chunk.toReadonlyArray(yield* Fiber.join(consumer));
      }),
    );
    expect(values).toEqual(["a", "b", "c"]);
  });

  test("ActiveEditor layer starts empty and tracks the focused scope", () => {
    const result = run(
      Effect.gen(function* () {
        const active = yield* ActiveEditor;
        const initial = yield* active.get;
        yield* active.set(Option.some(EditorScopeId.make("editor:1")));
        return { initial, focused: yield* active.get };
      }).pipe(Effect.provide(ActiveEditorLive)),
    );
    expect(Option.isNone(result.initial)).toBe(true);
    expect(Option.getOrNull(result.focused)).toBe("editor:1");
  });
});
