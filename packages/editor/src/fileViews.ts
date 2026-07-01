import { Context, Effect } from "effect";
import type { FileIoError } from "./files";
import type { DocumentId, EditorScopeId } from "./services";

/** Result of opening a file-backed document into a rendered editor view. */
export type OpenInViewResult = {
  readonly documentId: DocumentId;
  readonly viewId: EditorScopeId;
  /** Workbench panel id, intentionally string-shaped to avoid coupling editor to workbench. */
  readonly panelId: string;
};

/**
 * FileViews is the visible half of file opening.
 *
 * `Files.openPath` owns disk → document binding. `FileViews.openInView` asks the
 * product host to attach that document to a rendered editor tab and focus it.
 * The implementation is host-owned because it includes surface vocabulary,
 * pane creation, layout placement, and focus policy; the tag lives in the public
 * editor package so product packages and user extensions can name the operation.
 */
export class FileViews extends Context.Tag("@the-editor/editor/FileViews")<
  FileViews,
  FileViews.Service
>() {}

export namespace FileViews {
  export interface Service {
    readonly openInView: (path: string) => Effect.Effect<OpenInViewResult, FileIoError>;
  }
}
