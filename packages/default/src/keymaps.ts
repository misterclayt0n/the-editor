import { dispatch, keymap, type EditorModule } from "@the-editor/editor";

export function defaultKeymaps(): EditorModule {
  return (editor) => {
    editor.keymaps.register({
      id: "default.helix.normal",
      scope: "normal",
      tree: keymap("Normal mode", {
        "h | left": "move.charLeft",
        "j | down": "move.visualLineDown",
        "k | up": "move.visualLineUp",
        "l | right": "move.charRight",

        home: "goto.lineStart",
        end: "goto.lineEnd",

        i: "mode.insert",
        a: ["move.charRight", "mode.insert"],
        I: ["goto.lineStart", "mode.insert"],
        A: ["goto.lineEnd", "mode.insert"],

        g: keymap("Goto", {
          g: "goto.documentStart",
          e: "goto.documentEnd",
          h: "goto.lineStart",
          l: "goto.lineEnd",
        }),
      }),
    });

    editor.keymaps.register({
      id: "default.helix.insert",
      scope: "insert",
      tree: keymap("Insert mode", {
        esc: "mode.normal",
        "C-h | backspace | S-backspace": "edit.deleteBackward",
        "C-d | del": "edit.deleteForward",
        "C-j | ret": "edit.insertNewline",
        tab: dispatch("edit.insertText", { text: "\t" }),
        up: "move.visualLineUp",
        down: "move.visualLineDown",
        left: "move.charLeft",
        right: "move.charRight",
        home: "goto.lineStart",
        end: "goto.lineEnd",
      }),
    });
  };
}
