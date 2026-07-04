import { Context, Effect, Exit, Ref, Scope } from "effect";
import { PackageActivationContext } from "@the-editor/extensions";
import {
  RegistryId,
  makeRegistry,
  type ContributionIndex,
  type OwnedContribution,
} from "@the-editor/kernel";
import { ViewInput, defaultInputContext, makeViewInput } from "./viewInput.js";
import {
  DEFAULT_INPUT_PROFILE,
  InputProfileId,
  type InputProfileContribution,
  type InputProfileDefinition,
  type InputViewTarget,
  type ViewInputService,
} from "./types.js";

export interface InputViewLike {
  readonly id: string;
  readonly context: Context.Context<any>;
  readonly scope: Scope.Scope;
}

type ProfileRecord = {
  readonly owned: OwnedContribution<InputProfileContribution>;
  readonly profile: InputProfileContribution;
  readonly scope: Scope.Scope;
};

type ViewRecord = {
  readonly id: string;
  readonly baseContext: Context.Context<any>;
  readonly scope: Scope.Scope;
  readonly input: ViewInputService;
  attachment: Scope.CloseableScope | null;
  appliedProfile: string;
};

export declare namespace InputProfiles {
  export interface Service {
    readonly register: (
      definition: InputProfileDefinition,
    ) => Effect.Effect<OwnedContribution<InputProfileContribution>, unknown, Scope.Scope | PackageActivationContext>;
    readonly list: Effect.Effect<readonly OwnedContribution<InputProfileContribution>[]>;
    /** The requested active profile id. If it is not currently registered,
     *  views run on the built-in default until it appears. */
    readonly activeProfile: Effect.Effect<string>;
    readonly setActiveProfile: (id: string) => Effect.Effect<void>;
    /** Host side: enrich a view context with ViewInput and track it for
     *  profile switching/retraction. */
    readonly attachView: <V extends InputViewLike>(view: V) => Effect.Effect<V>;
  }
}

export class InputProfiles extends Context.Tag("@the-editor/input/InputProfiles")<
  InputProfiles,
  InputProfiles.Service
>() {}

const builtInDefaultProfile: InputProfileContribution = {
  id: DEFAULT_INPUT_PROFILE,
  title: "Default",
  initialContext: defaultInputContext,
};

export const makeInputProfiles = (
  contributionIndex: ContributionIndex,
): Effect.Effect<InputProfiles.Service> =>
  Effect.gen(function* () {
    const registry = yield* makeRegistry<InputProfileContribution>(
      RegistryId("the-editor.input-profiles"),
      { contributionIndex },
    );
    const records = yield* Ref.make(new Map<string, ProfileRecord>());
    const views = yield* Ref.make<readonly ViewRecord[]>([]);
    const activeId = yield* Ref.make<string>(String(DEFAULT_INPUT_PROFILE));

    const profileRecord = (id: string): Effect.Effect<ProfileRecord | null> =>
      Ref.get(records).pipe(Effect.map((map) => map.get(id) ?? null));

    const profileFor = (requested: string): Effect.Effect<{
      readonly profile: InputProfileContribution;
      readonly scope: Scope.Scope | null;
      readonly appliedId: string;
    }> =>
      Effect.gen(function* () {
        if (requested === String(DEFAULT_INPUT_PROFILE)) {
          return { profile: builtInDefaultProfile, scope: null, appliedId: String(DEFAULT_INPUT_PROFILE) };
        }
        const record = yield* profileRecord(requested);
        return record === null
          ? { profile: builtInDefaultProfile, scope: null, appliedId: String(DEFAULT_INPUT_PROFILE) }
          : { profile: record.profile, scope: record.scope, appliedId: requested };
      });

    const applyProfileToView = (view: ViewRecord, requested: string): Effect.Effect<void> =>
      Effect.gen(function* () {
        const resolved = yield* profileFor(requested);
        if (view.attachment !== null) {
          yield* Scope.close(view.attachment, Exit.void);
          view.attachment = null;
        }
        view.appliedProfile = resolved.appliedId;
        yield* view.input.clearContinuations();
        yield* view.input.set(resolved.profile.initialContext());
        if (resolved.profile.attachView === undefined) return;
        const parent = resolved.scope ?? view.scope;
        const attachment = yield* Scope.fork(parent);
        view.attachment = attachment;
        if (resolved.scope !== null) {
          yield* Scope.addFinalizer(view.scope, Scope.close(attachment, Exit.void));
        }
        const target: InputViewTarget = {
          viewId: view.id,
          context: Context.add(view.baseContext, ViewInput, view.input),
          input: view.input,
        };
        yield* Scope.extend(resolved.profile.attachView(target), attachment);
      });

    const applyActiveToAll = Effect.gen(function* () {
      const requested = yield* Ref.get(activeId);
      yield* Effect.forEach(yield* Ref.get(views), (view) => applyProfileToView(view, requested), {
        discard: true,
      });
    });

    const resetRetractedProfile = (id: string): Effect.Effect<void> =>
      Effect.gen(function* () {
        yield* Ref.update(records, (map) => {
          const next = new Map(map);
          next.delete(id);
          return next;
        });
        const active = yield* Ref.get(activeId);
        if (active === id) yield* Ref.set(activeId, String(DEFAULT_INPUT_PROFILE));
        yield* Effect.forEach(
          yield* Ref.get(views),
          (view) =>
            view.appliedProfile === id || active === id
              ? applyProfileToView(view, String(DEFAULT_INPUT_PROFILE))
              : Effect.void,
          { discard: true },
        );
      });

    const service: InputProfiles.Service = {
      register: (definition) =>
        Effect.gen(function* () {
          const extension = yield* PackageActivationContext;
          const profile: InputProfileContribution = {
            ...definition,
            id: InputProfileId(definition.id),
          };
          const owned = yield* extension.contributions.register(registry, profile);
          const scope = yield* Effect.scope;
          const record: ProfileRecord = { owned, profile, scope };
          yield* Ref.update(records, (map) => new Map(map).set(definition.id, record));
          yield* Effect.addFinalizer(() => resetRetractedProfile(definition.id));
          if ((yield* Ref.get(activeId)) === definition.id) yield* applyActiveToAll;
          return owned;
        }),
      list: registry.list,
      activeProfile: Ref.get(activeId),
      setActiveProfile: (id) =>
        Effect.gen(function* () {
          yield* Ref.set(activeId, id);
          yield* applyActiveToAll;
        }),
      attachView: <V extends InputViewLike>(view: V): Effect.Effect<V> =>
        Effect.gen(function* () {
          const input = yield* makeViewInput(defaultInputContext());
          const record: ViewRecord = {
            id: view.id,
            baseContext: view.context,
            scope: view.scope,
            input,
            attachment: null,
            appliedProfile: String(DEFAULT_INPUT_PROFILE),
          };
          yield* Ref.update(views, (current) => [...current, record]);
          yield* Scope.addFinalizer(
            view.scope,
            Ref.update(views, (current) => current.filter((entry) => entry !== record)).pipe(
              Effect.zipRight(
                record.attachment !== null ? Scope.close(record.attachment, Exit.void) : Effect.void,
              ),
            ),
          );
          yield* applyProfileToView(record, yield* Ref.get(activeId));
          return {
            ...view,
            context: Context.add(view.context, ViewInput, input),
          };
        }),
    };

    return service;
  });
