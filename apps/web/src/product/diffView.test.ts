import { describe, expect, test } from "vitest";
import { Context, Effect, Layer } from "effect";
import { Commands, CommandId, makeCommands } from "@the-editor/commands";
import { toPackageDescriptor } from "@the-editor/extensions";
import { Keymaps, makeKeymaps } from "@the-editor/keymaps";
import { makePackageRuntime } from "@the-editor/kernel";
import { SurfaceScope } from "@the-editor/workbench";
import { makePanelRenderers, PanelRenderers } from "../workbench/panelRenderers";
import {
  DEFAULT_DIFF_VIEW_SETTINGS,
  DiffViewPreferences,
  DiffViewSurface,
  HostDiffView,
  diffViewPackage,
  makeDiffViewPreferences,
} from "./diffView";

const activateDiffViewPackage = Effect.gen(function* () {
  const runtime = yield* makePackageRuntime;
  const commands = yield* makeCommands(runtime.contributionIndex);
  const keymaps = yield* makeKeymaps(runtime.contributionIndex);
  const preferences = makeDiffViewPreferences();
  const panelRenderers = makePanelRenderers();
  const layer = Layer.mergeAll(
    Layer.succeed(Commands, commands),
    Layer.succeed(Keymaps, keymaps),
    Layer.succeed(DiffViewPreferences, preferences),
    Layer.succeed(PanelRenderers, panelRenderers),
    Layer.succeed(HostDiffView, {
      renderer: {
        title: () => "Diff",
        mount: () => ({ dispose: () => undefined }),
      },
      open: () => undefined,
    }),
  );

  yield* runtime.activateGraph([toPackageDescriptor(diffViewPackage, layer)]);
  return { commands, preferences };
});

describe("DiffViewPreferences", () => {
  test("defaults hide the controls bar", () => {
    expect(DEFAULT_DIFF_VIEW_SETTINGS.controlsVisible).toBe(false);
    expect(makeDiffViewPreferences().current().controlsVisible).toBe(false);
  });
});

describe("diffViewPackage commands", () => {
  test("registers scoped settings commands with View/Diff palette metadata", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const { commands } = yield* activateDiffViewPackage;
        const settingsCommandIds = [
          "diffs.layout.split",
          "diffs.layout.stacked",
          "diffs.indicators.bars",
          "diffs.indicators.classic",
          "diffs.indicators.none",
          "diffs.lineDiff.wordAlt",
          "diffs.lineDiff.word",
          "diffs.lineDiff.char",
          "diffs.lineDiff.none",
          "diffs.hunkSeparators.line",
          "diffs.hunkSeparators.basic",
          "diffs.hunkSeparators.metadata",
          "diffs.hunkSeparators.simple",
          "diffs.hunkSeparators.custom",
          "diffs.toggleBackgrounds",
          "diffs.toggleWrapping",
          "diffs.toggleLineNumbers",
          "diffs.toggleControls",
        ];

        expect((yield* commands.get("diffs.openDiffView"))?.contribution.scope).toEqual(
          SurfaceScope.global,
        );
        for (const id of settingsCommandIds) {
          const command = (yield* commands.get(id))?.contribution;
          expect(command, id).toBeDefined();
          expect(command?.title, id).toMatch(/^Diff View: /);
          expect(command?.scope, id).toEqual(SurfaceScope.surface(DiffViewSurface));
          expect(command?.palette, id).toMatchObject({ group: "View/Diff" });
          expect(
            (command?.palette as { aliases?: readonly string[] } | undefined)?.aliases?.length,
            id,
          ).toBeGreaterThan(0);
        }
      }),
    );
  });

  test("settings commands update diff preferences", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const { commands, preferences } = yield* activateDiffViewPackage;
        const run = (id: string) =>
          Effect.gen(function* () {
            const command = yield* commands.get(id);
            if (command === undefined) throw new Error(`Missing command ${id}`);
            yield* command.contribution
              .run({ id: CommandId(id), args: undefined })
              .pipe(Effect.provide(Context.empty() as Context.Context<any>));
          });

        yield* run("diffs.layout.stacked");
        yield* run("diffs.indicators.classic");
        yield* run("diffs.lineDiff.char");
        yield* run("diffs.hunkSeparators.metadata");
        yield* run("diffs.toggleControls");
        yield* run("diffs.toggleLineNumbers");

        expect(preferences.current()).toMatchObject({
          layout: "stacked",
          indicators: "classic",
          lineDiff: "char",
          hunkSeparators: "metadata",
          controlsVisible: true,
          lineNumbers: false,
        });

        yield* run("diffs.toggleControls");
        expect(preferences.current().controlsVisible).toBe(false);
      }),
    );
  });
});
