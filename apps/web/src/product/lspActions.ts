/**
 * LSP feature commands: definition, formatting, rename, inlay hints. The
 * host owns the mechanics (jumping panes, the rename prompt); the package
 * owns command identity, keybindings, and palette presence.
 */

import { Context, Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import { SurfaceScope } from "@the-editor/workbench";
import { EditorSurface } from "./editorDefaults";

export class HostLspActions extends Context.Tag("the-editor/web/HostLspActions")<
  HostLspActions,
  {
    readonly goToDefinition: () => void;
    readonly formatDocument: () => void;
    readonly renameSymbol: () => void;
    readonly toggleInlayHints: () => void;
    readonly toggleLsp: () => void;
  }
>() {}

const editorScope = SurfaceScope.surface(EditorSurface);

export const lspActions = definePackage("the-editor/lsp-actions", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;
    const host = yield* HostLspActions;

    yield* commands.register("lsp.definition", {
      title: "Go to Definition",
      // Global so it also fires in the multibuffer (resolves to the source doc);
      // the handler no-ops when no text pane is focused.
      scope: SurfaceScope.global,
      palette: { group: "LSP", aliases: ["gd"] },
      run: () => Effect.sync(() => host.goToDefinition()),
    });
    yield* commands.register("lsp.format", {
      title: "Format Document",
      scope: editorScope,
      palette: { group: "LSP", aliases: ["fmt"] },
      run: () => Effect.sync(() => host.formatDocument()),
    });
    yield* commands.register("lsp.rename", {
      title: "Rename Symbol…",
      scope: editorScope,
      palette: { group: "LSP", aliases: ["rn"] },
      run: () => Effect.sync(() => host.renameSymbol()),
    });
    yield* commands.register("lsp.toggleInlayHints", {
      title: "Toggle Inlay Hints",
      scope: editorScope,
      palette: { group: "LSP" },
      run: () => Effect.sync(() => host.toggleInlayHints()),
    });
    // Global so it's reachable even when focus is on the file tree or an empty
    // workbench: stops every server, or restarts them + re-attaches open docs.
    yield* commands.register("lsp.toggle", {
      title: "Toggle Language Servers",
      scope: SurfaceScope.global,
      palette: { group: "LSP", aliases: ["lsp", "restart lsp", "stop lsp"] },
      run: () => Effect.sync(() => host.toggleLsp()),
    });

    yield* keymaps.registerAll([
      { keys: "f12", command: "lsp.definition", scope: editorScope },
      { keys: "f2", command: "lsp.rename", scope: editorScope },
      { keys: "alt+shift+f", command: "lsp.format", scope: editorScope },
    ]);
  }),
);
