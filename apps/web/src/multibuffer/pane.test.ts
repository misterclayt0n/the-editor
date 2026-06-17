import { Effect } from "effect";
import { describe, expect, test } from "vitest";
import {
  Selection,
  TextBuffer,
  Transaction,
  type ChangeDesc,
  type TextEdit,
} from "@the-editor/engine";
import { DocumentInvalidated, payloadsOf, type DocumentChangeRange } from "@the-editor/editor";
import type { KeyboardEventLike } from "@the-editor/keymaps";
import type { PanelId } from "@the-editor/workbench";
import { buildMultiBuffer } from "./builder";
import { MultiBufferPane, type MultiBufferHost } from "./pane";

const key = (key: string): KeyboardEventLike =>
  ({ key, metaKey: false, ctrlKey: false, shiftKey: false, altKey: false }) as KeyboardEventLike;

/** A host whose commits apply edits to in-memory buffers (mirrors the runtime). */
function makeHost(buffers: Map<string, TextBuffer>) {
  const versions = new Map<string, number>();
  const subscribers = new Map<string, Set<() => void>>();
  const notify = (docId: string) => subscribers.get(docId)?.forEach((listener) => listener());
  const host: MultiBufferHost = {
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
    documentBuffer: (docId) => buffers.get(docId),
    documentHighlights: () => ({
      ensureSynced: () => {},
      runsBetween: (from, to) => (to > from ? [{ from, to, kind: "keyword" }] : []),
      subscribe: () => () => {},
    }),
    documentVersion: (docId) => versions.get(docId) ?? 0,
    documentVisuals: () => ({
      generation: 1,
      textStylesBetween: () => [],
      // A stub "diagnostic" underline at the start of any queried range.
      decorationsBetween: (from, to) =>
        to > from ? [{ from, to: Math.min(to, from + 3), layer: "underline", className: "diag" }] : [],
      virtualTextBetween: () => [],
      widgets: () => [],
      totalWidgetRows: 0,
      gutterBands: [],
    }),
    subscribeDocument: (docId, listener) => {
      if (!subscribers.has(docId)) subscribers.set(docId, new Set());
      subscribers.get(docId)!.add(listener);
      return () => subscribers.get(docId)?.delete(listener);
    },
    attachDocumentIntel: () => {},
    detachDocumentIntel: () => {},
  };
  return { host, notify };
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
  const { host, notify } = makeHost(buffers);
  const pane = new MultiBufferPane("panel:test" as unknown as PanelId, "test", build, host);
  return { pane, buffers, build, host, notify };
};

/** Intel attaches per visible excerpt and the attach is debounced; cover the
 * whole buffer and wait out the debounce so syntax/diagnostics are live. */
const attachViewport = async (pane: MultiBufferPane): Promise<void> => {
  pane.viewportChanged(0, 0, 10_000);
  await new Promise((resolve) => setTimeout(resolve, 80));
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

  test("external source document edits rebuild the unified snapshot and remap match ranges", async () => {
    const { pane, buffers, notify } = makePane();
    await attachViewport(pane);
    const before = pane.snapshot().text.sliceString(0, 20);
    const matchBefore = pane.metas[0]!.matches[0]!;

    // Simulate an external edit that inserts text inside the excerpt but before
    // the matched region, so the match span must slide to stay on the same text.
    buffers.set("doc:a", TextBuffer.fromString("alpha\nXXbeta\ngamma\ndelta\nepsilon\n"));
    notify("doc:a");

    const after = pane.snapshot().text.sliceString(0, 20);
    expect(after).not.toBe(before);
    expect(pane.snapshot().text.sliceString(0, 7)).toBe("XXbeta\n");

    // The match span should have slid forward by the inserted length.
    const matchAfter = pane.metas[0]!.matches[0]!;
    expect(matchAfter.start).toBe(matchBefore.start + 2);
    expect(matchAfter.end).toBe(matchBefore.end + 2);
  });

  test("a typed char emits its unified change range (so completion can auto-trigger)", () => {
    const { pane } = makePane();
    const changes: DocumentChangeRange[] = [];
    pane.subscribe((notification, source) => {
      if (source !== "document") return;
      for (const payload of payloadsOf(notification, DocumentInvalidated)) {
        if (payload.changes !== undefined) changes.push(...payload.changes);
      }
    });
    pane.setSelection(Selection.point(0));
    pane.keyInterceptor(key("X"));
    // Exactly one single-char insert at offset 0; the controller reads the new
    // text at [fromB, toB) to learn the typed char — here "X".
    expect(changes).toEqual([{ fromA: 0, toA: 0, fromB: 0, toB: 1 }]);
    expect(pane.snapshot().text.sliceString(changes[0]!.fromB, changes[0]!.toB)).toBe("X");
  });

  test("Backspace deletes through editPlan; caret follows", () => {
    const { pane, buffers } = makePane();
    pane.setSelection(Selection.point(1)); // after the first char of the excerpt
    expect(pane.keyInterceptor(key("Backspace"))).toBe(true);
    expect(buffers.get("doc:a")!.sliceString(0, 6)).toBe("alpha\n"); // "beta" lost its "b"
    expect(pane.snapshot().selection.primary.head).toBe(0);
  });

  test("folding a file collapses it to its header only; unfolding restores it", () => {
    const { pane } = makePane();
    const full = pane.snapshot().text.lineCount;
    pane.toggleFold("doc:a");
    const folded = pane.metas.find((m) => m.excerpt.documentId === "doc:a")!;
    expect(folded.folded).toBe(true);
    expect(folded.excerpt.sourceRange.start).toBe(folded.excerpt.sourceRange.end);
    expect(pane.snapshot().text.sliceString(0, pane.snapshot().text.length)).not.toContain("beta");
    pane.toggleFold("doc:a");
    expect(pane.snapshot().text.lineCount).toBe(full);
  });

  test("expanding an excerpt grows its context", () => {
    const { pane, build } = makePane();
    const before = pane.snapshot().text.length;
    pane.expandExcerpt(build.excerpts[0]!.id, "down");
    expect(pane.snapshot().text.length).toBeGreaterThan(before);
  });

  test("syntax adapter maps source runs into unified coordinates", async () => {
    const { pane } = makePane();
    await attachViewport(pane);
    const runs = pane.highlights!.runsBetween(0, 4);
    expect(runs.length).toBeGreaterThan(0);
    for (const run of runs) {
      expect(run.from).toBeGreaterThanOrEqual(0);
      expect(run.to).toBeLessThanOrEqual(pane.snapshot().text.length);
      expect(run.kind).toBe("keyword");
    }
  });

  test("source-doc diagnostics surface as unified decorations", async () => {
    const { pane } = makePane();
    await attachViewport(pane);
    const decorations = pane.visualsSnapshot().decorationsBetween(0, pane.snapshot().text.length);
    // The stub LSP underline from documentVisuals is mapped into the unified view.
    expect(decorations.some((d) => d.layer === "underline" && d.className === "diag")).toBe(true);
    // Match-highlight backgrounds still come through too.
    expect(decorations.some((d) => d.className === "mb-match")).toBe(true);
  });
});
