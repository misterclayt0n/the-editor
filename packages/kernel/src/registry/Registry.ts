import { Effect, Ref, type Scope } from "effect";
import { RegistryDuplicateContributionError } from "../errors.js";
import type { RegistryId } from "../ids.js";
import type { ContributionIndex } from "../contribution/ContributionIndex.js";
import type { RegistryError } from "../errors.js";
import type { ContributionOwner, IdentifiedContribution, OwnedContribution } from "../ownership.js";

export interface RegistryOptions {
  readonly contributionIndex?: ContributionIndex;
}

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
  options: RegistryOptions = {},
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
          const alreadyRegistered = yield* Ref.get(entries).pipe(Effect.map((map) => map.has(key)));
          if (alreadyRegistered) {
            return yield* new RegistryDuplicateContributionError({
              registryId: id,
              contributionId: contribution.id,
            });
          }
          if (options.contributionIndex) {
            yield* options.contributionIndex.add({
              owner,
              registryId: id,
              contributionId: contribution.id,
            });
          }
          yield* Ref.update(entries, (map) => {
            const next = new Map(map);
            next.set(key, owned);
            return next;
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
