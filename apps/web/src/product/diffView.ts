/**
 * The diff view as a product package: a global workbench panel that shows the
 * working-tree diff against HEAD. Follows the same contribution shape as the
 * file tree — surface kind, host seam, command, and keybinding.
 */

import { Context, Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import { SurfaceKind, SurfaceScope } from "@the-editor/workbench";
import { PanelRenderers, type PanelRenderer } from "../workbench/panelRenderers";

export const DiffViewSurface = SurfaceKind("the-editor/diff-view");

export class HostDiffView extends Context.Tag("the-editor/web/HostDiffView")<
  HostDiffView,
  {
    readonly renderer: PanelRenderer;
    readonly toggle: () => void;
  }
>() {}

export const diffViewPackage = definePackage("the-editor/diff-view", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;
    const renderers = yield* PanelRenderers;
    const host = yield* HostDiffView;

    yield* renderers.register(DiffViewSurface, host.renderer);

    yield* commands.register("diffs.toggleDiffView", {
      title: "Toggle Diff View",
      scope: SurfaceScope.global,
      palette: { group: "View", aliases: ["diff", "changes", "source control"] },
      run: () => Effect.sync(() => host.toggle()),
    });
    yield* keymaps.registerAll([
      { keys: "meta+shift+g", command: "diffs.toggleDiffView", scope: SurfaceScope.global },
      { keys: "ctrl+shift+g", command: "diffs.toggleDiffView", scope: SurfaceScope.global },
    ]);
  }),
);
