import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "sidra",
  "title": "Sidra",
  "palette": {
    "bg_normal": "#BD93F9",
    "bg_insert": "#50fa7b",
    "bg_select": "#8be9fd",
    "bg_match": "#D3D3D3",
    "plus": "#4F6F52",
    "minus": "#B80000",
    "delta": "#3876BF",
    "markup_heading": "#ff69b4",
    "markup_bold": "#e7c547",
    "markup_italic": "#b294bb",
    "markup_strikethrough": "#d7005f",
    "markup_link_url": "#3876BF",
    "markup_link_text": "#FFA500",
    "markup_raw": "#808080",
    "foreground": "#D3D3D3",
    "background": "#1f1f21",
    "fg_linenr": "#747575",
    "fg_linenr_sld": "#c7dddd",
    "fg_help": "#D3D3D3",
    "bg_help": "#35353a",
    "bg_popup": "#3b3b3d",
    "fg_window": "#F1DCA7",
    "fg_text": "#D3D3D3",
    "fg_text_focus": "#83c679",
    "fg_text_inactive": "#93a56f",
    "fg_virtual": "#F1DCA7",
    "bg_virtual_ruler": "#363638",
    "fg_virtual_indent": "#5B5B5A",
    "fg_debug": "#634450",
    "fg_debug_active": "#3876BF",
    "fg_debug_breakpoint": "#B80000",
    "bg_cursorline": "#2d303e",
    "bg_selection": "#71797E",
    "fg_statusline": "#D3D3D3",
    "bg_statusline": "#303030",
    "fg_inactive": "#4b5059",
    "bg_inactive": "#303030",
    "cl_hint": "#f1fa8c",
    "cl_info": "#f1fa8c",
    "cl_error": "#ff69b4",
    "cl_warning": "#f1fa8c"
  },
  "styles": {
    "ui.background": {
      "fg": "foreground",
      "bg": "background"
    },
    "ui.background.separator": {
      "fg": "white"
    },
    "ui.linenr": {
      "fg": "fg_linenr"
    },
    "ui.linenr.selected": {
      "fg": "fg_linenr_sld"
    },
    "ui.help": {
      "fg": "fg_help",
      "bg": "bg_help"
    },
    "ui.popup": {
      "bg": "bg_popup"
    },
    "ui.window": {
      "fg": "fg_window"
    },
    "ui.text": {
      "fg": "fg_text"
    },
    "ui.text.focus": {
      "fg": "fg_text_focus"
    },
    "ui.text.inactive": {
      "fg": "fg_text_inactive"
    },
    "ui.virtual": {
      "fg": "fg_virtual"
    },
    "ui.virtual.ruler": {
      "bg": "bg_virtual_ruler"
    },
    "ui.virtual.indent-guide": {
      "fg": "fg_virtual_indent"
    },
    "ui.debug": {
      "fg": "fg_debug",
      "modifiers": [
        "bold"
      ]
    },
    "ui.debug.active": {
      "fg": "fg_debug_active",
      "modifiers": [
        "bold"
      ]
    },
    "ui.debug.breakpoint": {
      "fg": "fg_debug_breakpoint",
      "modifiers": [
        "bold"
      ]
    },
    "ui.menu": {
      "fg": "gray",
      "bg": "black"
    },
    "ui.menu.selected": {
      "fg": "black",
      "bg": "gray"
    },
    "ui.cursor": {
      "fg": "foreground",
      "bg": "bg_normal",
      "modifiers": [
        "dim"
      ]
    },
    "ui.cursor.match": {
      "fg": "foreground",
      "bg": "bg_match",
      "modifiers": [
        "dim"
      ]
    },
    "ui.cursor.normal": {
      "fg": "background",
      "bg": "bg_normal",
      "modifiers": [
        "dim"
      ]
    },
    "ui.cursor.insert": {
      "fg": "background",
      "bg": "bg_insert",
      "modifiers": [
        "dim"
      ]
    },
    "ui.cursor.select": {
      "fg": "background",
      "bg": "bg_select",
      "modifiers": [
        "dim"
      ]
    },
    "ui.cursor.primary.normal": {
      "fg": "background",
      "bg": "bg_normal"
    },
    "ui.cursor.primary.insert": {
      "fg": "background",
      "bg": "bg_insert"
    },
    "ui.cursor.primary.select": {
      "fg": "background",
      "bg": "bg_select"
    },
    "ui.cursorline": {
      "bg": "bg_cursorline"
    },
    "ui.cursorline.primary": {
      "bg": "bg_cursorline"
    },
    "ui.selection": {
      "fg": "white",
      "bg": "bg_selection"
    },
    "ui.selection.primary": {
      "fg": "white",
      "bg": "bg_selection"
    },
    "ui.statusline": {
      "fg": "fg_statusline",
      "bg": "bg_statusline"
    },
    "ui.statusline.inactive": {
      "fg": "fg_inactive",
      "bg": "bg_inactive"
    },
    "ui.statusline.normal": {
      "fg": "black",
      "bg": "bg_normal",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.insert": {
      "fg": "black",
      "bg": "bg_insert",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.select": {
      "fg": "black",
      "bg": "bg_select",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading": {
      "fg": "markup_heading"
    },
    "markup.bold": {
      "fg": "markup_bold",
      "modifiers": [
        "bold"
      ]
    },
    "markup.italic": {
      "fg": "markup_italic",
      "modifiers": [
        "italic"
      ]
    },
    "markup.strikethrough": {
      "fg": "markup_strikethrough",
      "modifiers": [
        "crossed_out",
        "bold"
      ]
    },
    "markup.link.url": {
      "fg": "markup_link_url",
      "modifiers": [
        "underlined"
      ]
    },
    "markup.link.text": {
      "fg": "markup_link_text"
    },
    "markup.raw": {
      "fg": "markup_raw"
    },
    "diff.plus": {
      "fg": "plus",
      "modifiers": [
        "bold"
      ]
    },
    "diff.minus": {
      "fg": "minus",
      "modifiers": [
        "bold"
      ]
    },
    "diff.delta": {
      "fg": "delta",
      "modifiers": [
        "bold"
      ]
    },
    "diagnostic.hint": {
      "underline": {
        "color": "cl_hint",
        "style": "curl"
      }
    },
    "diagnostic.info": {
      "underline": {
        "color": "cl_info",
        "style": "curl"
      }
    },
    "diagnostic.error": {
      "underline": {
        "color": "cl_error",
        "style": "curl"
      }
    },
    "diagnostic.warning": {
      "underline": {
        "color": "cl_warning",
        "style": "curl"
      }
    },
    "hint": "cl_hint",
    "info": "cl_info",
    "error": "cl_error",
    "warning": "cl_warning",
    "attribute": "#a7bf67",
    "keyword": "#A4A2B4",
    "keyword.directive": "#D3D3D3",
    "namespace": "#7095bf",
    "punctuation": "#FFFFFF",
    "punctuation.delimiter": "#FFFFFF",
    "operator": "#987654",
    "special": "#c90076",
    "variable": "#7ec67f",
    "variable.other.member": "#7ec67f",
    "variable.parameter": "#a8ffb4",
    "variable.builtin": "#a8ffb4",
    "type": "#efbe4c",
    "type.builtin": "#efbe4c",
    "constructor": "#c19ef7",
    "function": "#987654",
    "function.macro": "#987654",
    "function.builtin": "#db985e",
    "tag": "#d37a78",
    "comment": "#D3D3D3",
    "constant": "#A5C4D4",
    "constant.builtin": "#f1fa8c",
    "constant.numeric": "#b577b0",
    "constant.character.escape": "#c95c56",
    "string": "#d6a560",
    "label": "#abcc8a"
  }
});
