import { Effect, Ref, type Scope } from "effect";
import { RegistryDuplicateContributionError } from "../errors.js";
import type { RegistryId } from "../ids.js";
import type { ContributionOwner, IdentifiedContribution, OwnedContribution } from "../ownership.js";
import type { RegistryError } from "../errors.js";

export interface Registry<A extends IdentifiedContribution> {
  readonly id: RegistryId;
  readonly register: (
    owner: ContributionOwner,
    contribution: A,
  ) => Effect.Effect<OwnedContribution<A>, RegistryError, Scope.Scope>;
  readonly list: Effect.Effect<readonly OwnedContribution<A>[]>;
  readonly get: (id: A["id"]) => Effect.Effect<OwnedContribution<A> | undefined>;
}

export const makeRegistry = <A extends IdentifiedContribution>(
  id: RegistryId,
): Effect.Effect<Registry<A>> =>
  Effect.gen(function* () {
    const entries = yield* Ref.make(new Map<string, OwnedContribution<A>>());
    return {
      id,
      register: (owner, contribution) =>
        Effect.gen(function* () {
          const owned: OwnedContribution<A> = {
            owner,
            registryId: id,
            contributionId: contribution.id,
            contribution,
          };
          const key = contribution.id;
          const ok = yield* Ref.modify(entries, (map) => {
            if (map.has(key)) return [false, map] as const;
            const next = new Map(map);
            next.set(key, owned);
            return [true, next] as const;
          });
          if (!ok)
            return yield* new RegistryDuplicateContributionError({
              registryId: id,
              contributionId: contribution.id,
            });
          yield* Effect.addFinalizer(() =>
            Ref.update(entries, (map) => {
              const current = map.get(key);
              if (current !== owned) return map;
              const next = new Map(map);
              next.delete(key);
              return next;
            }),
          );
          return owned;
        }),
      list: Ref.get(entries).pipe(Effect.map((map) => Array.from(map.values()))),
      get: (contributionId) => Ref.get(entries).pipe(Effect.map((map) => map.get(contributionId))),
    } satisfies Registry<A>;
  });
