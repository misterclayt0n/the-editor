import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "nyxvamp-obsidian",
  "title": "Nyxvamp Obsidian",
  "palette": {
    "background": "#000A0F",
    "foreground": "#C0C0CE",
    "cursor_fg": "#0E0E10",
    "cursor_bg": "#F28FAD",
    "cursorline": "#1E1E20",
    "selection": "#2E2E30",
    "line_nr": "#5E5A76",
    "line_nr_selected": "#C0C0CE",
    "status_fg": "#C0C0CE",
    "status_bg": "#1E1E20",
    "status_inactive_fg": "#5E5A76",
    "status_inactive_bg": "#0E0E10",
    "menu_fg": "#C0C0CE",
    "menu_bg": "#0E0E10",
    "menu_sel_fg": "#0E0E10",
    "menu_sel_bg": "#F28FAD",
    "menu_scroll_fg": "#5E5A76",
    "menu_scroll_bg": "#0E0E10",
    "popup_fg": "#C0C0CE",
    "popup_bg": "#0E0E10",
    "match_paren_fg": "#F28FAD",
    "match_paren_bg": "#0E0E10",
    "virtual_ruler_bg": "#191921",
    "comment_fg": "#5E5A76",
    "string_fg": "#8FBF8F",
    "string_special_fg": "#F28FAD",
    "constant_fg": "#F28FAD",
    "constant_builtin_fg": "#F28FAD",
    "number_fg": "#D8A080",
    "boolean_fg": "#F28FAD",
    "function_fg": "#7FAFD7",
    "function_builtin_fg": "#7FAFD7",
    "keyword_fg": "#F5C2E7",
    "keyword_control_fg": "#F5C2E7",
    "operator_fg": "#C0C0CE",
    "variable_fg": "#C0C0CE",
    "variable_builtin_fg": "#F28FAD",
    "type_fg": "#A0A0D0",
    "type_builtin_fg": "#A0A0D0",
    "attribute_fg": "#F5C2E7",
    "namespace_fg": "#A090C0",
    "punctuation_fg": "#C0C0CE",
    "symbol_fg": "#F28FAD",
    "error_fg": "#D78284",
    "warning_fg": "#D5B880",
    "info_fg": "#7A9CCC",
    "hint_fg": "#7BB5A8",
    "diff_add_fg": "#86BA75",
    "diff_delete_fg": "#D78284",
    "diff_change_fg": "#D5B880",
    "markup_heading_fg": "#F5C2E7",
    "markup_bold_fg": "#F5C2E7",
    "markup_italic_fg": "#F5C2E7",
    "markup_link_url_fg": "#7FAFD7",
    "markup_link_text_fg": "#F5C2E7",
    "markup_quote_fg": "#5E5A76"
  },
  "styles": {
    "ui.background": {
      "bg": "background",
      "fg": "foreground"
    },
    "ui.text": {
      "fg": "foreground"
    },
    "ui.cursor": {
      "fg": "cursor_fg",
      "bg": "cursor_bg"
    },
    "ui.cursorline.primary": {
      "bg": "cursorline"
    },
    "ui.selection": {
      "bg": "selection"
    },
    "ui.linenr": {
      "fg": "line_nr"
    },
    "ui.linenr.selected": {
      "fg": "line_nr_selected",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline": {
      "fg": "status_fg",
      "bg": "status_bg"
    },
    "ui.statusline.inactive": {
      "fg": "status_inactive_fg",
      "bg": "status_inactive_bg"
    },
    "ui.menu": {
      "fg": "menu_fg",
      "bg": "menu_bg"
    },
    "ui.menu.selected": {
      "fg": "menu_sel_fg",
      "bg": "menu_sel_bg"
    },
    "ui.menu.scroll": {
      "fg": "menu_scroll_fg",
      "bg": "menu_scroll_bg"
    },
    "ui.popup": {
      "fg": "popup_fg",
      "bg": "popup_bg"
    },
    "ui.match_paren": {
      "fg": "match_paren_fg",
      "bg": "match_paren_bg",
      "modifiers": [
        "bold"
      ]
    },
    "ui.virtual": {
      "fg": "comment_fg"
    },
    "ui.virtual.ruler": {
      "bg": "virtual_ruler_bg"
    },
    "comment": {
      "fg": "comment_fg",
      "modifiers": [
        "italic"
      ]
    },
    "string": {
      "fg": "string_fg"
    },
    "string.special": {
      "fg": "string_special_fg"
    },
    "constant": {
      "fg": "constant_fg"
    },
    "constant.builtin": {
      "fg": "constant_builtin_fg",
      "modifiers": [
        "bold"
      ]
    },
    "number": {
      "fg": "number_fg"
    },
    "boolean": {
      "fg": "boolean_fg"
    },
    "function": {
      "fg": "function_fg",
      "modifiers": [
        "bold"
      ]
    },
    "function.builtin": {
      "fg": "function_builtin_fg",
      "modifiers": [
        "bold"
      ]
    },
    "keyword": {
      "fg": "keyword_fg",
      "modifiers": [
        "bold"
      ]
    },
    "keyword.control": {
      "fg": "keyword_control_fg",
      "modifiers": [
        "bold"
      ]
    },
    "operator": {
      "fg": "operator_fg"
    },
    "variable": {
      "fg": "variable_fg"
    },
    "variable.builtin": {
      "fg": "variable_builtin_fg",
      "modifiers": [
        "italic"
      ]
    },
    "type": {
      "fg": "type_fg",
      "modifiers": [
        "italic"
      ]
    },
    "type.builtin": {
      "fg": "type_builtin_fg",
      "modifiers": [
        "italic"
      ]
    },
    "attribute": {
      "fg": "attribute_fg"
    },
    "namespace": {
      "fg": "namespace_fg"
    },
    "punctuation": {
      "fg": "punctuation_fg"
    },
    "symbol": {
      "fg": "symbol_fg",
      "modifiers": [
        "italic"
      ]
    },
    "diagnostic.error": {
      "fg": "error_fg",
      "modifiers": [
        "bold"
      ]
    },
    "diagnostic.warning": {
      "fg": "warning_fg",
      "modifiers": [
        "bold"
      ]
    },
    "diagnostic.info": {
      "fg": "info_fg",
      "modifiers": [
        "bold"
      ]
    },
    "diagnostic.hint": {
      "fg": "hint_fg",
      "modifiers": [
        "bold"
      ]
    },
    "diff.plus": {
      "fg": "diff_add_fg"
    },
    "diff.minus": {
      "fg": "diff_delete_fg"
    },
    "diff.delta": {
      "fg": "diff_change_fg"
    },
    "markup.heading": {
      "fg": "markup_heading_fg",
      "modifiers": [
        "bold"
      ]
    },
    "markup.bold": {
      "fg": "markup_bold_fg",
      "modifiers": [
        "bold"
      ]
    },
    "markup.italic": {
      "fg": "markup_italic_fg",
      "modifiers": [
        "italic"
      ]
    },
    "markup.link.url": {
      "fg": "markup_link_url_fg",
      "modifiers": [
        "underlined"
      ]
    },
    "markup.link.text": {
      "fg": "markup_link_text_fg"
    },
    "markup.quote": {
      "fg": "markup_quote_fg",
      "modifiers": [
        "italic"
      ]
    }
  }
});
