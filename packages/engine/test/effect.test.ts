import { Effect, Either } from "effect";
import { describe, expect, test } from "vitest";
import {
  LengthMismatch,
  makeTextDocument,
  TextBuffer,
  TextDocument,
  TextDocumentLive,
  Transaction,
  type TextError,
} from "../src/index";

const run = <A>(effect: Effect.Effect<A, TextError>) => Effect.runSync(effect);

describe("TextDocument Effect layer", () => {
  test("constructs from supported inputs", () => {
    const empty = run(makeTextDocument());
    expect(run(empty.get).toString()).toBe("");
    expect(run(empty.snapshot)).toMatchObject({ version: 0, dirty: false });

    const fromString = run(makeTextDocument("abc"));
    expect(run(fromString.get).toString()).toBe("abc");

    const buffer = TextBuffer.fromString("buffer");
    const fromBuffer = run(makeTextDocument(buffer));
    expect(run(fromBuffer.get)).toBe(buffer);

    const fromTextObject = run(makeTextDocument({ text: "text", dirty: true, version: 3 }));
    expect(run(fromTextObject.snapshot)).toMatchObject({ version: 3, dirty: true });
    expect(run(fromTextObject.get).toString()).toBe("text");

    const fromBufferObject = run(makeTextDocument({ buffer, dirty: false, version: 2 }));
    expect(run(fromBufferObject.snapshot)).toEqual({ buffer, version: 2, dirty: false });
  });

  test("get snapshot version and dirty reflect state", () => {
    const doc = run(makeTextDocument("abc"));
    expect(run(doc.version)).toBe(0);
    expect(run(doc.dirty)).toBe(false);
    expect(run(doc.snapshot).buffer.toString()).toBe("abc");
  });

  test("transact updates buffer, version, and dirty", () => {
    const doc = run(makeTextDocument("abc"));
    const tx = run(Transaction.insertAt(3, 1, "X"));
    const next = run(doc.transact(tx));
    expect(next.toString()).toBe("aXbc");
    expect(run(doc.get).toString()).toBe("aXbc");
    expect(run(doc.version)).toBe(1);
    expect(run(doc.dirty)).toBe(true);
  });

  test("transact failure is atomic", () => {
    const doc = run(makeTextDocument({ text: "abc", dirty: false, version: 4 }));
    const tx = run(Transaction.insertAt(99, 1, "X"));
    const result = Effect.runSync(Effect.either(doc.transact(tx)));
    expect(Either.isLeft(result) && result.left instanceof LengthMismatch).toBe(true);
    expect(run(doc.get).toString()).toBe("abc");
    expect(run(doc.version)).toBe(4);
    expect(run(doc.dirty)).toBe(false);
  });

  test("update builds and applies a transaction", () => {
    const doc = run(makeTextDocument("abc"));
    const next = run(
      doc.update((buffer) => Transaction.insertAt(buffer.length, buffer.length, "!")),
    );
    expect(next.toString()).toBe("abc!");
    expect(run(doc.version)).toBe(1);
  });

  test("update callback failure is atomic", () => {
    const doc = run(makeTextDocument({ text: "abc", dirty: false, version: 7 }));
    const failure = new LengthMismatch({ expected: 1, actual: 3 });
    const result = Effect.runSync(Effect.either(doc.update(() => Effect.fail(failure))));
    expect(Either.isLeft(result)).toBe(true);
    expect(run(doc.get).toString()).toBe("abc");
    expect(run(doc.version)).toBe(7);
    expect(run(doc.dirty)).toBe(false);
  });

  test("set replaces buffer and marks dirty by default", () => {
    const doc = run(makeTextDocument({ text: "abc", dirty: false, version: 1 }));
    const next = run(doc.set(TextBuffer.fromString("xyz")));
    expect(next.toString()).toBe("xyz");
    expect(run(doc.snapshot)).toMatchObject({ version: 2, dirty: true });

    run(doc.set(TextBuffer.fromString("clean"), { dirty: false }));
    expect(run(doc.snapshot)).toMatchObject({ version: 3, dirty: false });
    expect(run(doc.get).toString()).toBe("clean");
  });

  test("markSaved clears dirty without changing version or buffer", () => {
    const doc = run(makeTextDocument({ text: "abc", dirty: true, version: 5 }));
    run(doc.markSaved);
    expect(run(doc.snapshot)).toMatchObject({ version: 5, dirty: false });
    expect(run(doc.get).toString()).toBe("abc");
  });

  test("Layer service usage", () => {
    const program = Effect.gen(function* () {
      const doc = yield* TextDocument;
      const tx = yield* Transaction.insertAt(3, 3, "!");
      yield* doc.transact(tx);
      return yield* doc.get;
    }).pipe(Effect.provide(TextDocumentLive("abc")));

    expect(Effect.runSync(program).toString()).toBe("abc!");
  });
});
