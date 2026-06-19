import { PackageActivationContext } from "@the-editor/extensions";
import { Context, Data, Effect, Layer, Ref, type Scope, type Stream } from "effect";
import { makeStateCell } from "@the-editor/kernel";
import type { ContributionOwner } from "@the-editor/kernel";
import { defaultFontConfig } from "./defaults.js";
import type {
  FontConfigDefinition,
  FontFaceDefinition,
  FontFeatureSettings,
  FontFeatureValue,
  FontSlotConfig,
  FontSlotDefinition,
  FontStyle,
  FontWeight,
  ResolvedFontSlot,
  ResolvedFontSystem,
} from "./types.js";

export class FontDuplicateContributionError extends Data.TaggedError(
  "FontDuplicateContributionError",
)<{
  readonly contributionId: string;
}> {}

export class FontConfigurationError extends Data.TaggedError("FontConfigurationError")<{
  readonly message: string;
}> {}

export type FontError = FontDuplicateContributionError | FontConfigurationError;

export interface OwnedFontFaceContribution {
  readonly owner: ContributionOwner;
  readonly contribution: FontFaceDefinition;
}

export interface OwnedFontConfigContribution {
  readonly owner: ContributionOwner;
  readonly contribution: FontConfigDefinition;
}

export declare namespace Fonts {
  export interface Service {
    readonly registerFace: (
      face: FontFaceDefinition,
    ) => Effect.Effect<void, FontError, Scope.Scope | PackageActivationContext>;
    readonly configure: (
      config: FontConfigDefinition,
    ) => Effect.Effect<void, FontError, Scope.Scope | PackageActivationContext>;
    readonly faces: Effect.Effect<readonly OwnedFontFaceContribution[]>;
    readonly configs: Effect.Effect<readonly OwnedFontConfigContribution[]>;
    readonly resolved: Effect.Effect<ResolvedFontSystem>;
    readonly changes: Stream.Stream<ResolvedFontSystem>;
    readonly resolveSlot: (slotId: string) => Effect.Effect<ResolvedFontSlot, FontConfigurationError>;
  }
}

export class Fonts extends Context.Tag("@the-editor/fonts/Fonts")<Fonts, Fonts.Service>() {}

const CSS_GENERIC_FAMILIES: Record<string, true> = {
  serif: true,
  "sans-serif": true,
  monospace: true,
  cursive: true,
  fantasy: true,
  "system-ui": true,
  "ui-serif": true,
  "ui-sans-serif": true,
  "ui-monospace": true,
  "ui-rounded": true,
  emoji: true,
  math: true,
  fangsong: true,
  inherit: true,
  initial: true,
  unset: true,
  revert: true,
  "revert-layer": true,
  "-apple-system": true,
  BlinkMacSystemFont: true,
};

const hasCssIdentifierShape = (family: string): boolean => /^-?[_a-zA-Z][-_a-zA-Z0-9]*$/.test(family);

export const cssFontFamily = (family: string): string => {
  if (CSS_GENERIC_FAMILIES[family] === true || hasCssIdentifierShape(family)) return family;
  return JSON.stringify(family);
};

export const cssFontFamilyList = (family: string, fallbacks: readonly string[]): string =>
  [family, ...fallbacks].map(cssFontFamily).join(", ");

const featureTagIsValid = (tag: string): boolean => tag.length === 4;

export const cssFontFeatureSettings = (features: FontFeatureSettings): string => {
  const entries = Object.entries(features).filter(([tag]) => featureTagIsValid(tag));
  if (entries.length === 0) return "normal";
  return entries
    .map(([tag, value]) => {
      const cssValue = value === true ? 1 : value === false ? 0 : value;
      return `${JSON.stringify(tag)} ${cssValue}`;
    })
    .join(", ");
};

const slotCssKey = (slotId: string): string => slotId.replaceAll(".", "-").replaceAll("_", "-");

const lineHeightCss = (lineHeight: number): string =>
  lineHeight >= 4 ? `${lineHeight}px` : String(lineHeight);

const validateFace = (face: FontFaceDefinition): Effect.Effect<void, FontConfigurationError> =>
  Effect.try({
    try: () => {
      if (face.id.trim() === "") throw new FontConfigurationError({ message: "Font face id must be non-empty" });
      if (face.family.trim() === "") {
        throw new FontConfigurationError({ message: `Font face ${face.id} needs a family` });
      }
      if (face.source.kind === "file" && face.source.path.trim() === "") {
        throw new FontConfigurationError({ message: `Font face ${face.id} needs a file path` });
      }
      if (face.source.kind === "url" && face.source.url.trim() === "") {
        throw new FontConfigurationError({ message: `Font face ${face.id} needs a URL` });
      }
    },
    catch: (error) =>
      error instanceof FontConfigurationError
        ? error
        : new FontConfigurationError({ message: String(error) }),
  });

