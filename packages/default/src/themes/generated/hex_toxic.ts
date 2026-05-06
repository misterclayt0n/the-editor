import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "hex_toxic",
  "title": "Hex Toxic",
  "palette": {
    "t1": "#101719",
    "t2": "#1b2a32",
    "t3": "#4b5968",
    "t4": "#8792ab",
    "t5": "#6f91bc",
    "t6": "#8bb2b9",
    "t7": "#eeac90",
    "t8": "#b0bd9f",
    "t9": "#b3ccd0",
    "t10": "#b0d4d8",
    "t11": "#ffbf52",
    "highlight": "#ff0a50",
    "highlight_two": "#0affa9",
    "highlight_three": "#f8ed8b",
    "selection": "#b10656",
    "selection_fg": "#101719",
    "black": "#000000",
    "comment": "#417e8c",
    "comment_doc": "#234048",
    "hints": "#39515c",
    "ruler": "#1e3039",
    "error": "#ff0900",
    "warning": "#ffbf00",
    "display": "#57ff89",
    "info": "#dad7d5",
    "diff_minus": "#ff4000",
    "diff_delta": "#0078bd",
    "diff_plus": "#c9d400",
    "diff_delta_moved": "#0048bd"
  },
  "styles": {
    "comment": {
      "fg": "comment"
    },
    "comment.block.documentation": {
      "bg": "comment_doc",
      "modifiers": [
        "italic"
      ]
    },
    "comment.line.documentation": {
      "bg": "comment_doc",
      "modifiers": [
        "italic"
      ]
    },
    "constant": {
      "fg": "t11"
    },
    "function": {
      "fg": "t10"
    },
    "function.method": {
      "fg": "t7"
    },
    "function.macro": {
      "fg": "t7"
    },
    "keyword.storage.modifier": {
      "fg": "t7"
    },
    "keyword.control.import": {
      "fg": "t8"
    },
    "keyword.control": {
      "fg": "t8"
    },
    "keyword.function": {
      "fg": "t11"
    },
    "keyword": {
      "fg": "t6"
    },
    "operator": {
      "fg": "t8"
    },
    "punctuation": {
      "fg": "t9"
    },
    "string": {
      "fg": "t6",
      "modifiers": [
        "italic"
      ]
    },
    "string.regexp": {
      "fg": "t6"
    },
    "tag": {
      "fg": "t4"
    },
    "type": {
      "fg": "t8",
      "modifiers": [
        "bold"
      ]
    },
    "namespace": {
      "fg": "t6",
      "modifiers": [
        "bold"
      ]
    },
    "variable": {
      "fg": "t4"
    },
    "variable.parameter": {
      "fg": "t6"
    },
    "variable.other.member": {
      "fg": "t3"
    },
    "label": {
      "fg": "t4"
    },
    "diff.plus": {
      "fg": "diff_plus"
    },
    "diff.delta": {
      "fg": "diff_delta"
    },
    "diff.delta.moved": {
      "fg": "diff_delta_moved"
    },
    "diff.minus": {
      "fg": "diff_minus"
    },
    "ui.cursor.primary.insert": {
      "fg": "t2",
      "bg": "highlight"
    },
    "ui.cursor.primary.select": {
      "fg": "t2",
      "bg": "highlight_two"
    },
    "ui.cursor.primary": {
      "fg": "t1",
      "bg": "highlight_three"
    },
    "ui.cursor.match": {
      "fg": "highlight",
      "bg": "t1",
      "modifiers": [
        "bold"
      ]
    },
    "ui.cursorline.primary": {
      "bg": "ruler"
    },
    "ui.cursorline.secondary": {
      "bg": "ruler"
    },
    "ui.linenr": {
      "fg": "t3",
      "bg": "t2"
    },
    "ui.linenr.selected": {
      "fg": "highlight_three",
      "bg": "t2"
    },
    "ui.gutter": {
      "bg": "t2"
    },
    "ui.background": {
      "fg": "t4",
      "bg": "t2"
    },
    "ui.background.separator": {
      "fg": "t3"
    },
    "ui.help": {
      "fg": "t4",
      "bg": "t1"
    },
    "ui.menu": {
      "fg": "t4",
      "bg": "t1"
    },
    "ui.menu.selected": {
      "fg": "highlight_three",
      "bg": "t1"
    },
    "ui.popup": {
      "fg": "t4",
      "bg": "t1"
    },
    "ui.window": {
      "fg": "t4"
    },
    "ui.selection": {
      "fg": "selection_fg",
      "bg": "selection"
    },
    "ui.statusline": {
      "fg": "t4",
      "bg": "t1"
    },
    "ui.statusline.inactive": {
      "fg": "t4",
      "bg": "t1"
    },
    "ui.statusline.normal": {
      "fg": "t3",
      "bg": "t1"
    },
    "ui.statusline.insert": {
      "fg": "t3",
      "bg": "t1"
    },
    "ui.statusline.select": {
      "fg": "highlight",
      "bg": "t4"
    },
    "ui.text": {
      "fg": "t4"
    },
    "ui.text.focus": {
      "fg": "highlight_three",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual.ruler": {
      "bg": "ruler"
    },
    "ui.virtual.indent-guide": {
      "fg": "t3"
    },
    "ui.virtual.whitespace": {
      "fg": "t3"
    },
    "ui.virtual.jump-label": {
      "fg": "t11",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual.inlay-hint": {
      "fg": "hints",
      "modifiers": [
        "bold"
      ]
    },
    "ui.bufferline": {
      "fg": "t3",
      "bg": "t1"
    },
    "ui.bufferline.active": {
      "fg": "t7",
      "bg": "t2"
    },
    "diagnostic.error": {
      "underline": {
        "color": "error",
        "style": "curl"
      }
    },
    "diagnostic.warning": {
      "underline": {
        "color": "warning",
        "style": "curl"
      }
    },
    "diagnostic.info": {
      "underline": {
        "color": "info",
        "style": "curl"
      }
    },
    "diagnostic.hint": {
      "underline": {
        "color": "display",
        "style": "curl"
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
      "fg": "display",
      "modifiers": [
        "bold"
      ]
    },
    "special": {
      "fg": "t7",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading": {
      "fg": "t7"
    },
    "markup.list": {
      "fg": "t7"
    },
    "markup.bold": {
      "fg": "t4"
    },
    "markup.italic": {
      "fg": "t4"
    },
    "markup.strikethrough": {
      "fg": "t4",
      "modifiers": [
        "crossed_out"
      ]
    },
    "markup.link.url": {
      "fg": "t11",
      "modifiers": [
        "underlined"
      ]
    },
    "markup.link.text": {
      "fg": "t11"
    },
    "markup.quote": {
      "fg": "t5"
    },
    "markup.raw": {
      "fg": "t4"
    }
  }
});
