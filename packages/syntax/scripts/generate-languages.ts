/**
 * Language registry codegen: runtime/languages.toml (vendored from helix) →
 * src/generated/languages.ts, a SyntaxConfiguration value.
 *
 * Helix's file is the source of truth for language IDENTITY — names, scopes,
 * file types, shebangs, injection regexes, comment tokens, indent units,
 * grammar names. Editor-side concerns (language servers, formatters,
 * debuggers, roots) are deliberately not extracted. Run with:
 *
 *   pnpm --filter @the-editor/syntax codegen
 */

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "smol-toml";

const here = dirname(fileURLToPath(import.meta.url));
// Interim: codegen reads helix's languages.toml straight from a checkout
// (override with an argument). Asset distribution gets designed later.
const source = process.argv[2] ?? join(homedir(), "code", "helix", "languages.toml");
const target = join(here, "..", "src", "generated", "languages.ts");
// Second output: the LSP server catalog, consumed by the web app's
// lsp-defaults package (pure data; the web bundle must not import the
// syntax package, which pulls native tree-sitter).
const serversTarget = join(
  here,
  "..",
  "..",
  "..",
  "apps",
  "web",
  "src",
  "generated",
  "languageServers.ts",
);

type HelixLanguageServerRef = string | { readonly name: string };

type HelixLanguageServer = {
  readonly command: string;
  readonly args?: readonly string[];
  readonly config?: unknown;
  readonly environment?: Record<string, string>;
};

type HelixLanguage = {
  readonly name: string;
  readonly scope?: string;
  readonly "language-id"?: string;
  readonly "language-servers"?: ReadonlyArray<HelixLanguageServerRef>;
  readonly roots?: readonly string[];
  readonly "file-types"?: ReadonlyArray<string | { readonly glob: string }>;
  readonly shebangs?: readonly string[];
  readonly "comment-token"?: string;
  readonly "comment-tokens"?: readonly string[] | string;
  readonly "block-comment-tokens"?:
    | { readonly start: string; readonly end: string }
    | ReadonlyArray<{ readonly start: string; readonly end: string }>;
  readonly "injection-regex"?: string;
  readonly grammar?: string;
  readonly "text-width"?: number;
  readonly "soft-wrap"?: { readonly enable?: boolean };
  readonly indent?: { readonly "tab-width": number; readonly unit: string };
  readonly rulers?: readonly number[];
};

const raw = parse(readFileSync(source, "utf8")) as {
  language: HelixLanguage[];
  "language-server": Record<string, HelixLanguageServer>;
};

const asArray = <T>(value: T | readonly T[] | undefined): readonly T[] =>
  value === undefined ? [] : Array.isArray(value) ? value : [value as T];

// Helix allows several languages to claim one bare extension (resolved by
// priority there); our registry treats bare-extension claims as unique, so
// first claim wins and later ones demote to a glob (still matched, after
// exact extensions).
const seenExtensions = new Set<string>();

const languages = raw.language.map((lang) => {
  const fileTypes = (lang["file-types"] ?? []).map((ft) => {
    if (typeof ft !== "string") return ft;
    if (seenExtensions.has(ft)) return { glob: `*.${ft}` };
    seenExtensions.add(ft);
    return ft;
  });
  const commentTokens = [
    ...asArray(lang["comment-token"]),
    ...asArray(lang["comment-tokens"] as string | readonly string[] | undefined),
  ];
  // Two shapes exist in the wild: [{start, end}, ...] and a bare
  // [start, end] string pair (koto).
  const rawBlock = asArray(lang["block-comment-tokens"]) as ReadonlyArray<
    { readonly start: string; readonly end: string } | string
  >;
  const blockCommentTokens =
    rawBlock.length === 2 && rawBlock.every((t) => typeof t === "string")
      ? [{ start: rawBlock[0] as string, end: rawBlock[1] as string }]
      : rawBlock.filter((t): t is { start: string; end: string } => typeof t !== "string");
  return {
    name: lang.name,
    scope: lang.scope ?? `source.${lang.name}`,
    fileTypes,
    ...(lang.shebangs?.length ? { shebangs: lang.shebangs } : {}),
    ...(commentTokens.length ? { commentTokens: [...new Set(commentTokens)] } : {}),
    ...(blockCommentTokens.length ? { blockCommentTokens } : {}),
    ...(lang["injection-regex"] ? { injectionRegex: lang["injection-regex"] } : {}),
    ...(lang.grammar ? { grammar: lang.grammar } : {}),
    ...(lang["text-width"] !== undefined ? { textWidth: lang["text-width"] } : {}),
    ...(lang.indent
      ? { indent: { tabWidth: lang.indent["tab-width"], unit: lang.indent.unit } }
      : {}),
    ...(lang.rulers?.length ? { rulers: lang.rulers } : {}),
  };
});

