/**
 * Language server DEFINITIONS as a registry: which command serves which
 * file types. The defaults below are an ordinary product package
 * contribution — a user package registers its own server through the SAME
 * interface (and retraction follows package scope, like every registry in
 * the editor). The LSP manager consults the registry lazily, so servers
 * registered later still win new documents.
 */

import { Context, Effect, Ref, Scope } from "effect";
import { definePackage } from "@the-editor/extensions";
import {
  generatedLanguageServers,
  generatedServerLanguages,
} from "../generated/languageServers";

export type LanguageServerDefinition = {
  /** Stable id; one client per (id, workspace root). */
  readonly id: string;
  readonly command: string;
  readonly args?: readonly string[];
  /** LSP languageId by file extension (bare, no dot). */
  readonly languages: ReadonlyArray<{
    readonly languageId: string;
    readonly fileTypes: readonly string[];
    /** Position in the language's server list — feature routing order. */
    readonly priority?: number;
    /** Helix only-features: this server serves ONLY these for the language. */
    readonly only?: readonly string[];
    /** Helix except-features: everything BUT these. */
    readonly except?: readonly string[];
  }>;
  /** Files marking the workspace root, nearest-ancestor-wins. */
  readonly rootMarkers?: readonly string[];
  /** Passed verbatim in initialize. */
  readonly initializationOptions?: unknown;
  /** Workspace settings; answers workspace/configuration by section path. */
  readonly settings?: Record<string, unknown>;
};

export class LanguageServers extends Context.Tag("the-editor/web/LanguageServers")<
  LanguageServers,
  {
    /** Scoped: the definition retracts when the contributing scope closes. */
    readonly register: (
      definition: LanguageServerDefinition,
    ) => Effect.Effect<void, never, Scope.Scope>;
    readonly list: Effect.Effect<ReadonlyArray<LanguageServerDefinition>>;
  }
>() {}

export const makeLanguageServers = Effect.gen(function* () {
  const definitions = yield* Ref.make<ReadonlyArray<LanguageServerDefinition>>([]);
  return {
    register: (definition: LanguageServerDefinition) =>
      Effect.acquireRelease(
        Ref.update(definitions, (list) => [...list, definition]),
        () =>
          Ref.update(definitions, (list) => list.filter((entry) => entry !== definition)),
      ).pipe(Effect.asVoid),
    list: Ref.get(definitions),
  } satisfies LanguageServers.Service;
});

export namespace LanguageServers {
  export type Service = Context.Tag.Service<LanguageServers>;
}

/**
 * Stock servers: the ENTIRE helix catalog, generated from languages.toml
 * (188 servers; ~156 languages wired). EVERY server in a language's
 * language-servers list claims its file types — documents attach to all of
 * them, and the list position (priority) decides feature routing, with
 * helix only-/except-features narrowing what each serves. Commands resolve
 * from PATH and absent ones simply fail to spawn with backoff. Helix's
 * per-server `config` doubles as initializationOptions and as the
 * workspace/configuration answer source, matching helix semantics.
 */
export const lspDefaults = definePackage("the-editor/lsp-defaults", () =>
  Effect.gen(function* () {
    const servers = yield* LanguageServers;

    type Group = {
      languages: Array<{
        languageId: string;
        fileTypes: readonly string[];
        priority?: number;
        only?: readonly string[];
        except?: readonly string[];
      }>;
      roots: Set<string>;
    };
    const byServer = new Map<string, Group>();
    for (const language of generatedServerLanguages) {
      language.servers.forEach((server, index) => {
        const group = byServer.get(server.name) ?? { languages: [], roots: new Set<string>() };
        group.languages.push({
          languageId: language.languageId,
          fileTypes: language.fileTypes,
          priority: index,
          ...(server.only !== undefined ? { only: server.only } : {}),
          ...(server.except !== undefined ? { except: server.except } : {}),
        });
        for (const root of language.roots ?? []) group.roots.add(root);
        byServer.set(server.name, group);
      });
    }
    for (const [id, group] of byServer) {
      const spec = generatedLanguageServers[id];
      if (spec === undefined) continue;
      yield* servers.register({
        id,
        command: spec.command,
        ...(spec.args ? { args: spec.args } : {}),
        languages: group.languages,
        rootMarkers: [...group.roots],
        ...(spec.config !== undefined
          ? {
              initializationOptions: spec.config,
              settings: spec.config as Record<string, unknown>,
            }
          : {}),
      });
    }

    // Headless verification hook: tests inject extra definitions before boot.
    const isDev = location.hostname === "localhost" || location.hostname === "127.0.0.1";
    const injected = isDev
      ? (window as unknown as { __testLspServers?: LanguageServerDefinition[] }).__testLspServers
      : undefined;
    for (const definition of injected ?? []) {
      yield* servers.register(definition);
    }
  }),
);
