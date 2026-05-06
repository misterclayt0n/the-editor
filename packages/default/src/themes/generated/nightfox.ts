import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "nightfox",
  "title": "Nightfox",
  "palette": {
    "black": "#393b44",
    "red": "#c94f6d",
    "red-dim": "#2f2837",
    "green": "#81b29a",
    "green-dim": "#26343c",
    "yellow": "#dbc074",
    "yellow-bright": "#e0c989",
    "blue": "#719cd6",
    "blue-bright": "#86abdc",
    "blue-dim": "#2f2837",
    "magenta": "#9d79d6",
    "magenta-bright": "#baa1e2",
    "cyan": "#63cdcf",
    "cyan-bright": "#7ad4d6",
    "cyan-dim": "#253f4a",
    "white": "#dfdfe0",
    "orange": "#f4a261",
    "orange-bright": "#f6b079",
    "pink": "#d67ad2",
    "pink-bright": "#dc8ed9",
    "comment": "#738091",
    "bg0": "#131a24",
    "bg1": "#192330",
    "bg2": "#212e3f",
    "bg3": "#29394f",
    "bg4": "#39506d",
    "fg0": "#d6d6d7",
    "fg1": "#cdcecf",
    "fg2": "#aeafb0",
    "fg3": "#71839b",
    "sel0": "#2b3b51",
    "sel1": "#3c5372"
  },
  "styles": {
    "ui.background": {
      "bg": "bg1"
    },
    "ui.window": {
      "fg": "bg0"
    },
    "ui.gutter": {
      "fg": "fg3"
    },
    "ui.text": {
      "fg": "fg1"
    },
    "ui.text.directory": {
      "fg": "blue-bright"
    },
    "ui.text.focus": {
      "bg": "sel1",
      "fg": "fg1"
    },
    "ui.text.info": {
      "fg": "fg2",
      "bg": "sel0"
    },
    "ui.cursor": {
      "bg": "fg3",
      "fg": "bg1"
    },
    "ui.cursor.primary": {
      "bg": "fg1",
      "fg": "bg1"
    },
    "ui.cursor.match": {
      "fg": "yellow",
      "modifiers": [
        "bold"
      ]
    },
    "ui.selection": {
      "bg": "bg4"
    },
    "ui.selection.primary": {
      "bg": "sel1"
    },
    "ui.cursorline.primary": {
      "bg": "bg3"
    },
    "ui.linenr": {
      "fg": "fg3"
    },
    "ui.linenr.selected": {
      "fg": "yellow",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual.ruler": {
      "bg": "bg3"
    },
    "ui.virtual.whitespace": {
      "fg": "bg3"
    },
    "ui.virtual.indent-guide": {
      "fg": "black"
    },
    "ui.virtual.inlay-hint": {
      "fg": "comment",
      "bg": "bg2"
    },
    "ui.virtual.jump-label": {
      "fg": "blue",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual.wrap": {
      "fg": "fg3"
    },
    "ui.statusline": {
      "fg": "fg2",
      "bg": "bg0"
    },
    "ui.statusline.inactive": {
      "fg": "fg3",
      "bg": "bg0"
    },
    "ui.statusline.normal": {
      "bg": "blue",
      "fg": "bg0",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.insert": {
      "bg": "green",
      "fg": "bg0",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.select": {
      "bg": "magenta",
      "fg": "bg0",
      "modifiers": [
        "bold"
      ]
    },
    "ui.bufferline": {
      "fg": "fg3",
      "bg": "bg2",
      "underline": {
        "style": "line"
      }
    },
    "ui.bufferline.active": {
      "fg": "fg2",
      "bg": "bg4"
    },
    "ui.bufferline.background": {
      "bg": "bg0"
    },
    "ui.help": {
      "bg": "sel0",
      "fg": "fg1"
    },
    "ui.menu": {
      "bg": "sel0",
      "fg": "fg1"
    },
    "ui.menu.selected": {
      "bg": "fg3"
    },
    "ui.menu.scroll": {
      "fg": "fg3"
    },
    "ui.popup": {
      "bg": "bg0",
      "fg": "fg1"
    },
    "ui.popup.info": {
      "bg": "sel0",
      "fg": "fg1"
    },
    "markup.raw": {
      "fg": "magenta"
    },
    "markup.raw.inline": {
      "fg": "orange"
    },
    "markup.heading": {
      "fg": "yellow",
      "modifiers": [
        "bold"
      ]
    },
    "markup.list": {
      "fg": "magenta",
      "modifiers": [
        "bold"
      ]
    },
    "markup.bold": {
      "fg": "orange",
      "modifiers": [
        "bold"
      ]
    },
    "markup.italic": {
      "fg": "pink"
    },
    "markup.strikethrough": {
      "modifiers": [
        "crossed_out"
      ]
    },
    "markup.link": {
      "fg": "yellow-bright",
      "modifiers": [
        "bold"
      ]
    },
    "markup.quote": {
      "fg": "blue"
    },
    "warning": {
      "fg": "yellow",
      "bg": "bg1"
    },
    "error": {
      "fg": "red",
      "bg": "bg1"
    },
    "info": {
      "fg": "blue",
      "bg": "bg1"
    },
    "hint": {
      "fg": "green",
      "bg": "bg1"
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
    "special": {
      "fg": "fg2"
    },
    "attribute": {
      "fg": "yellow"
    },
    "type": {
      "fg": "yellow"
    },
    "type.builtin": {
      "fg": "cyan-bright"
    },
    "type.enum.variant": {
      "fg": "orange-bright"
    },
    "constructor": {
      "fg": "magenta"
    },
    "constant": {
      "fg": "orange-bright"
    },
    "constant.builtin": {
      "fg": "orange-bright"
    },
    "constant.builtin.boolean": {
      "fg": "orange"
    },
    "constant.character": {
      "fg": "green"
    },
    "constant.character.escape": {
      "fg": "yellow-bright",
      "modifiers": [
        "bold"
      ]
    },
    "constant.numeric": {
      "fg": "orange"
    },
    "string": {
      "fg": "green"
    },
    "string.regexp": {
      "fg": "yellow-bright"
    },
    "string.special": {
      "fg": "yellow-bright",
      "modifiers": [
        "bold"
      ]
    },
    "string.special.url": {
      "fg": "cyan",
      "modifiers": [
        "bold"
      ]
    },
    "comment": {
      "fg": "comment"
    },
    "comment.block.documentation": {
      "fg": "comment",
      "modifiers": [
        "bold"
      ]
    },
    "comment.line.documentation": {
      "fg": "comment",
      "modifiers": [
        "bold"
      ]
    },
    "variable": {
      "fg": "white"
    },
    "variable.builtin": {
      "fg": "red"
    },
    "variable.parameter": {
      "fg": "cyan-bright"
    },
    "variable.other.member": {
      "fg": "fg2"
    },
    "label": {
      "fg": "magenta-bright"
    },
    "punctuation": {
      "fg": "fg2"
    },
    "keyword": {
      "fg": "magenta"
    },
    "keyword.control": {
      "fg": "pink"
    },
    "keyword.control.conditional": {
      "fg": "magenta-bright"
    },
    "keyword.control.repeat": {
      "fg": "magenta-bright"
    },
    "keyword.control.import": {
      "fg": "pink-bright"
    },
    "keyword.control.return": {
      "fg": "magenta"
    },
    "keyword.control.exception": {
      "fg": "magenta"
    },
    "keyword.operator": {
      "fg": "fg2",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.directive": {
      "fg": "pink-bright"
    },
    "keyword.function": {
      "fg": "red"
    },
    "keyword.storage": {
      "fg": "magenta"
    },
    "keyword.storage.type": {
      "fg": "magenta"
    },
    "keyword.storage.modifier": {
      "fg": "yellow"
    },
    "operator": {
      "fg": "fg2"
    },
    "function": {
      "fg": "blue-bright"
    },
    "function.builtin": {
      "fg": "red"
    },
    "function.macro": {
      "fg": "red"
    },
    "tag": {
      "fg": "blue-bright"
    },
    "namespace": {
      "fg": "cyan-bright"
    },
    "diff.plus": "green",
    "diff.minus": "red",
    "diff.delta": "blue",
    "diff.delta.moved": "cyan"
  }
});
