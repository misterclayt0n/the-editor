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

export type DocumentId = string & { readonly DocumentId: unique symbol };

export class InvalidDocumentIdError extends EditorServiceError {
  constructor(readonly value: string) {
    super(`invalid document id ${JSON.stringify(value)}`);
  }
}

export const DocumentId = {
  make(value: string): DocumentId {
    if (value.length === 0) throw new InvalidDocumentIdError(value);
    return value as DocumentId;
  },
};

export class ActiveDocumentId extends Context.Tag("@the-editor/editor/ActiveDocumentId")<
  ActiveDocumentId,
  DocumentId
>() {}

export type WorkspaceId = string & { readonly WorkspaceId: unique symbol };

export class InvalidWorkspaceIdError extends EditorServiceError {
  constructor(readonly value: string) {
    super(`invalid workspace id ${JSON.stringify(value)}`);
  }
}

export const WorkspaceId = {
  make(value: string): WorkspaceId {
    if (value.length === 0) throw new InvalidWorkspaceIdError(value);
    return value as WorkspaceId;
  },
};

export class ActiveWorkspaceId extends Context.Tag("@the-editor/editor/ActiveWorkspaceId")<
  ActiveWorkspaceId,
  WorkspaceId
>() {}

export class ActiveRegisters extends Context.Tag("@the-editor/editor/ActiveRegisters")<
  ActiveRegisters,
  Registers
>() {}

export class ActiveClipboard extends Context.Tag("@the-editor/editor/ActiveClipboard")<
  ActiveClipboard,
  ClipboardProvider
>() {}
