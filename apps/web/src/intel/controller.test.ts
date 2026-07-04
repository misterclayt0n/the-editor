/**
 * IntelController completion-session seams: the filter-vs-requery decision.
 *
 * The contract under test (Zed's initial_query rule): every edit refilters
 * the cached provider results client-side — instantly, in BOTH directions —
 * and providers are re-queried exactly when the cache can no longer serve
 * the live prefix: an incomplete list (every edit), or a prefix that
 * stopped extending the one the results were fetched at (deletes). A
 * server-filtered list fetched for "foo" is not a superset of the matches
 * for "fo", so deleting past the fetch point must ask again.
 */

import { Effect, Scope } from "effect";
import { describe, expect, test, vi } from "vitest";

// The real runtime module drags the whole app graph (LSP wire protocol,
// shell, terminal) into the test process; the controller only needs the
// EditorPane class identity for its `instanceof` branches — neither of
// which fires for the fake pane used here.
vi.mock("../runtime", () => ({ EditorPane: class EditorPane {} }));
import { Selection, type RenderableText } from "@the-editor/engine";
import {
  DocumentInvalidated,
  invalidate,
  type DocumentChangeRange,
  type EditorNotification,
} from "@the-editor/editor";
import {
  makeCompletionProviders,
  makeHoverProviders,
  makeSignatureProviders,
  type CompletionItem,
  type CompletionProvider,
  type CompletionResult,
} from "@the-editor/intel";
import { IntelController } from "./controller";
import type { AppRuntime, PaneHost } from "../runtime";

/** Two macrotask turns: enough for a fetch AND its chained stale re-fetch. */
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await new Promise((resolve) => setTimeout(resolve, 0));
};

const items = (labels: readonly string[]): CompletionItem[] =>
  labels.map((label) => ({ label, insertText: label }));

/**
 * A fake single-document pane: a mutable string + caret, publishing
 * DocumentInvalidated batches with real change ranges the way a commit
 * does. Floats and interaction are inert stubs — the controller renders
 * nothing here; the assertions read `getState()`.
 */
function makePane(initial: string) {
  let text = initial;
  let caret = initial.length;
  let version = 1;
  let serial = 0;
  const listeners = new Set<
    (notification: EditorNotification, source: "document" | "view") => void
  >();
  const renderable = {
    get length() {
      return text.length;
    },
    sliceString: (from: number, to: number) => text.slice(from, to),
  } as unknown as RenderableText;
  const pane = {
    mounts: {},
    dispatch: {},
    floats: {
      create: () => ({
        element: {} as HTMLElement,
        setAnchor: () => {},
        hide: () => {},
        dispose: () => {},
      }),
    },
    interaction: {
      onPointerHover: () => () => {},
      onPointerScrolled: () => () => {},
      onKeyIntercept: () => () => {},
    },
    registerPreKeymap: () => () => {},
    dispatchPreKeymap: () => false,
    resolveSource: (offset: number) => ({
      docId: "doc-1",
      path: "/main.ts",
      text: renderable,
      offset,
    }),
    snapshot: () => ({
      id: "editor-1",
      text: renderable,
      version,
      dirty: false,
      selection: Selection.point(caret),
    }),
    visualsSnapshot: () => ({}),
    setSelection: () => {},
    subscribeTheme: () => () => {},
    subscribeFont: () => () => {},
    subscribe: (listener: (n: EditorNotification, source: "document" | "view") => void) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  } as unknown as PaneHost;
  const publish = (changes: ReadonlyArray<DocumentChangeRange>): void => {
    version++;
    const notification: EditorNotification = {
      serial: ++serial,
      invalidations: [invalidate(DocumentInvalidated, { version, changes })],
    };
    for (const listener of [...listeners]) listener(notification, "document");
  };
  return {
    pane,
    /** Type one character at the caret (a single-char insertion change). */
    insert(character: string): void {
      const at = caret;
      text = text.slice(0, at) + character + text.slice(at);
      caret = at + character.length;
      publish([{ fromA: at, toA: at, fromB: at, toB: at + character.length }]);
    },
    /** Backspace: delete the character before the caret. */
    deleteBack(): void {
      const from = caret - 1;
      const oldCaret = caret;
      text = text.slice(0, from) + text.slice(oldCaret);
      caret = from;
      publish([{ fromA: from, toA: oldCaret, fromB: from, toB: from }]);
    },
  };
}

function makeHarness(providers: readonly CompletionProvider[], initialText = "") {
  const completions = Effect.runSync(makeCompletionProviders);
  const hovers = Effect.runSync(makeHoverProviders);
  const signatures = Effect.runSync(makeSignatureProviders);
  const scope = Effect.runSync(Scope.make());
  for (const provider of providers) {
    Effect.runSync(completions.register(provider).pipe(Scope.extend(scope)));
  }
  const controller = new IntelController({} as AppRuntime, completions, hovers, signatures);
  const kit = makePane(initialText);
  controller.setPane(kit.pane);
  return {
    controller,
    insert: kit.insert,
    deleteBack: kit.deleteBack,
    labels: () =>
      controller
        .getState()
        .completion.items.map((scored) => scored.item.label)
        .sort(),
    open: () => controller.getState().completion.open,
  };
}

