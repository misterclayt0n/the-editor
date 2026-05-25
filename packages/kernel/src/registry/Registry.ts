import type { Effect, Scope } from "effect";
import type { RegistryError } from "../errors.js";
import type { RegistryId } from "../ids.js";
import type { ContributionOwner, IdentifiedContribution, OwnedContribution } from "../ownership.js";

export type RegistryDuplicatePolicy = "reject";

export interface RegistryOptions {
  readonly duplicatePolicy?: RegistryDuplicatePolicy;
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
