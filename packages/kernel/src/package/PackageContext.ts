import type { ActivationId, PackageId } from "../ids.js";
import type { ContributionOwner } from "../ownership.js";
import type { RegistryRuntime } from "../registry/RegistryRuntime.js";
import type { StoreRuntime } from "../store/StoreRuntime.js";

export interface PackageContext {
  readonly packageId: PackageId;
  readonly activationId: ActivationId;
  readonly owner: ContributionOwner;
  readonly registries: RegistryRuntime;
  readonly stores: StoreRuntime;
}
