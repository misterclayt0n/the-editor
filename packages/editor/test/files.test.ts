import { Context, Effect, Option } from "effect";
import { describe, expect, test } from "vitest";
import { TextDocument, Transaction } from "@the-editor/engine";
import {
  closeWorkspaceScope,
  commitDocumentTransaction,
  DocumentInvalidated,
  DocumentNotifications,
  Files,
  makeWorkspaceScope,
  payloadsOf,
  FileSyncInvalidated,
  type DocumentScope,
  type EditorNotification,
  type FileEvent,
  type FileStat,
  type FileSystemPort,
} from "../src/index";
import { Stream } from "effect";

/** In-memory file system with manual watcher firing. */
function makeFakePort() {
  const disk = new Map<string, { content: string; mtimeMs: number }>();
  const watchers = new Map<string, Set<(event: FileEvent) => void>>();
  let clock = 1000;

  const port: FileSystemPort.Service = {
    readFile: (path) =>
      disk.has(path)
        ? Effect.succeed(disk.get(path)!.content)
        : Effect.fail(new (Error as never as new (...args: never[]) => never)() as never),
    writeFile: (path, text) =>
      Effect.sync((): FileStat => {
        clock += 10;
        disk.set(path, { content: text, mtimeMs: clock });
        return { mtimeMs: clock, size: text.length };
      }),
    stat: (path) =>
      Effect.succeed(
        disk.has(path)
          ? Option.some({ mtimeMs: disk.get(path)!.mtimeMs, size: disk.get(path)!.content.length })
          : Option.none(),
      ),
    watch: (path, onEvent) =>
      Effect.sync(() => {
        const set = watchers.get(path) ?? new Set();
        set.add(onEvent);
        watchers.set(path, set);
        return () => set.delete(onEvent);
      }),
    pickFile: () => Effect.succeed(Option.none()),
    capabilities: { watch: true },
  };

  return {
    port,
    seed(path: string, content: string) {
      clock += 10;
      disk.set(path, { content, mtimeMs: clock });
    },
    externalWrite(path: string, content: string) {
      clock += 10;
      disk.set(path, { content, mtimeMs: clock });
      for (const listener of watchers.get(path) ?? []) listener({ path, kind: "changed" });
    },
    externalDelete(path: string) {
      disk.delete(path);
      for (const listener of watchers.get(path) ?? []) listener({ path, kind: "deleted" });
    },
    read: (path: string) => disk.get(path)?.content,
  };
}

const textOf = (document: DocumentScope) =>
  Effect.flatMap(
    Effect.succeed(Context.get(document.context as Context.Context<TextDocument>, TextDocument)),
    (text) => text.get,
  ).pipe(Effect.map((buffer) => buffer.sliceString(0, buffer.length)));

const editDocument = (document: DocumentScope, at: number, insert: string) =>
  Effect.gen(function* () {
    const text = Context.get(document.context as Context.Context<TextDocument>, TextDocument);
    const length = (yield* text.get).length;
    const transaction = yield* Transaction.fromReplacements(length, [
      { from: at, to: at, text: insert },
    ]).pipe(Effect.orDie);
    yield* commitDocumentTransaction(transaction).pipe(
      Effect.provide(document.context as Context.Context<any>),
      Effect.orDie,
    );
  });

const settle = Effect.sleep(20);

