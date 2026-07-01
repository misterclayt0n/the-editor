/** Public diff-view preference registry. */

import { Context, Effect, type Scope } from "effect";

export type DiffViewLayout = "split" | "stacked";
export type DiffViewIndicators = "bars" | "classic" | "none";
export type DiffViewLineDiff = "word-alt" | "word" | "char" | "none";
export type DiffViewHunkSeparators =
  | "line-info"
  | "line-info-basic"
  | "metadata"
  | "simple"
  | "custom-css";

export type DiffViewSettings = {
  readonly layout: DiffViewLayout;
  readonly indicators: DiffViewIndicators;
  readonly lineDiff: DiffViewLineDiff;
  readonly backgrounds: boolean;
  readonly wrapping: boolean;
  readonly lineNumbers: boolean;
  readonly controlsVisible: boolean;
  readonly hunkSeparators: DiffViewHunkSeparators;
};

export type DiffViewSettingsPatch = Partial<DiffViewSettings>;

export type DiffViewSettingsContribution = {
  readonly id: string;
  readonly settings: DiffViewSettingsPatch;
};

export const DEFAULT_DIFF_VIEW_SETTINGS: DiffViewSettings = {
  layout: "split",
  indicators: "bars",
  lineDiff: "word-alt",
  backgrounds: true,
  wrapping: false,
  lineNumbers: true,
  controlsVisible: false,
  hunkSeparators: "line-info",
};

export class DiffViewPreferences extends Context.Tag("@the-editor/diff-view/DiffViewPreferences")<
  DiffViewPreferences,
  {
    readonly register: (
      contribution: DiffViewSettingsContribution,
    ) => Effect.Effect<void, never, Scope.Scope>;
    readonly current: () => DiffViewSettings;
    readonly update: (patch: DiffViewSettingsPatch) => void;
    readonly subscribe: (listener: () => void) => () => void;
  }
>() {}

export namespace DiffViewPreferences {
  export type Service = Context.Tag.Service<DiffViewPreferences>;
}

const applyDiffViewSettingsPatch = (
  state: {
    layout: DiffViewLayout;
    indicators: DiffViewIndicators;
    lineDiff: DiffViewLineDiff;
    backgrounds: boolean;
    wrapping: boolean;
    lineNumbers: boolean;
    controlsVisible: boolean;
    hunkSeparators: DiffViewHunkSeparators;
  },
  patch: DiffViewSettingsPatch,
): void => {
  if (patch.layout !== undefined) state.layout = patch.layout;
  if (patch.indicators !== undefined) state.indicators = patch.indicators;
  if (patch.lineDiff !== undefined) state.lineDiff = patch.lineDiff;
  if (patch.backgrounds !== undefined) state.backgrounds = patch.backgrounds;
  if (patch.wrapping !== undefined) state.wrapping = patch.wrapping;
  if (patch.lineNumbers !== undefined) state.lineNumbers = patch.lineNumbers;
  if (patch.controlsVisible !== undefined) state.controlsVisible = patch.controlsVisible;
  if (patch.hunkSeparators !== undefined) state.hunkSeparators = patch.hunkSeparators;
};

const sameDiffViewSettings = (a: DiffViewSettings, b: DiffViewSettings): boolean =>
  a.layout === b.layout &&
  a.indicators === b.indicators &&
  a.lineDiff === b.lineDiff &&
  a.backgrounds === b.backgrounds &&
  a.wrapping === b.wrapping &&
  a.lineNumbers === b.lineNumbers &&
  a.controlsVisible === b.controlsVisible &&
  a.hunkSeparators === b.hunkSeparators;

export const makeDiffViewPreferences = (): DiffViewPreferences.Service => {
  const contributions: DiffViewSettingsContribution[] = [];
  const listeners = new Set<() => void>();
  let runtimeOverrides: DiffViewSettingsPatch = {};
  let effective = DEFAULT_DIFF_VIEW_SETTINGS;

  const recompute = (): DiffViewSettings => {
    const next = { ...DEFAULT_DIFF_VIEW_SETTINGS };
    for (const contribution of contributions) {
      applyDiffViewSettingsPatch(next, contribution.settings);
    }
    applyDiffViewSettingsPatch(next, runtimeOverrides);
    return next;
  };

  const commit = (next: DiffViewSettings): void => {
    if (sameDiffViewSettings(effective, next)) return;
    effective = next;
    for (const listener of listeners) listener();
  };

  return {
    register: (contribution) =>
      Effect.acquireRelease(
        Effect.sync(() => {
          contributions.push(contribution);
          commit(recompute());
        }),
        () =>
          Effect.sync(() => {
            const index = contributions.indexOf(contribution);
            if (index === -1) return;
            contributions.splice(index, 1);
            commit(recompute());
          }),
      ).pipe(Effect.asVoid),
    current: () => effective,
    update: (patch) => {
      runtimeOverrides = { ...runtimeOverrides, ...patch };
      commit(recompute());
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },
  };
};
