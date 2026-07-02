import { describe, expect, it } from "@effect/vitest";
import { Effect, Layer, Ref } from "effect";
import { makeAttention, makePackageRuntime } from "@the-editor/kernel";
import { definePackage, type EditorConfig } from "@the-editor/extensions";
import * as ExtensionsNamespace from "@the-editor/extensions";
import { FileIoError, type FileSystemPort } from "@the-editor/editor";
import { makeDefaultPackagesController, makeReadEditorConfig } from "./defaultPackages";
import type { DefaultPackageEntry } from "./manifest";
import type { ExtensionTrust } from "./extensionTrust";

/** A marker package: logs +id on activate, -id on dispose (scope close). */
const marker = (id: string, log: Ref.Ref<string[]>) =>
  definePackage(id, () =>
    Effect.acquireRelease(
      Ref.update(log, (l) => [...l, `+${id}`]),
      () => Ref.update(log, (l) => [...l, `-${id}`]),
    ).pipe(Effect.asVoid),
  );

const entries = (log: Ref.Ref<string[]>): readonly DefaultPackageEntry[] => [
  { pkg: marker("core", log), required: true },
  { pkg: marker("a", log), required: false },
  { pkg: marker("b", log), required: false },
];

describe("makeDefaultPackagesController", () => {
  it.scoped("boot activates the effective set: a disabled swappable never loads", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const attention = yield* makeAttention();
      const log = yield* Ref.make<string[]>([]);
      const controller = makeDefaultPackagesController({
        entries: entries(log),
        services: Layer.empty,
        packages: runtime,
        attention,
        readConfig: () => Effect.succeed<EditorConfig>({ disable: ["a"] }),
      });

      const descriptors = yield* controller.initialDescriptors();
      yield* runtime.activateGraph(descriptors);

      // "a" was dropped from the manifest — never activated.
      expect(yield* Ref.get(log)).toEqual(["+core", "+b"]);
    }),
  );

  it.scoped("a config disabling a REQUIRED package is ignored (foundation stays)", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const attention = yield* makeAttention();
      const log = yield* Ref.make<string[]>([]);
      const controller = makeDefaultPackagesController({
        entries: entries(log),
        services: Layer.empty,
        packages: runtime,
        attention,
        readConfig: () => Effect.succeed<EditorConfig>({ disable: ["core", "b"] }),
      });

      const descriptors = yield* controller.initialDescriptors();
      yield* runtime.activateGraph(descriptors);

      // "core" is required → kept; "b" swappable → dropped.
      expect(yield* Ref.get(log)).toEqual(["+core", "+a"]);
    }),
  );

  it.scoped("reconcile disposes a newly-disabled default and re-activates a re-enabled one", () =>
    Effect.gen(function* () {
      const runtime = yield* makePackageRuntime;
      const attention = yield* makeAttention();
      const log = yield* Ref.make<string[]>([]);
      const disabled = yield* Ref.make<readonly string[]>([]);
      const controller = makeDefaultPackagesController({
        entries: entries(log),
        services: Layer.empty,
        packages: runtime,
        attention,
        readConfig: () => Ref.get(disabled).pipe(Effect.map((d) => ({ disable: d }))),
      });

      yield* runtime.activateGraph(yield* controller.initialDescriptors());
      expect(yield* Ref.get(log)).toEqual(["+core", "+a", "+b"]);

      // Config now disables "a" → reload reconcile disposes it.
      yield* Ref.set(disabled, ["a"]);
      yield* controller.reconcile();
      expect(yield* Ref.get(log)).toEqual(["+core", "+a", "+b", "-a"]);

      // Config re-enables "a" → reconcile brings it back.
      yield* Ref.set(disabled, []);
      yield* controller.reconcile();
      expect(yield* Ref.get(log)).toEqual(["+core", "+a", "+b", "-a", "+a"]);
    }),
  );
});

// Config reading only calls readFile; a partial stub is enough.
const configFs = (files: Record<string, string>): FileSystemPort.Service =>
  ({
    readFile: (path: string) =>
      path in files
        ? Effect.succeed(files[path]!)
        : Effect.fail(new FileIoError(path, "read", "not found")),
  }) as unknown as FileSystemPort.Service;

const configSource = (ids: readonly string[]): string =>
  `module.exports.default = require("@the-editor/extensions").defineConfig(${JSON.stringify({
    disable: ids,
  })});`;

const identityTranspiler = { transform: async (source: string) => source };
const configModules = { "@the-editor/extensions": ExtensionsNamespace };
const fakeTrust = (trusted: boolean): ExtensionTrust => ({
  ensureTrust: () => Effect.succeed(trusted),
});

describe("makeReadEditorConfig", () => {
  it.scoped("reads the global config's disable list", () =>
    Effect.gen(function* () {
      const attention = yield* makeAttention();
      const read = makeReadEditorConfig({
        fileSystem: configFs({ "/home/u/.the-editor/config.ts": configSource(["a"]) }),
        transpiler: identityTranspiler,
        virtualModules: configModules,
        attention,
        homeDir: "/home/u",
        currentRoot: () => Effect.succeed(undefined),
        trust: fakeTrust(true),
      });
      expect((yield* read()).disable).toEqual(["a"]);
    }),
  );

  it.scoped("merges a trusted project config over the global one", () =>
    Effect.gen(function* () {
      const attention = yield* makeAttention();
      const read = makeReadEditorConfig({
        fileSystem: configFs({
          "/home/u/.the-editor/config.ts": configSource(["a"]),
          "/proj/.the-editor/config.ts": configSource(["b"]),
        }),
        transpiler: identityTranspiler,
        virtualModules: configModules,
        attention,
        homeDir: "/home/u",
        currentRoot: () => Effect.succeed("/proj"),
        trust: fakeTrust(true),
      });
      expect([...(yield* read()).disable!].sort()).toEqual(["a", "b"]);
    }),
  );

  it.scoped("ignores an UNtrusted project config (global only)", () =>
    Effect.gen(function* () {
      const attention = yield* makeAttention();
      const read = makeReadEditorConfig({
        fileSystem: configFs({
          "/home/u/.the-editor/config.ts": configSource(["a"]),
          "/proj/.the-editor/config.ts": configSource(["b"]),
        }),
        transpiler: identityTranspiler,
        virtualModules: configModules,
        attention,
        homeDir: "/home/u",
        currentRoot: () => Effect.succeed("/proj"),
        trust: fakeTrust(false),
      });
      expect((yield* read()).disable).toEqual(["a"]);
    }),
  );

  it.scoped("returns an empty disable list when no config file exists", () =>
    Effect.gen(function* () {
      const attention = yield* makeAttention();
      const read = makeReadEditorConfig({
        fileSystem: configFs({}),
        transpiler: identityTranspiler,
        virtualModules: configModules,
        attention,
        homeDir: "/home/u",
        currentRoot: () => Effect.succeed(undefined),
        trust: fakeTrust(true),
      });
      expect((yield* read()).disable).toEqual([]);
    }),
  );
});
