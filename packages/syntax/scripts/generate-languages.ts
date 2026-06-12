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

type HelixLanguage = {
  readonly name: string;
  readonly scope?: string;
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
