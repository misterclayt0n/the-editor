export type Appearance = "light" | "dark";
export type ThemeMode = Appearance | "system";

export type FontStyleContent = "normal" | "italic" | "oblique";
export type FontWeightContent = 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;

export interface HighlightStyleContent {
  readonly color?: string | null;
  readonly background_color?: string | null;
  readonly font_style?: FontStyleContent | null;
  readonly font_weight?: FontWeightContent | null;
}

export interface HighlightStyle {
  readonly color?: string;
  readonly backgroundColor?: string;
  readonly fontStyle?: FontStyleContent;
  readonly fontWeight?: FontWeightContent;
}

export interface PlayerColorContent {
  readonly cursor?: string | null;
  readonly background?: string | null;
  readonly selection?: string | null;
}

export interface PlayerColor {
  readonly cursor: string;
  readonly background: string;
  readonly selection: string;
}

type ThemeStyleObject =
  | readonly (string | null)[]
  | readonly PlayerColorContent[]
  | Readonly<Record<string, HighlightStyleContent>>;

export interface ThemeStyleContent {
  readonly accents?: readonly (string | null)[];
  readonly players?: readonly PlayerColorContent[];
  readonly syntax?: Readonly<Record<string, HighlightStyleContent>>;
  readonly [key: string]: string | null | ThemeStyleObject | undefined;
}

export interface ThemeContent {
  readonly name: string;
  readonly appearance: Appearance;
  readonly style: ThemeStyleContent;
}

export interface ThemeFamilyContent {
  readonly $schema?: string;
  readonly name: string;
  readonly author: string;
  readonly themes: readonly ThemeContent[];
}

export interface ThemeMeta {
  readonly name: string;
  readonly family: string;
  readonly author: string;
  readonly appearance: Appearance;
}

export interface SyntaxTheme {
  readonly entries: readonly (readonly [string, HighlightStyle])[];
  readonly resolve: (captureName: string) => HighlightStyle | undefined;
}

export type ThemeColors = Readonly<Record<string, string>>;

export interface ResolvedTheme extends ThemeMeta {
  readonly colors: ThemeColors;
  readonly accents: readonly string[];
  readonly players: readonly PlayerColor[];
  readonly syntax: SyntaxTheme;
  readonly cssVars: Readonly<Record<string, string>>;
}

export interface ThemeSelection {
  readonly mode: ThemeMode;
  readonly light: string;
  readonly dark: string;
}
