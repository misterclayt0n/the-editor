import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "modus_vivendi_tritanopia",
  "title": "Modus Vivendi Tritanopia",
  "palette": {
    "bg-main": "#000000",
    "bg-dim": "#1e1e1e",
    "fg-main": "#ffffff",
    "fg-dim": "#989898",
    "fg-alt": "#c6daff",
    "bg-active": "#535353",
    "bg-inactive": "#303030",
    "red": "#ff5f59",
    "red-warmer": "#ff6740",
    "red-cooler": "#ff7f86",
    "red-faint": "#ff9070",
    "red-intense": "#ff5f5f",
    "green": "#44bc44",
    "green-warmer": "#70b900",
    "green-cooler": "#00c06f",
    "green-faint": "#88ca9f",
    "green-intense": "#44df44",
    "yellow": "#cabf00",
    "yellow-warmer": "#ffa00f",
    "yellow-cooler": "#d8af7a",
    "yellow-faint": "#d2b580",
    "yellow-intense": "#efef00",
    "blue": "#2fafff",
    "blue-warmer": "#79a8ff",
    "blue-cooler": "#00bcff",
    "blue-faint": "#82b0ec",
    "blue-intense": "#338fff",
    "magenta": "#feacd0",
    "magenta-warmer": "#f78fe7",
    "magenta-cooler": "#b6a0ff",
    "magenta-faint": "#caa6df",
    "magenta-intense": "#ef7fff",
    "cyan": "#00d3d0",
    "cyan-warmer": "#4ae2ff",
    "cyan-cooler": "#6ae4b9",
    "cyan-faint": "#7fdbdf",
    "cyan-intense": "#00eff0",
    "rust": "#db7b5f",
    "gold": "#c0965b",
    "olive": "#9cbd6f",
    "slate": "#76afbf",
    "indigo": "#9099d9",
    "maroon": "#cf7fa7",
    "pink": "#d09dc0",
    "bg-red-intense": "#9d1f1f",
    "bg-green-intense": "#2f822f",
    "bg-yellow-intense": "#7a6100",
    "bg-blue-intense": "#1640b0",
    "bg-magenta-intense": "#7030af",
    "bg-cyan-intense": "#2266ae",
    "bg-red-subtle": "#620f2a",
    "bg-green-subtle": "#00422a",
    "bg-yellow-subtle": "#4a4000",
    "bg-blue-subtle": "#242679",
    "bg-magenta-subtle": "#552f5f",
    "bg-cyan-subtle": "#004065",
    "bg-red-nuanced": "#3a0c14",
    "bg-green-nuanced": "#092f1f",
    "bg-yellow-nuanced": "#381d0f",
    "bg-blue-nuanced": "#12154a",
    "bg-magenta-nuanced": "#2f0c3f",
    "bg-cyan-nuanced": "#042837",
    "bg-ochre": "#442c2f",
    "bg-lavender": "#38325c",
    "bg-sage": "#0f3d30",
    "bg-completion": "#004253",
    "bg-hover": "#8e3e3b",
    "bg-hover-secondary": "#00405f",
    "bg-hl-line": "#2f3849",
    "bg-region": "#5a5a5a",
    "fg-region": "#ffffff",
    "bg-char-0": "#922a00",
    "bg-char-1": "#00709f",
    "bg-char-2": "#5f3faf",
    "bg-mode-line-active": "#003c52",
    "fg-mode-line-active": "#f0f0f0",
    "border-mode-line-active": "#5f8fb4",
    "bg-mode-line-inactive": "#2d2d2d",
    "fg-mode-line-inactive": "#969696",
    "border-mode-line-inactive": "#606060",
    "modeline-err": "#ff7fbf",
    "modeline-warning": "#df9f93",
    "modeline-info": "#4fcfef",
    "bg-tab-bar": "#313131",
    "bg-tab-current": "#000000",
    "bg-tab-other": "#545454",
    "bg-added": "#004254",
    "bg-added-faint": "#003042",
    "bg-added-refine": "#004f7f",
    "bg-added-fringe": "#008fcf",
    "fg-added": "#9fdfdf",
    "fg-added-intense": "#50c0ef",
    "bg-changed": "#2f123f",
    "bg-changed-faint": "#1f022f",
    "bg-changed-refine": "#3f325f",
    "bg-changed-fringe": "#7f55a0",
    "fg-changed": "#e3cfff",
    "fg-changed-intense": "#cf9fe2",
    "bg-removed": "#4f1119",
    "bg-removed-faint": "#380a0f",
    "bg-removed-refine": "#781a1f",
    "bg-removed-fringe": "#b81a1f",
    "fg-removed": "#ffbfbf",
    "fg-removed-intense": "#ff9095",
    "bg-diff-context": "#1a1a1a",
    "bg-paren-match": "#2f7f9f",
    "tg-paren-expression": "#453040",
    "border": "#646464",
    "bg-graph-red-0": "#b52c2c",
    "bg-graph-red-1": "#702020",
    "bg-graph-green-0": "#afd1c0",
    "bg-graph-green-1": "#607a8f",
    "bg-graph-yellow-0": "#facfd6",
    "bg-graph-yellow-1": "#b57b85",
    "bg-graph-blue-0": "#4f9fdf",
    "bg-graph-blue-1": "#004559",
    "bg-graph-magenta-0": "#b6427f",
    "bg-graph-magenta-1": "#7f506f",
    "bg-graph-cyan-0": "#57dfea",
    "bg-graph-cyan-1": "#00808f",
    "bg-paren-expression": "#453040"
  },
  "styles": {
    "type": "blue-warmer",
    "constructor": "blue-warmer",
    "constant": "green-cooler",
    "constant.character.escape": "red-cooler",
    "string": "cyan",
    "string.regexp": "magenta-faint",
    "string.special": "blue-faint",
    "comment": "red-faint",
    "variable.parameter": "cyan-cooler",
    "variable.builtin": "magenta-cooler",
    "label": "fg-dim",
    "keyword": "red-cooler",
    "keyword.directive": "red-warmer",
    "function": "cyan-warmer",
    "function.macro": "magenta",
    "punctuation": "fg-dim",
    "tag": "red-cooler",
    "attribute": "cyan-cooler",
    "namespace": "blue-cooler",
    "special": "red-cooler",
    "markup.heading.marker": "fg-dim",
    "markup.heading.1": {
      "fg": "fg-main",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.2": {
      "fg": "red-faint",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.3": {
      "fg": "cyan-faint",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.4": {
      "fg": "magenta",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.5": {
      "fg": "green-faint",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.6": {
      "fg": "magenta-faint",
      "modifiers": [
        "bold"
      ]
    },
    "markup.list": "fg-dim",
    "markup.list.checked": {
      "fg": "yellow-warmer"
    },
    "markup.list.unchecked": {
      "fg": "yellow-warmer"
    },
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
    "markup.link.url": {
      "fg": "cyan-cooler"
    },
    "markup.link.text": {
      "fg": "cyan",
      "modifiers": [
        "underlined"
      ]
    },
    "markup.raw.block": {
      "bg": "bg-dim"
    },
    "markup.raw.inline": {
      "fg": "green-cooler"
    },
    "diff.plus": {
      "fg": "fg-added",
      "bg": "bg-added"
    },
    "diff.plus.gutter": "green-intense",
    "diff.minus": {
      "fg": "fg-removed",
      "bg": "bg-removed"
    },
    "diff.minus.gutter": "red-intense",
    "diff.delta": {
      "fg": "fg-changed",
      "bg": "bg-changed"
    },
    "diff.delta.gutter": "yellow-intense",
    "ui.background": {
      "bg": "bg-main"
    },
    "ui.linenr": {
      "fg": "fg-dim",
      "bg": "bg-dim"
    },
    "ui.linenr.selected": {
      "fg": "fg-main",
      "bg": "bg-active"
    },
    "ui.statusline": {
      "fg": "fg-mode-line-active",
      "bg": "bg-mode-line-active"
    },
    "ui.statusline.inactive": {
      "fg": "fg-mode-line-inactive",
      "bg": "bg-mode-line-inactive"
    },
    "ui.statusline.normal": {
      "fg": "blue-warmer"
    },
    "ui.statusline.insert": {
      "fg": "green-warmer"
    },
    "ui.statusline.select": {
      "fg": "magenta-warmer"
    },
    "ui.bufferline": {
      "bg": "bg-tab-bar"
    },
    "ui.bufferline.active": {
      "bg": "bg-tab-current",
      "fg": "fg-mode-line-active"
    },
    "ui.popup": {
      "fg": "fg-main",
      "bg": "bg-dim"
    },
    "ui.window": {
      "fg": "fg-dim"
    },
    "ui.help": {
      "fg": "fg-main",
      "bg": "bg-active"
    },
    "ui.gutter": {
      "bg": "bg-dim"
    },
    "ui.text": "fg-main",
    "ui.text.directory": "blue-cooler",
    "ui.text.focus": {
      "fg": "fg-main",
      "bg": "bg-completion",
      "modifiers": [
        "bold"
      ]
    },
    "ui.text.inactive": {
      "fg": "fg-dim"
    },
    "ui.virtual": "bg-active",
    "ui.virtual.ruler": {
      "bg": "bg-dim"
    },
    "ui.virtual.inlay-hint": {
      "fg": "fg-dim",
      "modifiers": [
        "italic"
      ]
    },
    "ui.virtual.jump-label": {
      "fg": "yellow-cooler",
      "modifiers": [
        "bold"
      ]
    },
    "ui.selection": {
      "fg": "fg-main",
      "bg": "bg-inactive"
    },
    "ui.selection.primary": {
      "fg": "fg-main",
      "bg": "bg-active"
    },
    "ui.cursor": {
      "fg": "bg-main",
      "bg": "red-intense"
    },
    "ui.cursor.primary": {
      "fg": "bg-main",
      "bg": "fg-dim"
    },
    "ui.cursor.match": {
      "bg": "bg-paren-match",
      "fg": "fg-main"
    },
    "ui.cursorline.primary": {
      "bg": "bg-hl-line"
    },
    "ui.highlight": {
      "bg": "bg-hl-line"
    },
    "ui.menu": {
      "fg": "fg-main",
      "bg": "bg-dim"
    },
    "ui.menu.selected": {
      "fg": "fg-main",
      "bg": "bg-completion",
      "modifiers": [
        "bold"
      ]
    },
    "ui.menu.scroll": {
      "fg": "fg-dim",
      "bg": "bg-cyan-intense"
    },
    "diagnostic.error": {
      "underline": {
        "color": "red-warmer",
        "style": "curl"
      }
    },
    "diagnostic.warning": {
      "underline": {
        "color": "magenta",
        "style": "curl"
      }
    },
    "diagnostic.info": {
      "underline": {
        "color": "cyan",
        "style": "curl"
      }
    },
    "diagnostic.hint": {
      "underline": {
        "color": "blue",
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
    "error": "red-warmer",
    "warning": "magenta",
    "info": "cyan",
    "hint": "blue",
    "ui.cursor.primary.normal": {
      "fg": "bg-main",
      "bg": "red-intense"
    }
  }
});
