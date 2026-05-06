import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "kaolin-dark",
  "title": "Kaolin Dark",
  "palette": {
    "bg0": "#18181b",
    "bg1": "#222225",
    "bg2": "#303035",
    "fg0": "#e6e6e8",
    "fg1": "#adadb9",
    "gray0": "#545c5e",
    "gray1": "#919a9c",
    "green0": "#6fb593",
    "selection0": "#68f3ca",
    "selection1": "#2e403b",
    "type": "#cd9575",
    "hint": "#4d9391",
    "info": "#80bcb6",
    "warn": "#dbac66",
    "error": "#cd5c60",
    "constant": "#ab98b5",
    "ultramarine4_dark": "#968cc7",
    "violet7": "#766884"
  },
  "styles": {
    "attribute": "constant",
    "keyword": {
      "fg": "hint"
    },
    "keyword.directive": "info",
    "namespace": "constant",
    "punctuation": "fg0",
    "punctuation.delimiter": "fg0",
    "operator": "fg0",
    "special": "constant",
    "variable.other.member": {
      "fg": "violet7",
      "modifiers": [
        "italic"
      ]
    },
    "variable": "ultramarine4_dark",
    "variable.builtin": "hint",
    "variable.parameter": "ultramarine4_dark",
    "type": "type",
    "type.builtin": "info",
    "constructor": {
      "fg": "type"
    },
    "function": {
      "fg": "info"
    },
    "function.macro": "constant",
    "function.builtin": "selection0",
    "tag": "#7aaaa5",
    "comment": {
      "fg": "gray0",
      "modifiers": [
        "italic"
      ]
    },
    "constant": {
      "fg": "constant"
    },
    "constant.builtin": {
      "fg": "info"
    },
    "string": "hint",
    "constant.numeric": "info",
    "constant.character.escape": {
      "fg": "hint",
      "modifiers": [
        "bold"
      ]
    },
    "label": "constant",
    "module": "info",
    "diff.plus": "green0",
    "diff.delta": "warn",
    "diff.minus": "error",
    "warning": "warn",
    "error": "error",
    "info": "info",
    "hint": "hint",
    "diagnostic.error": {
      "underline": {
        "style": "curl",
        "color": "error"
      }
    },
    "diagnostic.warning": {
      "underline": {
        "style": "curl",
        "color": "warn"
      }
    },
    "diagnostic.info": {
      "underline": {
        "style": "curl",
        "color": "info"
      }
    },
    "diagnostic.hint": {
      "underline": {
        "style": "curl",
        "color": "hint"
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
    "ui.background": {
      "bg": "bg0"
    },
    "ui.linenr": {
      "fg": "gray0"
    },
    "ui.linenr.selected": {
      "fg": "gray1",
      "modifiers": [
        "bold"
      ]
    },
    "ui.cursorline": {
      "bg": "bg1"
    },
    "ui.cursorline.secondary": {
      "bg": "bg2"
    },
    "ui.cursorcolumn.primary": {
      "bg": "bg1"
    },
    "ui.cursorcolumn.secondary": {
      "bg": "bg2"
    },
    "ui.statusline": {
      "fg": "info",
      "bg": "bg1"
    },
    "ui.statusline.normal": {
      "fg": "hint",
      "bg": "bg1",
      "modifiers": [
        "reversed",
        "bold"
      ]
    },
    "ui.statusline.insert": {
      "fg": "green0",
      "bg": "bg1",
      "modifiers": [
        "reversed",
        "bold"
      ]
    },
    "ui.statusline.select": {
      "fg": "ultramarine4_dark",
      "bg": "bg1",
      "modifiers": [
        "reversed",
        "bold"
      ]
    },
    "ui.statusline.inactive": {
      "fg": "gray0",
      "bg": "bg1"
    },
    "ui.popup": {
      "bg": "bg1"
    },
    "ui.window": {
      "fg": "bg1",
      "bg": "bg0"
    },
    "ui.help": {
      "bg": "bg1",
      "fg": "fg0"
    },
    "ui.text": {
      "fg": "fg1"
    },
    "ui.text.focus": {
      "fg": "selection0"
    },
    "ui.selection": {
      "bg": "selection1"
    },
    "ui.selection.primary": {
      "bg": "selection1"
    },
    "ui.cursor.primary": {
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.match": {
      "fg": "bg1",
      "bg": "ultramarine4_dark"
    },
    "ui.menu": {
      "fg": "fg0",
      "bg": "bg1"
    },
    "ui.menu.selected": {
      "fg": "selection0",
      "bg": "bg2",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual.wrap": "bg2",
    "ui.virtual.whitespace": "fg1",
    "ui.virtual.indent-guide": "bg2",
    "ui.virtual.ruler": {
      "bg": "bg1"
    },
    "ui.virtual.inlay-hint": "gray1",
    "ui.virtual.inlay-hint.parameter": {
      "fg": "gray1",
      "modifiers": [
        "dim"
      ]
    },
    "ui.virtual.inlay-hint.type": {
      "fg": "gray1",
      "modifiers": [
        "dim"
      ]
    },
    "ui.debug.breakpoint": "error",
    "ui.debug.active": "warn",
    "markup.heading": "info",
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
    "markup.link.url": {
      "fg": "warn",
      "underline": {
        "style": "line"
      }
    },
    "markup.link.text": "selection0",
    "markup.raw": "selection0"
  }
});
