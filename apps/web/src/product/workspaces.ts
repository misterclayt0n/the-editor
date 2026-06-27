/**
 * Workspaces as a product package: a workspace IS an open folder, so opening a
 * folder, switching, and closing are the three primitives — one coherent noun,
 * three verbs. "Open Folder…" is the sole creation path (it folds in what used
 * to be a separate File > Open Folder command); the host decides whether to
 * reuse an untitled scratch workspace or spawn a new one. The switcher is a
 * command-palette mode that doubles as the manager (open + close inline).
 *
 * The host owns the actual session lifecycle through HostWorkspaceActions and
 * just receives these intents, so the package never sees the session list and
 * cannot own it.
 */

import { Context, Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import { SurfaceScope } from "@the-editor/workbench";

export class HostWorkspaceActions extends Context.Tag("the-editor/web/HostWorkspaceActions")<
  HostWorkspaceActions,
  {
    readonly openFolder: () => void;
    readonly openSwitcher: () => void;
    readonly closeWorkspace: () => void;
  }
>() {}

export const workspacesPackage = definePackage("the-editor/workspaces", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;
    const host = yield* HostWorkspaceActions;

    yield* commands.register("workspace.open", {
      title: "Open Folder…",
      scope: SurfaceScope.global,
      palette: {
        group: "Workspace",
        aliases: ["open folder", "open workspace", "new workspace", "folder", "cd"],
      },
      run: () => Effect.sync(() => host.openFolder()),
    });
    yield* commands.register("workspace.switch", {
      title: "Switch Workspace",
      scope: SurfaceScope.global,
      palette: { group: "Workspace", aliases: ["session", "project", "switch", "workspaces"] },
      run: () => Effect.sync(() => host.openSwitcher()),
    });
    yield* commands.register("workspace.close", {
      title: "Close Workspace",
      scope: SurfaceScope.global,
      palette: { group: "Workspace", aliases: ["close session", "close project", "close folder"] },
      run: () => Effect.sync(() => host.closeWorkspace()),
    });

    yield* keymaps.registerAll([
      { keys: "meta+k meta+o", command: "workspace.open", scope: SurfaceScope.global },
      { keys: "ctrl+k ctrl+o", command: "workspace.open", scope: SurfaceScope.global },
      { keys: "meta+k meta+w", command: "workspace.switch", scope: SurfaceScope.global },
      { keys: "ctrl+k ctrl+w", command: "workspace.switch", scope: SurfaceScope.global },
    ]);
  }),
);
