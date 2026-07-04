import { Context, Effect, Ref, Stream } from "effect";
import { makeStateCell } from "@the-editor/kernel";
import {
  factsOfInputContext,
  type InputContextSnapshot,
  type InputContinuation,
  type InputContinuationResult,
  type InputInterceptor,
  type InputKeyEvent,
  type InputStatus,
  type ViewInputService,
} from "./types.js";

export class ViewInput extends Context.Tag("@the-editor/input/ViewInput")<
  ViewInput,
  ViewInputService
>() {}

export const defaultInputContext = (): InputContextSnapshot => ({
  profile: "default",
  mode: "insert",
  textFallback: "insert",
});

type InterceptorRecord = {
  readonly interceptor: InputInterceptor;
};

export const makeViewInput = (
  initial: InputContextSnapshot = defaultInputContext(),
): Effect.Effect<ViewInputService> =>
  Effect.gen(function* () {
    const contextCell = yield* makeStateCell<InputContextSnapshot>(initial);
    const continuations = yield* Ref.make<readonly InputContinuation[]>([]);
    const interceptors = yield* Ref.make<readonly InterceptorRecord[]>([]);

    const clearContinuations = (owner?: string): Effect.Effect<void> =>
      Effect.gen(function* () {
        const current = yield* Ref.get(continuations);
        const [cancelled, kept] = owner === undefined
          ? [current, [] as readonly InputContinuation[]]
          : [current.filter((entry) => entry.owner === owner), current.filter((entry) => entry.owner !== owner)];
        for (const entry of cancelled) entry.onCancel?.();
        yield* Ref.set(continuations, kept);
      });

    const service: ViewInputService = {
      context: contextCell.get,
      changes: contextCell.changes,
      facts: contextCell.get.pipe(Effect.map(factsOfInputContext)),
      set: (next) => contextCell.set(next),
      update: (f) => contextCell.update(f).pipe(Effect.asVoid),
      pushContinuation: (continuation) =>
        Ref.update(continuations, (current) => [...current, continuation]),
      clearContinuations,
      peekContinuation: Ref.get(continuations).pipe(
        Effect.map((current) => current[current.length - 1] ?? null),
      ),
      handleContinuation: (chord) =>
        Effect.gen(function* () {
          const current = yield* Ref.get(continuations);
          const top = current[current.length - 1];
          if (top === undefined) return null;
          const context = yield* contextCell.get;
          const result = top.onKey(chord, context);
          if (result._tag === "handled" || result._tag === "execute" || result._tag === "cancel") {
            yield* Ref.update(continuations, (list) => list.filter((entry) => entry !== top));
            if (result._tag === "cancel") top.onCancel?.();
          }
          return result satisfies InputContinuationResult;
        }),
      registerPreKeymap: (interceptor) =>
        Effect.acquireRelease(
          Ref.update(interceptors, (current) => [...current, { interceptor }]),
          () =>
            Ref.update(interceptors, (current) =>
              current.filter((entry) => entry.interceptor !== interceptor),
            ),
        ),
      dispatchPreKeymap: (event: InputKeyEvent) =>
        Effect.gen(function* () {
          const current = yield* Ref.get(interceptors);
          for (const { interceptor } of [...current].reverse()) {
            const result = interceptor.onKey(event);
            if (result._tag === "handled") return result;
          }
          return { _tag: "passthrough" } as const;
        }),
      status: Effect.gen(function* () {
        const context = yield* contextCell.get;
        const top = (yield* Ref.get(continuations)).at(-1);
        return {
          context,
          facts: factsOfInputContext(context),
          ...(top !== undefined ? { pendingContinuationLabel: top.label } : {}),
        } satisfies InputStatus;
      }),
    };

    // Stream import is intentionally used by the service type; this keeps the
    // generated d.ts stable under tsdown's value/type elision.
    void Stream;
    return service;
  });
