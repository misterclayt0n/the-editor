import { PackageActivationContext } from "@the-editor/extensions";
import { Context, Effect, Option, Ref, Stream, type Scope } from "effect";
import { makeStateCell } from "@the-editor/kernel";
import { defaultThemeSelection } from "./defaults.js";
import type {
  Appearance,
  HighlightStyle,
  HighlightStyleContent,
  PlayerColor,
  ResolvedTheme,
  SyntaxTheme,
  ThemeContent,
  ThemeFamilyContent,
  ThemeMeta,
  ThemeMode,
  ThemeSelection,
  ThemeStyleContent,
} from "./types.js";

export class ThemeValidationError extends Error {
  readonly _tag = "ThemeValidationError";
  constructor(message: string) {
    super(message);
  }
}

export class ThemeDuplicateError extends Error {
  readonly _tag = "ThemeDuplicateError";
  constructor(readonly themeName: string) {
    super(`Theme already registered: ${themeName}`);
  }
}

export class ThemeNotFoundError extends Error {
  readonly _tag = "ThemeNotFoundError";
  constructor(readonly themeName: string) {
    super(`Theme not found: ${themeName}`);
  }
}

export type ThemeError = ThemeValidationError | ThemeDuplicateError | ThemeNotFoundError;

type RegisteredTheme = {
  readonly ownerKey: string;
  readonly theme: ResolvedTheme;
};

type PreviewState = {
  readonly originalSelection: ThemeSelection;
};

const STYLE_OBJECT_KEYS: Record<string, true> = { accents: true, players: true, syntax: true };

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const isAppearance = (value: unknown): value is Appearance => value === "light" || value === "dark";

const stringOrNull = (value: unknown): value is string | null =>
  typeof value === "string" || value === null;

export const parseThemeFamily = (raw: unknown): Effect.Effect<ThemeFamilyContent, ThemeValidationError> =>
  Effect.gen(function* () {
    if (!isObject(raw)) return yield* Effect.fail(new ThemeValidationError("Theme family must be an object"));
    if (typeof raw.name !== "string") {
      return yield* Effect.fail(new ThemeValidationError("Theme family needs a name"));
    }
    if (typeof raw.author !== "string") {
      return yield* Effect.fail(new ThemeValidationError("Theme family needs an author"));
    }
    if (!Array.isArray(raw.themes)) {
      return yield* Effect.fail(new ThemeValidationError("Theme family needs themes"));
    }

    const themes: ThemeContent[] = [];
    for (const item of raw.themes) {
      if (!isObject(item)) return yield* Effect.fail(new ThemeValidationError("Theme entry must be an object"));
      if (typeof item.name !== "string") {
        return yield* Effect.fail(new ThemeValidationError("Theme entry needs a name"));
      }
      if (!isAppearance(item.appearance)) {
        return yield* Effect.fail(new ThemeValidationError(`Theme ${item.name} needs light or dark appearance`));
      }
      if (!isObject(item.style)) {
        return yield* Effect.fail(new ThemeValidationError(`Theme ${item.name} needs a style object`));
      }
      const style: Record<string, ThemeStyleContent[string]> = {};
      for (const [key, value] of Object.entries(item.style)) {
        if (STYLE_OBJECT_KEYS[key] === true) {
          style[key] = value as ThemeStyleContent[string];
        } else if (stringOrNull(value)) {
          style[key] = value;
        }
      }
      themes.push({ name: item.name, appearance: item.appearance, style });
    }
    return { name: raw.name, author: raw.author, themes };
  });

const toStyle = (content: HighlightStyleContent | undefined): HighlightStyle | undefined => {
  if (content === undefined) return undefined;
  const out: {
    color?: string;
    backgroundColor?: string;
    fontStyle?: NonNullable<HighlightStyle["fontStyle"]>;
    fontWeight?: NonNullable<HighlightStyle["fontWeight"]>;
  } = {};
  if (typeof content.color === "string") out.color = content.color;
  if (typeof content.background_color === "string") out.backgroundColor = content.background_color;
  if (content.font_style !== undefined && content.font_style !== null) out.fontStyle = content.font_style;
  if (content.font_weight !== undefined && content.font_weight !== null) out.fontWeight = content.font_weight;
  return Object.keys(out).length === 0 ? undefined : out;
};

const zedCompatibleCaptureName = (captureName: string): string => {
  if (captureName === "variable.other.member" || captureName.startsWith("variable.other.member.")) {
    return "property";
  }
  if (captureName === "constant.numeric" || captureName.startsWith("constant.numeric.")) {
    return "number";
  }
  if (captureName === "constant.character.escape") return "string.escape";
  if (captureName === "string.regexp" || captureName.startsWith("string.regexp.")) {
    return "string.regex";
  }
  if (captureName === "variable.builtin" || captureName.startsWith("variable.builtin.")) {
    return "variable.special";
  }
  return captureName;
};

