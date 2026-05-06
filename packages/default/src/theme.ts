import type { EditorModule } from "@the-editor/editor";
import { defineTheme } from "@the-editor/editor";
import { importedThemes } from "./themes/generated/index.js";

export const defaultDarkTheme = defineTheme({
  id: "default.dark",
  title: "Default Dark",
  palette: {
    background: "#101010",
    panel: "#111111",
    panel_alt: "#121212",
    panel_strong: "#141414",
    border: "rgba(255, 255, 255, 0.06)",
    border_soft: "rgba(255, 255, 255, 0.04)",
    border_faint: "rgba(255, 255, 255, 0.03)",
    text: "#d7d7d7",
    text_strong: "#f3f4f6",
    text_muted: "#8f949c",
    text_subtle: "#8a91a1",
    accent: "#f4bf75",
    accent_soft: "rgba(244, 191, 117, 0.12)",
    accent_border: "rgba(244, 191, 117, 0.2)",
    accent_focus: "rgba(76, 122, 255, 0.4)",
    selection: "#264f78",
    cursor: "#e6e6e6",
    cursor_text: "#101010",
    overlay: "rgba(0, 0, 0, 0.32)",
    popup: "rgba(26, 26, 26, 0.96)",
    popup_footer: "rgba(20, 20, 20, 0.98)",
    danger: "#fca5a5",
    warning: "#f4bf75",
    info: "#93c5fd",
    hint: "#9ae6b4",
    keyword: "#c586c0",
    type: "#dcdcaa",
    variable: "#9cdcfe",
    string: "#ce9178",
    comment: "#6a9955",
  },
  styles: {
    keyword: "keyword",
    type: "type",
    variable: "variable",
    string: "string",
    comment: "comment",

    error: "danger",
    warning: "warning",
    info: "info",
    hint: "hint",

    "diagnostic.error": "danger",
    "diagnostic.warning": "warning",
    "diagnostic.info": "info",
    "diagnostic.hint": "hint",

    "ui.background": { bg: "background" },
    "ui.background.separator": { fg: "border_soft" },
    "ui.text": { fg: "text" },
    "ui.text.focus": { fg: "text_strong" },
    "ui.text.inactive": { fg: "text_muted" },
    "ui.selection": { bg: "selection" },
    "ui.cursor": { bg: "cursor", fg: "cursor_text" },
    "ui.cursor.insert": { bg: "cursor", fg: "cursor_text" },
    "ui.statusline": { bg: "panel_strong", fg: "text" },
    "ui.menu": { bg: "panel", fg: "text" },
    "ui.menu.selected": { bg: "panel_alt", fg: "text_strong" },
    "ui.popup": { bg: "popup", fg: "text" },
    "ui.popup.border": { fg: "border" },
    "ui.command_palette": { bg: "popup", fg: "text" },
    "ui.command_palette.list.selected": { bg: "panel_alt", fg: "text_strong" },
    "ui.command_palette.backdrop": { bg: "overlay" },
    "ui.command_palette.footer": { bg: "popup_footer", fg: "text_muted" },
    "ui.command_palette.border": { fg: "border" },
    "ui.command_palette.border.subtle": { fg: "border_faint" },
    "ui.command_palette.icon": { fg: "text_muted" },
    "ui.command_palette.placeholder": { fg: "text_muted" },
    "ui.command_palette.detail": { fg: "text_subtle" },
    "ui.command_palette.detail.selected": { fg: "text" },
    "ui.inspector": { bg: "panel", fg: "text" },
    "ui.inspector.card": { bg: "rgba(255, 255, 255, 0.02)", fg: "text" },
    "ui.inspector.border": { fg: "rgba(255, 255, 255, 0.05)" },
    "ui.inspector.button": { bg: "#191c24", fg: "text" },
    "ui.inspector.button.active": { fg: "#f3f6ff", underline: { color: "accent_focus", style: "line" } },
    "ui.inspector.badge": { bg: "accent_soft", fg: "accent" },
    "ui.inspector.warning": { fg: "warning" },
  },
});

const themeModule: EditorModule = (editor) => {
  editor.themes.register(defaultDarkTheme);
  for (const theme of importedThemes) {
    editor.themes.register(theme);
  }
};

export default themeModule;
