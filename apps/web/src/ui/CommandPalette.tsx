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
import { CommandDialog } from "@/ui/command";
import { VirtualCommand, type VirtualCommandRow } from "@/ui/virtual-command";

interface PaletteAction {
  readonly id: string;
  readonly title: string;
  readonly command: string;
  readonly aliases: readonly string[];
  readonly group: string;
}

export type CommandPaletteMode = "commands" | "themes" | "workspaces";

/** A single open workspace, projected by the host for the switcher mode. */
export interface WorkspaceItem {
  readonly id: string;
  readonly label: string;
}

/** The host's workspace switcher data + actions. The palette is a pure view:
 *  it lists/opens/closes and emits the intent; it never owns session lifecycle. */
export interface WorkspacePalette {
  readonly items: readonly WorkspaceItem[];
  readonly activeId: string | null;
  /** False when only one workspace is open (the last can't be closed). */
  readonly canClose: boolean;
  readonly onSelect: (id: string) => void;
  readonly onClose: (id: string) => void;
  readonly onOpenFolder: () => void;
}

const defaultGroup = "Commands";

const paletteDefinition = (
  palette: boolean | CommandPaletteDefinition | undefined,
): CommandPaletteDefinition | undefined => {
  if (palette === true) return { enabled: true };
  if (typeof palette === "object" && palette.enabled !== false) return palette;
  return undefined;
};
const normalizeSearch = (value: string): string => value.trim().toLowerCase();

const actionRank = (action: PaletteAction, query: string): number => {
  if (query.length === 0) return 0;
  const terms = [action.title, action.command, ...action.aliases].map(normalizeSearch);
  if (terms.some((term) => term === query)) return 0;
  if (terms.some((term) => term.startsWith(query))) return 1;
  if (terms.some((term) => term.split(/\s+/).some((word) => word.startsWith(query)))) return 2;
  if (terms.some((term) => term.includes(query))) return 3;
  return 4;
};

const compareActions = (left: PaletteAction, right: PaletteAction, query: string): number =>
  actionRank(left, query) - actionRank(right, query) ||
  left.title.localeCompare(right.title) ||
  left.command.localeCompare(right.command);

const ROW_HEIGHT = 26;

/** A flat palette row. The command palette hides group headings (CSS), so all
 *  rows are selectable items — no heading rows. */
type PaletteRow = VirtualCommandRow &
  (
    | { readonly kind: "command"; readonly action: PaletteAction }
    | { readonly kind: "theme"; readonly theme: ThemeMeta }
    | { readonly kind: "workspace"; readonly workspace: WorkspaceItem }
    | { readonly kind: "open-folder" }
  );

export interface CommandPaletteProps {
  readonly open: boolean;
  readonly mode: CommandPaletteMode;
  readonly onOpenChange: (open: boolean) => void;
  readonly commands: Commands.Service;
  readonly themes: Themes.Service;
  readonly workspaces: WorkspacePalette;
}

