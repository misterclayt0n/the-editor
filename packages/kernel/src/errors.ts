import { Data } from "effect";
import type { ActivationId, ContributionId, PackageId, RegistryId, StoreId } from "./ids.js";

export class RegistryDuplicateContributionError extends Data.TaggedError(
  "RegistryDuplicateContributionError",
)<{
  readonly registryId: RegistryId;
  readonly contributionId: ContributionId;
}> {}

export type RegistryError = RegistryDuplicateContributionError;

export class PackageDependencyMissingError extends Data.TaggedError(
  "PackageDependencyMissingError",
)<{
  readonly packageId: PackageId;
  readonly dependencyId: PackageId;
}> {}

export class PackageDependencyCycleError extends Data.TaggedError("PackageDependencyCycleError")<{
  readonly packageIds: readonly PackageId[];
}> {}

export class PackageActivationError extends Data.TaggedError("PackageActivationError")<{
  readonly packageId: PackageId;
  readonly activationId: ActivationId;
  readonly cause: unknown;
}> {}

export class PackageNotFoundError extends Data.TaggedError("PackageNotFoundError")<{
  readonly packageId: PackageId;
}> {}

export type PackageError =
  | PackageDependencyMissingError
  | PackageDependencyCycleError
  | PackageActivationError
  | PackageNotFoundError;

export class StoreDeserializeError extends Data.TaggedError("StoreDeserializeError")<{
  readonly storeId: StoreId;
  readonly cause: unknown;
}> {}

export type StoreError = StoreDeserializeError;
