import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "ayu_evolve",
  "title": "Ayu Evolve",
  "palette": {
    "background": "#020202",
    "foreground": "#bfbdb6",
    "black": "#0D0D0D",
    "blue": "#59c2ff",
    "dark_gray": "#2d3640",
    "cyan": "#73b8ff",
    "gray": "#5c6773",
    "green": "#aad94c",
    "magenta": "#d2a6ff",
    "orange": "#ff8f40",
    "red": "#DD3E25",
    "yellow": "#e6b450",
    "light_gray": "#dedede",
    "vibrant_yellow": "#CFCA0D",
    "vibrant_orange": "#FF8732"
  },
  "styles": {
    "type": "blue",
    "type.builtin": {
      "fg": "blue",
      "modifiers": [
        "italic"
      ]
    },
    "constructor": "magenta",
    "constant": "magenta",
    "string": "green",
    "string.regexp": "orange",
    "string.special": "yellow",
    "comment": {
      "fg": "gray",
      "modifiers": [
        "italic"
      ]
    },
    "variable": "light_gray",
    "label": "orange",
    "punctuation": "foreground",
    "keyword": "orange",
    "keyword.control": "orange",
    "keyword.directive": "yellow",
    "operator": "orange",
    "function": "yellow",
    "tag": "blue",
    "namespace": "blue",
    "markup.heading": "orange",
    "markup.list": "yellow",
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
    "markup.raw.block": {
      "bg": "black"
    },
    "markup.link.url": "blue",
    "markup.link.text": "yellow",
    "markup.link.label": "green",
    "markup.quote": "yellow",
    "diff.plus": "green",
    "diff.minus": "red",
    "diff.delta": "yellow",
    "ui.background": {
      "bg": "background"
    },
    "ui.cursor": {
      "fg": "dark_gray",
      "bg": "light_gray"
    },
    "ui.cursor.primary": {
      "fg": "dark_gray",
      "bg": "orange"
    },
    "ui.cursor.match": "orange",
    "ui.linenr": "dark_gray",
    "ui.linenr.selected": "gray",
    "ui.statusline": {
      "fg": "foreground",
      "bg": "black"
    },
    "ui.statusline.normal": {
      "fg": "black",
      "bg": "blue"
    },
    "ui.statusline.insert": {
      "fg": "black",
      "bg": "orange"
    },
    "ui.statusline.select": {
      "fg": "black",
      "bg": "magenta"
    },
    "ui.cursorline": {
      "bg": "black"
    },
    "ui.popup": {
      "fg": "#7B91b3",
      "bg": "black"
    },
    "ui.window": "dark_gray",
    "ui.help": {
      "fg": "#7B91b3",
      "bg": "black"
    },
    "ui.text": "foreground",
    "ui.text.focus": {
      "bg": "dark_gray",
      "fg": "foreground"
    },
    "ui.text.info": "foreground",
    "ui.virtual.whitespace": "dark_gray",
    "ui.virtual.ruler": {
      "bg": "black"
    },
    "ui.virtual.inlay-hint": {
      "fg": "#e6b450",
      "bg": "#302a20"
    },
    "ui.menu": {
      "fg": "foreground",
      "bg": "black"
    },
    "ui.menu.selected": {
      "bg": "gray",
      "fg": "background"
    },
    "ui.selection": {
      "bg": "dark_gray"
    },
    "warning": "vibrant_orange",
    "error": "red",
    "info": "white",
    "hint": "vibrant_yellow",
    "diagnostic.hint": {
      "underline": {
        "color": "vibrant_yellow",
        "style": "curl"
      }
    },
    "diagnostic.info": {
      "underline": {
        "color": "white",
        "style": "curl"
      }
    },
    "diagnostic.warning": {
      "underline": {
        "color": "vibrant_orange",
        "style": "curl"
      }
    },
    "diagnostic.error": {
      "underline": {
        "color": "red",
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
    "ui.bufferline": {
      "fg": "light_gray",
      "bg": "background"
    },
    "ui.bufferline.active": {
      "fg": "light_gray",
      "bg": "dark_gray"
    },
    "ui.debug": {
      "fg": "orange",
      "bg": "background"
    },
    "ui.highlight.frameline": {
      "bg": "#0067a3"
    },
    "ui.virtual.jump-label": {
      "fg": "red",
      "modifiers": [
        "italic",
        "bold"
      ]
    },
    "special": "orange",
    "keyword.storage": "yellow",
    "keyword.storage.modifier": "magenta",
    "markup.raw.inline": {
      "bg": "black"
    },
    "ui.cursor.primary.select": {
      "fg": "dark_gray",
      "bg": "magenta"
    },
    "ui.cursor.primary.insert": {
      "fg": "dark_gray",
      "bg": "green"
    },
    "ui.text.inactive": "gray"
  }
});
