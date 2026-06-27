import { Chunk, Effect, Exit, Fiber, Scope, Stream } from "effect";
import { describe, expect, test } from "vitest";
import {
  AttentionUnhandledError,
  makeAttention,
  makeNoticeTopic,
  makeRequestTopic,
  type Pending,
} from "../src/index.js";

const AgentReady = makeNoticeTopic<{ readonly session: string }>("test/agent-ready");
const ConfirmDiscard = makeRequestTopic<{ readonly name: string }, "save" | "discard" | "cancel">(
  "test/confirm-discard",
);

const scoped = <A, E>(effect: Effect.Effect<A, E, Scope.Scope>) =>
  Effect.runPromise(Effect.scoped(effect));

describe("attention fabric", () => {
  test("notify fans a fact out to every consumer", async () => {
    const facts = await scoped(
      Effect.gen(function* () {
        const attention = yield* makeAttention();
        const consumer = yield* attention.notices.pipe(
          Stream.take(2),
          Stream.runCollect,
          Effect.fork,
        );
        yield* Effect.yieldNow();
        yield* attention.notify(AgentReady, { session: "a" }, { level: "success" });
        yield* attention.notify(AgentReady, { session: "b" });
        return Chunk.toReadonlyArray(yield* Fiber.join(consumer));
      }),
    );
    expect(facts).toEqual([
      { topicId: "test/agent-ready", level: "success", payload: { session: "a" } },
      { topicId: "test/agent-ready", level: "info", payload: { session: "b" } },
    ]);
  });

  test("ask suspends until a responder answers; the answer flows back", async () => {
    const answer = await scoped(
      Effect.gen(function* () {
        const attention = yield* makeAttention();
        yield* attention.registerResponder;

        const asking = yield* attention
          .ask(ConfirmDiscard, { name: "notes.md" }, { level: "warning" })
          .pipe(Effect.fork);

        // The responder reads the inbox and answers the open request.
        const pending = yield* attention.requestChanges.pipe(
          Stream.filter((requests) => requests.length === 1),
          Stream.take(1),
          Stream.runHead,
          Effect.map((head) => (head._tag === "Some" ? head.value[0] : undefined)),
        );
        expect(pending?.question).toEqual({ name: "notes.md" });
        yield* (pending as Pending<unknown, "save" | "discard" | "cancel">).respond("discard");

        return yield* Fiber.join(asking);
      }),
    );
    expect(answer).toBe("discard");
  });

  test("answered requests retire from the inbox", async () => {
    const remaining = await scoped(
      Effect.gen(function* () {
        const attention = yield* makeAttention();
        yield* attention.registerResponder;
        const asking = yield* attention
          .ask(ConfirmDiscard, { name: "x" }, { default: "cancel" })
          .pipe(Effect.fork);
        const pending = yield* attention.requestChanges.pipe(
          Stream.filter((r) => r.length === 1),
          Stream.take(1),
          Stream.runHead,
          Effect.map((head) => (head._tag === "Some" ? head.value[0]! : undefined)),
        );
        yield* (pending as Pending<unknown, "save" | "discard" | "cancel">).respond("save");
        yield* Fiber.join(asking);
        return yield* attention.openRequests;
      }),
    );
    expect(remaining).toEqual([]);
  });

  test("with no responder, ask resolves the default instead of deadlocking", async () => {
    const answer = await scoped(
      Effect.gen(function* () {
        const attention = yield* makeAttention();
        return yield* attention.ask(ConfirmDiscard, { name: "y" }, { default: "cancel" });
      }),
    );
    expect(answer).toBe("cancel");
  });

  test("with no responder and no default, ask fails AttentionUnhandledError", async () => {
    const exit = await scoped(
      Effect.gen(function* () {
        const attention = yield* makeAttention();
        return yield* attention.ask(ConfirmDiscard, { name: "z" }).pipe(Effect.exit);
      }),
    );
    expect(Exit.isFailure(exit)).toBe(true);
  });

  test("interrupting the asker retires its request", async () => {
    const remaining = await scoped(
      Effect.gen(function* () {
        const attention = yield* makeAttention();
        yield* attention.registerResponder;
        const asking = yield* attention
          .ask(ConfirmDiscard, { name: "w" }, { default: "cancel" })
          .pipe(Effect.fork);
        yield* attention.requestChanges.pipe(
          Stream.filter((r) => r.length === 1),
          Stream.take(1),
          Stream.runDrain,
        );
        yield* Fiber.interrupt(asking);
        return yield* attention.openRequests;
      }),
    );
    expect(remaining).toEqual([]);
  });

  test("ask reports unhandled as a typed failure", async () => {
    const result = await scoped(
      Effect.gen(function* () {
        const attention = yield* makeAttention();
        return yield* attention.ask(ConfirmDiscard, { name: "q" }).pipe(
          Effect.catchAll((error) => Effect.succeed(error instanceof AttentionUnhandledError)),
        );
      }),
    );
    expect(result).toBe(true);
  });
});
