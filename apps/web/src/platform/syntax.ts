/**
 * SyntaxPort: the platform seam for tree-sitter — fully SYNCHRONOUS. The
 * desktop preload hosts the engine in the renderer process itself, so
 * highlight queries are plain function calls the layout pass can make with
 * no round trip: the frame that paints an edit already has its colors.
 *
 * Priority: dev test port (headless plumbing verification) > Electron
 * bridge. No browser fallback — without a host, documents simply stay
 * unhighlighted (capability gating, like FileSystemPort).
 *
 * Offsets are UTF-16 string indices on both sides. Edits address the OLD
 * text, ascending and non-overlapping (exactly DocumentChangeRange order).
 */

export type SyntaxEdit = { readonly start: number; readonly end: number; readonly text: string };

export type HighlightRun = { readonly from: number; readonly to: number; readonly kind: string };

/** "pending" = tree still parsing (big file); keep pumping. */
export type SyntaxStatus = "accurate" | "pending" | "inactive";

export interface SyntaxPort {
  open(docId: string, path: string | undefined, text: string): { language: string } | null;
  edit(docId: string, edits: readonly SyntaxEdit[]): SyntaxStatus;
  pump(docId: string, budgetMs: number): SyntaxStatus;
  highlights(docId: string, from: number, to: number): readonly HighlightRun[];
  close(docId: string): void;
}

export function createSyntaxPort(): SyntaxPort | undefined {
  const w = window as unknown as {
    __testSyntaxPort?: SyntaxPort;
    theEditorSyntaxSync?: SyntaxPort;
  };
  const isDev = location.hostname === "localhost" || location.hostname === "127.0.0.1";
  if (isDev && w.__testSyntaxPort) return w.__testSyntaxPort;
  if (w.theEditorSyntaxSync) return w.theEditorSyntaxSync;
  return undefined;
}
