/**
 * File commands as a product package: save and open, bound to the platform
 * through FileOperations (the host owns dialogs and view swapping; the package
 * owns command identity and keybindings, so they are palette-ready and
 * rebindable like everything else).
 */

import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import { SurfaceScope } from "@the-editor/workbench";
import { FileOperations } from "@the-editor/editor";
import { EditorSurface } from "./editorDefaults";

const editorScope = SurfaceScope.surface(EditorSurface);

export const filesPackage = definePackage("the-editor/files", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;
    const files = yield* FileOperations;

    yield* commands.register("file.save", {
      title: "Save File",
      scope: editorScope,
      palette: { group: "File", aliases: ["w", "write"] },
      run: () => Effect.sync(() => files.save()),
    });
    yield* commands.register("file.open", {
      title: "Open File…",
      scope: editorScope,
      palette: { group: "File", aliases: ["e", "edit"] },
      run: () => Effect.sync(() => files.open()),
    });

    yield* commands.register("file.find", {
      title: "Find File…",
      scope: SurfaceScope.global,
      palette: { group: "File", aliases: ["quick open", "go to file", "fff"] },
      run: () => Effect.sync(() => files.find()),
    });

    yield* commands.register("file.findInProject", {
      title: "Find in Files…",
      scope: SurfaceScope.global,
      palette: { group: "File", aliases: ["search", "grep", "find in files", "rg"] },
      run: () => Effect.sync(() => files.findInProject()),
    });

    yield* keymaps.registerAll([
      { keys: "meta+s", command: "file.save", scope: editorScope },
      { keys: "ctrl+s", command: "file.save", scope: editorScope },
      { keys: "meta+o", command: "file.open", scope: editorScope },
      { keys: "ctrl+o", command: "file.open", scope: editorScope },
      { keys: "meta+p", command: "file.find", scope: SurfaceScope.global },
      { keys: "ctrl+p", command: "file.find", scope: SurfaceScope.global },
      { keys: "meta+shift+f", command: "file.findInProject", scope: SurfaceScope.global },
      { keys: "ctrl+shift+f", command: "file.findInProject", scope: SurfaceScope.global },
    ]);
  }),
);
