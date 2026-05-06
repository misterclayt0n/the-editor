import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "kanagawa-dragon",
  "title": "Kanagawa Dragon",
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
    "katanaGray": "#717C7C",
    "dragonBlack0": "#0d0c0c",
    "dragonBlack1": "#12120f",
    "dragonBlack2": "#1D1C19",
    "dragonBlack3": "#181616",
    "dragonBlack4": "#282727",
    "dragonBlack5": "#393836",
    "dragonBlack6": "#625e5a",
    "dragonWhite": "#c5c9c5",
    "dragonGreen": "#87a987",
    "dragonGreen2": "#8a9a7b",
    "dragonPink": "#a292a3",
    "dragonOrange": "#b6927b",
    "dragonOrange2": "#b98d7b",
    "dragonGray": "#a6a69c",
    "dragonGray2": "#9e9b93",
    "dragonGray3": "#7a8382",
    "dragonBlue2": "#8ba4b0",
    "dragonViolet": "#8992a7",
    "dragonRed": "#c4746e",
    "dragonAqua": "#8ea4a2",
    "dragonAsh": "#737c73",
    "dragonTeal": "#949fb5",
    "dragonYellow": "#c4b28a"
  },
  "styles": {
    "ui.selection": {
      "bg": "waveBlue2"
    },
    "ui.selection.primary": {
      "bg": "waveBlue1"
    },
    "ui.background": {
      "fg": "dragonWhite",
      "bg": "dragonBlack3"
    },
    "ui.linenr": {
      "fg": "dragonBlack6",
      "bg": "dragonBlack4"
    },
    "ui.linenr.selected": {
      "fg": "roninYellow",
      "modifiers": [
        "bold"
      ]
    },
    "ui.gutter": {
      "fg": "dragonBlack6",
      "bg": "dragonBlack4"
    },
    "ui.virtual": "dragonBlack4",
    "ui.virtual.ruler": {
      "bg": "dragonBlack5"
    },
    "ui.virtual.inlay-hint": "dragonGray2",
    "ui.virtual.jump-label": {
      "fg": "dragonRed",
      "modifiers": [
        "bold"
      ]
    },
    "ui.statusline": {
      "fg": "oldWhite",
      "bg": "dragonBlack0"
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
      "fg": "oldWhite",
      "bg": "dragonBlack0"
    },
    "ui.bufferline.active": {
      "fg": "oldWhite",
      "bg": "dragonBlack0"
    },
    "ui.bufferline.background": {
      "bg": "sumiInk0"
    },
    "ui.popup": {
      "fg": "oldWhite",
      "bg": "dragonBlack0"
    },
    "ui.window": {
      "fg": "sumiInk0"
    },
    "ui.help": {
      "fg": "fujiWhite",
      "bg": "sumiInk0"
    },
    "ui.text": "dragonWhite",
    "ui.text.focus": {
      "fg": "dragonWhite",
      "bg": "waveBlue1",
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
      "fg": "roninYellow",
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
      "bg": "dragonBlack5"
    },
    "ui.cursorcolumn.primary": {
      "bg": "dragonBlack5"
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
      "fg": "katanaGray",
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
    "type": "dragonAqua",
    "type.builtin": "springBlue",
    "constructor": "dragonTeal",
    "constant": "dragonOrange",
    "constant.numeric": "dragonPink",
    "constant.character.escape": "dragonBlue2",
    "string": "dragonGreen2",
    "string.regexp": "dragonRed",
    "string.special.url": "dragonTeal",
    "string.special.symbol": "dragonTeal",
    "comment": "dragonAsh",
    "variable": "dragonWhite",
    "variable.builtin": "dragonRed",
    "variable.parameter": "dragonGray",
    "variable.other.member": "dragonYellow",
    "label": "dragonRed",
    "punctuation": "dragonWhite",
    "keyword": "dragonViolet",
    "keyword.control.return": "dragonRed",
    "keyword.control.exception": "dragonRed",
    "keyword.directive": "dragonRed",
    "operator": "dragonRed",
    "function": "dragonBlue2",
    "function.builtin": "dragonBlue2",
    "function.macro": "dragonRed",
    "tag": "dragonYellow",
    "namespace": "dragonWhite",
    "special": "dragonYellow",
    "markup.heading": {
      "fg": "springViolet2",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.marker": "dragonViolet",
    "markup.heading.1": {
      "fg": "dragonOrange",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.2": {
      "fg": "dragonYellow",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.3": {
      "fg": "dragonBlue2",
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
    "markup.link.text": "dragonTeal",
    "markup.link.url": {
      "fg": "dragonPink",
      "underline": {
        "style": "line"
      }
    },
    "markup.link.label": "dragonBlue2",
    "markup.quote": "springViolet1",
    "markup.raw": "dragonGreen2",
    "ui.virtual.inlay-hint.parameter": {
      "fg": "dragonYellow",
      "modifiers": [
        "dim"
      ]
    },
    "ui.virtual.inlay-hint.type": {
      "fg": "dragonAqua",
      "modifiers": [
        "dim"
      ]
    },
    "type.enum.variant": "dragonOrange",
    "markup.heading.4": {
      "fg": "dragonWhite",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.5": {
      "fg": "dragonRed",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.6": {
      "fg": "dragonPink",
      "modifiers": [
        "bold"
      ]
    },
    "markup.list.numbered": "dragonPink",
    "markup.list.unnumbered": "dragonRed"
  }
});
