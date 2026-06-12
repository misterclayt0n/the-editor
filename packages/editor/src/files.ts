/**
 * Files: documents bound to disk, through a platform port.
 *
 * The design rule that keeps the engine pure: TextDocument never learns
 * about IO. Instead, EXTERNAL CONTENT ENTERS AS ORDINARY TRANSACTIONS —
 * when a watched file changes under a clean document, the new content is
 * diffed against the buffer (engine diffText) and committed through the
 * document commit path. Undo history stays coherent, every view's selection
 * maps through, every extension's RangeSets slide, language servers get
 * exact deltas: hot reload costs a keystroke.
 *
 * - FileSystemPort is the platform seam (Electron IPC, File System Access,
 *   in-memory fakes for tests). The editor package defines only the
 *   interface; whoever boots a workspace injects an implementation, and the
 *   Files service exists only when one is present (capability gating).
 * - Sync states: clean (matches disk), dirty (buffer ahead of disk),
 *   conflict (BOTH changed — never auto-clobbered; an explicit save wins),
 *   missing (deleted on disk). State changes publish FileSyncInvalidated on
 *   the DOCUMENT bus, so any view or product of that document can surface
 *   them.
 * - Our own saves echo back through the watcher; they are recognized by the
 *   recorded post-write mtime and ignored.
 */

import { Context, Effect, Option, Ref } from "effect";
import { diffText, TextDocument } from "@the-editor/engine";
import { DocumentId } from "./services";
import {
  DocumentNotifications,
  invalidate,
  makeInvalidationTopic,
  type NotificationBus,
} from "./notifications";
import { commitDocumentTransaction } from "./commit";
import { EditorVisuals } from "./visuals";
import type { DocumentRegistry, DocumentScope } from "./workspace";

export class FileIoError extends Error {
  constructor(
    readonly path: string,
    readonly operation: "read" | "write" | "stat" | "watch" | "pick",
    override readonly cause?: unknown,
  ) {
    super(`file ${operation} failed for ${JSON.stringify(path)}`);
  }
}

export class UnboundDocumentError extends Error {
  constructor(readonly documentId: DocumentId) {
    super(`document ${JSON.stringify(documentId)} is not bound to a file`);
  }
}

export type FileStat = {
  readonly mtimeMs: number;
  readonly size: number;
};

export type FileEvent = {
  readonly path: string;
  readonly kind: "changed" | "deleted";
};

export type DirEntry = {
  readonly name: string;
  readonly kind: "file" | "directory";
};

export class FileSystemPort extends Context.Tag("@the-editor/editor/FileSystemPort")<
  FileSystemPort,
  FileSystemPort.Service
>() {}

export namespace FileSystemPort {
  export interface Service {
    readonly readFile: (path: string) => Effect.Effect<string, FileIoError>;
    /** Returns the post-write stat so callers can recognize their own echo. */
    readonly writeFile: (path: string, text: string) => Effect.Effect<FileStat, FileIoError>;
    readonly stat: (path: string) => Effect.Effect<Option.Option<FileStat>, FileIoError>;
    /**
     * Start watching one path; returns a disposer. Implementations without
     * native watching may poll (or report watch: false and return a no-op).
     */
    readonly watch: (
      path: string,
      onEvent: (event: FileEvent) => void,
    ) => Effect.Effect<() => void, FileIoError>;
    /** Native open-file dialog where the platform has one. */
    readonly pickFile: () => Effect.Effect<Option.Option<string>, FileIoError>;
    /** Native open-folder dialog. Optional: capability-gated like watch. */
    readonly pickFolder?: () => Effect.Effect<Option.Option<string>, FileIoError>;
    /** Directory listing. Optional: only platforms with real paths have it. */
    readonly readDir?: (
      path: string,
    ) => Effect.Effect<ReadonlyArray<DirEntry>, FileIoError>;
    /**
     * Recursive directory watch (tree/VCS refresh). Optional; event paths
     * are best-effort (some platforms only report "something changed").
     */
    readonly watchDir?: (
      path: string,
      onEvent: (event: FileEvent) => void,
    ) => Effect.Effect<() => void, FileIoError>;
    readonly capabilities: { readonly watch: boolean };
  }
}

