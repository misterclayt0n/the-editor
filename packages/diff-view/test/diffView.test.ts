import { Effect } from "effect";
import { describe, expect, it } from "vitest";
import { makeDiffViewPreferences } from "../src/index";

describe("DiffViewPreferences", () => {
  it("starts with hidden controls", () => {
    expect(makeDiffViewPreferences().current().controlsVisible).toBe(false);
  });

  it("layers scoped contributions and runtime overrides", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const preferences = makeDiffViewPreferences();
        const during = yield* Effect.scoped(
          Effect.gen(function* () {
            yield* preferences.register({ id: "a", settings: { wrapping: true } });
            preferences.update({ lineNumbers: false });
            return preferences.current();
          }),
        );
        const after = preferences.current();
        return { during, after };
      }),
    );

    expect(result.during.wrapping).toBe(true);
    expect(result.during.lineNumbers).toBe(false);
    expect(result.after.wrapping).toBe(false);
    expect(result.after.lineNumbers).toBe(false);
  });
});
