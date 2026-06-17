/**
 * Builds a MultiBufferSnapshot from global-search matches and derives the
 * per-excerpt render metadata.
 *
 * The input is already normalized to SOURCE OFFSETS per file (the runtime
 * adapter converts fff's line/col/byteOffset shape into character offsets, so
 * this stays a pure, testable function over engine primitives). Each match is
 * expanded to a whole-line window with `contextLines` of surrounding context,
 * overlapping/adjacent windows per file are merged, and the result is fed to
 * `MultiBufferSnapshot.fromExcerpts`.
 *
 * `metasFor` is reused by the pane to recompute metadata after edits, expands,
 * and fold toggles (which rebuild the active snapshot from canonical state).
 */

import { MultiBufferSnapshot, type Excerpt, type Span, type TextBuffer } from "@the-editor/engine";

/** One source file with the primary match spans (source offsets) found in it. */
export type MultiBufferMatchFile = {
  /** Absolute path, used for jump-to-source. */
  readonly path: string;
  /** Document id keying the excerpt's source buffer (e.g. `file:${path}`). */
  readonly documentId: string;
  /** Current source snapshot of the file. */
  readonly buffer: TextBuffer;
  /** Primary match spans in source character offsets. */
  readonly matches: readonly Span[];
};

/** Render-facing metadata for one excerpt card, parallel to the active excerpts. */
export type ExcerptMeta = {
  readonly excerpt: Excerpt;
  /** Unified offset of the excerpt's first character. */
  readonly startOffset: number;
  /** Editor line after which the header block is inserted. */
  readonly headerAfterLine: number;
  readonly path: string;
  readonly fileName: string;
  /** Parent directory (display), without the file name. */
  readonly dir: string;
  /** True when this excerpt starts a new source file (file header vs divider). */
  readonly isFirstOfFile: boolean;
  /** Match spans inside this excerpt, in SOURCE offsets (for highlighting). */
  readonly matches: readonly Span[];
  /** Whether this excerpt's file is collapsed (header shows a ▸ chevron). */
  readonly folded: boolean;
  /** Total matches across the file (shown on the folded header). */
  readonly fileMatchCount: number;
  /** Whether the excerpt can grow up / down (chevron enablement). */
  readonly canExpandUp: boolean;
  readonly canExpandDown: boolean;
};

export type MultiBufferBuild = {
  readonly snapshot: MultiBufferSnapshot;
  readonly metas: readonly ExcerptMeta[];
  /** Canonical (unfolded) excerpts; the pane mutates these on edit/expand. */
  readonly excerpts: readonly Excerpt[];
  readonly documents: ReadonlyMap<string, TextBuffer>;
  readonly pathByDoc: ReadonlyMap<string, string>;
  readonly matchesByExcerptId: ReadonlyMap<string, readonly Span[]>;
};

/** The pieces metasFor needs that are not in the snapshot itself. */
export type MetaContext = {
  readonly pathByDoc: ReadonlyMap<string, string>;
  readonly matchesByExcerptId: ReadonlyMap<string, readonly Span[]>;
  readonly matchCountByDoc: ReadonlyMap<string, number>;
  readonly foldedDocs: ReadonlySet<string>;
  readonly documents: ReadonlyMap<string, TextBuffer>;
};

const DEFAULT_CONTEXT_LINES = 2;

const clamp = (n: number, min: number, max: number): number => Math.max(min, Math.min(max, n));

const clampSpan = (span: Span, from: number, to: number): Span => ({
  start: clamp(span.start, from, to),
  end: clamp(span.end, from, to),
});

const baseName = (path: string): string => {
  const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return slash < 0 ? path : path.slice(slash + 1);
};

const dirName = (path: string): string => {
  const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return slash < 0 ? "" : path.slice(0, slash);
};

type PendingExcerpt = {
  readonly excerpt: Excerpt;
  readonly path: string;
  readonly matches: readonly Span[];
};

type Window = { startLine: number; endLine: number; matches: Span[] };

