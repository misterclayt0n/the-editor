/**
 * File commands as a product package: save and open, bound to the platform
 * through HostFileActions (the host owns dialogs and view swapping; the
 * package owns command identity and keybindings, so they are palette-ready
 * and rebindable like everything else).
 */

import { Context, Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import { SurfaceScope } from "@the-editor/workbench";
import { EditorSurface } from "./editorDefaults";

/**
 * Host-side file actions. Save/open complete asynchronously (IO crosses the
 * platform port); the host surfaces results through its own state.
 */
export class HostFileActions extends Context.Tag("the-editor/web/HostFileActions")<
  HostFileActions,
  { readonly save: () => void; readonly open: () => void }
>() {}

const editorScope = SurfaceScope.surface(EditorSurface);

export const filesPackage = definePackage("the-editor/files", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;
    const host = yield* HostFileActions;

    yield* commands.register("file.save", {
      title: "Save File",
      scope: editorScope,
      run: () => Effect.sync(() => host.save()),
    });
    yield* commands.register("file.open", {
      title: "Open File…",
      scope: editorScope,
      run: () => Effect.sync(() => host.open()),
    });

    yield* keymaps.registerAll([
      { keys: "meta+s", command: "file.save", scope: editorScope },
      { keys: "ctrl+s", command: "file.save", scope: editorScope },
      { keys: "meta+o", command: "file.open", scope: editorScope },
      { keys: "ctrl+o", command: "file.open", scope: editorScope },
    ]);
  }),
);
