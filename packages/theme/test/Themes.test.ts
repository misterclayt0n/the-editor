import { describe, expect, it } from "vitest";
import { Effect, Scope } from "effect";
import { PackageActivationContext, type ExtensionContext } from "@the-editor/extensions";
import { ActivationId, PackageId } from "@the-editor/kernel";
import { makeThemes, parseThemeFamily } from "../src/Themes.js";
import { pierreThemeFamily } from "../src/defaults.js";

const activation = {
  packageId: PackageId("test/theme"),
  activationId: ActivationId("test/theme#1"),
  owner: { packageId: PackageId("test/theme"), activationId: ActivationId("test/theme#1") },
  contributions: {
    register: () => Effect.die("not used"),
  },
  stores: {
    activation: () => Effect.die("not used"),
    package: () => Effect.die("not used"),
    application: () => Effect.die("not used"),
  },
} satisfies ExtensionContext;

const provideActivation = <A, E, R>(effect: Effect.Effect<A, E, R | PackageActivationContext | Scope.Scope>) =>
  effect.pipe(Effect.provideService(PackageActivationContext, activation));

describe("Themes", () => {
  it("loads Zed-compatible theme families and resolves active theme", () => {
    const result = Effect.runSync(
      Effect.scoped(Effect.gen(function* () {
        const themes = yield* makeThemes();
        yield* provideActivation(themes.registerFamily(pierreThemeFamily));
        const active = yield* themes.active;
        const list = yield* themes.list;
        return { active, list };
      })),
    );

    expect(result.active.name).toBe("Pierre Dark");
    expect(result.active.cssVars["--bg-editor"]).toBe("#0a0a0a");
    expect(result.list.map((theme) => theme.name)).toEqual([
      "Pierre Dark",
      "Pierre Dark Soft",
      "Pierre Light",
      "Pierre Light Soft",
    ]);
  });

  it("previews and rolls back without changing theme slots", () => {
    const result = Effect.runSync(
      Effect.scoped(Effect.gen(function* () {
        const themes = yield* makeThemes();
        yield* provideActivation(themes.registerFamily(pierreThemeFamily));
        yield* themes.preview("Pierre Light");
        const previewed = yield* themes.active;
        yield* themes.cancelPreview;
        const restored = yield* themes.active;
        return { previewed: previewed.name, restored: restored.name };
      })),
    );

    expect(result).toEqual({ previewed: "Pierre Light", restored: "Pierre Dark" });
  });

  it("uses dotted capture resolution", () => {
    const color = Effect.runSync(
      Effect.scoped(Effect.gen(function* () {
        const themes = yield* makeThemes();
        yield* provideActivation(themes.registerFamily(pierreThemeFamily));
        return yield* themes.syntaxStyle("function.method.async");
      })),
    );

    expect(color?.color).toBe("#9d6afb");
  });

  it("maps Helix capture names onto Zed syntax slots", () => {
    const styles = Effect.runSync(
      Effect.scoped(Effect.gen(function* () {
        const themes = yield* makeThemes();
        yield* provideActivation(themes.registerFamily(pierreThemeFamily));
        return {
          property: yield* themes.syntaxStyle("variable.other.member"),
          number: yield* themes.syntaxStyle("constant.numeric.integer"),
          regex: yield* themes.syntaxStyle("string.regexp"),
        };
      })),
    );

    expect(styles.property?.color).toBe("#ffa359");
    expect(styles.number?.color).toBe("#68cdf2");
    expect(styles.regex?.color).toBe("#64d1db");
  });

  it("validates the JSON boundary", () => {
    const parsed = Effect.runSync(
      parseThemeFamily({
        name: "Custom",
        author: "Tester",
        themes: [{ name: "Custom Dark", appearance: "dark", style: { background: "#000000ff" } }],
      }),
    );

    expect(parsed.themes[0]?.name).toBe("Custom Dark");
  });
});
