/**
 * Workspaces as a product package: switching, creating, and closing the open
 * workspace sessions are ordinary, rebindable commands — no bespoke chrome.
 * The switcher is a command-palette mode (like theme select); the host owns the
 * actual session lifecycle through HostWorkspaceActions and just receives these
 * intents. The package never sees the session list, so it cannot own it.
 */

import { Context, Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import { SurfaceScope } from "@the-editor/workbench";

export class HostWorkspaceActions extends Context.Tag("the-editor/web/HostWorkspaceActions")<
  HostWorkspaceActions,
  {
    readonly openSwitcher: () => void;
    readonly newWorkspace: () => void;
    readonly closeWorkspace: () => void;
  }
>() {}

export const workspacesPackage = definePackage("the-editor/workspaces", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;
    const host = yield* HostWorkspaceActions;

    yield* commands.register("workspace.switch", {
      title: "Switch Workspace",
      scope: SurfaceScope.global,
      palette: { group: "Workspace", aliases: ["session", "project", "switch", "open workspace"] },
      run: () => Effect.sync(() => host.openSwitcher()),
    });
    yield* commands.register("workspace.new", {
      title: "New Workspace",
      scope: SurfaceScope.global,
      palette: { group: "Workspace", aliases: ["open folder", "add workspace", "new session"] },
      run: () => Effect.sync(() => host.newWorkspace()),
    });
    yield* commands.register("workspace.close", {
      title: "Close Workspace",
      scope: SurfaceScope.global,
      palette: { group: "Workspace", aliases: ["close session", "close project"] },
      run: () => Effect.sync(() => host.closeWorkspace()),
    });

    yield* keymaps.registerAll([
      { keys: "meta+k meta+w", command: "workspace.switch", scope: SurfaceScope.global },
      { keys: "ctrl+k ctrl+w", command: "workspace.switch", scope: SurfaceScope.global },
    ]);
  }),
);
