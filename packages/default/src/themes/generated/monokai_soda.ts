import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "monokai_soda",
  "title": "Monokai Soda",
  "palette": {
    "purple": "#AE81FF",
    "yellow": "#E6DB74",
    "pink": "#f92a72",
    "white": "#cfcfc2",
    "gray": "#75715e",
    "darkgray": "#444444",
    "black": "#222222",
    "blue": "#66d9ef",
    "green": "#a6e22e",
    "softorange": "#f59762",
    "orange": "#fd971f",
    "background": "#191919"
  },
  "styles": {
    "constant": "white",
    "constant.builtin": "pink",
    "constant.character.escape": "blue",
    "constant.numeric": "purple",
    "diagnostic": {
      "modifiers": [
        "underlined"
      ]
    },
    "diagnostic.error": {
      "underline": {
        "style": "curl",
        "color": "pink"
      }
    },
    "diagnostic.warning": {
      "underline": {
        "style": "curl",
        "color": "orange"
      }
    },
    "diagnostic.info": {
      "underline": {
        "style": "curl",
        "color": "white"
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
    "diff.plus": "green",
    "diff.delta": "orange",
    "diff.minus": "pink",
    "diff.delta.moved": "orange",
    "function": "green",
    "function.macro": "blue",
    "function.builtin": "pink",
    "constructor": "blue",
    "keyword": "pink",
    "keyword.directive": "blue",
    "punctuation": "gray",
    "string": "yellow",
    "type": "blue",
    "type.builtin": "pink",
    "variable": "white",
    "variable.builtin": "pink",
    "variable.other.member": "white",
    "variable.parameter": "softorange",
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
    "markup.link.url": {
      "fg": "orange",
      "modifiers": [
        "underlined"
      ]
    },
    "markup.link.text": "yellow",
    "markup.quote": "green",
    "attribute": "blue",
    "comment": {
      "fg": "gray",
      "modifiers": [
        "italic"
      ]
    },
    "error": "pink",
    "hint": "white",
    "info": "white",
    "label": "yellow",
    "module": "softorange",
    "namespace": "pink",
    "operator": "pink",
    "special": "softorange",
    "warning": "orange",
    "ui.background": {
      "bg": "background"
    },
    "ui.text": "white",
    "ui.window": {
      "bg": "darkgray"
    },
    "ui.menu": {
      "fg": "white",
      "bg": "darkgray"
    },
    "ui.menu.selected": {
      "modifiers": [
        "reversed"
      ]
    },
    "ui.popup": {
      "bg": "darkgray"
    },
    "ui.help": {
      "fg": "white",
      "bg": "darkgray"
    },
    "ui.linenr": "darkgray",
    "ui.linenr.selected": "orange",
    "ui.cursor.primary": {
      "fg": "white",
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.match": {
      "fg": "white",
      "modifiers": [
        "reversed"
      ]
    },
    "ui.selection": {
      "bg": "darkgray"
    },
    "ui.statusline": {
      "bg": "darkgray"
    },
    "ui.statusline.inactive": {
      "fg": "white",
      "bg": "darkgray"
    },
    "ui.statusline.normal": {
      "fg": "white",
      "bg": "blue"
    },
    "ui.statusline.insert": {
      "fg": "white",
      "bg": "green"
    },
    "ui.statusline.select": {
      "fg": "white",
      "bg": "purple"
    },
    "ui.text.focus": {
      "fg": "yellow",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual": "darkgray",
    "ui.virtual.ruler": {
      "bg": "darkgray"
    },
    "ui.virtual.jump-label": {
      "fg": "softorange",
      "modifiers": [
        "bold"
      ]
    }
  }
});
