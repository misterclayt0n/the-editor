/**
 * The workspace scope and the document registry: the layer above editors.
 *
 * Scope hierarchy (each scope's context EXTENDS its parent's, so an effect
 * provided a view context can reach document and workspace services):
 *
 *   WorkspaceScope   roots, the document set, registers/clipboard,
 *                    workspace notifications, the ActiveEditor cell
 *     DocumentScope  TextDocument, document notifications,
 *                    document-anchored visuals — SHARED by all its views
 *       ViewScope    selection, view notifications — one per pane
 *
 * The split exists because a document is not an editor: splits show one
 * document in two views (shared text and history, separate selections);
 * file IO, language servers, and VCS bind to documents and workspaces, never
 * to panes. Documents outlive views: the registry owns document lifecycle
 * (closing the last view of a dirty document must not destroy it), and the
 * workspace closes everything beneath it on shutdown.
 */

import { Context, Effect, Exit, Option, Ref, Scope } from "effect";
import {
  makeTextDocument,
  NoClipboard,
  Registers,
  TextBuffer,
  TextDocument,
  type ClipboardProvider,
} from "@the-editor/engine";
import { makeStateCell, type StateCell } from "@the-editor/kernel";
import {
  ActiveClipboard,
  ActiveDocumentId,
  ActiveRegisters,
  ActiveWorkspaceId,
  DocumentId,
  WorkspaceId,
  type EditorScopeId,
} from "./services";
import {
  DocumentNotifications,
  makeInvalidationTopic,
  makeNotificationBus,
  WorkspaceNotifications,
  invalidate,
} from "./notifications";
import { ActiveEditor } from "./state";
import { EditorVisuals, makeEditorVisuals } from "./visuals";

let nextWorkspaceId = 1;
let nextDocumentId = 1;

/** Workspace folder roots; a StateCell so products can watch them. */
export class WorkspaceRoots extends Context.Tag("@the-editor/editor/WorkspaceRoots")<
  WorkspaceRoots,
  StateCell<ReadonlyArray<string>>
>() {}

/** Published on the workspace bus when the document set changes. */
export type DocumentsChange = {
  readonly opened?: DocumentId;
  readonly closed?: DocumentId;
};

export const DocumentsChanged = makeInvalidationTopic<DocumentsChange>(
  "@the-editor/editor/workspace/documents",
);

export interface DocumentScope {
  readonly id: DocumentId;
  readonly scope: Scope.CloseableScope;
  /** Workspace context + this document's services. */
  readonly context: Context.Context<any>;
}

export type OpenDocumentInput = {
  readonly id?: string | DocumentId;
  readonly text?: string | TextBuffer;
};

export class DocumentRegistry extends Context.Tag("@the-editor/editor/DocumentRegistry")<
  DocumentRegistry,
  DocumentRegistry.Service
>() {}

export namespace DocumentRegistry {
  export interface Service {
    /** Create a document. Its lifetime is the registry's until `close`. */
    readonly open: (input?: OpenDocumentInput) => Effect.Effect<DocumentScope>;
    readonly get: (id: DocumentId) => Effect.Effect<Option.Option<DocumentScope>>;
    readonly list: Effect.Effect<ReadonlyArray<DocumentScope>>;
    /** Close a document: ends its scope (buses, finalizers) and retracts it. */
    readonly close: (id: DocumentId) => Effect.Effect<void>;
  }
}

export type CreateWorkspaceInput = {
  readonly id?: string | WorkspaceId;
  readonly roots?: ReadonlyArray<string>;
  readonly registers?: Registers;
  readonly clipboard?: ClipboardProvider;
};

export interface WorkspaceScope {
  readonly id: WorkspaceId;
  readonly scope: Scope.CloseableScope;
  readonly context: Context.Context<any>;
}

function workspaceId(id: string | WorkspaceId | undefined): WorkspaceId {
  return typeof id === "string"
    ? WorkspaceId.make(id)
    : (id ?? WorkspaceId.make(`workspace:${nextWorkspaceId++}`));
}

