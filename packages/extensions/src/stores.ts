import {
  StoreId,
  type Store,
  type StoreDefinition,
  type StoreError,
  type StoreRuntime,
} from "@the-editor/kernel";
import type { Effect, Scope } from "effect";
import type { ExtensionContextIdentity } from "./ExtensionContext.js";

export interface ExtensionStoreDefinition<A> {
  readonly id: string;
  readonly version: number;
  readonly initial: Effect.Effect<A>;
  readonly serialize?: (value: A) => unknown;
  readonly deserialize?: (raw: unknown) => Effect.Effect<A, StoreError>;
  readonly migrate?: (raw: unknown, fromVersion: number) => Effect.Effect<unknown, StoreError>;
}

export interface ExtensionStoreApi {
  readonly activation: <A>(
    definition: ExtensionStoreDefinition<A>,
  ) => Effect.Effect<Store<A>, never, Scope.Scope>;
  readonly package: <A>(
    definition: ExtensionStoreDefinition<A>,
  ) => Effect.Effect<Store<A>, never, Scope.Scope>;
  readonly application: <A>(
    definition: ExtensionStoreDefinition<A>,
  ) => Effect.Effect<Store<A>, never, Scope.Scope>;
}

const toKernelDefinition = <A>(
  definition: ExtensionStoreDefinition<A>,
  lifetime: StoreDefinition<A>["lifetime"],
): StoreDefinition<A> => ({
  ...definition,
  id: StoreId(definition.id),
  lifetime,
});

export const makeExtensionStoreApi = (
  identity: ExtensionContextIdentity,
  stores: StoreRuntime,
): ExtensionStoreApi => ({
  activation: (definition) =>
    stores.getOrCreate(toKernelDefinition(definition, "activation"), identity),
  package: (definition) => stores.getOrCreate(toKernelDefinition(definition, "package"), identity),
  application: (definition) =>
    stores.getOrCreate(toKernelDefinition(definition, "application"), identity),
});
