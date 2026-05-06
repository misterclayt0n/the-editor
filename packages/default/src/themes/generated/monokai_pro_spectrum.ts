import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "monokai_pro_spectrum",
  "title": "Monokai Pro Spectrum",
  "palette": {
    "red": "#fc618d",
    "orange": "#fd9353",
    "yellow": "#fce566",
    "green": "#7bd88f",
    "blue": "#5ad4e6",
    "purple": "#948ae3",
    "base0": "#131313",
    "base1": "#191919",
    "base2": "#222222",
    "base3": "#363537",
    "base4": "#525053",
    "base5": "#69676c",
    "base6": "#8b888f",
    "base7": "#bab6c0",
    "base8": "#f7f1ff",
    "base8x0c": "#2b2b2b"
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
    "ui.virtual.whitespace": "base4",
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
    "ui.statusline": {
      "fg": "base8",
      "bg": "base4"
    },
    "ui.statusline.inactive": {
      "fg": "base8",
      "bg": "base8x0c"
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
      "fg": "blue"
    },
    "operator": "red",
    "variable.other.member": "base8",
    "keyword": {
      "fg": "red"
    },
    "keyword.directive": "blue",
    "variable.parameter": "#f59762",
    "error": {
      "fg": "red",
      "modifiers": [
        "bold"
      ]
    },
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