const validateConfig = (config: FontConfigDefinition): Effect.Effect<void, FontConfigurationError> =>
  Effect.try({
    try: () => {
      if (config.id.trim() === "") throw new FontConfigurationError({ message: "Font config id must be non-empty" });
      for (const [slotId, slot] of Object.entries(config.slots)) {
        if (slotId.trim() === "") {
          throw new FontConfigurationError({ message: `Font config ${config.id} has an empty slot id` });
        }
        if (slot.size !== undefined && (!Number.isFinite(slot.size) || slot.size <= 0)) {
          throw new FontConfigurationError({ message: `Font slot ${slotId} size must be positive` });
        }
        if (slot.lineHeight !== undefined && (!Number.isFinite(slot.lineHeight) || slot.lineHeight <= 0)) {
          throw new FontConfigurationError({ message: `Font slot ${slotId} line height must be positive` });
        }
        if (slot.features !== undefined) {
          for (const [tag, value] of Object.entries(slot.features)) {
            if (!featureTagIsValid(tag)) {
              throw new FontConfigurationError({ message: `Font feature ${JSON.stringify(tag)} must be four characters` });
            }
            if (typeof value === "number" && !Number.isFinite(value)) {
              throw new FontConfigurationError({ message: `Font feature ${tag} must be finite` });
            }
          }
        }
      }
    },
    catch: (error) =>
      error instanceof FontConfigurationError
        ? error
        : new FontConfigurationError({ message: String(error) }),
  });

const patchSlot = (base: FontSlotDefinition | undefined, patch: FontSlotDefinition): FontSlotDefinition => {
  const next: {
    extends?: string;
    family?: string;
    fallbacks?: readonly string[];
    size?: number;
    weight?: FontWeight;
    style?: FontStyle;
    lineHeight?: number;
    features?: FontFeatureSettings;
  } = { ...(base ?? {}) };
  if (patch.extends !== undefined) next.extends = patch.extends;
  if (patch.family !== undefined) next.family = patch.family;
  if (patch.fallbacks !== undefined) next.fallbacks = patch.fallbacks;
  if (patch.size !== undefined) next.size = patch.size;
  if (patch.weight !== undefined) next.weight = patch.weight;
  if (patch.style !== undefined) next.style = patch.style;
  if (patch.lineHeight !== undefined) next.lineHeight = patch.lineHeight;
  if (patch.features !== undefined) next.features = { ...(base?.features ?? {}), ...patch.features };
  return next;
};

const mergeFontConfigs = (configs: readonly FontConfigDefinition[]): FontSlotConfig => {
  const slots: Record<string, FontSlotDefinition> = {};
  for (const config of configs) {
    for (const [slotId, patch] of Object.entries(config.slots)) {
      slots[slotId] = patchSlot(slots[slotId], patch);
    }
  }
  return slots;
};

const resolveSlotFromConfig = (
  slots: FontSlotConfig,
  slotId: string,
  stack: readonly string[] = [],
): ResolvedFontSlot => {
  if (stack.includes(slotId)) {
    throw new FontConfigurationError({ message: `Font slot inheritance cycle: ${[...stack, slotId].join(" -> ")}` });
  }
  const slot = slots[slotId];
  if (slot === undefined) throw new FontConfigurationError({ message: `Font slot not found: ${slotId}` });

  const parent = slot.extends === undefined ? undefined : resolveSlotFromConfig(slots, slot.extends, [...stack, slotId]);
  const family = slot.family ?? parent?.family;
  if (family === undefined || family.trim() === "") {
    throw new FontConfigurationError({ message: `Font slot ${slotId} needs a family` });
  }
  const fallbacks = slot.fallbacks ?? parent?.fallbacks ?? [];
  const features = { ...(parent?.features ?? {}), ...(slot.features ?? {}) } satisfies Record<
    string,
    FontFeatureValue
  >;
  const size = slot.size ?? parent?.size ?? 13;
  const weight = slot.weight ?? parent?.weight ?? 400;
  const style = slot.style ?? parent?.style ?? "normal";
  const lineHeight = slot.lineHeight ?? parent?.lineHeight ?? Math.round(size * 1.45);
  return {
    id: slotId,
    family,
    fallbacks,
    cssFamily: cssFontFamilyList(family, fallbacks),
    size,
    weight,
    style,
    lineHeight,
    features,
    cssFeatureSettings: cssFontFeatureSettings(features),
  };
};

