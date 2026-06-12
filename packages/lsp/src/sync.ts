/**
 * Position bookkeeping for document synchronization.
 *
 * LSP positions are line/character with characters in UTF-16 code units —
 * exactly the editor's native offsets, so conversion is pure line-start
 * arithmetic. `DocumentSyncState` maintains the line index of the text AS
 * THE SERVER KNOWS IT and converts editor change sets (simultaneous,
 * old-text coordinates, ascending) into LSP incremental contentChanges
 * (sequential semantics — achieved by emitting in DESCENDING order, where
 * old-text coordinates stay valid at each step).
 */

import type * as lsp from "vscode-languageserver-protocol";

export type SyncEdit = { readonly start: number; readonly end: number; readonly text: string };

const scanLineStarts = (text: string): number[] => {
  const starts = [0];
  for (let i = text.indexOf("\n"); i !== -1; i = text.indexOf("\n", i + 1)) starts.push(i + 1);
  return starts;
};

const rowOf = (starts: readonly number[], offset: number): number => {
  let lo = 0;
  let hi = starts.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (starts[mid]! <= offset) lo = mid;
    else hi = mid - 1;
  }
  return lo;
};

/** Offset → LSP position against a line-starts index. */
const positionAt = (starts: readonly number[], offset: number): lsp.Position => {
  const line = rowOf(starts, offset);
  return { line, character: offset - starts[line]! };
};

/** LSP position → offset in `text` (clamped to the line and document). */
export function offsetAtPosition(text: string, position: lsp.Position): number {
  let line = 0;
  let lineStart = 0;
  while (line < position.line) {
    const next = text.indexOf("\n", lineStart);
    if (next === -1) return text.length;
    lineStart = next + 1;
    line++;
  }
  const lineEnd = text.indexOf("\n", lineStart);
  const end = lineEnd === -1 ? text.length : lineEnd;
  return Math.min(lineStart + Math.max(0, position.character), end);
}

export class DocumentSyncState {
  private starts: number[];

  constructor(text: string) {
    this.starts = scanLineStarts(text);
  }

  reset(text: string): void {
    this.starts = scanLineStarts(text);
  }

  /**
   * Convert one editor change set into LSP contentChanges and advance the
   * index. Input edits are old-text coordinates, ascending, disjoint;
   * output is the equivalent SEQUENTIAL change list (descending order).
   */
  applyEdits(edits: readonly SyncEdit[]): lsp.TextDocumentContentChangeEvent[] {
    const out: lsp.TextDocumentContentChangeEvent[] = [];
    for (let i = edits.length - 1; i >= 0; i--) {
      const edit = edits[i]!;
      out.push({
        range: {
          start: positionAt(this.starts, edit.start),
          end: positionAt(this.starts, edit.end),
        },
        text: edit.text,
      });
    }
    // Advance the index (descending keeps earlier offsets stable).
    for (let i = edits.length - 1; i >= 0; i--) {
      const edit = edits[i]!;
      const delta = edit.text.length - (edit.end - edit.start);
      const startRow = rowOf(this.starts, edit.start);
      const endRow = rowOf(this.starts, edit.end);
      const inserted: number[] = [];
      for (
        let at = edit.text.indexOf("\n");
        at !== -1;
        at = edit.text.indexOf("\n", at + 1)
      ) {
        inserted.push(edit.start + at + 1);
      }
      const tail = this.starts.slice(endRow + 1).map((start) => start + delta);
      this.starts = [...this.starts.slice(0, startRow + 1), ...inserted, ...tail];
    }
    return out;
  }
}

/** Minimal glob matching for watched-files patterns (`**`, `*`, `?`). */
export function globMatches(pattern: string, path: string): boolean {
  let regex = "^";
  for (let i = 0; i < pattern.length; i++) {
    const ch = pattern[i]!;
    if (ch === "*") {
      if (pattern[i + 1] === "*") {
        regex += ".*";
        i++;
        if (pattern[i + 1] === "/") i++; // `**/` also matches zero segments
      } else {
        regex += "[^/]*";
      }
    } else if (ch === "?") {
      regex += "[^/]";
    } else if (ch === "{") {
      regex += "(";
    } else if (ch === "}") {
      regex += ")";
    } else if (ch === ",") {
      regex += "|";
    } else {
      regex += ch.replace(/[.+^$()|[\]\\]/g, "\\$&");
    }
  }
  regex += "$";
  try {
    return new RegExp(regex).test(path);
  } catch {
    return false;
  }
}
