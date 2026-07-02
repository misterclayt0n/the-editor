/**
 * The workspace switcher as a picker request. App owns session lifecycle and
 * projects it here; the switcher doubles as the manager (open a folder, close
 * a row), which is why rows use the `renderItem` escape hatch — the inline
 * close button is bespoke interactivity the data fields don't model.
 */

import type { PickerItem, PickerRequest } from "@the-editor/picker";

/** A single open workspace, projected by the host for the switcher. */
export interface WorkspaceItem {
  readonly id: string;
  readonly label: string;
}

/** The host's switcher data + actions. The picker is a pure view over this:
 *  it lists/opens/closes and emits the intent; it never owns sessions. */
export interface WorkspacePalette {
  readonly items: readonly WorkspaceItem[];
  readonly activeId: string | null;
  /** False when only one workspace is open (the last can't be closed). */
  readonly canClose: boolean;
  readonly onSelect: (id: string) => void;
  readonly onClose: (id: string) => void;
  readonly onOpenFolder: () => void;
}

type SwitcherEntry = { readonly kind: "workspace"; readonly id: string } | { readonly kind: "open-folder" };

const OPEN_FOLDER_ID = "__open_folder__";

export function workspaceSwitcherRequest(
  /** Live getter: the switcher stays open across row-close mutations, so the
   *  source must re-read App's current session list on every (re)query. */
  getWorkspaces: () => WorkspacePalette,
  /** Re-runs the open picker's query after a row action mutates the data. */
  refresh: () => void,
): PickerRequest<SwitcherEntry> {
  const initial = getWorkspaces();
  return {
    title: "Switch Workspace",
    ...(initial.activeId !== null ? { initialSelectedId: initial.activeId } : {}),
    source: {
      id: "the-editor/workspaces",
      placeholder: "Switch workspace…",
      query: () => {
        const workspaces = getWorkspaces();
        return {
          items: [
            {
              id: OPEN_FOLDER_ID,
              label: "Open Folder…",
              searchTerms: ["open folder", "open workspace", "new workspace", "add"],
              data: { kind: "open-folder" as const },
            },
            ...workspaces.items.map(
              (workspace): PickerItem<SwitcherEntry> => ({
                id: workspace.id,
                label: workspace.label,
                ...(workspace.id === workspaces.activeId ? { hint: "active" } : {}),
                data: { kind: "workspace" as const, id: workspace.id },
              }),
            ),
          ],
        };
      },
    },
    onChoose: (item) => {
      const workspaces = getWorkspaces();
      if (item.data.kind === "open-folder") workspaces.onOpenFolder();
      else workspaces.onSelect(item.data.id);
    },
    renderItem: (item, { selected }) => (
      <div
        className="command-palette__item command-item"
        data-slot="command-item"
        data-selected={selected ? "true" : undefined}
      >
        <span className="command-palette__item-title">{item.label}</span>
        {item.hint !== undefined && <span className="command-palette__shortcut">{item.hint}</span>}
        {item.data.kind === "workspace" && getWorkspaces().canClose && (
          <button
            type="button"
            className="command-palette__row-close"
            aria-label={`Close workspace ${item.label}`}
            onClick={(event) => {
              event.stopPropagation();
              getWorkspaces().onClose(item.id);
              refresh();
            }}
          >
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
              <path
                d="M1 1l8 8M9 1l-8 8"
                stroke="currentColor"
                strokeWidth="1.3"
                strokeLinecap="round"
              />
            </svg>
          </button>
        )}
      </div>
    ),
  };
}
