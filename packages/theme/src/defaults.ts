import type { ThemeFamilyContent, ThemeSelection } from "./types.js";

export const DEFAULT_LIGHT_THEME = "Pierre Light";
export const DEFAULT_DARK_THEME = "Pierre Dark";

export const defaultThemeSelection: ThemeSelection = {
  mode: "system",
  light: DEFAULT_LIGHT_THEME,
  dark: DEFAULT_DARK_THEME,
};

/** Bundled default theme family by pierrecomputer (Zed v0.2.0 schema). */
export const pierreThemeFamily: ThemeFamilyContent = {
  "$schema": "https://zed.dev/schema/themes/v0.2.0.json",
  "name": "Pierre",
  "author": "pierrecomputer",
  "themes": [
    {
      "name": "Pierre Light",
      "appearance": "light",
      "style": {
        "background": "#f5f5f5",
        "surface.background": "#f5f5f5",
        "elevated_surface.background": "#f7f7f7",
        "drop_target.background": "#009fff26",
        "editor.background": "#ffffff",
        "editor.foreground": "#0a0a0a",
        "editor.gutter.background": "#ffffff",
        "editor.active_line.background": "#dfebff8c",
        "editor.active_line_number": "#525252",
        "editor.line_number": "#737373",
        "editor.highlighted_line.background": "#dfebff59",
        "editor.indent_guide": "#e5e5e5",
        "editor.indent_guide_active": "#d4d4d4",
        "editor.invisible": "#8a8a8a",
        "editor.wrap_guide": "#e5e5e5",
        "editor.active_wrap_guide": "#d4d4d4",
        "editor.document_highlight.read_background": "#009fff1a",
        "editor.document_highlight.write_background": "#009fff2e",
        "editor.document_highlight.bracket_background": "#009fff33",
        "editor.subheader.background": "#f5f5f5",
        "text": "#0a0a0a",
        "text.muted": "#737373",
        "text.placeholder": "#8a8a8a",
        "text.disabled": "#8a8a8a",
        "text.accent": "#009fff",
        "border": "#d4d4d4",
        "border.variant": "#e5e5e5",
        "border.focused": "#009fff",
        "border.selected": "#009fff",
        "border.transparent": "transparent",
        "border.disabled": "#d4d4d4",
        "element.background": "#ededed",
        "element.hover": "#dfebff80",
        "element.active": "#dfebffb3",
        "element.selected": "#dfebffcc",
        "element.disabled": "#ededed80",
        "ghost_element.background": "transparent",
        "ghost_element.hover": "#dfebff59",
        "ghost_element.active": "#dfebff8c",
        "ghost_element.selected": "#dfebffa6",
        "ghost_element.disabled": "transparent",
        "icon": "#525252",
        "icon.muted": "#737373",
        "icon.disabled": "#8a8a8a",
        "icon.placeholder": "#8a8a8a",
        "icon.accent": "#009fff",
        "link_text.hover": "#009fff",
        "error": "#d52c36",
        "error.background": "#d52c361a",
        "error.border": "#d52c364d",
        "warning": "#009fff",
        "warning.background": "#009fff1a",
        "warning.border": "#009fff4d",
        "success": "#18a46c",
        "success.background": "#18a46c1a",
        "success.border": "#18a46c4d",
        "info": "#1ca1c7",
        "info.background": "#1ca1c71a",
        "info.border": "#1ca1c74d",
        "hint": "#737373",
        "hint.background": "#7373731a",
        "hint.border": "#73737333",
        "predictive": "#8a8a8a",
        "predictive.background": "#8a8a8a1a",
        "predictive.border": "#8a8a8a33",
        "unreachable": "#8a8a8a",
        "unreachable.background": "#8a8a8a0d",
        "unreachable.border": "#8a8a8a1a",
        "created": "#18a46c",
        "created.background": "#18a46c1a",
        "created.border": "#18a46c4d",
        "modified": "#009fff",
        "modified.background": "#009fff1a",
        "modified.border": "#009fff4d",
        "deleted": "#d52c36",
        "deleted.background": "#d52c361a",
        "deleted.border": "#d52c364d",
        "conflict": "#693acf",
        "conflict.background": "#693acf1a",
        "conflict.border": "#693acf4d",
        "hidden": "#8a8a8a",
        "hidden.background": "#8a8a8a0d",
        "hidden.border": "#8a8a8a1a",
        "ignored": "#737373",
        "ignored.background": "#7373730d",
        "ignored.border": "#7373731a",
        "renamed": "#1ca1c7",
        "renamed.background": "#1ca1c71a",
        "renamed.border": "#1ca1c74d",
        "search.match_background": "#d5a9104d",
        "tab_bar.background": "#f5f5f5",
        "tab.active_background": "#f5f5f5",
        "tab.inactive_background": "#f5f5f5",
        "toolbar.background": "#f5f5f5",
        "title_bar.background": "#f5f5f5",
        "title_bar.inactive_background": "#f5f5f5",
        "panel.background": "#f5f5f5",
        "panel.focused_border": "#009fff",
        "status_bar.background": "#f5f5f5",
        "scrollbar.thumb.background": "#8a8a8a4d",
        "scrollbar.thumb.hover_background": "#8a8a8a80",
        "scrollbar.thumb.border": "transparent",
        "scrollbar.track.background": "transparent",
        "scrollbar.track.border": "transparent",
        "terminal.background": "#ffffff",
        "terminal.ansi.background": "#ffffff",
        "terminal.foreground": "#525252",
        "terminal.bright_foreground": "#0a0a0a",
        "terminal.dim_foreground": "#737373",
        "terminal.ansi.black": "#1d1d1d",
        "terminal.ansi.red": "#d32a61",
        "terminal.ansi.green": "#199f43",
        "terminal.ansi.yellow": "#d5a910",
        "terminal.ansi.blue": "#009fff",
        "terminal.ansi.magenta": "#a631be",
        "terminal.ansi.cyan": "#1ca1c7",
        "terminal.ansi.white": "#bcbcbc",
        "terminal.ansi.bright_black": "#1d1d1d",
        "terminal.ansi.bright_red": "#d5512f",
        "terminal.ansi.bright_green": "#18a46c",
        "terminal.ansi.bright_yellow": "#d5a910",
        "terminal.ansi.bright_blue": "#009fff",
        "terminal.ansi.bright_magenta": "#693acf",
        "terminal.ansi.bright_cyan": "#1ca1c7",
        "terminal.ansi.bright_white": "#bcbcbc",
        "terminal.ansi.dim_black": "#737373",
        "terminal.ansi.dim_red": "#8a2d43",
        "terminal.ansi.dim_green": "#307a43",
        "terminal.ansi.dim_yellow": "#8a721d",
        "terminal.ansi.dim_blue": "#1f6f9f",
        "terminal.ansi.dim_magenta": "#704085",
        "terminal.ansi.dim_cyan": "#347b8f",
        "terminal.ansi.dim_white": "#737373",
        "players": [
          {
            "cursor": "#009fff",
            "background": "#009fff",
            "selection": "#009fff40"
          },
          {
            "cursor": "#18a46c",
            "background": "#18a46c",
            "selection": "#18a46c40"
          },
          {
            "cursor": "#d32a61",
            "background": "#d32a61",
            "selection": "#d32a6140"
          },
          {
            "cursor": "#693acf",
            "background": "#693acf",
            "selection": "#693acf40"
          },
          {
            "cursor": "#199f43",
            "background": "#199f43",
            "selection": "#199f4340"
          },
          {
            "cursor": "#d5a910",
            "background": "#d5a910",
            "selection": "#d5a91040"
          },
          {
            "cursor": "#a631be",
            "background": "#a631be",
            "selection": "#a631be40"
          },
          {
            "cursor": "#1ca1c7",
            "background": "#1ca1c7",
            "selection": "#1ca1c740"
          }
        ],
        "syntax": {
          "comment": {
            "color": "#737373"
          },
          "comment.doc": {
            "color": "#737373"
          },
          "string": {
            "color": "#199f43"
          },
          "string.escape": {
            "color": "#16a994"
          },
          "string.regex": {
            "color": "#17a5af"
          },
          "string.special": {
            "color": "#16a994"
          },
          "string.special.symbol": {
            "color": "#d5a910"
          },
          "number": {
            "color": "#1ca1c7"
          },
          "constant": {
            "color": "#d5a910"
          },
          "boolean": {
            "color": "#1ca1c7"
          },
          "keyword": {
            "color": "#d32a61"
          },
          "keyword.operator": {
            "color": "#08c0ef"
          },
          "function": {
            "color": "#693acf"
          },
          "function.method": {
            "color": "#693acf"
          },
          "function.builtin": {
            "color": "#693acf"
          },
          "function.special.definition": {
            "color": "#693acf"
          },
          "function.call": {
            "color": "#693acf"
          },
          "type": {
            "color": "#a631be"
          },
          "type.builtin": {
            "color": "#a631be"
          },
          "constructor": {
            "color": "#a631be"
          },
          "variable": {
            "color": "#d47628"
          },
          "variable.builtin": {
            "color": "#d5901c"
          },
          "variable.member": {
            "color": "#d47628"
          },
          "variable.parameter": {
            "color": "#636363"
          },
          "variable.special": {
            "color": "#d5901c"
          },
          "property": {
            "color": "#d47628"
          },
          "property.css": {
            "color": "#009fff"
          },
          "property.definition": {
            "color": "#009fff"
          },
          "property_name": {
            "color": "#009fff"
          },
          "value": {
            "color": "#1ca1c7"
          },
          "constant.css": {
            "color": "#d5a910"
          },
          "string.plain": {
            "color": "#1ca1c7"
          },
          "plain_value": {
            "color": "#1ca1c7"
          },
          "tag.css": {
            "color": "#d5512f"
          },
          "tag_name": {
            "color": "#d5512f"
          },
          "class": {
            "color": "#18a46c"
          },
          "class_name": {
            "color": "#18a46c"
          },
          "selector.class": {
            "color": "#18a46c"
          },
          "selector.id": {
            "color": "#693acf"
          },
          "id_name": {
            "color": "#693acf"
          },
          "selector.pseudo": {
            "color": "#08c0ef"
          },
          "pseudo_class_selector": {
            "color": "#08c0ef"
          },
          "pseudo_element_selector": {
            "color": "#08c0ef"
          },
          "keyword.directive": {
            "color": "#d32a61"
          },
          "keyword.control.at-rule": {
            "color": "#d32a61"
          },
          "at_keyword": {
            "color": "#d32a61"
          },
          "variable.scss": {
            "color": "#d47628"
          },
          "variable.css": {
            "color": "#d47628"
          },
          "property.custom": {
            "color": "#d47628"
          },
          "unit": {
            "color": "#1ca1c7"
          },
          "number.unit": {
            "color": "#1ca1c7"
          },
          "color": {
            "color": "#d5a910"
          },
          "constant.color": {
            "color": "#d5a910"
          },
          "keyword.important": {
            "color": "#d32a61"
          },
          "variable.language": {
            "color": "#d5901c"
          },
          "this": {
            "color": "#d5901c"
          },
          "self": {
            "color": "#d5901c"
          },
          "type.class": {
            "color": "#a631be"
          },
          "property.object": {
            "color": "#d47628"
          },
          "property_identifier": {
            "color": "#d47628"
          },
          "shorthand_property_identifier": {
            "color": "#d47628"
          },
          "shorthand_property_identifier_pattern": {
            "color": "#d47628"
          },
          "method_definition": {
            "color": "#693acf"
          },
          "function.method.call": {
            "color": "#693acf"
          },
          "string.template": {
            "color": "#199f43"
          },
          "template_string": {
            "color": "#199f43"
          },
          "tag.jsx": {
            "color": "#d5512f"
          },
          "tag.component": {
            "color": "#a631be"
          },
          "punctuation": {
            "color": "#636363"
          },
          "punctuation.bracket": {
            "color": "#636363"
          },
          "punctuation.delimiter": {
            "color": "#636363"
          },
          "punctuation.list_marker": {
            "color": "#636363"
          },
          "punctuation.special": {
            "color": "#d32a61"
          },
          "operator": {
            "color": "#08c0ef"
          },
          "tag": {
            "color": "#d5512f"
          },
          "attribute": {
            "color": "#18a46c"
          },
          "label": {
            "color": "#d5901c"
          },
          "namespace": {
            "color": "#d5901c"
          },
          "decorator": {
            "color": "#1a85d4"
          },
          "attribute.builtin": {
            "color": "#1a85d4"
          },
          "embedded": {
            "color": "#0a0a0a"
          },
          "preproc": {
            "color": "#d32a61"
          },
          "text.literal": {
            "color": "#199f43"
          },
          "markup.heading": {
            "color": "#d5512f",
            "font_weight": 700
          },
          "markup.bold": {
            "color": "#d5a910",
            "font_weight": 700
          },
          "markup.italic": {
            "color": "#d32a61",
            "font_style": "italic"
          },
          "markup.strikethrough": {
            "color": "#737373"
          },
          "markup.link.url": {
            "color": "#009fff"
          },
          "markup.link.text": {
            "color": "#693acf"
          },
          "markup.quote": {
            "color": "#737373",
            "font_style": "italic"
          },
          "markup.list": {
            "color": "#d5512f"
          },
          "markup.list.numbered": {
            "color": "#d5512f"
          },
          "markup.list.unnumbered": {
            "color": "#d5512f"
          },
          "markup.raw": {
            "color": "#199f43"
          },
          "markup.raw.inline": {
            "color": "#199f43"
          },
          "markup.raw.block": {
            "color": "#199f43"
          },
          "diff.plus": {
            "color": "#18a46c"
          },
          "diff.minus": {
            "color": "#d52c36"
          },
          "diff.delta": {
            "color": "#d5a910"
          },
          "link_text": {
            "color": "#009fff"
          },
          "link_uri": {
            "color": "#d32a61"
          },
          "emphasis": {
            "font_style": "italic"
          },
          "emphasis.strong": {
            "font_weight": 700
          },
          "primary": {
            "color": "#009fff"
          },
          "title": {
            "color": "#d5512f",
            "font_weight": 700
          },
          "predictive": {
            "color": "#8a8a8a",
            "font_style": "italic"
          }
        }
      }
    },
    {
      "name": "Pierre Light Soft",
      "appearance": "light",
      "style": {
        "background": "#f7f7f7",
        "surface.background": "#f7f7f7",
        "elevated_surface.background": "#fafafa",
        "drop_target.background": "#009fff26",
        "editor.background": "#ffffff",
        "editor.foreground": "#525252",
        "editor.gutter.background": "#ffffff",
        "editor.active_line.background": "#dfebff8c",
        "editor.active_line_number": "#737373",
        "editor.line_number": "#8a8a8a",
        "editor.highlighted_line.background": "#dfebff59",
        "editor.indent_guide": "#ededed",
        "editor.indent_guide_active": "#e5e5e5",
        "editor.invisible": "#a3a3a3",
        "editor.wrap_guide": "#ededed",
        "editor.active_wrap_guide": "#e5e5e5",
        "editor.document_highlight.read_background": "#009fff1a",
        "editor.document_highlight.write_background": "#009fff2e",
        "editor.document_highlight.bracket_background": "#009fff33",
        "editor.subheader.background": "#f7f7f7",
        "text": "#525252",
        "text.muted": "#8a8a8a",
        "text.placeholder": "#a3a3a3",
        "text.disabled": "#a3a3a3",
        "text.accent": "#009fff",
        "border": "#e5e5e5",
        "border.variant": "#ededed",
        "border.focused": "#009fff",
        "border.selected": "#009fff",
        "border.transparent": "transparent",
        "border.disabled": "#d4d4d4",
        "element.background": "#f5f5f5",
        "element.hover": "#dfebff80",
        "element.active": "#dfebffb3",
        "element.selected": "#dfebffcc",
        "element.disabled": "#f5f5f580",
        "ghost_element.background": "transparent",
        "ghost_element.hover": "#dfebff59",
        "ghost_element.active": "#dfebff8c",
        "ghost_element.selected": "#dfebffa6",
        "ghost_element.disabled": "transparent",
        "icon": "#737373",
        "icon.muted": "#8a8a8a",
        "icon.disabled": "#a3a3a3",
        "icon.placeholder": "#a3a3a3",
        "icon.accent": "#009fff",
        "link_text.hover": "#009fff",
        "error": "#ff2e3f",
        "error.background": "#ff2e3f1a",
        "error.border": "#ff2e3f4d",
        "warning": "#009fff",
        "warning.background": "#009fff1a",
        "warning.border": "#009fff4d",
        "success": "#07c480",
        "success.background": "#07c4801a",
        "success.border": "#07c4804d",
        "info": "#08c0ef",
        "info.background": "#08c0ef1a",
        "info.border": "#08c0ef4d",
        "hint": "#8a8a8a",
        "hint.background": "#8a8a8a1a",
        "hint.border": "#8a8a8a33",
        "predictive": "#a3a3a3",
        "predictive.background": "#a3a3a31a",
        "predictive.border": "#a3a3a333",
        "unreachable": "#a3a3a3",
        "unreachable.background": "#a3a3a30d",
        "unreachable.border": "#a3a3a31a",
        "created": "#07c480",
        "created.background": "#07c4801a",
        "created.border": "#07c4804d",
        "modified": "#009fff",
        "modified.background": "#009fff1a",
        "modified.border": "#009fff4d",
        "deleted": "#ff2e3f",
        "deleted.background": "#ff2e3f1a",
        "deleted.border": "#ff2e3f4d",
        "conflict": "#7b43f8",
        "conflict.background": "#7b43f81a",
        "conflict.border": "#7b43f84d",
        "hidden": "#a3a3a3",
        "hidden.background": "#a3a3a30d",
        "hidden.border": "#a3a3a31a",
        "ignored": "#8a8a8a",
        "ignored.background": "#8a8a8a0d",
        "ignored.border": "#8a8a8a1a",
        "renamed": "#08c0ef",
        "renamed.background": "#08c0ef1a",
        "renamed.border": "#08c0ef4d",
        "search.match_background": "#ffca004d",
        "tab_bar.background": "#f7f7f7",
        "tab.active_background": "#f7f7f7",
        "tab.inactive_background": "#f7f7f7",
        "toolbar.background": "#f7f7f7",
        "title_bar.background": "#f7f7f7",
        "title_bar.inactive_background": "#f7f7f7",
        "panel.background": "#f7f7f7",
        "panel.focused_border": "#009fff",
        "status_bar.background": "#f7f7f7",
        "scrollbar.thumb.background": "#a3a3a34d",
        "scrollbar.thumb.hover_background": "#a3a3a380",
        "scrollbar.thumb.border": "transparent",
        "scrollbar.track.background": "transparent",
        "scrollbar.track.border": "transparent",
        "terminal.background": "#ffffff",
        "terminal.ansi.background": "#ffffff",
        "terminal.foreground": "#737373",
        "terminal.bright_foreground": "#525252",
        "terminal.dim_foreground": "#8a8a8a",
        "terminal.ansi.black": "#1d1d1d",
        "terminal.ansi.red": "#ff2e3f",
        "terminal.ansi.green": "#0dbe4e",
        "terminal.ansi.yellow": "#ffca00",
        "terminal.ansi.blue": "#009fff",
        "terminal.ansi.magenta": "#d568ea",
        "terminal.ansi.cyan": "#08c0ef",
        "terminal.ansi.white": "#bcbcbc",
        "terminal.ansi.bright_black": "#1d1d1d",
        "terminal.ansi.bright_red": "#ff5d36",
        "terminal.ansi.bright_green": "#07c480",
        "terminal.ansi.bright_yellow": "#ffca00",
        "terminal.ansi.bright_blue": "#009fff",
        "terminal.ansi.bright_magenta": "#9d6afb",
        "terminal.ansi.bright_cyan": "#08c0ef",
        "terminal.ansi.bright_white": "#bcbcbc",
        "terminal.ansi.dim_black": "#8a8a8a",
        "terminal.ansi.dim_red": "#a83b50",
        "terminal.ansi.dim_green": "#3d8a4e",
        "terminal.ansi.dim_yellow": "#a38300",
        "terminal.ansi.dim_blue": "#1f6f9f",
        "terminal.ansi.dim_magenta": "#7d4aa6",
        "terminal.ansi.dim_cyan": "#2d8395",
        "terminal.ansi.dim_white": "#8a8a8a",
        "players": [
          {
            "cursor": "#009fff",
            "background": "#009fff",
            "selection": "#009fff40"
          },
          {
            "cursor": "#07c480",
            "background": "#07c480",
            "selection": "#07c48040"
          },
          {
            "cursor": "#ff678d",
            "background": "#ff678d",
            "selection": "#ff678d40"
          },
          {
            "cursor": "#9d6afb",
            "background": "#9d6afb",
            "selection": "#9d6afb40"
          },
          {
            "cursor": "#0dbe4e",
            "background": "#0dbe4e",
            "selection": "#0dbe4e40"
          },
          {
            "cursor": "#ffca00",
            "background": "#ffca00",
            "selection": "#ffca0040"
          },
          {
            "cursor": "#d568ea",
            "background": "#d568ea",
            "selection": "#d568ea40"
          },
          {
            "cursor": "#08c0ef",
            "background": "#08c0ef",
            "selection": "#08c0ef40"
          }
        ],
        "syntax": {
          "comment": {
            "color": "#8a8a8a"
          },
          "comment.doc": {
            "color": "#8a8a8a"
          },
          "string": {
            "color": "#0dbe4e"
          },
          "string.escape": {
            "color": "#00cab1"
          },
          "string.regex": {
            "color": "#00c5d2"
          },
          "string.special": {
            "color": "#00cab1"
          },
          "string.special.symbol": {
            "color": "#ffca00"
          },
          "number": {
            "color": "#08c0ef"
          },
          "constant": {
            "color": "#ffca00"
          },
          "boolean": {
            "color": "#08c0ef"
          },
          "keyword": {
            "color": "#ff678d"
          },
          "keyword.operator": {
            "color": "#68cdf2"
          },
          "function": {
            "color": "#9d6afb"
          },
          "function.method": {
            "color": "#9d6afb"
          },
          "function.builtin": {
            "color": "#9d6afb"
          },
          "function.special.definition": {
            "color": "#9d6afb"
          },
          "function.call": {
            "color": "#9d6afb"
          },
          "type": {
            "color": "#d568ea"
          },
          "type.builtin": {
            "color": "#d568ea"
          },
          "constructor": {
            "color": "#d568ea"
          },
          "variable": {
            "color": "#fe8c2c"
          },
          "variable.builtin": {
            "color": "#ffab16"
          },
          "variable.member": {
            "color": "#fe8c2c"
          },
          "variable.parameter": {
            "color": "#737373"
          },
          "variable.special": {
            "color": "#ffab16"
          },
          "property": {
            "color": "#fe8c2c"
          },
          "property.css": {
            "color": "#009fff"
          },
          "property.definition": {
            "color": "#009fff"
          },
          "property_name": {
            "color": "#009fff"
          },
          "value": {
            "color": "#08c0ef"
          },
          "constant.css": {
            "color": "#ffca00"
          },
          "string.plain": {
            "color": "#08c0ef"
          },
          "plain_value": {
            "color": "#08c0ef"
          },
          "tag.css": {
            "color": "#ff5d36"
          },
          "tag_name": {
            "color": "#ff5d36"
          },
          "class": {
            "color": "#07c480"
          },
          "class_name": {
            "color": "#07c480"
          },
          "selector.class": {
            "color": "#07c480"
          },
          "selector.id": {
            "color": "#9d6afb"
          },
          "id_name": {
            "color": "#9d6afb"
          },
          "selector.pseudo": {
            "color": "#68cdf2"
          },
          "pseudo_class_selector": {
            "color": "#68cdf2"
          },
          "pseudo_element_selector": {
            "color": "#68cdf2"
          },
          "keyword.directive": {
            "color": "#ff678d"
          },
          "keyword.control.at-rule": {
            "color": "#ff678d"
          },
          "at_keyword": {
            "color": "#ff678d"
          },
          "variable.scss": {
            "color": "#fe8c2c"
          },
          "variable.css": {
            "color": "#fe8c2c"
          },
          "property.custom": {
            "color": "#fe8c2c"
          },
          "unit": {
            "color": "#08c0ef"
          },
          "number.unit": {
            "color": "#08c0ef"
          },
          "color": {
            "color": "#ffca00"
          },
          "constant.color": {
            "color": "#ffca00"
          },
          "keyword.important": {
            "color": "#ff678d"
          },
          "variable.language": {
            "color": "#ffab16"
          },
          "this": {
            "color": "#ffab16"
          },
          "self": {
            "color": "#ffab16"
          },
          "type.class": {
            "color": "#d568ea"
          },
          "property.object": {
            "color": "#fe8c2c"
          },
          "property_identifier": {
            "color": "#fe8c2c"
          },
          "shorthand_property_identifier": {
            "color": "#fe8c2c"
          },
          "shorthand_property_identifier_pattern": {
            "color": "#fe8c2c"
          },
          "method_definition": {
            "color": "#9d6afb"
          },
          "function.method.call": {
            "color": "#9d6afb"
          },
          "string.template": {
            "color": "#0dbe4e"
          },
          "template_string": {
            "color": "#0dbe4e"
          },
          "tag.jsx": {
            "color": "#ff5d36"
          },
          "tag.component": {
            "color": "#d568ea"
          },
          "punctuation": {
            "color": "#737373"
          },
          "punctuation.bracket": {
            "color": "#737373"
          },
          "punctuation.delimiter": {
            "color": "#737373"
          },
          "punctuation.list_marker": {
            "color": "#737373"
          },
          "punctuation.special": {
            "color": "#ff678d"
          },
          "operator": {
            "color": "#68cdf2"
          },
          "tag": {
            "color": "#ff5d36"
          },
          "attribute": {
            "color": "#07c480"
          },
          "label": {
            "color": "#ffab16"
          },
          "namespace": {
            "color": "#ffab16"
          },
          "decorator": {
            "color": "#69b1ff"
          },
          "attribute.builtin": {
            "color": "#69b1ff"
          },
          "embedded": {
            "color": "#525252"
          },
          "preproc": {
            "color": "#ff678d"
          },
          "text.literal": {
            "color": "#0dbe4e"
          },
          "markup.heading": {
            "color": "#ff5d36",
            "font_weight": 700
          },
          "markup.bold": {
            "color": "#ffca00",
            "font_weight": 700
          },
          "markup.italic": {
            "color": "#ff678d",
            "font_style": "italic"
          },
          "markup.strikethrough": {
            "color": "#8a8a8a"
          },
          "markup.link.url": {
            "color": "#009fff"
          },
          "markup.link.text": {
            "color": "#9d6afb"
          },
          "markup.quote": {
            "color": "#8a8a8a",
            "font_style": "italic"
          },
          "markup.list": {
            "color": "#ff5d36"
          },
          "markup.list.numbered": {
            "color": "#ff5d36"
          },
          "markup.list.unnumbered": {
            "color": "#ff5d36"
          },
          "markup.raw": {
            "color": "#0dbe4e"
          },
          "markup.raw.inline": {
            "color": "#0dbe4e"
          },
          "markup.raw.block": {
            "color": "#0dbe4e"
          },
          "diff.plus": {
            "color": "#07c480"
          },
          "diff.minus": {
            "color": "#ff2e3f"
          },
          "diff.delta": {
            "color": "#ffca00"
          },
          "link_text": {
            "color": "#009fff"
          },
          "link_uri": {
            "color": "#ff678d"
          },
          "emphasis": {
            "font_style": "italic"
          },
          "emphasis.strong": {
            "font_weight": 700
          },
          "primary": {
            "color": "#009fff"
          },
          "title": {
            "color": "#ff5d36",
            "font_weight": 700
          },
          "predictive": {
            "color": "#a3a3a3",
            "font_style": "italic"
          }
        }
      }
    },
    {
      "name": "Pierre Dark",
      "appearance": "dark",
      "style": {
        "background": "#171717",
        "surface.background": "#171717",
        "elevated_surface.background": "#101010",
        "drop_target.background": "#009fff26",
        "editor.background": "#0a0a0a",
        "editor.foreground": "#fafafa",
        "editor.gutter.background": "#0a0a0a",
        "editor.active_line.background": "#19283c8c",
        "editor.active_line_number": "#a3a3a3",
        "editor.line_number": "#737373",
        "editor.highlighted_line.background": "#19283c59",
        "editor.indent_guide": "#1d1d1d",
        "editor.indent_guide_active": "#262626",
        "editor.invisible": "#636363",
        "editor.wrap_guide": "#1d1d1d",
        "editor.active_wrap_guide": "#262626",
        "editor.document_highlight.read_background": "#009fff26",
        "editor.document_highlight.write_background": "#009fff40",
        "editor.document_highlight.bracket_background": "#009fff33",
        "editor.subheader.background": "#171717",
        "text": "#fafafa",
        "text.muted": "#737373",
        "text.placeholder": "#636363",
        "text.disabled": "#636363",
        "text.accent": "#009fff",
        "border": "#1d1d1d",
        "border.variant": "#262626",
        "border.focused": "#009fff",
        "border.selected": "#009fff",
        "border.transparent": "transparent",
        "border.disabled": "#262626",
        "element.background": "#1d1d1d",
        "element.hover": "#19283c80",
        "element.active": "#19283cb3",
        "element.selected": "#19283c99",
        "element.disabled": "#1d1d1d80",
        "ghost_element.background": "transparent",
        "ghost_element.hover": "#19283c59",
        "ghost_element.active": "#19283c8c",
        "ghost_element.selected": "#19283c80",
        "ghost_element.disabled": "transparent",
        "icon": "#a3a3a3",
        "icon.muted": "#737373",
        "icon.disabled": "#636363",
        "icon.placeholder": "#636363",
        "icon.accent": "#009fff",
        "link_text.hover": "#009fff",
        "error": "#ff2e3f",
        "error.background": "#ff2e3f1a",
        "error.border": "#ff2e3f4d",
        "warning": "#009fff",
        "warning.background": "#009fff1a",
        "warning.border": "#009fff4d",
        "success": "#07c480",
        "success.background": "#07c4801a",
        "success.border": "#07c4804d",
        "info": "#08c0ef",
        "info.background": "#08c0ef1a",
        "info.border": "#08c0ef4d",
        "hint": "#737373",
        "hint.background": "#7373731a",
        "hint.border": "#73737333",
        "predictive": "#636363",
        "predictive.background": "#6363631a",
        "predictive.border": "#63636333",
        "unreachable": "#636363",
        "unreachable.background": "#6363630d",
        "unreachable.border": "#6363631a",
        "created": "#07c480",
        "created.background": "#07c4801a",
        "created.border": "#07c4804d",
        "modified": "#009fff",
        "modified.background": "#009fff1a",
        "modified.border": "#009fff4d",
        "deleted": "#ff2e3f",
        "deleted.background": "#ff2e3f1a",
        "deleted.border": "#ff2e3f4d",
        "conflict": "#7b43f8",
        "conflict.background": "#7b43f81a",
        "conflict.border": "#7b43f84d",
        "hidden": "#636363",
        "hidden.background": "#6363630d",
        "hidden.border": "#6363631a",
        "ignored": "#737373",
        "ignored.background": "#7373730d",
        "ignored.border": "#7373731a",
        "renamed": "#08c0ef",
        "renamed.background": "#08c0ef1a",
        "renamed.border": "#08c0ef4d",
        "search.match_background": "#ffca004d",
        "tab_bar.background": "#171717",
        "tab.active_background": "#171717",
        "tab.inactive_background": "#171717",
        "toolbar.background": "#171717",
        "title_bar.background": "#171717",
        "title_bar.inactive_background": "#171717",
        "panel.background": "#171717",
        "panel.focused_border": "#009fff",
        "status_bar.background": "#171717",
        "scrollbar.thumb.background": "#6363634d",
        "scrollbar.thumb.hover_background": "#63636380",
        "scrollbar.thumb.border": "transparent",
        "scrollbar.track.background": "transparent",
        "scrollbar.track.border": "transparent",
        "terminal.background": "#0a0a0a",
        "terminal.ansi.background": "#0a0a0a",
        "terminal.foreground": "#a3a3a3",
        "terminal.bright_foreground": "#fafafa",
        "terminal.dim_foreground": "#737373",
        "terminal.ansi.black": "#171717",
        "terminal.ansi.red": "#ff678d",
        "terminal.ansi.green": "#5ecc71",
        "terminal.ansi.yellow": "#ffd452",
        "terminal.ansi.blue": "#009fff",
        "terminal.ansi.magenta": "#d568ea",
        "terminal.ansi.cyan": "#64d1db",
        "terminal.ansi.white": "#bcbcbc",
        "terminal.ansi.bright_black": "#737373",
        "terminal.ansi.bright_red": "#ff855e",
        "terminal.ansi.bright_green": "#60d199",
        "terminal.ansi.bright_yellow": "#ffd452",
        "terminal.ansi.bright_blue": "#009fff",
        "terminal.ansi.bright_magenta": "#9d6afb",
        "terminal.ansi.bright_cyan": "#68cdf2",
        "terminal.ansi.bright_white": "#bcbcbc",
        "terminal.ansi.dim_black": "#101010",
        "terminal.ansi.dim_red": "#9f3f56",
        "terminal.ansi.dim_green": "#3e8f50",
        "terminal.ansi.dim_yellow": "#a9892f",
        "terminal.ansi.dim_blue": "#0066a3",
        "terminal.ansi.dim_magenta": "#884aab",
        "terminal.ansi.dim_cyan": "#3e8990",
        "terminal.ansi.dim_white": "#737373",
        "players": [
          {
            "cursor": "#009fff",
            "background": "#009fff",
            "selection": "#009fff40"
          },
          {
            "cursor": "#07c480",
            "background": "#07c480",
            "selection": "#07c48040"
          },
          {
            "cursor": "#ff678d",
            "background": "#ff678d",
            "selection": "#ff678d40"
          },
          {
            "cursor": "#9d6afb",
            "background": "#9d6afb",
            "selection": "#9d6afb40"
          },
          {
            "cursor": "#5ecc71",
            "background": "#5ecc71",
            "selection": "#5ecc7140"
          },
          {
            "cursor": "#ffca00",
            "background": "#ffca00",
            "selection": "#ffca0040"
          },
          {
            "cursor": "#d568ea",
            "background": "#d568ea",
            "selection": "#d568ea40"
          },
          {
            "cursor": "#08c0ef",
            "background": "#08c0ef",
            "selection": "#08c0ef40"
          }
        ],
        "syntax": {
          "comment": {
            "color": "#737373"
          },
          "comment.doc": {
            "color": "#737373"
          },
          "string": {
            "color": "#5ecc71"
          },
          "string.escape": {
            "color": "#61d5c0"
          },
          "string.regex": {
            "color": "#64d1db"
          },
          "string.special": {
            "color": "#61d5c0"
          },
          "string.special.symbol": {
            "color": "#ffd452"
          },
          "number": {
            "color": "#68cdf2"
          },
          "constant": {
            "color": "#ffd452"
          },
          "boolean": {
            "color": "#68cdf2"
          },
          "keyword": {
            "color": "#ff678d"
          },
          "keyword.operator": {
            "color": "#08c0ef"
          },
          "function": {
            "color": "#9d6afb"
          },
          "function.method": {
            "color": "#9d6afb"
          },
          "function.builtin": {
            "color": "#9d6afb"
          },
          "function.special.definition": {
            "color": "#9d6afb"
          },
          "function.call": {
            "color": "#9d6afb"
          },
          "type": {
            "color": "#d568ea"
          },
          "type.builtin": {
            "color": "#d568ea"
          },
          "constructor": {
            "color": "#d568ea"
          },
          "variable": {
            "color": "#ffa359"
          },
          "variable.builtin": {
            "color": "#ffab16"
          },
          "variable.member": {
            "color": "#ffa359"
          },
          "variable.parameter": {
            "color": "#a3a3a3"
          },
          "variable.special": {
            "color": "#ffab16"
          },
          "property": {
            "color": "#ffa359"
          },
          "property.css": {
            "color": "#009fff"
          },
          "property.definition": {
            "color": "#009fff"
          },
          "property_name": {
            "color": "#009fff"
          },
          "value": {
            "color": "#68cdf2"
          },
          "constant.css": {
            "color": "#ffd452"
          },
          "string.plain": {
            "color": "#68cdf2"
          },
          "plain_value": {
            "color": "#68cdf2"
          },
          "tag.css": {
            "color": "#ff855e"
          },
          "tag_name": {
            "color": "#ff855e"
          },
          "class": {
            "color": "#60d199"
          },
          "class_name": {
            "color": "#60d199"
          },
          "selector.class": {
            "color": "#60d199"
          },
          "selector.id": {
            "color": "#9d6afb"
          },
          "id_name": {
            "color": "#9d6afb"
          },
          "selector.pseudo": {
            "color": "#08c0ef"
          },
          "pseudo_class_selector": {
            "color": "#08c0ef"
          },
          "pseudo_element_selector": {
            "color": "#08c0ef"
          },
          "keyword.directive": {
            "color": "#ff678d"
          },
          "keyword.control.at-rule": {
            "color": "#ff678d"
          },
          "at_keyword": {
            "color": "#ff678d"
          },
          "variable.scss": {
            "color": "#ffa359"
          },
          "variable.css": {
            "color": "#ffa359"
          },
          "property.custom": {
            "color": "#ffa359"
          },
          "unit": {
            "color": "#68cdf2"
          },
          "number.unit": {
            "color": "#68cdf2"
          },
          "color": {
            "color": "#ffd452"
          },
          "constant.color": {
            "color": "#ffd452"
          },
          "keyword.important": {
            "color": "#ff678d"
          },
          "variable.language": {
            "color": "#ffab16"
          },
          "this": {
            "color": "#ffab16"
          },
          "self": {
            "color": "#ffab16"
          },
          "type.class": {
            "color": "#d568ea"
          },
          "property.object": {
            "color": "#ffa359"
          },
          "property_identifier": {
            "color": "#ffa359"
          },
          "shorthand_property_identifier": {
            "color": "#ffa359"
          },
          "shorthand_property_identifier_pattern": {
            "color": "#ffa359"
          },
          "method_definition": {
            "color": "#9d6afb"
          },
          "function.method.call": {
            "color": "#9d6afb"
          },
          "string.template": {
            "color": "#5ecc71"
          },
          "template_string": {
            "color": "#5ecc71"
          },
          "tag.jsx": {
            "color": "#ff855e"
          },
          "tag.component": {
            "color": "#d568ea"
          },
          "punctuation": {
            "color": "#636363"
          },
          "punctuation.bracket": {
            "color": "#636363"
          },
          "punctuation.delimiter": {
            "color": "#636363"
          },
          "punctuation.list_marker": {
            "color": "#636363"
          },
          "punctuation.special": {
            "color": "#ff678d"
          },
          "operator": {
            "color": "#08c0ef"
          },
          "tag": {
            "color": "#ff855e"
          },
          "attribute": {
            "color": "#60d199"
          },
          "label": {
            "color": "#ffab16"
          },
          "namespace": {
            "color": "#ffab16"
          },
          "decorator": {
            "color": "#69b1ff"
          },
          "attribute.builtin": {
            "color": "#69b1ff"
          },
          "embedded": {
            "color": "#fafafa"
          },
          "preproc": {
            "color": "#ff678d"
          },
          "text.literal": {
            "color": "#5ecc71"
          },
          "markup.heading": {
            "color": "#ff855e",
            "font_weight": 700
          },
          "markup.bold": {
            "color": "#ffd452",
            "font_weight": 700
          },
          "markup.italic": {
            "color": "#ff678d",
            "font_style": "italic"
          },
          "markup.strikethrough": {
            "color": "#737373"
          },
          "markup.link.url": {
            "color": "#69b1ff"
          },
          "markup.link.text": {
            "color": "#9d6afb"
          },
          "markup.quote": {
            "color": "#737373",
            "font_style": "italic"
          },
          "markup.list": {
            "color": "#ff855e"
          },
          "markup.list.numbered": {
            "color": "#ff855e"
          },
          "markup.list.unnumbered": {
            "color": "#ff855e"
          },
          "markup.raw": {
            "color": "#5ecc71"
          },
          "markup.raw.inline": {
            "color": "#5ecc71"
          },
          "markup.raw.block": {
            "color": "#5ecc71"
          },
          "diff.plus": {
            "color": "#07c480"
          },
          "diff.minus": {
            "color": "#ff2e3f"
          },
          "diff.delta": {
            "color": "#ffca00"
          },
          "link_text": {
            "color": "#69b1ff"
          },
          "link_uri": {
            "color": "#ff678d"
          },
          "emphasis": {
            "font_style": "italic"
          },
          "emphasis.strong": {
            "font_weight": 700
          },
          "primary": {
            "color": "#69b1ff"
          },
          "title": {
            "color": "#ff855e",
            "font_weight": 700
          },
          "predictive": {
            "color": "#636363",
            "font_style": "italic"
          }
        }
      }
    },
    {
      "name": "Pierre Dark Soft",
      "appearance": "dark",
      "style": {
        "background": "#101010",
        "surface.background": "#101010",
        "elevated_surface.background": "#1d1d1d",
        "drop_target.background": "#69b1ff26",
        "editor.background": "#171717",
        "editor.foreground": "#d4d4d4",
        "editor.gutter.background": "#171717",
        "editor.active_line.background": "#1f3e5e8c",
        "editor.active_line_number": "#8a8a8a",
        "editor.line_number": "#636363",
        "editor.highlighted_line.background": "#1f3e5e59",
        "editor.indent_guide": "#262626",
        "editor.indent_guide_active": "#2c2c2c",
        "editor.invisible": "#525252",
        "editor.wrap_guide": "#262626",
        "editor.active_wrap_guide": "#2c2c2c",
        "editor.document_highlight.read_background": "#69b1ff26",
        "editor.document_highlight.write_background": "#69b1ff40",
        "editor.document_highlight.bracket_background": "#69b1ff33",
        "editor.subheader.background": "#101010",
        "text": "#d4d4d4",
        "text.muted": "#636363",
        "text.placeholder": "#525252",
        "text.disabled": "#525252",
        "text.accent": "#69b1ff",
        "border": "#262626",
        "border.variant": "#2c2c2c",
        "border.focused": "#69b1ff",
        "border.selected": "#69b1ff",
        "border.transparent": "transparent",
        "border.disabled": "#2c2c2c",
        "element.background": "#262626",
        "element.hover": "#1f3e5e80",
        "element.active": "#1f3e5eb3",
        "element.selected": "#1f3e5e99",
        "element.disabled": "#26262680",
        "ghost_element.background": "transparent",
        "ghost_element.hover": "#1f3e5e59",
        "ghost_element.active": "#1f3e5e8c",
        "ghost_element.selected": "#1f3e5e80",
        "ghost_element.disabled": "transparent",
        "icon": "#8a8a8a",
        "icon.muted": "#636363",
        "icon.disabled": "#525252",
        "icon.placeholder": "#525252",
        "icon.accent": "#69b1ff",
        "link_text.hover": "#69b1ff",
        "error": "#ff6762",
        "error.background": "#ff67621a",
        "error.border": "#ff67624d",
        "warning": "#69b1ff",
        "warning.background": "#69b1ff1a",
        "warning.border": "#69b1ff4d",
        "success": "#60d199",
        "success.background": "#60d1991a",
        "success.border": "#60d1994d",
        "info": "#68cdf2",
        "info.background": "#68cdf21a",
        "info.border": "#68cdf24d",
        "hint": "#636363",
        "hint.background": "#6363631a",
        "hint.border": "#63636333",
        "predictive": "#525252",
        "predictive.background": "#5252521a",
        "predictive.border": "#52525233",
        "unreachable": "#525252",
        "unreachable.background": "#5252520d",
        "unreachable.border": "#5252521a",
        "created": "#60d199",
        "created.background": "#60d1991a",
        "created.border": "#60d1994d",
        "modified": "#69b1ff",
        "modified.background": "#69b1ff1a",
        "modified.border": "#69b1ff4d",
        "deleted": "#ff6762",
        "deleted.background": "#ff67621a",
        "deleted.border": "#ff67624d",
        "conflict": "#9d6afb",
        "conflict.background": "#9d6afb1a",
        "conflict.border": "#9d6afb4d",
        "hidden": "#525252",
        "hidden.background": "#5252520d",
        "hidden.border": "#5252521a",
        "ignored": "#636363",
        "ignored.background": "#6363630d",
        "ignored.border": "#6363631a",
        "renamed": "#68cdf2",
        "renamed.background": "#68cdf21a",
        "renamed.border": "#68cdf24d",
        "search.match_background": "#ffd4524d",
        "tab_bar.background": "#101010",
        "tab.active_background": "#101010",
        "tab.inactive_background": "#101010",
        "toolbar.background": "#101010",
        "title_bar.background": "#101010",
        "title_bar.inactive_background": "#101010",
        "panel.background": "#101010",
        "panel.focused_border": "#69b1ff",
        "status_bar.background": "#101010",
        "scrollbar.thumb.background": "#5252524d",
        "scrollbar.thumb.hover_background": "#52525280",
        "scrollbar.thumb.border": "transparent",
        "scrollbar.track.background": "transparent",
        "scrollbar.track.border": "transparent",
        "terminal.background": "#171717",
        "terminal.ansi.background": "#171717",
        "terminal.foreground": "#8a8a8a",
        "terminal.bright_foreground": "#d4d4d4",
        "terminal.dim_foreground": "#636363",
        "terminal.ansi.black": "#262626",
        "terminal.ansi.red": "#ff91a8",
        "terminal.ansi.green": "#8cda94",
        "terminal.ansi.yellow": "#ffde80",
        "terminal.ansi.blue": "#69b1ff",
        "terminal.ansi.magenta": "#e290f0",
        "terminal.ansi.cyan": "#92dde4",
        "terminal.ansi.white": "#bcbcbc",
        "terminal.ansi.bright_black": "#636363",
        "terminal.ansi.bright_red": "#ffa685",
        "terminal.ansi.bright_green": "#8eddb2",
        "terminal.ansi.bright_yellow": "#ffde80",
        "terminal.ansi.bright_blue": "#69b1ff",
        "terminal.ansi.bright_magenta": "#ba8ffd",
        "terminal.ansi.bright_cyan": "#96d9f6",
        "terminal.ansi.bright_white": "#bcbcbc",
        "terminal.ansi.dim_black": "#262626",
        "terminal.ansi.dim_red": "#a65d70",
        "terminal.ansi.dim_green": "#5d9666",
        "terminal.ansi.dim_yellow": "#a68f52",
        "terminal.ansi.dim_blue": "#466f9f",
        "terminal.ansi.dim_magenta": "#8a5fac",
        "terminal.ansi.dim_cyan": "#5f9298",
        "terminal.ansi.dim_white": "#636363",
        "players": [
          {
            "cursor": "#69b1ff",
            "background": "#69b1ff",
            "selection": "#69b1ff40"
          },
          {
            "cursor": "#60d199",
            "background": "#60d199",
            "selection": "#60d19940"
          },
          {
            "cursor": "#ff91a8",
            "background": "#ff91a8",
            "selection": "#ff91a840"
          },
          {
            "cursor": "#ba8ffd",
            "background": "#ba8ffd",
            "selection": "#ba8ffd40"
          },
          {
            "cursor": "#8cda94",
            "background": "#8cda94",
            "selection": "#8cda9440"
          },
          {
            "cursor": "#ffd452",
            "background": "#ffd452",
            "selection": "#ffd45240"
          },
          {
            "cursor": "#e290f0",
            "background": "#e290f0",
            "selection": "#e290f040"
          },
          {
            "cursor": "#68cdf2",
            "background": "#68cdf2",
            "selection": "#68cdf240"
          }
        ],
        "syntax": {
          "comment": {
            "color": "#636363"
          },
          "comment.doc": {
            "color": "#636363"
          },
          "string": {
            "color": "#8cda94"
          },
          "string.escape": {
            "color": "#8fe0d0"
          },
          "string.regex": {
            "color": "#92dde4"
          },
          "string.special": {
            "color": "#8fe0d0"
          },
          "string.special.symbol": {
            "color": "#ffde80"
          },
          "number": {
            "color": "#96d9f6"
          },
          "constant": {
            "color": "#ffde80"
          },
          "boolean": {
            "color": "#96d9f6"
          },
          "keyword": {
            "color": "#ff91a8"
          },
          "keyword.operator": {
            "color": "#68cdf2"
          },
          "function": {
            "color": "#ba8ffd"
          },
          "function.method": {
            "color": "#ba8ffd"
          },
          "function.builtin": {
            "color": "#ba8ffd"
          },
          "function.special.definition": {
            "color": "#ba8ffd"
          },
          "function.call": {
            "color": "#ba8ffd"
          },
          "type": {
            "color": "#e290f0"
          },
          "type.builtin": {
            "color": "#e290f0"
          },
          "constructor": {
            "color": "#e290f0"
          },
          "variable": {
            "color": "#ffba82"
          },
          "variable.builtin": {
            "color": "#ffbc56"
          },
          "variable.member": {
            "color": "#ffba82"
          },
          "variable.parameter": {
            "color": "#8a8a8a"
          },
          "variable.special": {
            "color": "#ffbc56"
          },
          "property": {
            "color": "#ffba82"
          },
          "property.css": {
            "color": "#69b1ff"
          },
          "property.definition": {
            "color": "#69b1ff"
          },
          "property_name": {
            "color": "#69b1ff"
          },
          "value": {
            "color": "#96d9f6"
          },
          "constant.css": {
            "color": "#ffde80"
          },
          "string.plain": {
            "color": "#96d9f6"
          },
          "plain_value": {
            "color": "#96d9f6"
          },
          "tag.css": {
            "color": "#ffa685"
          },
          "tag_name": {
            "color": "#ffa685"
          },
          "class": {
            "color": "#8eddb2"
          },
          "class_name": {
            "color": "#8eddb2"
          },
          "selector.class": {
            "color": "#8eddb2"
          },
          "selector.id": {
            "color": "#ba8ffd"
          },
          "id_name": {
            "color": "#ba8ffd"
          },
          "selector.pseudo": {
            "color": "#68cdf2"
          },
          "pseudo_class_selector": {
            "color": "#68cdf2"
          },
          "pseudo_element_selector": {
            "color": "#68cdf2"
          },
          "keyword.directive": {
            "color": "#ff91a8"
          },
          "keyword.control.at-rule": {
            "color": "#ff91a8"
          },
          "at_keyword": {
            "color": "#ff91a8"
          },
          "variable.scss": {
            "color": "#ffba82"
          },
          "variable.css": {
            "color": "#ffba82"
          },
          "property.custom": {
            "color": "#ffba82"
          },
          "unit": {
            "color": "#96d9f6"
          },
          "number.unit": {
            "color": "#96d9f6"
          },
          "color": {
            "color": "#ffde80"
          },
          "constant.color": {
            "color": "#ffde80"
          },
          "keyword.important": {
            "color": "#ff91a8"
          },
          "variable.language": {
            "color": "#ffbc56"
          },
          "this": {
            "color": "#ffbc56"
          },
          "self": {
            "color": "#ffbc56"
          },
          "type.class": {
            "color": "#e290f0"
          },
          "property.object": {
            "color": "#ffba82"
          },
          "property_identifier": {
            "color": "#ffba82"
          },
          "shorthand_property_identifier": {
            "color": "#ffba82"
          },
          "shorthand_property_identifier_pattern": {
            "color": "#ffba82"
          },
          "method_definition": {
            "color": "#ba8ffd"
          },
          "function.method.call": {
            "color": "#ba8ffd"
          },
          "string.template": {
            "color": "#8cda94"
          },
          "template_string": {
            "color": "#8cda94"
          },
          "tag.jsx": {
            "color": "#ffa685"
          },
          "tag.component": {
            "color": "#e290f0"
          },
          "punctuation": {
            "color": "#737373"
          },
          "punctuation.bracket": {
            "color": "#737373"
          },
          "punctuation.delimiter": {
            "color": "#737373"
          },
          "punctuation.list_marker": {
            "color": "#737373"
          },
          "punctuation.special": {
            "color": "#ff91a8"
          },
          "operator": {
            "color": "#68cdf2"
          },
          "tag": {
            "color": "#ffa685"
          },
          "attribute": {
            "color": "#8eddb2"
          },
          "label": {
            "color": "#ffbc56"
          },
          "namespace": {
            "color": "#ffbc56"
          },
          "decorator": {
            "color": "#97c4ff"
          },
          "attribute.builtin": {
            "color": "#97c4ff"
          },
          "embedded": {
            "color": "#d4d4d4"
          },
          "preproc": {
            "color": "#ff91a8"
          },
          "text.literal": {
            "color": "#8cda94"
          },
          "markup.heading": {
            "color": "#ffa685",
            "font_weight": 700
          },
          "markup.bold": {
            "color": "#ffde80",
            "font_weight": 700
          },
          "markup.italic": {
            "color": "#ff91a8",
            "font_style": "italic"
          },
          "markup.strikethrough": {
            "color": "#636363"
          },
          "markup.link.url": {
            "color": "#69b1ff"
          },
          "markup.link.text": {
            "color": "#ba8ffd"
          },
          "markup.quote": {
            "color": "#636363",
            "font_style": "italic"
          },
          "markup.list": {
            "color": "#ffa685"
          },
          "markup.list.numbered": {
            "color": "#ffa685"
          },
          "markup.list.unnumbered": {
            "color": "#ffa685"
          },
          "markup.raw": {
            "color": "#8cda94"
          },
          "markup.raw.inline": {
            "color": "#8cda94"
          },
          "markup.raw.block": {
            "color": "#8cda94"
          },
          "diff.plus": {
            "color": "#60d199"
          },
          "diff.minus": {
            "color": "#ff6762"
          },
          "diff.delta": {
            "color": "#ffd452"
          },
          "link_text": {
            "color": "#69b1ff"
          },
          "link_uri": {
            "color": "#ff91a8"
          },
          "emphasis": {
            "font_style": "italic"
          },
          "emphasis.strong": {
            "font_weight": 700
          },
          "primary": {
            "color": "#69b1ff"
          },
          "title": {
            "color": "#ffa685",
            "font_weight": 700
          },
          "predictive": {
            "color": "#525252",
            "font_style": "italic"
          }
        }
      }
    }
  ]
};
