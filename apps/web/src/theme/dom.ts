import type { HighlightStyle, ResolvedTheme } from "@the-editor/theme";

let activeTheme: ResolvedTheme | undefined;

export const setCurrentTheme = (theme: ResolvedTheme): void => {
  activeTheme = theme;
};

export const currentTheme = (): ResolvedTheme | undefined => activeTheme;

export const applyThemeToDocument = (theme: ResolvedTheme): void => {
  setCurrentTheme(theme);
  const root = document.documentElement;
  root.dataset.appearance = theme.appearance;
  root.style.colorScheme = theme.appearance;
  for (const [name, value] of Object.entries(theme.cssVars)) root.style.setProperty(name, value);
};

export const resolveSyntaxStyle = (captureName: string): HighlightStyle | undefined =>
  activeTheme?.syntax.resolve(captureName);

export const terminalTheme = (theme: ResolvedTheme): Record<string, string> => {
  const foreground = theme.colors["terminal.foreground"] ?? theme.colors["editor.foreground"] ?? "currentColor";
  const out: Record<string, string> = {
    background: theme.colors["terminal.background"] ?? theme.colors["editor.background"] ?? "transparent",
    foreground,
    cursor: theme.players[0]?.cursor ?? theme.colors["text.accent"] ?? foreground,
  };
  if (theme.colors["terminal.ansi.black"] !== undefined) out.black = theme.colors["terminal.ansi.black"];
  if (theme.colors["terminal.ansi.bright_black"] !== undefined) out.brightBlack = theme.colors["terminal.ansi.bright_black"];
  if (theme.colors["terminal.ansi.red"] !== undefined) out.red = theme.colors["terminal.ansi.red"];
  out.brightRed = theme.colors["terminal.ansi.bright_red"] ?? out.red ?? foreground;
  if (theme.colors["terminal.ansi.green"] !== undefined) out.green = theme.colors["terminal.ansi.green"];
  out.brightGreen = theme.colors["terminal.ansi.bright_green"] ?? out.green ?? foreground;
  if (theme.colors["terminal.ansi.yellow"] !== undefined) out.yellow = theme.colors["terminal.ansi.yellow"];
  out.brightYellow = theme.colors["terminal.ansi.bright_yellow"] ?? out.yellow ?? foreground;
  if (theme.colors["terminal.ansi.blue"] !== undefined) out.blue = theme.colors["terminal.ansi.blue"];
  out.brightBlue = theme.colors["terminal.ansi.bright_blue"] ?? out.blue ?? foreground;
  if (theme.colors["terminal.ansi.magenta"] !== undefined) out.magenta = theme.colors["terminal.ansi.magenta"];
  out.brightMagenta = theme.colors["terminal.ansi.bright_magenta"] ?? out.magenta ?? foreground;
  if (theme.colors["terminal.ansi.cyan"] !== undefined) out.cyan = theme.colors["terminal.ansi.cyan"];
  out.brightCyan = theme.colors["terminal.ansi.bright_cyan"] ?? out.cyan ?? foreground;
  if (theme.colors["terminal.ansi.white"] !== undefined) out.white = theme.colors["terminal.ansi.white"];
  out.brightWhite = theme.colors["terminal.ansi.bright_white"] ?? out.white ?? foreground;
  return out;
};
