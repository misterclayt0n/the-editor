import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "flexoki_dark",
  "title": "Flexoki Dark",
  "palette": {
    "tx": "#CECDC3",
    "tx-2": "#878580",
    "tx-3": "#575653",
    "ui-3": "#403E3C",
    "ui-2": "#343331",
    "ui": "#282726",
    "bg-2": "#1C1B1A",
    "bg": "#100F0F",
    "re": "#D14D41",
    "or": "#DA702C",
    "ye": "#D0A215",
    "gr": "#879A39",
    "cy": "#3AA99F",
    "bl": "#4385BE",
    "pu": "#8B7EC8",
    "ma": "#CE5D97"
  },
  "styles": {
    "ui.background": {
      "bg": "bg"
    },
    "ui.cursor": {
      "fg": "tx",
      "bg": "tx-3"
    },
    "ui.cursor.primary": {
      "fg": "bg",
      "bg": "tx"
    },
    "ui.cursor.match": {
      "modifiers": [
        "bold"
      ]
    },
    "ui.linenr": "tx-3",
    "ui.linenr.selected": "tx",
    "ui.selection": {
      "bg": "ui-2"
    },
    "ui.statusline": {
      "fg": "tx",
      "bg": "bg-2"
    },
    "ui.statusline.normal": {
      "fg": "bg",
      "bg": "bl"
    },
    "ui.statusline.insert": {
      "fg": "bg",
      "bg": "or"
    },
    "ui.statusline.select": {
      "fg": "bg",
      "bg": "ma"
    },
    "ui.cursorline": {
      "bg": "bg-2"
    },
    "ui.popup": {
      "fg": "tx",
      "bg": "bg-2"
    },
    "ui.window": "tx",
    "ui.help": {
      "fg": "tx",
      "bg": "bg"
    },
    "ui.text": "tx",
    "ui.text.focus": {
      "bg": "bg-2",
      "fg": "tx"
    },
    "ui.text.info": "tx",
    "ui.virtual.whitespace": "bg-2",
    "ui.virtual.ruler": {
      "bg": "bg-2"
    },
    "ui.virtual.inlay-hint": {
      "fg": "tx-3",
      "bg": "bg"
    },
    "ui.virtual.jump-label": {
      "bg": "bg-2",
      "modifiers": [
        "bold"
      ]
    },
    "ui.menu": {
      "fg": "tx",
      "bg": "bg"
    },
    "ui.menu.selected": {
      "bg": "ui",
      "fg": "tx"
    },
    "ui.debug": {
      "fg": "or",
      "bg": "bg"
    },
    "ui.highlight.frameline": {
      "bg": "ye"
    },
    "ui.bufferline": {
      "fg": "tx-2",
      "bg": "bg-2"
    },
    "ui.bufferline.active": {
      "fg": "ye",
      "bg": "bg-2"
    },
    "diagnostic.hint": {
      "underline": {
        "color": "bl",
        "style": "curl"
      }
    },
    "diagnostic.info": {
      "underline": {
        "color": "bl",
        "style": "curl"
      }
    },
    "diagnostic.warning": {
      "underline": {
        "color": "ye",
        "style": "curl"
      }
    },
    "diagnostic.error": {
      "underline": {
        "color": "re",
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
    "hint": {
      "fg": "bl",
      "modifiers": [
        "bold"
      ]
    },
    "info": {
      "fg": "ye",
      "modifiers": [
        "bold"
      ]
    },
    "warning": {
      "fg": "or",
      "modifiers": [
        "bold"
      ]
    },
    "error": {
      "fg": "re",
      "modifiers": [
        "bold"
      ]
    },
    "attribute": "ye",
    "type": "ye",
    "constructor": "gr",
    "constant": "pu",
    "constant.builtin": "ye",
    "string": "cy",
    "string.regexp": "or",
    "string.special": "ye",
    "comment": "tx-3",
    "variable": "tx",
    "variable.builtin": "ma",
    "variable.other": "bl",
    "punctuation": "tx-2",
    "keyword": "gr",
    "keyword.control": "re",
    "keyword.control.conditional": "gr",
    "keyword.control.import": "ye",
    "keyword.control.return": "gr",
    "keyword.function": "gr",
    "keyword.storage.type": "bl",
    "keyword.storage.modifier": "bl",
    "operator": "tx-2",
    "function": "or",
    "tag": "bl",
    "namespace": "re",
    "markup.heading": "or",
    "markup.list": "ye",
    "markup.bold": {
      "fg": "or",
      "modifiers": [
        "bold"
      ]
    },
    "markup.italic": {
      "fg": "or",
      "modifiers": [
        "italic"
      ]
    },
    "markup.strikethrough": {
      "modifiers": [
        "crossed_out"
      ]
    },
    "markup.raw.block": "or",
    "markup.link.url": "bl",
    "markup.link.text": "ye",
    "markup.link.label": "gr",
    "markup.quote": "ye",
    "markup.raw": "bl",
    "diff.plus": "gr",
    "diff.minus": "re",
    "diff.delta": "ye"
  }
});
