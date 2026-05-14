import { contributionKind } from "./contributions.js";
import { defineHostStore } from "./stores.js";
import type { EditorHost } from "./editorHost.js";
import { toDisposable, type Disposable } from "./disposable.js";
import type { BuiltinThemeStyleKey } from "./themeStyleKeys.js";

export { builtinThemeStyleKeys } from "./themeStyleKeys.js";
export type { BuiltinThemeStyleKey } from "./themeStyleKeys.js";

export type ThemeId = string & { readonly __brand?: "ThemeId" };
export type ThemeModifier = string;

export interface ThemeUnderline {
  color?: string;
  style?: string;
}

export interface ThemeStyle {
  fg?: string;
  bg?: string;
  modifiers?: readonly ThemeModifier[];
  underline?: ThemeUnderline;
}

export type ThemeStyleValue = string | ThemeStyle;

/**
 * Augment this interface to add new semantic theme entries from extensions.
 *
 * declare module "@the-editor/editor/themes" {
 *   interface ThemeStyleEntryAugmentations {
 *     "my.tool.panel": ThemeStyleValue;
 *   }
 * }
 */
export interface ThemeStyleEntryAugmentations {
  readonly __never?: never;
}

export type ThemeStyleKey = BuiltinThemeStyleKey | (Exclude<keyof ThemeStyleEntryAugmentations, "__never"> & string);
export type ThemeStyleFallback = ThemeStyleKey | readonly ThemeStyleKey[];

export interface ThemeGhosttyPalette {
  background?: string;
  foreground?: string;
  cursorColor?: string;
  selectionBackground?: string;
  selectionForeground?: string;
  palette?: Record<string, string>;
}

export interface ThemeDefinition {
  id: ThemeId | string;
  title?: string;
  palette?: Record<string, string>;
  styles?: Record<string, ThemeStyleValue>;
  ghostty?: ThemeGhosttyPalette;
}

export interface ResolvedThemeStyle {
  fg?: string;
  bg?: string;
  modifiers?: ThemeModifier[];
  underline?: {
    color?: string;
    style?: string;
  };
}

export interface ResolvedThemeGhosttyPalette {
  background?: string;
  foreground?: string;
  cursorColor?: string;
  selectionBackground?: string;
  selectionForeground?: string;
  palette?: Record<string, string>;
}

export interface ResolvedThemeDefinition {
  id: ThemeId;
  title?: string;
  palette: Record<string, string>;
  styles: Record<string, ResolvedThemeStyle>;
  ghostty?: ResolvedThemeGhosttyPalette;
}

export interface ThemeInfo {
  id: ThemeId;
  title?: string;
}

export interface ThemeSnapshot {
  selectedId?: ThemeId;
  active?: ResolvedThemeDefinition;
  available: ThemeInfo[];
}

interface RegisteredTheme {
  theme: ResolvedThemeDefinition;
  order: number;
}

const selectedThemeIdStore = defineHostStore<ThemeId | undefined>({
  id: "editor.themes.selectedId",
  init: () => undefined,
});
const defaultThemePalette: Record<string, string> = {
  black: "#000000",
  red: "#cd3131",
  green: "#0dbc79",
  yellow: "#e5e510",
  blue: "#2472c8",
  magenta: "#bc3fbc",
  cyan: "#11a8cd",
  gray: "#e5e5e5",
  "light-red": "#f14c4c",
  "light-green": "#23d18b",
  "light-yellow": "#f5f543",
  "light-blue": "#3b8eea",
  "light-magenta": "#d670d6",
  "light-cyan": "#29b8db",
  "light-gray": "#666666",
  white: "#ffffff",
};

export class ThemeRegistry {
  private readonly themes = new Map<ThemeId, RegisteredTheme>();
  private nextOrder = 0;

  constructor(private readonly editor: EditorHost) {}

  register<const TDefinition extends ThemeDefinition>(definition: ValidateThemeDefinition<TDefinition>, moduleId = this.editor.currentModuleId()): Disposable {
    const id = themeId(definition.id);
    const entry: RegisteredTheme = {
      theme: resolveTheme(definition),
      order: this.nextOrder++,
    };
    this.themes.set(id, entry);
    this.editor.workbench.invalidate("theme");

    const unregister = toDisposable(() => {
      const current = this.themes.get(id);
      if (current !== entry) return;
      this.themes.delete(id);
      this.editor.workbench.invalidate("theme");
    });

    return this.editor.contributions.add({
      moduleId,
      kind: contributionKind("theme"),
      localId: String(id),
      label: definition.title,
      disposable: unregister,
    });
  }

