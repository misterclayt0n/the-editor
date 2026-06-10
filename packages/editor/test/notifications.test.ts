import { Chunk, Effect, Exit, Fiber, Stream } from "effect";
import { describe, expect, test } from "vitest";
import {
  closeEditorScope,
  DocumentInvalidated,
  EditorNotifications,
  hasInvalidation,
  invalidate,
  InvalidInvalidationTopicIdError,
  makeEditorScope,
  makeInvalidationTopic,
  payloadsOf,
  provideEditorScope,
  SelectionInvalidated,
  type EditorNotification,
} from "../src/index";

const run = <A, E>(effect: Effect.Effect<A, E>) => Effect.runSync(effect);
const runAsync = <A, E>(effect: Effect.Effect<A, E>) => Effect.runPromise(effect);

const service = Effect.flatMap(EditorNotifications, (notifications) =>
  Effect.succeed(notifications),
);

describe("invalidation topics", () => {
  test("topics are typed at the edges and opaque on the wire", () => {
    const topic = makeInvalidationTopic<{ readonly count: number }>("test/counted");
    const batch = invalidate(topic, { count: 3 });
    expect(batch.topicId).toBe("test/counted");
    expect(batch.payload).toEqual({ count: 3 });
  });

  test("void topics take no payload argument", () => {
    const batch = invalidate(SelectionInvalidated);
    expect(batch.topicId).toBe("@the-editor/editor/selection");
    expect(batch.payload).toBeUndefined();
  });

  test("topic ids must be non-empty", () => {
    expect(() => makeInvalidationTopic("")).toThrow(InvalidInvalidationTopicIdError);
  });

  test("owner is carried opaquely", () => {
    const topic = makeInvalidationTopic("test/owned", { owner: "some-package" });
    expect(topic.owner).toBe("some-package");
  });
});

