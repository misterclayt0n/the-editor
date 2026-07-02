/**
 * Theme commands — fully on public seams: `Themes` (the data + preview
 * transaction) and `Picker` (the surface). The selector demonstrates the
 * picker's live-selection contract: preview as the highlight moves, commit on
 * choose, revert on dismiss.
 */

import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import { Themes } from "@the-editor/theme";
import { SurfaceScope } from "@the-editor/workbench";
import { Picker } from "@the-editor/picker";

export const themeCommands = definePackage("the-editor/theme-commands", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;
    const themes = yield* Themes;
    const picker = yield* Picker;

    yield* commands.register("theme.select", {
      title: "Select Theme",
      scope: SurfaceScope.global,
      palette: { group: "Preferences", aliases: ["theme"] },
      run: () =>
        Effect.gen(function* () {
          const { active, list } = yield* Effect.all({ active: themes.active, list: themes.list });
          picker.open<string>({
            title: "Select Theme",
            initialSelectedId: active.name,
            source: {
              id: "the-editor/themes",
              placeholder: "Select a theme…",
              query: () => ({
                items: list.map((theme) => ({
                  id: theme.name,
                  label: theme.name,
                  hint: theme.appearance,
                  searchTerms: [theme.family, theme.author, theme.appearance],
                  data: theme.name,
                })),
              }),
            },
            onSelectionChange: (item) => {
              if (item === null) return;
              Effect.runFork(
                themes
                  .preview(item.data)
                  .pipe(Effect.catchAll((error) => Effect.sync(() => console.error(error)))),
              );
            },
            onChoose: (item) => {
              Effect.runFork(
                themes.commitPreview.pipe(
                  Effect.andThen(themes.setTheme(item.data)),
                  Effect.ignore,
                ),
              );
            },
            onDismiss: () => {
              Effect.runFork(themes.cancelPreview.pipe(Effect.ignore));
            },
          });
        }),
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
