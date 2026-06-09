import { Effect } from "effect";
import { describe, expect, test } from "vitest";
import { Selection, SelectionState, TextDocument, Transaction } from "@the-editor/engine";
import { editorSnapshot, makeEditorScope, provideEditorScope } from "../src/index";

const run = <A, E>(effect: Effect.Effect<A, E>) => Effect.runSync(effect);

describe("editor snapshot", () => {
  test("snapshot contains id, text, version, dirty, selection", () => {
    const selection = Selection.single(1, 2);
    const editor = run(makeEditorScope({ id: "snap", text: "abc", selection }));
    const snapshot = run(provideEditorScope(editor, editorSnapshot()));
    expect(snapshot.id).toBe("snap");
    expect(snapshot.text.toString()).toBe("abc");
    expect(snapshot.version).toBe(0);
    expect(snapshot.dirty).toBe(false);
    expect(snapshot.selection.ranges).toEqual(selection.ranges);
  });

  test("snapshot updates after document transaction and selection set", () => {
    const editor = run(makeEditorScope({ text: "abc" }));
    run(
      provideEditorScope(
        editor,
        Effect.gen(function* () {
          const doc = yield* TextDocument;
          const selection = yield* SelectionState;
          const tx = yield* Transaction.insertAt(3, 3, "d");
          yield* doc.transact(tx);
          yield* selection.set(Selection.point(4));
        }),
      ),
    );
    const snapshot = run(provideEditorScope(editor, editorSnapshot()));
    expect(snapshot.text.toString()).toBe("abcd");
    expect(snapshot.version).toBe(1);
    expect(snapshot.dirty).toBe(true);
    expect(snapshot.selection.ranges).toEqual(Selection.point(4).ranges);
  });
});
