import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { defaultFontConfig } from "./defaults.js";
import { Fonts } from "./Fonts.js";

export const fontDefaults = definePackage("the-editor/font-defaults", () =>
  Effect.gen(function* () {
    const fonts = yield* Fonts;
    yield* fonts.configure(defaultFontConfig);
  }),
);
