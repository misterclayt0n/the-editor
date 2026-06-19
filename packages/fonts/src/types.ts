export type FontStyle = "normal" | "italic" | "oblique";
export type FontDisplay = "auto" | "block" | "swap" | "fallback" | "optional";
export type FontWeight =
  | 100
  | 200
  | 300
  | 400
  | 500
  | 600
  | 700
  | 800
  | 900
  | `${number}`
  | `${number} ${number}`;

export type FontFeatureValue = boolean | number;
export type FontFeatureSettings = Readonly<Record<string, FontFeatureValue>>;

export type FontSource =
  | {
      readonly kind: "file";
      readonly path: string;
    }
  | {
      readonly kind: "url";
      readonly url: string;
    };

export interface FontFaceDefinition {
  readonly id: string;
  readonly family: string;
  readonly source: FontSource;
  readonly weight?: FontWeight;
  readonly style?: FontStyle;
  readonly display?: FontDisplay;
  readonly unicodeRange?: string;
  readonly featureSettings?: FontFeatureSettings;
}

export interface FontSlotDefinition {
  readonly extends?: string;
  readonly family?: string;
  readonly fallbacks?: readonly string[];
  readonly size?: number;
  readonly weight?: FontWeight;
  readonly style?: FontStyle;
  readonly lineHeight?: number;
  readonly features?: FontFeatureSettings;
}

export type FontSlotConfig = Readonly<Record<string, FontSlotDefinition>>;

export interface FontConfigDefinition {
  readonly id: string;
  readonly slots: FontSlotConfig;
}

export interface ResolvedFontSlot {
  readonly id: string;
  readonly family: string;
  readonly fallbacks: readonly string[];
  readonly cssFamily: string;
  readonly size: number;
  readonly weight: FontWeight;
  readonly style: FontStyle;
  readonly lineHeight: number;
  readonly features: FontFeatureSettings;
  readonly cssFeatureSettings: string;
}

export interface ResolvedFontSystem {
  readonly slots: Readonly<Record<string, ResolvedFontSlot>>;
  readonly faces: readonly FontFaceDefinition[];
  readonly cssVars: Readonly<Record<string, string>>;
}