export type FileSyncStatus = "clean" | "dirty" | "conflict" | "missing";

export type FileBinding = {
  readonly path: string;
  readonly status: FileSyncStatus;
  readonly mtimeMs: number;
  /** Document version last written to (or read from) disk. */
  readonly savedVersion: number;
};

export type FileSyncInvalidation = {
  readonly path: string;
  readonly status: FileSyncStatus;
};

/** Published on the DOCUMENT bus whenever a binding's sync state changes. */
export const FileSyncInvalidated = makeInvalidationTopic<FileSyncInvalidation>(
  "@the-editor/editor/files/sync",
);

export class Files extends Context.Tag("@the-editor/editor/Files")<Files, Files.Service>() {}

export namespace Files {
  export interface Service {
    /**
     * Open the document for a path (reading it from disk), or return the
     * already-open document bound to it. Starts watching.
     */
    readonly openPath: (path: string) => Effect.Effect<DocumentScope, FileIoError>;
    /** Write the buffer to disk; clears dirty/conflict. Explicit saves win. */
    readonly save: (id: DocumentId) => Effect.Effect<void, FileIoError | UnboundDocumentError>;
    readonly bindingFor: (id: DocumentId) => Effect.Effect<Option.Option<FileBinding>>;
    /** Stop watching and forget the binding (the document stays open). */
    readonly unbind: (id: DocumentId) => Effect.Effect<void>;
  }
}

type BindingState = FileBinding & {
  readonly document: DocumentScope;
  readonly unwatch: () => void;
};

const documentBusOf = (document: DocumentScope): NotificationBus =>
  Context.get(document.context as Context.Context<DocumentNotifications>, DocumentNotifications);

const textDocumentOf = (document: DocumentScope): TextDocument.Service =>
  Context.get(document.context as Context.Context<TextDocument>, TextDocument);

