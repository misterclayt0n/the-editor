import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "kaolin-light",
  "title": "Kaolin Light",
  "palette": {
    "bg0": "#edeeeb",
    "bg1": "#dfe1dc",
    "bg2": "#d1d4cd",
    "fg0": "#383e3f",
    "fg1": "#bebec4",
    "gray0": "#7c878a",
    "gray1": "#919a9c",
    "green0": "#39854c",
    "selection0": "#4c7a90",
    "selection1": "#d3e4f0",
    "type": "#e36b3f",
    "hint": "#13665f",
    "info": "#3b84cc",
    "warn": "#dbac66",
    "error": "#e84c58",
    "constant": "#845A84",
    "ultramarine4_dark": "#968cc7",
    "violet7": "#766884",
    "green1": "#18a318",
    "ultramarine1": "#6d46e3"
  },
  "styles": {
    "attribute": "constant",
    "keyword": {
      "fg": "hint"
    },
    "keyword.directive": "selection0",
    "namespace": "constant",
    "punctuation": "fg0",
    "punctuation.delimiter": "fg0",
    "operator": "fg0",
    "special": "ultramarine1",
    "variable.other.member": {
      "fg": "constant",
      "modifiers": [
        "italic"
      ]
    },
    "variable": "ultramarine1",
    "variable.builtin": "hint",
    "variable.parameter": "constant",
    "type": "type",
    "type.builtin": "selection0",
    "constructor": {
      "fg": "type"
    },
    "function": {
      "fg": "selection0"
    },
    "function.macro": "constant",
    "function.builtin": "ultramarine1",
    "tag": {
      "fg": "hint",
      "modifiers": [
        "italic"
      ]
    },
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
      "fg": "selection0"
    },
    "string": "green0",
    "constant.numeric": "selection0",
    "constant.character.escape": {
      "fg": "hint",
      "modifiers": [
        "bold"
      ]
    },
    "label": "constant",
    "module": "selection0",
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
      "fg": "green1",
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
      "fg": "selection0",
      "bg": "bg1"
    },
    "ui.statusline.normal": {
      "fg": "hint",
      "bg": "bg1",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.insert": {
      "fg": "warn",
      "bg": "bg1",
      "modifiers": [
        "reversed",
        "bold"
      ]
    },
    "ui.statusline.select": {
      "fg": "constant",
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
      "fg": "fg0"
    },
    "ui.text.focus": {
      "fg": "green1",
      "modifiers": [
        "bold"
      ]
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
      "fg": "fg1",
      "bg": "gray0"
    },
    "ui.menu": {
      "fg": "fg0",
      "bg": "bg1"
    },
    "ui.menu.selected": {
      "fg": "green1",
      "bg": "bg2",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual.wrap": "fg1",
    "ui.virtual.whitespace": "fg1",
    "ui.virtual.indent-guide": "fg1",
    "ui.virtual.ruler": {
      "bg": "bg1"
    },
    "ui.virtual.inlay-hint": "gray0",
    "ui.virtual.inlay-hint.parameter": {
      "fg": "gray0",
      "modifiers": [
        "italic"
      ]
    },
    "ui.virtual.inlay-hint.type": {
      "fg": "gray0",
      "modifiers": [
        "italic"
      ]
    },
    "ui.debug.breakpoint": "error",
    "ui.debug.active": "warn",
    "markup.heading": {
      "fg": "ultramarine1",
      "modifiers": [
        "bold"
      ]
    },
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
      "fg": "info",
      "underline": {
        "style": "line"
      }
    },
    "markup.link.text": "info",
    "markup.raw": "constant"
  }
});
