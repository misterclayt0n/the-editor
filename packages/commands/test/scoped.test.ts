import { describe, expect, it } from "@effect/vitest";
import { Context, Effect, Layer, Option } from "effect";
import { definePackage, toPackageDescriptor } from "@the-editor/extensions";
import { makePackageRuntime } from "@the-editor/kernel";
import {
  collectSlots,
  makeStack,
  makeWorkbench,
  placeInStack,
  registerSurfaceFallback,
  SurfaceKind,
  SurfaceScope,
  type Panel,
  type Workbench,
} from "@the-editor/workbench";
import { CommandNotApplicableError, Commands, makeCommands } from "../src/index.js";

const Editor = SurfaceKind("test/editor");
const Tree = SurfaceKind("test/tree");

class Doc extends Context.Tag("test/Doc")<Doc, { readonly text: string }>() {}

const openFocused = (wb: Workbench.Service, kind: SurfaceKind, context?: Context.Context<never>) =>
  Effect.gen(function* () {
    const panel = yield* wb.createPanel({ kind, ...(context ? { context } : {}) });
    yield* wb.updateArrangement((a) =>
      a.tiles === null
        ? { ...a, tiles: makeStack([panel.id]) }
        : placeInStack(a, collectSlots(a)[0]!.id, panel.id),
    );
    const arrangement = yield* wb.arrangement;
    const slot = collectSlots(arrangement).find(
      (s) => s.kind === "stack" && s.stack.panels.includes(panel.id),
    )!;
    yield* wb.setFocus(slot.id);
    return panel;
  });

const setup = Effect.gen(function* () {
  const runtime = yield* makePackageRuntime;
  const wb = yield* makeWorkbench();
  const commands = yield* makeCommands(runtime.contributionIndex, { workbench: wb });
  return { runtime, wb, commands, layer: Layer.succeed(Commands, commands) };
});

