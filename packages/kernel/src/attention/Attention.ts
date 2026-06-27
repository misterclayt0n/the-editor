/**
 * The Attention fabric: the editor's human-facing notification substrate.
 *
 * This is a third plane, distinct from the two the kernel already has:
 *
 * - StateCell transports *state* (a current value plus its changes).
 * - The editor's NotificationBus transports *invalidation* (code-facing
 *   staleness hints, per-scope, hot-path).
 * - Attention transports *facts for a person* and *questions a person must
 *   answer*. One application-level fabric, drives the UI.
 *
 * It owns the *mechanism* only, exactly like the invalidation fabric: typed
 * topics, owner keys for reloadable retraction, scoped fan-out. It has no
 * vocabulary of product concepts and no notion of "toast" vs "dialog" — a
 * consumer decides presentation. Whoever owns a concern mints its topic.
 *
 * Two verbs, because they are genuinely different shapes:
 *
 * - `notify` announces a fact and returns immediately. It NEVER suspends, so
 *   it is safe to call from a command or the hot path. Fans out to every
 *   consumer (a toast stack, a badge counter, a needs-attention projection).
 * - `ask` poses a question and SUSPENDS until exactly one responder answers.
 *   It is an interaction by definition, never the keystroke path. Backed by a
 *   Deferred; the first responder to call `respond` wins, the rest no-op.
 *
 * The careful corner: `ask` must never deadlock when nothing is listening
 * (headless, tests, no UI mounted). Responders register their presence for
 * their lifetime via `registerResponder`; with zero responders `ask` resolves
 * to the caller's `default`, or fails `AttentionUnhandledError` if none was
 * given. It never blocks forever on an empty room.
 */

import { Context, Deferred, Effect, PubSub, Ref, Stream, type Scope } from "effect";
import { makeStateCell } from "../state/StateCell.js";

const DEFAULT_NOTICE_CAPACITY = 256;

export class AttentionError extends Error {}

export class InvalidAttentionTopicIdError extends AttentionError {
  constructor(readonly value: string) {
    super(`invalid attention topic id ${JSON.stringify(value)}`);
  }
}

/** Raised by `ask` when no responder is present and the caller gave no default. */
export class AttentionUnhandledError extends AttentionError {
  constructor(readonly topicId: AttentionTopicId) {
    super(`no responder for attention request ${JSON.stringify(topicId)}`);
  }
}

export type AttentionTopicId = string & { readonly AttentionTopicId: unique symbol };

export const AttentionTopicId = {
  make(value: string): AttentionTopicId {
    if (value.length === 0) throw new InvalidAttentionTopicIdError(value);
    return value as AttentionTopicId;
  },
};

/**
 * A coarse routing hint a consumer reads to decide presentation (a green
 * toast, a warning dialog). The fabric attaches no behaviour to it.
 */
export type AttentionLevel = "info" | "success" | "warning" | "error";

/**
 * A fire-and-forget topic. `Payload` is a phantom parameter typing the edges
 * (`notify`, the consumer's read) while the wire format stays opaque to the
 * fabric — consistent with the editor's InvalidationTopic.
 */
export interface NoticeTopic<Payload = void> {
  readonly id: AttentionTopicId;
  readonly owner?: string;
  readonly _Payload?: Payload;
}

/**
 * A request/response topic. `Question` is what the asker hands the responder;
 * `Answer` is what comes back. Both are phantom — typed at the `ask`/`respond`
 * edges, opaque on the wire.
 */
export interface RequestTopic<Question = void, Answer = void> {
  readonly id: AttentionTopicId;
  readonly owner?: string;
  readonly _Question?: Question;
  readonly _Answer?: Answer;
}

export function makeNoticeTopic<Payload = void>(
  id: string,
  options?: { readonly owner?: string },
): NoticeTopic<Payload> {
  return options?.owner !== undefined
    ? { id: AttentionTopicId.make(id), owner: options.owner }
    : { id: AttentionTopicId.make(id) };
}

export function makeRequestTopic<Question = void, Answer = void>(
  id: string,
  options?: { readonly owner?: string },
): RequestTopic<Question, Answer> {
  return options?.owner !== undefined
    ? { id: AttentionTopicId.make(id), owner: options.owner }
    : { id: AttentionTopicId.make(id) };
}

/** One announced fact on the wire. Payloads are opaque to the fabric. */
export interface Notice<Payload = unknown> {
  readonly topicId: AttentionTopicId;
  readonly level: AttentionLevel;
  readonly payload: Payload;
}

export type RequestId = number & { readonly RequestId: unique symbol };

/**
 * An open question awaiting an answer. Carried in the inbox a responder reads.
 * `respond` resolves the asker's `ask` and retires the request; the first call
 * wins, later calls are no-ops. Calling it is the responder's whole job.
 */