describe("Files", () => {
  test("openPath loads disk content; reopening returns the same document", async () => {
    const fake = makeFakePort();
    fake.seed("/p/a.ts", "hello\n");
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const workspace = yield* makeWorkspaceScope({ fileSystem: fake.port });
        const files = Context.get(workspace.context as Context.Context<Files>, Files);
        const doc1 = yield* files.openPath("/p/a.ts");
        const doc2 = yield* files.openPath("/p/a.ts");
        const content = yield* textOf(doc1);
        const binding = yield* files.bindingFor(doc1.id);
        yield* closeWorkspaceScope(workspace);
        return { same: doc1.id === doc2.id, content, binding: Option.getOrThrow(binding) };
      }),
    );
    expect(result.same).toBe(true);
    expect(result.content).toBe("hello\n");
    expect(result.binding.status).toBe("clean");
    expect(result.binding.path).toBe("/p/a.ts");
  });

  test("edits mark dirty; save writes and returns to clean", async () => {
    const fake = makeFakePort();
    fake.seed("/p/b.ts", "one\n");
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const workspace = yield* makeWorkspaceScope({ fileSystem: fake.port });
        const files = Context.get(workspace.context as Context.Context<Files>, Files);
        const doc = yield* files.openPath("/p/b.ts");
        yield* editDocument(doc, 0, "EDIT ");
        yield* settle;
        const afterEdit = Option.getOrThrow(yield* files.bindingFor(doc.id));
        yield* files.save(doc.id);
        const afterSave = Option.getOrThrow(yield* files.bindingFor(doc.id));
        const text = Context.get(doc.context as Context.Context<TextDocument>, TextDocument);
        const engineDirty = yield* text.dirty;
        yield* closeWorkspaceScope(workspace);
        return { afterEdit: afterEdit.status, afterSave: afterSave.status, engineDirty };
      }),
    );
    expect(result.afterEdit).toBe("dirty");
    expect(result.afterSave).toBe("clean");
    expect(result.engineDirty).toBe(false);
    expect(fake.read("/p/b.ts")).toBe("EDIT one\n");
  });

  test("external change on a clean document hot-reloads as a minimal transaction", async () => {
    const fake = makeFakePort();
    fake.seed("/p/c.ts", "alpha\nbeta\ngamma\n");
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const workspace = yield* makeWorkspaceScope({ fileSystem: fake.port });
        const files = Context.get(workspace.context as Context.Context<Files>, Files);
        const doc = yield* files.openPath("/p/c.ts");
        const bus = Context.get(
          doc.context as Context.Context<DocumentNotifications>,
          DocumentNotifications,
        );
        const received: EditorNotification[] = [];
        yield* Stream.runForEach(bus.notifications, (n) =>
          Effect.sync(() => received.push(n)),
        ).pipe(Effect.forkDaemon);
        yield* Effect.yieldNow();

        fake.externalWrite("/p/c.ts", "alpha\nBETA!\ngamma\n");
        yield* settle;

        const content = yield* textOf(doc);
        const binding = Option.getOrThrow(yield* files.bindingFor(doc.id));
        const docChanges = received.flatMap((n) => payloadsOf(n, DocumentInvalidated));
        yield* closeWorkspaceScope(workspace);
        return { content, status: binding.status, docChanges };
      }),
    );
    expect(result.content).toBe("alpha\nBETA!\ngamma\n");
    expect(result.status).toBe("clean");
    // Hot reload entered as a transaction with localized change ranges.
    expect(result.docChanges).toHaveLength(1);
    const ranges = result.docChanges[0]!.changes!;
    expect(ranges.length).toBeGreaterThan(0);
    expect(ranges[0]!.toA - ranges[0]!.fromA).toBeLessThan(8);
  });

  test("external change on a dirty document flags conflict; explicit save wins", async () => {
    const fake = makeFakePort();
    fake.seed("/p/d.ts", "base\n");
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const workspace = yield* makeWorkspaceScope({ fileSystem: fake.port });
        const files = Context.get(workspace.context as Context.Context<Files>, Files);
        const doc = yield* files.openPath("/p/d.ts");
        yield* editDocument(doc, 0, "LOCAL ");
        yield* settle;
        fake.externalWrite("/p/d.ts", "REMOTE base\n");
        yield* settle;
        const conflicted = Option.getOrThrow(yield* files.bindingFor(doc.id));
        const bufferUntouched = yield* textOf(doc);
        yield* files.save(doc.id);
        const afterSave = Option.getOrThrow(yield* files.bindingFor(doc.id));
        yield* closeWorkspaceScope(workspace);
        return { conflicted: conflicted.status, bufferUntouched, afterSave: afterSave.status };
      }),
    );
    expect(result.conflicted).toBe("conflict");
    expect(result.bufferUntouched).toBe("LOCAL base\n");
    expect(result.afterSave).toBe("clean");
    expect(fake.read("/p/d.ts")).toBe("LOCAL base\n");
  });

  test("deletion on disk flags missing; sync events ride the document bus", async () => {
    const fake = makeFakePort();
    fake.seed("/p/e.ts", "content\n");
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const workspace = yield* makeWorkspaceScope({ fileSystem: fake.port });
        const files = Context.get(workspace.context as Context.Context<Files>, Files);
        const doc = yield* files.openPath("/p/e.ts");
        const bus = Context.get(
          doc.context as Context.Context<DocumentNotifications>,
          DocumentNotifications,
        );
        const statuses: string[] = [];
        yield* Stream.runForEach(bus.notifications, (n) =>
          Effect.sync(() => {
            for (const payload of payloadsOf(n, FileSyncInvalidated)) statuses.push(payload.status);
          }),
        ).pipe(Effect.forkDaemon);
        yield* Effect.yieldNow();

        fake.externalDelete("/p/e.ts");
        yield* settle;
        const binding = Option.getOrThrow(yield* files.bindingFor(doc.id));
        yield* closeWorkspaceScope(workspace);
        return { status: binding.status, statuses };
      }),
    );
    expect(result.status).toBe("missing");
    expect(result.statuses).toContain("missing");
  });
});
