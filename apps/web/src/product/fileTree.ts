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
import { Keymaps, ctrlCmd } from "@the-editor/keymaps";
import {
  PanelRenderers,
  SurfaceKind,
  SurfaceScope,
  type PanelRenderer,
} from "@the-editor/workbench";

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
      title: "Flip Sidebar",
      scope: SurfaceScope.global,
      palette: {
        group: "View",
        aliases: ["sidebar", "move sidebar", "swap side", "other side", "left", "right", "dock"],
      },
      run: () => Effect.sync(() => host.moveSide()),
    });
    yield* keymaps.registerAll([
      { keys: ctrlCmd("b"), command: "files.toggleTree", scope: SurfaceScope.global },
    ]);
  }),
);
