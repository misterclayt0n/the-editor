/**
 * One-shot syntax highlighting for detached code snippets (hover cards,
 * docs, anywhere outside a document). Rides the same synchronous syntax
 * engine documents use: open a transient session under a synthetic
 * filename for the language, pull the runs, close. Snippets are small, so
 * the parse is a sub-millisecond plain function call.
 *
 * Composable on purpose: any UI that has a code string and a markdown
 * fence tag can call this — it has no coupling to hover or intel.
 */

import { createSyntaxPort, type SyntaxPort } from "../platform/syntax";
import { resolveSyntaxStyle } from "../theme/dom";

export type SnippetSpan = { readonly text: string; readonly color?: string };

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

let port: SyntaxPort | undefined;
let portResolved = false;
let counter = 0;

/**
 * Highlight `code` as `languageTag` (a markdown fence tag: "ts", "rust",
 * "typescript"…). Returns spans covering the WHOLE string in order, or
 * null when the language is unknown or no syntax host exists.
 */
export function highlightSnippet(code: string, languageTag: string): SnippetSpan[] | null {
  if (code.length === 0 || code.length > MAX_SNIPPET_CHARS) return null;
  const tag = languageTag.trim().toLowerCase();
  if (tag.length === 0) return null;
  if (!portResolved) {
    port = createSyntaxPort();
    portResolved = true;
  }
  if (port === undefined) return null;
  const ext = FENCE_ALIASES[tag] ?? tag;
  const docId = `the-editor/snippet#${++counter}`;
  const opened = port.open(docId, `snippet.${ext}`, code);
  if (opened === null) return null;
  try {
    port.pump(docId, 5); // small snippets parse synchronously at open
    const runs = port.highlights(docId, 0, code.length);
    if (runs.length === 0) return null;
    const spans: SnippetSpan[] = [];
    let position = 0;
    for (const run of runs) {
      const from = Math.max(position, run.from);
      const to = Math.min(code.length, run.to);
      if (to <= from) continue;
      if (from > position) spans.push({ text: code.slice(position, from) });
      const color = resolveSyntaxStyle(run.kind)?.color;
      spans.push({ text: code.slice(from, to), ...(color !== undefined ? { color } : {}) });
      position = to;
    }
    if (position < code.length) spans.push({ text: code.slice(position) });
    return spans;
  } finally {
    port.close(docId);
  }
}
