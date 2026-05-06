import { defineTheme } from "@the-editor/editor";

export default defineTheme({
  "id": "modus_operandi_deuteranopia",
  "title": "Modus Operandi Deuteranopia",
  "palette": {
    "bg-main": "#ffffff",
    "bg-dim": "#f2f2f2",
    "fg-main": "#000000",
    "fg-dim": "#595959",
    "fg-alt": "#193668",
    "bg-active": "#c4c4c4",
    "bg-inactive": "#e0e0e0",
    "red": "#a60000",
    "red-warmer": "#972500",
    "red-cooler": "#a0132f",
    "red-faint": "#7f0000",
    "red-intense": "#d00000",
    "green": "#006800",
    "green-warmer": "#316500",
    "green-cooler": "#00663f",
    "green-faint": "#2a5045",
    "green-intense": "#008900",
    "yellow": "#695500",
    "yellow-warmer": "#973300",
    "yellow-cooler": "#77492f",
    "yellow-faint": "#624416",
    "yellow-intense": "#808000",
    "blue": "#0031a9",
    "blue-warmer": "#354fcf",
    "blue-cooler": "#0000b0",
    "blue-faint": "#003497",
    "blue-intense": "#0000ff",
    "magenta": "#721045",
    "magenta-warmer": "#8f0075",
    "magenta-cooler": "#531ab6",
    "magenta-faint": "#7c318f",
    "magenta-intense": "#dd22dd",
    "cyan": "#005e8b",
    "cyan-warmer": "#3f578f",
    "cyan-cooler": "#005f5f",
    "cyan-faint": "#005077",
    "cyan-intense": "#008899",
    "bg-red-intense": "#ff8f88",
    "bg-green-intense": "#8adf80",
    "bg-yellow-intense": "#f3d000",
    "bg-blue-intense": "#bfc9ff",
    "bg-magenta-intense": "#dfa0f0",
    "bg-cyan-intense": "#a4d5f9",
    "bg-red-subtle": "#ffcfbf",
    "bg-green-subtle": "#b3fabf",
    "bg-yellow-subtle": "#fff576",
    "bg-blue-subtle": "#ccdfff",
    "bg-magenta-subtle": "#ffddff",
    "bg-cyan-subtle": "#bfefff",
    "bg-red-nuanced": "#ffe8e8",
    "bg-green-nuanced": "#e0f6e0",
    "bg-yellow-nuanced": "#f8f0d0",
    "bg-blue-nuanced": "#ecedff",
    "bg-magenta-nuanced": "#f8e6f5",
    "bg-cyan-nuanced": "#e0f2fa",
    "bg-completion": "#c0deff",
    "bg-hl-line": "#dae5ec",
    "bg-mode-line-active": "#d0d6ff",
    "fg-mode-line-active": "#0f0f0f",
    "bg-mode-line-inactive": "#e6e6e6",
    "fg-mode-line-inactive": "#585858",
    "modeline-err": "#603a00",
    "modeline-warning": "#454500",
    "modeline-info": "#023d92",
    "bg-added": "#d5d7ff",
    "fg-added": "#303099",
    "bg-changed": "#eecfdf",
    "fg-changed": "#6f1343",
    "bg-removed": "#f4f099",
    "fg-removed": "#553d00",
    "bg-paren-match": "#5fcfff",
    "border-mode-line-active": "#4f4f74"
  },
  "styles": {
    "type": "cyan-cooler",
    "constructor": "cyan-cooler",
    "constant": "blue-cooler",
    "constant.character.escape": "magenta",
    "string": "blue-warmer",
    "string.regexp": "magenta-faint",
    "string.special": "blue-faint",
    "comment": "yellow-cooler",
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
    "ui.text.directory": "blue",
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
      "bg": "blue-intense"
    },
    "ui.cursor.primary": {
      "fg": "bg-main",
      "bg": "fg-dim"
    },
    "ui.cursor.match": {
      "fg": "fg-main",
      "bg": "bg-paren-match"
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
        "color": "yellow-intense",
        "style": "curl"
      }
    },
    "diagnostic.warning": {
      "underline": {
        "color": "magenta-faint",
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
    "error": "yellow-intense",
    "warning": "magenta-faint",
    "info": "cyan",
    "hint": "blue",
    "ui.cursor.primary.normal": {
      "fg": "bg-main",
      "bg": "blue-intense"
    }
  }
});
