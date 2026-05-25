import { Context, Effect, Layer, PubSub, Queue, Ref, type Scope } from "effect";
import { ContributionIndexDuplicateRecordError, type ContributionIndexError } from "../errors.js";
import type { ActivationId, ContributionId, PackageId, RegistryId } from "../ids.js";
import type { ContributionOwner } from "../ownership.js";

export interface ContributionRecord {
  readonly owner: ContributionOwner;
  readonly registryId: RegistryId;
  readonly contributionId: ContributionId;
}

export type ContributionIndexChange =
  | { readonly type: "added"; readonly record: ContributionRecord }
  | { readonly type: "removed"; readonly record: ContributionRecord };

export interface ContributionIndex {
  readonly add: (
    record: ContributionRecord,
  ) => Effect.Effect<void, ContributionIndexError, Scope.Scope>;
  readonly list: Effect.Effect<readonly ContributionRecord[]>;
  readonly listByPackage: (packageId: PackageId) => Effect.Effect<readonly ContributionRecord[]>;
  readonly listByActivation: (
    activationId: ActivationId,
  ) => Effect.Effect<readonly ContributionRecord[]>;
  readonly listByRegistry: (registryId: RegistryId) => Effect.Effect<readonly ContributionRecord[]>;
  readonly changes: Effect.Effect<Queue.Dequeue<ContributionIndexChange>, never, Scope.Scope>;
}

const recordKey = (record: ContributionRecord): string =>
  `${record.registryId}:${record.contributionId}`;

export const makeContributionIndex: Effect.Effect<ContributionIndex> = Effect.gen(function* () {
  const records = yield* Ref.make(new Map<string, ContributionRecord>());
  const changes = yield* PubSub.unbounded<ContributionIndexChange>();

  const publish = (change: ContributionIndexChange) =>
    PubSub.publish(changes, change).pipe(Effect.asVoid);

  const listMatching = (predicate: (record: ContributionRecord) => boolean) =>
    Ref.get(records).pipe(Effect.map((map) => Array.from(map.values()).filter(predicate)));

  return {
    add: (record) =>
      Effect.gen(function* () {
        const key = recordKey(record);
        const added = yield* Ref.modify(records, (map) => {
          if (map.has(key)) return [false, map] as const;
          const next = new Map(map);
          next.set(key, record);
          return [true, next] as const;
        });
        if (!added) {
          return yield* new ContributionIndexDuplicateRecordError({
            registryId: record.registryId,
            contributionId: record.contributionId,
          });
        }
        yield* publish({ type: "added", record });
        yield* Effect.addFinalizer(() =>
          Ref.modify(records, (map) => {
            if (map.get(key) !== record) return [false, map] as const;
            const next = new Map(map);
            next.delete(key);
            return [true, next] as const;
          }).pipe(
            Effect.flatMap((removed) =>
              removed ? publish({ type: "removed", record }) : Effect.void,
            ),
          ),
        );
      }),
    list: Ref.get(records).pipe(Effect.map((map) => Array.from(map.values()))),
    listByPackage: (packageId) => listMatching((record) => record.owner.packageId === packageId),
    listByActivation: (activationId) =>
      listMatching((record) => record.owner.activationId === activationId),
    listByRegistry: (registryId) => listMatching((record) => record.registryId === registryId),
    changes: PubSub.subscribe(changes),
  } satisfies ContributionIndex;
});

export class ContributionIndexService extends Context.Tag("@the-editor/kernel/ContributionIndex")<
  ContributionIndexService,
  ContributionIndex
>() {}
export const ContributionIndexLive = Layer.effect(ContributionIndexService, makeContributionIndex);
