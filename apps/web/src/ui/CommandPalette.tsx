/**
 * The command palette: a thin React shell over the Commands service. The
 * registry is the single source of truth — any package's command appears
 * here by declaring `palette` on its contribution; availability is
 * structural (the palette lists through `listAvailable`, so commands scoped
 * to an unfocused surface simply aren't offered).
 */

import { useEffect, useMemo, useRef, useState } from "react";
import { Effect } from "effect";
import type { Commands, CommandPaletteDefinition } from "@the-editor/commands";
import type { Themes, ThemeMeta } from "@the-editor/theme";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/ui/command";

interface PaletteAction {
  readonly id: string;
  readonly title: string;
  readonly command: string;
  readonly aliases: readonly string[];
  readonly group: string;
}

export type CommandPaletteMode = "commands" | "themes";

const defaultGroup = "Commands";

const paletteDefinition = (
  palette: boolean | CommandPaletteDefinition | undefined,
): CommandPaletteDefinition | undefined => {
  if (palette === true) return { enabled: true };
  if (typeof palette === "object" && palette.enabled !== false) return palette;
  return undefined;
};

export interface CommandPaletteProps {
  readonly open: boolean;
  readonly mode: CommandPaletteMode;
  readonly onOpenChange: (open: boolean) => void;
  readonly commands: Commands.Service;
  readonly themes: Themes.Service;
}

export function CommandPalette({ open, mode, onOpenChange, commands, themes }: CommandPaletteProps) {
  const [actions, setActions] = useState<readonly PaletteAction[]>([]);
  const [themeItems, setThemeItems] = useState<readonly ThemeMeta[]>([]);
  const [search, setSearch] = useState("");
  const [selectedValue, setSelectedValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const confirmedTheme = useRef(false);

  useEffect(() => {
    if (!open) return;
    const id = window.requestAnimationFrame(() => inputRef.current?.focus());
    return () => window.cancelAnimationFrame(id);
  }, [open]);

  useEffect(() => {
    if (!open) {
      setSearch("");
      setSelectedValue("");
      setActions([]);
      setThemeItems([]);
      return;
    }

    let cancelled = false;
    confirmedTheme.current = false;
    setSearch("");

    if (mode === "themes") {
      Effect.runPromise(
        Effect.all({ active: themes.active, list: themes.list }),
      )
        .then(({ active, list }) => {
          if (cancelled) return;
          setThemeItems(list);
          setSelectedValue(active.name);
        })
        .catch((error) => {
          if (!cancelled) console.error(error);
        });
      return () => {
        cancelled = true;
      };
    }

    Effect.runPromise(
      commands.listAvailable().pipe(
        Effect.map((registered) =>
          registered.flatMap((entry): PaletteAction[] => {
            const palette = paletteDefinition(entry.contribution.palette);
            if (!palette) return [];

            return [
              {
                id: entry.contribution.id,
                title: entry.contribution.title ?? entry.contribution.id,
                command: entry.contribution.id,
                aliases: palette.aliases ?? [],
                group: palette.group ?? defaultGroup,
              },
            ];
          }),
        ),
      ),
    )
      .then((nextActions) => {
        if (!cancelled) {
          setActions(nextActions);
          setSelectedValue(nextActions[0]?.title ?? "");
        }
      })
      .catch((error) => {
        if (!cancelled) console.error(error);
      });

    return () => {
      cancelled = true;
    };
  }, [commands, mode, open, themes]);

  useEffect(() => {
    if (!open || mode !== "themes" || selectedValue.length === 0) return;
    Effect.runFork(
      themes.preview(selectedValue).pipe(Effect.catchAll((error) => Effect.sync(() => console.error(error)))),
    );
  }, [mode, open, selectedValue, themes]);

  const groupedActions = useMemo(() => {
    const groups = new Map<string, readonly PaletteAction[]>();
    for (const action of actions) {
      groups.set(action.group, [...(groups.get(action.group) ?? []), action]);
    }
    return [...groups.entries()];
  }, [actions]);

  const exactAliasAction = useMemo(() => {
    const normalizedSearch = search.trim().toLowerCase();
    if (!normalizedSearch || mode !== "commands") return undefined;
    return actions.find((action) =>
      action.aliases.some((alias) => alias.toLowerCase() === normalizedSearch),
    );
  }, [actions, mode, search]);

  const requestOpenChange = (nextOpen: boolean) => {
    if (!nextOpen && mode === "themes" && !confirmedTheme.current) {
      Effect.runFork(themes.cancelPreview.pipe(Effect.ignore));
    }
    onOpenChange(nextOpen);
  };

  const run = (action: PaletteAction) => {
    onOpenChange(false);
    Effect.runFork(
      commands
        .execute(action.command)
        .pipe(Effect.catchAll((error) => Effect.sync(() => console.error(error)))),
    );
  };

  const selectTheme = (themeName: string) => {
    confirmedTheme.current = true;
    Effect.runFork(
      themes.commitPreview.pipe(Effect.andThen(themes.setTheme(themeName)), Effect.ignore),
    );
    onOpenChange(false);
  };

  return (
    <CommandDialog
      open={open}
      onOpenChange={requestOpenChange}
      title={mode === "themes" ? "Select Theme" : "Command Palette"}
      className="command-palette-dialog"
    >
      <Command
        className="command-palette"
        value={selectedValue}
        onValueChange={setSelectedValue}
        onKeyDownCapture={(event) => {
          if (event.key !== "Enter" || event.nativeEvent.isComposing || !exactAliasAction) return;
          event.preventDefault();
          event.stopPropagation();
          run(exactAliasAction);
        }}
      >
        <CommandInput
          ref={inputRef}
          className="command-palette__input"
          placeholder={mode === "themes" ? "Select a theme…" : "Type a command…"}
          value={search}
          onValueChange={setSearch}
        />
        <CommandList className="command-palette__list">
          <CommandEmpty className="command-palette__empty">
            {mode === "themes" ? "No themes found." : "No commands found."}
          </CommandEmpty>
          {mode === "themes" ? (
            <CommandGroup className="command-palette__group" heading="Themes">
              {themeItems.map((theme) => (
                <CommandItem
                  className="command-palette__item"
                  key={theme.name}
                  value={theme.name}
                  keywords={[theme.family, theme.author, theme.appearance]}
                  onSelect={() => selectTheme(theme.name)}
                >
                  <span className="command-palette__item-title">{theme.name}</span>
                  <span className="command-palette__shortcut">{theme.appearance}</span>
                </CommandItem>
              ))}
            </CommandGroup>
          ) : (
            groupedActions.map(([group, groupActions]) => (
              <CommandGroup className="command-palette__group" heading={group} key={group}>
                {groupActions.map((action) => (
                  <CommandItem
                    className="command-palette__item"
                    key={action.id}
                    value={action.title}
                    keywords={[...action.aliases, action.command]}
                    onSelect={() => run(action)}
                  >
                    <span className="command-palette__item-title">{action.title}</span>
                  </CommandItem>
                ))}
              </CommandGroup>
            ))
          )}
        </CommandList>
      </Command>
    </CommandDialog>
  );
}
