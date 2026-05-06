import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "carbonfox",
  "title": "Carbonfox",
  "palette": {
    "black": "#282828",
    "red": "#ee5396",
    "red-dim": "#ca4780",
    "green": "#25be6a",
    "green-dim": "#1fa25a",
    "yellow": "#08bdba",
    "yellow-bright": "#2dc7c4",
    "blue": "#78a9ff",
    "blue-bright": "#8cb6ff",
    "blue-dim": "#6690d9",
    "magenta": "#be95ff",
    "magenta-bright": "#c8a5ff",
    "cyan": "#33b1ff",
    "cyan-bright": "#52bdff",
    "cyan-dim": "#2b96d9",
    "white": "#dfdfe0",
    "orange": "#3ddbd9",
    "orange-bright": "#5ae0df",
    "pink": "#ff7eb6",
    "pink-bright": "#ff91c1",
    "comment": "#738091",
    "bg0": "#0c0c0c",
    "bg1": "#161616",
    "bg2": "#252525",
    "bg3": "#353535",
    "bg4": "#535353",
    "fg0": "#f9fbff",
    "fg1": "#f2f4f8",
    "fg2": "#b6b8bb",
    "fg3": "#7b7c7e",
    "sel0": "#2a2a2a",
    "sel1": "#525253"
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
      "fg": "magenta",
      "bg": "#2f2939"
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
      "fg": "orange",
      "bg": "#1c3433"
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
    "diff.delta.moved": "cyan",
    "error.bg": "#361f29",
    "info.bg": "#252c39"
  }
});