const banner = `// GENERATED FILE — do not edit.
// Source: helix-editor/helix languages.toml.
// Regenerate: pnpm --filter @the-editor/syntax codegen [path-to-languages.toml]

import type { SyntaxConfiguration } from "../syntax.js";

export const generatedLanguages: SyntaxConfiguration = `;

mkdirSync(dirname(target), { recursive: true });
writeFileSync(target, `${banner}${JSON.stringify({ language: languages }, null, 2)};\n`);
console.log(`generated ${languages.length} languages -> ${target}`);

// ---------------------------------------------------------- LSP catalog

const serverDefinitions = Object.fromEntries(
  Object.entries(raw["language-server"] ?? {}).map(([id, server]) => [
    id,
    {
      command: server.command,
      ...(server.args?.length ? { args: server.args } : {}),
      ...(server.config !== undefined ? { config: server.config } : {}),
      ...(server.environment !== undefined ? { environment: server.environment } : {}),
    },
  ]),
);

const serverLanguages = raw.language
  .map((lang) => {
    const servers = (lang["language-servers"] ?? []).map((ref) =>
      typeof ref === "string" ? ref : ref.name,
    );
    const fileTypes = (lang["file-types"] ?? []).filter(
      (ft): ft is string => typeof ft === "string",
    );
    return {
      name: lang.name,
      languageId: lang["language-id"] ?? lang.name,
      fileTypes,
      ...(lang.roots?.length ? { roots: lang.roots } : {}),
      servers,
    };
  })
  .filter((lang) => lang.servers.length > 0 && lang.fileTypes.length > 0);

const serversBanner = `// GENERATED FILE — do not edit.
// Source: helix-editor/helix languages.toml ([language-server] + per-language
// language-servers). Regenerate: pnpm --filter @the-editor/syntax codegen

export type GeneratedLanguageServer = {
  readonly command: string;
  readonly args?: readonly string[];
  readonly config?: unknown;
  readonly environment?: Readonly<Record<string, string>>;
};

export type GeneratedServerLanguage = {
  readonly name: string;
  /** LSP languageId for didOpen. */
  readonly languageId: string;
  readonly fileTypes: readonly string[];
  /** Workspace root markers. */
  readonly roots?: readonly string[];
  /** Server ids in priority order; the first is the primary. */
  readonly servers: readonly string[];
};

export const generatedLanguageServers: Readonly<Record<string, GeneratedLanguageServer>> = `;

mkdirSync(dirname(serversTarget), { recursive: true });
writeFileSync(
  serversTarget,
  `${serversBanner}${JSON.stringify(serverDefinitions, null, 2)};\n\n` +
    `export const generatedServerLanguages: ReadonlyArray<GeneratedServerLanguage> = ` +
    `${JSON.stringify(serverLanguages, null, 2)};\n`,
);
console.log(
  `generated ${Object.keys(serverDefinitions).length} servers, ${serverLanguages.length} languages -> ${serversTarget}`,
);
