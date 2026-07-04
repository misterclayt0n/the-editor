/**
 * LSP feature commands: definition, formatting, rename, inlay hints. The
 * host owns the mechanics (jumping panes, the rename prompt); the package
 * owns command identity and palette presence. Keybinding schemes bind these
 * commands separately.
 */

import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { SurfaceScope } from "@the-editor/workbench";
import { LspOperations } from "@the-editor/editor";
import { EditorSurface } from "./editorDefaults";

const editorScope = SurfaceScope.surface(EditorSurface);

export const lspActions = definePackage("the-editor/lsp-actions", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const lsp = yield* LspOperations;

    yield* commands.register("lsp.definition", {
      title: "Go to Definition",
      // Global so it also fires in the multibuffer (resolves to the source doc);
      // the handler no-ops when no text pane is focused.
      scope: SurfaceScope.global,
      palette: { group: "LSP", aliases: ["gd"] },
      run: () => Effect.sync(() => lsp.goToDefinition()),
    });
    yield* commands.register("lsp.format", {
      title: "Format Document",
      scope: editorScope,
      palette: { group: "LSP", aliases: ["fmt"] },
      run: () => Effect.sync(() => lsp.formatDocument()),
    });
    yield* commands.register("lsp.rename", {
      title: "Rename Symbol…",
      scope: editorScope,
      palette: { group: "LSP", aliases: ["rn"] },
      run: () => Effect.sync(() => lsp.renameSymbol()),
    });
    yield* commands.register("lsp.toggleInlayHints", {
      title: "Toggle Inlay Hints",
      scope: editorScope,
      palette: { group: "LSP" },
      run: () => Effect.sync(() => lsp.toggleInlayHints()),
    });
    // Global so it's reachable even when focus is on the file tree or an empty
    // workbench: stops every server, or restarts them + re-attaches open docs.
    yield* commands.register("lsp.toggle", {
      title: "Toggle Language Servers",
      scope: SurfaceScope.global,
      palette: { group: "LSP", aliases: ["lsp", "restart lsp", "stop lsp"] },
      run: () => Effect.sync(() => lsp.toggleLsp()),
    });

  }),
);
