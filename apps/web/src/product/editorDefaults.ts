/**
 * The default editor product package.
 *
 * This is deliberately an ordinary package: it mints the editor surface kind,
 * registers the editing/movement commands (kind-scoped — they exist wherever
 * an editor is focused, structurally unavailable elsewhere), and binds the
 * default keys. A user package activated later can shadow any of it.
 */

import { Effect, Option } from "effect";
import { definePackage } from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import { inputWhen } from "@the-editor/input";
import { Keymaps } from "@the-editor/keymaps";
import { SurfaceKind, SurfaceScope } from "@the-editor/workbench";
import {
  commitTransaction,
  editorSnapshot,
  EditorVisuals,
  setSelection,
  ViewWrapDisplay,
  type EditorSnapshot,
} from "@the-editor/editor";
import {
  deleteBackward,
  deleteForward,
  insertNewline,
  insertText,
  moveHorizontal,
  moveLineEdge,
  moveVertical,
  type CommandResult,
  type VerticalWrapContext,
} from "../editor/commands";

export const EditorSurface = SurfaceKind("the-editor/editor");

const editorScope = SurfaceScope.surface(EditorSurface);
const acceptsText = inputWhen({ acceptsText: true });

/**
 * Apply a pure command result through the focused panel's view context.
 * All mutation flows through the editor package's commit path — product
 * code never touches buses or visuals directly.
 */
const applyResult = (result: CommandResult | undefined) =>
  Effect.gen(function* () {
    if (!result) return;
    if (result.transaction) {
      yield* commitTransaction(result.transaction, result.selection).pipe(Effect.orDie);
    } else {
      yield* setSelection(result.selection);
    }
  });

const editorCommand = (f: (snapshot: EditorSnapshot, args: unknown) => CommandResult | undefined) =>
  ({
    scope: editorScope,
    run: ({ args }: { readonly args: unknown }) =>
      Effect.flatMap(editorSnapshot(), (snapshot) => applyResult(f(snapshot, args))),
  }) as const;

/**
 * cursor.up/down move by VISUAL rows under soft wrap: the renderer
 * publishes its live wrap display into the view scope (ViewWrapDisplay);
 * the command reads it plus the document's inlay hints (both occupy wrap
 * width) so it computes the exact breaks the renderer painted.
 */
const verticalCommand = (direction: -1 | 1) =>
  ({
    scope: editorScope,
    run: ({ args }: { readonly args: unknown }) =>
      Effect.gen(function* () {
        const snapshot = yield* editorSnapshot();
        const displayCell = yield* Effect.serviceOption(ViewWrapDisplay);
        const display = Option.isSome(displayCell) ? displayCell.value.get() : null;
        let wrap: VerticalWrapContext | null = null;
        if (display !== null) {
          const visuals = yield* Effect.serviceOption(EditorVisuals);
          const visualsSnapshot = Option.isSome(visuals) ? yield* visuals.value.snapshot : null;
          wrap = {
            ...display,
            virtualTextBetween: (from, to) =>
              visualsSnapshot?.virtualTextBetween(from, to) ?? [],
          };
        }
        yield* applyResult(moveVertical(snapshot, direction, extend(args), wrap));
      }),
  }) as const;

const extend = (args: unknown): boolean =>
  typeof args === "object" && args !== null && (args as { extend?: boolean }).extend === true;

const text = (args: unknown): string =>
  typeof args === "object" && args !== null && typeof (args as { text?: string }).text === "string"
    ? (args as { text: string }).text
    : "";

export const editorDefaults = definePackage("the-editor/editor-defaults", () =>
  Effect.gen(function* () {
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;

    yield* commands.register("editor.insertText", {
      title: "Insert Text",
      ...editorCommand((snapshot, args) => insertText(snapshot, text(args))),
    });
    yield* commands.register("editor.newline", {
      title: "Insert Newline",
      ...editorCommand((snapshot) => insertNewline(snapshot)),
    });
    yield* commands.register("editor.deleteBackward", {
      title: "Delete Backward",
      ...editorCommand((snapshot) => deleteBackward(snapshot)),
    });
    yield* commands.register("editor.deleteForward", {
      title: "Delete Forward",
      ...editorCommand((snapshot) => deleteForward(snapshot)),
    });
    yield* commands.register("cursor.left", {
      title: "Move Caret Left",
      ...editorCommand((snapshot, args) => moveHorizontal(snapshot, -1, extend(args))),
    });
    yield* commands.register("cursor.right", {
      title: "Move Caret Right",
      ...editorCommand((snapshot, args) => moveHorizontal(snapshot, 1, extend(args))),
    });
    yield* commands.register("cursor.up", {
      title: "Move Caret Up",
      ...verticalCommand(-1),
    });
    yield* commands.register("cursor.down", {
      title: "Move Caret Down",
      ...verticalCommand(1),
    });
    yield* commands.register("cursor.lineStart", {
      title: "Move to Line Start",
      ...editorCommand((snapshot, args) => moveLineEdge(snapshot, "start", extend(args))),
    });
    yield* commands.register("cursor.lineEnd", {
      title: "Move to Line End",
      ...editorCommand((snapshot, args) => moveLineEdge(snapshot, "end", extend(args))),
    });

    yield* keymaps.registerAll([
      { keys: "enter", command: "editor.newline", scope: editorScope, when: acceptsText },
      {
        keys: "backspace",
        command: "editor.deleteBackward",
        scope: editorScope,
        when: acceptsText,
      },
      { keys: "delete", command: "editor.deleteForward", scope: editorScope, when: acceptsText },
      {
        keys: "tab",
        command: "editor.insertText",
        args: { text: "\t" },
        scope: editorScope,
        when: acceptsText,
      },
      { keys: "arrowleft", command: "cursor.left", scope: editorScope },
      { keys: "arrowright", command: "cursor.right", scope: editorScope },
      { keys: "arrowup", command: "cursor.up", scope: editorScope },
      { keys: "arrowdown", command: "cursor.down", scope: editorScope },
      {
        keys: "shift+arrowleft",
        command: "cursor.left",
        args: { extend: true },
        scope: editorScope,
      },
      {
        keys: "shift+arrowright",
        command: "cursor.right",
        args: { extend: true },
        scope: editorScope,
      },
      {
        keys: "shift+arrowup",
        command: "cursor.up",
        args: { extend: true },
        scope: editorScope,
      },
      {
        keys: "shift+arrowdown",
        command: "cursor.down",
        args: { extend: true },
        scope: editorScope,
      },
      { keys: "home", command: "cursor.lineStart", scope: editorScope },
      { keys: "end", command: "cursor.lineEnd", scope: editorScope },
      {
        keys: "shift+home",
        command: "cursor.lineStart",
        args: { extend: true },
        scope: editorScope,
      },
      {
        keys: "shift+end",
        command: "cursor.lineEnd",
        args: { extend: true },
        scope: editorScope,
      },
    ]);
  }),
);
