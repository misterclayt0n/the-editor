import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "iroaseta",
  "title": "Iroaseta",
  "palette": {
    "amber_shadow": "#0d1117",
    "midnight_black": "#010409",
    "dark_steel": "#161b22",
    "pitch_black": "#000000",
    "misty_white": "#f2f0eb",
    "slate_gray": "#838a97",
    "ethereal_gray": "#91a3b0",
    "blaze_orange": "#ff9000",
    "lemon_zest": "#ffba00",
    "leafy_green": "#81be83",
    "dreamy_blue": "#6eb0ff",
    "crystal_blue": "#99c7ff",
    "twilight_slate": "#263c57",
    "sky_blue": "#45b1e8",
    "rustic_red": "#540b0c",
    "ruby_glow": "#fa7970",
    "walnut_brown": "#987654",
    "rustic_amber": "#9d5800",
    "slate_purple": "#d2a8ff",
    "light_purple": "#7533bd",
    "deep_purple": "#4c1785",
    "black": "#0d1117",
    "red": "#fa7970",
    "green": "#81be83",
    "yellow": "#ffba00",
    "orange": "#ff9000",
    "blue": "#45b1e8",
    "purple": "#d2a8ff",
    "brown": "#987654",
    "gray": "#838a97",
    "white": "#dadada"
  },
  "styles": {
    "attribute": {
      "fg": "yellow"
    },
    "type": {
      "fg": "white"
    },
    "type.builtin": {
      "fg": "white"
    },
    "type.parameter": {
      "fg": "white"
    },
    "type.enum": {
      "fg": "white"
    },
    "type.enum.variant": {
      "fg": "white"
    },
    "constructor": {
      "fg": "orange"
    },
    "constant": {
      "fg": "blue"
    },
    "constant.character.escape": {
      "fg": "yellow"
    },
    "string": {
      "fg": "blue"
    },
    "string.regexp": {
      "fg": "yellow"
    },
    "comment": {
      "fg": "gray"
    },
    "variable": {
      "fg": "orange"
    },
    "variable.parameter": {
      "fg": "yellow"
    },
    "variable.other": {
      "fg": "green"
    },
    "variable.other.member": {
      "fg": "green"
    },
    "label": {
      "fg": "blue"
    },
    "punctuation": {
      "fg": "white"
    },
    "punctuation.bracket": {
      "fg": "orange"
    },
    "punctuation.special": {
      "fg": "yellow"
    },
    "keyword": {
      "fg": "red"
    },
    "keyword.operator": {
      "fg": "blue"
    },
    "keyword.directive": {
      "fg": "white"
    },
    "keyword.function": {
      "fg": "red"
    },
    "keyword.storage": {
      "fg": "red"
    },
    "keyword.storage.modifier": {
      "fg": "green"
    },
    "operator": {
      "fg": "white"
    },
    "function": {
      "fg": "purple"
    },
    "function.method": {
      "fg": "green"
    },
    "function.macro": {
      "fg": "green"
    },
    "function.special": {
      "fg": "yellow"
    },
    "tag": {
      "fg": "green"
    },
    "namespace": {
      "fg": "white"
    },
    "diff": {
      "fg": "white"
    },
    "diff.minus": {
      "fg": "red"
    },
    "diff.delta": {
      "fg": "brown"
    },
    "markup.heading": {
      "fg": "blaze_orange"
    },
    "markup.heading.1": {
      "fg": "crystal_blue",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.2": {
      "fg": "sky_blue",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.3": {
      "fg": "dreamy_blue",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.4": {
      "fg": "crystal_blue"
    },
    "markup.heading.5": {
      "fg": "sky_blue"
    },
    "markup.heading.6": {
      "fg": "dreamy_blue"
    },
    "markup.list": {
      "fg": "blaze_orange"
    },
    "markup.bold": {
      "fg": "crystal_blue",
      "modifiers": [
        "bold"
      ]
    },
    "markup.italic": {
      "fg": "crystal_blue",
      "modifiers": [
        "italic"
      ]
    },
    "markup.strikethrough": {
      "fg": "crystal_blue",
      "modifiers": [
        "crossed_out"
      ]
    },
    "markup.link": {
      "fg": "crystal_blue",
      "underline": {
        "color": "light_purple",
        "style": "line"
      }
    },
    "markup.link.url": {
      "fg": "slate_purple",
      "underline": {
        "color": "slate_purple",
        "style": "line"
      }
    },
    "markup.link.label": {
      "fg": "crystal_blue"
    },
    "markup.link.text": {
      "fg": "crystal_blue",
      "modifiers": [
        "bold"
      ]
    },
    "markup.quote": {
      "fg": "misty_white",
      "modifiers": [
        "italic"
      ]
    },
    "markup.raw": {
      "fg": "misty_white"
    },
    "markup.raw.block": {
      "fg": "white"
    },
    "ui.background": {
      "bg": "amber_shadow"
    },
    "ui.background.separator": {
      "fg": "misty_white"
    },
    "ui.gutter": {
      "bg": "amber_shadow"
    },
    "ui.gutter.selected": {
      "bg": "pitch_black"
    },
    "ui.linenr": {
      "fg": "slate_gray"
    },
    "ui.linenr.selected": {
      "fg": "blaze_orange",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline": {
      "fg": "misty_white",
      "bg": "pitch_black"
    },
    "ui.statusline.inactive": {
      "fg": "slate_gray",
      "bg": "dark_steel"
    },
    "ui.statusline.normal": {
      "fg": "amber_shadow",
      "bg": "leafy_green",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.insert": {
      "fg": "amber_shadow",
      "bg": "blaze_orange",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.select": {
      "fg": "amber_shadow",
      "bg": "sky_blue",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.separator": {
      "fg": "misty_white"
    },
    "ui.bufferline": {
      "fg": "slate_gray",
      "modifiers": [
        "bold"
      ]
    },
    "ui.bufferline.active": {
      "fg": "misty_white",
      "bg": "rustic_red"
    },
    "ui.bufferline.background": {
      "bg": "pitch_black"
    },
    "ui.virtual.ruler": {
      "bg": "rustic_red"
    },
    "ui.virtual.whitespace": {
      "fg": "brown"
    },
    "ui.virtual.indent-guide": {
      "fg": "brown"
    },
    "ui.virtual.inlay-hint": {
      "fg": "slate_gray"
    },
    "ui.virtual.inlay-hint.parameter": {
      "fg": "slate_gray"
    },
    "ui.virtual.inlay-hint.type": {
      "fg": "slate_gray"
    },
    "ui.virtual.wrap": {
      "fg": "slate_gray"
    },
    "ui.virtual.jump-label": {
      "modifiers": [
        "reversed"
      ]
    },
    "ui.selection": {
      "bg": "deep_purple"
    },
    "ui.selection.primary": {
      "bg": "light_purple"
    },
    "ui.cursor": {
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.normal": {
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.insert": {
      "bg": "rustic_red"
    },
    "ui.cursor.select": {
      "bg": "deep_purple"
    },
    "ui.cursor.match": {
      "fg": "amber_shadow",
      "bg": "blaze_orange",
      "modifiers": [
        "bold"
      ]
    },
    "ui.cursor.primary": {
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.primary.normal": {
      "modifiers": [
        "reversed"
      ]
    },
    "ui.cursor.primary.insert": {
      "fg": "amber_shadow",
      "bg": "ruby_glow"
    },
    "ui.cursor.primary.select": {
      "fg": "misty_white",
      "bg": "deep_purple"
    },
    "ui.cursorline.primary": {
      "bg": "dark_steel"
    },
    "ui.cursorline.secondary": {
      "bg": "midnight_black"
    },
    "ui.cursorcolumn.primary": {
      "bg": "dark_steel"
    },
    "ui.cursorcolumn.secondary": {
      "bg": "midnight_black"
    },
    "ui.popup": {
      "fg": "misty_white",
      "bg": "midnight_black"
    },
    "ui.popup.info": {
      "fg": "misty_white",
      "bg": "midnight_black"
    },
    "ui.window": {
      "fg": "pitch_black"
    },
    "ui.help": {
      "fg": "misty_white",
      "bg": "midnight_black"
    },
    "ui.text": {
      "fg": "white"
    },
    "ui.text.focus": {
      "fg": "leafy_green"
    },
    "ui.text.inactive": {
      "fg": "slate_gray"
    },
    "ui.text.info": {
      "fg": "misty_white",
      "bg": "midnight_black"
    },
    "ui.menu": {
      "fg": "ethereal_gray",
      "bg": "pitch_black"
    },
    "ui.menu.selected": {
      "fg": "misty_white",
      "bg": "dark_steel"
    },
    "ui.menu.scroll": {
      "fg": "slate_gray",
      "bg": "midnight_black"
    },
    "ui.highlight": {
      "underline": {
        "color": "blaze_orange",
        "style": "line"
      }
    },
    "warning": {
      "fg": "lemon_zest"
    },
    "error": {
      "fg": "ruby_glow"
    },
    "info": {
      "fg": "sky_blue"
    },
    "hint": {
      "fg": "walnut_brown"
    },
    "diagnostic": {
      "modifiers": [
        "reversed"
      ]
    },
    "diagnostic.hint": {
      "fg": "amber_shadow",
      "bg": "walnut_brown"
    },
    "diagnostic.info": {
      "fg": "misty_white",
      "bg": "twilight_slate"
    },
    "diagnostic.warning": {
      "fg": "misty_white",
      "bg": "rustic_amber"
    },
    "diagnostic.error": {
      "fg": "misty_white",
      "bg": "rustic_red"
    }
  }
});
