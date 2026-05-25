import type {
  ActivationId,
  ContributionOwner,
  PackageContext,
  PackageId,
} from "@the-editor/kernel";
import type { ExtensionContributionApi } from "./contributions.js";
import { makeExtensionStoreApi, type ExtensionStoreApi } from "./stores.js";

export interface ExtensionContextIdentity {
  readonly packageId: PackageId;
  readonly activationId: ActivationId;
}

export interface ExtensionContext extends ExtensionContextIdentity {
  readonly owner: ContributionOwner;
  readonly contributions: ExtensionContributionApi;
  readonly stores: ExtensionStoreApi;
}

export const makeExtensionContext = (context: PackageContext): ExtensionContext => ({
  packageId: context.packageId,
  activationId: context.activationId,
  owner: context.owner,
  contributions: {
    register: (registry, contribution) => registry.register(context.owner, contribution),
  },
  stores: makeExtensionStoreApi(context, context.stores),
});
