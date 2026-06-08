import { Context, Effect, Layer, Ref } from "effect";
import { TextBuffer } from "./buffer";
import { InvalidDocumentVersion, type TextError, Transaction } from "./transaction";

export type TextDocumentVersion = number;

export type TextDocumentState = {
  readonly buffer: TextBuffer;
  readonly version: TextDocumentVersion;
  readonly dirty: boolean;
};

export type TextDocumentSnapshot = TextDocumentState;

export type TextDocumentInput =
  | string
  | TextBuffer
  | {
      readonly buffer?: TextBuffer;
      readonly text?: string;
      readonly dirty?: boolean;
      readonly version?: number;
    };

export class TextDocument extends Context.Tag("@the-editor/engine/TextDocument")<
  TextDocument,
  TextDocument.Service
>() {}

export namespace TextDocument {
  export interface Service {
    readonly get: Effect.Effect<TextBuffer>;
    readonly snapshot: Effect.Effect<TextDocumentSnapshot>;
    readonly version: Effect.Effect<TextDocumentVersion>;
    readonly dirty: Effect.Effect<boolean>;
    readonly transact: (transaction: Transaction) => Effect.Effect<TextBuffer, TextError>;
    readonly applyTransaction: (transaction: Transaction) => Effect.Effect<TextBuffer, TextError>;
    readonly update: (
      f: (buffer: TextBuffer) => Effect.Effect<Transaction, TextError>,
    ) => Effect.Effect<TextBuffer, TextError>;
    readonly set: (
      buffer: TextBuffer,
      options?: { readonly dirty?: boolean },
    ) => Effect.Effect<TextBuffer>;
    readonly markSaved: Effect.Effect<void>;
  }
}

const isTextBuffer = (input: unknown): input is TextBuffer => input instanceof TextBuffer;

const initialState = (input?: TextDocumentInput): TextDocumentState => {
  if (input === undefined) return { buffer: TextBuffer.empty, version: 0, dirty: false };
  if (typeof input === "string")
    return { buffer: TextBuffer.fromString(input), version: 0, dirty: false };
  if (isTextBuffer(input)) return { buffer: input, version: 0, dirty: false };

  const buffer =
    input.buffer ??
    (input.text !== undefined ? TextBuffer.fromString(input.text) : TextBuffer.empty);
  return { buffer, version: input.version ?? 0, dirty: input.dirty ?? false };
};

const validateVersion = (version: number): Effect.Effect<void, TextError> =>
  Number.isInteger(version) && version >= 0
    ? Effect.void
    : Effect.fail(new InvalidDocumentVersion({ version }));

export const makeTextDocument = (
  input?: TextDocumentInput,
): Effect.Effect<TextDocument.Service, TextError> =>
  Effect.gen(function* () {
    const initial = initialState(input);
    yield* validateVersion(initial.version);
    const state = yield* Ref.make<TextDocumentState>(initial);

    const snapshot = Ref.get(state);
    const get = snapshot.pipe(Effect.map((s) => s.buffer));
    const version = snapshot.pipe(Effect.map((s) => s.version));
    const dirty = snapshot.pipe(Effect.map((s) => s.dirty));

    const transact = (transaction: Transaction): Effect.Effect<TextBuffer, TextError> =>
      Effect.gen(function* () {
        const current = yield* Ref.get(state);
        const nextBuffer = yield* transaction.applyTo(current.buffer);
        yield* Ref.set(state, { buffer: nextBuffer, version: current.version + 1, dirty: true });
        return nextBuffer;
      });

    const set = (
      buffer: TextBuffer,
      options?: { readonly dirty?: boolean },
    ): Effect.Effect<TextBuffer> =>
      Ref.modify(
        state,
        (current) =>
          [
            buffer,
            { buffer, version: current.version + 1, dirty: options?.dirty ?? true },
          ] as const,
      );

    return {
      get,
      snapshot,
      version,
      dirty,
      transact,
      applyTransaction: transact,
      update: (f) => Effect.flatMap(get, (buffer) => Effect.flatMap(f(buffer), transact)),
      set,
      markSaved: Ref.update(state, (current) => ({ ...current, dirty: false })),
    } satisfies TextDocument.Service;
  });

export const TextDocumentLive = (input?: TextDocumentInput): Layer.Layer<TextDocument, TextError> =>
  Layer.effect(TextDocument, makeTextDocument(input));
