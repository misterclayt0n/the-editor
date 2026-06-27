/**
 * One-shot syntax highlighting for detached code snippets (hover cards, docs,
 * anywhere outside a document). Maps a markdown fence tag to an extension and
 * defers to the shared detached highlighter; the snippet-specific part is only
 * the fence-tag aliasing and the small-snippet size cap.
 */

import { type CodeSpan, highlightToSpans } from "./highlightSpans";

/** Markdown fence tags → file extensions the engine's detector knows. */
const FENCE_ALIASES: Readonly<Record<string, string>> = {
  typescript: "ts",
  typescriptreact: "tsx",
  javascript: "js",
  javascriptreact: "jsx",
  python: "py",
  rust: "rs",
  "c++": "cpp",
  csharp: "cs",
  "c#": "cs",
  golang: "go",
  markdown: "md",
  shell: "sh",
  bash: "sh",
  zsh: "sh",
  shellscript: "sh",
  ruby: "rb",
  kotlin: "kt",
  haskell: "hs",
  erlang: "erl",
  elixir: "ex",
  objective_c: "m",
  "objective-c": "m",
  perl: "pl",
  yaml: "yml",
};

const MAX_SNIPPET_CHARS = 20_000;

/**
 * Highlight `code` as `languageTag` (a markdown fence tag: "ts", "rust",
 * "typescript"…). Returns spans covering the WHOLE string in order, or null
 * when the language is unknown or no syntax host exists.
 */
export function highlightSnippet(code: string, languageTag: string): CodeSpan[] | null {
  const tag = languageTag.trim().toLowerCase();
  if (tag.length === 0) return null;
  const ext = FENCE_ALIASES[tag] ?? tag;
  return highlightToSpans(code, ext, { maxChars: MAX_SNIPPET_CHARS, budgetMs: 5 });
}
