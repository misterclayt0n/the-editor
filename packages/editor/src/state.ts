/**
 * Editor-owned observable state. The generic StateCell mechanism lives in
 * `@the-editor/kernel`; this module only mints the editor-specific cells.
 */

import { Context, Layer, Option } from "effect";
import { makeStateCell, type StateCell } from "@the-editor/kernel";
import type { EditorScopeId } from "./services";

/**
 * Workspace-level "which editor scope has focus" state. This lives *above*
 * editor scopes (it refers to them), so it is provided as a layer to the
 * workspace context rather than created inside makeEditorScope.
 */
export class ActiveEditor extends Context.Tag("@the-editor/editor/ActiveEditor")<
  ActiveEditor,
  StateCell<Option.Option<EditorScopeId>>
>() {}

export const ActiveEditorLive: Layer.Layer<ActiveEditor> = Layer.effect(
  ActiveEditor,
  makeStateCell(Option.none<EditorScopeId>()),
);
