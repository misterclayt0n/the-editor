import { PackageActivationContext } from "@the-editor/extensions";
import { Context, Effect, Layer, Option, Stream, type Scope } from "effect";
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
import type { KeyBindingContribution, KeyBindingDefinition, KeyBindingFacts } from "./types.js";

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
    readonly handleKey: (
      chord: Chord,
      options?: { readonly facts?: KeyBindingFacts },
    ) => Effect.Effect<KeyResolution>;
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
    type PendingState = {
      readonly chords: ReadonlyArray<Chord>;
      readonly focusKey: string;
      readonly factsKey: string;
    };
    const EMPTY_PENDING: PendingState = { chords: [], focusKey: "", factsKey: "" };
    const pendingCell = yield* makeStateCell<PendingState>(EMPTY_PENDING);

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
          ...(definition.when !== undefined ? { when: definition.when } : {}),
          ...(definition.description !== undefined ? { description: definition.description } : {}),
        });
      });

    return {
      register,
      registerAll: (definitions) => Effect.forEach(definitions, register, { concurrency: 1 }),
      bindings: registry.list,
      handleKey: (chord, handleOptions) =>
        Effect.gen(function* () {
          const focus = yield* focusTarget;
          const owned = yield* registry.list;
          const facts = handleOptions?.facts ?? {};
          const focusKey = focus === undefined ? "" : `${focus.kind}:${focus.id}`;
          const factsKey = JSON.stringify(
            Object.entries(facts)
              .filter(([, value]) => value !== undefined)
              .sort(([a], [b]) => a.localeCompare(b)),
          );
          const pendingState = yield* pendingCell.get;
          const pending =
            pendingState.focusKey === focusKey && pendingState.factsKey === factsKey
              ? pendingState.chords
              : [];
          const resolution = resolveKey({
            bindings: owned.map((entry) => entry.contribution),
            focus,
            pending,
            chord,
            facts,
          });
          yield* pendingCell.set(
            resolution._tag === "pending"
              ? { chords: resolution.chords, focusKey, factsKey }
              : EMPTY_PENDING,
          );
          return resolution;
        }),
      pending: pendingCell.get.pipe(Effect.map((state) => state.chords)),
      pendingChanges: pendingCell.changes.pipe(Stream.map((state) => state.chords)),
      resetPending: pendingCell.set(EMPTY_PENDING),
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
