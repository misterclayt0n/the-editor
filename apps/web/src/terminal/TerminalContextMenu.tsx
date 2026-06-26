import * as React from "react";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuShortcut,
  ContextMenuTrigger,
} from "@/ui/context-menu";
import type { TerminalShortcutPlatform } from "./shortcuts";

export type TerminalContextMenuActions = {
  readonly copy: () => void | Promise<void>;
  readonly paste: () => void | Promise<void>;
  readonly clear: () => void;
  readonly find: () => void;
  readonly selectAll: () => void;
  readonly focus: () => void;
};

export type TerminalContextMenuProps = {
  readonly actions: TerminalContextMenuActions;
  readonly canCopy: () => boolean;
  readonly onBodyReady: (element: HTMLDivElement | null) => void;
  readonly platform: TerminalShortcutPlatform;
};

const shortcut = (platform: TerminalShortcutPlatform, action: "copy" | "paste" | "clear" | "find" | "selectAll"): string => {
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

export function TerminalContextMenu({
  actions,
  canCopy,
  onBodyReady,
  platform,
}: TerminalContextMenuProps): React.JSX.Element {
  const [copyEnabled, setCopyEnabled] = React.useState(false);

  const onOpenChange = React.useCallback((open: boolean) => {
    if (open) setCopyEnabled(canCopy());
  }, [canCopy]);

  const ignoreSearchContextMenu = React.useCallback((event: React.MouseEvent<HTMLDivElement>) => {
    const target = event.target;
    if (target instanceof Element && target.closest(".terminal-search")) {
      event.stopPropagation();
    }
  }, []);

  return (
    <ContextMenu onOpenChange={onOpenChange}>
      <ContextMenuTrigger asChild>
        <div
          ref={onBodyReady}
          className="terminal-body"
          onContextMenuCapture={ignoreSearchContextMenu}
        />
      </ContextMenuTrigger>
      <ContextMenuContent
        className="terminal-context-menu-content"
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          actions.focus();
        }}
      >
        <ContextMenuItem
          disabled={!copyEnabled}
          onSelect={() => {
            void actions.copy();
          }}
        >
          Copy
          <ContextMenuShortcut>{shortcut(platform, "copy")}</ContextMenuShortcut>
        </ContextMenuItem>
        <ContextMenuItem
          onSelect={() => {
            void actions.paste();
          }}
        >
          Paste
          <ContextMenuShortcut>{shortcut(platform, "paste")}</ContextMenuShortcut>
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem onSelect={actions.selectAll}>
          Select All
          <ContextMenuShortcut>{shortcut(platform, "selectAll")}</ContextMenuShortcut>
        </ContextMenuItem>
        <ContextMenuItem onSelect={actions.clear}>
          Clear
          <ContextMenuShortcut>{shortcut(platform, "clear")}</ContextMenuShortcut>
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem onSelect={actions.find}>
          Find
          <ContextMenuShortcut>{shortcut(platform, "find")}</ContextMenuShortcut>
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
