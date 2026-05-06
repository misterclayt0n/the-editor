import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "zed_onelight",
  "title": "Zed Onelight",
  "palette": {
    "yellow": "#dabb7e",
    "orange": "#d3604fff",
    "blue": "#5b79e3ff",
    "blue-gray": "#d9dcea",
    "red": "#d36151ff",
    "purple": "#a449abff",
    "green": "#649f57ff",
    "cyan": "#3882b7ff",
    "gray": "#eaeaed",
    "light-gray": "#a2a3a7ff",
    "faint-gray": "#efefef",
    "linenr": "#b0b1b3",
    "white": "#fafafaff",
    "black": "#383a41ff",
    "ui-text": "#383a41ff",
    "ui-text-reversed": "#fafafaff",
    "dark-blue": "#4a62db",
    "violet": "#9294beff",
    "gold": "#ad6e25ff",
    "light-black": "#2e323a",
    "dark-gray": "#ebebec"
  },
  "styles": {
    "attribute": {
      "fg": "green"
    },
    "comment": {
      "fg": "light-gray",
      "modifiers": [
        "italic"
      ]
    },
    "constant": {
      "fg": "green"
    },
    "constant.numeric": {
      "fg": "gold"
    },
    "constant.builtin": {
      "fg": "gold"
    },
    "constant.builtin.boolean": {
      "fg": "green"
    },
    "constant.character.escape": {
      "fg": "yellow"
    },
    "constructor": {
      "fg": "blue"
    },
    "function": {
      "fg": "blue"
    },
    "function.builtin": {
      "fg": "blue"
    },
    "function.method": {
      "fg": "blue"
    },
    "function.macro": {
      "fg": "blue"
    },
    "keyword": {
      "fg": "purple"
    },
    "label": {
      "fg": "ui-text"
    },
    "namespace": {
      "fg": "ui-text"
    },
    "operator": {
      "fg": "ui-text"
    },
    "puncuation": {
      "fg": "ui-text"
    },
    "special": {
      "fg": "ui-text"
    },
    "string": {
      "fg": "green"
    },
    "type": {
      "fg": "cyan"
    },
    "variable.builtin": {
      "fg": "gold"
    },
    "variable.parameter": {
      "fg": "ui-text"
    },
    "variable.other.member": {
      "fg": "orange"
    },
    "markup.heading": {
      "fg": "orange"
    },
    "markup.raw.inline": {
      "fg": "green"
    },
    "markup.bold": {
      "fg": "yellow",
      "modifiers": [
        "bold"
      ]
    },
    "markup.italic": {
      "fg": "purple",
      "modifiers": [
        "italic"
      ]
    },
    "markup.strikethrough": {
      "modifiers": [
        "crossed_out"
      ]
    },
    "markup.list": {
      "fg": "orange"
    },
    "markup.quote": {
      "fg": "green"
    },
    "markup.link.url": {
      "fg": "cyan",
      "modifiers": [
        "underlined"
      ]
    },
    "markup.link.text": {
      "fg": "purple"
    },
    "diff.plus": "green",
    "diff.delta": "yellow",
    "diff.minus": "red",
    "diagnostic.info": {
      "underline": {
        "color": "blue",
        "style": "curl"
      }
    },
    "diagnostic.hint": {
      "underline": {
        "color": "green",
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
    "info": {
      "fg": "blue",
      "modifiers": [
        "bold"
      ]
    },
    "hint": {
      "fg": "green",
      "modifiers": [
        "bold"
      ]
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
    "ui.background": {
      "bg": "ui-text-reversed"
    },
    "ui.gutter": {
      "bg": "gray"
    },
    "ui.virtual": {
      "fg": "faint-gray"
    },
    "ui.virtual.indent-guide": {
      "fg": "faint-gray"
    },
    "ui.virtual.whitespace": {
      "fg": "light-gray"
    },
    "ui.virtual.ruler": {
      "bg": "gray"
    },
    "ui.virtual.inlay-hint": {
      "fg": "violet",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual.jump-label": {
      "fg": "purple",
      "modifiers": [
        "bold",
        "italic"
      ]
    },
    "ui.cursor": {
      "fg": "dark-blue",
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.primary": {
      "fg": "dark-blue",
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.match": {
      "fg": "blue",
      "modifiers": [
        "underlined"
      ]
    },
    "ui.selection": {
      "bg": "faint-gray"
    },
    "ui.selection.primary": {
      "bg": "blue-gray"
    },
    "ui.cursorline.primary": {
      "bg": "faint-gray"
    },
    "ui.highlight": {
      "bg": "gray"
    },
    "ui.highlight.frameline": {
      "bg": "#97202a"
    },
    "ui.linenr": {
      "fg": "linenr"
    },
    "ui.linenr.selected": {
      "fg": "ui-text"
    },
    "ui.statusline": {
      "fg": "black",
      "bg": "gray"
    },
    "ui.statusline.inactive": {
      "fg": "white",
      "bg": "light-black"
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
    "ui.text": {
      "fg": "ui-text"
    },
    "ui.text.focus": {
      "fg": "ui-text",
      "bg": "gray",
      "modifiers": [
        "bold"
      ]
    },
    "ui.help": {
      "fg": "black",
      "bg": "dark-gray"
    },
    "ui.popup": {
      "bg": "dark-gray"
    },
    "ui.window": {
      "fg": "dark-gray"
    },
    "ui.menu": {
      "fg": "ui-text",
      "bg": "gray"
    },
    "ui.menu.selected": {
      "fg": "ui-text-reversed",
      "bg": "blue"
    },
    "ui.menu.scroll": {
      "fg": "ui-text",
      "bg": "light-gray"
    },
    "ui.debug": {
      "fg": "red"
    },
    "ui.cursor.insert": {
      "fg": "dark-blue"
    }
  }
});
