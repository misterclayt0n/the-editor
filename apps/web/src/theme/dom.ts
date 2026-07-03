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

const transparentTerminalColor = (color: string): string => {
  if (color[0] !== "#") return color;
  if (color.length === 4) {
    return `#${color[1]}${color[1]}${color[2]}${color[2]}${color[3]}${color[3]}`;
  }
  if (color.length === 5) {
    return `#${color[1]}${color[1]}${color[2]}${color[2]}${color[3]}${color[3]}${color[4]}${color[4]}`;
  }
  return color;
};

type RgbaColor = {
  readonly red: number;
  readonly green: number;
  readonly blue: number;
  readonly alpha: number;
};

const hexByte = (value: number): string => value.toString(16).padStart(2, "0");

const parseHexColor = (color: string): RgbaColor | null => {
  const expanded = transparentTerminalColor(color);
  if (expanded[0] !== "#" || (expanded.length !== 7 && expanded.length !== 9)) return null;
  const red = Number.parseInt(expanded.slice(1, 3), 16);
  const green = Number.parseInt(expanded.slice(3, 5), 16);
  const blue = Number.parseInt(expanded.slice(5, 7), 16);
  const alpha = expanded.length === 9 ? Number.parseInt(expanded.slice(7, 9), 16) : 255;
  if (![red, green, blue, alpha].every(Number.isFinite)) return null;
  return { red, green, blue, alpha };
};

const compositeOpaqueColor = (foreground: string, background: string): string => {
  const fg = parseHexColor(foreground);
  const bg = parseHexColor(opaqueTerminalColor(background));
  if (fg === null) return opaqueTerminalColor(background);
  if (bg === null || fg.alpha === 255) return opaqueTerminalColor(foreground);
  const alpha = fg.alpha / 255;
  const red = Math.round(fg.red * alpha + bg.red * (1 - alpha));
  const green = Math.round(fg.green * alpha + bg.green * (1 - alpha));
  const blue = Math.round(fg.blue * alpha + bg.blue * (1 - alpha));
  return `#${hexByte(red)}${hexByte(green)}${hexByte(blue)}`;
};

const setTerminalColor = (
  out: Record<string, string>,
  key: string,
  value: string | undefined,
): void => {
  if (value !== undefined) out[key] = opaqueTerminalColor(value);
};

