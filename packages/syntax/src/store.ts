/**
 * Runtime-asset SyntaxResources for Node hosts.
 *
 * Grammars are helix-built shared libraries (`<grammar>.so`, even on macOS)
 * in `<runtimeDir>/grammars/`, loaded through an injected dlopen-based
 * loader (see @the-editor/grammar-loader) — never wasm, never npm grammar
 * packages. A missing or ABI-incompatible library is simply a missing
 * grammar: the document stays unhighlighted.
 *
 * Queries are helix .scm files in `<runtimeDir>/queries/<lang>/<kind>.scm`,
 * with `; inherits:` chains resolved.
 */

import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import {
  readQueryWithInherits,
  queryKindFilename,
  type NativeTreeSitterLanguage,
  type QueryKind,
  type SyntaxResources,
} from "./syntax.js";

/** node-tree-sitter 0.21 accepts language ABI versions in this range. */
const MIN_ABI = 13;
const MAX_ABI = 14;

export type LoadedGrammar = {
  readonly language: unknown;
  readonly abiVersion: number;
};

export type CreateNodeSyntaxResourcesOptions = {
  /** Directory containing grammars/ and queries/ (the runtime dir). */
  readonly runtimeDir: string;
  /** dlopen loader: (sharedLibraryPath, symbol) → wrapped language. */
  readonly loadLanguage: (path: string, symbol: string) => LoadedGrammar;
};

export function createNodeSyntaxResources(
  options: CreateNodeSyntaxResourcesOptions,
): SyntaxResources {
  const languageCache = new Map<string, NativeTreeSitterLanguage | undefined>();
  const queryCache = new Map<string, string | undefined>();

  const rawQuery = (languageId: string, kind: QueryKind): string => {
    const path = join(options.runtimeDir, "queries", languageId, queryKindFilename(kind));
    if (!existsSync(path)) return "";
    return readFileSync(path, "utf8");
  };

  return {
    language: (grammarName) => {
      if (languageCache.has(grammarName)) return languageCache.get(grammarName);
      let language: NativeTreeSitterLanguage | undefined;
      const path = join(options.runtimeDir, "grammars", `${grammarName}.so`);
      if (existsSync(path)) {
        try {
          const symbol = `tree_sitter_${grammarName.replace(/-/g, "_")}`;
          const loaded = options.loadLanguage(path, symbol);
          if (loaded.abiVersion >= MIN_ABI && loaded.abiVersion <= MAX_ABI) {
            language = loaded;
          }
        } catch {
          language = undefined;
        }
      }
      languageCache.set(grammarName, language);
      return language;
    },
    query: (languageId, kind) => {
      const key = `${languageId}:${kind}`;
      if (queryCache.has(key)) return queryCache.get(key);
      const resolved = readQueryWithInherits(languageId, (id) => rawQuery(id, kind));
      const value = resolved.length > 0 ? resolved : undefined;
      queryCache.set(key, value);
      return value;
    },
  };
}
