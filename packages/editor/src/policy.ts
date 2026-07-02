/**
 * Per-document mutation policy. A read-only document rejects view/document
 * transactions at the single commit chokepoint (see commit.ts) — so a
 * provider-backed virtual buffer (a magit-style status view, a generated
 * report) can't be edited by typing; all interaction flows through commands
 * and scoped keymaps instead ("edits become operations"). The provider itself
 * writes through the forced path (`commitDocumentTransaction(tx, { force })`).
 *
 * The tag is optional in a document's context: absent = the default, editable.
 */

import { Context } from "effect";

export interface DocumentPolicyValue {
  readonly readOnly: boolean;
}

export class DocumentPolicy extends Context.Tag("@the-editor/editor/DocumentPolicy")<
  DocumentPolicy,
  DocumentPolicyValue
>() {}
