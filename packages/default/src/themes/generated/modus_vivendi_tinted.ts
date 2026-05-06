import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "modus_vivendi_tinted",
  "title": "Modus Vivendi Tinted",
  "palette": {
    "bg-main": "#0d0e1c",
    "bg-dim": "#1d2235",
    "fg-main": "#ffffff",
    "fg-dim": "#989898",
    "fg-alt": "#c6daff",
    "bg-active": "#4a4f69",
    "bg-inactive": "#2b3045",
    "red": "#ff5f59",
    "red-warmer": "#ff6b55",
    "red-cooler": "#ff7f86",
    "red-faint": "#ff9f80",
    "red-intense": "#ff5f5f",
    "green": "#44bc44",
    "green-warmer": "#70b900",
    "green-cooler": "#00c06f",
    "green-faint": "#88ca9f",
    "green-intense": "#44df44",
    "yellow": "#d0bc00",
    "yellow-warmer": "#fec43f",
    "yellow-cooler": "#dfaf7a",
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
    "magenta-intense": "#ff66ff",
    "cyan": "#00d3d0",
    "cyan-warmer": "#4ae2f0",
    "cyan-cooler": "#6ae4b9",
    "cyan-faint": "#9ac8e0",
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
    "bg-completion": "#483d8a",
    "bg-hover": "#45605e",
    "bg-hover-secondary": "#654a39",
    "bg-hl-line": "#303a6f",
    "bg-region": "#555a66",
    "fg-region": "#ffffff",
    "bg-char-0": "#0050af",
    "bg-char-1": "#7f1f7f",
    "bg-char-2": "#625a00",
    "bg-mode-line-active": "#484d67",
    "fg-mode-line-active": "#ffffff",
    "border-mode-line-active": "#979797",
    "bg-mode-line-inactive": "#292d48",
    "fg-mode-line-inactive": "#969696",
    "border-mode-line-inactive": "#606270",
    "modeline-err": "#ffa9bf",
    "modeline-warning": "#dfcf43",
    "modeline-info": "#9fefff",
    "bg-tab-bar": "#2c3045",
    "bg-tab-current": "#0d0e1c",
    "bg-tab-other": "#4a4f6a",
    "bg-added": "#003a2f",
    "bg-added-faint": "#002922",
    "bg-added-refine": "#035542",
    "bg-added-fringe": "#23884f",
    "fg-added": "#a0e0a0",
    "fg-added-intense": "#80e080",
    "bg-changed": "#363300",
    "bg-changed-faint": "#2a1f00",
    "bg-changed-refine": "#4a4a00",
    "bg-changed-fringe": "#8f7a30",
    "fg-changed": "#efef80",
    "fg-changed-intense": "#c0b05f",
    "bg-removed": "#4f1127",
    "bg-removed-faint": "#380a19",
    "bg-removed-refine": "#781a3a",
    "bg-removed-fringe": "#b81a26",
    "fg-removed": "#ffbfbf",
    "fg-removed-intense": "#ff9095",
    "bg-diff-context": "#1a1f30",
    "bg-paren-match": "#5f789f",
    "tg-paren-expression": "#453040",
    "border": "#61647a",
    "bg-paren-expression": "#453040"
  },
  "styles": {
    "type": "cyan-cooler",
    "constructor": "cyan-cooler",
    "constant": "blue-cooler",
    "constant.character.escape": "magenta",
    "string": "blue-warmer",
    "string.regexp": "magenta-faint",
    "string.special": "blue-faint",
    "comment": "red-faint",
    "variable.parameter": "cyan",
    "variable.builtin": "magenta-cooler",
    "label": "fg-dim",
    "keyword": "magenta-cooler",
    "keyword.directive": "red-cooler",
    "function": "magenta",
    "function.macro": "magenta-warmer",
    "punctuation": "fg-dim",
    "tag": "magenta",
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
      "fg": "yellow-faint",
      "modifiers": [
        "bold"
      ]
    },
    "markup.heading.3": {
      "fg": "fg-alt",
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
      "fg": "red-faint",
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
      "fg": "cyan"
    },
    "markup.link.text": {
      "fg": "blue-warmer",
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
      "bg": "fg-main"
    },
    "ui.cursor.primary": {
      "fg": "bg-main",
      "bg": "magenta-intense"
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
        "color": "red-intense",
        "style": "curl"
      }
    },
    "diagnostic.warning": {
      "underline": {
        "color": "yellow-intense",
        "style": "curl"
      }
    },
    "diagnostic.info": {
      "underline": {
        "color": "cyan-intense",
        "style": "curl"
      }
    },
    "diagnostic.hint": {
      "underline": {
        "color": "blue-intense",
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
    "error": "red",
    "warning": "yellow-warmer",
    "info": "cyan-cooler",
    "hint": "blue-cooler"
  }
});
