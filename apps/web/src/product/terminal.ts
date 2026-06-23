/**
 * The terminal as a product package: a workbench panel registered through
 * PanelRenderers, like the file tree. The package owns command identity and
 * the renderer registration; the host seam supplies the renderer (xterm.js +
 * PTY) and the toggle (arrangement surgery — docked at the bottom).
 */

import { Context, Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import { SurfaceKind, SurfaceScope } from "@the-editor/workbench";
import { PanelRenderers, type PanelRenderer } from "../workbench/panelRenderers";

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
    const host = yield* HostTerminal;

    yield* renderers.register(TerminalSurface, host.renderer);

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