export function CommandPalette({
  open,
  mode,
  onOpenChange,
  commands,
  themes,
  workspaces,
}: CommandPaletteProps) {
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

    if (mode === "workspaces") {
      setSelectedValue(workspaces.activeId ?? workspaces.items[0]?.id ?? "");
      return;
    }

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
          setSelectedValue(nextActions[0]?.id ?? "");
        }
      })
      .catch((error) => {
        if (!cancelled) console.error(error);
      });

    return () => {
      cancelled = true;
    };
  }, [commands, mode, open, themes, workspaces]);

  useEffect(() => {
    if (!open || mode !== "themes" || selectedValue.length === 0) return;
    Effect.runFork(
      themes.preview(selectedValue).pipe(Effect.catchAll((error) => Effect.sync(() => console.error(error)))),
    );
  }, [mode, open, selectedValue, themes]);

  // The windowed rows for the active mode. Commands are ranked/filtered/grouped
  // here (built-in filtering off); themes/workspaces use the built-in substring
  // filter over `searchText`.
  const rows = useMemo<ReadonlyArray<PaletteRow>>(() => {
    if (mode === "themes") {
      return themeItems.map((theme) => ({
        id: theme.name,
        kind: "theme" as const,
        theme,
        searchText: `${theme.name} ${theme.family} ${theme.author} ${theme.appearance}`.toLowerCase(),
      }));
    }
    if (mode === "workspaces") {
      return [
        { id: "__open_folder__", kind: "open-folder" as const, searchText: "open folder open workspace new workspace add" },
        ...workspaces.items.map((workspace) => ({
          id: workspace.id,
          kind: "workspace" as const,
          workspace,
          searchText: workspace.label.toLowerCase(),
        })),
      ];
    }
    const query = normalizeSearch(search);
    const matched = query.length === 0 ? actions : actions.filter((action) => actionRank(action, query) < 4);
    const ranked = query.length === 0 ? matched : [...matched].sort((left, right) => compareActions(left, right, query));
    const groups = new Map<string, PaletteAction[]>();
    for (const action of ranked) groups.set(action.group, [...(groups.get(action.group) ?? []), action]);
    return [...groups.values()].flat().map((action) => ({ id: action.id, kind: "command" as const, action }));
  }, [mode, actions, themeItems, workspaces, search]);

  const useBuiltinFilter = mode !== "commands";

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

  const selectWorkspace = (id: string) => {
    onOpenChange(false);
    workspaces.onSelect(id);
  };

  const openWorkspaceFolder = () => {
    onOpenChange(false);
    workspaces.onOpenFolder();
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
      title={
        mode === "themes"
          ? "Select Theme"
          : mode === "workspaces"
            ? "Switch Workspace"
            : "Command Palette"
      }
      className="command-palette-dialog"
    >
      <VirtualCommand<PaletteRow>
        className="command-palette command-root"
        listClassName="command-palette__list command-list"
        inputClassName="command-palette__input command-input"
        inputRef={inputRef}
        placeholder={
          mode === "themes"
            ? "Select a theme…"
            : mode === "workspaces"
              ? "Switch workspace…"
              : "Type a command…"
        }
        query={search}
        onQueryChange={setSearch}
        filter={useBuiltinFilter}
        rows={rows}
        rowHeight={ROW_HEIGHT}
        value={selectedValue}
        onValueChange={setSelectedValue}
        onChoose={(row) => {
          if (row.kind === "command") run(row.action);
          else if (row.kind === "theme") selectTheme(row.theme.name);
          else if (row.kind === "workspace") selectWorkspace(row.workspace.id);
          else openWorkspaceFolder();
        }}
        empty={
          <div className="command-palette__empty command-empty" data-slot="command-empty">
            {mode === "themes"
              ? "No themes found."
              : mode === "workspaces"
                ? "No workspaces found."
                : "No commands found."}
          </div>
        }
        renderRow={(row, { selected }) => (
          <div
            className="command-palette__item command-item"
            data-slot="command-item"
            data-selected={selected ? "true" : undefined}
          >
            {row.kind === "command" ? (
              <span className="command-palette__item-title">{row.action.title}</span>
            ) : row.kind === "theme" ? (
              <>
                <span className="command-palette__item-title">{row.theme.name}</span>
                <span className="command-palette__shortcut">{row.theme.appearance}</span>
              </>
            ) : row.kind === "open-folder" ? (
              <span className="command-palette__item-title">Open Folder…</span>
            ) : (
              <>
                <span className="command-palette__item-title">{row.workspace.label}</span>
                {row.workspace.id === workspaces.activeId && (
                  <span className="command-palette__shortcut">active</span>
                )}
                {workspaces.canClose && (
                  <button
                    type="button"
                    className="command-palette__row-close"
                    aria-label={`Close workspace ${row.workspace.label}`}
                    onClick={(event) => {
                      event.stopPropagation();
                      workspaces.onClose(row.workspace.id);
                    }}
                  >
                    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
                      <path d="M1 1l8 8M9 1l-8 8" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
                    </svg>
                  </button>
                )}
              </>
            )}
          </div>
        )}
      />
    </CommandDialog>
  );
}
