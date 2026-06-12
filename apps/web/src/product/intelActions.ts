/**
 * Generic intel commands (provider-agnostic): explicit completion trigger.
 */

import { Context, Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import { SurfaceScope } from "@the-editor/workbench";
import { EditorSurface } from "./editorDefaults";

export class HostIntelActions extends Context.Tag("the-editor/web/HostIntelActions")<
  HostIntelActions,
  {
    readonly triggerCompletion: () => void;
    readonly triggerSignatureHelp: () => void;
  }
>() {}

const editorScope = SurfaceScope.surface(EditorSurface);

export const intelActions = definePackage("the-editor/intel-actions", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;
    const host = yield* HostIntelActions;

    yield* commands.register("editor.triggerCompletion", {
      title: "Trigger Completion",
      scope: editorScope,
      palette: { group: "Editor" },
      run: () => Effect.sync(() => host.triggerCompletion()),
    });
    yield* commands.register("editor.triggerParameterHints", {
      title: "Trigger Parameter Hints",
      scope: editorScope,
      palette: { group: "Editor" },
      run: () => Effect.sync(() => host.triggerSignatureHelp()),
    });
    yield* keymaps.registerAll([
      { keys: "ctrl+space", command: "editor.triggerCompletion", scope: editorScope },
      { keys: "ctrl+shift+space", command: "editor.triggerParameterHints", scope: editorScope },
    ]);
  }),
);
