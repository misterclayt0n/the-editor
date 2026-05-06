import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "tokyonight_day",
  "title": "Tokyonight Day",
  "palette": {
    "red": "#f52a65",
    "orange": "#b15c00",
    "yellow": "#8c6c3e",
    "light-green": "#587539",
    "green": "#387068",
    "aqua": "#188092",
    "teal": "#118c74",
    "turquoise": "#006a83",
    "light-cyan": "#2e5857",
    "cyan": "#007197",
    "blue": "#2e7de9",
    "purple": "#7847bd",
    "magenta": "#9854f1",
    "comment": "#848cb5",
    "black": "#a1a6c5",
    "add": "#aecde6",
    "change": "#d6d8e3",
    "delete": "#dfccd4",
    "error": "#c64343",
    "info": "#07879d",
    "hint": "#118c74",
    "fg": "#3760bf",
    "fg-dark": "#6172b0",
    "fg-gutter": "#a8aecb",
    "fg-linenr": "#68709a",
    "fg-selected": "#b3b8d1",
    "border": "#e9e9ed",
    "border-highlight": "#2496ac",
    "bg": "#e1e2e7",
    "bg-inlay": "#acd7eb",
    "bg-selection": "#c4c8da",
    "bg-menu": "#e9e9ec",
    "bg-focus": "#b6bfe2"
  },
  "styles": {
    "attribute": {
      "fg": "cyan"
    },
    "comment": {
      "fg": "comment",
      "modifiers": [
        "italic"
      ]
    },
    "comment.block.documentation": {
      "fg": "yellow"
    },
    "comment.line.documentation": {
      "fg": "yellow"
    },
    "constant": {
      "fg": "orange"
    },
    "constant.builtin": {
      "fg": "aqua"
    },
    "constant.character": {
      "fg": "light-green"
    },
    "constant.character.escape": {
      "fg": "magenta"
    },
    "constructor": {
      "fg": "aqua"
    },
    "function": {
      "fg": "blue",
      "modifiers": [
        "italic"
      ]
    },
    "function.builtin": {
      "fg": "aqua"
    },
    "function.macro": {
      "fg": "cyan"
    },
    "function.special": {
      "fg": "cyan"
    },
    "keyword": {
      "fg": "purple",
      "modifiers": [
        "italic"
      ]
    },
    "keyword.control": {
      "fg": "magenta"
    },
    "keyword.control.import": {
      "fg": "cyan"
    },
    "keyword.control.return": {
      "fg": "purple",
      "modifiers": [
        "italic"
      ]
    },
    "keyword.directive": {
      "fg": "cyan"
    },
    "keyword.function": {
      "fg": "magenta"
    },
    "keyword.operator": {
      "fg": "magenta"
    },
    "label": {
      "fg": "blue"
    },
    "namespace": {
      "fg": "cyan"
    },
    "operator": {
      "fg": "turquoise"
    },
    "punctuation": {
      "fg": "turquoise"
    },
    "special": {
      "fg": "aqua"
    },
    "string": {
      "fg": "light-green"
    },
    "string.regexp": {
      "fg": "light-cyan"
    },
    "string.special": {
      "fg": "aqua"
    },
    "tag": {
      "fg": "magenta"
    },
    "type": {
      "fg": "aqua"
    },
    "type.builtin": {
      "fg": "aqua"
    },
    "type.enum.variant": {
      "fg": "orange"
    },
    "variable": {
      "fg": "fg"
    },
    "variable.builtin": {
      "fg": "red"
    },
    "variable.other.member": {
      "fg": "green"
    },
    "variable.parameter": {
      "fg": "yellow",
      "modifiers": [
        "italic"
      ]
    },
    "markup.bold": {
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading": {
      "fg": "blue",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.completion": {
      "bg": "bg-menu",
      "fg": "fg"
    },
    "markup.heading.hover": {
      "bg": "fg-selected"
    },
    "markup.italic": {
      "modifiers": [
        "italic"
      ]
    },
    "markup.link": {
      "fg": "blue",
      "underline": {
        "style": "line"
      }
    },
    "markup.link.label": {
      "fg": "teal"
    },
    "markup.link.text": {
      "fg": "teal"
    },
    "markup.link.url": {
      "underline": {
        "style": "line"
      }
    },
    "markup.list": {
      "fg": "orange",
      "modifiers": [
        "bold"
      ]
    },
    "markup.normal.completion": {
      "fg": "comment"
    },
    "markup.normal.hover": {
      "fg": "fg-dark"
    },
    "markup.raw": {
      "fg": "teal"
    },
    "markup.raw.inline": {
      "bg": "black",
      "fg": "blue"
    },
    "markup.strikethrough": {
      "modifiers": [
        "crossed_out"
      ]
    },
    "diff.delta": {
      "fg": "change"
    },
    "diff.delta.moved": {
      "fg": "blue"
    },
    "diff.minus": {
      "fg": "delete"
    },
    "diff.plus": {
      "fg": "add"
    },
    "error": {
      "fg": "error"
    },
    "warning": {
      "fg": "yellow"
    },
    "info": {
      "fg": "info"
    },
    "hint": {
      "fg": "hint"
    },
    "diagnostic.error": {
      "underline": {
        "style": "curl",
        "color": "error"
      }
    },
    "diagnostic.warning": {
      "underline": {
        "style": "curl",
        "color": "yellow"
      }
    },
    "diagnostic.info": {
      "underline": {
        "style": "curl",
        "color": "info"
      }
    },
    "diagnostic.hint": {
      "underline": {
        "style": "curl",
        "color": "hint"
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
    "ui.background": {
      "bg": "bg",
      "fg": "fg"
    },
    "ui.cursor": {
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.match": {
      "fg": "orange",
      "modifiers": [
        "bold"
      ]
    },
    "ui.cursorline.primary": {
      "bg": "bg-menu"
    },
    "ui.help": {
      "bg": "bg-menu",
      "fg": "fg"
    },
    "ui.linenr": {
      "fg": "fg-gutter"
    },
    "ui.linenr.selected": {
      "fg": "fg-linenr"
    },
    "ui.menu": {
      "bg": "bg-menu",
      "fg": "fg"
    },
    "ui.menu.selected": {
      "bg": "fg-selected"
    },
    "ui.popup": {
      "bg": "bg-menu",
      "fg": "border-highlight"
    },
    "ui.selection": {
      "bg": "bg-selection"
    },
    "ui.selection.primary": {
      "bg": "bg-selection"
    },
    "ui.statusline": {
      "bg": "bg-menu",
      "fg": "fg-dark"
    },
    "ui.statusline.inactive": {
      "bg": "bg-menu",
      "fg": "fg-gutter"
    },
    "ui.statusline.normal": {
      "bg": "blue",
      "fg": "bg",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.insert": {
      "bg": "light-green",
      "fg": "bg",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.select": {
      "bg": "magenta",
      "fg": "bg",
      "modifiers": [
        "bold"
      ]
    },
    "ui.text": {
      "fg": "fg"
    },
    "ui.text.focus": {
      "bg": "bg-focus"
    },
    "ui.text.inactive": {
      "fg": "comment",
      "modifiers": [
        "italic"
      ]
    },
    "ui.text.info": {
      "bg": "bg-menu",
      "fg": "fg"
    },
    "ui.text.directory": {
      "fg": "cyan"
    },
    "ui.virtual.ruler": {
      "bg": "fg-gutter"
    },
    "ui.virtual.whitespace": {
      "fg": "fg-gutter"
    },
    "ui.virtual.inlay-hint": {
      "bg": "bg-inlay",
      "fg": "teal"
    },
    "ui.virtual.jump-label": {
      "fg": "orange",
      "modifiers": [
        "bold"
      ]
    },
    "ui.window": {
      "fg": "border",
      "modifiers": [
        "bold"
      ]
    }
  }
});
