import { describe, expect, it } from "@effect/vitest";
import { Effect, Layer } from "effect";
import { definePackage, toPackageDescriptor } from "@the-editor/extensions";
import { makePackageRuntime, PackageId } from "@the-editor/kernel";
import {
  collectSlots,
  makeStack,
  makeWorkbench,
  SurfaceKind,
  SurfaceScope,
  type Workbench,
} from "@the-editor/workbench";
import { InvalidKeysError, Keymaps, makeKeymaps, parseChord } from "../src/index";

const Editor = SurfaceKind("test/editor");

const openFocused = (wb: Workbench.Service) =>
  Effect.gen(function* () {
    const panel = yield* wb.createPanel({ kind: Editor });
    yield* wb.updateArrangement((a) => ({ ...a, tiles: makeStack([panel.id]) }));
    const arrangement = yield* wb.arrangement;
    yield* wb.setFocus(collectSlots(arrangement)[0]!.id);
    return panel;
  });

const setup = Effect.gen(function* () {
  const runtime = yield* makePackageRuntime;
  const wb = yield* makeWorkbench();
  const keymaps = yield* makeKeymaps(runtime.contributionIndex, { workbench: wb });
  return { runtime, wb, keymaps, layer: Layer.succeed(Keymaps, keymaps) };
});

describe("keymaps service", () => {
  it.scoped("handleKey arms and resolves a pending sequence", () =>
    Effect.gen(function* () {
      const { runtime, wb, keymaps, layer } = yield* setup;
      const pkg = definePackage("pkg", () =>
        Effect.flatMap(Keymaps, (k) =>
          k.registerAll([
            { keys: "ctrl+w h", command: "focus.left" },
            { keys: "ctrl+w l", command: "focus.right" },
          ]),
        ),
      );
      yield* runtime.activateGraph([toPackageDescriptor(pkg, layer)]);
      yield* openFocused(wb);

      const first = yield* keymaps.handleKey(parseChord("ctrl+w"));
      expect(first._tag).toBe("pending");
      expect(yield* keymaps.pending).toHaveLength(1);

      const second = yield* keymaps.handleKey(parseChord("h"));
      expect(second).toMatchObject({ _tag: "match", binding: { command: "focus.left" } });
      expect(yield* keymaps.pending).toHaveLength(0);
    }),
  );

  it.scoped("a dead-end sequence resets pending state", () =>
    Effect.gen(function* () {
      const { runtime, keymaps, layer } = yield* setup;
      const pkg = definePackage("pkg", () =>
        Effect.flatMap(Keymaps, (k) => k.register({ keys: "ctrl+w h", command: "focus.left" })),
      );
      yield* runtime.activateGraph([toPackageDescriptor(pkg, layer)]);

      yield* keymaps.handleKey(parseChord("ctrl+w"));
      const dead = yield* keymaps.handleKey(parseChord("z"));
      expect(dead).toMatchObject({ _tag: "none", cancelled: [parseChord("ctrl+w"), parseChord("z")] });
      expect(yield* keymaps.pending).toHaveLength(0);
    }),
  );

  it.scoped("pending prefixes reset when input facts change", () =>
    Effect.gen(function* () {
      const { runtime, keymaps, layer } = yield* setup;
      const pkg = definePackage("pkg", () =>
        Effect.flatMap(Keymaps, (k) =>
          k.register({
            keys: "g d",
            command: "goto.definition",
            when: { "input.mode": "normal" },
          }),
        ),
      );
      yield* runtime.activateGraph([toPackageDescriptor(pkg, layer)]);

      const first = yield* keymaps.handleKey(parseChord("g"), {
        facts: { "input.mode": "normal" },
      });
      expect(first._tag).toBe("pending");
      const changed = yield* keymaps.handleKey(parseChord("d"), {
        facts: { "input.mode": "insert" },
      });
      expect(changed).toEqual({ _tag: "none" });
      expect(yield* keymaps.pending).toHaveLength(0);
    }),
  );

  it.scoped("platform-aware key definitions resolve for the configured platform", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const wb = yield* makeWorkbench();
      const keymaps = yield* makeKeymaps(runtime.contributionIndex, { workbench: wb, platform: "mac" });
      const layer = Layer.succeed(Keymaps, keymaps);
      const pkg = definePackage("pkg", () =>
        Effect.flatMap(Keymaps, (k) =>
          k.register({
            keys: { primary: "ctrl+p", mac: "meta+p" },
            command: "quick.open",
          }),
        ),
      );
      yield* runtime.activateGraph([toPackageDescriptor(pkg, layer)]);

      expect(yield* keymaps.handleKey(parseChord("ctrl+p"))).toEqual({ _tag: "none" });
      expect(yield* keymaps.handleKey(parseChord("meta+p"))).toMatchObject({
        _tag: "match",
        binding: { command: "quick.open" },
      });
    }),
  );

  it.scoped("kind-scoped bindings resolve only with matching focus", () =>
    Effect.gen(function* () {
      const { runtime, wb, keymaps, layer } = yield* setup;
      const pkg = definePackage("pkg", () =>
        Effect.flatMap(Keymaps, (k) =>
          k.register({ keys: "j", command: "editor.down", scope: SurfaceScope.surface(Editor) }),
        ),
      );
      yield* runtime.activateGraph([toPackageDescriptor(pkg, layer)]);

      const before = yield* keymaps.handleKey(parseChord("j"));
      expect(before).toEqual({ _tag: "none" });

      yield* openFocused(wb);
      const after = yield* keymaps.handleKey(parseChord("j"));
      expect(after).toMatchObject({ _tag: "match", binding: { command: "editor.down" } });
    }),
  );

  it.scoped("invalid keys fail registration with a typed error", () =>
    Effect.gen(function* () {
      const { runtime, layer } = yield* setup;
      const pkg = definePackage("pkg", () =>
        Effect.flatMap(Keymaps, (k) => k.register({ keys: "ctrl+", command: "broken" })),
      );
      const error = yield* Effect.flip(runtime.activateGraph([toPackageDescriptor(pkg, layer)]));
      expect(String((error as { cause?: unknown }).cause ?? error)).toContain("InvalidKeys");
      void InvalidKeysError;
    }),
  );

  it.scoped("package reload swaps bindings atomically", () =>
    Effect.gen(function* () {
      const { runtime, keymaps, layer } = yield* setup;
      const descriptor = (command: string) =>
        toPackageDescriptor(
          definePackage("pkg", () =>
            Effect.flatMap(Keymaps, (k) => k.register({ keys: "ctrl+k", command })),
          ),
          layer,
        );
      yield* runtime.activateGraph([descriptor("old")]);
      yield* runtime.reloadPackage(PackageId("pkg"), descriptor("new"));

      const result = yield* keymaps.handleKey(parseChord("ctrl+k"));
      expect(result).toMatchObject({ _tag: "match", binding: { command: "new" } });
      expect(yield* keymaps.bindings).toHaveLength(1);
    }),
  );
});
