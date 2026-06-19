/**
 * The diff view as a product package: a global workbench tab that shows the
 * working-tree diff against HEAD. Follows the same contribution shape as other
 * center content surfaces: surface kind, host seam, command, and keybinding.
 */

import { Context, Effect, type Scope } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands, type CommandDefinition } from "@the-editor/commands";
import { Keymaps } from "@the-editor/keymaps";
import { SurfaceKind, SurfaceScope } from "@the-editor/workbench";
import { PanelRenderers, type PanelRenderer } from "../workbench/panelRenderers";

export const DiffViewSurface = SurfaceKind("the-editor/diff-view");

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

export class DiffViewPreferences extends Context.Tag("the-editor/web/DiffViewPreferences")<
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

export class HostDiffView extends Context.Tag("the-editor/web/HostDiffView")<
  HostDiffView,
  {
    readonly renderer: PanelRenderer;
    readonly open: () => void;
  }
>() {}

type DiffViewCommandOption<Value extends string> = {
  readonly id: string;
  readonly title: string;
  readonly value: Value;
  readonly aliases: readonly string[];
};

type DiffViewToggleKey = "backgrounds" | "wrapping" | "lineNumbers" | "controlsVisible";

const diffViewSettingsScope = SurfaceScope.surface(DiffViewSurface);
const diffViewPalette = (
  aliases: readonly string[],
): { readonly group: string; readonly aliases: readonly string[] } => ({
  group: "View/Diff",
  aliases,
});

const diffViewSettingCommand = (
  preferences: DiffViewPreferences.Service,
  title: string,
  patch: DiffViewSettingsPatch,
  aliases: readonly string[],
): CommandDefinition => ({
  title,
  scope: diffViewSettingsScope,
  palette: diffViewPalette(aliases),
  run: () => Effect.sync(() => preferences.update(patch)),
});

const nextDiffViewTogglePatch = (
  settings: DiffViewSettings,
  key: DiffViewToggleKey,
): DiffViewSettingsPatch => {
  switch (key) {
    case "backgrounds":
      return { backgrounds: !settings.backgrounds };
    case "wrapping":
      return { wrapping: !settings.wrapping };
    case "lineNumbers":
      return { lineNumbers: !settings.lineNumbers };
    case "controlsVisible":
      return { controlsVisible: !settings.controlsVisible };
  }
};

const diffViewToggleCommand = (
  preferences: DiffViewPreferences.Service,
  title: string,
  key: DiffViewToggleKey,
  aliases: readonly string[],
): CommandDefinition => ({
  title,
  scope: diffViewSettingsScope,
  palette: diffViewPalette(aliases),
  run: () =>
    Effect.sync(() =>
      preferences.update(nextDiffViewTogglePatch(preferences.current(), key)),
    ),
});

const DIFF_VIEW_LAYOUT_COMMANDS: readonly DiffViewCommandOption<DiffViewLayout>[] = [
  {
    id: "diffs.layout.split",
    title: "Diff View: Use Split Layout",
    value: "split",
    aliases: ["diff split", "side by side diff"],
  },
  {
    id: "diffs.layout.stacked",
    title: "Diff View: Use Stacked Layout",
    value: "stacked",
    aliases: ["diff stacked", "unified diff"],
  },
];

const DIFF_VIEW_INDICATOR_COMMANDS: readonly DiffViewCommandOption<DiffViewIndicators>[] = [
  {
    id: "diffs.indicators.bars",
    title: "Diff View: Use Bar Indicators",
    value: "bars",
    aliases: ["diff bars", "change bars"],
  },
  {
    id: "diffs.indicators.classic",
    title: "Diff View: Use Classic Indicators",
    value: "classic",
    aliases: ["diff classic", "plus minus diff"],
  },
  {
    id: "diffs.indicators.none",
    title: "Diff View: Hide Indicators",
    value: "none",
    aliases: ["diff no indicators", "hide change markers"],
  },
];

const DIFF_VIEW_LINE_DIFF_COMMANDS: readonly DiffViewCommandOption<DiffViewLineDiff>[] = [
  {
    id: "diffs.lineDiff.wordAlt",
    title: "Diff View: Use Alternate Word Line Diff",
    value: "word-alt",
    aliases: ["diff word alt", "alternate word diff"],
  },
  {
    id: "diffs.lineDiff.word",
    title: "Diff View: Use Word Line Diff",
    value: "word",
    aliases: ["diff words", "word diff"],
  },
  {
    id: "diffs.lineDiff.char",
    title: "Diff View: Use Character Line Diff",
    value: "char",
    aliases: ["diff characters", "character diff"],
  },
  {
    id: "diffs.lineDiff.none",
    title: "Diff View: Disable Line Diff",
    value: "none",
    aliases: ["diff no line diff", "hide inline diff"],
  },
];

