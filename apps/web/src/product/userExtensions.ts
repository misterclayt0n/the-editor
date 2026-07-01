/**
 * User extensions: `~/.the-editor/extensions` (global) and
 * `<workspace>/.the-editor/extensions` (project-local, trust-gated) — the
 * editor's equivalent of pi's `~/.pi/agent/extensions` + `.pi/extensions`.
 *
 * There is no bespoke "extension API" here. A file in either directory is a
 * TypeScript module that `export default`s the result of `definePackage(...)`
 * — the exact same shape `apps/web/src/product/*.ts` already uses. Loading
 * one is: read the source (FileSystemPort) -> compile + evaluate it against a
 * fixed set of importable modules (`loadExtensionPackage`, in
 * @the-editor/extensions) -> hand the result to the SAME `PackageRuntime`
 * and the SAME `services` layer every first-party product package already
 * activates through. Whatever a first-party package can reach, a loaded one
 * can reach too, and nothing more.
 *
 * Reload mirrors pi's explicit `/reload` (never file-watch/HMR): re-discover,
 * diff against what's currently active by path, and let the kernel's
 * existing `reloadPackage`/`disposePackage`/`activatePackage` do the actual
 * dispose-then-reactivate work with per-extension failure isolation.
 *
 * `reconcile` is a check-then-act sequence against shared `Map`/`Set` state,
 * and it has THREE independent entry points (boot, the reactive workspace-
 * root subscription, and the `reload` command) that can all fire
 * concurrently. All three funnel through `reloadAll`, which holds a single
 * `Effect.makeSemaphore(1)` permit for the whole discover+reconcile pass —
 * one serialized worker, many producers, no interleaved check-then-act races.
 */

import { Effect, Either, Layer, Option, Stream } from "effect";
import * as EffectNamespace from "effect";
import {
  Attention,
  makeNoticeTopic,
  PackageId,
  PackageRuntimeService,
  type PackageSnapshot,
} from "@the-editor/kernel";
import {
  definePackage,
  loadExtensionPackage,
  toPackageDescriptor,
  type ExtensionPackage,
  type ExtensionTranspiler,
} from "@the-editor/extensions";
import * as ExtensionsNamespace from "@the-editor/extensions";
import { Commands } from "@the-editor/commands";
import * as CommandsNamespace from "@the-editor/commands";
import * as KeymapsNamespace from "@the-editor/keymaps";
import * as WorkbenchNamespace from "@the-editor/workbench";
import * as EditorNamespace from "@the-editor/editor";
import * as EngineNamespace from "@the-editor/engine";
import * as IntelNamespace from "@the-editor/intel";
import * as VcsNamespace from "@the-editor/vcs";
import * as DiffViewNamespace from "@the-editor/diff-view";
import * as ThemeNamespace from "@the-editor/theme";
import * as FontsNamespace from "@the-editor/fonts";
import { FileSystemPort, WorkspaceRoots, type DirEntry } from "@the-editor/editor";
import { SurfaceScope } from "@the-editor/workbench";
import { makeExtensionTrust } from "./extensionTrust";

/** The fixed import surface a user extension gets. Deliberately closed
 *  rather than resolved against a real module graph (see loadModule.ts) —
 *  this is the whole seam, not a sandbox.
 *
 *  It mirrors the import surface first-party product packages already use:
 *  everything here is a package tag whose SERVICE is in the layer extensions
 *  activate through, so "a theme is just an extension" is literally true — a
 *  `~/.the-editor/extensions/my-theme.ts` can `yield* Themes;
 *  themes.registerFamily(...)` exactly like product/themes.ts, and reloads
 *  through the same command for free.
 *
 *  (`@the-editor/lsp` is deliberately NOT here: it's low-level protocol
 *  plumbing, not a registration seam. Language-server contributions use the
 *  public `LanguageServers` registry from `@the-editor/intel`.) */
export function buildExtensionVirtualModules(): Readonly<Record<string, unknown>> {
  return {
    effect: EffectNamespace,
    "@the-editor/extensions": ExtensionsNamespace,
    "@the-editor/commands": CommandsNamespace,
    "@the-editor/keymaps": KeymapsNamespace,
    "@the-editor/workbench": WorkbenchNamespace,
    "@the-editor/editor": EditorNamespace,
    "@the-editor/engine": EngineNamespace,
    "@the-editor/intel": IntelNamespace,
    "@the-editor/vcs": VcsNamespace,
    "@the-editor/diff-view": DiffViewNamespace,
    "@the-editor/theme": ThemeNamespace,
    "@the-editor/fonts": FontsNamespace,
  };
}

/** Any extension lifecycle failure: load (compile/evaluate/id-collision) or
 *  dispose (an extension's own cleanup throwing on unload). Both are equally
 *  the user's business, so both are equally loud. */
const ExtensionLifecycleFailed = makeNoticeTopic<{
  readonly path: string;
  readonly message: string;
}>("@the-editor/web/extension-lifecycle-failed");

