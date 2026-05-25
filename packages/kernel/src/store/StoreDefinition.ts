import type { Effect } from "effect";
import type { StoreError } from "../errors.js";
import type { StoreId } from "../ids.js";

export type StoreLifetime = "activation" | "package" | "application";

export interface StoreDefinition<A> {
  readonly id: StoreId;
  readonly version: number;
  readonly lifetime: StoreLifetime;
  readonly initial: Effect.Effect<A>;
  readonly serialize?: (value: A) => unknown;
  readonly deserialize?: (raw: unknown) => Effect.Effect<A, StoreError>;
  readonly migrate?: (raw: unknown, fromVersion: number) => Effect.Effect<unknown, StoreError>;
}

export interface Store<A> {
  readonly definition: StoreDefinition<A>;
  readonly get: Effect.Effect<A>;
  readonly set: (value: A) => Effect.Effect<void>;
  readonly update: (f: (value: A) => A) => Effect.Effect<void>;
}
