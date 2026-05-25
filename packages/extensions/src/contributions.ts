import type {
  IdentifiedContribution,
  OwnedContribution,
  Registry,
  RegistryError,
} from "@the-editor/kernel";
import type { Effect, Scope } from "effect";

export interface ExtensionContributionApi {
  readonly register: <A extends IdentifiedContribution>(
    registry: Registry<A>,
    contribution: A,
  ) => Effect.Effect<OwnedContribution<A>, RegistryError, Scope.Scope>;
}