describe("completion session cache validity", () => {
  test("deleting past the fetched prefix re-queries the provider", async () => {
    const corpus = ["alpha", "foreach", "fork"];
    const calls: Array<{ prefix: string; kind: string }> = [];
    // A "complete" provider that filters server-side — the shape that made
    // the old refilter-only session show stale lists on deletion.
    const serverFiltered: CompletionProvider = {
      id: "server",
      provide: (context) => {
        calls.push({ prefix: context.prefix, kind: context.trigger.kind });
        return { items: items(corpus.filter((word) => word.startsWith(context.prefix))) };
      },
    };
    const h = makeHarness([serverFiltered]);

    h.insert("f");
    await settle();
    expect(calls).toEqual([{ prefix: "f", kind: "typing" }]);
    expect(h.labels()).toEqual(["foreach", "fork"]);

    // Typing forward extends the fetched prefix: client-side narrowing only.
    h.insert("o");
    await settle();
    expect(calls.length).toBe(1);
    expect(h.labels()).toEqual(["foreach", "fork"]);

    // Deleting back TO the fetched prefix: the cache is still a superset.
    h.deleteBack();
    await settle();
    expect(calls.length).toBe(1);
    expect(h.labels()).toEqual(["foreach", "fork"]);

    // Deleting PAST it: "" does not extend "f" — the "f"-filtered list is
    // not a superset of the matches for "" — re-query, list broadens.
    h.deleteBack();
    await settle();
    expect(calls.length).toBe(2);
    expect(calls[1]).toEqual({ prefix: "", kind: "invoked" });
    expect(h.labels()).toEqual(["alpha", "foreach", "fork"]);
  });

  test("incomplete lists refilter instantly and re-query on every edit, deletes included", async () => {
    const corpus = ["foreach", "fork", "future"];
    const calls: Array<{ prefix: string; kind: string }> = [];
    // Truncated server list, marked incomplete (rust-analyzer style).
    const truncated: CompletionProvider = {
      id: "truncated",
      provide: (context) => {
        calls.push({ prefix: context.prefix, kind: context.trigger.kind });
        const matching = corpus.filter((word) => word.startsWith(context.prefix));
        return { items: items(matching.slice(0, 2)), isIncomplete: true };
      },
    };
    const h = makeHarness([truncated]);

    h.insert("f");
    await settle();
    expect(h.labels()).toEqual(["foreach", "fork"]);

    // The partial cache has no "fu" match — the popup empties INSTANTLY but
    // the session survives (the cache can't say; the fetch decides).
    h.insert("u");
    expect(h.open()).toBe(false);
    expect(h.labels()).toEqual([]);
    await settle();
    expect(calls.at(-1)).toEqual({ prefix: "fu", kind: "incomplete" });
    expect(h.labels()).toEqual(["future"]);
    expect(h.open()).toBe(true);

    // Delete: instant client-side refilter of the cache, then a fresh fetch.
    h.deleteBack();
    expect(h.labels()).toEqual(["future"]);
    await settle();
    expect(calls.at(-1)).toEqual({ prefix: "f", kind: "incomplete" });
    expect(h.labels()).toEqual(["foreach", "fork"]);
  });

  test("prefix-independent providers are never re-queried; deletes broaden client-side", async () => {
    const corpus = ["alpha", "beta", "fork"];
    let calls = 0;
    // The word-completions shape: a full-buffer corpus, fetched once.
    const words: CompletionProvider = {
      id: "words",
      fallback: true,
      prefixIndependent: true,
      provide: () => {
        calls++;
        return { items: items(corpus) };
      },
    };
    const h = makeHarness([words]);

    h.insert("f");
    await settle();
    expect(h.labels()).toEqual(["fork"]);

    h.insert("o");
    await settle();
    h.deleteBack();
    h.deleteBack();
    await settle();
    expect(calls).toBe(1); // the corpus covers any prefix — refilter is enough
    expect(h.labels()).toEqual(["alpha", "beta", "fork"]);
  });

  test("deleting past the word start dismisses the session", async () => {
    const provider: CompletionProvider = {
      id: "p",
      provide: () => ({ items: items(["foo"]) }),
    };
    const h = makeHarness([provider], "x ");

    h.insert("f");
    await settle();
    expect(h.open()).toBe(true);

    h.deleteBack(); // caret back AT the anchor: empty prefix, still alive
    await settle();
    expect(h.open()).toBe(true);

    h.deleteBack(); // caret BEFORE the anchor: the session is over
    await settle();
    expect(h.open()).toBe(false);
  });

  test("a fetch that lands stale (prefix shrank mid-flight) re-queries once more", async () => {
    const corpus = ["alpha", "fork"];
    const calls: string[] = [];
    let release: (() => void) | null = null;
    const slow: CompletionProvider = {
      id: "slow",
      provide: (context) => {
        calls.push(context.prefix);
        if (context.prefix === "f") {
          // Hang the first fetch until the test releases it.
          return new Promise<CompletionResult>((resolve) => {
            release = () => resolve({ items: items(["fork"]) });
          });
        }
        return { items: items(corpus.filter((word) => word.startsWith(context.prefix))) };
      },
    };
    const h = makeHarness([slow]);

    h.insert("f"); // fetch@"f" hangs
    h.deleteBack(); // prefix is "" before the session ever loaded
    release!();
    await settle();
    // The landed "f" results were already stale — one follow-up fetch at "".
    expect(calls).toEqual(["f", ""]);
    expect(h.labels()).toEqual(["alpha", "fork"]);
  });
});
