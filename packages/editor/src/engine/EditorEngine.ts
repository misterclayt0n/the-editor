import { Context, Effect } from "effect";
import type { Scope } from "effect";
import type {
  CreateModelInput,
  EditorEngineError,
  EditorInstance,
  EditorModel,
  EditorTheme,
  MountEditorInput,
} from "./types.js";

export class EditorEngine extends Context.Tag("@the-editor/editor/EditorEngine")<
  EditorEngine,
  EditorEngine.Service
>() {}

export namespace EditorEngine {
  export interface Service {
    readonly createModel: (
      input: CreateModelInput,
    ) => Effect.Effect<EditorModel, EditorEngineError, Scope.Scope>;
    readonly mount: (
      input: MountEditorInput,
    ) => Effect.Effect<EditorInstance, EditorEngineError, Scope.Scope>;
    readonly setTheme: (theme: EditorTheme) => Effect.Effect<void, EditorEngineError>;
  }
}
