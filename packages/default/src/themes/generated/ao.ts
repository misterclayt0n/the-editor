import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "ao",
  "title": "Ao",
  "palette": {
    "deep_abyss": "#080d15",
    "stormy_night": "#254862",
    "nightfall_blue": "#1f2937",
    "midnight_thunder": "#0d1526",
    "twilight_blue": "#2c5484",
    "pitch_black": "#000000",
    "winter_sky": "#f3f4f6",
    "slate_gray": "#838a97",
    "blaze_orange": "#ff9000",
    "lemon_zest": "#ffba00",
    "leafy_green": "#81be83",
    "dreamy_blue": "#6eb0ff",
    "crystal_blue": "#99c7ff",
    "sky_blue": "#45b1e8",
    "moonlight_ocean": "#0c1420",
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
      "fg": "winter_sky",
      "modifiers": [
        "italic"
      ]
    },
    "markup.raw": {
      "fg": "winter_sky"
    },
    "markup.raw.block": {
      "fg": "white"
    },
    "ui.background": {
      "bg": "deep_abyss"
    },
    "ui.background.separator": {
      "fg": "winter_sky"
    },
    "ui.gutter": {
      "bg": "deep_abyss"
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
      "fg": "winter_sky",
      "bg": "twilight_blue"
    },
    "ui.statusline.inactive": {
      "fg": "slate_gray",
      "bg": "pitch_black"
    },
    "ui.statusline.normal": {
      "fg": "deep_abyss",
      "bg": "leafy_green",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.insert": {
      "fg": "deep_abyss",
      "bg": "blaze_orange",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.select": {
      "fg": "deep_abyss",
      "bg": "sky_blue",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.separator": {
      "fg": "winter_sky"
    },
    "ui.bufferline": {
      "fg": "slate_gray",
      "modifiers": [
        "bold"
      ]
    },
    "ui.bufferline.active": {
      "fg": "winter_sky",
      "bg": "twilight_blue"
    },
    "ui.bufferline.background": {
      "bg": "pitch_black"
    },
    "ui.virtual.ruler": {
      "bg": "stormy_night"
    },
    "ui.virtual.whitespace": {
      "fg": "stormy_night"
    },
    "ui.virtual.indent-guide": {
      "fg": "stormy_night"
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
      "fg": "winter_sky",
      "bg": "twilight_blue"
    },
    "ui.cursor.insert": {
      "bg": "rustic_red"
    },
    "ui.cursor.select": {
      "bg": "deep_purple"
    },
    "ui.cursor.match": {
      "fg": "deep_abyss",
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
      "fg": "deep_abyss",
      "bg": "sky_blue"
    },
    "ui.cursor.primary.insert": {
      "fg": "deep_abyss",
      "bg": "ruby_glow"
    },
    "ui.cursor.primary.select": {
      "fg": "winter_sky",
      "bg": "deep_purple"
    },
    "ui.cursorline.primary": {
      "bg": "nightfall_blue"
    },
    "ui.cursorline.secondary": {
      "bg": "midnight_thunder"
    },
    "ui.cursorcolumn.primary": {
      "bg": "nightfall_blue"
    },
    "ui.cursorcolumn.secondary": {
      "bg": "midnight_thunder"
    },
    "ui.popup": {
      "fg": "winter_sky",
      "bg": "midnight_thunder"
    },
    "ui.popup.info": {
      "fg": "winter_sky",
      "bg": "nightfall_blue"
    },
    "ui.window": {
      "fg": "slate_gray"
    },
    "ui.help": {
      "fg": "winter_sky",
      "bg": "nightfall_blue"
    },
    "ui.text": {
      "fg": "white"
    },
    "ui.text.focus": {
      "fg": "dreamy_blue"
    },
    "ui.text.inactive": {
      "fg": "slate_gray"
    },
    "ui.text.info": {
      "fg": "winter_sky",
      "bg": "nightfall_blue"
    },
    "ui.menu": {
      "fg": "winter_sky",
      "bg": "midnight_thunder"
    },
    "ui.menu.selected": {
      "fg": "winter_sky",
      "bg": "twilight_blue"
    },
    "ui.menu.scroll": {
      "fg": "crystal_blue",
      "bg": "moonlight_ocean"
    },
    "ui.highlight": {
      "underline": {
        "color": "sky_blue",
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
      "fg": "deep_abyss",
      "bg": "walnut_brown"
    },
    "diagnostic.info": {
      "fg": "winter_sky",
      "bg": "twilight_blue"
    },
    "diagnostic.warning": {
      "fg": "winter_sky",
      "bg": "rustic_amber"
    },
    "diagnostic.error": {
      "fg": "winter_sky",
      "bg": "rustic_red"
    }
  }
});
