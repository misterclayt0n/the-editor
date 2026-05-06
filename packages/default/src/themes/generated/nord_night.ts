import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "nord-night",
  "title": "Nord Night",
  "palette": {
    "nord0": "#252933",
    "nord1": "#3B4252",
    "nord2": "#434C5E",
    "nord3": "#4C566A",
    "nord3_bright": "#616e88",
    "nord4": "#C0C5CF",
    "nord5": "#E5E9F0",
    "nord6": "#ECEFF4",
    "nord7": "#8FBCBB",
    "nord8": "#88C0D0",
    "nord9": "#81A1C1",
    "nord10": "#5E81AC",
    "nord11": "#BF616A",
    "nord12": "#D08770",
    "nord13": "#EBCB8B",
    "nord14": "#A3BE8C",
    "nord15": "#B48EAD"
  },
  "styles": {
    "constant": "nord13",
    "constant.builtin": "nord9",
    "constant.builtin.boolean": "nord13",
    "constant.builtin.character": "nord15",
    "constant.character.escape": "nord13",
    "constant.macro": "nord9",
    "constant.numeric": "nord13",
    "constructor": "nord8",
    "diagnostic": {
      "underline": {
        "color": "nord13",
        "style": "curl"
      }
    },
    "diagnostic.error": {
      "underline": {
        "color": "nord11",
        "style": "curl"
      }
    },
    "error": "nord11",
    "diagnostic.hint": {
      "underline": {
        "color": "nord10",
        "style": "curl"
      }
    },
    "hint": "nord10",
    "diagnostic.info": {
      "underline": {
        "color": "nord8",
        "style": "curl"
      }
    },
    "info": "nord8",
    "diagnostic.warning": {
      "underline": {
        "color": "nord13",
        "style": "curl"
      }
    },
    "warning": "nord13",
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
    "diff.delta": "nord13",
    "diff.minus": "nord11",
    "diff.plus": "nord14",
    "function": "nord8",
    "function.builtin": "nord7",
    "function.method": "nord8",
    "function.macro": "nord9",
    "function.special": "nord9",
    "git.delta.moved": "nord12",
    "keyword": "nord9",
    "keyword.control.conditional": "nord11",
    "keyword.control.exception": "nord11",
    "keyword.control.repeat": "nord11",
    "keyword.directive": "nord9",
    "keyword.function": "nord9",
    "keyword.operator": "nord9",
    "keyword.return": "nord9",
    "keyword.storage.modifier": "nord9",
    "keyword.storage.type": "nord9",
    "punctuation": "nord6",
    "punctuation.bracket": "nord6",
    "punctuation.delimiter": "nord6",
    "punctuation.special": "nord9",
    "string": "nord14",
    "string.escape": "nord13",
    "string.regex": "nord13",
    "string.special": "nord13",
    "type": "nord7",
    "type.builtin": "nord7",
    "variable": "nord4",
    "variable.builtin": "nord9",
    "variable.other.member": "nord4",
    "variable.parameter": "nord15",
    "attribute": "nord9",
    "label": "nord7",
    "namespace": "nord4",
    "operator": "nord9",
    "special": "nord9",
    "tag": "nord7",
    "comment": {
      "fg": "nord3_bright",
      "modifiers": [
        "italic"
      ]
    },
    "ui.background": {
      "bg": "nord0"
    },
    "ui.text": "nord4",
    "ui.window": "nord1",
    "ui.debug.active": "nord13",
    "ui.debug.breakpoint": "nord11",
    "ui.menu": {
      "bg": "nord1"
    },
    "ui.menu.scroll": {
      "fg": "nord4",
      "bg": "nord3"
    },
    "ui.menu.selected": {
      "fg": "nord8",
      "bg": "nord2"
    },
    "ui.popup": {
      "bg": "nord1"
    },
    "ui.popup.info": {
      "bg": "nord1"
    },
    "ui.help": {
      "bg": "nord1"
    },
    "ui.text.focus": {
      "fg": "nord8",
      "bg": "nord2"
    },
    "ui.gutter": "nord5",
    "ui.linenr": "nord3",
    "ui.linenr.selected": "nord5",
    "ui.cursor": {
      "fg": "nord4",
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursorcolumn.primary": {
      "bg": "nord1"
    },
    "ui.cursorline.primary": {
      "bg": "nord1"
    },
    "ui.cursor.match": {
      "bg": "nord3"
    },
    "ui.selection": {
      "bg": "nord3"
    },
    "ui.highlight": {
      "fg": "nord8",
      "bg": "nord2"
    },
    "ui.statusline": {
      "bg": "nord1"
    },
    "ui.statusline.inactive": {
      "fg": "nord8",
      "bg": "nord1"
    },
    "ui.statusline.insert": {
      "fg": "nord1",
      "bg": "nord6"
    },
    "ui.statusline.normal": {
      "fg": "nord1",
      "bg": "nord8"
    },
    "ui.statusline.select": {
      "fg": "nord1",
      "bg": "nord7"
    },
    "ui.statusline.separator": "nord3",
    "ui.virtual.indent-guide": "nord3",
    "ui.virtual.inlay-hint": {
      "fg": "nord3",
      "modifiers": [
        "italic"
      ]
    },
    "ui.virtual.jump-label": {
      "fg": "nord11",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual.ruler": {
      "bg": "nord1"
    },
    "ui.virtual.whitespace": "nord3",
    "ui.virtual.wrap": "nord3",
    "ui.bufferline": {
      "fg": "nord5",
      "bg": "nord1"
    },
    "ui.bufferline.active": {
      "fg": "nord6",
      "bg": "nord2",
      "underline": {
        "color": "nord8",
        "style": "line"
      },
      "modifiers": [
        "italic"
      ]
    },
    "markup.heading": "nord8",
    "markup.list": "nord9",
    "markup.bold": {
      "modifiers": [
        "bold"
      ]
    },
    "markup.italic": {
      "modifiers": [
        "italic"
      ]
    },
    "markup.strikethrough": {
      "modifiers": [
        "crossed_out"
      ]
    },
    "markup.link.text": "nord8",
    "markup.raw": "nord7",
    "keyword.control": "nord11",
    "keyword.control.return": "nord11"
  }
});
