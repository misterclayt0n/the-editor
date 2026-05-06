import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "kaolin-valley-dark",
  "title": "Kaolin Valley Dark",
  "palette": {
    "bg0": "#262221",
    "bg1": "#2e2a29",
    "bg2": "#383030",
    "fg0": "#eee6d3",
    "fg1": "#cea2ca",
    "gray0": "#635c4a",
    "gray1": "#52413f",
    "green0": "#91f368",
    "selection0": "#ee7042",
    "selection1": "#402e2e",
    "type": "#eed891",
    "hint": "#6bd9db",
    "info": "#41b0f3",
    "warn": "#cfb05f",
    "error": "#e84c58",
    "constant": "#ef6787",
    "ultramarine4_dark": "#968cc7",
    "violet7": "#766884",
    "valley-dark-fn": "#7ed7e6",
    "teal3": "#49bdb0",
    "aquamarine3": "#53e6b5",
    "ultramarine3": "#9587dd"
  },
  "styles": {
    "attribute": "fg1",
    "keyword": {
      "fg": "teal3"
    },
    "keyword.directive": "green0",
    "namespace": "constant",
    "punctuation": "fg0",
    "punctuation.delimiter": "fg0",
    "operator": "teal3",
    "special": "ultramarine3",
    "variable.other.member": {
      "fg": "selection0",
      "modifiers": [
        "italic"
      ]
    },
    "variable": "constant",
    "variable.builtin": "teal3",
    "variable.parameter": "constant",
    "type": "type",
    "type.builtin": "aquamarine3",
    "constructor": {
      "fg": "type"
    },
    "function": {
      "fg": "valley-dark-fn"
    },
    "function.macro": "valley-dark-fn",
    "function.builtin": "aquamarine3",
    "tag": "green0",
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
      "fg": "aquamarine3"
    },
    "string": "fg1",
    "constant.numeric": "aquamarine3",
    "constant.character.escape": "fg1",
    "label": "ultramarine3",
    "module": "green0",
    "diff.plus": "green0",
    "diff.delta": "type",
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
      "fg": "warn",
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
      "fg": "fg0",
      "bg": "bg1"
    },
    "ui.statusline.normal": {
      "fg": "warn",
      "bg": "bg1",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.insert": {
      "fg": "hint",
      "bg": "bg1",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.select": {
      "fg": "constant",
      "bg": "bg1",
      "modifiers": [
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
      "fg": "selection0",
      "bg": "bg1"
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
      "fg": "bg0",
      "bg": "selection0"
    },
    "ui.menu": {
      "fg": "fg1",
      "bg": "bg1"
    },
    "ui.menu.selected": {
      "fg": "selection0",
      "bg": "bg1",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual.wrap": "gray1",
    "ui.virtual.whitespace": "gray1",
    "ui.virtual.indent-guide": "gray1",
    "ui.virtual.ruler": {
      "bg": "bg2"
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
    "markup.heading": "type",
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
      "fg": "constant",
      "modifiers": [
        "underlined"
      ]
    },
    "markup.link.text": "valley-dark-fn",
    "markup.raw": "selection0"
  }
});
