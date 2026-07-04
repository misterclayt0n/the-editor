/**
 * Workspaces as a product package: a workspace IS an open folder, so opening a
 * folder, switching, and closing are the three primitives — one coherent noun,
 * three verbs. "Open Folder…" is the sole creation path (it folds in what used
 * to be a separate File > Open Folder command); the host decides whether to
 * reuse an untitled scratch workspace or spawn a new one. The switcher is a
 * command-palette mode that doubles as the manager (open + close inline).
 *
 * The host owns the actual session lifecycle through WorkspaceOperations and
 * just receives these intents, so the package never sees the session list and
 * cannot own it.
 */

import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { SurfaceScope } from "@the-editor/workbench";
import { WorkspaceOperations } from "@the-editor/editor";

export const workspacesPackage = definePackage("the-editor/workspaces", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const workspace = yield* WorkspaceOperations;

    yield* commands.register("workspace.open", {
      title: "Open Folder…",
      scope: SurfaceScope.global,
      palette: {
        group: "Workspace",
        aliases: ["open folder", "open workspace", "new workspace", "folder", "cd"],
      },
      run: () => Effect.sync(() => workspace.openFolder()),
    });
    yield* commands.register("workspace.switch", {
      title: "Switch Workspace",
      scope: SurfaceScope.global,
      palette: { group: "Workspace", aliases: ["session", "project", "switch", "workspaces"] },
      run: () => Effect.sync(() => workspace.openSwitcher()),
    });
    yield* commands.register("workspace.close", {
      title: "Close Workspace",
      scope: SurfaceScope.global,
      palette: { group: "Workspace", aliases: ["close session", "close project", "close folder"] },
      run: () => Effect.sync(() => workspace.closeWorkspace()),
    });

  }),
);
