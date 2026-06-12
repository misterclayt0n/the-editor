/**
 * Window-management commands as a product package: splits and tab cycling
 * are ordinary commands with ordinary keybindings, so they are palette-ready
 * and rebindable. The host owns the gestures' mechanics (pane lifecycle,
 * arrangement surgery) behind HostViewActions.
 *
 * Keybinding note: chords match on `event.key`, so shifted punctuation
 * ("meta+shift+\\" arriving as "|") never matches — bindings below stick to
 * keys whose names survive modifiers.
 */

import { Context, Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import { SurfaceScope } from "@the-editor/workbench";
import { EditorSurface } from "./editorDefaults";

export class HostViewActions extends Context.Tag("the-editor/web/HostViewActions")<
  HostViewActions,
  {
    readonly splitRight: () => void;
    readonly splitDown: () => void;
    readonly closeTab: () => void;
    readonly nextTab: () => void;
    readonly previousTab: () => void;
  }
>() {}

const editorScope = SurfaceScope.surface(EditorSurface);

export const viewPackage = definePackage("the-editor/view", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;
    const host = yield* HostViewActions;

    yield* commands.register("view.splitRight", {
      title: "Split Right",
      scope: editorScope,
      palette: { group: "View", aliases: ["vs", "vsplit"] },
      run: () => Effect.sync(() => host.splitRight()),
    });
    yield* commands.register("view.splitDown", {
      title: "Split Down",
      scope: editorScope,
      palette: { group: "View", aliases: ["sp", "hsplit"] },
      run: () => Effect.sync(() => host.splitDown()),
    });
    yield* commands.register("view.closeTab", {
      title: "Close Tab",
      scope: editorScope,
      palette: { group: "View", aliases: ["q"] },
      run: () => Effect.sync(() => host.closeTab()),
    });
    yield* commands.register("view.nextTab", {
      title: "Next Tab",
      scope: editorScope,
      palette: { group: "View", aliases: ["bn"] },
      run: () => Effect.sync(() => host.nextTab()),
    });
    yield* commands.register("view.previousTab", {
      title: "Previous Tab",
      scope: editorScope,
      palette: { group: "View", aliases: ["bp"] },
      run: () => Effect.sync(() => host.previousTab()),
    });

    yield* keymaps.registerAll([
      { keys: "meta+\\", command: "view.splitRight", scope: editorScope },
      { keys: "ctrl+\\", command: "view.splitRight", scope: editorScope },
      { keys: "meta+w", command: "view.closeTab", scope: editorScope },
      { keys: "ctrl+w", command: "view.closeTab", scope: editorScope },
      { keys: "meta+alt+arrowright", command: "view.nextTab", scope: editorScope },
      { keys: "meta+alt+arrowleft", command: "view.previousTab", scope: editorScope },
      { keys: "ctrl+pagedown", command: "view.nextTab", scope: editorScope },
      { keys: "ctrl+pageup", command: "view.previousTab", scope: editorScope },
    ]);
  }),
);
