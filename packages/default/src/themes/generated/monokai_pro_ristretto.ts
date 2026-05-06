import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "monokai_pro_ristretto",
  "title": "Monokai Pro Ristretto",
  "palette": {
    "red": "#fd6883",
    "orange": "#f38d70",
    "yellow": "#f9cc6c",
    "green": "#adda78",
    "blue": "#85dacc",
    "purple": "#a8a9eb",
    "base0": "#191515",
    "base1": "#211c1c",
    "base2": "#2c2525",
    "base3": "#403838",
    "base4": "#5b5353",
    "base5": "#72696a",
    "base6": "#8c8384",
    "base7": "#c3b7b8",
    "base8": "#fff1f3",
    "base8x0c": "#352e2e"
  },
  "styles": {
    "ui.linenr.selected": {
      "bg": "base3"
    },
    "ui.text.focus": {
      "fg": "yellow",
      "modifiers": [
        "bold"
      ]
    },
    "ui.menu": {
      "fg": "base8",
      "bg": "base3"
    },
    "ui.menu.selected": {
      "fg": "base2",
      "bg": "yellow"
    },
    "ui.virtual.whitespace": "base5",
    "ui.virtual.ruler": {
      "bg": "base1"
    },
    "ui.virtual.jump-label": {
      "fg": "red",
      "modifiers": [
        "bold"
      ]
    },
    "info": "base8",
    "hint": "base8",
    "ui.background": {
      "bg": "base2"
    },
    "ui.statusline.inactive": {
      "fg": "base8",
      "bg": "base8x0c"
    },
    "ui.statusline": {
      "fg": "base8",
      "bg": "base4"
    },
    "ui.popup": {
      "bg": "base3"
    },
    "ui.window": {
      "bg": "base3"
    },
    "ui.help": {
      "fg": "base8",
      "bg": "base3"
    },
    "ui.selection": {
      "bg": "base4"
    },
    "ui.cursor.match": {
      "bg": "base4"
    },
    "ui.cursorline": {
      "bg": "base1"
    },
    "ui.bufferline": {
      "fg": "base6",
      "bg": "base8x0c"
    },
    "ui.bufferline.active": {
      "fg": "base8",
      "bg": "base4"
    },
    "ui.virtual.inlay-hint": {
      "fg": "base6"
    },
    "comment": {
      "fg": "base5",
      "modifiers": [
        "italic"
      ]
    },
    "ui.linenr": {
      "fg": "base5"
    },
    "ui.cursor.primary": {
      "fg": "base7",
      "modifiers": [
        "reversed"
      ]
    },
    "attribute": "blue",
    "variable": "base8",
    "constant": "orange",
    "variable.builtin": "red",
    "constant.builtin": "red",
    "namespace": "base8",
    "ui.text": {
      "fg": "base8"
    },
    "punctuation": "base6",
    "type": "green",
    "type.builtin": {
      "fg": "red"
    },
    "label": "base8",
    "constructor": "blue",
    "function": "green",
    "function.macro": {
      "fg": "blue"
    },
    "function.builtin": {
      "fg": "cyan"
    },
    "operator": "red",
    "variable.other.member": "base8",
    "keyword": {
      "fg": "red"
    },
    "keyword.directive": "blue",
    "variable.parameter": "#f59762",
    "error": "red",
    "special": "#f59762",
    "module": "#f59762",
    "warning": "orange",
    "constant.character.escape": {
      "fg": "base8"
    },
    "string": "yellow",
    "constant.numeric": "purple",
    "diff.plus": "green",
    "diff.delta": "orange",
    "diff.minus": "red",
    "diagnostic.warning": {
      "underline": {
        "color": "orange",
        "style": "curl"
      }
    },
    "diagnostic.error": {
      "underline": {
        "color": "red",
        "style": "curl"
      }
    },
    "diagnostic.info": {
      "underline": {
        "color": "base8",
        "style": "curl"
      }
    },
    "diagnostic.hint": {
      "underline": {
        "color": "base8",
        "style": "curl"
      }
    },
    "diagnostic.unnecessary": {
      "modifiers": [
        "dim"
      ]
    },
    "diagnostic.deprecated": {
      "modifiers": [
        "crossed_out"
      ]
    },
    "markup.heading": "green",
    "markup.bold": {
      "fg": "orange",
      "modifiers": [
        "bold"
      ]
    },
    "markup.italic": {
      "fg": "orange",
      "modifiers": [
        "italic"
      ]
    },
    "markup.strikethrough": {
      "modifiers": [
        "crossed_out"
      ]
    },
    "markup.link.url": {
      "fg": "orange",
      "modifiers": [
        "underlined"
      ]
    },
    "markup.link.text": "yellow",
    "markup.quote": "green"
  }
});
