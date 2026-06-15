import { Context, Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import { Themes } from "@the-editor/theme";
import { SurfaceScope } from "@the-editor/workbench";

export class HostThemeActions extends Context.Tag("the-editor/web/HostThemeActions")<
  HostThemeActions,
  { readonly openSelector: () => void }
>() {}

export const themeCommands = definePackage("the-editor/theme-commands", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;
    const themes = yield* Themes;
    const host = yield* HostThemeActions;

    yield* commands.register("theme.select", {
      title: "Select Theme",
      scope: SurfaceScope.global,
      palette: { group: "Preferences", aliases: ["theme"] },
      run: () => Effect.sync(() => host.openSelector()),
    });

    yield* commands.register("theme.toggleLightDark", {
      title: "Toggle Light/Dark Theme",
      scope: SurfaceScope.global,
      palette: { group: "Preferences", aliases: ["toggle theme", "dark", "light"] },
      run: () => themes.toggleMode,
    });

    yield* keymaps.registerAll([
      { keys: "meta+k meta+t", command: "theme.select", scope: SurfaceScope.global },
      { keys: "meta+k meta+l", command: "theme.toggleLightDark", scope: SurfaceScope.global },
    ]);
  }),
);
