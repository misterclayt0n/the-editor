/**
 * Palette opening as a product package: "palette.open" is an ordinary
 * command with ordinary keybindings, so it is itself rebindable. The host
 * owns the dialog (a React surface) through PaletteOperations.
 */

import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import { PaletteOperations } from "@the-editor/editor";

export const palettePackage = definePackage("the-editor/palette", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;
    const palette = yield* PaletteOperations;

    // Global scope: the palette opens from anywhere. It does not list
    // itself (no palette metadata) — it is already open when you'd see it.
    yield* commands.register("palette.open", {
      title: "Command Palette",
      run: () => Effect.sync(() => palette.open()),
    });

    yield* keymaps.registerAll([
      { keys: "meta+shift+p", command: "palette.open" },
      { keys: "ctrl+shift+p", command: "palette.open" },
    ]);
  }),
);
