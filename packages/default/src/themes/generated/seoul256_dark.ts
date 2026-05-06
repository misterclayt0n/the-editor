import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "seoul256-dark",
  "title": "Seoul256 Dark",
  "palette": {
    "black": "#000000",
    "black1": "#14161B",
    "red": "#d70000",
    "green": "#afd75f",
    "green1": "#5f875f",
    "green2": "#5f8700",
    "green3": "#87af87",
    "green4": "#5f5f00",
    "yellow": "#d8af5f",
    "yellow1": "#ffd787",
    "yellow2": "#d7d7af",
    "yellow3": "#d7d787",
    "yellow4": "#87875f",
    "yellow5": "#6B5300",
    "blue": "#87afaf",
    "blue1": "#5f87d7",
    "blue2": "#5f5f87",
    "blue3": "#a6dbff",
    "blue4": "#005f5f",
    "blue5": "#5fafaf",
    "blue6": "#008787",
    "magenta": "#af5f5f",
    "magenta1": "#af5f87",
    "magenta2": "#875f5f",
    "magenta3": "#d7005f",
    "cyan": "#87d7d7",
    "cyan1": "#afd7d7",
    "white": "#d0d0d0",
    "white1": "#dadada",
    "purple": "#8787af",
    "brown": "#af875f",
    "brown1": "#875f00",
    "orange": "#ff5f00",
    "salmon": "#ffaf87",
    "salmon1": "#d78787",
    "salmon2": "#d7afaf",
    "salmon3": "#d7875f",
    "mauve": "#d75f87",
    "gray": "#121212",
    "gray1": "#1c1c1c",
    "gray2": "#262626",
    "gray3": "#303030",
    "gray4": "#3a3a3a",
    "gray5": "#444444",
    "gray6": "#585858",
    "gray7": "#626262",
    "gray8": "#4e4e4e",
    "gray9": "#5f5f5f",
    "gray10": "#c6c6c6",
    "gray11": "#eeeeee",
    "gray12": "#e4e4e4",
    "gray13": "#bcbcbc"
  },
  "styles": {
    "attribute": {
      "fg": "yellow"
    },
    "comment": {
      "fg": "green1"
    },
    "constant": {
      "fg": "blue5"
    },
    "constant.numeric": {
      "fg": "yellow1"
    },
    "constant.builtin.boolean": {
      "fg": "purple"
    },
    "constant.character.escape": {
      "fg": "salmon"
    },
    "constructor": {
      "fg": "yellow"
    },
    "function": {
      "fg": "yellow2"
    },
    "function.builtin": {
      "fg": "blue1"
    },
    "function.method": {
      "fg": "salmon"
    },
    "function.macro": {
      "fg": "yellow2"
    },
    "keyword": {
      "fg": "mauve"
    },
    "label": {
      "fg": "magenta"
    },
    "namespace": {
      "fg": "cyan"
    },
    "operator": {
      "fg": "yellow3"
    },
    "punctuation": {
      "fg": "brown"
    },
    "special": {
      "fg": "yellow2"
    },
    "string": {
      "fg": "blue"
    },
    "type": {
      "fg": "yellow"
    },
    "type.builtin": {
      "fg": "salmon"
    },
    "variable": {
      "fg": "white"
    },
    "variable.builtin": {
      "fg": "salmon"
    },
    "variable.parameter": {
      "fg": "white"
    },
    "variable.other.member": {
      "fg": "white"
    },
    "markup.heading": {
      "fg": "salmon2",
      "modifiers": [
        "bold"
      ]
    },
    "markup.raw.inline": {
      "fg": "green"
    },
    "markup.bold": {
      "fg": "yellow1",
      "modifiers": [
        "bold"
      ]
    },
    "markup.italic": {
      "fg": "magenta1",
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
      "fg": "salmon"
    },
    "markup.quote": {
      "fg": "yellow"
    },
    "markup.link.url": {
      "fg": "cyan",
      "modifiers": [
        "underlined"
      ]
    },
    "markup.link.text": {
      "fg": "salmon2"
    },
    "diff.plus": {
      "fg": "green3"
    },
    "diff.delta": {
      "fg": "blue1"
    },
    "diff.minus": {
      "fg": "magenta3"
    },
    "diagnostic.info": {
      "underline": {
        "color": "cyan",
        "style": "curl"
      }
    },
    "diagnostic.hint": {
      "underline": {
        "color": "green1",
        "style": "curl"
      }
    },
    "diagnostic.warning": {
      "underline": {
        "color": "yellow1",
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
      "fg": "blue2",
      "modifiers": [
        "bold"
      ]
    },
    "hint": {
      "fg": "blue3",
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
      "bg": "gray4"
    },
    "ui.gutter": {
      "bg": "gray5"
    },
    "ui.gutter.selected": {
      "bg": "gray3"
    },
    "ui.virtual": {
      "fg": "gray6"
    },
    "ui.virtual.indent-guide": {
      "fg": "gray6"
    },
    "ui.virtual.whitespace": {
      "fg": "gray6"
    },
    "ui.virtual.ruler": {
      "bg": "gray5"
    },
    "ui.virtual.inlay-hint": {
      "fg": "gray9",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual.jump-label": {
      "fg": "red",
      "modifiers": [
        "bold"
      ]
    },
    "ui.cursor": {
      "fg": "white",
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.primary": {
      "fg": "white",
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.match": {
      "bg": "gray4",
      "modifiers": [
        "underlined"
      ]
    },
    "ui.cursor.insert": {
      "fg": "blue1"
    },
    "ui.selection": {
      "bg": "magenta2"
    },
    "ui.selection.primary": {
      "bg": "blue4"
    },
    "ui.cursorline.primary": {
      "bg": "gray3"
    },
    "ui.highlight": {
      "bg": "gray5"
    },
    "ui.highlight.frameline": {
      "bg": "red"
    },
    "ui.linenr": {
      "fg": "yellow4"
    },
    "ui.linenr.selected": {
      "bg": "gray3",
      "fg": "magenta",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline": {
      "fg": "magenta2",
      "bg": "yellow2"
    },
    "ui.statusline.inactive": {
      "fg": "gray6",
      "bg": "black"
    },
    "ui.statusline.normal": {
      "fg": "black",
      "bg": "blue2"
    },
    "ui.statusline.insert": {
      "fg": "black",
      "bg": "green2"
    },
    "ui.statusline.select": {
      "fg": "black",
      "bg": "magenta"
    },
    "ui.text": {
      "fg": "white"
    },
    "ui.text.focus": {
      "fg": "white",
      "bg": "magenta2",
      "modifiers": [
        "bold"
      ]
    },
    "ui.help": {
      "fg": "white",
      "bg": "gray3"
    },
    "ui.popup": {
      "fg": "white",
      "bg": "gray3"
    },
    "ui.window": {
      "fg": "white"
    },
    "ui.menu": {
      "fg": "white",
      "bg": "gray3"
    },
    "ui.menu.selected": {
      "fg": "white",
      "bg": "magenta2"
    },
    "ui.menu.scroll": {
      "fg": "gray7",
      "bg": "gray6"
    },
    "ui.debug": {
      "fg": "red"
    }
  }
});