export const resolveFontSystem = (
  faces: readonly FontFaceDefinition[],
  configs: readonly FontConfigDefinition[],
): ResolvedFontSystem => {
  const slots = mergeFontConfigs([defaultFontConfig, ...configs]);
  const resolvedSlots: Record<string, ResolvedFontSlot> = {};
  for (const slotId of Object.keys(slots)) {
    resolvedSlots[slotId] = resolveSlotFromConfig(slots, slotId);
  }
  const cssVars: Record<string, string> = {};
  for (const slot of Object.values(resolvedSlots)) {
    const key = slotCssKey(slot.id);
    cssVars[`--font-${key}-family`] = slot.cssFamily;
    cssVars[`--font-${key}-size`] = `${slot.size}px`;
    cssVars[`--font-${key}-weight`] = String(slot.weight);
    cssVars[`--font-${key}-style`] = slot.style;
    cssVars[`--font-${key}-line-height`] = lineHeightCss(slot.lineHeight);
    cssVars[`--font-${key}-feature-settings`] = slot.cssFeatureSettings;
  }
  return { faces, slots: resolvedSlots, cssVars };
};

const duplicateId = <A extends { readonly contribution: { readonly id: string } }>(
  entries: readonly A[],
  id: string,
): boolean => entries.some((entry) => entry.contribution.id === id);

export const makeFonts: Effect.Effect<Fonts.Service> = Effect.gen(function* () {
  const faceEntries = yield* Ref.make<OwnedFontFaceContribution[]>([]);
  const configEntries = yield* Ref.make<OwnedFontConfigContribution[]>([]);
  const resolvedCell = yield* makeStateCell<ResolvedFontSystem>(resolveFontSystem([], []));

  const recompute = Effect.gen(function* () {
    const faces = (yield* Ref.get(faceEntries)).map((entry) => entry.contribution);
    const configs = (yield* Ref.get(configEntries)).map((entry) => entry.contribution);
    yield* resolvedCell.set(resolveFontSystem(faces, configs));
  });

  return {
    registerFace: (face) =>
      Effect.gen(function* () {
        yield* validateFace(face);
        const activation = yield* PackageActivationContext;
        const entry: OwnedFontFaceContribution = { owner: activation.owner, contribution: face };
        const currentFaces = yield* Ref.get(faceEntries);
        if (duplicateId(currentFaces, face.id)) return yield* new FontDuplicateContributionError({ contributionId: face.id });
        const nextFaces = [...currentFaces, entry];
        const configs = (yield* Ref.get(configEntries)).map((item) => item.contribution);
        yield* Effect.try({
          try: () => resolveFontSystem(nextFaces.map((item) => item.contribution), configs),
          catch: (error) =>
            error instanceof FontConfigurationError
              ? error
              : new FontConfigurationError({ message: String(error) }),
        });
        yield* Ref.set(faceEntries, nextFaces);
        yield* recompute;
        yield* Effect.addFinalizer(() =>
          Effect.gen(function* () {
            yield* Ref.update(faceEntries, (entries) => entries.filter((item) => item !== entry));
            yield* recompute;
          }),
        );
      }),
    configure: (config) =>
      Effect.gen(function* () {
        yield* validateConfig(config);
        const activation = yield* PackageActivationContext;
        const entry: OwnedFontConfigContribution = { owner: activation.owner, contribution: config };
        const currentConfigs = yield* Ref.get(configEntries);
        if (duplicateId(currentConfigs, config.id)) {
          return yield* new FontDuplicateContributionError({ contributionId: config.id });
        }
        const nextConfigs = [...currentConfigs, entry];
        const faces = (yield* Ref.get(faceEntries)).map((item) => item.contribution);
        yield* Effect.try({
          try: () => resolveFontSystem(faces, nextConfigs.map((item) => item.contribution)),
          catch: (error) =>
            error instanceof FontConfigurationError
              ? error
              : new FontConfigurationError({ message: String(error) }),
        });
        yield* Ref.set(configEntries, nextConfigs);
        yield* recompute;
        yield* Effect.addFinalizer(() =>
          Effect.gen(function* () {
            yield* Ref.update(configEntries, (entries) => entries.filter((item) => item !== entry));
            yield* recompute;
          }),
        );
      }),
    faces: Ref.get(faceEntries),
    configs: Ref.get(configEntries),
    resolved: resolvedCell.get,
    changes: resolvedCell.changes,
    resolveSlot: (slotId) =>
      resolvedCell.get.pipe(
        Effect.flatMap((system) => {
          const slot = system.slots[slotId];
          return slot === undefined
            ? Effect.fail(new FontConfigurationError({ message: `Font slot not found: ${slotId}` }))
            : Effect.succeed(slot);
        }),
      ),
  } satisfies Fonts.Service;
});

export const FontsLive = Layer.effect(Fonts, makeFonts);
