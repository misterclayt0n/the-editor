import { Chunk, Effect, Fiber, Stream } from "effect";
import { describe, expect, test } from "vitest";
import { makeStateCell } from "../src/index.js";

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
});
