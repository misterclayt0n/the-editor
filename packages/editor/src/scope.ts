import { Context, Effect, Exit, Scope } from "effect";
import {
  makeSelectionState,
  makeTextDocument,
  NoClipboard,
  Registers,
  Selection,
  SelectionState,
  TextBuffer,
  TextDocument,
  type ClipboardProvider,
} from "@the-editor/engine";
import {
  ActiveClipboard,
  ActiveEditorScopeId,
  ActiveRegisters,
  EditorScopeId,
  type EditorScopeId as EditorScopeIdType,
} from "./services";
import { EditorNotifications, makeEditorNotifications } from "./notifications";
import { EditorVisuals, makeEditorVisuals } from "./visuals";

let nextScopeId = 1;

export type CreateEditorScopeInput = {
  readonly id?: string | EditorScopeIdType;
  readonly text?: string | TextBuffer;
  readonly selection?: Selection;
  readonly clipboard?: ClipboardProvider;
  readonly registers?: Registers;
};

export interface EditorScope {
  readonly id: EditorScopeIdType;
  readonly scope: Scope.CloseableScope;
  readonly context: Context.Context<any>;
}

function inputId(id: string | EditorScopeIdType | undefined): EditorScopeIdType {
  return typeof id === "string"
    ? EditorScopeId.make(id)
    : (id ?? EditorScopeId.make(`editor:${nextScopeId++}`));
}

export function makeEditorScope(input: CreateEditorScopeInput = {}): Effect.Effect<EditorScope> {
  return Effect.gen(function* () {
    const id = inputId(input.id);
    const scope = yield* Scope.make();
    const textDocument = yield* makeTextDocument(input.text ?? "").pipe(Effect.orDie);
    const selectionState = yield* makeSelectionState({
      selection: input.selection ?? Selection.point(0),
    });
    const registers = input.registers ?? Registers.create();
    const clipboard = input.clipboard ?? new NoClipboard();
    // The notification bus is per editor scope: its PubSub shuts down (ending
    // all consumer streams) when the scope closes.
    const notifications = yield* Scope.extend(makeEditorNotifications(), scope);
    const visuals = yield* makeEditorVisuals(notifications);
    const context = Context.empty().pipe(
      Context.add(ActiveEditorScopeId, id),
      Context.add(TextDocument, textDocument),
      Context.add(SelectionState, selectionState),
      Context.add(ActiveRegisters, registers),
      Context.add(ActiveClipboard, clipboard),
      Context.add(EditorNotifications, notifications),
      Context.add(EditorVisuals, visuals),
    ) as Context.Context<any>;
    return { id, scope, context } satisfies EditorScope;
  });
}

export function closeEditorScope(editor: EditorScope): Effect.Effect<void> {
  return Scope.close(editor.scope, Exit.void);
}

export function provideEditorScope<A, E, R>(
  editor: EditorScope,
  effect: Effect.Effect<A, E, R>,
): Effect.Effect<A, E> {
  return effect.pipe(Effect.provide(editor.context as any)) as Effect.Effect<A, E>;
}
