import { Effect } from "effect";
import { describe, expect, test } from "vitest";
import {
  RegisterData,
  Selection,
  TextDocument,
  SelectionState,
  type ClipboardProvider,
  type ClipboardError,
} from "@the-editor/engine";
import {
  ActiveClipboard,
  ActiveRegisters,
  makeEditorScope,
  provideEditorScope,
  closeEditorScope,
} from "../src/index";

const run = <A, E>(effect: Effect.Effect<A, E>) => Effect.runSync(effect);

class CustomClipboard implements ClipboardProvider {
  readonly name = "custom";
  getContents() {
    return Effect.succeed("custom");
  }
  setContents() {
    return Effect.void as Effect.Effect<void, ClipboardError>;
  }
}

describe("editor scope", () => {
  test("makeEditorScope creates an id and closeable scope", () => {
    const editor = run(makeEditorScope());
    expect(editor.id).toMatch(/^editor:/);
    expect(editor.context).toBeDefined();
    run(closeEditorScope(editor));
  });

  test("provided initial text is visible through TextDocument", () => {
    const editor = run(makeEditorScope({ text: "abc" }));
    const text = run(
      provideEditorScope(
        editor,
        Effect.flatMap(TextDocument, (d) => d.get),
      ),
    );
    expect(text.toString()).toBe("abc");
  });

  test("provided initial selection is visible through SelectionState", () => {
    const selection = Selection.single(1, 2);
    const editor = run(makeEditorScope({ text: "abc", selection }));
    const current = run(
      provideEditorScope(
        editor,
        Effect.flatMap(SelectionState, (s) => s.get),
      ),
    );
    expect(current.ranges).toEqual(selection.ranges);
  });

  test("registers service is scoped and mutable", () => {
    const editor = run(makeEditorScope());
    const value = run(
      provideEditorScope(
        editor,
        Effect.map(ActiveRegisters, (r) => {
          r.write("a", RegisterData.text("x"));
          return r.read("a");
        }),
      ),
    );
    expect(value).toEqual(RegisterData.text("x"));
  });

  test("clipboard defaults to NoClipboard", () => {
    const editor = run(makeEditorScope());
    expect(
      run(
        provideEditorScope(
          editor,
          Effect.map(ActiveClipboard, (c) => c.name),
        ),
      ),
    ).toBe("none");
  });

  test("custom clipboard provider is provided", () => {
    const editor = run(makeEditorScope({ clipboard: new CustomClipboard() }));
    expect(
      run(
        provideEditorScope(
          editor,
          Effect.map(ActiveClipboard, (c) => c.name),
        ),
      ),
    ).toBe("custom");
  });
});