describe("scoped commands", () => {
  it.scoped("surface-scoped command runs when its kind is focused", () =>
    Effect.gen(function* () {
      const { runtime, wb, commands, layer } = yield* setup;
      let ran = 0;
      const pkg = definePackage("pkg", () =>
        Effect.flatMap(Commands, (c) =>
          c.register("editor.only", {
            scope: SurfaceScope.surface(Editor),
            run: () => Effect.sync(() => ran++),
          }),
        ),
      );
      yield* runtime.activateGraph([toPackageDescriptor(pkg, layer)]);

      yield* openFocused(wb, Editor);
      yield* commands.execute("editor.only");
      expect(ran).toBe(1);
    }),
  );

  it.scoped("surface-scoped command is not applicable elsewhere", () =>
    Effect.gen(function* () {
      const { runtime, wb, commands, layer } = yield* setup;
      const pkg = definePackage("pkg", () =>
        Effect.flatMap(Commands, (c) =>
          c.register("editor.only", {
            scope: SurfaceScope.surface(Editor),
            run: () => Effect.void,
          }),
        ),
      );
      yield* runtime.activateGraph([toPackageDescriptor(pkg, layer)]);

      // Nothing focused at all:
      const noFocus = yield* Effect.flip(commands.execute("editor.only"));
      expect(noFocus).toBeInstanceOf(CommandNotApplicableError);

      // A different surface focused:
      yield* openFocused(wb, Tree);
      const wrongKind = yield* Effect.flip(commands.execute("editor.only"));
      expect(wrongKind).toBeInstanceOf(CommandNotApplicableError);
    }),
  );

  it.scoped("handler receives the focused panel's context", () =>
    Effect.gen(function* () {
      const { runtime, wb, commands, layer } = yield* setup;
      let seen: string | undefined;
      const pkg = definePackage("pkg", () =>
        Effect.flatMap(Commands, (c) =>
          c.register("read.doc", {
            scope: SurfaceScope.surface(Editor),
            run: () => Effect.map(Doc, (doc) => void (seen = doc.text)),
          }),
        ),
      );
      yield* runtime.activateGraph([toPackageDescriptor(pkg, layer)]);

      yield* openFocused(
        wb,
        Editor,
        Context.make(Doc, { text: "hello" }) as Context.Context<never>,
      );
      yield* commands.execute("read.doc");
      expect(seen).toBe("hello");
    }),
  );

  it.scoped("executeIn targets an explicit panel regardless of focus", () =>
    Effect.gen(function* () {
      const { runtime, wb, commands, layer } = yield* setup;
      let seen: string | undefined;
      const pkg = definePackage("pkg", () =>
        Effect.flatMap(Commands, (c) =>
          c.register("read.doc", {
            scope: SurfaceScope.surface(Editor),
            run: () => Effect.map(Doc, (doc) => void (seen = doc.text)),
          }),
        ),
      );
      yield* runtime.activateGraph([toPackageDescriptor(pkg, layer)]);

      const editor: Panel = yield* openFocused(
        wb,
        Editor,
        Context.make(Doc, { text: "origin" }) as Context.Context<never>,
      );
      // Focus moves to a tree; execute on behalf of the editor anyway.
      yield* openFocused(wb, Tree);
      yield* commands.executeIn(editor.id, "read.doc");
      expect(seen).toBe("origin");
    }),
  );

  it.scoped("listAvailable filters by the focused surface", () =>
    Effect.gen(function* () {
      const { runtime, wb, commands, layer } = yield* setup;
      const pkg = definePackage("pkg", () =>
        Effect.gen(function* () {
          const c = yield* Commands;
          yield* c.register("anywhere", { run: () => Effect.void });
          yield* c.register("editor.thing", {
            scope: SurfaceScope.surface(Editor),
            run: () => Effect.void,
          });
          yield* c.register("tree.thing", {
            scope: SurfaceScope.surface(Tree),
            run: () => Effect.void,
          });
        }),
      );
      yield* runtime.activateGraph([toPackageDescriptor(pkg, layer)]);

      const unfocused = yield* commands.listAvailable();
      expect(unfocused.map((c) => c.contributionId)).toEqual(["anywhere"]);

      yield* openFocused(wb, Editor);
      const atEditor = yield* commands.listAvailable();
      expect(atEditor.map((c) => c.contributionId).sort()).toEqual(["anywhere", "editor.thing"]);
    }),
  );

  it.scoped("listAvailable includes commands from a registered parent surface", () =>
    Effect.gen(function* () {
      const { runtime, wb, commands, layer } = yield* setup;
      const Editor = SurfaceKind("test/editor");
      const MultiBuffer = SurfaceKind("test/multibuffer");
      registerSurfaceFallback(MultiBuffer, Editor);

      const pkg = definePackage("pkg", () =>
        Effect.flatMap(Commands, (c) =>
          Effect.all([
            c.register("editor.thing", {
              scope: SurfaceScope.surface(Editor),
              palette: true,
              run: () => Effect.void,
            }),
            c.register("mb.thing", {
              scope: SurfaceScope.surface(MultiBuffer),
              palette: true,
              run: () => Effect.void,
            }),
          ]),
        ),
      );
      yield* runtime.activateGraph([toPackageDescriptor(pkg, layer)]);

      yield* openFocused(wb, MultiBuffer);
      const available = yield* commands.listAvailable();
      expect(available.map((c) => c.contributionId).sort()).toEqual(["editor.thing", "mb.thing"]);
    }),
  );

  it.scoped("panel-scoped command applies to that instance only", () =>
    Effect.gen(function* () {
      const { runtime, wb, commands, layer } = yield* setup;
      const first = yield* openFocused(wb, Editor);
      let ran = 0;
      const pkg = definePackage("pkg", () =>
        Effect.flatMap(Commands, (c) =>
          c.register("this.one", {
            scope: SurfaceScope.panel(first.id),
            run: () => Effect.sync(() => ran++),
          }),
        ),
      );
      yield* runtime.activateGraph([toPackageDescriptor(pkg, layer)]);

      yield* commands.execute("this.one");
      expect(ran).toBe(1);

      yield* openFocused(wb, Editor); // second editor instance takes focus
      const error = yield* Effect.flip(commands.execute("this.one"));
      expect(error).toBeInstanceOf(CommandNotApplicableError);
      expect(Option.isSome(yield* wb.focusedPanel)).toBe(true);
    }),
  );
});
