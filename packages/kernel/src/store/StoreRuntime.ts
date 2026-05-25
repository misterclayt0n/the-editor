import { Effect, Ref, type Scope } from "effect";
import type { ActivationId, PackageId } from "../ids.js";
import type { Store, StoreDefinition } from "./StoreDefinition.js";

interface Key {
  readonly packageId: PackageId;
  readonly activationId: ActivationId;
}

export interface StoreRuntime {
  readonly getOrCreate: <A>(
    definition: StoreDefinition<A>,
    key: Key,
  ) => Effect.Effect<Store<A>, never, Scope.Scope>;
}

export const makeStoreRuntime = (): Effect.Effect<StoreRuntime> =>
  Effect.gen(function* () {
    const refs = yield* Ref.make(new Map<string, Ref.Ref<unknown>>());
    const keyOf = (definition: StoreDefinition<unknown>, key: Key) => {
      switch (definition.lifetime) {
        case "activation":
          return `activation:${key.packageId}:${key.activationId}:${definition.id}`;
        case "package":
          return `package:${key.packageId}:${definition.id}`;
        case "application":
          return `application:${definition.id}`;
      }
    };
    const getOrCreate = <A>(
      definition: StoreDefinition<A>,
      key: Key,
    ): Effect.Effect<Store<A>, never, Scope.Scope> =>
      Effect.gen(function* () {
        const storageKey = keyOf(definition as StoreDefinition<unknown>, key);
        let ref = (yield* Ref.get(refs)).get(storageKey) as Ref.Ref<A> | undefined;
        if (!ref) {
          const initial = yield* definition.initial;
          ref = yield* Ref.make(initial);
          yield* Ref.update(refs, (map) => new Map(map).set(storageKey, ref as Ref.Ref<unknown>));
          if (definition.lifetime === "activation") {
            yield* Effect.addFinalizer(() =>
              Ref.update(refs, (map) => {
                const next = new Map(map);
                next.delete(storageKey);
                return next;
              }),
            );
          }
        }
        return {
          definition,
          get: Ref.get(ref),
          set: (value: A) => Ref.set(ref, value),
          update: (f: (value: A) => A) => Ref.update(ref, f),
        };
      });
    return { getOrCreate };
  });
