/**
 * Editor notification fabric: scoped, batched, serialized invalidation pub/sub.
 *
 * This module owns the *mechanism* only: typed topics, notification batches,
 * monotonic serials, and scoped fan-out. It deliberately has no vocabulary of
 * product concepts. Whoever owns a piece of state mints the topic for it; this
 * package mints topics for exactly the two truths an editor scope owns
 * (document and selection). Decorations, configuration, diagnostics, and
 * friends belong to their owners (packages/services), never to this union of
 * one.
 *
 * Rules of the fabric:
 *
 * - Notifications carry hints, not state. Consumers re-read the owning
 *   service (TextDocument, SelectionState, ...) when notified.
 * - Delivery is sliding: a slow consumer drops the oldest batches instead of
 *   backpressuring the input path. Consumers that track deltas must detect
 *   serial gaps (serial > last + 1) and fall back to a full re-read.
 * - Publishing never suspends, so it is safe on the synchronous hot path.
 */

import { Context, Effect, PubSub, Ref, Stream, type Scope } from "effect";

const DEFAULT_CAPACITY = 256;

export class EditorNotificationError extends Error {}
export class InvalidInvalidationTopicIdError extends EditorNotificationError {
  constructor(readonly value: string) {
    super(`invalid invalidation topic id ${JSON.stringify(value)}`);
  }
}

export type InvalidationTopicId = string & { readonly InvalidationTopicId: unique symbol };

export const InvalidationTopicId = {
  make(value: string): InvalidationTopicId {
    if (value.length === 0) throw new InvalidInvalidationTopicIdError(value);
    return value as InvalidationTopicId;
  },
};

/**
 * A typed invalidation topic. `Payload` is a phantom parameter: it types the
 * payload at the edges (`invalidate`, `payloadsOf`) while the wire format
 * stays opaque to the fabric.
 *
 * `owner` is an opaque owner key (a kernel PackageId at the package layer) so
 * reloadable packages can retract their topics; the fabric itself does not
 * interpret it.
 */
export interface InvalidationTopic<Payload = void> {
  readonly id: InvalidationTopicId;
  readonly owner?: string;
  readonly _Payload?: (payload: Payload) => Payload;
}

export function makeInvalidationTopic<Payload = void>(
  id: string,
  options?: { readonly owner?: string },
): InvalidationTopic<Payload> {
  return options?.owner !== undefined
    ? { id: InvalidationTopicId.make(id), owner: options.owner }
    : { id: InvalidationTopicId.make(id) };
}

/** A single invalidation on the wire. Payloads are opaque to the fabric. */
export type Invalidation = {
  readonly topicId: InvalidationTopicId;
  readonly payload: unknown;
};

export function invalidate<Payload>(
  topic: InvalidationTopic<Payload>,
  ...payload: [Payload] extends [void] ? [] : [payload: Payload]
): Invalidation {
  return { topicId: topic.id, payload: payload[0] };
}

/**
 * One notification batch. Producers publish all invalidations caused by a
 * single logical operation (e.g. one transaction) as one batch so consumers
 * observe them atomically. `serial` is monotonic per editor scope.
 */
export type EditorNotification = {
  readonly serial: number;
  readonly invalidations: ReadonlyArray<Invalidation>;
};

export function hasInvalidation(
  notification: EditorNotification,
  topic: InvalidationTopic<any>,
): boolean {
  return notification.invalidations.some((invalidation) => invalidation.topicId === topic.id);
}

export function payloadsOf<Payload>(
  notification: EditorNotification,
  topic: InvalidationTopic<Payload>,
): ReadonlyArray<Payload> {
  return notification.invalidations
    .filter((invalidation) => invalidation.topicId === topic.id)
    .map((invalidation) => invalidation.payload as Payload);
}

export class EditorNotifications extends Context.Tag("@the-editor/editor/EditorNotifications")<
  EditorNotifications,
  EditorNotifications.Service
>() {}

export namespace EditorNotifications {
  export interface Service {
    /** Publish one batch. Never suspends; an empty batch is a no-op. */
    readonly publish: (invalidations: ReadonlyArray<Invalidation>) => Effect.Effect<void>;
    /**
     * Stream of notification batches. Every run of the stream is an
     * independent subscription that sees batches published after it starts
     * and ends when the owning editor scope closes.
     */
    readonly notifications: Stream.Stream<EditorNotification>;
    /** Serial of the most recently published batch (0 before any publish). */
    readonly serial: Effect.Effect<number>;
  }
}

export const makeEditorNotifications = (options?: {
  readonly capacity?: number;
}): Effect.Effect<EditorNotifications.Service, never, Scope.Scope> =>
  Effect.gen(function* () {
    const pubsub = yield* Effect.acquireRelease(
      PubSub.sliding<EditorNotification>(options?.capacity ?? DEFAULT_CAPACITY),
      PubSub.shutdown,
    );
    const serialRef = yield* Ref.make(0);

    const publish = (invalidations: ReadonlyArray<Invalidation>): Effect.Effect<void> =>
      invalidations.length === 0
        ? Effect.void
        : Ref.updateAndGet(serialRef, (serial) => serial + 1).pipe(
            Effect.flatMap((serial) => PubSub.publish(pubsub, { serial, invalidations })),
            Effect.asVoid,
          );

    return {
      publish,
      notifications: Stream.fromPubSub(pubsub),
      serial: Ref.get(serialRef),
    } satisfies EditorNotifications.Service;
  });

/**
 * The two truths an editor scope owns. Payloads are staleness hints only;
 * consumers re-read TextDocument / SelectionState.
 */
export type DocumentInvalidation = {
  readonly version: number;
};

export const DocumentInvalidated = makeInvalidationTopic<DocumentInvalidation>(
  "@the-editor/editor/document",
);

export const SelectionInvalidated = makeInvalidationTopic("@the-editor/editor/selection");
