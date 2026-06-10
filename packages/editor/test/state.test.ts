import { Effect, Option } from "effect";
import { describe, expect, test } from "vitest";
import { ActiveEditor, ActiveEditorLive, EditorScopeId } from "../src/index";

const run = <A, E>(effect: Effect.Effect<A, E>) => Effect.runSync(effect);

describe("editor state", () => {
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
