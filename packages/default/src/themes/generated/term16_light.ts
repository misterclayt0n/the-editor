import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "term16_light",
  "title": "Term16 Light",
  "palette": {},
  "styles": {
    "ui.background.separator": "light-gray",
    "ui.cursor": {
      "fg": "gray",
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.match": {
      "fg": "yellow",
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.primary": {
      "fg": "black",
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.secondary": {
      "fg": "gray",
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursorline.primary": {
      "bg": "white"
    },
    "ui.gutter.selected": {
      "bg": "white"
    },
    "ui.help": {
      "fg": "white",
      "bg": "black"
    },
    "ui.linenr": {
      "fg": "gray",
      "modifiers": [
        "dim"
      ]
    },
    "ui.linenr.selected": {
      "fg": "black",
      "modifiers": [
        "bold"
      ]
    },
    "ui.menu": {
      "bg": "light-gray"
    },
    "ui.menu.selected": {
      "fg": "white",
      "bg": "gray",
      "modifiers": [
        "bold"
      ]
    },
    "ui.menu.scroll": {
      "fg": "light-blue"
    },
    "ui.popup": {
      "bg": "white"
    },
    "ui.selection": {
      "bg": "light-gray"
    },
    "ui.statusline": {
      "bg": "white"
    },
    "ui.statusline.inactive": {
      "fg": "gray",
      "modifiers": [
        "underlined"
      ]
    },
    "ui.virtual": {
      "fg": "light-gray"
    },
    "ui.virtual.indent-guide": {
      "fg": "light-gray",
      "modifiers": [
        "dim"
      ]
    },
    "ui.virtual.wrap": {
      "fg": "light-gray"
    },
    "ui.virtual.inlay-hint": {
      "fg": "light-gray",
      "modifiers": [
        "dim",
        "italic"
      ]
    },
    "ui.virtual.inlay-hint.parameter": {
      "fg": "yellow",
      "modifiers": [
        "dim",
        "italic"
      ]
    },
    "ui.virtual.inlay-hint.type": {
      "fg": "blue",
      "modifiers": [
        "dim",
        "italic"
      ]
    },
    "ui.window": {
      "fg": "gray",
      "modifiers": [
        "dim"
      ]
    },
    "comment": {
      "fg": "gray",
      "modifiers": [
        "italic",
        "dim"
      ]
    },
    "attribute": "yellow",
    "constant": {
      "fg": "yellow",
      "modifiers": [
        "bold"
      ]
    },
    "constant.numeric": {
      "fg": "yellow",
      "modifiers": [
        "bold"
      ]
    },
    "constant.character.escape": "blue",
    "constructor": "blue",
    "function": "blue",
    "function.macro": "light-red",
    "function.builtin": {
      "fg": "blue",
      "modifiers": [
        "bold"
      ]
    },
    "tag": {
      "fg": "magenta",
      "modifiers": [
        "dim"
      ]
    },
    "type": "blue",
    "type.builtin": {
      "fg": "blue",
      "modifiers": [
        "bold"
      ]
    },
    "type.enum.variant": {
      "fg": "magenta",
      "modifiers": [
        "dim"
      ]
    },
    "string": "green",
    "special": "red",
    "variable": {
      "fg": "black",
      "modifiers": [
        "dim"
      ]
    },
    "variable.parameter": {
      "fg": "red",
      "modifiers": [
        "italic",
        "dim"
      ]
    },
    "variable.other.member": "green",
    "keyword": "magenta",
    "keyword.control.exception": "red",
    "keyword.directive": {
      "fg": "yellow",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.operator": {
      "fg": "blue",
      "modifiers": [
        "bold"
      ]
    },
    "label": "red",
    "namespace": {
      "fg": "blue",
      "modifiers": [
        "dim"
      ]
    },
    "markup.heading": {
      "fg": "blue",
      "modifiers": [
        "bold"
      ]
    },
    "markup.list": "red",
    "markup.bold": {
      "fg": "cyan",
      "modifiers": [
        "bold"
      ]
    },
    "markup.italic": {
      "fg": "blue",
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
      "fg": "magenta",
      "modifiers": [
        "dim"
      ]
    },
    "markup.link.text": {
      "fg": "magenta",
      "modifiers": [
        "bold"
      ]
    },
    "markup.quote": "cyan",
    "markup.raw": "blue",
    "diff.plus": "green",
    "diff.delta": "yellow",
    "diff.minus": "red",
    "diagnostic.hint": {
      "underline": {
        "color": "cyan",
        "style": "curl"
      }
    },
    "diagnostic.info": {
      "underline": {
        "color": "blue",
        "style": "curl"
      }
    },
    "diagnostic.warning": {
      "underline": {
        "color": "yellow",
        "style": "curl"
      }
    },
    "diagnostic.error": {
      "underline": {
        "color": "red",
        "style": "curl"
      }
    },
    "info": "blue",
    "hint": "cyan",
    "debug": "light-yellow",
    "warning": "yellow",
    "error": "red",
    "ui.cursorline.secondary": {
      "bg": "white"
    },
    "ui.cursorcolumn.primary": {
      "bg": "white"
    },
    "ui.cursorcolumn.secondary": {
      "bg": "white"
    },
    "ui.statusline.insert": {
      "fg": "white",
      "bg": "blue"
    },
    "ui.statusline.select": {
      "fg": "white",
      "bg": "magenta"
    },
    "ui.virtual.ruler": {
      "bg": "white"
    }
  }
});
