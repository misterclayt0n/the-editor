import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "nyxvamp-radiance",
  "title": "Nyxvamp Radiance",
  "palette": {
    "background": "#F7F7FF",
    "foreground": "#1E1E2E",
    "cursor_fg": "#F7F7FF",
    "cursor_bg": "#9655FF",
    "cursorline": "#E8E8F0",
    "selection": "#FEF5BC",
    "line_nr": "#6E6A86",
    "line_nr_selected": "#1E1E2E",
    "status_fg": "#1E1E2E",
    "status_bg": "#E8E8F0",
    "status_inactive_fg": "#6E6A86",
    "status_inactive_bg": "#F7F7FF",
    "menu_fg": "#1E1E2E",
    "menu_bg": "#E8E8F0",
    "menu_sel_fg": "#F7F7FF",
    "menu_sel_bg": "#FEF5BC",
    "menu_scroll_fg": "#6E6A86",
    "menu_scroll_bg": "#E8E8F0",
    "popup_fg": "#1E1E2E",
    "popup_bg": "#E8E8F0",
    "match_paren_fg": "#1E1E2E",
    "match_paren_bg": "#FEF5BC",
    "virtual_ruler_bg": "#EAEAF0",
    "comment_fg": "#6E6A86",
    "string_fg": "#FEF5BC",
    "string_special_fg": "#F28FAD",
    "constant_fg": "#F28FAD",
    "constant_builtin_fg": "#F28FAD",
    "number_fg": "#F8BD96",
    "boolean_fg": "#F28FAD",
    "function_fg": "#005F87",
    "function_builtin_fg": "#005F87",
    "keyword_fg": "#9655FF",
    "keyword_control_fg": "#9655FF",
    "operator_fg": "#6E6A86",
    "variable_fg": "#1E1E2E",
    "variable_builtin_fg": "#F28FAD",
    "type_fg": "#C9CBFF",
    "type_builtin_fg": "#C9CBFF",
    "attribute_fg": "#9655FF",
    "namespace_fg": "#9655FF",
    "punctuation_fg": "#1E1E2E",
    "symbol_fg": "#F28FAD",
    "error_fg": "#E78284",
    "warning_fg": "#E5C890",
    "info_fg": "#8CAAEE",
    "hint_fg": "#8BD5CA",
    "diff_add_fg": "#A6DA95",
    "diff_delete_fg": "#E78284",
    "diff_change_fg": "#E5C890",
    "markup_heading_fg": "#9655FF",
    "markup_bold_fg": "#1E1E2E",
    "markup_italic_fg": "#1E1E2E",
    "markup_link_url_fg": "#005F87",
    "markup_link_text_fg": "#9655FF",
    "markup_quote_fg": "#6E6A86"
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
