import {
  EditorEngine,
  EditorEngineError,
  type CreateModelInput,
  type EditorInstance,
  type EditorModel,
  type EditorPosition,
  type EditorRange,
  type EditorSelection,
  type TextEdit,
} from "@the-editor/editor";
import { Effect, Layer } from "effect";
import * as monaco from "monaco-editor";
import { configureMonacoWorkers } from "./MonacoWorkers.js";

const fail = (operation: string, cause: unknown) => new EditorEngineError({ operation, cause });

const attempt = <A>(operation: string, evaluate: () => A) =>
  Effect.try({ try: evaluate, catch: (cause) => fail(operation, cause) });

const toMonacoRange = (range: EditorRange) =>
  new monaco.Range(range.startLineNumber, range.startColumn, range.endLineNumber, range.endColumn);

const fromSelection = (selection: monaco.Selection): EditorSelection => ({
  startLineNumber: selection.startLineNumber,
  startColumn: selection.startColumn,
  endLineNumber: selection.endLineNumber,
  endColumn: selection.endColumn,
  selectionStartLineNumber: selection.selectionStartLineNumber,
  selectionStartColumn: selection.selectionStartColumn,
  positionLineNumber: selection.positionLineNumber,
  positionColumn: selection.positionColumn,
});

const toSelection = (selection: EditorSelection) =>
  new monaco.Selection(
    selection.selectionStartLineNumber,
    selection.selectionStartColumn,
    selection.positionLineNumber,
    selection.positionColumn,
  );

const makeModel = (model: monaco.editor.ITextModel, input: CreateModelInput): EditorModel => ({
  uri: input.uri,
  languageId: input.languageId,
  getValue: attempt("model.getValue", () => model.getValue()),
  setValue: (value) => attempt("model.setValue", () => model.setValue(value)),
  getLineCount: attempt("model.getLineCount", () => model.getLineCount()),
});

const makeEditor = (editor: monaco.editor.IStandaloneCodeEditor): EditorInstance => ({
  focus: attempt("editor.focus", () => editor.focus()),
  layout: attempt("editor.layout", () => editor.layout()),
  getPosition: attempt("editor.getPosition", () => editor.getPosition() ?? undefined),
  setPosition: (position: EditorPosition) =>
    attempt("editor.setPosition", () => editor.setPosition(position)),
  getSelection: attempt("editor.getSelection", () => {
    const selection = editor.getSelection();
    return selection ? fromSelection(selection) : undefined;
  }),
  setSelection: (selection) =>
    attempt("editor.setSelection", () => editor.setSelection(toSelection(selection))),
  applyEdits: (edits: readonly TextEdit[]) =>
    attempt("editor.applyEdits", () => {
      editor.executeEdits(
        "the-editor",
        edits.map((edit) => ({ range: toMonacoRange(edit.range), text: edit.text })),
      );
    }),
  revealPosition: (position) =>
    attempt("editor.revealPosition", () => editor.revealPosition(position)),
  onDidChangeContent: (listener) =>
    Effect.acquireRelease(
      attempt("editor.onDidChangeModelContent", () => {
        const disposable = editor.onDidChangeModelContent(() => listener());
        return { dispose: attempt("editor.contentListener.dispose", () => disposable.dispose()) };
      }),
      (disposable) => disposable.dispose.pipe(Effect.orDie),
    ),
  onDidChangeSelection: (listener) =>
    Effect.acquireRelease(
      attempt("editor.onDidChangeCursorSelection", () => {
        const disposable = editor.onDidChangeCursorSelection((event) =>
          listener(fromSelection(event.selection)),
        );
        return { dispose: attempt("editor.selectionListener.dispose", () => disposable.dispose()) };
      }),
      (disposable) => disposable.dispose.pipe(Effect.orDie),
    ),
});

export const MonacoEditorEngineLive = Layer.succeed(EditorEngine, {
  createModel: (input) =>
    Effect.acquireRelease(
      attempt("monaco.editor.createModel", () =>
        makeModel(
          monaco.editor.createModel(input.value, input.languageId, monaco.Uri.parse(input.uri)),
          input,
        ),
      ),
      (model) =>
        attempt("model.dispose", () => {
          const monacoModel = monaco.editor.getModel(monaco.Uri.parse(model.uri));
          monacoModel?.dispose();
        }).pipe(Effect.orDie),
    ),
  mount: (input) =>
    Effect.acquireRelease(
      attempt("monaco.editor.create", () => {
        monaco.editor.setTheme(input.theme ?? "vs-dark");
        const model = monaco.editor.getModel(monaco.Uri.parse(input.model.uri));
        if (!model) throw new Error(`Monaco model not found: ${input.model.uri}`);
        const editor = monaco.editor.create(input.element, {
          model,
          automaticLayout: true,
          theme: input.theme ?? "vs-dark",
          minimap: { enabled: false },
        });
        return [makeEditor(editor), editor] as const;
      }),
      ([, editor]) => attempt("editor.dispose", () => editor.dispose()).pipe(Effect.orDie),
    ).pipe(Effect.map(([editor]) => editor)),
  setTheme: (theme) => attempt("monaco.editor.setTheme", () => monaco.editor.setTheme(theme)),
});

export const configureMonacoEditorEngine = Effect.sync(configureMonacoWorkers);
