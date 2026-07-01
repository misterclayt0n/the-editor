/**
 * Language server DEFINITIONS as a registry: which command serves which
 * file types. The defaults below are an ordinary product package
 * contribution — a user package registers its own server through the SAME
 * interface (and retraction follows package scope, like every registry in
 * the editor). The LSP manager consults the registry lazily, so servers
 * registered later still win new documents.
 */

import { Effect } from "effect";
import { definePackage } from "@the-editor/extensions";
import { LanguageServers, type LanguageServerDefinition } from "@the-editor/intel";
import { generatedLanguageServers, generatedServerLanguages } from "../generated/languageServers";

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