describe("editor notifications", () => {
  test("publish delivers one batch with all invalidations", async () => {
    const editor = run(makeEditorScope());
    const received = await runAsync(
      provideEditorScope(
        editor,
        Effect.gen(function* () {
          const notifications = yield* service;
          const consumer = yield* notifications.notifications.pipe(
            Stream.take(1),
            Stream.runCollect,
            Effect.fork,
          );
          // Give the subscription a chance to register before publishing.
          yield* Effect.yieldNow();
          yield* notifications.publish([
            invalidate(DocumentInvalidated, { version: 7 }),
            invalidate(SelectionInvalidated),
          ]);
          return Chunk.toReadonlyArray(yield* Fiber.join(consumer));
        }),
      ),
    );
    expect(received).toHaveLength(1);
    const notification = received[0]!;
    expect(notification.serial).toBe(1);
    expect(hasInvalidation(notification, DocumentInvalidated)).toBe(true);
    expect(hasInvalidation(notification, SelectionInvalidated)).toBe(true);
    expect(payloadsOf(notification, DocumentInvalidated)).toEqual([{ version: 7 }]);
    run(closeEditorScope(editor));
  });

  test("serials are monotonic and empty batches are no-ops", () => {
    const editor = run(makeEditorScope());
    const serials = run(
      provideEditorScope(
        editor,
        Effect.gen(function* () {
          const notifications = yield* service;
          const before = yield* notifications.serial;
          yield* notifications.publish([]);
          const afterEmpty = yield* notifications.serial;
          yield* notifications.publish([invalidate(SelectionInvalidated)]);
          yield* notifications.publish([invalidate(SelectionInvalidated)]);
          const after = yield* notifications.serial;
          return { before, afterEmpty, after };
        }),
      ),
    );
    expect(serials).toEqual({ before: 0, afterEmpty: 0, after: 2 });
    run(closeEditorScope(editor));
  });

  test("publish never suspends: safe under runSync on the hot path", () => {
    const editor = run(makeEditorScope());
    run(
      provideEditorScope(
        editor,
        Effect.flatMap(service, (notifications) =>
          notifications.publish([invalidate(SelectionInvalidated)]),
        ),
      ),
    );
    expect(
      run(
        provideEditorScope(
          editor,
          Effect.flatMap(service, (s) => s.serial),
        ),
      ),
    ).toBe(1);
    run(closeEditorScope(editor));
  });

  test("fan-out: independent subscribers each receive every batch", async () => {
    const editor = run(makeEditorScope());
    const [first, second] = await runAsync(
      provideEditorScope(
        editor,
        Effect.gen(function* () {
          const notifications = yield* service;
          const consume = notifications.notifications.pipe(
            Stream.take(2),
            Stream.runCollect,
            Effect.map((chunk): ReadonlyArray<EditorNotification> => Chunk.toReadonlyArray(chunk)),
          );
          const a = yield* Effect.fork(consume);
          const b = yield* Effect.fork(consume);
          yield* Effect.yieldNow();
          yield* notifications.publish([invalidate(SelectionInvalidated)]);
          yield* notifications.publish([invalidate(DocumentInvalidated, { version: 1 })]);
          return [yield* Fiber.join(a), yield* Fiber.join(b)] as const;
        }),
      ),
    );
    expect(first.map((n) => n.serial)).toEqual([1, 2]);
    expect(second.map((n) => n.serial)).toEqual([1, 2]);
    run(closeEditorScope(editor));
  });

  test("late subscribers do not replay old batches", async () => {
    const editor = run(makeEditorScope());
    const received = await runAsync(
      provideEditorScope(
        editor,
        Effect.gen(function* () {
          const notifications = yield* service;
          yield* notifications.publish([invalidate(SelectionInvalidated)]);
          const consumer = yield* notifications.notifications.pipe(
            Stream.take(1),
            Stream.runCollect,
            Effect.fork,
          );
          yield* Effect.yieldNow();
          yield* notifications.publish([invalidate(SelectionInvalidated)]);
          return Chunk.toReadonlyArray(yield* Fiber.join(consumer));
        }),
      ),
    );
    expect(received.map((n) => n.serial)).toEqual([2]);
    run(closeEditorScope(editor));
  });

  test("scopes are isolated: no cross-talk between editors", async () => {
    const editorA = run(makeEditorScope({ id: "a" }));
    const editorB = run(makeEditorScope({ id: "b" }));
    const received = await runAsync(
      Effect.gen(function* () {
        const consumer = yield* provideEditorScope(
          editorA,
          Effect.flatMap(service, (notifications) =>
            notifications.notifications.pipe(Stream.take(1), Stream.runCollect, Effect.fork),
          ),
        );
        yield* Effect.yieldNow();
        yield* provideEditorScope(
          editorB,
          Effect.flatMap(service, (notifications) =>
            notifications.publish([invalidate(DocumentInvalidated, { version: 99 })]),
          ),
        );
        yield* provideEditorScope(
          editorA,
          Effect.flatMap(service, (notifications) =>
            notifications.publish([invalidate(SelectionInvalidated)]),
          ),
        );
        return Chunk.toReadonlyArray(yield* Fiber.join(consumer));
      }),
    );
    expect(received).toHaveLength(1);
    expect(hasInvalidation(received[0]!, SelectionInvalidated)).toBe(true);
    expect(hasInvalidation(received[0]!, DocumentInvalidated)).toBe(false);
    run(closeEditorScope(editorA));
    run(closeEditorScope(editorB));
  });

  test("closing the editor scope ends consumer streams", async () => {
    const editor = run(makeEditorScope());
    const exit = await runAsync(
      Effect.gen(function* () {
        const consumer = yield* provideEditorScope(
          editor,
          Effect.flatMap(service, (notifications) =>
            notifications.notifications.pipe(Stream.runDrain, Effect.fork),
          ),
        );
        yield* Effect.yieldNow();
        yield* closeEditorScope(editor);
        return yield* Fiber.await(consumer);
      }),
    );
    expect(Exit.isSuccess(exit) || Exit.isInterrupted(exit)).toBe(true);
  });
});