export const terminalTheme = (theme: ResolvedTheme): Record<string, string> => {
  const terminalBackground =
    theme.colors["terminal.background"] ??
    theme.colors["panel.background"] ??
    theme.colors["editor.background"] ??
    "#000000";
  const terminalAnsiBackground = theme.colors["terminal.ansi.background"] ?? terminalBackground;
  const foreground = opaqueTerminalColor(
    theme.colors["terminal.foreground"] ?? theme.colors["editor.foreground"] ?? "#d4d4d4",
  );
  const selectionBackground =
    theme.colors["terminal.selection.background"] ??
    theme.players[0]?.selection ??
    theme.colors["search.match_background"] ??
    theme.colors["text.accent"] ??
    foreground;
  const out: Record<string, string> = {
    background: opaqueTerminalColor(terminalAnsiBackground),
    foreground,
    cursor: opaqueTerminalColor(theme.players[0]?.cursor ?? theme.colors["text.accent"] ?? foreground),
    cursorAccent: opaqueTerminalColor(terminalAnsiBackground),
    selectionBackground: transparentTerminalColor(selectionBackground),
    selectionInactiveBackground: transparentTerminalColor(
      theme.colors["terminal.selection.inactive_background"] ?? selectionBackground,
    ),
  };
  // The canvas renderer paints selected text with selectionForeground
  // unconditionally (it cannot keep per-cell colors), and its built-in
  // default is near-black — unreadable on dark themes. Fall back to the
  // terminal foreground so selected text stays legible.
  out.selectionForeground = opaqueTerminalColor(
    theme.colors["terminal.selection.foreground"] ?? foreground,
  );
  setTerminalColor(out, "surfaceBackground", terminalBackground);
  setTerminalColor(out, "ansiBackground", terminalAnsiBackground);
  out.brightForeground = opaqueTerminalColor(theme.colors["terminal.bright_foreground"] ?? foreground);
  out.dimForeground = opaqueTerminalColor(theme.colors["terminal.dim_foreground"] ?? foreground);
  setTerminalColor(out, "black", theme.colors["terminal.ansi.black"]);
  setTerminalColor(out, "brightBlack", theme.colors["terminal.ansi.bright_black"]);
  out.dimBlack = opaqueTerminalColor(theme.colors["terminal.ansi.dim_black"] ?? out.brightBlack ?? out.black ?? foreground);
  setTerminalColor(out, "red", theme.colors["terminal.ansi.red"]);
  out.brightRed = opaqueTerminalColor(theme.colors["terminal.ansi.bright_red"] ?? out.red ?? foreground);
  out.dimRed = opaqueTerminalColor(theme.colors["terminal.ansi.dim_red"] ?? out.red ?? foreground);
  setTerminalColor(out, "green", theme.colors["terminal.ansi.green"]);
  out.brightGreen = opaqueTerminalColor(theme.colors["terminal.ansi.bright_green"] ?? out.green ?? foreground);
  out.dimGreen = opaqueTerminalColor(theme.colors["terminal.ansi.dim_green"] ?? out.green ?? foreground);
  setTerminalColor(out, "yellow", theme.colors["terminal.ansi.yellow"]);
  out.brightYellow = opaqueTerminalColor(theme.colors["terminal.ansi.bright_yellow"] ?? out.yellow ?? foreground);
  out.dimYellow = opaqueTerminalColor(theme.colors["terminal.ansi.dim_yellow"] ?? out.yellow ?? foreground);
  setTerminalColor(out, "blue", theme.colors["terminal.ansi.blue"]);
  out.brightBlue = opaqueTerminalColor(theme.colors["terminal.ansi.bright_blue"] ?? out.blue ?? foreground);
  out.dimBlue = opaqueTerminalColor(theme.colors["terminal.ansi.dim_blue"] ?? out.blue ?? foreground);
  setTerminalColor(out, "magenta", theme.colors["terminal.ansi.magenta"]);
  out.brightMagenta = opaqueTerminalColor(theme.colors["terminal.ansi.bright_magenta"] ?? out.magenta ?? foreground);
  out.dimMagenta = opaqueTerminalColor(theme.colors["terminal.ansi.dim_magenta"] ?? out.magenta ?? foreground);
  setTerminalColor(out, "cyan", theme.colors["terminal.ansi.cyan"]);
  out.brightCyan = opaqueTerminalColor(theme.colors["terminal.ansi.bright_cyan"] ?? out.cyan ?? foreground);
  out.dimCyan = opaqueTerminalColor(theme.colors["terminal.ansi.dim_cyan"] ?? out.cyan ?? foreground);
  setTerminalColor(out, "white", theme.colors["terminal.ansi.white"]);
  out.brightWhite = opaqueTerminalColor(theme.colors["terminal.ansi.bright_white"] ?? out.white ?? foreground);
  out.dimWhite = opaqueTerminalColor(theme.colors["terminal.ansi.dim_white"] ?? out.white ?? foreground);
  return out;
};

export type TerminalSearchDecorations = {
  readonly matchBackground: string;
  readonly activeMatchBackground: string;
  readonly matchOverviewRuler: string;
  readonly activeMatchColorOverviewRuler: string;
  readonly matchBorder?: string;
  readonly activeMatchBorder?: string;
};

export const terminalSearchDecorations = (theme: ResolvedTheme): TerminalSearchDecorations => {
  const terminalBackground =
    theme.colors["terminal.ansi.background"] ??
    theme.colors["terminal.background"] ??
    theme.colors["editor.background"] ??
    "#000000";
  const match =
    theme.colors["terminal.find.match_background"] ??
    theme.colors["search.match_background"] ??
    theme.colors["editor.document_highlight.read_background"] ??
    theme.players[0]?.selection ??
    theme.colors["text.accent"] ??
    "#000000";
  const activeMatch =
    theme.colors["terminal.find.active_match_background"] ??
    theme.players[0]?.selection ??
    theme.colors["editor.document_highlight.write_background"] ??
    match;
  const matchColor = compositeOpaqueColor(match, terminalBackground);
  const activeMatchColor = compositeOpaqueColor(activeMatch, terminalBackground);
  const decorations: TerminalSearchDecorations = {
    matchBackground: matchColor,
    activeMatchBackground: activeMatchColor,
    matchOverviewRuler: opaqueTerminalColor(theme.colors["terminal.find.match_overview_ruler"] ?? matchColor),
    activeMatchColorOverviewRuler: opaqueTerminalColor(
      theme.colors["terminal.find.active_match_overview_ruler"] ?? activeMatchColor,
    ),
  };
  const matchBorder = theme.colors["terminal.find.match_border"];
  const activeMatchBorder = theme.colors["terminal.find.active_match_border"];
  return {
    ...decorations,
    ...(matchBorder !== undefined ? { matchBorder: opaqueTerminalColor(matchBorder) } : {}),
    ...(activeMatchBorder !== undefined ? { activeMatchBorder: opaqueTerminalColor(activeMatchBorder) } : {}),
  };
};