const mergeSyntax = (
  base: Readonly<Record<string, HighlightStyleContent>>,
  override: Readonly<Record<string, HighlightStyleContent>> | undefined,
): SyntaxTheme => {
  const map = new Map<string, HighlightStyle>();
  for (const [key, value] of Object.entries(base)) {
    const style = toStyle(value);
    if (style !== undefined) map.set(key, style);
  }
  for (const [key, value] of Object.entries(override ?? {})) {
    const existing = map.get(key) ?? {};
    const next = toStyle(value);
    if (next !== undefined) map.set(key, { ...existing, ...next });
  }
  const entries = [...map.entries()] as readonly (readonly [string, HighlightStyle])[];
  const cache = new Map<string, HighlightStyle | undefined>();
  const bestStyle = (captureName: string): HighlightStyle | undefined => {
    let best: HighlightStyle | undefined;
    let bestScore = -1;
    const parts = captureName.split(".");
    for (const [key, style] of entries) {
      const keyParts = key.split(".");
      if (keyParts.every((part) => parts.includes(part)) && keyParts.length > bestScore) {
        best = style;
        bestScore = keyParts.length;
      }
    }
    return best;
  };
  const resolve = (captureName: string): HighlightStyle | undefined => {
    if (cache.has(captureName)) return cache.get(captureName);
    let best = map.get(captureName);
    if (best === undefined) {
      const alias = zedCompatibleCaptureName(captureName);
      best = alias === captureName ? bestStyle(captureName) : (map.get(alias) ?? bestStyle(alias));
    }
    cache.set(captureName, best);
    return best;
  };
  return { entries, resolve };
};

const colorEntries = (style: ThemeStyleContent): Record<string, string> => {
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(style)) {
    if (typeof value === "string") out[key] = value;
  }
  return out;
};

const resolvePlayers = (style: ThemeStyleContent, colors: Record<string, string>): readonly PlayerColor[] => {
  const players = Array.isArray(style.players) ? style.players : [];
  const fallbackCursor = colors["text.accent"] ?? colors.text ?? colors["editor.foreground"]!;
  const fallbackSelection = colors["search.match_background"] ?? fallbackCursor;
  const resolved = players.flatMap((player): PlayerColor[] => {
    if (!isObject(player)) return [];
    return [
      {
        cursor: typeof player.cursor === "string" ? player.cursor : fallbackCursor,
        background: typeof player.background === "string" ? player.background : fallbackCursor,
        selection: typeof player.selection === "string" ? player.selection : fallbackSelection,
      },
    ];
  });
  return resolved.length > 0
    ? resolved
    : [{ cursor: fallbackCursor, background: fallbackCursor, selection: fallbackSelection }];
};

const resolveAccents = (style: ThemeStyleContent, colors: Record<string, string>): readonly string[] => {
  const accents = Array.isArray(style.accents) ? style.accents.filter((value): value is string => typeof value === "string") : [];
  return accents.length > 0 ? accents : [colors["text.accent"] ?? colors.text!];
};

const cssVarName = (key: string) => `--theme-${key.replaceAll(".", "-").replaceAll("_", "-")}`;

const makeCssVars = (
  colors: Record<string, string>,
  appearance: Appearance,
  players: readonly PlayerColor[],
  syntax: SyntaxTheme,
): Readonly<Record<string, string>> => {
  const v = (key: string, fallback: string): string => colors[key] ?? colors[fallback] ?? "transparent";
  const css: Record<string, string> = { "--theme-color-scheme": appearance };
  for (const [key, value] of Object.entries(colors)) css[cssVarName(key)] = value;
  for (const [key, style] of syntax.entries) {
    if (style.color !== undefined) css[cssVarName(key)] = style.color;
    if (style.color !== undefined) css[`--theme-syntax-${key.replaceAll(".", "-").replaceAll("_", "-")}`] = style.color;
  }

  css["--bg"] = v("background", "editor.background");
  css["--bg-panel"] = v("panel.background", "surface.background");
  css["--bg-editor"] = v("editor.background", "background");
  css["--fg"] = v("text", "editor.foreground");
  css["--fg-dim"] = v("text.muted", "editor.line_number");
  css["--caret"] = players[0]?.cursor ?? v("text.accent", "text");
  css["--accent"] = v("text.accent", "border.focused");
  css["--selection"] = players[0]?.selection ?? v("search.match_background", "text.accent");
  css["--status-bg"] = v("status_bar.background", "background");
  css["--border"] = v("border", "border.variant");

  css["--theme-player-cursor"] = players[0]?.cursor ?? css["--caret"]!;
  css["--theme-player-selection"] = players[0]?.selection ?? css["--selection"]!;
  css["--theme-diagnostic-error"] = v("error", "deleted");
  css["--theme-diagnostic-warning"] = v("warning", "modified");
  css["--theme-diagnostic-info"] = v("info", "text.accent");
  css["--theme-diagnostic-hint"] = v("hint", "text.muted");
  return css;
};

