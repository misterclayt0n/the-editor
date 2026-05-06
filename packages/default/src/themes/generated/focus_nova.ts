import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "focus_nova",
  "title": "Focus Nova",
  "palette": {
    "bg0": "#1e1e2e",
    "bg0_s": "#1c1c2a",
    "bg1": "#2e2e3e",
    "bg2": "#3b3b4d",
    "bg3": "#4b4b5e",
    "bg4": "#5a5a72",
    "fg0": "#f4f4f5",
    "fg1": "#dcdcdc",
    "fg2": "#c0c0c0",
    "fg3": "#a8a8a8",
    "fg4": "#909090",
    "gray": "#7a7a89",
    "red0": "#f7768e",
    "red1": "#ff6e7f",
    "green0": "#9ece6a",
    "green1": "#c0fca0",
    "yellow0": "#e0af68",
    "yellow1": "#f9e2af",
    "blue0": "#7aa2f7",
    "blue1": "#89b4fa",
    "purple0": "#bb9af7",
    "purple1": "#cba6f7",
    "aqua0": "#7dcfff",
    "aqua1": "#94e2d5",
    "orange0": "#fab387",
    "orange1": "#fca86f"
  },
  "styles": {
    "annotation": {
      "fg": "fg1"
    },
    "attribute": {
      "fg": "aqua1",
      "modifiers": [
        "italic"
      ]
    },
    "comment": {
      "fg": "gray",
      "modifiers": [
        "italic"
      ]
    },
    "constant": {
      "fg": "purple1"
    },
    "constant.character": {
      "fg": "aqua1"
    },
    "constant.character.escape": {
      "fg": "orange1"
    },
    "constant.macro": {
      "fg": "aqua1"
    },
    "constructor": {
      "fg": "purple1"
    },
    "definition": {
      "underline": {
        "color": "aqua1"
      }
    },
    "diagnostic": {
      "underline": {
        "color": "orange1",
        "style": "curl"
      }
    },
    "diagnostic.deprecated": {
      "modifiers": [
        "crossed_out"
      ]
    },
    "diagnostic.error": {
      "fg": "red1",
      "underline": {
        "color": "red1",
        "style": "curl"
      }
    },
    "diagnostic.warning": {
      "fg": "yellow1",
      "underline": {
        "color": "yellow1",
        "style": "curl"
      }
    },
    "diagnostic.hint": {
      "underline": {
        "color": "blue1",
        "style": "curl"
      }
    },
    "diagnostic.info": {
      "underline": {
        "color": "aqua1",
        "style": "curl"
      }
    },
    "error": {
      "fg": "red1"
    },
    "hint": {
      "fg": "blue1"
    },
    "info": {
      "fg": "aqua1"
    },
    "warning": {
      "fg": "yellow1"
    },
    "diff.plus": {
      "fg": "green1",
      "bg": "bg2"
    },
    "diff.minus": {
      "fg": "red1",
      "bg": "bg2"
    },
    "diff.delta": {
      "fg": "yellow1",
      "bg": "bg2"
    },
    "function": {
      "fg": "green1"
    },
    "function.builtin": {
      "fg": "yellow1"
    },
    "function.macro": {
      "fg": "blue1"
    },
    "keyword": {
      "fg": "red1"
    },
    "keyword.control.import": {
      "fg": "aqua1"
    },
    "label": {
      "fg": "red1"
    },
    "markup.bold": {
      "fg": "blue1",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading": {
      "fg": "purple1",
      "modifiers": [
        "bold"
      ]
    },
    "markup.italic": {
      "fg": "yellow1",
      "modifiers": [
        "italic"
      ]
    },
    "markup.link.label": {
      "fg": "blue1",
      "modifiers": [
        "italic"
      ]
    },
    "markup.link.text": "purple0",
    "markup.link.url": "aqua1",
    "markup.list": "aqua1",
    "markup.quote": {
      "fg": "yellow1",
      "modifiers": [
        "italic"
      ]
    },
    "markup.raw": {
      "fg": "fg1"
    },
    "markup.strikethrough": {
      "modifiers": [
        "crossed_out"
      ]
    },
    "module": {
      "fg": "aqua1"
    },
    "namespace": {
      "fg": "fg1"
    },
    "operator": {
      "fg": "purple1"
    },
    "punctuation": {
      "fg": "orange1"
    },
    "special": {
      "fg": "purple0"
    },
    "string": {
      "fg": "green1"
    },
    "string.regexp": {
      "fg": "orange1"
    },
    "string.special": {
      "fg": "orange1"
    },
    "string.symbol": {
      "fg": "yellow1"
    },
    "tag": {
      "fg": "aqua1"
    },
    "type": {
      "fg": "yellow1"
    },
    "type.enum.variant": {
      "modifiers": [
        "italic"
      ]
    },
    "ui.background": {
      "bg": "bg0"
    },
    "ui.bufferline": {
      "fg": "fg1",
      "bg": "bg1"
    },
    "ui.bufferline.active": {
      "fg": "bg0",
      "bg": "yellow0"
    },
    "ui.bufferline.background": {
      "bg": "bg2"
    },
    "ui.cursor": {
      "fg": "bg1",
      "bg": "bg2"
    },
    "ui.cursor.insert": {
      "fg": "bg1",
      "bg": "blue0"
    },
    "ui.cursor.normal": {
      "fg": "bg1",
      "bg": "gray"
    },
    "ui.cursor.select": {
      "fg": "bg1",
      "bg": "orange0"
    },
    "ui.cursor.match": {
      "fg": "fg1",
      "bg": "bg2",
      "modifiers": [
        "bold"
      ]
    },
    "ui.cursor.primary": {
      "bg": "fg3",
      "fg": "bg1"
    },
    "ui.cursor.primary.insert": {
      "fg": "bg1",
      "bg": "blue1"
    },
    "ui.cursor.primary.normal": {
      "fg": "bg1",
      "bg": "fg3"
    },
    "ui.cursor.primary.select": {
      "fg": "bg1",
      "bg": "orange1"
    },
    "ui.cursorline": {
      "bg": "bg0_s"
    },
    "ui.cursorline.primary": {
      "bg": "bg1"
    },
    "ui.help": {
      "bg": "bg1",
      "fg": "fg1"
    },
    "ui.linenr": {
      "fg": "bg3"
    },
    "ui.linenr.selected": {
      "fg": "yellow1"
    },
    "ui.menu": {
      "fg": "fg1",
      "bg": "bg2"
    },
    "ui.menu.selected": {
      "fg": "bg1",
      "bg": "blue1",
      "modifiers": [
        "bold"
      ]
    },
    "ui.popup": {
      "bg": "bg1"
    },
    "ui.picker.header.column": {
      "underline": {
        "style": "line"
      }
    },
    "ui.picker.header.column.active": {
      "modifiers": [
        "bold"
      ],
      "underline": {
        "style": "line"
      }
    },
    "ui.selection": {
      "bg": "bg3"
    },
    "ui.selection.primary": {
      "bg": "bg4"
    },
    "ui.statusline": {
      "fg": "fg1",
      "bg": "bg2"
    },
    "ui.statusline.inactive": {
      "fg": "fg4",
      "bg": "bg2"
    },
    "ui.statusline.insert": {
      "fg": "bg1",
      "bg": "blue1",
      "modifiers": [
        "bold",
        "italic"
      ]
    },
    "ui.statusline.normal": {
      "fg": "bg1",
      "bg": "fg3",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.select": {
      "fg": "bg1",
      "bg": "orange1",
      "modifiers": [
        "bold"
      ]
    },
    "ui.text": {
      "fg": "fg1"
    },
    "ui.text.directory": {
      "fg": "blue1"
    },
    "ui.virtual.inlay-hint": {
      "fg": "gray"
    },
    "ui.virtual.jump-label": {
      "fg": "purple0",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual.ruler": {
      "bg": "bg1"
    },
    "ui.virtual.whitespace": "bg2",
    "ui.virtual.wrap": {
      "fg": "bg2"
    },
    "ui.window": {
      "bg": "bg1"
    },
    "variable": {
      "fg": "fg1"
    },
    "variable.builtin": {
      "fg": "orange1",
      "modifiers": [
        "italic"
      ]
    },
    "variable.other.member": {
      "fg": "blue1"
    },
    "variable.parameter": {
      "fg": "blue1",
      "modifiers": [
        "italic",
        "bold"
      ]
    }
  }
});
