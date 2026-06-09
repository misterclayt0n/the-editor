import { Effect } from "effect";
import { describe, expect, test } from "vitest";
import {
  AutoPairs,
  EmptyClosePairError,
  EmptyOpenPairError,
  InvalidTypedCharacterError,
  Selection,
  SelectionRange,
  TextBuffer,
  editForBackspace,
  editForTypedChar,
  type AutoPairsEdit,
} from "../src/index";
import { OverlappingReplacement } from "../src/text/transaction";

const run = <A, E>(effect: Effect.Effect<A, E>) => Effect.runSync(effect);
const apply = (buffer: TextBuffer, edit: AutoPairsEdit) =>
  edit.transaction.apply(buffer).toString();
const doc = (text: string) => TextBuffer.fromString(text);
const heads = (selection: Selection) => selection.ranges.map((r) => r.head);
const pairs = () =>
  run(
    AutoPairs.create([
      ['"""', '"""'],
      ["{%", "%}"],
      ["{{", "}}"],
      ['"', '"'],
      ["(", ")"],
    ]),
  );

describe("auto pairs", () => {
  test("rejects empty pairs", () => {
    expect(Effect.runSync(Effect.flip(AutoPairs.create([["", ")"]])))).toBeInstanceOf(
      EmptyOpenPairError,
    );
    expect(Effect.runSync(Effect.flip(AutoPairs.create([["(", ""]])))).toBeInstanceOf(
      EmptyClosePairError,
    );
  });

  test("invalid typed char", () => {
    const buffer = doc("");
    const selection = Selection.point(0);
    expect(Effect.runSync(Effect.flip(editForTypedChar(buffer, selection, "")))).toBeInstanceOf(
      InvalidTypedCharacterError,
    );
    expect(Effect.runSync(Effect.flip(editForTypedChar(buffer, selection, "ab")))).toBeInstanceOf(
      InvalidTypedCharacterError,
    );
  });

  test("inserts single-char pair and selection", () => {
    const buffer = doc("");
    const edit = run(editForTypedChar(buffer, Selection.point(0), "("))!;
    expect(apply(buffer, edit)).toBe("()");
    expect(edit.selection.ranges).toEqual([SelectionRange.point(1)]);
  });

  test("quote heuristic inside/outside words", () => {
    let buffer = doc("hello ");
    let edit = run(editForTypedChar(buffer, Selection.point(6), '"'))!;
    expect(apply(buffer, edit)).toBe('hello ""');

    buffer = doc("hello");
    edit = run(editForTypedChar(buffer, Selection.point(2), '"'))!;
    expect(apply(buffer, edit)).toBe('he"llo');
    expect(edit.selection.ranges).toEqual([SelectionRange.point(3)]);

    buffer = doc("ab");
    edit = run(editForTypedChar(buffer, Selection.point(1), "("))!;
    expect(apply(buffer, edit)).toBe("a(b");
    expect(edit.selection.ranges).toEqual([SelectionRange.point(2)]);
  });

  test("skips existing closer", () => {
    const buffer = doc("()");
    const edit = run(editForTypedChar(buffer, Selection.point(1), ")"))!;
    expect(apply(buffer, edit)).toBe("()");
    expect(edit.selection.ranges).toEqual([SelectionRange.point(2)]);
  });

  test("multi-character pairs insert/skip/backspace", () => {
    const custom = pairs();
    let buffer = doc("{");
    let edit = run(editForTypedChar(buffer, Selection.point(1), "%", custom))!;
    expect(apply(buffer, edit)).toBe("{%%}");
    expect(edit.selection.ranges).toEqual([SelectionRange.point(2)]);

    buffer = doc("{%%}");
    edit = run(editForTypedChar(buffer, Selection.point(2), "%", custom))!;
    expect(apply(buffer, edit)).toBe("{%%}");
    expect(edit.selection.ranges).toEqual([SelectionRange.point(4)]);

    edit = run(editForBackspace(buffer, Selection.point(2), custom))!;
    expect(apply(buffer, edit)).toBe("");
    expect(edit.selection.ranges).toEqual([SelectionRange.point(0)]);
  });

  test("longest match wins and config order breaks ties", () => {
    let buffer = doc('""');
    let edit = run(editForTypedChar(buffer, Selection.point(2), '"', pairs()))!;
    expect(apply(buffer, edit)).toBe('""""""');

    const ordered = run(
      AutoPairs.create([
        ["ax", "yz"],
        ["ax", "yw"],
      ]),
    );
    buffer = doc("a");
    edit = run(editForTypedChar(buffer, Selection.point(1), "x", ordered))!;
    expect(apply(buffer, edit)).toBe("axyz");
    expect(edit.selection.ranges).toEqual([SelectionRange.point(2)]);
  });

  test("multi-cursor all-or-nothing", () => {
    let buffer = doc("a b");
    let selection = Selection.create([SelectionRange.point(1), SelectionRange.point(3)]);
    let edit = run(editForTypedChar(buffer, selection, "("))!;
    expect(apply(buffer, edit)).toBe("a() b()");
    expect(heads(edit.selection)).toEqual([2, 6]);

    buffer = doc("() ()");
    selection = Selection.create([SelectionRange.point(1), SelectionRange.point(4)]);
    edit = run(editForBackspace(buffer, selection))!;
    expect(apply(buffer, edit)).toBe(" ");
    expect(heads(edit.selection)).toEqual([0, 1]);

    buffer = doc("() x");
    selection = Selection.create([SelectionRange.point(1), SelectionRange.point(4)]);
    expect(run(editForBackspace(buffer, selection))).toBeUndefined();
  });

  test("non-empty selection uses insertion edge, not surround", () => {
    const buffer = doc("printf");
    const edit = run(editForTypedChar(buffer, Selection.single(0, 6), "["))!;
    expect(apply(buffer, edit)).toBe("printf[]");
    expect(edit.selection.ranges).toEqual([SelectionRange.create(0, 7)]);
  });

  test("CRLF single-grapheme backward landing behavior", () => {
    const buffer = doc("foo\r\n");
    const selection = Selection.single(5, 3);
    let edit = run(editForTypedChar(buffer, selection, "("))!;
    expect(apply(buffer, edit)).toBe("foo()\r\n");
    expect(edit.selection.ranges).toEqual([SelectionRange.create(5, 4)]);

    edit = run(editForTypedChar(buffer, selection, "'"))!;
    expect(apply(buffer, edit)).toBe("foo'\r\n");
    expect(edit.selection.ranges).toEqual([SelectionRange.create(6, 4)]);
  });

  test("irrelevant char declines", () => {
    expect(run(editForTypedChar(doc(""), Selection.point(0), "a"))).toBeUndefined();
  });

  test("overlapping backspace spans errors", () => {
    const buffer = doc('""""""');
    const selection = Selection.create([SelectionRange.point(1), SelectionRange.point(3)]);
    const custom = run(
      AutoPairs.create([
        ['"""', '"""'],
        ['"', '"'],
      ]),
    );
    expect(Effect.runSync(Effect.flip(editForBackspace(buffer, selection, custom)))).toBeInstanceOf(
      OverlappingReplacement,
    );
  });
});
