import { Context, Effect } from "effect";
import type { DocumentId, EditorScopeId } from "./services";

/** A live provider-backed view: content the provider drives + how to close it. */
export interface VirtualViewHandle {
  readonly documentId: DocumentId;
  readonly viewId: EditorScopeId;
  /** Workbench panel id; string-shaped to keep editor decoupled from workbench. */
  readonly panelId: string;
  /** Replace the buffer's whole content (a status refresh, a REPL append). */
  readonly setText: (text: string) => void;
  /** Close the view and dispose the backing document. */
  readonly close: () => void;
}

export interface VirtualViewSpec {
  /** Tab title for the view. */
  readonly title: string;
  readonly initialText?: string;
  /**
   * When true, typing edits the buffer directly. Default (false) is a
   * read-only provider buffer: typing is rejected and all interaction flows
   * through commands + scoped keymaps (magit-style status, diagnostics report),
   * while the provider owns content through `setText`.
   */
  readonly editable?: boolean;
}

/**
 * The visible half of a provider-backed (non-file) document. `DocumentRegistry`
 * already mints unbound in-memory documents; `VirtualViews.open` is the C1
 * equivalent of `FileViews.openInView` for one — it attaches the document to a
 * rendered editor tab, applies the read-only policy, and returns a handle the
 * provider drives. Host-owned (pane creation, placement, focus); the tag is
 * public so product packages and user extensions can open a virtual view.
 */
export class VirtualViews extends Context.Tag("@the-editor/editor/VirtualViews")<
  VirtualViews,
  VirtualViews.Service
>() {}

export namespace VirtualViews {
  export interface Service {
    readonly open: (spec: VirtualViewSpec) => Effect.Effect<VirtualViewHandle>;
  }
}
