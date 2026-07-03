/**
 * The terminal as a product package: a workbench panel registered through
 * PanelRenderers, like the file tree. The package owns command identity and
 * the renderer registration; the host seam supplies the renderer (an
 * emulator engine — xterm.js by default, ghostty-web via config — wired to a
 * PTY) and the region toggle (arrangement surgery — bottom tab group).
 */

import { Context, Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import {
  ContextMenuProviders,
  PanelRenderers,
  SurfaceKind,
  SurfaceScope,
  type PanelRenderer,
} from "@the-editor/workbench";
import { isTerminalMenuData, terminalMenuEntries } from "../terminal/menu";

export const TerminalSurface = SurfaceKind("the-editor/terminal");

export class HostTerminal extends Context.Tag("the-editor/web/HostTerminal")<
  HostTerminal,
  {
    readonly renderer: PanelRenderer;
    readonly toggle: () => void;
  }
>() {}

export const terminalPackage = definePackage("the-editor/terminal", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;
    const renderers = yield* PanelRenderers;
    const contextMenus = yield* ContextMenuProviders;
    const host = yield* HostTerminal;

    yield* renderers.register(TerminalSurface, host.renderer);

    // The terminal's context menu is a registry contribution rendered by the
    // OS: one provider builds entries from each popup's per-instance data (the
    // terminal's actions + live copy-enabled state).
    yield* contextMenus.register({
      id: "terminal",
      provide: (request) =>
        request.menu === "terminal" && isTerminalMenuData(request.data)
          ? terminalMenuEntries(request.data)
          : [],
    });

    yield* commands.register("terminal.toggle", {
      title: "Toggle Terminal",
      scope: SurfaceScope.global,
      palette: { group: "View", aliases: ["term", "terminal"] },
      run: () => Effect.sync(() => host.toggle()),
    });
    yield* keymaps.registerAll([
      { keys: "ctrl+`", command: "terminal.toggle", scope: SurfaceScope.global },
      { keys: "meta+j", command: "terminal.toggle", scope: SurfaceScope.global },
    ]);
  }),
);
