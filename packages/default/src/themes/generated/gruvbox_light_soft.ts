import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "gruvbox_light_soft",
  "title": "Gruvbox Light Soft",
  "palette": {
    "bg0": "#f2e5bc",
    "bg0_s": "#32302f",
    "bg1": "#ebdbb2",
    "bg2": "#d5c4a1",
    "bg3": "#bdae93",
    "bg4": "#a89984",
    "fg0": "#282828",
    "fg1": "#3c3836",
    "fg2": "#504945",
    "fg3": "#665c54",
    "fg4": "#7c6f64",
    "gray": "#928374",
    "red0": "#cc241d",
    "red1": "#9d0006",
    "green0": "#98971a",
    "green1": "#79740e",
    "yellow0": "#d79921",
    "yellow1": "#b57614",
    "blue0": "#458588",
    "blue1": "#076678",
    "purple0": "#b16286",
    "purple1": "#8f3f71",
    "aqua0": "#689d6a",
    "aqua1": "#427b58",
    "orange0": "#d65d0e",
    "orange1": "#af3a03",
    "gray0": "#7c6f64"
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
      "underline": {
        "color": "red1",
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
    "diagnostic.warning": {
      "underline": {
        "color": "yellow1",
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
    "diff.delta": {
      "fg": "yellow1"
    },
    "diff.minus": {
      "fg": "red1"
    },
    "diff.plus": {
      "fg": "green1"
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
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading": "aqua1",
    "markup.italic": {
      "modifiers": [
        "italic"
      ]
    },
    "markup.link.text": "red1",
    "markup.link.url": {
      "fg": "green1",
      "modifiers": [
        "underlined"
      ]
    },
    "markup.raw": "red1",
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
      "bg": "bg2"
    },
    "ui.cursor.primary": {
      "modifiers": [
        "reversed"
      ]
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
      "bg": "bg1"
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
      "fg": "bg2",
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
      "bg": "bg2"
    },
    "ui.selection.primary": {
      "bg": "bg3"
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
        "bold"
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
    "ui.text.focus": {
      "fg": "green1"
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
        "italic"
      ]
    }
  }
});