type DiscoveredSource = {
  readonly path: string;
  readonly mtimeMs: number;
  readonly size: number;
};

type ActiveEntry = {
  readonly id: PackageId;
  readonly mtimeMs: number;
  readonly size: number;
};

const trimTrailingSlash = (path: string): string => path.replace(/\/+$/, "") || "/";

/** Direct .ts files, plus `<dir>/name/index.ts` for directory-form
 *  extensions. No package.json/npm-dependency tier yet (v1 scope). */
function discoverDir(
  fs: FileSystemPort.Service,
  dir: string,
): Effect.Effect<readonly DiscoveredSource[]> {
  if (fs.readDir === undefined) return Effect.succeed([]);
  const readDir = fs.readDir;
  return Effect.gen(function* () {
    const entries: readonly DirEntry[] = yield* readDir(dir).pipe(Effect.orElseSucceed(() => []));
    const candidates: string[] = [];
    for (const entry of entries) {
      if (entry.name.startsWith(".")) continue;
      if (entry.kind === "file" && entry.name.endsWith(".ts")) {
        candidates.push(`${dir}/${entry.name}`);
      } else if (entry.kind === "directory") {
        candidates.push(`${dir}/${entry.name}/index.ts`);
      }
    }
    const stats = yield* Effect.forEach(
      candidates,
      (path) =>
        fs.stat(path).pipe(
          Effect.map((maybe): DiscoveredSource | null =>
            Option.isSome(maybe)
              ? { path, mtimeMs: maybe.value.mtimeMs, size: maybe.value.size }
              : null,
          ),
          Effect.orElseSucceed(() => null),
        ),
      { concurrency: "unbounded" },
    );
    return stats.filter((entry): entry is DiscoveredSource => entry !== null);
  });
}

export interface MakeUserExtensionsPackageOptions<ExtR> {
  /** The exact layer every first-party product package activates through.
   *  A loaded extension gets this too — no separate, smaller capability set. */
  readonly servicesForExtensions: Layer.Layer<ExtR>;
  readonly transpiler: ExtensionTranspiler;
  readonly virtualModules: Readonly<Record<string, unknown>>;
  /** Absent without an Electron bridge: global discovery capability-gates
   *  off, same as every other host-only port in this app. */
  readonly homeDir: string | undefined;
}

export function makeUserExtensionsPackage<ExtR>(
  options: MakeUserExtensionsPackageOptions<ExtR>,
): ExtensionPackage<
  FileSystemPort | PackageRuntimeService | Attention | Commands | WorkspaceRoots
