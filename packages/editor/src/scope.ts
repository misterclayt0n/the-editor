/**
 * Editor scope: compatibility sugar over the real hierarchy.
 *
 * The actual structure is workspace ⊃ document ⊃ view (see workspace.ts and
 * view.ts). `makeEditorScope` builds a private workspace with one document
 * and one view and hands back the view context — the shape a single
 * standalone editor (and most tests) wants. New code that needs splits,
 * shared documents, or files should talk to the workspace directly.
 */

import { Context, Effect, Exit, Scope } from "effect";
import { Registers, Selection, TextBuffer, type ClipboardProvider } from "@the-editor/engine";
import type { EditorScopeId as EditorScopeIdType } from "./services";
import { closeWorkspaceScope, DocumentRegistry, makeWorkspaceScope } from "./workspace";
import { makeViewScope, type ViewScope } from "./view";
import type { DocumentScope, WorkspaceScope } from "./workspace";

export type CreateEditorScopeInput = {
  readonly id?: string | EditorScopeIdType;
  readonly text?: string | TextBuffer;
  readonly selection?: Selection;
  readonly clipboard?: ClipboardProvider;
  readonly registers?: Registers;
};

export interface EditorScope {
  readonly id: EditorScopeIdType;
  /** Closing this closes the whole private workspace beneath the editor. */
  readonly scope: Scope.CloseableScope;
  /** The view context: view + document + workspace services. */
  readonly context: Context.Context<any>;
  readonly workspace: WorkspaceScope;
  readonly document: DocumentScope;
  readonly view: ViewScope;
}

let nextEditorId = 1;

export function makeEditorScope(input: CreateEditorScopeInput = {}): Effect.Effect<EditorScope> {
  return Effect.gen(function* () {
    const id = input.id ?? `editor:${nextEditorId++}`;
    const workspace = yield* makeWorkspaceScope({
      ...(input.registers ? { registers: input.registers } : {}),
      ...(input.clipboard ? { clipboard: input.clipboard } : {}),
    });
    const registry = Context.get(
      workspace.context as Context.Context<DocumentRegistry>,
      DocumentRegistry,
    );
    const document = yield* registry.open({
      ...(input.text !== undefined ? { text: input.text } : {}),
    });
    const view = yield* makeViewScope(document, {
      id,
      ...(input.selection ? { selection: input.selection } : {}),
    });
    // The view dies with the workspace.
    yield* Scope.addFinalizer(workspace.scope, Scope.close(view.scope, Exit.void));
    return {
      id: view.id,
      scope: workspace.scope,
      context: view.context,
      workspace,
      document,
      view,
    } satisfies EditorScope;
  });
}

export function closeEditorScope(editor: EditorScope): Effect.Effect<void> {
  return closeWorkspaceScope(editor.workspace);
}

export function provideEditorScope<A, E, R>(
  editor: EditorScope,
  effect: Effect.Effect<A, E, R>,
): Effect.Effect<A, E> {
  return effect.pipe(Effect.provide(editor.context as any)) as Effect.Effect<A, E>;
}