export const makeFiles = (
  port: FileSystemPort.Service,
  registry: DocumentRegistry.Service,
): Effect.Effect<Files.Service> =>
  Effect.gen(function* () {
    const bindings = yield* Ref.make<ReadonlyMap<DocumentId, BindingState>>(new Map());
    const byPath = yield* Ref.make<ReadonlyMap<string, DocumentId>>(new Map());

    const publishStatus = (state: BindingState) =>
      documentBusOf(state.document).publish([
        invalidate(FileSyncInvalidated, { path: state.path, status: state.status }),
      ]);

    const setBinding = (id: DocumentId, next: BindingState, notify: boolean) =>
      Ref.update(bindings, (map) => new Map(map).set(id, next)).pipe(
        Effect.andThen(notify ? publishStatus(next) : Effect.void),
      );

    /** Disk changed: reload clean documents as a transaction, flag dirty ones. */
    const onDiskChanged = (id: DocumentId) =>
      Effect.gen(function* () {
        const state = (yield* Ref.get(bindings)).get(id);
        if (state === undefined) return;
        const statNow = yield* port.stat(state.path).pipe(Effect.orElseSucceed(Option.none));
        if (Option.isNone(statNow)) {
          yield* setBinding(id, { ...state, status: "missing" }, true);
          return;
        }
        // Our own save echoing back through the watcher.
        if (statNow.value.mtimeMs <= state.mtimeMs) return;
        const text = textDocumentOf(state.document);
        const version = yield* text.version;
        const bufferIsDirty = version !== state.savedVersion;
        if (bufferIsDirty) {
          yield* setBinding(
            id,
            { ...state, status: "conflict", mtimeMs: statNow.value.mtimeMs },
            true,
          );
          return;
        }
        const diskContent = yield* port.readFile(state.path);
        const buffer = yield* text.get;
        const transaction = yield* diffText(buffer, diskContent).pipe(Effect.orDie);
        if (transaction.changes.desc().ranges.length > 0) {
          // Record the post-commit version BEFORE publishing, so the dirty
          // tracker on the document bus never sees a transient mismatch.
          yield* setBinding(id, { ...state, savedVersion: version + 1 }, false);
          yield* commitDocumentTransaction(transaction).pipe(
            Effect.provide(state.document.context as Context.Context<any>),
            Effect.orDie,
          );
        }
        yield* text.markSaved;
        const newVersion = yield* text.version;
        const current = (yield* Ref.get(bindings)).get(id);
        if (current === undefined) return;
        yield* setBinding(
          id,
          { ...current, status: "clean", mtimeMs: statNow.value.mtimeMs, savedVersion: newVersion },
          true,
        );
      });

    const onDiskEvent = (id: DocumentId, event: FileEvent) => {
      const effect =
        event.kind === "deleted"
          ? Effect.gen(function* () {
              const state = (yield* Ref.get(bindings)).get(id);
              if (state === undefined) return;
              yield* setBinding(id, { ...state, status: "missing" }, true);
            })
          : onDiskChanged(id);
      Effect.runFork(effect);
    };

    const openPath = (path: string) =>
      Effect.gen(function* () {
        const existing = (yield* Ref.get(byPath)).get(path);
        if (existing !== undefined) {
          const state = (yield* Ref.get(bindings)).get(existing);
          if (state !== undefined) return state.document;
        }
        const content = yield* port.readFile(path);
        const statResult = yield* port.stat(path);
        const mtimeMs = Option.isSome(statResult) ? statResult.value.mtimeMs : Date.now();
        const document = yield* registry.open({ id: `file:${path}`, text: content });
        const text = textDocumentOf(document);
        yield* text.markSaved;
        const savedVersion = yield* text.version;
        const unwatch = yield* port.watch(path, (event) => onDiskEvent(document.id, event));
        const state: BindingState = {
          path,
          status: "clean",
          mtimeMs,
          savedVersion,
          document,
          unwatch,
        };
        yield* Ref.update(bindings, (map) => new Map(map).set(document.id, state));
        yield* Ref.update(byPath, (map) => new Map(map).set(path, document.id));
        return document;
      });

    const save = (id: DocumentId) =>
      Effect.gen(function* () {
        const state = (yield* Ref.get(bindings)).get(id);
        if (state === undefined) return yield* Effect.fail(new UnboundDocumentError(id));
        const text = textDocumentOf(state.document);
        const buffer = yield* text.get;
        const stat = yield* port.writeFile(state.path, buffer.sliceString(0, buffer.length));
        yield* text.markSaved;
        const version = yield* text.version;
        yield* setBinding(
          id,
          { ...state, status: "clean", mtimeMs: stat.mtimeMs, savedVersion: version },
          true,
        );
      });

    // clean/dirty is DERIVED from version vs savedVersion at query time;
    // only conflict/missing are stored states. No tracking fiber needed.
    const bindingFor = (id: DocumentId) =>
      Effect.gen(function* () {
        const state = (yield* Ref.get(bindings)).get(id);
        if (state === undefined) return Option.none<FileBinding>();
        let status: FileSyncStatus = state.status;
        if (status === "clean" || status === "dirty") {
          const version = yield* textDocumentOf(state.document).version;
          status = version === state.savedVersion ? "clean" : "dirty";
        }
        return Option.some({
          path: state.path,
          status,
          mtimeMs: state.mtimeMs,
          savedVersion: state.savedVersion,
        });
      });

    const unbind = (id: DocumentId) =>
      Effect.gen(function* () {
        const state = (yield* Ref.get(bindings)).get(id);
        if (state === undefined) return;
        state.unwatch();
        yield* Ref.update(bindings, (map) => {
          const next = new Map(map);
          next.delete(id);
          return next;
        });
        yield* Ref.update(byPath, (map) => {
          const next = new Map(map);
          next.delete(state.path);
          return next;
        });
      });

    return { openPath, save, bindingFor, unbind } satisfies Files.Service;
  });
