import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "rose_pine_dawn",
  "title": "Rose Pine Dawn",
  "palette": {
    "base": "#faf4ed",
    "surface": "#fffaf3",
    "overlay": "#f2e9e1",
    "muted": "#9893a5",
    "subtle": "#797593",
    "text": "#575279",
    "love": "#b4637a",
    "love_10": "#f6e4e0",
    "gold": "#ea9d34",
    "gold_10": "#fbead8",
    "rose": "#d7827e",
    "rose_10": "#fae8e1",
    "pine": "#286983",
    "pine_10": "#e5e6e2",
    "foam": "#56949f",
    "foam_10": "#eaeae5",
    "iris": "#907aa9",
    "iris_10": "#f1e8e6",
    "highlight_low": "#f4ede8",
    "highlight_med": "#dfdad9",
    "highlight_high": "#cecacd"
  },
  "styles": {
    "ui.background": {
      "bg": "base"
    },
    "ui.background.separator": {
      "bg": "base"
    },
    "ui.cursor": {
      "fg": "text",
      "bg": "highlight_high"
    },
    "ui.cursor.match": {
      "fg": "text",
      "bg": "highlight_med"
    },
    "ui.cursor.primary": {
      "fg": "text",
      "bg": "muted"
    },
    "ui.linenr": {
      "fg": "muted"
    },
    "ui.linenr.selected": {
      "fg": "text"
    },
    "ui.bufferline": {
      "fg": "muted",
      "bg": "base"
    },
    "ui.bufferline.active": {
      "fg": "text",
      "bg": "overlay"
    },
    "ui.statusline": {
      "fg": "subtle",
      "bg": "surface"
    },
    "ui.statusline.inactive": {
      "fg": "muted",
      "bg": "surface"
    },
    "ui.statusline.normal": {
      "fg": "rose",
      "bg": "rose_10"
    },
    "ui.statusline.insert": {
      "fg": "foam",
      "bg": "foam_10"
    },
    "ui.statusline.select": {
      "fg": "iris",
      "bg": "iris_10"
    },
    "ui.popup": {
      "bg": "surface"
    },
    "ui.popup.info": {
      "bg": "surface"
    },
    "ui.window": {
      "fg": "overlay",
      "bg": "base"
    },
    "ui.help": {
      "fg": "subtle",
      "bg": "overlay"
    },
    "ui.text": {
      "fg": "text"
    },
    "ui.text.focus": {
      "bg": "overlay"
    },
    "ui.text.info": {
      "fg": "subtle"
    },
    "ui.text.directory": {
      "fg": "iris"
    },
    "ui.virtual.jump-label": {
      "fg": "love",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual.ruler": {
      "bg": "overlay"
    },
    "ui.virtual.whitespace": {
      "fg": "highlight_high"
    },
    "ui.virtual.indent-guide": {
      "fg": "muted"
    },
    "ui.virtual.inlay-hint": {
      "fg": "subtle"
    },
    "ui.menu": {
      "fg": "subtle",
      "bg": "surface"
    },
    "ui.menu.selected": {
      "fg": "text",
      "bg": "overlay"
    },
    "ui.menu.scroll": {
      "fg": "muted",
      "bg": "highlight_med"
    },
    "ui.selection": {
      "bg": "overlay"
    },
    "ui.selection.primary": {
      "bg": "highlight_med"
    },
    "ui.cursorline.primary": {
      "bg": "highlight_low"
    },
    "ui.cursorline.secondary": {
      "bg": "surface"
    },
    "warning": "gold",
    "error": "love",
    "info": "foam",
    "hint": "iris",
    "debug": "rose",
    "diagnostic": {
      "underline": {
        "color": "subtle",
        "style": "curl"
      }
    },
    "diagnostic.hint": {
      "underline": {
        "color": "iris",
        "style": "curl"
      }
    },
    "diagnostic.info": {
      "underline": {
        "color": "foam",
        "style": "curl"
      }
    },
    "diagnostic.warning": {
      "underline": {
        "color": "gold",
        "style": "curl"
      }
    },
    "diagnostic.error": {
      "underline": {
        "color": "love",
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
    "special": "rose",
    "attribute": "iris",
    "type": "foam",
    "constructor": "foam",
    "constant": "foam",
    "constant.builtin": "love",
    "constant.builtin.boolean": "rose",
    "constant.character": "gold",
    "constant.character.escape": "pine",
    "constant.numeric": "gold",
    "string": "gold",
    "comment": {
      "fg": "muted",
      "modifiers": [
        "italic"
      ]
    },
    "variable": "text",
    "variable.builtin": "love",
    "variable.parameter": "iris",
    "variable.other.member": "foam",
    "label": "foam",
    "punctuation": "subtle",
    "keyword": "pine",
    "keyword.operator": "subtle",
    "operator": "subtle",
    "function": "rose",
    "function.builtin": "love",
    "tag": "foam",
    "namespace": "text",
    "markup.heading.marker": "muted",
    "markup.heading": {
      "fg": "iris",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.1": {
      "fg": "iris",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.2": {
      "fg": "foam",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.3": {
      "fg": "rose",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.4": {
      "fg": "gold",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.5": {
      "fg": "pine",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.6": {
      "fg": "foam",
      "modifiers": [
        "bold"
      ]
    },
    "markup.list": "muted",
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
    "markup.link": "iris",
    "markup.link.url": {
      "fg": "iris",
      "underline": {
        "color": "iris",
        "style": "line"
      }
    },
    "markup.link.label": "subtle",
    "markup.link.text": "text",
    "markup.quote": "subtle",
    "markup.raw": "subtle",
    "diff": "overlay",
    "diff.plus": "foam",
    "diff.minus": "love",
    "diff.delta": "highlight_high"
  }
});
