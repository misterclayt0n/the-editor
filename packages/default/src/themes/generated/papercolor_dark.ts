import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "papercolor-dark",
  "title": "Papercolor Dark",
  "palette": {
    "background": "#1c1c1c",
    "foreground": "#d0d0d0",
    "regular0": "#1c1c1c",
    "regular1": "#af005f",
    "regular2": "#5faf00",
    "regular3": "#d7af5f",
    "regular4": "#5fafd7",
    "regular5": "#808080",
    "regular6": "#d7875f",
    "regular7": "#d0d0d0",
    "bright0": "#585858",
    "bright1": "#5faf5f",
    "bright2": "#afd700",
    "bright3": "#af87d7",
    "bright4": "#ffaf00",
    "bright5": "#ff5faf",
    "bright6": "#00afaf",
    "bright7": "#5f8787",
    "selection_fg": "#000000",
    "selection_bg": "#8787af",
    "selection_secondary_fg": "#333333",
    "selection_secondary_bg": "#707097",
    "special": "#3e999f",
    "cursorline_bg": "#303030",
    "cursorline_secondary_bg": "#2a2a2a",
    "cursorcolumn_bg": "#303030",
    "cursorcolumn_secondary_bg": "#2a2a2a",
    "cursorlinenr_fg": "#ffff00",
    "popupmenu_fg": "#c6c6c6",
    "popupmenu_bg": "#303030",
    "linenumber_fg": "#585858",
    "vertsplit_fg": "#5f8787",
    "statusline_active_fg": "#1c1c1c",
    "statusline_active_bg": "#5f8787",
    "statusline_inactive_fg": "#bcbcbc",
    "statusline_inactive_bg": "#3a3a3a",
    "todo_fg": "#ff8700",
    "error_fg": "#af005f",
    "error_bg": "#5f0000",
    "matchparen_bg": "#4e4e4e",
    "matchparen_fg": "#c6c6c6",
    "wildmenu_fg": "#1c1c1c",
    "wildmenu_bg": "#afd700",
    "diffadd_fg": "#87d700",
    "diffadd_bg": "#005f00",
    "diffdelete_fg": "#af005f",
    "diffdelete_bg": "#5f0000",
    "diffchange_bg": "#005f5f",
    "black": "#1c1c1c",
    "red": "#af005f",
    "green": "#5faf00",
    "yellow": "#d7af5f",
    "blue": "#5fafd7",
    "magenta": "#808080",
    "cyan": "#d7875f",
    "white": "#d0d0d0",
    "light-black": "#585858",
    "light-red": "#5faf5f",
    "light-green": "#afd700",
    "light-yellow": "#af87d7",
    "light-blue": "#ffaf00",
    "light-magenta": "#ff5faf",
    "light-cyan": "#00afaf",
    "light-white": "#5f8787"
  },
  "styles": {
    "ui.linenr.selected": {
      "fg": "cursorlinenr_fg",
      "modifiers": [
        "bold"
      ]
    },
    "ui.linenr": {
      "fg": "linenumber_fg"
    },
    "ui.background": {
      "bg": "background"
    },
    "ui.text": "foreground",
    "ui.text.focus": {
      "fg": "selection_bg",
      "modifiers": [
        "bold"
      ]
    },
    "ui.selection": {
      "bg": "selection_secondary_bg",
      "fg": "selection_secondary_fg"
    },
    "ui.selection.primary": {
      "bg": "selection_bg",
      "fg": "selection_fg"
    },
    "ui.highlight": {
      "bg": "cursorline_bg"
    },
    "ui.cursorline": {
      "bg": "cursorline_bg"
    },
    "ui.cursorline.secondary": {
      "bg": "cursorline_secondary_bg"
    },
    "ui.cursorcolumn": {
      "bg": "cursorline_bg"
    },
    "ui.cursorcolumn.secondary": {
      "bg": "cursorcolumn_secondary_bg"
    },
    "ui.statusline": {
      "bg": "statusline_active_bg",
      "fg": "statusline_active_fg"
    },
    "ui.statusline.inactive": {
      "bg": "statusline_inactive_bg",
      "fg": "statusline_inactive_fg"
    },
    "ui.statusline.normal": {
      "bg": "statusline_inactive_bg",
      "fg": "bright6"
    },
    "ui.statusline.insert": {
      "bg": "statusline_inactive_bg",
      "fg": "bright4"
    },
    "ui.statusline.select": {
      "bg": "statusline_inactive_bg",
      "fg": "regular3"
    },
    "ui.statusline.separator": {
      "bg": "statusline_active_bg",
      "fg": "statusline_active_bg"
    },
    "ui.virtual": {
      "fg": "cursorlinenr_fg"
    },
    "ui.virtual.whitespace": {
      "fg": "regular5"
    },
    "ui.virtual.indent-guide": {
      "fg": "bright0"
    },
    "ui.virtual.ruler": {
      "bg": "cursorline_secondary_bg",
      "fg": "regular4"
    },
    "ui.virtual.inlay-hint": {
      "fg": "regular5",
      "modifiers": [
        "italic"
      ]
    },
    "ui.virtual.inlay-hint.parameter": {
      "fg": "regular5",
      "modifiers": [
        "italic"
      ]
    },
    "ui.virtual.inlay-hint.type": {
      "fg": "regular5",
      "modifiers": [
        "bold",
        "italic"
      ]
    },
    "ui.virtual.jump-label": {
      "fg": "bright1",
      "modifiers": [
        "bold"
      ]
    },
    "ui.cursor.match": {
      "bg": "matchparen_bg",
      "fg": "matchparen_fg"
    },
    "ui.cursor": {
      "bg": "regular5",
      "fg": "background"
    },
    "ui.cursor.primary": {
      "bg": "foreground",
      "fg": "background"
    },
    "ui.window": {
      "fg": "vertsplit_fg"
    },
    "ui.help": {
      "bg": "wildmenu_bg",
      "fg": "wildmenu_fg"
    },
    "ui.popup": {
      "bg": "popupmenu_bg",
      "fg": "popupmenu_fg"
    },
    "ui.popup.info": {
      "bg": "popupmenu_bg",
      "fg": "bright7",
      "modifiers": [
        "bold"
      ]
    },
    "ui.menu": {
      "bg": "popupmenu_bg",
      "fg": "foreground"
    },
    "ui.menu.selected": {
      "bg": "selection_bg",
      "fg": "selection_fg"
    },
    "warning": "bright5",
    "error": {
      "bg": "error_bg",
      "fg": "error_fg"
    },
    "info": "todo_fg",
    "diagnostic.warning": {
      "fg": "bright0",
      "modifiers": [
        "dim"
      ],
      "underline": {
        "color": "bright5",
        "style": "curl"
      }
    },
    "diagnostic.error": {
      "underline": {
        "color": "bright1",
        "style": "curl"
      }
    },
    "diagnostic.info": {
      "underline": {
        "color": "bright4",
        "style": "curl"
      }
    },
    "diagnostic.hint": {
      "underline": {
        "color": "bright6",
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
    "attribute": "bright4",
    "type": {
      "fg": "bright2",
      "modifiers": [
        "bold"
      ]
    },
    "type.builtin": {
      "fg": "bright2",
      "modifiers": [
        "bold"
      ]
    },
    "type.parameter": {
      "fg": "foreground"
    },
    "type.enum": {
      "fg": "foreground"
    },
    "type.enum.variant": {
      "fg": "foreground"
    },
    "constructor": "foreground",
    "constant": "bright5",
    "constant.builtin": "regular3",
    "constant.builtin.boolean": {
      "fg": "regular2",
      "modifiers": [
        "bold"
      ]
    },
    "constant.character.escape": {
      "fg": "bright3",
      "modifiers": [
        "bold"
      ]
    },
    "constant.character": {
      "fg": "regular3"
    },
    "constant.numeric": "bright5",
    "string": "regular3",
    "string.regexp": "bright3",
    "comment": {
      "fg": "regular5",
      "modifiers": [
        "italic"
      ]
    },
    "comment.line": {
      "fg": "regular5",
      "modifiers": [
        "italic"
      ]
    },
    "comment.line.documentation": {
      "fg": "regular5",
      "modifiers": [
        "bold"
      ]
    },
    "comment.block": {
      "fg": "regular5",
      "modifiers": [
        "italic"
      ]
    },
    "comment.block.documentation": {
      "fg": "regular5",
      "modifiers": [
        "bold"
      ]
    },
    "variable": "foreground",
    "variable.builtin": "bright5",
    "variable.other.member": "foreground",
    "variable.parameter": "foreground",
    "label": {
      "fg": "selection_bg",
      "modifiers": [
        "bold",
        "italic"
      ]
    },
    "punctuation": {
      "fg": "foreground"
    },
    "punctuation.delimiter": {
      "fg": "regular4",
      "modifiers": [
        "bold"
      ]
    },
    "punctuation.bracket": {
      "fg": "foreground"
    },
    "punctuation.special": {
      "fg": "bright1",
      "modifiers": [
        "bold"
      ]
    },
    "keyword": {
      "fg": "bright2"
    },
    "keyword.control": "bright1",
    "keyword.control.conditional": {
      "fg": "bright3",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.control.repeat": {
      "fg": "bright3",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.control.import": {
      "fg": "bright2"
    },
    "keyword.control.return": {
      "fg": "bright2"
    },
    "keyword.control.exception": {
      "fg": "bright1"
    },
    "keyword.operator": {
      "fg": "regular4",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.directive": "regular4",
    "keyword.function": "bright2",
    "keyword.storage": "bright2",
    "keyword.storage.type": {
      "fg": "regular4",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.storage.modifier": {
      "fg": "regular6",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.storage.modifier.ref": {
      "fg": "regular4",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.special": "bright1",
    "operator": {
      "fg": "regular4",
      "modifiers": [
        "bold"
      ]
    },
    "function": {
      "fg": "foreground"
    },
    "function.builtin": {
      "fg": "bright6"
    },
    "function.method": {
      "fg": "foreground"
    },
    "function.macro": {
      "fg": "regular3",
      "modifiers": [
        "bold"
      ]
    },
    "function.special": {
      "fg": "bright4"
    },
    "tag": {
      "fg": "regular4"
    },
    "namespace": "bright6",
    "special": "special",
    "markup.heading": {
      "fg": "bright4",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.marker": {
      "fg": "bright2",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.1": {
      "fg": "bright2",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.2": {
      "fg": "bright5",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.3": {
      "fg": "bright3",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.4": {
      "fg": "bright4",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.5": {
      "fg": "bright4",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.6": {
      "fg": "bright4",
      "modifiers": [
        "bold"
      ]
    },
    "markup.list": "regular4",
    "markup.bold": {
      "fg": "foreground",
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
    "markup.link.url": {
      "fg": "bright6",
      "underline": {
        "style": "line"
      }
    },
    "markup.link.text": "bright2",
    "markup.link.label": {
      "fg": "regular2",
      "modifiers": [
        "bold"
      ]
    },
    "markup.quote": "regular4",
    "markup.raw": "regular3",
    "diff.plus": {
      "bg": "diffadd_bg",
      "fg": "diffadd_fg"
    },
    "diff.delta": {
      "bg": "diffchange_bg"
    },
    "diff.delta.moved": {
      "modifiers": [
        "italic"
      ]
    },
    "diff.minus": {
      "bg": "diffdelete_bg",
      "fg": "diffdelete_fg"
    }
  }
});