/** Derive per-excerpt metadata for the snapshot's CURRENT (active) excerpts. */
export function metasFor(snapshot: MultiBufferSnapshot, context: MetaContext): ExcerptMeta[] {
  const excerpts = snapshot.excerpts();
  const headerAfterLine = (excerpt: Excerpt, startOffset: number): number => {
    if (startOffset === 0) return -1;
    if (excerpt.sourceRange.start === excerpt.sourceRange.end)
      return snapshot.offsetToLine(startOffset - 1);
    return snapshot.offsetToLine(startOffset) - 1;
  };
  const boundaries = snapshot.excerptBoundariesBetween(0, snapshot.length);
  return excerpts.map((excerpt, i): ExcerptMeta => {
    const path = context.pathByDoc.get(excerpt.documentId) ?? excerpt.documentId;
    const buffer = context.documents.get(excerpt.documentId);
    const folded = context.foldedDocs.has(excerpt.documentId);
    const matches = folded
      ? []
      : (context.matchesByExcerptId.get(excerpt.id) ?? [])
          .map((m) => clampSpan(m, excerpt.sourceRange.start, excerpt.sourceRange.end))
          .filter((m) => m.end > m.start);
    const startOffset = boundaries[i]!.offset;
    return {
      excerpt,
      startOffset,
      headerAfterLine: headerAfterLine(excerpt, startOffset),
      path,
      fileName: baseName(path),
      dir: dirName(path),
      isFirstOfFile: i === 0 || excerpts[i - 1]!.documentId !== excerpt.documentId,
      matches,
      folded,
      fileMatchCount: context.matchCountByDoc.get(excerpt.documentId) ?? 0,
      canExpandUp: !folded && excerpt.sourceRange.start > 0,
      canExpandDown: !folded && buffer !== undefined && excerpt.sourceRange.end < buffer.length,
    };
  });
}

/** Build the snapshot + canonical state + metadata from per-file matches. */
export function buildMultiBuffer(
  files: readonly MultiBufferMatchFile[],
  options?: { readonly contextLines?: number },
): MultiBufferBuild {
  const context = Math.max(0, options?.contextLines ?? DEFAULT_CONTEXT_LINES);
  const documents = new Map<string, TextBuffer>();
  const pending: PendingExcerpt[] = [];
  let seq = 0;

  for (const file of files) {
    const { buffer } = file;
    const windows: Window[] = file.matches
      .map((m) => {
        const startLine = Math.max(
          0,
          buffer.lineRangeAt(clamp(m.start, 0, buffer.length)).index - context,
        );
        const endLine = Math.min(
          buffer.lineCount - 1,
          buffer.lineRangeAt(clamp(m.end, 0, buffer.length)).index + context,
        );
        return { startLine, endLine, matches: [m] };
      })
      .sort((a, b) => a.startLine - b.startLine || a.endLine - b.endLine);
    if (windows.length === 0) continue;

    const merged: Window[] = [];
    for (const w of windows) {
      const last = merged[merged.length - 1];
      if (last && w.startLine <= last.endLine + 1) {
        last.endLine = Math.max(last.endLine, w.endLine);
        last.matches.push(...w.matches);
      } else {
        merged.push({ startLine: w.startLine, endLine: w.endLine, matches: [...w.matches] });
      }
    }

    let registered = false;
    for (const w of merged) {
      const from = buffer.lineRange(w.startLine).from;
      const to = buffer.lineRange(w.endLine).to;
      if (to <= from) continue;
      const sourceRange: Span = { start: from, end: to };
      const matches = w.matches
        .map((m) => clampSpan(m, from, to))
        .sort((a, b) => a.start - b.start || a.end - b.end);
      const primaryRange = matches[0] ?? sourceRange;
      pending.push({
        excerpt: { id: `${file.documentId}#${seq++}`, documentId: file.documentId, sourceRange, primaryRange },
        path: file.path,
        matches,
      });
      registered = true;
    }
    if (registered) documents.set(file.documentId, buffer);
  }

  const excerpts = pending.map((p) => p.excerpt);
  const pathByDoc = new Map<string, string>();
  const matchesByExcerptId = new Map<string, readonly Span[]>();
  const matchCountByDoc = new Map<string, number>();
  for (const p of pending) {
    if (!pathByDoc.has(p.excerpt.documentId)) pathByDoc.set(p.excerpt.documentId, p.path);
    matchesByExcerptId.set(p.excerpt.id, p.matches);
    matchCountByDoc.set(
      p.excerpt.documentId,
      (matchCountByDoc.get(p.excerpt.documentId) ?? 0) + p.matches.length,
    );
  }

  const snapshot = MultiBufferSnapshot.fromExcerpts(documents, excerpts);
  const metas = metasFor(snapshot, {
    pathByDoc,
    matchesByExcerptId,
    matchCountByDoc,
    foldedDocs: new Set(),
    documents,
  });
  return { snapshot, metas, excerpts, documents, pathByDoc, matchesByExcerptId };
}
