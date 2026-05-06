import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "yo_berry",
  "title": "Yo Berry",
  "palette": {
    "p1": "#121113",
    "p2": "#1a191b",
    "p3": "#232225",
    "p4": "#2b292d",
    "p5": "#323035",
    "p6": "#3c393f",
    "p7": "#49474e",
    "p8": "#625f69",
    "p9": "#6f6d78",
    "p10": "#7c7a85",
    "p11": "#b5b2bc",
    "p12": "#eeeef0",
    "error": "#ee518a",
    "warning": "#ffff57",
    "info": "#3b9eff",
    "string": "#0eb39e",
    "constant": "#b658c4",
    "keyword": "#9eb1ff"
  },
  "styles": {
    "ui.background": {
      "fg": "p11",
      "bg": "p2"
    },
    "ui.background.separator": {
      "fg": "p7"
    },
    "ui.text": {
      "fg": "p11"
    },
    "ui.text.focus": {
      "fg": "p12",
      "modifiers": [
        "bold"
      ]
    },
    "ui.window": {
      "fg": "p7"
    },
    "ui.popup": {
      "fg": "p12",
      "bg": "p4"
    },
    "ui.popup.info": {
      "fg": "p12",
      "bg": "p2"
    },
    "ui.text.info": {
      "fg": "p12",
      "bg": "p2"
    },
    "ui.help": {
      "fg": "p12",
      "bg": "p2"
    },
    "ui.menu": {
      "fg": "p11",
      "bg": "p4"
    },
    "ui.menu.selected": {
      "fg": "p12",
      "modifiers": [
        "bold"
      ]
    },
    "ui.menu.scroll": {
      "fg": "p8",
      "bg": "p4"
    },
    "ui.picker.header.column": {
      "underline": {
        "style": "line"
      }
    },
    "ui.cursor": {
      "fg": "p2",
      "bg": "p11"
    },
    "ui.cursor.insert": {
      "fg": "p2",
      "bg": "keyword"
    },
    "ui.cursor.select": {
      "fg": "p2",
      "bg": "p12"
    },
    "ui.cursor.match": {
      "fg": "p12",
      "modifiers": [
        "bold"
      ]
    },
    "ui.cursor.primary": {
      "fg": "p2",
      "bg": "p11",
      "modifiers": [
        "bold"
      ]
    },
    "ui.cursor.primary.insert": {
      "fg": "p2",
      "bg": "keyword",
      "modifiers": [
        "bold"
      ]
    },
    "ui.cursor.primary.select": {
      "fg": "p2",
      "bg": "p12",
      "modifiers": [
        "bold"
      ]
    },
    "ui.selection": {
      "bg": "p4"
    },
    "ui.selection.primary": {
      "bg": "p5"
    },
    "ui.cursorline.primary": {
      "bg": "p3"
    },
    "ui.cursorcolumn.primary": {
      "bg": "p3"
    },
    "ui.linenr": {
      "fg": "p7"
    },
    "ui.linenr.selected": {
      "fg": "p11"
    },
    "diff": {
      "fg": "p8"
    },
    "ui.bufferline": {
      "fg": "p11",
      "bg": "p4"
    },
    "ui.bufferline.active": {
      "fg": "p2",
      "bg": "p11"
    },
    "ui.statusline": {
      "fg": "p11",
      "bg": "p4"
    },
    "ui.statusline.inactive": {
      "fg": "p11",
      "bg": "p2"
    },
    "ui.statusline.normal": {
      "fg": "p2",
      "bg": "p11"
    },
    "ui.statusline.insert": {
      "fg": "p2",
      "bg": "keyword"
    },
    "ui.statusline.select": {
      "fg": "p2",
      "bg": "p12"
    },
    "ui.statusline.separator": {
      "fg": "p7"
    },
    "ui.virtual": {
      "fg": "p6"
    },
    "ui.virtual.ruler": {
      "bg": "p3"
    },
    "ui.virtual.inlay-hint": {
      "fg": "p7",
      "underline": {
        "style": "dotted"
      }
    },
    "ui.virtual.jump-label": {
      "fg": "p12",
      "modifiers": [
        "bold"
      ],
      "underline": {
        "style": "curl",
        "color": "info"
      }
    },
    "error": {
      "fg": "error",
      "modifiers": [
        "bold"
      ]
    },
    "warning": {
      "fg": "warning",
      "modifiers": [
        "bold"
      ]
    },
    "info": {
      "fg": "info",
      "modifiers": [
        "bold"
      ]
    },
    "hint": {
      "fg": "info",
      "modifiers": [
        "bold"
      ]
    },
    "diagnostic.error": {
      "fg": "error",
      "modifiers": [
        "bold"
      ]
    },
    "diagnostic.warning": {
      "fg": "warning",
      "modifiers": [
        "bold"
      ]
    },
    "diagnostic.info": {
      "fg": "info",
      "modifiers": [
        "bold"
      ]
    },
    "diagnostic.hint": {
      "fg": "info",
      "modifiers": [
        "bold"
      ]
    },
    "comment": {
      "fg": "info"
    },
    "keyword": {
      "fg": "keyword"
    },
    "operator": {
      "fg": "keyword"
    },
    "string": {
      "fg": "string"
    },
    "constant": {
      "fg": "constant"
    },
    "string.special.symbol": {
      "fg": "constant"
    },
    "variable": {
      "fg": "p10"
    },
    "namespace": {
      "fg": "p10"
    },
    "punctuation": {
      "fg": "p9"
    },
    "punctuation.delimiter": {
      "fg": "p8"
    },
    "function": {
      "fg": "p11"
    },
    "attribute": {
      "fg": "p10"
    },
    "tag": {
      "fg": "keyword"
    },
    "label": {
      "fg": "p12"
    },
    "constructor": {
      "fg": "p12"
    },
    "type": {
      "fg": "p12"
    },
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
    "markup.heading": {
      "fg": "p12",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.marker": {
      "fg": "p8"
    },
    "markup.list": {
      "fg": "p8"
    },
    "markup.link.url": {
      "underline": {
        "style": "line"
      }
    },
    "markup.link.label": {
      "underline": {
        "style": "dotted"
      }
    },
    "markup.quote": {
      "fg": "p10"
    }
  }
});
