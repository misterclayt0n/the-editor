/**
 * The default-packages controller: reads user config and drives the kernel so
 * the active first-party set matches the manifest MINUS whatever config
 * disabled. This is the "manifest phase" of config-as-code — it runs before the
 * user-extensions loader (the "contribution phase") so a disabled default never
 * activates, and it reconciles on `reload` so opt-out rides the same explicit
 * refresh everything else does.
 *
 * Package granularity by design: disabling a package drops all its
 * contributions together (never a per-contribution suppression list, which
 * would break single-owner + reload). A `disable` naming a `required` entry is
 * ignored with a notice — the foundation stays.
 */

import { Effect, Either, Option, type Layer } from "effect";
import { Attention, makeNoticeTopic, PackageId, type PackageRuntime } from "@the-editor/kernel";
import {
  loadConfig,
  toPackageDescriptor,
  type EditorConfig,
  type ExtensionTranspiler,
  type InputProfileSetting,
  type TerminalEngineSetting,
} from "@the-editor/extensions";
import { FileSystemPort } from "@the-editor/editor";
import type { DefaultPackageEntry } from "./manifest";
import type { ExtensionTrust } from "./extensionTrust";

/** A user config file failed to compile/evaluate — loud, like a bad extension. */
const ConfigLoadFailed = makeNoticeTopic<{
  readonly path: string;
  readonly message: string;
}>("@the-editor/web/config-load-failed");

/** A config tried to disable a required (foundation) package — ignored. */
const RequiredNotDisablable = makeNoticeTopic<{ readonly id: string }>(
  "@the-editor/web/required-not-disablable",
);

const trimTrailingSlash = (path: string): string => path.replace(/\/+$/, "") || "/";

export interface MakeReadEditorConfigOptions {
  readonly fileSystem: FileSystemPort.Service;
  readonly transpiler: ExtensionTranspiler;
  readonly virtualModules: Readonly<Record<string, unknown>>;
  readonly attention: Attention.Service;
  readonly homeDir: string | undefined;
  /** The live workspace root (project config is read from it, trust-gated). */
  readonly currentRoot: () => Effect.Effect<string | undefined>;
  /** Shared with the extension loader — one trust prompt per root. */
  readonly trust: ExtensionTrust;
}

/**
 * Read the effective config: global `~/.the-editor/config.ts` (implicitly
 * trusted) merged with the project `.the-editor/config.ts` (behind the same
 * trust gate as project extensions). A missing file is not an error; a broken
 * one notifies and is skipped. `disable` lists union.
 */
export function makeReadEditorConfig(
  options: MakeReadEditorConfigOptions,
): () => Effect.Effect<EditorConfig> {
  const { fileSystem, transpiler, virtualModules, attention, homeDir, currentRoot, trust } = options;

  const readOne = (path: string): Effect.Effect<EditorConfig | undefined> =>
    Effect.gen(function* () {
      // stat-first: a missing config file is the COMMON case (most users have
      // none). Reading it directly makes the host fs bridge log a scary ENOENT
      // even though we handle the failure — `stat` reports absence as `None`
      // with no noise (the same reason extension discovery stats candidates).
      const stat = yield* fileSystem.stat(path).pipe(Effect.orElseSucceed(() => Option.none()));
      if (Option.isNone(stat)) return undefined;
      const source = yield* fileSystem.readFile(path).pipe(Effect.either);
      if (Either.isLeft(source)) return undefined; // raced away since stat
      const loaded = yield* loadConfig(source.right, path, { transpiler, virtualModules }).pipe(
        Effect.either,
      );
      if (Either.isLeft(loaded)) {
        yield* attention.notify(
          ConfigLoadFailed,
          { path, message: loaded.left.message },
          { level: "error" },
        );
        return undefined;
      }
      return loaded.right;
    });

  return () =>
    Effect.gen(function* () {
      const disable = new Set<string>();
      let terminalEngine: TerminalEngineSetting | undefined;
      let inputProfile: InputProfileSetting | undefined;
      if (homeDir !== undefined) {
        const global = yield* readOne(`${trimTrailingSlash(homeDir)}/.the-editor/config.ts`);
        for (const id of global?.disable ?? []) disable.add(id);
        terminalEngine = global?.terminal?.engine ?? terminalEngine;
        inputProfile = global?.input?.profile ?? inputProfile;
      }
      const root = yield* currentRoot();
      if (root !== undefined) {
        const trusted = yield* trust.ensureTrust(root).pipe(Effect.orElseSucceed(() => false));
        if (trusted) {
          const project = yield* readOne(`${trimTrailingSlash(root)}/.the-editor/config.ts`);
          for (const id of project?.disable ?? []) disable.add(id);
          // Project config wins over global for scalar settings.
          terminalEngine = project?.terminal?.engine ?? terminalEngine;
          inputProfile = project?.input?.profile ?? inputProfile;
        }
      }
      return {
        disable: [...disable],
        ...(terminalEngine !== undefined ? { terminal: { engine: terminalEngine } } : {}),
        ...(inputProfile !== undefined ? { input: { profile: inputProfile } } : {}),
      };
    });
}

