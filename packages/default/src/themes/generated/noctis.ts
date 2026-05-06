import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "noctis",
  "title": "Noctis",
  "palette": {
    "dark-green": "#00262a",
    "mid-green": "#073a40",
    "autocomp-green": "#0d6772",
    "light-green": "#48e9a7",
    "pink": "#df769b",
    "yellow": "#ffd800",
    "purple": "#918cff",
    "white": "#b1cace",
    "orange": "#ffa864",
    "light-orange": "#fff2c5",
    "gray": "#5b858b",
    "light-gray": "#2a3c41",
    "red": "#e34e1b",
    "dark-blue": "#19a2b7",
    "mid-blue": "#47ace8",
    "light-blue": "#87efff"
  },
  "styles": {
    "property": {
      "fg": "red"
    },
    "warning": {
      "fg": "yellow",
      "modifiers": [
        "bold"
      ]
    },
    "error": {
      "fg": "red",
      "modifiers": [
        "bold"
      ]
    },
    "info": {
      "fg": "mid-blue"
    },
    "hint": {
      "fg": "light-green",
      "modifiers": [
        "bold"
      ]
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
    "diagnostic.info": {
      "underline": {
        "color": "mid-blue",
        "style": "curl"
      }
    },
    "diagnostic.hint": {
      "underline": {
        "color": "light-green",
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
    "ui.background": {
      "fg": "autocomp-green",
      "bg": "dark-green"
    },
    "ui.window": {
      "fg": "mid-green"
    },
    "ui.text": {
      "fg": "white"
    },
    "ui.text.focus": {
      "fg": "white",
      "bg": "mid-green",
      "modifiers": [
        "bold"
      ]
    },
    "ui.text.info": {
      "fg": "white"
    },
    "ui.cursor": {
      "fg": "dark-green",
      "bg": "white"
    },
    "ui.cursor.primary": {
      "fg": "dark-green",
      "bg": "light-blue"
    },
    "ui.cursor.insert": {
      "fg": "dark-green",
      "bg": "light-blue"
    },
    "ui.cursor.select": {
      "fg": "dark-green",
      "bg": "light-blue"
    },
    "ui.cursor.match": {
      "fg": "dark-green",
      "bg": "red",
      "modifiers": [
        "bold"
      ]
    },
    "ui.selection": {
      "bg": "autocomp-green"
    },
    "ui.selection.primary": {
      "bg": "autocomp-green"
    },
    "ui.cursorline.primary": {
      "bg": "mid-green"
    },
    "ui.linenr": {
      "fg": "gray"
    },
    "ui.linenr.selected": {
      "fg": "light-green",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual": {
      "fg": "autocomp-green"
    },
    "ui.virtual.ruler": {
      "bg": "mid-green"
    },
    "ui.virtual.whitespace": {
      "fg": "mid-green"
    },
    "ui.virtual.inlay-hint": {
      "fg": "autocomp-green"
    },
    "ui.virtual.indent-guide": {
      "fg": "mid-green"
    },
    "ui.virtual.wrap": {
      "fg": "mid-green"
    },
    "ui.virtual.jump-label": {
      "fg": "autocomp-green",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline": {
      "fg": "light-green",
      "bg": "autocomp-green"
    },
    "ui.statusline.inactive": {
      "fg": "white",
      "bg": "mid-green"
    },
    "ui.statusline.normal": {
      "fg": "dark-green",
      "bg": "mid-blue",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.insert": {
      "fg": "dark-green",
      "bg": "pink",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.select": {
      "fg": "dark-green",
      "bg": "yellow",
      "modifiers": [
        "bold"
      ]
    },
    "ui.help": {
      "bg": "mid-green",
      "fg": "white"
    },
    "ui.highlight": {
      "bg": "mid-green"
    },
    "ui.menu": {
      "fg": "white",
      "bg": "mid-green"
    },
    "ui.menu.selected": {
      "fg": "light-green",
      "bg": "autocomp-green"
    },
    "ui.popup": {
      "bg": "mid-green"
    },
    "special": {
      "fg": "mid-blue"
    },
    "attribute": {
      "fg": "yellow"
    },
    "type": {
      "fg": "orange"
    },
    "constructor": {
      "fg": "light-blue",
      "modifiers": [
        "bold"
      ]
    },
    "constant": {
      "fg": "light-blue",
      "modifiers": [
        "bold"
      ]
    },
    "constant.builtin": {
      "fg": "mid-blue",
      "modifiers": [
        "bold"
      ]
    },
    "constant.character": {
      "fg": "light-green"
    },
    "constant.character.escape": {
      "fg": "purple",
      "modifiers": [
        "bold"
      ]
    },
    "constant.numeric": {
      "fg": "purple"
    },
    "string": {
      "fg": "light-green"
    },
    "string.special": {
      "fg": "red"
    },
    "comment": {
      "fg": "gray",
      "modifiers": [
        "italic"
      ]
    },
    "variable": {
      "fg": "light-orange"
    },
    "variable.other.member": {
      "fg": "orange"
    },
    "label": {
      "fg": "purple"
    },
    "punctuation": {
      "fg": "yellow",
      "modifiers": [
        "bold"
      ]
    },
    "keyword": {
      "fg": "pink",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.control": {
      "fg": "pink",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.control.conditional": {
      "fg": "pink",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.control.import": {
      "fg": "pink",
      "modifiers": [
        "italic"
      ]
    },
    "keyword.control.exception": {
      "fg": "pink",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.operator": {
      "fg": "pink"
    },
    "keyword.directive": {
      "fg": "purple"
    },
    "keyword.function": {
      "fg": "red"
    },
    "operator": {
      "fg": "pink",
      "modifiers": [
        "bold"
      ]
    },
    "function": {
      "fg": "mid-blue"
    },
    "function.builtin": {
      "fg": "dark-blue"
    },
    "function.method": {
      "fg": "dark-blue"
    },
    "function.macro": {
      "fg": "purple"
    },
    "tag": {
      "fg": "yellow"
    },
    "namespace": {
      "fg": "mid-blue"
    },
    "markup.heading.marker": {
      "fg": "red"
    },
    "markup.list": {
      "fg": "red"
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
    "markup.link": {
      "fg": "light-blue",
      "modifiers": [
        "underlined"
      ]
    },
    "markup.link.text": {
      "fg": "purple"
    },
    "diff.plus": "light-green",
    "diff.minus": "red",
    "diff.delta": "yellow"
  }
});