  activate(id: ThemeId | string): void {
    this.editor.stores.set(selectedThemeIdStore, themeId(id));
    this.editor.workbench.invalidate("theme");
  }

  selectedId(): ThemeId | undefined {
    return this.editor.stores.get(selectedThemeIdStore);
  }

  active(): ResolvedThemeDefinition | undefined {
    const selectedId = this.selectedId();
    if (selectedId) {
      const selected = this.themes.get(selectedId)?.theme;
      if (selected) return selected;
    }
    return this.availableThemes()[0]?.theme;
  }

  all(): ThemeInfo[] {
    return this.availableThemes().map(({ theme }) => ({ id: theme.id, title: theme.title }));
  }

  get(id: ThemeId | string): ResolvedThemeDefinition | undefined {
    return this.themes.get(themeId(id))?.theme;
  }

  getColor(name: string, theme = this.active()): string | undefined {
    if (!theme || !name) return undefined;
    return resolveThemeColor(theme, name);
  }

  getStyle(key: ThemeStyleKey, options: { theme?: ResolvedThemeDefinition; fallback?: ThemeStyleFallback } = {}): ResolvedThemeStyle | undefined {
    const theme = options.theme ?? this.active();
    if (!theme || !key) return undefined;
    return resolveThemeStyle(theme, key, options.fallback);
  }

  snapshot(): ThemeSnapshot {
    return {
      selectedId: this.selectedId(),
      active: this.active(),
      available: this.all(),
    };
  }

  private availableThemes(): RegisteredTheme[] {
    return [...this.themes.values()].sort((a, b) => a.order - b.order || String(a.theme.id).localeCompare(String(b.theme.id)));
  }
}

type UnknownThemeStyleKeys<TDefinition extends ThemeDefinition> = Exclude<Extract<keyof NonNullable<TDefinition["styles"]>, string>, ThemeStyleKey>;

type ValidateThemeDefinition<TDefinition extends ThemeDefinition> = [UnknownThemeStyleKeys<TDefinition>] extends [never]
  ? TDefinition
  : TDefinition & {
      styles?: NonNullable<TDefinition["styles"]> & {
        [K in UnknownThemeStyleKeys<TDefinition>]: never;
      };
    };

export function defineTheme<const TDefinition extends ThemeDefinition>(definition: ValidateThemeDefinition<TDefinition>): TDefinition {
  return definition;
}

export function themeId(id: ThemeId | string): ThemeId {
  return id as ThemeId;
}

export function resolveTheme(definition: ThemeDefinition): ResolvedThemeDefinition {
  const palette = { ...defaultThemePalette, ...(definition.palette ?? {}) };
  const styles = Object.fromEntries(
    Object.entries(definition.styles ?? {}).map(([key, value]) => [key, resolveThemeStyleValue(value, palette)]),
  ) as Record<string, ResolvedThemeStyle>;

  return {
    id: themeId(definition.id),
    title: definition.title,
    palette,
    styles,
    ghostty: definition.ghostty ? resolveGhosttyTheme(definition.ghostty, palette) : undefined,
  };
}

export function resolveThemeColor(theme: Pick<ResolvedThemeDefinition, "palette">, value?: string): string | undefined {
  if (!value || value === "default") return undefined;
  return theme.palette[value] ?? defaultThemePalette[value] ?? value;
}

export function resolveThemeStyle(
  theme: Pick<ResolvedThemeDefinition, "styles">,
  key: ThemeStyleKey,
  fallback?: ThemeStyleFallback,
): ResolvedThemeStyle | undefined {
  return resolveThemeStyleLoose(theme, key, fallback);
}

