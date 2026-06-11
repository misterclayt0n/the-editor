/**
 * Notification fabric: scoped, batched, serialized invalidation pub/sub.
 *
 * This module owns the *mechanism* only: typed topics, notification batches,
 * monotonic serials, and scoped fan-out. It deliberately has no vocabulary of
 * product concepts. Whoever owns a piece of state mints the topic for it.
 *
 * There is one bus per scope, mirroring the scope hierarchy:
 *
 * - WorkspaceNotifications: workspace truths (the document set, roots).
 * - DocumentNotifications: document truths (text, document-anchored
 *   visuals). Shared by every view of the document.
 * - ViewNotifications: view truths (selection). Private to one view.
 *
 * Rules of the fabric:
 *
 * - Notifications carry hints, not state. Consumers re-read the owning
 *   service (TextDocument, SelectionState, ...) when notified.
 * - Delivery is sliding: a slow consumer drops the oldest batches instead of
 *   backpressuring the input path. Consumers that track deltas must detect
 *   serial gaps (serial > last + 1) and fall back to a full re-read.
 *   Serials are PER BUS — never compare serials across buses.
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

export interface NotificationBus {
  /** Publish one batch. Never suspends; an empty batch is a no-op. */
  readonly publish: (invalidations: ReadonlyArray<Invalidation>) => Effect.Effect<void>;
  /**
   * Stream of notification batches. Every run of the stream is an
   * independent subscription that sees batches published after it starts
   * and ends when the owning scope closes.
   */
  readonly notifications: Stream.Stream<EditorNotification>;
  /** Serial of the most recently published batch (0 before any publish). */
  readonly serial: Effect.Effect<number>;
}

/** Document truths: text changes, document-anchored visuals. One per document. */
export class DocumentNotifications extends Context.Tag("@the-editor/editor/DocumentNotifications")<
  DocumentNotifications,
  NotificationBus
>() {}

/** View truths: selection, view-local visuals. One per view. */
export class ViewNotifications extends Context.Tag("@the-editor/editor/ViewNotifications")<
  ViewNotifications,
  NotificationBus
>() {}

/** Workspace truths: the document set, roots. One per workspace. */
export class WorkspaceNotifications extends Context.Tag(
  "@the-editor/editor/WorkspaceNotifications",
)<WorkspaceNotifications, NotificationBus>() {}

export const makeNotificationBus = (options?: {
  readonly capacity?: number;
}): Effect.Effect<NotificationBus, never, Scope.Scope> =>
  Effect.gen(function* () {
    // Shutdown is fire-and-forget: closing a scope must never suspend on
    // consumer teardown (the fabric is non-blocking end to end), so scope
    // closes stay runSync-safe. Consumer streams end momentarily after.
    const pubsub = yield* Effect.acquireRelease(
      PubSub.sliding<EditorNotification>(options?.capacity ?? DEFAULT_CAPACITY),
      (hub) => Effect.forkDaemon(PubSub.shutdown(hub)),
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
    } satisfies NotificationBus;
  });

/**
 * Core truths and their buses. Payloads are staleness hints only; consumers
 * re-read TextDocument / SelectionState.
 */

/**
 * One changed span: [fromA, toA) in the old document was replaced by
 * [fromB, toB) in the new one. Structurally compatible with the engine's
 * ChangeRange so producers can pass `transaction.changes.desc().ranges`
 * straight through.
 */
export type DocumentChangeRange = {
  readonly fromA: number;
  readonly toA: number;
  readonly fromB: number;
  readonly toB: number;
};

export type DocumentInvalidation = {
  readonly version: number;
  /**
   * Changed spans for this version bump, when the producer knows them.
   * Consumers that miss a batch (serial gap) or see no ranges must fall back
   * to treating the whole document as dirty.
   */
  readonly changes?: ReadonlyArray<DocumentChangeRange>;
};

export const DocumentInvalidated = makeInvalidationTopic<DocumentInvalidation>(
  "@the-editor/editor/document",
);

export const SelectionInvalidated = makeInvalidationTopic("@the-editor/editor/selection");
