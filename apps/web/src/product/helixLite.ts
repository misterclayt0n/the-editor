/**
 * A deliberately small Helix-style input profile. It proves that modal editing
 * is package-owned behavior built from public seams: InputProfiles owns mode
 * state, Keymaps owns bindings, Commands own behavior, and the renderer-facing
 * cursor policy is just a view attachment.
 */

import { Context, Effect, Option, Scope, Stream } from "effect";
import { Commands } from "@the-editor/commands";
import { definePackage } from "@the-editor/extensions";
import { InputProfiles, ViewInput, inputWhen, type InputViewTarget } from "@the-editor/input";
import { Keymaps } from "@the-editor/keymaps";
import { Selection, SelectionRange } from "@the-editor/engine";
import {
  ViewCursor,
  editorSnapshot,
  setSelection,
} from "@the-editor/editor";
import { SurfaceKind, SurfaceScope } from "@the-editor/workbench";

const editorScope = SurfaceScope.surface(SurfaceKind("the-editor/editor"));
const normal = inputWhen({ profile: "helix-lite", mode: "normal" });
const insert = inputWhen({ profile: "helix-lite", mode: "insert" });
const select = inputWhen({ profile: "helix-lite", mode: "select" });

const helixContext = (mode: "normal" | "insert" | "select") => ({
  profile: "helix-lite",
  mode,
  textFallback: mode === "insert" ? "insert" as const : "none" as const,
});

const setMode = (mode: "normal" | "insert" | "select") =>
  Effect.gen(function* () {
    const input = yield* ViewInput;
    yield* input.clearContinuations("helix-lite");
    yield* input.set(helixContext(mode));
  });

const attachCursor = (target: InputViewTarget): Effect.Effect<void, never, Scope.Scope> =>
  Effect.gen(function* () {
    const cursor = Context.getOption(target.context as Context.Context<ViewCursor>, ViewCursor);
    if (Option.isNone(cursor)) return;
    const apply = (mode: string | undefined) => {
      if (mode === "insert") cursor.value.setStyle({ shape: "bar", blink: true });
      else if (mode === "select") cursor.value.setStyle({ shape: "underline", blink: true });
      else cursor.value.setStyle({ shape: "block", blink: false });
    };
    apply((yield* target.input.context).mode);
    yield* Stream.runForEach(target.input.changes, (context) =>
      Effect.sync(() => apply(context.mode)),
    ).pipe(Effect.forkScoped, Effect.asVoid);
  });

const findCharForward = (char: string) =>
  Effect.gen(function* () {
    const snapshot = yield* editorSnapshot();
    const head = snapshot.selection.primary.head;
    const line = snapshot.text.lineRangeAt(head);
    const text = snapshot.text.sliceString(Math.min(head + 1, line.to), line.to);
    const relative = text.indexOf(char);
    if (relative < 0) return;
    const target = Math.min(head + 1, line.to) + relative;
    yield* setSelection(Selection.point(target));
  });

export const helixLite = definePackage("the-editor/helix-lite", () =>
  Effect.gen(function* () {
    const profiles = yield* InputProfiles;
    const commands = yield* Commands;
    const keymaps = yield* Keymaps;

    yield* profiles.register({
      id: "helix-lite",
      title: "Helix Lite",
      initialContext: () => helixContext("normal"),
      attachView: attachCursor,
    });

    yield* commands.register("helix.enterNormal", {
      title: "Helix: Normal Mode",
      scope: editorScope,
      run: () => setMode("normal"),
    });
    yield* commands.register("helix.enterInsert", {
      title: "Helix: Insert Mode",
      scope: editorScope,
      run: () => setMode("insert"),
    });
    yield* commands.register("helix.enterSelect", {
      title: "Helix: Select Mode",
      scope: editorScope,
      run: () =>
        Effect.gen(function* () {
          const snapshot = yield* editorSnapshot();
          const head = snapshot.selection.primary.head;
          const anchor = head === snapshot.text.length ? Math.max(0, head - 1) : head;
          const extent = head === snapshot.text.length ? head : Math.min(snapshot.text.length, head + 1);
          yield* setSelection(Selection.create([SelectionRange.create(anchor, extent)]));
          yield* setMode("select");
        }),
    });
    yield* commands.register("helix.findChar", {
      title: "Helix: Find Character",
      scope: editorScope,
      run: () =>
        Effect.gen(function* () {
          const input = yield* ViewInput;
          yield* input.pushContinuation({
            owner: "helix-lite",
            label: "find char",
            onKey: (chord) => {
              if (chord.ctrl || chord.alt || chord.meta || chord.key.length !== 1) {
                return { _tag: "cancel" };
              }
              return {
                _tag: "execute",
                command: "helix.findChar.execute",
                args: { char: chord.key },
              };
            },
          });
        }),
    });
    yield* commands.register("helix.findChar.execute", {
      title: "Helix: Find Character Execute",
      scope: editorScope,
      run: ({ args }) =>
        typeof args === "object" &&
        args !== null &&
        typeof (args as { char?: unknown }).char === "string"
          ? findCharForward((args as { char: string }).char)
          : Effect.void,
    });

    yield* keymaps.registerAll([
      { keys: "i", command: "helix.enterInsert", scope: editorScope, when: normal },
      { keys: "esc", command: "helix.enterNormal", scope: editorScope, when: insert },
      { keys: "esc", command: "helix.enterNormal", scope: editorScope, when: select },
      { keys: "v", command: "helix.enterSelect", scope: editorScope, when: normal },
      { keys: "f", command: "helix.findChar", scope: editorScope, when: normal },

      { keys: "h", command: "cursor.left", scope: editorScope, when: normal },
      { keys: "j", command: "cursor.down", scope: editorScope, when: normal },
      { keys: "k", command: "cursor.up", scope: editorScope, when: normal },
      { keys: "l", command: "cursor.right", scope: editorScope, when: normal },
      { keys: "arrowleft", command: "cursor.left", scope: editorScope, when: normal },
      { keys: "arrowdown", command: "cursor.down", scope: editorScope, when: normal },
      { keys: "arrowup", command: "cursor.up", scope: editorScope, when: normal },
      { keys: "arrowright", command: "cursor.right", scope: editorScope, when: normal },
      { keys: "x", command: "editor.deleteForward", scope: editorScope, when: normal },

      { keys: "h", command: "cursor.left", args: { extend: true }, scope: editorScope, when: select },
      { keys: "j", command: "cursor.down", args: { extend: true }, scope: editorScope, when: select },
      { keys: "k", command: "cursor.up", args: { extend: true }, scope: editorScope, when: select },
      { keys: "l", command: "cursor.right", args: { extend: true }, scope: editorScope, when: select },
      { keys: "arrowleft", command: "cursor.left", args: { extend: true }, scope: editorScope, when: select },
      { keys: "arrowdown", command: "cursor.down", args: { extend: true }, scope: editorScope, when: select },
      { keys: "arrowup", command: "cursor.up", args: { extend: true }, scope: editorScope, when: select },
      { keys: "arrowright", command: "cursor.right", args: { extend: true }, scope: editorScope, when: select },
      { keys: "d", command: "editor.deleteForward", scope: editorScope, when: select },
    ]);
  }),
);