function documentId(id: string | DocumentId | undefined): DocumentId {
  return typeof id === "string"
    ? DocumentId.make(id)
    : (id ?? DocumentId.make(`document:${nextDocumentId++}`));
}

export function makeWorkspaceScope(
  input: CreateWorkspaceInput = {},
): Effect.Effect<WorkspaceScope> {
  return Effect.gen(function* () {
    const id = workspaceId(input.id);
    const scope = yield* Scope.make();
    const bus = yield* Scope.extend(makeNotificationBus(), scope);
    const roots = yield* makeStateCell<ReadonlyArray<string>>(input.roots ?? []);
    const activeEditor = yield* makeStateCell(Option.none<EditorScopeId>());
    const registers = input.registers ?? Registers.create();
    const clipboard = input.clipboard ?? new NoClipboard();

    const documents = yield* Ref.make<ReadonlyMap<DocumentId, DocumentScope>>(new Map());

    // The registry needs the workspace context (documents extend it), and
    // the context needs the registry: tie the knot with a mutable slot that
    // is written before anything can observe it.
    let workspaceContext!: Context.Context<any>;

    const open = (openInput: OpenDocumentInput = {}) =>
      Effect.gen(function* () {
        const docId = documentId(openInput.id);
        const docScope = yield* Scope.make();
        const textDocument = yield* makeTextDocument(openInput.text ?? "").pipe(Effect.orDie);
        const docBus = yield* Scope.extend(makeNotificationBus(), docScope);
        const visuals = yield* makeEditorVisuals(docBus);
        const context = workspaceContext.pipe(
          Context.add(ActiveDocumentId, docId),
          Context.add(TextDocument, textDocument),
          Context.add(DocumentNotifications, docBus),
          Context.add(EditorVisuals, visuals),
        ) as Context.Context<any>;
        const document: DocumentScope = { id: docId, scope: docScope, context };
        yield* Ref.update(documents, (map) => new Map(map).set(docId, document));
        yield* bus.publish([invalidate(DocumentsChanged, { opened: docId })]);
        return document;
      });

    const close = (docId: DocumentId) =>
      Effect.gen(function* () {
        const map = yield* Ref.get(documents);
        const document = map.get(docId);
        if (document === undefined) return;
        yield* Ref.update(documents, (current) => {
          const next = new Map(current);
          next.delete(docId);
          return next;
        });
        yield* Scope.close(document.scope, Exit.void);
        yield* bus.publish([invalidate(DocumentsChanged, { closed: docId })]);
      });

    const registry: DocumentRegistry.Service = {
      open,
      get: (docId) =>
        Ref.get(documents).pipe(Effect.map((map) => Option.fromNullable(map.get(docId)))),
      list: Ref.get(documents).pipe(Effect.map((map) => [...map.values()])),
      close,
    };

    // Workspace shutdown closes every remaining document.
    yield* Scope.addFinalizer(
      scope,
      Ref.get(documents).pipe(
        Effect.flatMap((map) =>
          Effect.forEach([...map.values()], (document) => Scope.close(document.scope, Exit.void), {
            discard: true,
          }),
        ),
      ),
    );

    workspaceContext = Context.empty().pipe(
      Context.add(ActiveWorkspaceId, id),
      Context.add(WorkspaceNotifications, bus),
      Context.add(WorkspaceRoots, roots),
      Context.add(ActiveEditor, activeEditor),
      Context.add(ActiveRegisters, registers),
      Context.add(ActiveClipboard, clipboard),
      Context.add(DocumentRegistry, registry),
    ) as Context.Context<any>;

    return { id, scope, context: workspaceContext } satisfies WorkspaceScope;
  });
}

export function closeWorkspaceScope(workspace: WorkspaceScope): Effect.Effect<void> {
  return Scope.close(workspace.scope, Exit.void);
}

export function provideWorkspaceScope<A, E, R>(
  workspace: WorkspaceScope,
  effect: Effect.Effect<A, E, R>,
): Effect.Effect<A, E> {
  return effect.pipe(Effect.provide(workspace.context as any)) as Effect.Effect<A, E>;
}
