import { Chunk, Effect, Fiber, Scope, Stream } from "effect";
import { describe, expect, test } from "vitest";
import { RangeSet, TextBuffer, Transaction } from "@the-editor/engine";
import {
  closeEditorScope,
  EditorNotifications,
  EditorVisuals,
  hasInvalidation,
  makeEditorScope,
  payloadsOf,
  provideEditorScope,
  VisualsInvalidated,
  type EditorNotification,
} from "../src/index";

const run = <A, E>(effect: Effect.Effect<A, E>) => Effect.runSync(effect);

const withEditor = <A>(
  f: (ctx: {
    visuals: EditorVisuals.Service;
    notifications: EditorNotifications.Service;
  }) => Effect.Effect<A, never, Scope.Scope>,
) =>
  Effect.gen(function* () {
    const editor = yield* makeEditorScope({ text: "alpha\nbeta\ngamma\n" });
    const result = yield* provideEditorScope(
      editor,
      Effect.gen(function* () {
        const visuals = yield* EditorVisuals;
        const notifications = yield* EditorNotifications;
        return yield* Effect.scoped(f({ visuals, notifications }));
      }),
    );
    yield* closeEditorScope(editor);
    return result;
  });

describe("EditorVisuals", () => {
  test("contributions are queryable through the merged snapshot", () => {
    const result = run(
      withEditor(({ visuals }) =>
        Effect.gen(function* () {
          yield* visuals.contribute("test/styles", {
            textStyles: RangeSet.of([
              { from: 0, to: 5, value: { className: "kw" } },
              { from: 100, to: 110, value: { className: "far" } },
            ]),
            decorations: RangeSet.of([
              { from: 6, to: 10, value: { layer: "background" as const, className: "hl" } },
            ]),
            virtualText: RangeSet.of([{ from: 5, to: 5, value: { text: " hint" } }]),
          });
          const snapshot = yield* visuals.snapshot;
          return {
            styles: snapshot.textStylesBetween(0, 20),
            decorations: snapshot.decorationsBetween(0, 20),
            virtual: snapshot.virtualTextBetween(0, 20),
          };
        }),
      ),
    );
    expect(result.styles).toEqual([{ from: 0, to: 5, owner: "test/styles", className: "kw" }]);
    expect(result.decorations).toEqual([
      { from: 6, to: 10, owner: "test/styles", layer: "background", className: "hl" },
    ]);
    expect(result.virtual).toEqual([{ at: 5, owner: "test/styles", text: " hint" }]);
  });

  test("closing the contributing scope retracts the visuals", () => {
    const result = run(
      withEditor(({ visuals }) =>
        Effect.gen(function* () {
          const inner = yield* Scope.make();
          yield* visuals
            .contribute("test/tmp", {
              textStyles: RangeSet.of([{ from: 0, to: 3, value: {} }]),
            })
            .pipe(Scope.extend(inner));
          const before = (yield* visuals.snapshot).textStylesBetween(0, 10).length;
          yield* Scope.close(inner, { _tag: "Success", value: undefined } as never);
          const after = (yield* visuals.snapshot).textStylesBetween(0, 10).length;
          return { before, after };
        }),
      ),
    );
    expect(result).toEqual({ before: 1, after: 0 });
  });

  test("updates publish VisualsInvalidated with changed ranges", async () => {
    const result = await Effect.runPromise(
      withEditor(({ visuals, notifications }) =>
        Effect.gen(function* () {
          const received: EditorNotification[] = [];
          const fiber = yield* Stream.runForEach(
            Stream.take(notifications.notifications, 2),
            (notification) => Effect.sync(() => received.push(notification)),
          ).pipe(Effect.fork);
          yield* Effect.yieldNow();
          const handle = yield* visuals.contribute("test/u", {
            decorations: RangeSet.of([{ from: 0, to: 2, value: { layer: "outline" as const } }]),
          });
          yield* handle.update(
            {
              decorations: RangeSet.of([{ from: 4, to: 8, value: { layer: "outline" as const } }]),
            },
            [{ from: 0, to: 8 }],
          );
          yield* Fiber.join(fiber);
          return received;
        }),
      ) as Effect.Effect<EditorNotification[], never>,
    );
    expect(result).toHaveLength(2);
    expect(hasInvalidation(result[1]!, VisualsInvalidated)).toBe(true);
    const payload = payloadsOf(result[1]!, VisualsInvalidated)[0]!;
    expect(payload.changes).toEqual([{ from: 0, to: 8 }]);
    expect(payload.generation).toBeGreaterThan(0);
  });

  test("mapChanges slides sets without bumping generation", () => {
    const result = run(
      withEditor(({ visuals }) =>
        Effect.gen(function* () {
          yield* visuals.contribute("test/map", {
            decorations: RangeSet.of([
              { from: 10, to: 14, value: { layer: "background" as const } },
            ]),
          });
          const before = yield* visuals.snapshot;
          const transaction = yield* Transaction.fromReplacements(100, [
            { from: 0, to: 0, text: "abc" },
          ]).pipe(Effect.orDie);
          yield* visuals.mapChanges(transaction.changes.desc());
          const after = yield* visuals.snapshot;
          return {
            generationBefore: before.generation,
            generationAfter: after.generation,
            moved: after.decorationsBetween(0, 50),
          };
        }),
      ),
    );
    expect(result.generationAfter).toBe(result.generationBefore);
    expect(result.moved).toEqual([{ from: 13, to: 17, owner: "test/map", layer: "background" }]);
  });

  test("widgets resolve to lines and gutter bands sort by side/priority", () => {
    const result = run(
      withEditor(({ visuals }) =>
        Effect.gen(function* () {
          yield* visuals.contribute("test/w", {
            blockWidgets: RangeSet.of([
              { from: 7, to: 7, value: { id: "w1", heightRows: 2 } }, // inside "beta" line
            ]),
            gutterBands: [
              { id: "b", side: "left", widthCh: 4, priority: 1 },
              { id: "a", side: "left", widthCh: 6, priority: 0 },
            ],
          });
          const snapshot = yield* visuals.snapshot;
          const text = TextBuffer.fromString("alpha\nbeta\ngamma\n");
          return {
            widgets: snapshot.widgets(text),
            totalRows: snapshot.totalWidgetRows,
            bands: snapshot.gutterBands.map((band) => band.id),
          };
        }),
      ),
    );
    expect(result.widgets).toEqual([
      { id: "w1", atOffset: 7, afterLine: 1, heightRows: 2, owner: "test/w" },
    ]);
    expect(result.totalRows).toBe(2);
    expect(result.bands).toEqual(["a", "b"]);
  });
});