export interface Pending<Question = unknown, Answer = unknown> {
  readonly id: RequestId;
  readonly topicId: AttentionTopicId;
  readonly level: AttentionLevel;
  readonly question: Question;
  readonly respond: (answer: Answer) => Effect.Effect<void>;
}

export type NotifyOptions = {
  readonly level?: AttentionLevel;
};

export type AskOptions<Answer> = {
  readonly level?: AttentionLevel;
  /** Resolution when no responder is present (headless, tests, no UI). */
  readonly default?: Answer;
};

export class Attention extends Context.Tag("@the-editor/kernel/Attention")<
  Attention,
  Attention.Service
>() {}

export namespace Attention {
  export interface Service {
    /**
     * Announce a fact. Never suspends — safe on the hot path. Fans out to
     * every live `notices` consumer.
     */
    readonly notify: <Payload>(
      topic: NoticeTopic<Payload>,
      payload: Payload,
      options?: NotifyOptions,
    ) => Effect.Effect<void>;

    /**
     * Pose a question; suspend until a responder answers. With no responder
     * present, resolve `options.default` or fail `AttentionUnhandledError`.
     * Interrupting the asker retires the request.
     */
    readonly ask: <Question, Answer>(
      topic: RequestTopic<Question, Answer>,
      question: Question,
      options?: AskOptions<Answer>,
    ) => Effect.Effect<Answer, AttentionUnhandledError>;

    /**
     * Stream of announced facts. Every run is an independent subscription
     * (sliding: a slow consumer drops oldest, never backpressures `notify`).
     */
    readonly notices: Stream.Stream<Notice>;

    /** The open requests right now — a responder mounting late still sees them. */
    readonly openRequests: Effect.Effect<ReadonlyArray<Pending>>;
    /** Current open requests immediately, then on every change. */
    readonly requestChanges: Stream.Stream<ReadonlyArray<Pending>>;

    /**
     * Mark a responder present for the lifetime of the acquiring scope. While
     * at least one is registered, `ask` enqueues and waits; with none it
     * resolves via the caller's policy instead of deadlocking.
     */
    readonly registerResponder: Effect.Effect<void, never, Scope.Scope>;
  }
}

export type MakeAttentionOptions = {
  readonly capacity?: number;
};

export const makeAttention = (
  options?: MakeAttentionOptions,
): Effect.Effect<Attention.Service, never, Scope.Scope> =>
  Effect.gen(function* () {
    // Fire-and-forget shutdown so closing the scope never suspends — `notify`
    // stays runSync-safe end to end, mirroring the invalidation fabric.
    const notices = yield* Effect.acquireRelease(
      PubSub.sliding<Notice>(options?.capacity ?? DEFAULT_NOTICE_CAPACITY),
      (hub) => Effect.forkDaemon(PubSub.shutdown(hub)),
    );
    const inbox = yield* makeStateCell<ReadonlyArray<Pending>>([]);
    const responderCount = yield* Ref.make(0);
    let nextId = 1;

    const retire = (id: RequestId): Effect.Effect<void> =>
      inbox.update((requests) => requests.filter((request) => request.id !== id)).pipe(Effect.asVoid);

    const notify: Attention.Service["notify"] = (topic, payload, opts) =>
      PubSub.publish(notices, {
        topicId: topic.id,
        level: opts?.level ?? "info",
        payload,
      }).pipe(Effect.asVoid);

    const ask: Attention.Service["ask"] = <Question, Answer>(
      topic: RequestTopic<Question, Answer>,
      question: Question,
      opts?: AskOptions<Answer>,
    ) =>
      Effect.gen(function* () {
        const responders = yield* Ref.get(responderCount);
        if (responders === 0) {
          if (opts && "default" in opts) return opts.default as Answer;
          return yield* Effect.fail(new AttentionUnhandledError(topic.id));
        }
        const deferred = yield* Deferred.make<Answer>();
        const id = nextId++ as RequestId;
        const pending: Pending<Question, Answer> = {
          id,
          topicId: topic.id,
          level: opts?.level ?? "info",
          question,
          respond: (answer) =>
            Deferred.succeed(deferred, answer).pipe(
              Effect.flatMap(() => retire(id)),
              Effect.asVoid,
            ),
        };
        yield* inbox.update((requests) => [...requests, pending as Pending]);
        // If the asker is interrupted (its scope torn down mid-question), the
        // request is abandoned: retire it so no responder is left holding a
        // dead card.
        return yield* Deferred.await(deferred).pipe(Effect.onInterrupt(() => retire(id)));
      });

    const registerResponder = Effect.acquireRelease(
      Ref.update(responderCount, (n) => n + 1),
      () => Ref.update(responderCount, (n) => Math.max(0, n - 1)),
    ).pipe(Effect.asVoid);

    return {
      notify,
      ask,
      notices: Stream.fromPubSub(notices),
      openRequests: inbox.get,
      requestChanges: inbox.changes,
      registerResponder,
    } satisfies Attention.Service;
  });
