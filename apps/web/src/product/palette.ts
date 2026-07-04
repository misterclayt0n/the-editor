/**
 * The command palette as a product package — and the first first-party
 * surface built ENTIRELY on public seams: `Commands` (the data) + `Picker`
 * (the surface). No host tag, no host callback; a user extension could ship
 * this exact file. The registry stays the single source of truth: any
 * package's command appears by declaring `palette` on its contribution, and
 * availability is structural (`listAvailable` omits commands scoped to
 * unfocused surfaces).
 */

import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands, type CommandPaletteDefinition } from "@the-editor/commands";
import { Picker, type PickerItem } from "@the-editor/picker";

const defaultGroup = "Commands";

const paletteDefinition = (
  palette: boolean | CommandPaletteDefinition | undefined,
): CommandPaletteDefinition | undefined => {
  if (palette === true) return { enabled: true };
  if (typeof palette === "object" && palette.enabled !== false) return palette;
  return undefined;
};

export const palettePackage = definePackage("the-editor/palette", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const picker = yield* Picker;

    const listActions = (): Promise<ReadonlyArray<PickerItem<string>>> =>
      Effect.runPromise(
        commands.listAvailable().pipe(
          Effect.map((registered) => {
            const groups = new Map<string, PickerItem<string>[]>();
            for (const entry of registered) {
              const palette = paletteDefinition(entry.contribution.palette);
              if (palette === undefined) continue;
              const group = palette.group ?? defaultGroup;
              const item: PickerItem<string> = {
                id: entry.contribution.id,
                label: entry.contribution.title ?? entry.contribution.id,
                searchTerms: [entry.contribution.id, ...(palette.aliases ?? [])],
                data: entry.contribution.id,
              };
              groups.set(group, [...(groups.get(group) ?? []), item]);
            }
            // Group clusters survive ranking: the controller's sort is stable,
            // so at the empty query the palette keeps registration order.
            return [...groups.values()].flat();
          }),
        ),
      );

    // Global scope: the palette opens from anywhere. It does not list
    // itself (no palette metadata) — it is already open when you'd see it.
    yield* commands.register("palette.open", {
      title: "Command Palette",
      run: () =>
        Effect.sync(() =>
          picker.open<string>({
            title: "Command Palette",
            source: {
              id: "the-editor/commands",
              placeholder: "Type a command…",
              query: () => listActions().then((items) => ({ items })),
            },
            onChoose: (item) => {
              Effect.runFork(
                commands
                  .execute(item.data)
                  .pipe(Effect.catchAll((error) => Effect.sync(() => console.error(error)))),
              );
            },
          }),
        ),
    });

  }),
);
