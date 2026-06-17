import { Effect } from "effect";
import { describe, expect, test } from "vitest";
import {
  Selection,
  TextBuffer,
  Transaction,
  type ChangeDesc,
  type TextEdit,
} from "@the-editor/engine";
import type { KeyboardEventLike } from "@the-editor/keymaps";
import type { PanelId } from "@the-editor/workbench";
import { buildMultiBuffer } from "./builder";
import { MultiBufferPane, type MultiBufferHost } from "./pane";

const key = (key: string): KeyboardEventLike =>
  ({ key, metaKey: false, ctrlKey: false, shiftKey: false, altKey: false }) as KeyboardEventLike;

/** A host whose commits apply edits to in-memory buffers (mirrors the runtime). */
function makeHost(buffers: Map<string, TextBuffer>): MultiBufferHost {
  const versions = new Map<string, number>();
  return {
    subscribeTheme: () => () => {},
    jumpToLocation: async () => {},
    commitDocumentEdits(docId: string, edits: readonly TextEdit[]) {
      const before = buffers.get(docId);
      if (before === undefined) return undefined;
      const transaction = Effect.runSync(
        Transaction.fromReplacements(before.length, edits).pipe(Effect.orDie),
      );
      const after = transaction.apply(before);
      buffers.set(docId, after);
      versions.set(docId, (versions.get(docId) ?? 0) + 1);
      return { change: transaction.changes.desc() as ChangeDesc, buffer: after };
    },
    documentHighlights: () => ({
      ensureSynced: () => {},
      runsBetween: (from, to) => (to > from ? [{ from, to, kind: "keyword" }] : []),
      subscribe: () => () => {},
    }),
    documentVersion: (docId) => versions.get(docId) ?? 0,
  };
}

const makePane = () => {
  const a = TextBuffer.fromString("alpha\nbeta\ngamma\ndelta\nepsilon\n");
  const b = TextBuffer.fromString("one\ntwo\nthree\nfour\n");
  const buffers = new Map([
    ["doc:a", a],
    ["doc:b", b],
  ]);
  const build = buildMultiBuffer(
    [
      { path: "src/a.ts", documentId: "doc:a", buffer: a, matches: [{ start: a.lineRange(2).from, end: a.lineRange(2).to }] },
      { path: "src/b.ts", documentId: "doc:b", buffer: b, matches: [{ start: b.lineRange(1).from, end: b.lineRange(1).to }] },
    ],
    { contextLines: 1 },
  );
  const pane = new MultiBufferPane("panel:test" as unknown as PanelId, "test", build, makeHost(buffers));
  return { pane, buffers, build };
};

describe("MultiBufferPane", () => {
  test("typing routes through editPlan to the real source document", () => {
    const { pane, buffers } = makePane();
    pane.setSelection(Selection.point(0));
    expect(pane.keyInterceptor(key("X"))).toBe(true);
    // The unified text now starts with the inserted char...
    expect(pane.snapshot().text.sliceString(0, 1)).toBe("X");
    // ...and so does the underlying source document (excerpt starts at "beta").
    const source = buffers.get("doc:a")!;
    expect(source.sliceString(0, 7)).toBe("alpha\nX");
  });

  test("Backspace deletes through editPlan; caret follows", () => {
    const { pane, buffers } = makePane();
    pane.setSelection(Selection.point(1)); // after the first char of the excerpt
    expect(pane.keyInterceptor(key("Backspace"))).toBe(true);
    expect(buffers.get("doc:a")!.sliceString(0, 6)).toBe("alpha\n"); // "beta" lost its "b"
    expect(pane.snapshot().selection.primary.head).toBe(0);
  });

  test("folding a file collapses it; unfolding restores it", () => {
    const { pane } = makePane();
    const full = pane.snapshot().text.lineCount;
    pane.toggleFold("doc:a");
    expect(pane.snapshot().text.lineCount).toBeLessThan(full);
    expect(pane.metas.find((m) => m.excerpt.documentId === "doc:a")?.folded).toBe(true);
    pane.toggleFold("doc:a");
    expect(pane.snapshot().text.lineCount).toBe(full);
  });

  test("expanding an excerpt grows its context", () => {
    const { pane, build } = makePane();
    const before = pane.snapshot().text.length;
    pane.expandExcerpt(build.excerpts[0]!.id, "down");
    expect(pane.snapshot().text.length).toBeGreaterThan(before);
  });

  test("syntax adapter maps source runs into unified coordinates", () => {
    const { pane } = makePane();
    const runs = pane.highlights!.runsBetween(0, 4);
    expect(runs.length).toBeGreaterThan(0);
    for (const run of runs) {
      expect(run.from).toBeGreaterThanOrEqual(0);
      expect(run.to).toBeLessThanOrEqual(pane.snapshot().text.length);
      expect(run.kind).toBe("keyword");
    }
  });
});
