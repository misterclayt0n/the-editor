/**
 * Window-management commands as a product package: splits and tab cycling
 * are ordinary commands with ordinary keybindings, so they are palette-ready
 * and rebindable. The host owns the gestures' mechanics (pane lifecycle,
 * arrangement surgery) behind ViewOperations.
 *
 * Keybinding note: chords match on `event.key`, so shifted punctuation
 * ("meta+shift+\\" arriving as "|") never matches — bindings below stick to
 * keys whose names survive modifiers.
 */

import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import { SurfaceScope } from "@the-editor/workbench";
import { ViewOperations } from "@the-editor/editor";
import { EditorSurface } from "./editorDefaults";

const editorScope = SurfaceScope.surface(EditorSurface);

export const viewPackage = definePackage("the-editor/view", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;
    const view = yield* ViewOperations;

    yield* commands.register("view.splitRight", {
      title: "Split Right",
      scope: editorScope,
      palette: { group: "View", aliases: ["vs", "vsplit"] },
      run: () => Effect.sync(() => view.splitRight()),
    });
    yield* commands.register("view.splitDown", {
      title: "Split Down",
      scope: editorScope,
      palette: { group: "View", aliases: ["sp", "hsplit"] },
      run: () => Effect.sync(() => view.splitDown()),
    });
    // Global: closes whichever workbench panel tab is focused — editors,
    // multibuffers, file trees, terminals. This must not be editor-scoped:
    // terminal and custom panels often swallow their own keydowns and only ask
    // the host for global shortcuts.
    yield* commands.register("view.closeTab", {
      title: "Close Tab",
      scope: SurfaceScope.global,
      palette: { group: "View", aliases: ["q"] },
      run: () => Effect.sync(() => view.closeTab()),
    });
    yield* commands.register("view.nextTab", {
      title: "Next Tab",
      scope: editorScope,
      palette: { group: "View", aliases: ["bn"] },
      run: () => Effect.sync(() => view.nextTab()),
    });
    yield* commands.register("view.previousTab", {
      title: "Previous Tab",
      scope: editorScope,
      palette: { group: "View", aliases: ["bp"] },
      run: () => Effect.sync(() => view.previousTab()),
    });
    // Global: reachable from the file tree (its single tab is the obvious use),
    // an empty workbench, etc. Hidden tabs can still be cycled by keyboard.
    yield* commands.register("view.toggleTabs", {
      title: "Toggle Panel Tabs",
      scope: SurfaceScope.global,
      palette: { group: "View", aliases: ["tabs"] },
      run: () => Effect.sync(() => view.toggleTabs()),
    });

    yield* keymaps.registerAll([
      { keys: "meta+\\", command: "view.splitRight", scope: editorScope },
      { keys: "ctrl+\\", command: "view.splitRight", scope: editorScope },
      { keys: "meta+w", command: "view.closeTab", scope: SurfaceScope.global },
      { keys: "ctrl+w", command: "view.closeTab", scope: editorScope },
      { keys: "meta+alt+arrowright", command: "view.nextTab", scope: editorScope },
      { keys: "meta+alt+arrowleft", command: "view.previousTab", scope: editorScope },
      { keys: "ctrl+pagedown", command: "view.nextTab", scope: editorScope },
      { keys: "ctrl+pageup", command: "view.previousTab", scope: editorScope },
    ]);
  }),
);
