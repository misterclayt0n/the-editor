import { Context } from "effect";
import type { ClipboardProvider, Registers } from "@the-editor/engine";

export type EditorScopeId = string & { readonly EditorScopeId: unique symbol };

export class EditorServiceError extends Error {}
export class InvalidEditorScopeIdError extends EditorServiceError {
  constructor(readonly value: string) {
    super(`invalid editor scope id ${JSON.stringify(value)}`);
  }
}

export const EditorScopeId = {
  make(value: string): EditorScopeId {
    if (value.length === 0) throw new InvalidEditorScopeIdError(value);
    return value as EditorScopeId;
  },
};

export class ActiveEditorScopeId extends Context.Tag("@the-editor/editor/ActiveEditorScopeId")<
  ActiveEditorScopeId,
  EditorScopeId
>() {}

export class ActiveRegisters extends Context.Tag("@the-editor/editor/ActiveRegisters")<
  ActiveRegisters,
  Registers
>() {}

export class ActiveClipboard extends Context.Tag("@the-editor/editor/ActiveClipboard")<
  ActiveClipboard,
  ClipboardProvider
>() {}
