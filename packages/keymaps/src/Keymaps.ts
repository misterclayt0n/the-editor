import { PackageActivationContext } from "@the-editor/extensions";
import { Context, Effect, Layer, Option, type Scope, type Stream } from "effect";
import {
  ContributionId,
  makeRegistry,
  makeStateCell,
  PackageRuntimeService,
  RegistryId,
  type ContributionIndex,
  type OwnedContribution,
} from "@the-editor/kernel";
import { SurfaceScope, Workbench } from "@the-editor/workbench";
import { parseKeys, type Chord } from "./chord.js";
import { InvalidKeysError, type KeymapsError } from "./errors.js";
import { resolveKey, type FocusTarget, type KeyResolution } from "./resolver.js";
import type { KeyBindingContribution, KeyBindingDefinition } from "./types.js";

let nextBindingId = 1;

export declare namespace Keymaps {
  export interface Service {
    readonly register: (
      definition: KeyBindingDefinition,
    ) => Effect.Effect<
      OwnedContribution<KeyBindingContribution>,
      KeymapsError,
      Scope.Scope | PackageActivationContext
    >;
    readonly registerAll: (
      definitions: ReadonlyArray<KeyBindingDefinition>,
    ) => Effect.Effect<
      ReadonlyArray<OwnedContribution<KeyBindingContribution>>,
      KeymapsError,
      Scope.Scope | PackageActivationContext
    >;
    readonly bindings: Effect.Effect<readonly OwnedContribution<KeyBindingContribution>[]>;

    /**
     * Feed one chord through the keymap state machine. Stateful: a "pending"
     * resolution arms the pending prefix (chord leaders like ctrl+w); the
     * next chord resolves against it. Match and none both reset it. Never
     * suspends — safe under runSync on the input hot path.
     *
     * Resolution only: executing the matched command is the host's move.
     */
    readonly handleKey: (chord: Chord) => Effect.Effect<KeyResolution>;
    readonly pending: Effect.Effect<ReadonlyArray<Chord>>;
    readonly pendingChanges: Stream.Stream<ReadonlyArray<Chord>>;
    readonly resetPending: Effect.Effect<void>;
  }
}

export class Keymaps extends Context.Tag("@the-editor/keymaps/Keymaps")<
  Keymaps,
  Keymaps.Service
>() {}

export type MakeKeymapsOptions = {
  /** Source of focus for scope filtering. Without it only global bindings resolve. */
  readonly workbench?: Workbench.Service;
};

export const makeKeymaps = (
  contributionIndex: ContributionIndex,
  options?: MakeKeymapsOptions,
): Effect.Effect<Keymaps.Service> =>
  Effect.gen(function* () {
    const registry = yield* makeRegistry<KeyBindingContribution>(
      RegistryId("the-editor.keybindings"),
      { contributionIndex },
    );
    const workbench = options?.workbench;
    const pendingCell = yield* makeStateCell<ReadonlyArray<Chord>>([]);

    const focusTarget: Effect.Effect<FocusTarget> = workbench
      ? workbench.focusedPanel.pipe(
          Effect.map(
            Option.match({
              onNone: () => undefined,
              onSome: (panel): FocusTarget => ({ id: panel.id, kind: panel.kind }),
            }),
          ),
        )
      : Effect.succeed(undefined);

    const register: Keymaps.Service["register"] = (definition) =>
      Effect.gen(function* () {
        const chords = yield* Effect.try({
          try: () => parseKeys(definition.keys),
          catch: (error) =>
            error instanceof InvalidKeysError
              ? error
              : new InvalidKeysError({ keys: definition.keys, reason: String(error) }),
        });
        const extension = yield* PackageActivationContext;
        return yield* extension.contributions.register(registry, {
          id: ContributionId(`keybinding:${nextBindingId++}`),
          chords,
          command: definition.command,
          scope: definition.scope ?? SurfaceScope.global,
          ...(definition.args !== undefined ? { args: definition.args } : {}),
          ...(definition.description !== undefined ? { description: definition.description } : {}),
        });
      });

    return {
      register,
      registerAll: (definitions) => Effect.forEach(definitions, register, { concurrency: 1 }),
      bindings: registry.list,
      handleKey: (chord) =>
        Effect.gen(function* () {
          const focus = yield* focusTarget;
          const owned = yield* registry.list;
          const pending = yield* pendingCell.get;
          const resolution = resolveKey({
            bindings: owned.map((entry) => entry.contribution),
            focus,
            pending,
            chord,
          });
          yield* pendingCell.set(resolution._tag === "pending" ? resolution.chords : []);
          return resolution;
        }),
      pending: pendingCell.get,
      pendingChanges: pendingCell.changes,
      resetPending: pendingCell.set([]),
    } satisfies Keymaps.Service;
  });

export const KeymapsLiveFromRuntime = Layer.effect(
  Keymaps,
  Effect.gen(function* () {
    const runtime = yield* PackageRuntimeService;
    return yield* makeKeymaps(runtime.contributionIndex);
  }),
);

export const KeymapsLive = KeymapsLiveFromRuntime;