export interface DefaultPackagesController {
  /** Compute the effective set for boot, seed the active set, and return the
   *  descriptors to activate (via the graph, for dependency ordering). */
  readonly initialDescriptors: () => Effect.Effect<ReadonlyArray<ReturnType<typeof toPackageDescriptor>>>;
  /** Re-read config and drive the kernel to match: dispose now-disabled
   *  packages, activate now-enabled ones (manifest order). Runs on `reload`. */
  readonly reconcile: () => Effect.Effect<void>;
}

export interface MakeDefaultPackagesControllerOptions<ExtR> {
  readonly entries: readonly DefaultPackageEntry[];
  readonly services: Layer.Layer<ExtR>;
  readonly packages: PackageRuntime;
  readonly attention: Attention.Service;
  readonly readConfig: () => Effect.Effect<EditorConfig>;
}

export function makeDefaultPackagesController<ExtR>(
  options: MakeDefaultPackagesControllerOptions<ExtR>,
): DefaultPackagesController {
  const { entries, services, packages, attention, readConfig } = options;
  const active = new Set<PackageId>();

  const desiredEntries = (): Effect.Effect<readonly DefaultPackageEntry[]> =>
    Effect.gen(function* () {
      const config = yield* readConfig();
      const disabled = new Set(config.disable ?? []);
      const desired: DefaultPackageEntry[] = [];
      for (const entry of entries) {
        if (!disabled.has(entry.pkg.id)) {
          desired.push(entry);
          continue;
        }
        if (entry.required) {
          // Foundation can't be dropped — keep it, tell the user why.
          yield* attention.notify(
            RequiredNotDisablable,
            { id: entry.pkg.id },
            { level: "warning" },
          );
          desired.push(entry);
        }
        // swappable + disabled → omitted (never loaded)
      }
      return desired;
    });

  return {
    initialDescriptors: () =>
      Effect.gen(function* () {
        const desired = yield* desiredEntries();
        for (const entry of desired) active.add(entry.pkg.id);
        return desired.map((entry) => toPackageDescriptor(entry.pkg, services));
      }),

    reconcile: () =>
      Effect.gen(function* () {
        const desired = yield* desiredEntries();
        const desiredIds = new Set(desired.map((entry) => entry.pkg.id));
        // Newly disabled: dispose (retracts its contributions cleanly).
        for (const id of [...active]) {
          if (desiredIds.has(id)) continue;
          yield* packages.disposePackage(id).pipe(Effect.ignore);
          active.delete(id);
        }
        // Newly enabled: activate in manifest order.
        for (const entry of desired) {
          if (active.has(entry.pkg.id)) continue;
          yield* packages
            .activatePackage(toPackageDescriptor(entry.pkg, services))
            .pipe(Effect.ignore);
          active.add(entry.pkg.id);
        }
      }),
  };
}