export const resolveTheme = (
  family: Pick<ThemeFamilyContent, "name" | "author">,
  content: ThemeContent,
  baseContent: ThemeContent,
): ResolvedTheme => {
  const style = { ...baseContent.style, ...content.style } satisfies ThemeStyleContent;
  const colors = colorEntries(style);
  const accents = resolveAccents(style, colors);
  const players = resolvePlayers(style, colors);
  const syntax = mergeSyntax(baseContent.style.syntax ?? {}, content.style.syntax);
  return {
    name: content.name,
    family: family.name,
    author: family.author,
    appearance: content.appearance,
    colors,
    accents,
    players,
    syntax,
    cssVars: makeCssVars(colors, content.appearance, players, syntax),
  };
};

export declare namespace Themes {
  export interface Service {
    readonly registerFamily: (
      family: ThemeFamilyContent,
    ) => Effect.Effect<void, ThemeError, Scope.Scope | PackageActivationContext>;
    readonly list: Effect.Effect<readonly ThemeMeta[]>;
    readonly get: (name: string) => Effect.Effect<ResolvedTheme | undefined>;
    readonly active: Effect.Effect<ResolvedTheme, ThemeNotFoundError>;
    readonly changes: Stream.Stream<ResolvedTheme>;
    readonly selection: Effect.Effect<ThemeSelection>;
    readonly systemAppearance: Effect.Effect<Appearance>;
    readonly setSystemAppearance: (appearance: Appearance) => Effect.Effect<void, ThemeNotFoundError>;
    readonly setMode: (mode: ThemeMode) => Effect.Effect<void, ThemeNotFoundError>;
    readonly toggleMode: Effect.Effect<void, ThemeNotFoundError>;
    readonly setTheme: (name: string) => Effect.Effect<void, ThemeNotFoundError>;
    readonly preview: (name: string) => Effect.Effect<void, ThemeNotFoundError>;
    readonly commitPreview: Effect.Effect<void>;
    readonly cancelPreview: Effect.Effect<void, ThemeNotFoundError>;
    readonly syntaxStyle: (captureName: string) => Effect.Effect<HighlightStyle | undefined, ThemeNotFoundError>;
  }
}

export class Themes extends Context.Tag("@the-editor/theme/Themes")<Themes, Themes.Service>() {}

const themeNameFor = (selection: ThemeSelection, systemAppearance: Appearance): string => {
  const appearance = selection.mode === "system" ? systemAppearance : selection.mode;
  return appearance === "light" ? selection.light : selection.dark;
};

const selectionForTheme = (
  selection: ThemeSelection,
  systemAppearance: Appearance,
  theme: ResolvedTheme,
): ThemeSelection => {
  const mode = selection.mode === "system" && theme.appearance === systemAppearance
    ? "system"
    : theme.appearance;
  return theme.appearance === "light"
    ? { ...selection, mode, light: theme.name }
    : { ...selection, mode, dark: theme.name };
};

