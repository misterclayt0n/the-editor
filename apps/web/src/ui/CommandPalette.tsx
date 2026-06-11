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
  readonly onOpenChange: (open: boolean) => void;
  readonly commands: Commands.Service;
}

export function CommandPalette({ open, onOpenChange, commands }: CommandPaletteProps) {
  const [actions, setActions] = useState<readonly PaletteAction[]>([]);
  const [search, setSearch] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    const id = window.requestAnimationFrame(() => inputRef.current?.focus());
    return () => window.cancelAnimationFrame(id);
  }, [open]);

  useEffect(() => {
    if (!open) {
      setSearch("");
      return;
    }

    let cancelled = false;
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
        if (!cancelled) setActions(nextActions);
      })
      .catch((error) => {
        if (!cancelled) console.error(error);
      });

    return () => {
      cancelled = true;
    };
  }, [commands, open]);

  const groupedActions = useMemo(() => {
    const groups = new Map<string, readonly PaletteAction[]>();
    for (const action of actions) {
      groups.set(action.group, [...(groups.get(action.group) ?? []), action]);
    }
    return [...groups.entries()];
  }, [actions]);

  const exactAliasAction = useMemo(() => {
    const normalizedSearch = search.trim().toLowerCase();
    if (!normalizedSearch) return undefined;
    return actions.find((action) =>
      action.aliases.some((alias) => alias.toLowerCase() === normalizedSearch),
    );
  }, [actions, search]);

  const run = (action: PaletteAction) => {
    onOpenChange(false);
    Effect.runFork(
      commands
        .execute(action.command)
        .pipe(Effect.catchAll((error) => Effect.sync(() => console.error(error)))),
    );
  };

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Command Palette"
      className="command-palette-dialog"
    >
      <Command
        className="command-palette"
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
          placeholder="Type a command…"
          value={search}
          onValueChange={setSearch}
        />
        <CommandList className="command-palette__list">
          <CommandEmpty className="command-palette__empty">No commands found.</CommandEmpty>
          {groupedActions.map(([group, groupActions]) => (
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
          ))}
        </CommandList>
      </Command>
    </CommandDialog>
  );
}
