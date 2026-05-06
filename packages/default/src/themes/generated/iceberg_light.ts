import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "iceberg-light",
  "title": "Iceberg Light",
  "palette": {
    "orange": "#c67439",
    "pale": "#505695",
    "purple": "#785ab5",
    "black": "#dcdfe7",
    "gray": "#8389a3",
    "red": "#cd517a",
    "light-red": "#cc3768",
    "green": "#668f3d",
    "light-green": "#598030",
    "yellow": "#c57339",
    "light-yellow": "#b6662d",
    "blue": "#2e539e",
    "light-blue": "#22478e",
    "magenta": "#7759b4",
    "light-magenta": "#6845ad",
    "cyan": "#3f84a6",
    "light-cyan": "#327698",
    "white": "#33374c",
    "light-gray": "#262a3f",
    "background_bg": "#e9e9ed",
    "background_fg": "#33374d",
    "comment_fg": "#8489a4",
    "cursorlinenr_bg": "#cccfe0",
    "linenr_bg": "#dddfe9",
    "linenr_fg": "#a0a5c0",
    "matchparen_bg": "#bec0ca",
    "menusel_bg": "#a9afd1",
    "menusel_fg": "#f0f1f5",
    "sel_bg": "#cacdd8",
    "statusline_bg": "#cad0de",
    "statusline_fg": "#757da3"
  },
  "styles": {
    "attribute": {
      "fg": "green"
    },
    "boolean": {
      "fg": "purple"
    },
    "character": {
      "fg": "purple"
    },
    "comment": {
      "fg": "comment_fg"
    },
    "conditional": {
      "fg": "blue"
    },
    "constant": {
      "fg": "purple"
    },
    "constructor": {
      "fg": "blue"
    },
    "diagnostic.deprecated": {
      "modifiers": [
        "crossed_out"
      ]
    },
    "diagnostic.error": {
      "underline": {
        "style": "curl",
        "color": "red"
      }
    },
    "diagnostic.hint": {
      "underline": {
        "style": "curl",
        "color": "comment_fg"
      }
    },
    "diagnostic.info": {
      "underline": {
        "style": "curl",
        "color": "cyan"
      }
    },
    "diagnostic.unnecessary": {
      "modifiers": [
        "dim"
      ]
    },
    "diagnostic.warning": {
      "underline": {
        "style": "curl",
        "color": "orange"
      }
    },
    "diff.delta": {
      "fg": "blue"
    },
    "diff.delta.gutter": {
      "fg": "cyan",
      "bg": "linenr_bg"
    },
    "diff.minus": {
      "fg": "red"
    },
    "diff.minus.gutter": {
      "fg": "red",
      "bg": "linenr_bg"
    },
    "diff.plus": {
      "fg": "green"
    },
    "diff.plus.gutter": {
      "fg": "green",
      "bg": "linenr_bg"
    },
    "error": {
      "fg": "red"
    },
    "exception": {
      "fg": "blue"
    },
    "field": {
      "fg": "background_fg"
    },
    "float": {
      "fg": "purple"
    },
    "function": {
      "fg": "pale"
    },
    "function.macro": {
      "fg": "green"
    },
    "hint": {
      "fg": "comment_fg"
    },
    "identifier": {
      "fg": "blue"
    },
    "info": {
      "fg": "cyan"
    },
    "keyword": {
      "fg": "blue"
    },
    "keyword.directive": {
      "fg": "green"
    },
    "keyword.import": {
      "fg": "pale"
    },
    "label": {
      "fg": "green"
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
      "fg": "cyan"
    },
    "markup.link.text": {
      "fg": "cyan"
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
    "markup.raw": {
      "fg": "cyan"
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
    "method": {
      "fg": "pale"
    },
    "namespace": {
      "fg": "blue"
    },
    "number": {
      "fg": "purple"
    },
    "operator": {
      "fg": "blue"
    },
    "parameter": {
      "fg": "background_fg"
    },
    "property": {
      "fg": "background_fg"
    },
    "punctuation.bracket": {
      "fg": "background_fg"
    },
    "punctuation.delimiter": {
      "fg": "background_fg"
    },
    "punctuation.special": {
      "fg": "green"
    },
    "repeat": {
      "fg": "blue"
    },
    "special": {
      "fg": "green"
    },
    "string": {
      "fg": "cyan"
    },
    "string.escape": {
      "fg": "green"
    },
    "string.special": {
      "fg": "green"
    },
    "tag": {
      "fg": "blue"
    },
    "tag.attribute": {
      "fg": "purple"
    },
    "text": {
      "fg": "background_fg"
    },
    "type": {
      "fg": "blue"
    },
    "ui.background": {
      "fg": "background_fg",
      "bg": "background_bg"
    },
    "ui.background.separator": {
      "fg": "comment_fg"
    },
    "ui.bufferline.active": {
      "fg": "pale"
    },
    "ui.cursor.match": {
      "fg": "background_fg",
      "bg": "matchparen_bg"
    },
    "ui.cursor.normal": {
      "bg": "gray"
    },
    "ui.cursor.primary": {
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.select": {
      "bg": "gray"
    },
    "ui.cursorline.primary": {
      "bg": "linenr_bg"
    },
    "ui.gutter": {
      "fg": "linenr_fg",
      "bg": "linenr_bg"
    },
    "ui.help": {
      "fg": "background_fg",
      "bg": "cursorlinenr_bg"
    },
    "ui.linenr": {
      "fg": "linenr_fg",
      "bg": "linenr_bg"
    },
    "ui.menu": {
      "fg": "background_fg",
      "bg": "cursorlinenr_bg"
    },
    "ui.menu.border": {
      "fg": "comment_fg"
    },
    "ui.menu.selected": {
      "fg": "background_fg",
      "bg": "menusel_bg"
    },
    "ui.popup": {
      "fg": "background_fg",
      "bg": "cursorlinenr_bg"
    },
    "ui.popup.info": {
      "fg": "blue"
    },
    "ui.selection": {
      "bg": "sel_bg"
    },
    "ui.statusline": {
      "bg": "statusline_bg",
      "fg": "statusline_fg"
    },
    "ui.statusline.insert": {
      "fg": "black",
      "bg": "blue"
    },
    "ui.statusline.select": {
      "fg": "black",
      "bg": "green"
    },
    "ui.text.focus": {
      "fg": "orange"
    },
    "ui.virtual": {
      "fg": "linenr_fg"
    },
    "ui.virtual.indent-guide": {
      "fg": "linenr_fg"
    },
    "ui.virtual.jump-label": {
      "fg": "orange",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual.ruler": {
      "bg": "linenr_bg"
    },
    "ui.virtual.whitespace": {
      "fg": "sel_bg"
    },
    "ui.window": {
      "fg": "comment_fg",
      "modifiers": [
        "bold"
      ]
    },
    "variable": {
      "fg": "background_fg"
    },
    "variable.builtin": {
      "fg": "blue"
    },
    "warning": {
      "fg": "orange"
    }
  }
});