export function resolveThemeStyleLoose(
  theme: Pick<ResolvedThemeDefinition, "styles">,
  key: string,
  fallback?: string | readonly string[],
): ResolvedThemeStyle | undefined {
  const style = mergeThemeStyles(themeStyleCandidates(key).map((candidate) => theme.styles[candidate as ThemeStyleKey]));
  if (!isThemeStyleEmpty(style)) return style;
  const fallbacks = fallback === undefined ? [] : Array.isArray(fallback) ? fallback : [fallback];
  for (const next of fallbacks) {
    const resolved = mergeThemeStyles(themeStyleCandidates(next).map((candidate) => theme.styles[candidate as ThemeStyleKey]));
    if (!isThemeStyleEmpty(resolved)) return resolved;
  }
  return undefined;
}

export function themeCssVariables(theme?: ResolvedThemeDefinition): Record<string, string> {
  if (!theme) return {};
  const vars: Record<string, string> = {};

  for (const [name, value] of Object.entries(theme.palette)) {
    vars[`--theme-palette-${themeCssSegment(name)}`] = value;
  }

  for (const [key, style] of Object.entries(theme.styles)) {
    const base = `--theme-${themeCssSegment(key)}`;
    if (style.fg) vars[`${base}-fg`] = style.fg;
    if (style.bg) vars[`${base}-bg`] = style.bg;
    if (style.underline?.color) vars[`${base}-underline-color`] = style.underline.color;
    if (style.underline?.style) vars[`${base}-underline-style`] = style.underline.style;
  }

  const background = resolveThemeStyle(theme, "ui.background")?.bg;
  const foreground = resolveThemeStyle(theme, "ui.text")?.fg;
  if (background) vars["--theme-background"] = background;
  if (foreground) vars["--theme-foreground"] = foreground;

  return vars;
}

export function themeCssSegment(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
}

function resolveThemeStyleValue(value: ThemeStyleValue, palette: Record<string, string>): ResolvedThemeStyle {
  if (typeof value === "string") return { fg: palette[value] ?? value };
  return {
    fg: paletteColor(palette, value.fg),
    bg: paletteColor(palette, value.bg),
    modifiers: value.modifiers ? [...value.modifiers] : undefined,
    underline: value.underline
      ? {
          color: paletteColor(palette, value.underline.color),
          style: value.underline.style,
        }
      : undefined,
  };
}

function resolveGhosttyTheme(ghostty: ThemeGhosttyPalette, palette: Record<string, string>): ResolvedThemeGhosttyPalette {
  return {
    background: paletteColor(palette, ghostty.background),
    foreground: paletteColor(palette, ghostty.foreground),
    cursorColor: paletteColor(palette, ghostty.cursorColor),
    selectionBackground: paletteColor(palette, ghostty.selectionBackground),
    selectionForeground: paletteColor(palette, ghostty.selectionForeground),
    palette: ghostty.palette
      ? Object.fromEntries(Object.entries(ghostty.palette).map(([slot, color]) => [slot, paletteColor(palette, color) ?? color]))
      : undefined,
  };
}

function paletteColor(palette: Record<string, string>, value?: string): string | undefined {
  if (!value || value === "default") return undefined;
  return palette[value] ?? defaultThemePalette[value] ?? value;
}

function themeStyleCandidates(key: string): string[] {
  const parts = key.split(".").filter(Boolean);
  const candidates: string[] = [];
  for (let index = 0; index < parts.length; index += 1) {
    candidates.push(parts.slice(0, index + 1).join("."));
  }
  return candidates;
}

function mergeThemeStyles(styles: Array<ResolvedThemeStyle | undefined>): ResolvedThemeStyle {
  const merged: ResolvedThemeStyle = {};
  for (const style of styles) {
    if (!style) continue;
    if (style.fg !== undefined) merged.fg = style.fg;
    if (style.bg !== undefined) merged.bg = style.bg;
    if (style.modifiers !== undefined) merged.modifiers = [...style.modifiers];
    if (style.underline !== undefined) {
      merged.underline = {
        ...(merged.underline ?? {}),
        ...style.underline,
      };
    }
  }
  return merged;
}

function isThemeStyleEmpty(style: ResolvedThemeStyle | undefined): boolean {
  return !style || (
    style.fg === undefined
    && style.bg === undefined
    && style.modifiers === undefined
    && style.underline?.color === undefined
    && style.underline?.style === undefined
  );
}