> {
  return definePackage("the-editor/user-extensions", () =>
    Effect.gen(function* () {
      const fileSystem = yield* FileSystemPort;
      const packages = yield* PackageRuntimeService;
      const attention = yield* Attention;
      const commands = yield* Commands;
      const workspaceRoots = yield* WorkspaceRoots;
      const trust = makeExtensionTrust(fileSystem, attention, options.homeDir);

      const globalActive = new Map<string, ActiveEntry>();
      const projectActive = new Map<string, ActiveEntry>();
      /** Ids this loader activated — the only ones it's allowed to reload or
       *  dispose. A collision with a first-party or foreign id is refused. */
      const ownedIds = new Set<PackageId>();

      const readAndLoad = (path: string) =>
        Effect.gen(function* () {
          const source = yield* fileSystem.readFile(path);
          return yield* loadExtensionPackage(source, path, {
            transpiler: options.transpiler,
            virtualModules: options.virtualModules,
          });
        });

      /** Diff `sources` against `activeMap` and drive the kernel to match:
       *  gone files dispose, changed files reload, new files activate. Every
       *  step is isolated — one bad extension never blocks the rest. Callers
       *  never invoke this directly; it only ever runs under `reloadAll`'s
       *  semaphore, so the check-then-act sequences below are never
       *  interleaved with another reconcile pass. */
      const reconcile = (
        activeMap: Map<string, ActiveEntry>,
        sources: readonly DiscoveredSource[],
      ): Effect.Effect<void> =>
        Effect.gen(function* () {
          const seen = new Set(sources.map((source) => source.path));

          for (const [path, entry] of [...activeMap]) {
            if (seen.has(path)) continue;
            const disposed = yield* packages.disposePackage(entry.id).pipe(Effect.either);
            if (Either.isLeft(disposed)) {
              yield* attention.notify(
                ExtensionLifecycleFailed,
                { path, message: String(disposed.left) },
                { level: "error" },
              );
            }
            activeMap.delete(path);
            ownedIds.delete(entry.id);
          }

          for (const source of sources) {
            const prior = activeMap.get(source.path);
            if (
              prior !== undefined &&
              prior.mtimeMs === source.mtimeMs &&
              prior.size === source.size
            ) {
              continue;
            }

            const loaded = yield* readAndLoad(source.path).pipe(Effect.either);
            if (Either.isLeft(loaded)) {
              yield* attention.notify(
                ExtensionLifecycleFailed,
                { path: source.path, message: loaded.left.message },
                { level: "error" },
              );
              continue; // keep whatever version (if any) was already running
            }
            const pkg = loaded.right;

            if (prior !== undefined && prior.id !== pkg.id) {
              yield* packages.disposePackage(prior.id).pipe(Effect.ignore);
              ownedIds.delete(prior.id);
              activeMap.delete(source.path);
            }

            const existing: PackageSnapshot | undefined = yield* packages.getPackage(pkg.id);
            if (existing !== undefined && existing.status !== "disposed" && !ownedIds.has(pkg.id)) {
              yield* attention.notify(
                ExtensionLifecycleFailed,
                {
                  path: source.path,
                  message: `id "${pkg.id}" is already used by another package — skipped`,
                },
                { level: "error" },
              );
              continue;
            }

            const descriptor = toPackageDescriptor(pkg, options.servicesForExtensions);
            const activation = ownedIds.has(pkg.id)
              ? packages.reloadPackage(pkg.id, descriptor)
              : packages.activatePackage(descriptor);
            const result = yield* activation.pipe(Effect.either);
            if (Either.isLeft(result)) {
              yield* attention.notify(
                ExtensionLifecycleFailed,
                { path: source.path, message: String(result.left) },
                { level: "error" },
              );
            }
            activeMap.set(source.path, { id: pkg.id, mtimeMs: source.mtimeMs, size: source.size });
            ownedIds.add(pkg.id);
          }
        });

      const reloadGlobal = (): Effect.Effect<void> =>
        Effect.gen(function* () {
          if (options.homeDir === undefined) return;
          const dir = `${trimTrailingSlash(options.homeDir)}/.the-editor/extensions`;
          const sources = yield* discoverDir(fileSystem, dir);
          yield* reconcile(globalActive, sources);
        });

      // Reads the CURRENT root from WorkspaceRoots (the single source of
      // truth) at the moment it actually runs, inside `reloadAll`'s lock —
      // never a value threaded in from outside. An earlier version took
      // `root` as a parameter and cached the last-seen value in a
      // `let currentRoot` for the manual-reload command to re-read; that
      // variable was written asynchronously (only once its OWN serialized
      // pass ran) but read at command DISPATCH time, so a manual reload
      // dispatched before the reactive root-change pass had run could still
      // observe the stale `undefined`, and once its turn on the lock came
      // up, wipe out a project extension a moment-earlier pass had
      // legitimately activated. Reading the live cell instead of a shadow
      // copy removes that whole class of stale-read bugs.
      const reloadProject = () =>
        Effect.gen(function* () {
          const roots = yield* workspaceRoots.get;
          const root = roots[0];
          if (root === undefined) {
            yield* reconcile(projectActive, []);
            return;
          }
          const trusted = yield* trust.ensureTrust(root);
          if (!trusted) {
            yield* reconcile(projectActive, []);
            return;
          }
          const dir = `${trimTrailingSlash(root)}/.the-editor/extensions`;
          const sources = yield* discoverDir(fileSystem, dir);
          yield* reconcile(projectActive, sources);
        });

      // One serialized worker for the whole discover+reconcile surface. Boot,
      // the reactive workspace-root subscription, and the `reload` command are
      // three independent producers of "please reconcile now" — without this,
      // their check-then-act sequences inside `reconcile` could interleave.
      // `withPermits(1)` turns "many producers" into "one queue".
      const reloadLock = yield* Effect.makeSemaphore(1);
      const reloadAll = () =>
        reloadLock.withPermits(1)(Effect.all([reloadGlobal(), reloadProject()], { discard: true }));

      // "Reload", not "Reload Extensions": in this editor everything is a
      // package, so a disk theme/keymap/panel is just an extension and
      // reloads through this one command. There is no per-subsystem reload;
      // future disk resources (user config, etc.) join `reloadAll`'s pass.
      yield* commands.register("reload", {
        title: "Reload",
        scope: SurfaceScope.global,
        palette: { group: "Extensions", aliases: ["reload extensions", "reload themes"] },
        // Commands.execute already wraps this in Effect.exit and turns
        // failures into CommandExecutionError; the palette's own
        // Effect.runFork(...).pipe(Effect.catchAll(...)) is the one place
        // that needs to (and does) observe it. No detached Effect.runPromise
        // here — that would only hide defects from both.
        run: () => reloadAll(),
      });

      // The initial global load and every subsequent workspace-root change
      // both funnel through the same reactive subscription: WorkspaceRoots
      // emits its current value immediately (even `[]` at boot) and then on
      // every change, so one forked fiber covers both "load once at boot"
      // and "reload the project half when the folder changes". Forked so
      // this real IO never blocks package activation itself, which the boot
      // sequence runs synchronously.
      yield* Effect.forkScoped(Stream.runForEach(workspaceRoots.changes, () => reloadAll()));
    }),
  );
}
