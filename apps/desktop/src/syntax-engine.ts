/**
 * The synchronous syntax engine: the foreground tree lives IN the renderer
 * process (loaded by the preload with sandbox disabled), so highlight
 * queries are ordinary function calls during layout — there is no frame in
 * which the editor can paint without colors it already owns. This is Zed's
 * model without the second thread: out-of-process highlighting always has
 * a round-trip frame, and that frame is exactly the white flash.
 *
 * Parse loop per session (persistent Parser, so timed-out parses RESUME):
 *  - open: small texts parse immediately; big ones return "pending" and
 *    complete via pump() slices (ts_parser_parse resumes after a timeout as
 *    long as the parser isn't reset).
 *  - edit: tree.edit interpolation, then a budgeted incremental reparse —
 *    after a keystroke this virtually always completes inside the budget,
 *    so the very same render pass paints accurate colors.
 *  - highlights: windowed captures on the current tree (accurate, or
 *    interpolated mid-pump — better than nothing, never than wrong-by-much).
 *
 * Offsets are UTF-16 string indices throughout.
 */

import { createRequire } from "node:module";
import { homedir } from "node:os";
import * as path from "node:path";
import Parser from "tree-sitter";
import {
  createNodeSyntaxResources,
  generatedLanguages,
  highlightRunsForTree,
  LanguageRegistry,
  syntaxTextFromString,
  type LanguageId,
} from "@the-editor/syntax";

type Point = { readonly row: number; readonly column: number };

/**
 * Line-start index for O(log lines) offset→point conversion. tree.edit
 * runs on the keystroke critical path: computing points by walking the
 * text from offset 0 (generateSyntaxEdits) costs ~25ms at the bottom of a
 * 12k-line file — visible input lag. Columns are UTF-16 code units: the
 * node binding multiplies every column/index by 2 itself (tree.cc
 * read_byte_count_from_js).
 */
class LineIndex {
  private starts: number[];

  constructor(text: string) {
    this.starts = LineIndex.scan(text);
  }

  private static scan(text: string): number[] {
    const starts = [0];
    for (let i = text.indexOf("\n"); i !== -1; i = text.indexOf("\n", i + 1)) {
      starts.push(i + 1);
    }
    return starts;
  }

  /** Rebuild after an edit batch (one native scan, ~0.3ms at 600KB). */
  reset(text: string): void {
    this.starts = LineIndex.scan(text);
  }

  point(offset: number): Point {
    const starts = this.starts;
    let lo = 0;
    let hi = starts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (starts[mid]! <= offset) lo = mid;
      else hi = mid - 1;
    }
    return { row: lo, column: offset - starts[lo]! };
  }
}

/** Point after appending `text` at `start` (walks the inserted text only). */
const advancePoint = (start: Point, text: string): Point => {
  const lastNewline = text.lastIndexOf("\n");
  if (lastNewline === -1) return { row: start.row, column: start.column + text.length };
  let rows = 0;
  for (let i = text.indexOf("\n"); i !== -1; i = text.indexOf("\n", i + 1)) rows++;
  return { row: start.row + rows, column: text.length - lastNewline - 1 };
};

export type SyntaxEdit = { readonly start: number; readonly end: number; readonly text: string };
export type HighlightRun = { readonly from: number; readonly to: number; readonly kind: string };
export type EngineStatus = "accurate" | "pending" | "inactive";

/** Keystroke budget: an incremental reparse almost always fits. */
const EDIT_PARSE_BUDGET_MS = 2;
/** Texts below this parse synchronously at open (fast anyway). */
const OPEN_SYNC_LIMIT_CHARS = 128_000;
/**
 * Above this size, skip the edit-time reparse attempt entirely: measured on
 * a 600KB rust file, the incremental reparse needs ~3ms and the budget
 * attempt just burns its full 2ms on the keystroke frame before timing
 * out. The off-frame pump finishes the (resumed) parse a frame later;
 * meanwhile queries serve the interpolated tree, which paints correctly.
 */
const EDIT_SYNC_LIMIT_CHARS = 200_000;

type Session = {
  text: string;
  readonly lines: LineIndex;
  readonly languageId: LanguageId;
  readonly language: string;
  readonly parser: Parser;
  tree: Parser.Tree | null;
  /** Tree lags the text (interpolated or still parsing). */
  dirty: boolean;
  /** A timed-out parse is mid-flight in the parser (resumable). */
  resuming: boolean;
};

export interface SyntaxEngine {
  open(docId: string, path: string | undefined, text: string, parseBudgetMs?: number): { language: string } | null;
  edit(docId: string, edits: readonly SyntaxEdit[]): EngineStatus;
  pump(docId: string, budgetMs: number): EngineStatus;
  highlights(docId: string, from: number, to: number): readonly HighlightRun[];
  close(docId: string): void;
}

