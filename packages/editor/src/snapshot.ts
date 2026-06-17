import { Effect } from "effect";
import { SelectionState, TextDocument, type RenderableText, type Selection } from "@the-editor/engine";
import { ActiveEditorScopeId, type EditorScopeId } from "./services";

export type EditorSnapshot = {
  readonly id: EditorScopeId;
  readonly text: RenderableText;
  readonly version: number;
  readonly dirty: boolean;
  readonly selection: Selection;
};

export function editorSnapshot(): Effect.Effect<
  EditorSnapshot,
  never,
  ActiveEditorScopeId | TextDocument | SelectionState
> {
  return Effect.gen(function* () {
    const id = yield* ActiveEditorScopeId;
    const document = yield* TextDocument;
    const selectionState = yield* SelectionState;
    const doc = yield* document.snapshot;
    const selection = yield* selectionState.get;
    return { id, text: doc.buffer, version: doc.version, dirty: doc.dirty, selection };
  });
}
