import { Effect, Either } from "effect";
import { describe, expect, test } from "vitest";
import {
  ChangeDesc,
  ChangeSet,
  ComposeLengthMismatch,
  LengthMismatch,
  MappedPos,
  Replacement,
  InvalidOperation,
  InvalidSpan,
  PositionsOutOfBounds,
  SpanOutOfBounds,
  Transaction,
  type Assoc,
  type ReplacementInput,
  type Span,
} from "../src/index";
import { TextBuffer } from "../src/index";

const run = <A>(e: Effect.Effect<A, unknown>) => Effect.runSync(e);
const fail = <A>(e: Effect.Effect<A, unknown>) => Either.isLeft(Effect.runSync(Effect.either(e)));
const oracle = (text: string, reps: readonly { from: number; to: number; text: string }[]) =>
  reps.reduceRight((s, r) => s.slice(0, r.from) + r.text + s.slice(r.to), text);
const applyString = (text: string, tx: Transaction): string =>
  tx.apply(TextBuffer.fromString(text)).toString();

describe("transaction layer", () => {
  test("Replacement helpers", () => {
    expect(Replacement.insert(2, "x")).toEqual({ span: { start: 2, end: 2 }, text: "x" });
    expect(Replacement.delete({ start: 1, end: 3 })).toEqual({
      span: { start: 1, end: 3 },
      text: null,
    });
    expect(Replacement.replace({ start: 1, end: 3 }, "x")).toEqual({
      span: { start: 1, end: 3 },
      text: "x",
    });
    expect(Replacement.isInsert(Replacement.insert(1, "x"))).toBe(true);
    expect(Replacement.isDelete(Replacement.delete({ start: 1, end: 2 }))).toBe(true);
    expect(Replacement.isReplace(Replacement.replace({ start: 1, end: 2 }, ""))).toBe(true);
    expect(Replacement.normalize({ from: 1, to: 3, text: "" })).toEqual({
      span: { start: 1, end: 3 },
      text: null,
    });
  });

  test("fromReplacements validation and overlap policies", () => {
    const cs = run(
      ChangeSet.fromReplacements(6, [
        { from: 1, to: 2, text: "XX" },
        { from: 4, to: 5, text: "" },
      ]),
    );
    expect(cs.replacements).toEqual([
      { from: 1, to: 2, text: "XX" },
      { from: 4, to: 5, text: "" },
    ]);
    expect(run(cs.applyTo(TextBuffer.fromString("abcdef"))).toString()).toBe(
      "aXXcd f".replace(" ", ""),
    );
    expect(fail(ChangeSet.fromReplacements(3, [Replacement.delete({ start: 2, end: 1 })]))).toBe(
      true,
    );
    expect(fail(ChangeSet.fromReplacements(3, [Replacement.delete({ start: 2, end: 4 })]))).toBe(
      true,
    );
    expect(
      fail(
        ChangeSet.fromReplacements(6, [
          { from: 1, to: 4, text: "a" },
          { from: 3, to: 5, text: "b" },
        ]),
      ),
    ).toBe(true);
    const ignored = run(
      ChangeSet.fromReplacements(
        6,
        [
          { from: 3, to: 5, text: "b" },
          { from: 1, to: 4, text: "a" },
        ],
        { overlap: "ignoreLater" },
      ),
    );
    expect(ignored.replacements).toEqual([{ from: 3, to: 5, text: "b" }]);
    const inserts = run(
      ChangeSet.fromReplacements(4, [
        { from: 2, to: 2, text: "A" },
        { from: 2, to: 2, text: "B" },
      ]),
    );
    expect(inserts.apply(TextBuffer.fromString("abcd")).toString()).toBe("abABcd");
    expect(
      run(
        ChangeSet.fromReplacements(4, [
          { from: 1, to: 2, text: "A" },
          { from: 2, to: 3, text: "B" },
        ]),
      )
        .apply(TextBuffer.fromString("abcd"))
        .toString(),
    ).toBe("aABd");
  });

  test("fromOperations normalizes, iterates replacements, and applies", () => {
    const replacement = run(
      ChangeSet.fromOperations([
        { kind: "retain", length: 2 },
        { kind: "delete", length: 1 },
        { kind: "delete", length: 1 },
        { kind: "insert", text: "X" },
        { kind: "insert", text: "Y" },
        { kind: "retain", length: 2 },
      ]),
    );
    expect(replacement.length).toBe(6);
    expect(replacement.lengthAfter).toBe(6);
    expect(replacement.changes()).toEqual([
      { kind: "retain", length: 2 },
      { kind: "delete", length: 2 },
      { kind: "insert", text: "XY" },
      { kind: "retain", length: 2 },
    ]);
    expect([...replacement.iterReplacements()]).toEqual([
      { span: { start: 2, end: 4 }, text: "XY" },
    ]);
    expect(replacement.apply(TextBuffer.fromString("abcdef")).toString()).toBe("abXYef");

    const insertThenDelete = run(
      ChangeSet.fromOperations([
        { kind: "retain", length: 1 },
        { kind: "insert", text: "I" },
        { kind: "delete", length: 2 },
        { kind: "retain", length: 3 },
      ]),
    );
    expect(insertThenDelete.length).toBe(6);
    expect(insertThenDelete.lengthAfter).toBe(5);
    expect([...insertThenDelete.iterReplacements()]).toEqual([
      { span: { start: 1, end: 1 }, text: "I" },
      { span: { start: 1, end: 3 }, text: null },
    ]);
    expect(insertThenDelete.apply(TextBuffer.fromString("abcdef")).toString()).toBe("aIdef");

    expect([
      ...run(
        ChangeSet.fromOperations([
          { kind: "retain", length: 1 },
          { kind: "insert", text: "Z" },
        ]),
      ).iterReplacements(),
    ]).toEqual([{ span: { start: 1, end: 1 }, text: "Z" }]);
    expect([
      ...run(
        ChangeSet.fromOperations([
          { kind: "retain", length: 1 },
          { kind: "delete", length: 2 },
        ]),
      ).iterReplacements(),
    ]).toEqual([{ span: { start: 1, end: 3 }, text: null }]);

    const zeros = run(
      ChangeSet.fromOperations([
        { kind: "retain", length: 0 },
        { kind: "delete", length: 0 },
        { kind: "insert", text: "" },
      ]),
    );
    expect(zeros.changes()).toEqual([]);
    for (const effect of [
      ChangeSet.fromOperations([{ kind: "retain", length: -1 }]),
      ChangeSet.fromOperations([{ kind: "delete", length: -1 }]),
      ChangeSet.fromOperations([{ kind: "retain", length: 1.5 }]),
    ]) {
      const result = Effect.runSync(Effect.either(effect));
      expect(Either.isLeft(result) && result.left instanceof InvalidOperation).toBe(true);
    }
  });

  test("apply and length mismatch", () => {
    const before = TextBuffer.fromString("ab\r\n😀\uD800z");
    const tx = run(
      Transaction.fromReplacements(before.length, [
        { from: 2, to: 4, text: "\n" },
        { from: before.length, to: before.length, text: "!" },
      ]),
    );
    expect(run(tx.applyTo(before)).toString()).toBe(
      oracle(before.toString(), tx.changes.replacements),
    );
    const either = Effect.runSync(Effect.either(tx.applyTo(TextBuffer.fromString("short"))));
    expect(Either.isLeft(either)).toBe(true);
    if (Either.isLeft(either)) expect(either.left).toBeInstanceOf(LengthMismatch);
  });

  test("invert restores original and length fields", () => {
    const samples = ["abc", "a\r\nb😀c", "x\uD800\uDC00y"];
    for (const text of samples) {
      const before = TextBuffer.fromString(text);
      const tx = run(
        Transaction.fromReplacements(before.length, [
          { from: 0, to: Math.min(1, before.length), text: "I" },
          { from: before.length, to: before.length, text: "!" },
        ]),
      );
      const after = tx.apply(before);
      const inv = run(tx.invert(before));
      expect(inv.length).toBe(tx.lengthAfter);
      expect(inv.lengthAfter).toBe(tx.length);
      expect(inv.apply(after).toString()).toBe(text);
    }
  });

  test("mapping basics and cumulative deltas", () => {
    const twoInsertions = run(
      Transaction.fromReplacements(6, [
        { from: 1, to: 1, text: "A" },
        { from: 3, to: 3, text: "B" },
      ]),
    );
    expect(twoInsertions.mapPosition(6, "after")).toBe(8);
    expect(twoInsertions.mapPosition(4, "after")).toBe(6);

    const mixed = run(
      Transaction.fromReplacements(10, [
        { from: 1, to: 1, text: "A" },
        { from: 3, to: 5, text: "" },
        { from: 8, to: 9, text: "XYZ" },
      ]),
    );
    expect(mixed.mapPosition(0)).toBe(0);
    expect(mixed.mapPosition(1, "before")).toBe(1);
    expect(mixed.mapPosition(1, "after")).toBe(2);
    expect(mixed.mapPosition(2)).toBe(3);
    expect(mixed.mapPosition(4, "before")).toBe(4);
    expect(mixed.mapPosition(6)).toBe(5);
    expect(mixed.mapPosition(8, "after")).toBe(7);
    expect(mixed.mapPosition(10)).toBe(11);
    for (const assoc of ["before", "after", "afterSticky", "trackDelAfter"] as const) {
      expect(mixed.desc().mapPosition(6, assoc)).toBe(mixed.mapPosition(6, assoc));
    }

    const repThenIns = run(
      Transaction.fromReplacements(8, [
        { from: 1, to: 3, text: "Q" },
        { from: 5, to: 5, text: "ZZ" },
      ]),
    );
    expect(repThenIns.mapPosition(8)).toBe(9);
    expect(repThenIns.mapPosition(4)).toBe(3);
    expect(repThenIns.mapPosition(2, "after")).toBe(2);
    expect(ChangeDesc.empty(3).mapPosition(2)).toBe(2);
  });

  test("Assoc insertion and word behavior has exact values", () => {
    const word = run(Transaction.insertAt(5, 2, "abc"));
    const nonWord = run(Transaction.insertAt(5, 2, "!!!"));
    const expected: Record<Assoc, number> = {
      before: 2,
      after: 5,
      beforeSticky: 2,
      afterSticky: 5,
      beforeWord: 2,
      afterWord: 5,
      trackDel: 2,
      trackDelSticky: 2,
      trackDelBefore: 2,
      trackDelAfter: 2,
      trackDelBeforeSticky: 2,
      trackDelAfterSticky: 5,
    };
    for (const [assoc, value] of Object.entries(expected) as Array<[Assoc, number]>) {
      expect(word.mapPosition(2, assoc)).toBe(value);
      expect(word.desc().mapPosition(2, assoc)).toBe(value);
    }
    expect(nonWord.mapPosition(2, "afterWord")).toBe(2);
    expect(nonWord.mapPosition(2, "beforeWord")).toBe(5);
    expect(nonWord.desc().mapPosition(2, "afterWord")).toBe(2);
    expect(nonWord.desc().mapPosition(2, "beforeWord")).toBe(5);

    const cases = [
      ["abc", 5, 2],
      ["!!!", 2, 5],
      ["abc!!!", 5, 8],
      ["!!!abc", 2, 5],
      ["abc!!!def", 5, 8],
    ] as const;
    for (const [text, afterWord, beforeWord] of cases) {
      const tx = run(Transaction.insertAt(5, 2, text));
      expect(tx.mapPosition(2, "afterWord")).toBe(afterWord);
      expect(tx.mapPosition(2, "beforeWord")).toBe(beforeWord);
      expect(tx.desc().mapPosition(2, "afterWord")).toBe(afterWord);
      expect(tx.desc().mapPosition(2, "beforeWord")).toBe(beforeWord);
    }
  });

  test("deletion tracking boundary semantics", () => {
    const del = run(Transaction.delete(7, { start: 2, end: 5 }));
    expect(del.mapPosition(1)).toBe(1);
    expect(del.mapPosition(6)).toBe(3);
    expect(del.mapPosition(3, "before")).toBe(2);
    const expectDeleted = (pos: number, assoc: Assoc, offset: number) => {
      const mapped = del.mapPositionTracked(pos, assoc);
      expect(mapped).toEqual({ type: "deleted", pos: 2, offset, length: 3 });
    };
    const expectPos = (pos: number, assoc: Assoc, mapped: number) =>
      expect(del.mapPositionTracked(pos, assoc)).toEqual({ type: "pos", pos: mapped });
    expectPos(2, "trackDelBefore", 2);
    expectDeleted(2, "trackDelAfter", 0);
    for (const assoc of [
      "trackDel",
      "trackDelSticky",
      "trackDelBefore",
      "trackDelAfter",
      "trackDelBeforeSticky",
      "trackDelAfterSticky",
    ] as const) {
      if (assoc === "trackDelAfter") expectDeleted(3, assoc, 1);
      else expectDeleted(3, assoc, 1);
      expectDeleted(4, assoc, 2);
    }
    expectDeleted(5, "trackDelBefore", 3);
    expectPos(5, "trackDelAfter", 2);
  });

  test("replacement mapping exact values", () => {
    const exact = run(Transaction.replace(7, { start: 2, end: 5 }, "XYZ"));
    expect(exact.mapPosition(3, "afterSticky")).toBe(3);
    expect(exact.mapPosition(3, "beforeSticky")).toBe(3);
    expect(exact.mapPosition(3, "before")).toBe(2);
    expect(exact.mapPosition(3, "after")).toBe(5);
    expect(exact.mapRange({ start: 1, end: 6 })).toEqual({ start: 1, end: 6 });
    const small = run(Transaction.replace(7, { start: 2, end: 5 }, "Q"));
    expect(small.mapPosition(3, "beforeSticky")).toBe(2);
    expect(small.mapPosition(3, "afterSticky")).toBe(3);
    expect(small.mapPosition(3, "before")).toBe(2);
    expect(small.mapPosition(3, "after")).toBe(3);
  });

  test("MappedPos helpers", () => {
    expect(MappedPos.position(MappedPos.pos(3))).toBe(3);
    const d = MappedPos.deleted({ pos: 2, offset: 1, length: 4 });
    expect(MappedPos.isDeleted(d)).toBe(true);
    expect(MappedPos.ok(d)).toBeUndefined();
  });

  test("updatePositions", () => {
    const cs = run(ChangeSet.fromReplacements(5, [{ from: 1, to: 2, text: "XX" }]));
    const a = { value: 0 },
      b = { value: 4 };
    run(
      cs.updatePositions([
        { position: b, assoc: "after" },
        { position: a, assoc: "before" },
      ]),
    );
    expect([a.value, b.value]).toEqual([0, cs.mapPosition(4, "after")]);
    expect(fail(cs.updatePositions([{ position: { value: 1 }, assoc: "trackDel" }]))).toBe(true);
    expect(fail(cs.updatePositions([{ position: { value: 99 }, assoc: "after" }]))).toBe(true);
    expect(
      Effect.runSync(
        Effect.either(cs.updatePositions([{ position: { value: 1 }, assoc: "trackDel" }])),
      )._tag,
    ).toBe("Left");
  });

  test("range mapping defaults and compatibility", () => {
    const ins = run(Transaction.insertAt(10, 2, "XX"));
    expect(ins.mapRange({ start: 2, end: 5 })).toEqual({ start: 2, end: 7 });
    expect(ins.mapRange({ start: 2, end: 5 }, "after")).toEqual({ start: 4, end: 7 });
    expect(ins.mapRange({ start: 2, end: 5 }, { startAssoc: "after", endAssoc: "before" })).toEqual(
      { start: 4, end: 7 },
    );
    const endIns = run(Transaction.insertAt(10, 5, "YY"));
    expect(endIns.mapRange({ start: 2, end: 5 })).toEqual({ start: 2, end: 7 });
    const del = run(Transaction.delete(10, { start: 3, end: 7 }));
    expect(del.mapRange({ start: 2, end: 8 })).toEqual({ start: 2, end: 4 });
  });

  test("Effect mapping validation", () => {
    const tx = run(Transaction.replace(6, { start: 2, end: 4 }, "X"));
    const badLow = Effect.runSync(Effect.either(tx.mapPositionEffect(-1)));
    const badHigh = Effect.runSync(Effect.either(tx.mapPositionEffect(7)));
    const badSpan = Effect.runSync(Effect.either(tx.mapRangeEffect({ start: 5, end: 2 })));
    const outSpan = Effect.runSync(Effect.either(tx.mapRangeEffect({ start: 0, end: 7 })));
    expect(Either.isLeft(badLow) && badLow.left instanceof PositionsOutOfBounds).toBe(true);
    expect(Either.isLeft(badHigh) && badHigh.left instanceof PositionsOutOfBounds).toBe(true);
    expect(Either.isLeft(badSpan) && badSpan.left instanceof InvalidSpan).toBe(true);
    expect(Either.isLeft(outSpan) && outSpan.left instanceof SpanOutOfBounds).toBe(true);
  });

  test("deleteSpans policies", () => {
    expect(
      fail(
        Transaction.deleteSpans(6, [
          { start: 1, end: 4 },
          { start: 3, end: 5 },
        ]),
      ),
    ).toBe(true);
    expect(
      run(
        Transaction.deleteSpans(
          6,
          [
            { start: 1, end: 4 },
            { start: 3, end: 5 },
          ],
          { overlap: "ignoreLater" },
        ),
      )
        .apply(TextBuffer.fromString("abcdef"))
        .toString(),
    ).toBe("aef");
    expect(
      run(
        Transaction.deleteSpans(
          6,
          [
            { start: 1, end: 3 },
            { start: 3, end: 5 },
          ],
          { overlap: "merge" },
        ),
      )
        .apply(TextBuffer.fromString("abcdef"))
        .toString(),
    ).toBe("af");
  });

  test("compose length mismatch and ChangeDesc compose", () => {
    const a = run(Transaction.insertAt(3, 1, "x"));
    const b = run(Transaction.insertAt(99, 1, "y"));
    const mismatch = Effect.runSync(Effect.either(a.compose(b)));
    expect(Either.isLeft(mismatch) && mismatch.left instanceof ComposeLengthMismatch).toBe(true);
    const descMismatch = Effect.runSync(Effect.either(a.desc().compose(b.desc())));
    expect(Either.isLeft(descMismatch) && descMismatch.left instanceof ComposeLengthMismatch).toBe(
      true,
    );

    const c = run(Transaction.insertAt(4, 2, "abc!!!def"));
    const d = run(Transaction.delete(c.lengthAfter, { start: 4, end: 7 }));
    const cd = run(c.compose(d));
    const composedDesc = run(c.desc().compose(d.desc()));
    for (const assoc of [
      "before",
      "after",
      "beforeSticky",
      "afterSticky",
      "beforeWord",
      "afterWord",
    ] as const) {
      for (const pos of [0, 2, 3, 4]) {
        expect(composedDesc.mapPosition(pos, assoc)).toBe(cd.mapPosition(pos, assoc));
      }
    }
  });

  test("compose sequential apply law for explicit edge cases", () => {
    const cases: Array<[string, Transaction, Transaction]> = [];
    const add = (text: string, a: Transaction, b: Transaction) => cases.push([text, a, b]);
    add("abcdef", run(Transaction.insertAt(6, 0, "A")), run(Transaction.insertAt(7, 7, "B")));
    add(
      "abcdef",
      run(Transaction.insertAt(6, 2, "X")),
      run(Transaction.delete(7, { start: 2, end: 3 })),
    );
    add(
      "abcdef",
      run(Transaction.insertAt(6, 2, "abc")),
      run(Transaction.replace(9, { start: 3, end: 4 }, "X")),
    );
    add(
      "abcdef",
      run(Transaction.delete(6, { start: 2, end: 4 })),
      run(Transaction.insertAt(4, 2, "XY")),
    );
    add(
      "abcdef",
      run(Transaction.replace(6, { start: 2, end: 4 }, "wxyz")),
      run(Transaction.replace(8, { start: 3, end: 5 }, "Q")),
    );
    add(
      "abcdef",
      run(Transaction.replace(6, { start: 2, end: 4 }, "wxyz")),
      run(Transaction.delete(8, { start: 3, end: 6 })),
    );
    add(
      "abcdef",
      run(Transaction.insertAt(6, 3, "XYZ")),
      run(Transaction.delete(9, { start: 2, end: 7 })),
    );
    add(
      "abcdef",
      run(
        Transaction.fromReplacements(6, [
          { from: 1, to: 2, text: "A" },
          { from: 3, to: 4, text: "B" },
        ]),
      ),
      run(
        Transaction.fromReplacements(6, [
          { from: 2, to: 3, text: "C" },
          { from: 4, to: 5, text: "D" },
        ]),
      ),
    );
    add("abcdef", run(Transaction.insertAt(6, 2, "A")), run(Transaction.insertAt(7, 2, "B")));
    add(
      "a\r\n😀\uD800z",
      run(Transaction.replace("a\r\n😀\uD800z".length, { start: 1, end: 4 }, "X\r\n")),
      run(Transaction.insertAt(7, 2, "👨")),
    );

    for (const [text, a, b] of cases) {
      const ab = run(a.compose(b));
      const sequential = applyString(applyString(text, a), b);
      expect(applyString(text, ab)).toBe(sequential);
      expect(
        run(ab.invert(TextBuffer.fromString(text)))
          .apply(TextBuffer.fromString(sequential))
          .toString(),
      ).toBe(text);
      let previousKind: string | undefined;
      for (const op of ab.changes.changes()) {
        if (op.kind === "insert") expect(op.text.length).toBeGreaterThan(0);
        else expect(op.length).toBeGreaterThan(0);
        expect(op.kind).not.toBe(previousKind);
        previousKind = op.kind;
      }
      expect(oracle(text, ab.changes.replacements)).toBe(sequential);
    }
  });

  test("compose mapping law for representative non-tracking assoc", () => {
    const text = "abcdefghij";
    const a = run(
      Transaction.fromReplacements(text.length, [
        { from: 1, to: 1, text: "abc!!!def" },
        { from: 5, to: 7, text: "Q" },
      ]),
    );
    const mid = applyString(text, a);
    const b = run(
      Transaction.fromReplacements(mid.length, [
        { from: 2, to: 5, text: "!!xy" },
        { from: mid.length, to: mid.length, text: "Z" },
      ]),
    );
    const ab = run(a.compose(b));
    for (const assoc of [
      "before",
      "after",
      "beforeSticky",
      "afterSticky",
      "beforeWord",
      "afterWord",
    ] as const) {
      for (const pos of [0, 1, 2, 5, 7, text.length]) {
        expect(ab.mapPosition(pos, assoc)).toBe(b.mapPosition(a.mapPosition(pos, assoc), assoc));
      }
    }
    const desc = run(a.desc().compose(b.desc()));
    for (const assoc of [
      "before",
      "after",
      "beforeSticky",
      "afterSticky",
      "beforeWord",
      "afterWord",
    ] as const) {
      for (const pos of [0, 1, 2, 5, 7, text.length]) {
        expect(desc.mapPosition(pos, assoc)).toBe(ab.mapPosition(pos, assoc));
      }
    }
    const tracked = ab.mapPositionTracked(8, "trackDelAfter");
    expect(MappedPos.position(tracked)).toBe(
      b.mapPosition(a.mapPosition(8, "trackDelAfter"), "trackDelAfter"),
    );
  });

  test("deterministic random compose apply/invert property", () => {
    let seed = 4321;
    const rnd = () => (seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32;
    const randomReps = (length: number): ReplacementInput[] => {
      const reps: ReplacementInput[] = [];
      let pos = 0;
      while (pos <= length && reps.length < 8) {
        pos += Math.floor(rnd() * 6);
        if (pos > length) break;
        const to = Math.min(length, pos + Math.floor(rnd() * 4));
        const text =
          Math.floor(rnd() * 4) === 0 ? "" : ["x", "YZ", "\r\n", "😀"][Math.floor(rnd() * 4)]!;
        reps.push({ from: pos, to, text });
        pos = to;
      }
      return reps;
    };
    for (let round = 0; round < 40; round++) {
      const text = Array.from({ length: 50 }, (_, i) =>
        i % 13 === 0 ? "\n" : String.fromCharCode(97 + Math.floor(rnd() * 26)),
      ).join("");
      const a = run(Transaction.fromReplacements(text.length, randomReps(text.length)));
      const mid = applyString(text, a);
      const b = run(Transaction.fromReplacements(mid.length, randomReps(mid.length)));
      const ab = run(a.compose(b));
      const finalText = applyString(mid, b);
      expect(applyString(text, ab)).toBe(finalText);
      expect(oracle(text, ab.changes.toLegacyReplacements())).toBe(finalText);
      expect(
        run(ab.invert(TextBuffer.fromString(text)))
          .apply(TextBuffer.fromString(finalText))
          .toString(),
      ).toBe(text);

      const c = run(Transaction.fromReplacements(finalText.length, randomReps(finalText.length)));
      const left = run(ab.compose(c));
      const right = run(a.compose(run(b.compose(c))));
      expect(applyString(text, left)).toBe(applyString(text, right));
    }
  });

  test("deterministic random apply/invert", () => {
    let seed = 1234;
    const rnd = () => (seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32;
    for (let round = 0; round < 30; round++) {
      const text = Array.from({ length: 80 }, (_, i) =>
        i % 11 === 0 ? "\n" : String.fromCharCode(97 + Math.floor(rnd() * 26)),
      ).join("");
      const reps: ReplacementInput[] = [];
      let pos = 0;
      while (pos <= text.length && reps.length < 10) {
        pos += Math.floor(rnd() * 7);
        if (pos > text.length) break;
        const to = Math.min(text.length, pos + Math.floor(rnd() * 4));
        reps.push({ from: pos, to, text: Math.floor(rnd() * 3) === 0 ? "" : `X${round}` });
        pos = to;
      }
      const tx = run(Transaction.fromReplacements(text.length, reps));
      const before = TextBuffer.fromString(text);
      const after = tx.apply(before);
      expect(after.toString()).toBe(oracle(text, tx.changes.replacements));
      expect(run(tx.invert(before)).apply(after).toString()).toBe(text);
    }
  });
});
