import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { pierreThemeFamily } from "./defaults.js";
import { Themes } from "./Themes.js";

export const themeDefaults = definePackage("the-editor/theme-defaults", () =>
  Effect.gen(function* () {
    const themes = yield* Themes;
    yield* themes.registerFamily(pierreThemeFamily);
  }),
);