const DIFF_VIEW_HUNK_SEPARATOR_COMMANDS: readonly DiffViewCommandOption<DiffViewHunkSeparators>[] = [
  {
    id: "diffs.hunkSeparators.line",
    title: "Diff View: Use Line Hunk Separators",
    value: "line-info",
    aliases: ["diff hunk line", "line separators"],
  },
  {
    id: "diffs.hunkSeparators.basic",
    title: "Diff View: Use Basic Hunk Separators",
    value: "line-info-basic",
    aliases: ["diff hunk basic", "basic separators"],
  },
  {
    id: "diffs.hunkSeparators.metadata",
    title: "Diff View: Use Metadata Hunk Separators",
    value: "metadata",
    aliases: ["diff hunk metadata", "metadata separators"],
  },
  {
    id: "diffs.hunkSeparators.simple",
    title: "Diff View: Use Simple Hunk Separators",
    value: "simple",
    aliases: ["diff hunk simple", "simple separators"],
  },
  {
    id: "diffs.hunkSeparators.custom",
    title: "Diff View: Use Custom Hunk Separators",
    value: "custom-css",
    aliases: ["diff hunk custom", "custom separators"],
  },
];

export const diffViewPackage = definePackage("the-editor/diff-view", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;
    const preferences = yield* DiffViewPreferences;
    const renderers = yield* PanelRenderers;
    const host = yield* HostDiffView;

    yield* renderers.register(DiffViewSurface, host.renderer);

    yield* commands.register("diffs.openDiffView", {
      title: "Open Diff View",
      scope: SurfaceScope.global,
      palette: { group: "View", aliases: ["diff", "changes", "source control"] },
      run: () => Effect.sync(() => host.open()),
    });

    for (const command of DIFF_VIEW_LAYOUT_COMMANDS) {
      yield* commands.register(
        command.id,
        diffViewSettingCommand(
          preferences,
          command.title,
          { layout: command.value },
          command.aliases,
        ),
      );
    }
    for (const command of DIFF_VIEW_INDICATOR_COMMANDS) {
      yield* commands.register(
        command.id,
        diffViewSettingCommand(
          preferences,
          command.title,
          { indicators: command.value },
          command.aliases,
        ),
      );
    }
    for (const command of DIFF_VIEW_LINE_DIFF_COMMANDS) {
      yield* commands.register(
        command.id,
        diffViewSettingCommand(
          preferences,
          command.title,
          { lineDiff: command.value },
          command.aliases,
        ),
      );
    }
    for (const command of DIFF_VIEW_HUNK_SEPARATOR_COMMANDS) {
      yield* commands.register(
        command.id,
        diffViewSettingCommand(
          preferences,
          command.title,
          { hunkSeparators: command.value },
          command.aliases,
        ),
      );
    }
    yield* commands.register(
      "diffs.toggleBackgrounds",
      diffViewToggleCommand(preferences, "Diff View: Toggle Backgrounds", "backgrounds", [
        "diff backgrounds",
        "diff colors",
      ]),
    );
    yield* commands.register(
      "diffs.toggleWrapping",
      diffViewToggleCommand(preferences, "Diff View: Toggle Wrapping", "wrapping", [
        "diff wrap",
        "wrap diff lines",
      ]),
    );
    yield* commands.register(
      "diffs.toggleLineNumbers",
      diffViewToggleCommand(preferences, "Diff View: Toggle Line Numbers", "lineNumbers", [
        "diff line numbers",
        "line numbers",
      ]),
    );
    yield* commands.register(
      "diffs.toggleControls",
      diffViewToggleCommand(preferences, "Diff View: Toggle Controls", "controlsVisible", [
        "diff controls",
        "diff toolbar",
        "button bar",
      ]),
    );
    yield* keymaps.registerAll([
      { keys: "meta+shift+g", command: "diffs.openDiffView", scope: SurfaceScope.global },
      { keys: "ctrl+shift+g", command: "diffs.openDiffView", scope: SurfaceScope.global },
    ]);
  }),
);
