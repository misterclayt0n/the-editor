/**
 * The file tree as a product package: a custom workbench panel (the first
 * non-editor surface) registered through PanelRenderers — proof that panel
 * kinds are ordinary contributions. The package owns command identity and
 * the renderer registration; the host seam supplies the renderer and the
 * toggle (which owns arrangement surgery).
 */

import { Context, Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import { SurfaceKind, SurfaceScope } from "@the-editor/workbench";
import { PanelRenderers, type PanelRenderer } from "../workbench/panelRenderers";

export const FileTreeSurface = SurfaceKind("the-editor/file-tree");

export class HostFileTree extends Context.Tag("the-editor/web/HostFileTree")<
  HostFileTree,
  {
    readonly renderer: PanelRenderer;
    readonly toggle: () => void;
    readonly moveSide: () => void;
  }
>() {}

export const fileTreePackage = definePackage("the-editor/file-tree", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;
    const renderers = yield* PanelRenderers;
    const host = yield* HostFileTree;

    yield* renderers.register(FileTreeSurface, host.renderer);

    yield* commands.register("files.toggleTree", {
      title: "Toggle File Tree",
      scope: SurfaceScope.global,
      palette: { group: "File", aliases: ["tree", "explorer"] },
      run: () => Effect.sync(() => host.toggle()),
    });
    yield* commands.register("files.moveTreeSide", {
      title: "Move File Tree to Other Side",
      scope: SurfaceScope.global,
      palette: { group: "View", aliases: ["sidebar", "left", "right", "dock"] },
      run: () => Effect.sync(() => host.moveSide()),
    });
    yield* keymaps.registerAll([
      { keys: "meta+b", command: "files.toggleTree", scope: SurfaceScope.global },
      { keys: "ctrl+b", command: "files.toggleTree", scope: SurfaceScope.global },
    ]);
  }),
);
