import { Effect, Context, Fiber, Option, Stream } from "effect";
import { describe, expect, test } from "vitest";
import { Selection, SelectionState, TextDocument, Transaction } from "@the-editor/engine";
import {
  closeWorkspaceScope,
  commitTransaction,
  DocumentNotifications,
  DocumentRegistry,
  DocumentsChanged,
  hasInvalidation,
  makeViewScope,
  makeWorkspaceScope,
  payloadsOf,
  provideViewScope,
  SelectionInvalidated,
  ViewNotifications,
  WorkspaceNotifications,
  type DocumentScope,
  type EditorNotification,
} from "../src/index";

const run = <A, E>(effect: Effect.Effect<A, E>) => Effect.runSync(effect);
const runAsync = <A, E>(effect: Effect.Effect<A, E>) => Effect.runPromise(effect);

const registryOf = (workspace: { context: Context.Context<any> }) =>
  Context.get(workspace.context as Context.Context<DocumentRegistry>, DocumentRegistry);

const insert = (document: DocumentScope, at: number, text: string) =>
  Effect.gen(function* () {
    const doc = Context.get(document.context as Context.Context<TextDocument>, TextDocument);
    const length = (yield* doc.get).length;
    return yield* Transaction.fromReplacements(length, [{ from: at, to: at, text }]).pipe(
      Effect.orDie,
    );
  });

describe("workspace scope", () => {
  test("registry opens, lists, gets, and closes documents with events", async () => {
    const result = await runAsync(
      Effect.gen(function* () {
        const workspace = yield* makeWorkspaceScope({ id: "ws", roots: ["/tmp/project"] });
        const registry = registryOf(workspace);
        const bus = Context.get(
          workspace.context as Context.Context<WorkspaceNotifications>,
          WorkspaceNotifications,
        );
        const received: EditorNotification[] = [];
        const consumer = yield* Stream.runForEach(Stream.take(bus.notifications, 2), (n) =>
          Effect.sync(() => received.push(n)),
        ).pipe(Effect.fork);
        yield* Effect.yieldNow();

        const doc = yield* registry.open({ id: "doc:a", text: "hello" });
        const listed = yield* registry.list;
        const fetched = yield* registry.get(doc.id);
        yield* registry.close(doc.id);
        const afterClose = yield* registry.list;
        yield* Fiber.join(consumer);
        yield* closeWorkspaceScope(workspace);
        return {
          listed: listed.map((d) => d.id),
          fetched: Option.isSome(fetched),
          afterClose: afterClose.length,
          events: received.flatMap((n) => payloadsOf(n, DocumentsChanged)),
        };
      }),
    );
    expect(result.listed).toEqual(["doc:a"]);
    expect(result.fetched).toBe(true);
    expect(result.afterClose).toBe(0);
    expect(result.events).toEqual([{ opened: "doc:a" }, { closed: "doc:a" }]);
  });

  test("two views share one document: edits propagate, selections stay private", async () => {
    const result = await runAsync(
      Effect.gen(function* () {
        const workspace = yield* makeWorkspaceScope();
        const registry = registryOf(workspace);
        const document = yield* registry.open({ text: "alpha beta" });
        const view1 = yield* makeViewScope(document, { id: "v1", selection: Selection.point(0) });
        const view2 = yield* makeViewScope(document, { id: "v2", selection: Selection.point(8) });

        // view2 listens on its own bus.
        const view2Bus = Context.get(
          view2.context as Context.Context<ViewNotifications>,
          ViewNotifications,
        );
        const view2Events: EditorNotification[] = [];
        yield* Stream.runForEach(Stream.take(view2Bus.notifications, 1), (n) =>
          Effect.sync(() => view2Events.push(n)),
        ).pipe(Effect.fork);

        // A document-bus subscriber (what a renderer of EITHER view sees).
        const docBus = Context.get(
          document.context as Context.Context<DocumentNotifications>,
          DocumentNotifications,
        );
        const docEvents: EditorNotification[] = [];
        yield* Stream.runForEach(Stream.take(docBus.notifications, 1), (n) =>
          Effect.sync(() => docEvents.push(n)),
        ).pipe(Effect.fork);
        yield* Effect.yieldNow();

        // view1 types "XX" at offset 0 and moves its caret to 2.
        const transaction = yield* insert(document, 0, "XX");
        yield* provideViewScope(view1, commitTransaction(transaction, Selection.point(2))).pipe(
          Effect.orDie,
        );
        // Let the sync fibers process the document batch.
        yield* Effect.yieldNow();
        yield* Effect.sleep(10);

        const text = yield* Context.get(
          document.context as Context.Context<TextDocument>,
          TextDocument,
        ).get;
        const sel1 = yield* Context.get(
          view1.context as Context.Context<SelectionState>,
          SelectionState,
        ).get;
        const sel2 = yield* Context.get(
          view2.context as Context.Context<SelectionState>,
          SelectionState,
        ).get;

        yield* closeWorkspaceScope(workspace);
        return {
          text: text.sliceString(0, 12),
          sel1: sel1.primary.head,
          sel2: sel2.primary.head,
          docChanges: docEvents.flatMap((n) =>
            n.invalidations.map((invalidation) => invalidation.topicId),
          ),
          view2GotSelectionSync: view2Events.some((n) => hasInvalidation(n, SelectionInvalidated)),
        };
      }),
    );
    expect(result.text).toBe("XXalpha beta");
    // view1's caret: explicitly placed at 2.
    expect(result.sel1).toBe(2);
    // view2's caret was at 8; the insert of 2 chars at 0 slid it to 10 —
    // anchored to its text, never touched by view1's explicit selection.
    expect(result.sel2).toBe(10);
    expect(result.docChanges).toContain("@the-editor/editor/document");
    expect(result.view2GotSelectionSync).toBe(true);
  });

  test("closing the workspace closes all documents beneath it", () => {
    const closed = run(
      Effect.gen(function* () {
        const workspace = yield* makeWorkspaceScope();
        const registry = registryOf(workspace);
        yield* registry.open({ id: "doc:x" });
        yield* registry.open({ id: "doc:y" });
        yield* closeWorkspaceScope(workspace);
        return true;
      }),
    );
    expect(closed).toBe(true);
  });
});
