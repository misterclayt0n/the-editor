/**
 * Observable state cells.
 *
 * Events and state are different things: notification fabrics transport
 * discrete facts, while a StateCell holds a current value plus a stream of
 * changes, for facts that *are* values (focus, layout, configuration, ...).
 * Backed by SubscriptionRef; the ref is hidden so the implementation can
 * change without touching consumers.
 *
 * Subscribing to `changes` emits the current value immediately, then every
 * subsequent value — subscribers never need a separate initial read.
 */

import { Effect, Ref, Stream, SubscriptionRef } from "effect";

export interface StateCell<A> {
  readonly get: Effect.Effect<A>;
  readonly set: (value: A) => Effect.Effect<void>;
  readonly update: (f: (value: A) => A) => Effect.Effect<A>;
  /** Current value immediately, then every change. */
  readonly changes: Stream.Stream<A>;
}

export const makeStateCell = <A>(initial: A): Effect.Effect<StateCell<A>> =>
  Effect.map(SubscriptionRef.make(initial), (ref) => ({
    get: Ref.get(ref),
    set: (value: A) => Ref.set(ref, value),
    update: (f: (value: A) => A) => Ref.updateAndGet(ref, f),
    changes: ref.changes,
  }));