export function createSyntaxEngine(loadModule: NodeJS.Require): SyntaxEngine {
  const { loadLanguage } = loadModule("@the-editor/grammar-loader") as {
    loadLanguage: (path: string, symbol: string) => { language: unknown; abiVersion: number };
  };
  const runtimeDir =
    process.env["THE_EDITOR_RUNTIME"] ?? path.join(homedir(), ".config", "the-editor", "runtime");
  const registry = LanguageRegistry.create(
    generatedLanguages,
    createNodeSyntaxResources({ runtimeDir, loadLanguage }),
  );

  const sessions = new Map<string, Session>();


  /** One budgeted parse step; resumes a timed-out parse transparently. */
  const parseStep = (session: Session, budgetMs: number): EngineStatus => {
    if (!session.dirty) return "accurate";
    (session.parser as any).setTimeoutMicros(Math.max(1, Math.round(budgetMs * 1000)));
    let next: Parser.Tree | null = null;
    try {
      // Chunked callback input: the binding's plain-string path throws
      // ("Invalid argument" — its whole-rest-slice callback is rejected),
      // and callback parses resume correctly after a timeout.
      const text = session.text;
      next = session.parser.parse(
        (index: number) => text.slice(index, Math.min(text.length, index + 4096)),
        session.tree ?? undefined,
      );
    } catch {
      next = null;
    }
    if (next === null || next === undefined) {
      session.resuming = true;
      return "pending";
    }
    session.tree = next;
    session.dirty = false;
    session.resuming = false;
    return "accurate";
  };

  const abortResume = (session: Session): void => {
    if (!session.resuming) return;
    session.parser.reset();
    session.resuming = false;
  };

  return {
    open(docId, filePath, text, parseBudgetMs) {
      this.close(docId);
      const syntaxText = syntaxTextFromString(text);
      const assignment = registry.detectLanguageForDocument({
        ...(filePath !== undefined ? { path: filePath } : {}),
        text: syntaxText,
      });
      if (assignment === undefined) return null;
      const grammar = registry.grammar(assignment.languageId);
      if (grammar === undefined) return null;
      const parser = new Parser();
      try {
        parser.setLanguage(grammar as any);
      } catch {
        return null;
      }
      const session: Session = {
        text,
        lines: new LineIndex(text),
        languageId: assignment.languageId,
        language: registry.language(assignment.languageId)?.config.name ?? "",
        parser,
        tree: null,
        dirty: true,
        resuming: false,
      };
      sessions.set(docId, session);
      // Compile the highlight query NOW: it costs ~15ms once per language
      // and must never land on the first keystroke-frame query.
      registry.language(assignment.languageId)?.highlightQuery(registry);
      // The editor opens with the full 1000ms budget (snappy file open). The
      // detached preview highlighter passes a small budget so a large file's
      // synchronous parse can't freeze the UI thread on every selection change —
      // it gets whatever highlights complete in the budget, plain text otherwise.
      const openBudget = parseBudgetMs ?? 1000;
      if (text.length <= OPEN_SYNC_LIMIT_CHARS) parseStep(session, openBudget);
      return { language: session.language };
    },

    edit(docId, edits) {
      const session = sessions.get(docId);
      if (session === undefined) return "inactive";
      if (edits.length === 0) return session.dirty ? "pending" : "accurate";
      // A resumable parse holds a snapshot of the OLD text; restart it.
      abortResume(session);
      if (session.tree !== null) {
        for (const e of edits) {
          const startPosition = session.lines.point(e.start);
          session.tree.edit({
            startIndex: e.start,
            oldEndIndex: e.end,
            newEndIndex: e.start + e.text.length,
            startPosition,
            oldEndPosition: session.lines.point(e.end),
            newEndPosition: advancePoint(startPosition, e.text),
          } as any);
        }
      }
      let next = session.text;
      for (let i = edits.length - 1; i >= 0; i--) {
        const e = edits[i]!;
        next = next.slice(0, e.start) + e.text + next.slice(e.end);
      }
      session.text = next;
      session.lines.reset(next);
      session.dirty = true;
      if (session.text.length > EDIT_SYNC_LIMIT_CHARS) return "pending";
      return parseStep(session, EDIT_PARSE_BUDGET_MS);
    },

    pump(docId, budgetMs) {
      const session = sessions.get(docId);
      if (session === undefined) return "inactive";
      return parseStep(session, budgetMs);
    },

    highlights(docId, from, to) {
      const session = sessions.get(docId);
      if (session === undefined || session.tree === null) return [];
      const clampedFrom = Math.max(0, Math.min(from, session.text.length));
      const clampedTo = Math.max(clampedFrom, Math.min(to, session.text.length));
      return highlightRunsForTree({
        tree: session.tree,
        registry,
        languageId: session.languageId,
        byteRange: { start: clampedFrom, end: clampedTo },
      }).map((run) => ({
        from: run.byteRange.start,
        to: run.byteRange.end,
        kind: registry.highlightName(run.kind) ?? "",
      }));
    },

    close(docId) {
      const session = sessions.get(docId);
      if (session === undefined) return;
      abortResume(session);
      sessions.delete(docId);
    },
  };
}

export const defaultRequire = createRequire(__filename);
