/**
 * The extension loader: turns TypeScript source text into a running
 * `ExtensionPackage`, the same shape `definePackage` produces for first-party
 * product packages. There is no separate "user extension API" — a package
 * loaded from `~/.the-editor/extensions/foo.ts` and `apps/web/src/product/
 * files.ts` are the same kind of thing, wired the same way, through
 * `toPackageDescriptor` + `PackageRuntime`.
 *
 * Two concerns, kept apart:
 *
 * - Compiling TypeScript to something `new Function` can run. This is a pure
 *   transform (source text in, JS text out) with no filesystem or bundler
 *   knowledge, so it is capability-shaped: `ExtensionTranspiler` is a real
 *   public Context.Tag, not a Host escape hatch, because ANY package could
 *   have a legitimate reason to compile-and-run a snippet (a REPL, a scratch
 *   evaluator), not just the extension loader.
 * - Evaluating compiled code against a fixed set of importable modules
 *   ("virtual modules", mirroring pi's jiti alias/virtualModules split). An
 *   extension can only `import` what the host explicitly hands it — this is
 *   the actual security/compat boundary, not a sandbox, just a fixed surface.
 *
 * Discovery (which files exist) and trust (should this file run at all) are
 * product concerns and live in apps/web, not here — this module only turns
 * one already-read source string into one validated package.
 */

import { Context, Effect } from "effect";
import type { ExtensionPackage } from "./definePackage.js";

/** Compiles one TypeScript source file to CommonJS-shaped JavaScript. */
export interface ExtensionTranspiler {
  readonly transform: (source: string, filename: string) => Promise<string>;
}

export class ExtensionTranspilerTag extends Context.Tag(
  "@the-editor/extensions/ExtensionTranspiler",
)<ExtensionTranspilerTag, ExtensionTranspiler>() {}

export interface CreateEsbuildTranspilerOptions {
  /**
   * The `esbuild-wasm` binary's URL, resolved by the host's bundler (e.g. a
   * Vite `?url` import). Required in a browser/Electron-renderer context;
   * omit under plain Node (tests), where `esbuild-wasm`'s Node build resolves
   * its own wasm binary from disk.
   */
  readonly wasmURL?: string | URL;
}

/**
 * esbuild-wasm's WASM runtime is realm-global: `initialize()` may run at most
 * ONCE per JS realm — a second call throws `Cannot call "initialize" more
 * than once`. So the init is memoized here at MODULE scope, not in a per-
 * instance closure. Every `createEsbuildTranspiler()` call, every Shell, and
 * every React StrictMode / HMR remount share this one initialization; the
 * first caller's `wasmURL` wins (a second init is impossible anyway, so later
 * URLs would be ignored regardless).
 */
let esbuildStarting: Promise<typeof import("esbuild-wasm")> | undefined;

function startEsbuild(
  wasmURL: string | URL | undefined,
): Promise<typeof import("esbuild-wasm")> {
  if (esbuildStarting === undefined) {
    esbuildStarting = import("esbuild-wasm").then(async (esbuild) => {
      await esbuild.initialize(wasmURL !== undefined ? { wasmURL } : {});
      return esbuild;
    });
    esbuildStarting.catch(() => {
      esbuildStarting = undefined; // transient init failure: let a later call retry
    });
  }
  return esbuildStarting;
}

/**
 * The default transpiler: `esbuild-wasm`, which is the same package under
 * both Node (tests, `main` build) and the browser/Electron renderer
 * (`browser` build) — one implementation, not a platform fork. The instance
 * is a thin, stateless wrapper: the realm-global init above is the only
 * shared state, so constructing many of these is free and safe.
 */
export function createEsbuildTranspiler(
  options: CreateEsbuildTranspilerOptions = {},
): ExtensionTranspiler {
  return {
    transform: async (source, filename) => {
      const esbuild = await startEsbuild(options.wasmURL);
      const result = await esbuild.transform(source, {
        // `.tsx` compiles JSX to the NATIVE DOM builder, not React:
        // `h`/`Fragment` from @the-editor/extensions (see h.ts). An extension
        // opts in per file with the extension + an `import { h, Fragment }`.
        loader: filename.endsWith(".tsx") ? "tsx" : "ts",
        jsxFactory: "h",
        jsxFragment: "Fragment",
        format: "cjs",
        target: "es2022",
        sourcefile: filename,
      });
      return result.code;
    },
  };
}

export class ExtensionLoadError extends Error {
  constructor(
    readonly path: string,
    reason: string,
    options?: { readonly cause?: unknown },
  ) {
    super(`extension "${path}": ${reason}`, options);
    this.name = "ExtensionLoadError";
  }
}

function isExtensionPackage(value: unknown): value is ExtensionPackage<never> {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { id?: unknown }).id === "string" &&
    typeof (value as { activate?: unknown }).activate === "function"
  );
}

export interface LoadExtensionPackageOptions {
  readonly transpiler: ExtensionTranspiler;
  /**
   * The fixed set of modules an extension may `import`. Bare specifiers not
   * in this map fail loudly at evaluation time — this is the whole import
   * surface an extension gets, deliberately closed rather than resolved
   * against a real module graph.
   */
  readonly virtualModules: Readonly<Record<string, unknown>>;
}

/**
 * Compile + evaluate one TypeScript source file against the fixed virtual
 * module surface and return its `default` export (unknown — callers validate
 * the shape). Shared by the extension loader and the config loader: both are
 * "a `.ts` file that `export default`s a value," they just validate different
 * shapes. Never throws for authored problems — everything is an
 * `ExtensionLoadError`.
 */
export function evaluateModuleDefault(
  source: string,
  path: string,
  options: LoadExtensionPackageOptions,
): Effect.Effect<unknown, ExtensionLoadError> {
  return Effect.gen(function* () {
    const code = yield* Effect.tryPromise({
      try: () => options.transpiler.transform(source, path),
      catch: (cause) => new ExtensionLoadError(path, "failed to compile", { cause }),
    });

    const moduleObject = { exports: {} as Record<string, unknown> };
    const requireVirtual = (specifier: string): unknown => {
      if (Object.hasOwn(options.virtualModules, specifier)) {
        return options.virtualModules[specifier];
      }
      throw new Error(`import not available to extensions: "${specifier}"`);
    };

    yield* Effect.try({
      try: () => {
        const run = new Function("module", "exports", "require", code);
        run(moduleObject, moduleObject.exports, requireVirtual);
      },
      catch: (cause) => new ExtensionLoadError(path, "failed to run", { cause }),
    });

    return (moduleObject.exports as { default?: unknown }).default;
  });
}

/**
 * Compile + evaluate one extension source file and validate its default
 * export is an `ExtensionPackage` (i.e. `export default definePackage(...)`).
 * Never throws for extension-authored problems — every failure mode reports
 * through `ExtensionLoadError` so a caller can isolate one bad file instead
 * of aborting a whole discovery pass.
 */
export function loadExtensionPackage(
  source: string,
  path: string,
  options: LoadExtensionPackageOptions,
): Effect.Effect<ExtensionPackage<never>, ExtensionLoadError> {
  return Effect.gen(function* () {
    const candidate = yield* evaluateModuleDefault(source, path, options);
    if (!isExtensionPackage(candidate)) {
      return yield* Effect.fail(
        new ExtensionLoadError(
          path,
          'must "export default" the result of definePackage(...)',
        ),
      );
    }
    return candidate;
  });
}
