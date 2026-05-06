import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "molokai",
  "title": "Molokai",
  "palette": {
    "type": "#A6E22E",
    "keyword": "#F92672",
    "regex": "#CE9178",
    "special": "#C586C0",
    "variable": "#F8F8F2",
    "fn_declaration": "#A6E22E",
    "background": "#1b1d1e",
    "text": "#f8f8f2",
    "active_text": "#ffffff",
    "cursor": "#a6a6a6",
    "inactive_cursor": "#878b91",
    "widget": "#1e1f1c",
    "selection": "#414339",
    "tag": "#F92672",
    "light-blue": "#66d9ef",
    "comment": "#7e8e91"
  },
  "styles": {
    "tag": {
      "fg": "tag"
    },
    "namespace": {
      "fg": "type"
    },
    "module": {
      "fg": "type"
    },
    "type": "light-blue",
    "type.builtin": {
      "fg": "#66D9EF"
    },
    "type.enum.variant": {
      "fg": "text"
    },
    "constructor": {
      "fg": "fn_declaration"
    },
    "variable.other.member": {
      "fg": "variable"
    },
    "keyword": {
      "fg": "keyword",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.directive": {
      "fg": "fn_declaration",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.control": {
      "fg": "keyword",
      "modifiers": [
        "bold"
      ]
    },
    "label": "#e6db74",
    "special": {
      "fg": "keyword"
    },
    "operator": "keyword",
    "punctuation": {
      "fg": "text"
    },
    "punctuation.delimiter": "#8f8f8f",
    "variable": {
      "fg": "variable"
    },
    "variable.parameter": "variable",
    "variable.builtin": {
      "fg": "#ae81ff",
      "modifiers": [
        "bold"
      ]
    },
    "constant": {
      "fg": "variable"
    },
    "constant.builtin": {
      "fg": "#ae81ff"
    },
    "function": {
      "fg": "fn_declaration"
    },
    "function.builtin": {
      "fg": "fn_declaration"
    },
    "function.macro": {
      "fg": "#c4be89",
      "modifiers": [
        "italic"
      ]
    },
    "attribute": {
      "fg": "fn_declaration"
    },
    "comment": "comment",
    "ui.virtual.whitespace": "#88846F",
    "ui.virtual.ruler": {
      "bg": "#24241e"
    },
    "ui.virtual.jump-label": {
      "fg": "special",
      "modifiers": [
        "bold"
      ]
    },
    "string": {
      "fg": "#e6db74"
    },
    "constant.character": {
      "fg": "#e6db74"
    },
    "string.regexp": {
      "fg": "regex"
    },
    "constant.numeric": {
      "fg": "#ae81ff"
    },
    "constant.character.escape": {
      "fg": "light-blue",
      "modifiers": [
        "italic"
      ]
    },
    "markup.heading": "blue",
    "markup.list": "red",
    "markup.bold": {
      "fg": "yellow",
      "modifiers": [
        "bold"
      ]
    },
    "markup.italic": {
      "fg": "magenta",
      "modifiers": [
        "italic"
      ]
    },
    "markup.strikethrough": {
      "modifiers": [
        "crossed_out"
      ]
    },
    "markup.link.url": {
      "fg": "yellow",
      "modifiers": [
        "underlined"
      ]
    },
    "markup.link.text": "red",
    "markup.quote": "cyan",
    "markup.raw": "green",
    "diff.plus": "text",
    "diff.delta": "#89807d",
    "diff.minus": "#960050",
    "ui.background": {
      "fg": "text",
      "bg": "background"
    },
    "ui.window": {
      "bg": "widget"
    },
    "ui.popup": {
      "fg": "light-blue",
      "bg": "#000000"
    },
    "ui.help": {
      "fg": "light-blue",
      "bg": "#000000"
    },
    "ui.menu": {
      "fg": "light-blue",
      "bg": "#000000"
    },
    "ui.menu.selected": {
      "bg": "#414339"
    },
    "ui.cursor": {
      "fg": "cursor",
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.primary": {
      "fg": "#000000",
      "bg": "#f8f8f0"
    },
    "ui.cursor.match": {
      "fg": "#000000",
      "bg": "#fd971f",
      "modifiers": [
        "bold"
      ]
    },
    "ui.selection": {
      "bg": "#403d3d"
    },
    "ui.selection.primary": {
      "bg": "#575b61"
    },
    "ui.linenr": "#465457",
    "ui.linenr.selected": "#fd971f",
    "ui.cursorline": {
      "bg": "#24241e"
    },
    "ui.statusline": {
      "fg": "active_text",
      "bg": "#414339"
    },
    "ui.statusline.inactive": {
      "fg": "active_text",
      "bg": "#75715e"
    },
    "ui.bufferline.active": {
      "fg": "active_text",
      "bg": "selection",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual.inlay-hint": {
      "fg": "#88846F"
    },
    "ui.text": {
      "fg": "text"
    },
    "ui.text.focus": {
      "fg": "active_text"
    },
    "warning": {
      "fg": "#ffffff",
      "modifiers": [
        "bold"
      ]
    },
    "error": {
      "fg": "#ffffff",
      "modifiers": [
        "bold"
      ]
    },
    "info": {
      "fg": "#ffffff",
      "modifiers": [
        "bold"
      ]
    },
    "hint": {
      "fg": "#ffffff",
      "modifiers": [
        "bold"
      ]
    },
    "diagnostic.warning": {
      "underline": {
        "color": "#cca700",
        "style": "curl"
      }
    },
    "diagnostic.error": {
      "underline": {
        "color": "#f48771",
        "style": "curl"
      }
    },
    "diagnostic.info": {
      "underline": {
        "color": "#75beff",
        "style": "curl"
      }
    },
    "diagnostic.hint": {
      "underline": {
        "color": "#eeeeb3",
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
    "keyword.control.exception": {
      "fg": "fn_declaration",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.storage.modifier": {
      "fg": "#fd971f",
      "modifiers": [
        "italic"
      ]
    },
    "tag.builtin": {
      "fg": "#ae81ff",
      "modifiers": [
        "bold"
      ]
    },
    "ui.popup.info": {
      "fg": "light-blue",
      "bg": "#000000"
    },
    "ui.text.info": {
      "fg": "light-blue",
      "bg": "#000000"
    }
  }
});
