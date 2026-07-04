/**
 * The VS Code-style default keybinding scheme.
 *
 * Product packages own commands and surfaces; this package owns the curated
 * default keyboard layout. Disabling/replacing it swaps the keymap without
 * retracting the underlying commands.
 */

import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { inputWhen } from "@the-editor/input";
import { Keymaps, ctrlCmd } from "@the-editor/keymaps";
import { SurfaceKind, SurfaceScope } from "@the-editor/workbench";

const editorScope = SurfaceScope.surface(SurfaceKind("the-editor/editor"));
const acceptsText = inputWhen({ acceptsText: true });

export const vscodeKeybindings = definePackage("the-editor/vscode-keybindings", () =>
  Effect.gen(function* () {
    const keymaps = yield* Keymaps;

    yield* keymaps.registerAll([
      // Core text editing.
      { keys: "enter", command: "editor.newline", scope: editorScope, when: acceptsText },
      { keys: "backspace", command: "editor.deleteBackward", scope: editorScope, when: acceptsText },
      { keys: "shift+backspace", command: "editor.deleteBackward", scope: editorScope, when: acceptsText },
      { keys: "delete", command: "editor.deleteForward", scope: editorScope, when: acceptsText },
      {
        keys: "tab",
        command: "editor.insertText",
        args: { text: "\t" },
        scope: editorScope,
        when: acceptsText,
      },

      // Core cursor movement.
      { keys: "arrowleft", command: "cursor.left", scope: editorScope },
      { keys: "arrowright", command: "cursor.right", scope: editorScope },
      { keys: "arrowup", command: "cursor.up", scope: editorScope },
      { keys: "arrowdown", command: "cursor.down", scope: editorScope },
      { keys: "shift+arrowleft", command: "cursor.left", args: { extend: true }, scope: editorScope },
      { keys: "shift+arrowright", command: "cursor.right", args: { extend: true }, scope: editorScope },
      { keys: "shift+arrowup", command: "cursor.up", args: { extend: true }, scope: editorScope },
      { keys: "shift+arrowdown", command: "cursor.down", args: { extend: true }, scope: editorScope },
      { keys: "home", command: "cursor.lineStart", scope: editorScope },
      { keys: "end", command: "cursor.lineEnd", scope: editorScope },
      { keys: "shift+home", command: "cursor.lineStart", args: { extend: true }, scope: editorScope },
      { keys: "shift+end", command: "cursor.lineEnd", args: { extend: true }, scope: editorScope },

      // Workbench/file commands.
      { keys: ctrlCmd("s"), command: "file.save", scope: editorScope },
      { keys: ctrlCmd("o"), command: "file.open", scope: editorScope },
      { keys: ctrlCmd("p"), command: "file.find", scope: SurfaceScope.global },
      { keys: ctrlCmd("shift+f"), command: "file.findInProject", scope: SurfaceScope.global },
      { keys: ctrlCmd("shift+p"), command: "palette.open", scope: SurfaceScope.global },
      { keys: "f1", command: "palette.open", scope: SurfaceScope.global },
      { keys: ctrlCmd("b"), command: "files.toggleTree", scope: SurfaceScope.global },
      { keys: "ctrl+`", command: "terminal.toggle", scope: SurfaceScope.global },
      // VS Code's Ctrl/Cmd+J toggles the bottom panel; until we have a generic
      // panel command, the terminal bottom panel is the closest product command.
      { keys: ctrlCmd("j"), command: "terminal.toggle", scope: SurfaceScope.global },

      // Editor/workbench layout.
      { keys: ctrlCmd("\\"), command: "view.splitRight", scope: editorScope },
      { keys: ctrlCmd("w"), command: "view.closeTab", scope: SurfaceScope.global },
      { keys: "ctrl+pagedown", command: "view.nextTab", scope: editorScope },
      { keys: "ctrl+pageup", command: "view.previousTab", scope: editorScope },
      { keys: { primary: "alt+arrowright", mac: "meta+alt+arrowright" }, command: "view.nextTab", scope: editorScope },
      { keys: { primary: "alt+arrowleft", mac: "meta+alt+arrowleft" }, command: "view.previousTab", scope: editorScope },
      { keys: "alt+z", command: "view.toggleSoftWrap", scope: SurfaceScope.global },

      // Language/intelligence commands that already exist in the product.
      { keys: "f12", command: "lsp.definition", scope: editorScope },
      { keys: "f2", command: "lsp.rename", scope: editorScope },
      { keys: "alt+shift+f", command: "lsp.format", scope: editorScope },
      { keys: "ctrl+space", command: "editor.triggerCompletion", scope: editorScope },
      { keys: ctrlCmd("shift+space"), command: "editor.triggerParameterHints", scope: editorScope },

      // Existing VS Code-like command chords.
      {
        keys: { primary: "ctrl+k ctrl+t", mac: "meta+k meta+t" },
        command: "theme.select",
        scope: SurfaceScope.global,
      },
      {
        keys: { primary: "ctrl+k ctrl+l", mac: "meta+k meta+l" },
        command: "theme.toggleLightDark",
        scope: SurfaceScope.global,
      },
      {
        keys: { primary: "ctrl+k ctrl+o", mac: "meta+k meta+o" },
        command: "workspace.open",
        scope: SurfaceScope.global,
      },
      {
        keys: { primary: "ctrl+k ctrl+w", mac: "meta+k meta+w" },
        command: "workspace.switch",
        scope: SurfaceScope.global,
      },
      { keys: ctrlCmd("shift+g"), command: "diffs.openDiffView", scope: SurfaceScope.global },
    ]);
  }),
);
