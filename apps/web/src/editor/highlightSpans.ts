/**
 * Detached syntax highlighting shared by every read-only code surface (hover
 * snippets, file preview). The synchronous syntax engine is opened under a
 * synthetic filename, queried once, and closed — no document binding, no
 * watchers. The run→span slicing and per-line splitting live here so snippets
 * and previews cannot drift into two highlighters.
 */

import { createSyntaxPort, type HighlightRun, type SyntaxPort } from "../platform/syntax";
import { resolveSyntaxStyle } from "../theme/dom";

export type CodeSpan = { readonly text: string; readonly color?: string };

/** Slice `text` into contiguous colored spans from ascending highlight runs. */
export function runsToSpans(
  text: string,
  runs: readonly HighlightRun[],
  resolve: (kind: string) => string | undefined,
): CodeSpan[] {
  const spans: CodeSpan[] = [];
  let position = 0;
  for (const run of runs) {
    const from = Math.max(position, run.from);
    const to = Math.min(text.length, run.to);
    if (to <= from) continue;
    if (from > position) spans.push({ text: text.slice(position, from) });
    const color = resolve(run.kind);
    spans.push({ text: text.slice(from, to), ...(color !== undefined ? { color } : {}) });
    position = to;
  }
  if (position < text.length) spans.push({ text: text.slice(position) });
  return spans;
}

/**
 * The sub-range [from, to) of flat spans, colors preserved — for surfaces
 * that wrap PART of a highlighted string in extra structure (signature help
 * nests the active parameter inside an emphasis span without losing the
 * syntax colors underneath it).
 */
export function sliceCodeSpans(
  spans: readonly CodeSpan[],
  from: number,
  to: number,
): CodeSpan[] {
  const out: CodeSpan[] = [];
  let position = 0;
  for (const span of spans) {
    const start = position;
    const end = position + span.text.length;
    position = end;
    if (end <= from) continue;
    if (start >= to) break;
    const text = span.text.slice(Math.max(start, from) - start, Math.min(end, to) - start);
    if (text.length > 0) {
      out.push(span.color !== undefined ? { text, color: span.color } : { text });
    }
  }
  return out;
}

/**
 * Split flat spans at newlines into per-line span arrays. The result always
 * has `text.split("\n").length` entries; the newline char itself is dropped
 * (each line renders as its own block).
 */
export function splitSpansByLine(spans: readonly CodeSpan[]): CodeSpan[][] {
  const lines: CodeSpan[][] = [];
  let current: CodeSpan[] = [];
  for (const span of spans) {
    const parts = span.text.split("\n");
    for (let i = 0; i < parts.length; i++) {
      if (i > 0) {
        lines.push(current);
        current = [];
      }
      const part = parts[i]!;
      if (part.length > 0) {
        current.push(span.color !== undefined ? { text: part, color: span.color } : { text: part });
      }
    }
  }
  lines.push(current);
  return lines;
}

let port: SyntaxPort | undefined;
let portResolved = false;
let counter = 0;

/**
 * Highlight `code` as the language inferred from `ext` (a bare extension like
 * "ts", "rs"). Returns spans covering the whole string, or null when the
 * language is unknown, the text is too large, or no syntax host exists.
 */
/** Total parse budget (open + pump) for a detached highlight. This is the only
 *  highlighter on the UI thread for transient previews, and it runs on every
 *  picker selection change — so it MUST stay bounded: a file that doesn't parse
 *  within the budget renders plain rather than freezing the frame. The editor's
 *  own highlighting is separate and unbounded (it owns the document). */
const DETACHED_OPEN_BUDGET_MS = 10;

export function highlightToSpans(
  code: string,
  ext: string,
  options: { readonly maxChars: number; readonly budgetMs?: number },
): CodeSpan[] | null {
  if (code.length === 0 || code.length > options.maxChars) return null;
  if (!portResolved) {
    port = createSyntaxPort();
    portResolved = true;
  }
  if (port === undefined) return null;
  const docId = `the-editor/highlight#${++counter}`;
  const opened = port.open(docId, `code.${ext}`, code, DETACHED_OPEN_BUDGET_MS);
  if (opened === null) return null;
  try {
    port.pump(docId, options.budgetMs ?? 5);
    const runs = port.highlights(docId, 0, code.length);
    if (runs.length === 0) return null;
    return runsToSpans(code, runs, (kind) => resolveSyntaxStyle(kind)?.color);
  } finally {
    port.close(docId);
  }
}
