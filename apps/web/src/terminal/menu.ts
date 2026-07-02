/**
 * The terminal's context menu as registry ENTRIES (data), not a component.
 * The terminal package registers a provider that turns a popup's per-instance
 * data into these entries; the host draws them with the OS menu.
 */

import type { ContextMenuEntry } from "@the-editor/workbench";
import type { TerminalShortcutPlatform } from "./shortcuts";

export type TerminalContextMenuActions = {
  readonly copy: () => void | Promise<void>;
  readonly paste: () => void | Promise<void>;
  readonly clear: () => void;
  readonly find: () => void;
  readonly selectAll: () => void;
  readonly focus: () => void;
};

/** Per-popup data a terminal instance hands the shared provider. */
export type TerminalMenuData = {
  readonly actions: TerminalContextMenuActions;
  readonly canCopy: () => boolean;
  readonly platform: TerminalShortcutPlatform;
};

const shortcut = (
  platform: TerminalShortcutPlatform,
  action: "copy" | "paste" | "clear" | "find" | "selectAll",
): string => {
  const mod = platform === "darwin" ? "⌘" : "Ctrl+";
  switch (action) {
    case "copy":
      return platform === "darwin" ? "⌘C" : "Ctrl+Shift+C";
    case "paste":
      return platform === "darwin" ? "⌘V" : "Ctrl+V";
    case "clear":
      return platform === "darwin" ? "⌘K" : "Ctrl+Shift+K";
    case "find":
      return `${mod}F`;
    case "selectAll":
      return platform === "darwin" ? "⌘A" : "Ctrl+Shift+A";
  }
};

export const isTerminalMenuData = (data: unknown): data is TerminalMenuData =>
  typeof data === "object" && data !== null && "actions" in data && "canCopy" in data;

export function terminalMenuEntries(data: TerminalMenuData): ContextMenuEntry[] {
  const { actions, platform } = data;
  return [
    {
      id: "terminal.copy",
      label: "Copy",
      enabled: data.canCopy(),
      accelerator: shortcut(platform, "copy"),
      run: () => void actions.copy(),
    },
    {
      id: "terminal.paste",
      label: "Paste",
      accelerator: shortcut(platform, "paste"),
      run: () => void actions.paste(),
    },
    { separator: true },
    {
      id: "terminal.selectAll",
      label: "Select All",
      accelerator: shortcut(platform, "selectAll"),
      run: () => actions.selectAll(),
    },
    {
      id: "terminal.clear",
      label: "Clear",
      accelerator: shortcut(platform, "clear"),
      run: () => actions.clear(),
    },
    { separator: true },
    {
      id: "terminal.find",
      label: "Find",
      accelerator: shortcut(platform, "find"),
      run: () => actions.find(),
    },
  ];
}
