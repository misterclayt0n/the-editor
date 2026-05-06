import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "yo_light",
  "title": "Yo Light",
  "palette": {
    "p1": "#fcfcfc",
    "p2": "#f9f9f9",
    "p3": "#f0f0f0",
    "p4": "#e8e8e8",
    "p5": "#e0e0e0",
    "p6": "#d9d9d9",
    "p7": "#cecece",
    "p8": "#bbbbbb",
    "p9": "#8d8d8d",
    "p10": "#838383",
    "p11": "#646464",
    "p12": "#202020",
    "error": "#dc3e42",
    "warning": "#ef5f00",
    "info": "#0588f0",
    "string": "#2b9a66",
    "constant": "#8347b9",
    "keyword": "#957468"
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
      "fg": "p8",
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
      "fg": "p9"
    }
  }
});
