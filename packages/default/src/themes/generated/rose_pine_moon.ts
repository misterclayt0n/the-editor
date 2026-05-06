import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "rose_pine_moon",
  "title": "Rose Pine Moon",
  "palette": {
    "base": "#232136",
    "surface": "#2a273f",
    "overlay": "#393552",
    "muted": "#6e6a86",
    "subtle": "#908caa",
    "text": "#e0def4",
    "love": "#eb6f92",
    "love_10": "#3a2841",
    "gold": "#f6c177",
    "gold_10": "#39313e",
    "rose": "#ea9a97",
    "rose_10": "#392d41",
    "pine": "#3e8fb0",
    "pine_10": "#252c44",
    "foam": "#9ccfd8",
    "foam_10": "#2e3248",
    "iris": "#c4a7e7",
    "iris_10": "#342e4a",
    "highlight_low": "#2a283e",
    "highlight_med": "#44415a",
    "highlight_high": "#56526e"
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
