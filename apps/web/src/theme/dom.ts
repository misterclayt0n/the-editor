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

const opaqueTerminalColor = (color: string): string => {
  if (color[0] !== "#") return color;
  if (color.length === 9) return color.slice(0, 7);
  if (color.length === 5) {
    return `#${color[1]}${color[1]}${color[2]}${color[2]}${color[3]}${color[3]}`;
  }
  return color;
};

const setTerminalColor = (
  out: Record<string, string>,
  key: string,
  value: string | undefined,
): void => {
  if (value !== undefined) out[key] = opaqueTerminalColor(value);
};

export const terminalTheme = (theme: ResolvedTheme): Record<string, string> => {
  const foreground = opaqueTerminalColor(
    theme.colors["terminal.foreground"] ?? theme.colors["editor.foreground"] ?? "#d4d4d4",
  );
  const out: Record<string, string> = {
    background: opaqueTerminalColor(
      theme.colors["terminal.background"] ?? theme.colors["editor.background"] ?? "#000000",
    ),
    foreground,
    cursor: opaqueTerminalColor(theme.players[0]?.cursor ?? theme.colors["text.accent"] ?? foreground),
  };
  setTerminalColor(out, "black", theme.colors["terminal.ansi.black"]);
  setTerminalColor(out, "brightBlack", theme.colors["terminal.ansi.bright_black"]);
  setTerminalColor(out, "red", theme.colors["terminal.ansi.red"]);
  out.brightRed = opaqueTerminalColor(theme.colors["terminal.ansi.bright_red"] ?? out.red ?? foreground);
  setTerminalColor(out, "green", theme.colors["terminal.ansi.green"]);
  out.brightGreen = opaqueTerminalColor(theme.colors["terminal.ansi.bright_green"] ?? out.green ?? foreground);
  setTerminalColor(out, "yellow", theme.colors["terminal.ansi.yellow"]);
  out.brightYellow = opaqueTerminalColor(theme.colors["terminal.ansi.bright_yellow"] ?? out.yellow ?? foreground);
  setTerminalColor(out, "blue", theme.colors["terminal.ansi.blue"]);
  out.brightBlue = opaqueTerminalColor(theme.colors["terminal.ansi.bright_blue"] ?? out.blue ?? foreground);
  setTerminalColor(out, "magenta", theme.colors["terminal.ansi.magenta"]);
  out.brightMagenta = opaqueTerminalColor(theme.colors["terminal.ansi.bright_magenta"] ?? out.magenta ?? foreground);
  setTerminalColor(out, "cyan", theme.colors["terminal.ansi.cyan"]);
  out.brightCyan = opaqueTerminalColor(theme.colors["terminal.ansi.bright_cyan"] ?? out.cyan ?? foreground);
  setTerminalColor(out, "white", theme.colors["terminal.ansi.white"]);
  out.brightWhite = opaqueTerminalColor(theme.colors["terminal.ansi.bright_white"] ?? out.white ?? foreground);
  return out;
};
