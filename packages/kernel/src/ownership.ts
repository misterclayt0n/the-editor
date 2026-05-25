import type { ActivationId, ContributionId, PackageId, RegistryId } from "./ids.js";

export interface ContributionOwner {
  readonly packageId: PackageId;
  readonly activationId: ActivationId;
}

export interface IdentifiedContribution {
  readonly id: ContributionId;
}

export interface OwnedContribution<A> {
  readonly owner: ContributionOwner;
  readonly registryId: RegistryId;
  readonly contributionId: ContributionId;
  readonly contribution: A;
}
