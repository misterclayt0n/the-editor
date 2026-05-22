import type { Effect, Scope } from "effect";
import { Data } from "effect";

export type EditorUri = string;
export type EditorLanguageId = string;

export interface EditorPosition {
  readonly lineNumber: number;
  readonly column: number;
}

export interface EditorRange {
  readonly startLineNumber: number;
  readonly startColumn: number;
  readonly endLineNumber: number;
  readonly endColumn: number;
}

export interface EditorSelection extends EditorRange {
  readonly selectionStartLineNumber: number;
  readonly selectionStartColumn: number;
  readonly positionLineNumber: number;
  readonly positionColumn: number;
}

export interface TextEdit {
  readonly range: EditorRange;
  readonly text: string;
}

export interface CreateModelInput {
  readonly uri: EditorUri;
  readonly languageId: EditorLanguageId;
  readonly value: string;
}

export interface MountEditorInput {
  readonly element: HTMLElement;
  readonly model: EditorModel;
  readonly theme?: EditorTheme;
}

export type EditorTheme = "vs" | "vs-dark" | "hc-black" | string;

export interface EditorDisposable {
  readonly dispose: Effect.Effect<void, EditorEngineError>;
}

export interface EditorModel {
  readonly uri: EditorUri;
  readonly languageId: EditorLanguageId;
  readonly getValue: Effect.Effect<string, EditorEngineError>;
  readonly setValue: (value: string) => Effect.Effect<void, EditorEngineError>;
  readonly getLineCount: Effect.Effect<number, EditorEngineError>;
}

export interface EditorInstance {
  readonly focus: Effect.Effect<void, EditorEngineError>;
  readonly layout: Effect.Effect<void, EditorEngineError>;
  readonly getPosition: Effect.Effect<EditorPosition | undefined, EditorEngineError>;
  readonly setPosition: (position: EditorPosition) => Effect.Effect<void, EditorEngineError>;
  readonly getSelection: Effect.Effect<EditorSelection | undefined, EditorEngineError>;
  readonly setSelection: (selection: EditorSelection) => Effect.Effect<void, EditorEngineError>;
  readonly applyEdits: (edits: readonly TextEdit[]) => Effect.Effect<void, EditorEngineError>;
  readonly revealPosition: (position: EditorPosition) => Effect.Effect<void, EditorEngineError>;
  readonly onDidChangeContent: (
    listener: () => void,
  ) => Effect.Effect<EditorDisposable, EditorEngineError, Scope.Scope>;
  readonly onDidChangeSelection: (
    listener: (selection: EditorSelection | undefined) => void,
  ) => Effect.Effect<EditorDisposable, EditorEngineError, Scope.Scope>;
}

export class EditorEngineError extends Data.TaggedError("EditorEngineError")<{
  readonly operation: string;
  readonly cause: unknown;
}> {}