export const makeThemes = (initialSelection: ThemeSelection = defaultThemeSelection): Effect.Effect<Themes.Service> =>
  Effect.gen(function* () {
    const themes = yield* Ref.make(new Map<string, RegisteredTheme>());
    const selectionCell = yield* makeStateCell(initialSelection);
    const systemAppearanceCell = yield* makeStateCell<Appearance>("dark");
    const activeCell = yield* makeStateCell<ResolvedTheme | undefined>(undefined);
    const preview = yield* Ref.make<PreviewState | undefined>(undefined);

    const resolveActive = Effect.gen(function* () {
      const selection = yield* selectionCell.get;
      const systemAppearance = yield* systemAppearanceCell.get;
      const name = themeNameFor(selection, systemAppearance);
      const theme = (yield* Ref.get(themes)).get(name)?.theme;
      if (theme === undefined) return yield* Effect.fail(new ThemeNotFoundError(name));
      yield* activeCell.set(theme);
    });

    const active = activeCell.get.pipe(
      Effect.flatMap((theme) => theme === undefined ? Effect.fail(new ThemeNotFoundError(themeNameFor(initialSelection, "dark"))) : Effect.succeed(theme)),
    );

    const updateSelection = (f: (selection: ThemeSelection) => ThemeSelection) =>
      Effect.gen(function* () {
        yield* selectionCell.update(f);
        yield* resolveActive;
      });

    const requireTheme = (name: string) =>
      Ref.get(themes).pipe(
        Effect.flatMap((map) => {
          const theme = map.get(name)?.theme;
          return theme === undefined ? Effect.fail(new ThemeNotFoundError(name)) : Effect.succeed(theme);
        }),
      );

    const setTheme = (name: string) =>
      Effect.gen(function* () {
        const theme = yield* requireTheme(name);
        const systemAppearance = yield* systemAppearanceCell.get;
        yield* updateSelection((selection) => selectionForTheme(selection, systemAppearance, theme));
      });

    return {
      registerFamily: (family) =>
        Effect.gen(function* () {
          const activation = yield* PackageActivationContext;
          const ownerKey = `${activation.packageId}:${activation.activationId}`;
          const baseLight = family.themes.find((theme) => theme.appearance === "light") ?? family.themes[0];
          const baseDark = family.themes.find((theme) => theme.appearance === "dark") ?? family.themes[0];
          if (baseLight === undefined || baseDark === undefined) {
            return yield* Effect.fail(new ThemeValidationError(`Theme family ${family.name} has no themes`));
          }
          const resolved = family.themes.map((theme) =>
            resolveTheme(family, theme, theme.appearance === "light" ? baseLight : baseDark),
          );
          const inserted = yield* Ref.modify(themes, (map) => {
            for (const theme of resolved) if (map.has(theme.name)) return [false, map] as const;
            const next = new Map(map);
            for (const theme of resolved) next.set(theme.name, { ownerKey, theme });
            return [true, next] as const;
          });
          if (!inserted) return yield* Effect.fail(new ThemeDuplicateError(resolved[0]?.name ?? family.name));
          yield* Effect.addFinalizer(() =>
            Ref.update(themes, (map) => {
              const next = new Map(map);
              for (const [name, entry] of next) if (entry.ownerKey === ownerKey) next.delete(name);
              return next;
            }),
          );
          yield* resolveActive.pipe(Effect.catchAll(() => Effect.void));
        }),
      list: Ref.get(themes).pipe(
        Effect.map((map) =>
          [...map.values()]
            .map(({ theme }): ThemeMeta => ({
              name: theme.name,
              family: theme.family,
              author: theme.author,
              appearance: theme.appearance,
            }))
            .sort((a, b) => Number(a.appearance === "light") - Number(b.appearance === "light") || a.name.localeCompare(b.name)),
        ),
      ),
      get: (name) => Ref.get(themes).pipe(Effect.map((map) => map.get(name)?.theme)),
      active,
      changes: activeCell.changes.pipe(Stream.filter((theme): theme is ResolvedTheme => theme !== undefined)),
      selection: selectionCell.get,
      systemAppearance: systemAppearanceCell.get,
      setSystemAppearance: (appearance) =>
        Effect.gen(function* () {
          yield* systemAppearanceCell.set(appearance);
          yield* resolveActive;
        }),
      setMode: (mode) => updateSelection((selection) => ({ ...selection, mode })),
      toggleMode: Effect.gen(function* () {
        const current = yield* active;
        yield* updateSelection((selection) => ({
          ...selection,
          mode: current.appearance === "dark" ? "light" : "dark",
        }));
      }),
      setTheme: (name) =>
        Effect.gen(function* () {
          yield* setTheme(name);
          yield* Ref.set(preview, undefined);
        }),
      preview: (name) =>
        Effect.gen(function* () {
          const currentPreview = yield* Ref.get(preview);
          if (currentPreview === undefined) {
            const originalSelection = yield* selectionCell.get;
            yield* Ref.set(preview, { originalSelection });
          }
          yield* setTheme(name);
        }),
      commitPreview: Ref.set(preview, undefined),
      cancelPreview: Effect.gen(function* () {
        const currentPreview = yield* Ref.get(preview);
        if (currentPreview === undefined) return;
        yield* Ref.set(preview, undefined);
        yield* selectionCell.set(currentPreview.originalSelection);
        yield* resolveActive;
      }),
      syntaxStyle: (captureName) => active.pipe(Effect.map((theme) => theme.syntax.resolve(captureName))),
    } satisfies Themes.Service;
  });

export const activeThemeOption = (themes: Themes.Service): Effect.Effect<Option.Option<ResolvedTheme>> =>
  themes.active.pipe(
    Effect.map(Option.some),
    Effect.catchAll(() => Effect.succeed(Option.none())),
  );
