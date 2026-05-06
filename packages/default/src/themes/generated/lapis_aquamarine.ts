import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "lapis_aquamarine",
  "title": "Lapis Aquamarine",
  "palette": {
    "bg": "#1B1F27",
    "fg": "#D6E2FF",
    "grey": "#7183A9",
    "blue": "#83ABFC",
    "green": "#ABFC83",
    "cyan": "#84FCD4",
    "red": "#FC83AB",
    "yellow": "#FCD583",
    "pink": "#FC83D6",
    "orange": "#FCAB83",
    "purple": "#D483FC",
    "bg_alt": "#14181D",
    "bg_highlight": "#43537A",
    "indent_color": "#2E3953"
  },
  "styles": {
    "ui.background": {
      "fg": "fg",
      "bg": "bg"
    },
    "ui.text": "fg",
    "ui.text.focus": {
      "fg": "bg",
      "bg": "fg"
    },
    "ui.cursor": {
      "bg": "blue",
      "fg": "bg"
    },
    "ui.linenr": "grey",
    "ui.statusline": {
      "fg": "fg"
    },
    "ui.selection": "green",
    "ui.selection.primary": {
      "bg": "bg_highlight"
    },
    "ui.virtual.ruler": {
      "bg": "bg_highlight"
    },
    "markup.raw": {
      "fg": "purple",
      "bg": "bg"
    },
    "markup.raw.inline": {
      "fg": "fg"
    },
    "markup.heading.1": {
      "fg": "#46D9FF",
      "bg": "#11313B"
    },
    "markup.heading.2": {
      "fg": "#C678DD",
      "bg": "#2D2233"
    },
    "markup.heading.3": {
      "fg": "#98BE65",
      "bg": "#253027"
    },
    "markup.heading.4": {
      "fg": "#A8A1E1",
      "bg": "#2A2738"
    },
    "markup.heading.5": {
      "fg": "#ECBE7B",
      "bg": "#3A3223"
    },
    "markup.heading.6": {
      "fg": "#46D9FF",
      "bg": "#1E2A30"
    },
    "markup.list": {
      "fg": "purple"
    },
    "markup.bold": {
      "fg": "fg",
      "modifiers": [
        "bold"
      ]
    },
    "markup.italic": {
      "fg": "fg"
    },
    "markup.strikethrough": {
      "fg": "red",
      "modifiers": [
        "crossed_out"
      ]
    },
    "markup.link": {
      "fg": "cyan"
    },
    "markup.quote": {
      "fg": "fg"
    },
    "comment": {
      "fg": "grey",
      "modifiers": [
        "italic"
      ]
    },
    "comment.line": {
      "fg": "grey",
      "modifiers": [
        "italic"
      ]
    },
    "comment.block": {
      "fg": "grey",
      "modifiers": [
        "italic"
      ]
    },
    "comment.documentation": {
      "fg": "blue",
      "modifiers": [
        "italic"
      ]
    },
    "keyword": "blue",
    "keyword.storage": "blue",
    "keyword.storage.type": "blue",
    "keyword.storage.modifier": "fg",
    "keyword.control.import": "blue",
    "keyword.control.export": "blue",
    "keyword.control.conditional": "blue",
    "keyword.directive": "blue",
    "keyword.control.repeat": "blue",
    "keyword.operator": "blue",
    "keyword.function": "blue",
    "type": "green",
    "type.builtin": "green",
    "type.parameter": "green",
    "type.enum": "cyan",
    "function": "purple",
    "function.builtin": "purple",
    "function.method": "blue",
    "variable": "fg",
    "variable.builtin": "fg",
    "variable.parameter": "cyan",
    "string": "green",
    "string.special": "green",
    "constant": "cyan",
    "constant.builtin": "blue",
    "constant.numeric": "cyan",
    "constant.character": "yellow",
    "operator": "fg",
    "tag": {
      "fg": "purple",
      "modifiers": [
        "bold"
      ]
    },
    "attribute": "blue",
    "namespace": "fg",
    "macro": "orange",
    "label": "red",
    "ui.cursorline.primary": {
      "bg": "bg_highlight"
    },
    "ui.cursorline.secondary": {
      "bg": "bg_alt"
    },
    "ui.cursorcolumn.primary": {
      "bg": "bg_highlight"
    },
    "ui.cursorcolumn.secondary": {
      "bg": "bg_alt"
    },
    "ui.statusline.normal": {
      "fg": "bg",
      "bg": "cyan"
    },
    "ui.statusline.insert": {
      "fg": "bg",
      "bg": "green"
    },
    "ui.statusline.select": {
      "fg": "bg",
      "bg": "purple"
    },
    "warning": {
      "fg": "yellow",
      "modifiers": [
        "bold"
      ]
    },
    "error": {
      "fg": "red",
      "modifiers": [
        "bold"
      ]
    },
    "info": {
      "fg": "cyan",
      "modifiers": [
        "bold"
      ]
    },
    "hint": {
      "fg": "blue",
      "modifiers": [
        "bold"
      ]
    },
    "diagnostic.error": {
      "fg": "grey"
    },
    "diagnostic.warning": {
      "fg": "grey"
    },
    "diagnostic.info": {
      "fg": "grey"
    },
    "diagnostic.hint": {
      "fg": "grey"
    },
    "diff.plus": "green",
    "diff.plus.gutter": "green",
    "diff.minus": "red",
    "diff.minus.gutter": "red",
    "diff.delta": "fg",
    "diff.delta.gutter": "fg",
    "diff.delta.moved": "orange",
    "diff.delta.conflict": "purple",
    "ui.popup": {
      "fg": "fg",
      "bg": "bg"
    },
    "ui.popup.info": {
      "fg": "fg",
      "bg": "bg"
    },
    "ui.menu": {
      "fg": "fg",
      "bg": "bg"
    },
    "ui.menu.selected": {
      "fg": "bg",
      "bg": "fg"
    },
    "ui.virtual.whitespace": "grey",
    "ui.virtual.indent-guide": {
      "fg": "indent_color",
      "underline": {
        "style": "dotted"
      }
    }
  }
});
