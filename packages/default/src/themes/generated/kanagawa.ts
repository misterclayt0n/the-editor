import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "kanagawa",
  "title": "Kanagawa",
  "palette": {
    "oldWhite": "#C8C093",
    "fujiWhite": "#DCD7BA",
    "fujiGray": "#727169",
    "sumiInk0": "#16161D",
    "sumiInk1": "#181820",
    "sumiInk2": "#1A1A22",
    "sumiInk3": "#1F1F28",
    "sumiInk4": "#2A2A37",
    "sumiInk5": "#363646",
    "sumiInk6": "#54546D",
    "waveBlue1": "#223249",
    "waveBlue2": "#2D4F67",
    "winterGreen": "#2B3328",
    "winterYellow": "#49443C",
    "winterRed": "#43242B",
    "winterBlue": "#252535",
    "autumnGreen": "#76946A",
    "autumnRed": "#C34043",
    "autumnYellow": "#DCA561",
    "samuraiRed": "#E82424",
    "roninYellow": "#FF9E3B",
    "waveAqua1": "#6A9589",
    "dragonBlue": "#658594",
    "oniViolet": "#957FB8",
    "oniViolet2": "#B8B4D0",
    "crystalBlue": "#7E9CD8",
    "springViolet1": "#938AA9",
    "springViolet2": "#9CABCA",
    "springBlue": "#7FB4CA",
    "lightBlue": "#A3D4D5",
    "waveAqua2": "#7AA89F",
    "waveAqua3": "#68AD99",
    "waveAqua4": "#7AA880",
    "waveAqua5": "#6CAF95",
    "springGreen": "#98BB6C",
    "boatYellow1": "#938056",
    "boatYellow2": "#C0A36E",
    "carpYellow": "#E6C384",
    "sakuraPink": "#D27E99",
    "waveRed": "#E46876",
    "peachRed": "#FF5D62",
    "surimiOrange": "#FFA066",
    "katanaGray": "#717C7C"
  },
  "styles": {
    "ui.selection": {
      "bg": "waveBlue2"
    },
    "ui.selection.primary": {
      "bg": "waveBlue2"
    },
    "ui.background": {
      "fg": "fujiWhite",
      "bg": "sumiInk3"
    },
    "ui.linenr": {
      "fg": "sumiInk6"
    },
    "ui.linenr.selected": {
      "fg": "roninYellow",
      "modifiers": [
        "bold"
      ]
    },
    "ui.gutter": {
      "fg": "sumiInk6",
      "bg": "sumiInk4"
    },
    "ui.virtual": "sumiInk6",
    "ui.virtual.ruler": {
      "bg": "sumiInk4"
    },
    "ui.virtual.inlay-hint": "sumiInk6",
    "ui.virtual.jump-label": {
      "fg": "peachRed",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline": {
      "fg": "oldWhite",
      "bg": "sumiInk0"
    },
    "ui.statusline.inactive": {
      "fg": "fujiGray",
      "bg": "sumiInk0"
    },
    "ui.statusline.normal": {
      "fg": "sumiInk0",
      "bg": "crystalBlue",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.insert": {
      "fg": "sumiInk0",
      "bg": "autumnGreen",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline.select": {
      "fg": "sumiInk0",
      "bg": "oniViolet",
      "modifiers": [
        "bold"
      ]
    },
    "ui.bufferline": {
      "fg": "fujiGray",
      "bg": "sumiInk0"
    },
    "ui.bufferline.active": {
      "fg": "oldWhite",
      "bg": "sumiInk0"
    },
    "ui.bufferline.background": {
      "bg": "sumiInk0"
    },
    "ui.popup": {
      "fg": "fujiWhite",
      "bg": "sumiInk0"
    },
    "ui.window": {
      "fg": "sumiInk0"
    },
    "ui.help": {
      "fg": "fujiWhite",
      "bg": "sumiInk0"
    },
    "ui.text": "fujiWhite",
    "ui.text.focus": {
      "fg": "fujiWhite",
      "bg": "waveBlue2",
      "modifiers": [
        "bold"
      ]
    },
    "ui.cursor": {
      "fg": "waveBlue1",
      "bg": "waveAqua2"
    },
    "ui.cursor.primary": {
      "fg": "waveBlue1",
      "bg": "fujiWhite"
    },
    "ui.cursor.match": {
      "fg": "waveRed",
      "modifiers": [
        "bold"
      ]
    },
    "ui.highlight": {
      "fg": "fujiWhite",
      "bg": "waveBlue2"
    },
    "ui.menu": {
      "fg": "fujiWhite",
      "bg": "waveBlue1"
    },
    "ui.menu.selected": {
      "fg": "fujiWhite",
      "bg": "waveBlue2",
      "modifiers": [
        "bold"
      ]
    },
    "ui.menu.scroll": {
      "fg": "oldWhite",
      "bg": "waveBlue1"
    },
    "ui.cursorline.primary": {
      "bg": "sumiInk5"
    },
    "ui.cursorcolumn.primary": {
      "bg": "sumiInk5"
    },
    "ui.debug.breakpoint": "springBlue",
    "ui.debug.active": "winterRed",
    "diagnostic.error": {
      "underline": {
        "color": "samuraiRed",
        "style": "curl"
      }
    },
    "diagnostic.warning": {
      "underline": {
        "color": "roninYellow",
        "style": "curl"
      }
    },
    "diagnostic.info": {
      "underline": {
        "color": "dragonBlue",
        "style": "curl"
      }
    },
    "diagnostic.hint": {
      "underline": {
        "color": "waveAqua1",
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
    "error": "samuraiRed",
    "warning": "roninYellow",
    "info": "dragonBlue",
    "hint": "waveAqua1",
    "diff.plus": "autumnGreen",
    "diff.minus": "autumnRed",
    "diff.delta": "autumnYellow",
    "attribute": "waveRed",
    "type": "waveAqua2",
    "type.builtin": "springBlue",
    "constructor": "springBlue",
    "constant": "surimiOrange",
    "constant.numeric": "sakuraPink",
    "constant.character.escape": {
      "fg": "boatYellow2",
      "modifiers": [
        "bold"
      ]
    },
    "string": "springGreen",
    "string.regexp": "boatYellow2",
    "string.special.url": "springBlue",
    "string.special.symbol": "oniViolet",
    "comment": "fujiGray",
    "variable": "fujiWhite",
    "variable.builtin": "waveRed",
    "variable.parameter": "oniViolet2",
    "variable.other.member": "carpYellow",
    "label": "springBlue",
    "punctuation": "springViolet2",
    "keyword": {
      "fg": "oniViolet",
      "modifiers": [
        "italic"
      ]
    },
    "keyword.control.return": "peachRed",
    "keyword.control.exception": "peachRed",
    "keyword.directive": "waveRed",
    "operator": "boatYellow2",
    "function": "crystalBlue",
    "function.builtin": "springBlue",
    "function.macro": "waveRed",
    "tag": "waveAqua2",
    "namespace": "surimiOrange",
    "special": "peachRed",
    "markup.heading": {
      "fg": "springViolet2",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.marker": "springViolet2",
    "markup.heading.1": {
      "fg": "carpYellow",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.2": {
      "fg": "crystalBlue",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.3": {
      "fg": "waveAqua2",
      "modifiers": [
        "bold"
      ]
    },
    "markup.list": "sakuraPink",
    "markup.bold": {
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
    "markup.link.text": {
      "fg": "springBlue"
    },
    "markup.link.url": {
      "fg": "lightBlue"
    },
    "markup.link.label": "surimiOrange",
    "markup.quote": "oniViolet2",
    "markup.raw": "springGreen"
  }
});
