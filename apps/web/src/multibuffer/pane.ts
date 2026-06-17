/**
 * A multibuffer view as a PaneHost, painted by the existing EditorRenderer.
 *
 * State is canonical -> derived. The canonical state is the live source buffers,
 * the unfolded `baseExcerpts` (source ranges that edits + expands mutate), the
 * set of folded documents, and the per-excerpt match spans. `rebuild()` derives
 * the active MultiBufferSnapshot (folds collapse a file to a single row), the
 * render metas, and the synthesized visuals from it.
 *
 * Editing routes the standard editing commands through `editPlan`: a unified
 * edit becomes per-document edits, each committed to the REAL source document
 * (so it persists, is undoable, and updates any other view), then the canonical
 * excerpts/matches are mapped through the document changes and the snapshot is
 * rebuilt. Selection is set from the command result (clamped).
 */

import {
  expandExcerptRange,
  MultiBufferSnapshot,
  Selection,
  SelectionRange,
  type ChangeDesc,
  type Excerpt,
  type Span,
  type TextBuffer,
  type TextEdit,
} from "@the-editor/engine";
import {
  DocumentInvalidated,
  invalidate,
  SelectionInvalidated,
  VisualsInvalidated,
  type EditorNotification,
  type EditorSnapshot,
  type Invalidation,
  type VisualsSnapshot,
} from "@the-editor/editor";
import type { KeyboardEventLike } from "@the-editor/keymaps";
import type { lsp } from "@the-editor/lsp";
import { SurfaceKind, type PanelId } from "@the-editor/workbench";
import {
  deleteBackwardEdits,
  deleteForwardEdits,
  editsToSelection,
  insertEdits,
  moveHorizontal,
  moveLineEdge,
  moveVertical,
  type Edit,
} from "../editor/commands";
import { FloatHub } from "../editor/floats";
import { MountHub } from "../editor/mounts";
import type { HighlightRun } from "../platform/syntax";
import type { PaneHighlights, PaneHost } from "../runtime";
import { metasFor, type ExcerptMeta, type MultiBufferBuild } from "./builder";
import { multiBufferVisuals } from "./visuals";

/** Surface kind for multibuffer panels (its own keymap/command scope). */
export const MultiBufferSurface = SurfaceKind("the-editor/multibuffer");

/** Lines added per expand-context click. */
const EXPAND_LINES = 5;

/** The slice of the app runtime a multibuffer pane needs. */
export interface MultiBufferHost {
  subscribeTheme(listener: () => void): () => void;
  jumpToLocation(path: string, position: lsp.Position): Promise<void>;
  /** Commit edits to a source document; returns its change + new buffer. */
  commitDocumentEdits(
    docId: string,
    edits: readonly TextEdit[],
  ): { readonly change: ChangeDesc; readonly buffer: TextBuffer } | undefined;
  /** The source document's syntax highlighter, if one is attached. */
  documentHighlights(docId: string): PaneHighlights | undefined;
  /** The source document's current version (for syntax sync). */
  documentVersion(docId: string): number;
}
const rangeMapOptions = { startAssoc: "before", endAssoc: "after" } as const;

let scopeSeq = 0;

export class MultiBufferPane implements PaneHost {
  readonly mounts = new MountHub();
  readonly floats = new FloatHub();

  // Canonical state.
  private readonly documents: Map<string, TextBuffer>;
  private baseExcerpts: Excerpt[];
  private matchesByExcerptId: Map<string, readonly Span[]>;
  private readonly pathByDoc: ReadonlyMap<string, string>;
  private readonly foldedDocs = new Set<string>();

  // Derived state (recomputed by rebuild()).
  private snapshotData!: MultiBufferSnapshot;
  private metasData!: ExcerptMeta[];
  private visualsData!: VisualsSnapshot;

  private readonly editorId: string;
  private selection = Selection.single(0, 0);
  private version = 1;
  private serial = 0;
  private readonly listeners = new Set<
    (notification: EditorNotification, source: "document" | "view") => void
  >();

  constructor(
    readonly panelId: PanelId,
    readonly title: string,
    build: MultiBufferBuild,
    private readonly host: MultiBufferHost,
  ) {
    this.editorId = `multibuffer:${++scopeSeq}`;
    this.documents = new Map(build.documents);
    this.baseExcerpts = [...build.excerpts];
    this.matchesByExcerptId = new Map(build.matchesByExcerptId);
    this.pathByDoc = build.pathByDoc;
    this.rebuild();
  }

  get metas(): readonly ExcerptMeta[] {
    return this.metasData;
  }

  snapshot(): EditorSnapshot {
    return {
      id: this.editorId as EditorSnapshot["id"],
      text: this.snapshotData,
      version: this.version,
      dirty: false,
      selection: this.selection,
    };
  }

  visualsSnapshot(): VisualsSnapshot {
    return this.visualsData;
  }

  setSelection(selection: Selection): void {
    this.selection = selection;
    this.notify([invalidate(SelectionInvalidated)], "view");
  }

  subscribe(
    listener: (notification: EditorNotification, source: "document" | "view") => void,
  ): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  subscribeTheme(listener: () => void): () => void {
    return this.host.subscribeTheme(listener);
  }

  /** Per-excerpt syntax: map each source doc's highlight runs into unified coords. */
  readonly highlights: PaneHighlights = {
    ensureSynced: () => {
      for (const docId of this.documents.keys()) {
        this.host.documentHighlights(docId)?.ensureSynced(this.host.documentVersion(docId), null);
      }
    },
    runsBetween: (from: number, to: number): readonly HighlightRun[] => {
      const out: HighlightRun[] = [];
      for (const meta of this.metasData) {
        const start = meta.startOffset;
        const end = start + (meta.excerpt.sourceRange.end - meta.excerpt.sourceRange.start);
        if (end <= from || start >= to) continue;
        const docHighlights = this.host.documentHighlights(meta.excerpt.documentId);
        if (docHighlights === undefined) continue;
        const base = start - meta.excerpt.sourceRange.start;
        const sourceFrom = meta.excerpt.sourceRange.start + Math.max(0, from - start);
        const sourceTo = meta.excerpt.sourceRange.start + Math.min(end - start, to - start);
        for (const run of docHighlights.runsBetween(sourceFrom, sourceTo)) {
          out.push({ from: base + run.from, to: base + run.to, kind: run.kind });
        }
      }
      return out;
    },
    subscribe: (listener: () => void): (() => void) => {
      const unsubscribes = [...this.documents.keys()].map(
        (docId) => this.host.documentHighlights(docId)?.subscribe(listener) ?? (() => {}),
      );
      return () => {
        for (const unsubscribe of unsubscribes) unsubscribe();
      };
    },
  };

  // The view registers here to render a sticky header for the top excerpt.
  private stickyListener: ((meta: ExcerptMeta | null) => void) | null = null;

  onViewport(listener: (meta: ExcerptMeta | null) => void): () => void {
    this.stickyListener = listener;
    return () => {
      if (this.stickyListener === listener) this.stickyListener = null;
    };
  }

  readonly viewportChanged = (topLine: number): void => {
    this.stickyListener?.(this.stickyHeaderForLine(topLine));
  };

  /** The file header to pin while scrolled into a file's body (null at its top). */
  private stickyHeaderForLine(line: number): ExcerptMeta | null {
    if (line <= 0 || line >= this.snapshotData.lineCount) return null;
    const source = this.snapshotData.offsetToSource(this.snapshotData.lineToOffset(line));
    if (source === undefined) return null;
    const header = this.metasData.find(
      (meta) => meta.isFirstOfFile && meta.excerpt.documentId === source.documentId,
    );
    if (header === undefined) return null;
    // Only pin once the real header has scrolled above the top line.
    return line > this.snapshotData.offsetToLine(header.startOffset) ? header : null;
  }

  /** A click on a gutter line number jumps to that line in the source file. */
  gutterClick(line: number): boolean {
    if (line < 0 || line >= this.snapshotData.lineCount) return false;
    const source = this.snapshotData.offsetToSource(this.snapshotData.lineToOffset(line));
    if (source === undefined) return false;
    const buffer = this.documents.get(source.documentId);
    const path = this.pathByDoc.get(source.documentId);
    if (buffer === undefined || path === undefined) return false;
    const at = buffer.offsetToLineColumn(source.offset);
    void this.host.jumpToLocation(path, { line: at.line, character: at.column });
    return true;
  }

  /** Read-only nav + editing. Runs before keymaps; returns true when consumed. */
  readonly keyInterceptor = (event: KeyboardEventLike): boolean => {
    const mod = event.metaKey === true || event.ctrlKey === true;
    const extend = event.shiftKey === true;
    const snap = this.snapshot();
    if (mod) {
      if (event.key === "a" || event.key === "A") {
        this.setSelection(Selection.single(0, this.snapshotData.length));
        return true;
      }
      return false; // other cmd/ctrl shortcuts pass through to global keymaps
    }
    switch (event.key) {
      case "ArrowLeft":
        this.setSelection(moveHorizontal(snap, -1, extend).selection);
        return true;
      case "ArrowRight":
        this.setSelection(moveHorizontal(snap, 1, extend).selection);
        return true;
      case "ArrowUp":
        this.setSelection(moveVertical(snap, -1, extend).selection);
        return true;
      case "ArrowDown":
        this.setSelection(moveVertical(snap, 1, extend).selection);
        return true;
      case "Home":
        this.setSelection(moveLineEdge(snap, "start", extend).selection);
        return true;
      case "End":
        this.setSelection(moveLineEdge(snap, "end", extend).selection);
        return true;
      case "Backspace":
        this.applyEdit(deleteBackwardEdits(snap));
        return true;
      case "Delete":
        this.applyEdit(deleteForwardEdits(snap));
        return true;
      case "Enter":
        this.applyEdit(insertEdits(snap, "\n"));
        return true;
      case "Tab":
        this.applyEdit(insertEdits(snap, "\t"));
        return true;
      default:
        if (!event.altKey && event.key.length === 1) {
          this.applyEdit(insertEdits(snap, event.key));
          return true;
        }
        return false;
    }
  };

  /** Grow an excerpt's context by EXPAND_LINES rows, clamped to neighbors. */
  expandExcerpt(excerptId: string, direction: "up" | "down"): void {
    const index = this.baseExcerpts.findIndex((excerpt) => excerpt.id === excerptId);
    if (index < 0) return;
    const excerpt = this.baseExcerpts[index]!;
    const buffer = this.documents.get(excerpt.documentId);
    if (buffer === undefined) return;
    const grown = this.clampExpansion(
      index,
      expandExcerptRange(buffer, excerpt.sourceRange, direction, EXPAND_LINES),
    );
    if (grown.start === excerpt.sourceRange.start && grown.end === excerpt.sourceRange.end) return;
    this.baseExcerpts = this.baseExcerpts.map((current, i) =>
      i === index ? { ...current, sourceRange: grown } : current,
    );
    this.rebuild();
    this.selection = this.clampSelection(this.selection);
    this.notifyDocument();
  }

  /** Collapse / restore a whole file's excerpts. */
  toggleFold(documentId: string): void {
    if (this.foldedDocs.has(documentId)) this.foldedDocs.delete(documentId);
    else this.foldedDocs.add(documentId);
    this.rebuild();
    this.selection = this.clampSelection(this.selection);
    this.notifyDocument();
  }

  /** Open the underlying source file at an excerpt's primary match. */
  openExcerpt(meta: ExcerptMeta): void {
    const buffer = this.documents.get(meta.excerpt.documentId);
    if (buffer === undefined) return;
    const at = buffer.offsetToLineColumn(meta.excerpt.primaryRange.start);
    void this.host.jumpToLocation(meta.path, { line: at.line, character: at.column });
  }

  // --- internals -----------------------------------------------------------

  private applyEdit(edits: readonly Edit[]): void {
    const effective = edits.filter((edit) => edit.from < edit.to || edit.insert.length > 0);
    if (effective.length === 0) return;
    const textEdits: TextEdit[] = effective.map((edit) => ({
      span: { start: edit.from, end: edit.to },
      text: edit.insert,
    }));
    let documentEdits: ReadonlyMap<string, readonly TextEdit[]>;
    try {
      documentEdits = this.snapshotData.editPlan(textEdits).documentEdits;
    } catch {
      return; // overlapping/invalid edit
    }
    if (documentEdits.size === 0) return; // e.g. deleting a synthetic excerpt separator
    const changes = new Map<string, ChangeDesc>();
    for (const [docId, docEdits] of documentEdits) {
      const result = this.host.commitDocumentEdits(docId, docEdits);
      if (result === undefined) return;
      this.documents.set(docId, result.buffer);
      changes.set(docId, result.change);
    }
    this.remapCanonical(changes);
    this.rebuild();
    this.selection = this.clampSelection(editsToSelection(effective));
    this.notifyDocument();
  }

  /** Slide canonical excerpts + matches through committed document changes. */
  private remapCanonical(changes: ReadonlyMap<string, ChangeDesc>): void {
    const nextExcerpts: Excerpt[] = [];
    const nextMatches = new Map<string, readonly Span[]>();
    for (const excerpt of this.baseExcerpts) {
      const change = changes.get(excerpt.documentId);
      if (change === undefined) {
        nextExcerpts.push(excerpt);
        nextMatches.set(excerpt.id, this.matchesByExcerptId.get(excerpt.id) ?? []);
        continue;
      }
      const sourceRange = change.mapRange(excerpt.sourceRange, rangeMapOptions);
      if (sourceRange.start === sourceRange.end) continue; // excerpt deleted away
      const primary = change.mapRange(excerpt.primaryRange, rangeMapOptions);
      nextExcerpts.push({
        id: excerpt.id,
        documentId: excerpt.documentId,
        sourceRange,
        primaryRange: {
          start: Math.max(sourceRange.start, Math.min(sourceRange.end, primary.start)),
          end: Math.max(sourceRange.start, Math.min(sourceRange.end, primary.end)),
        },
      });
      nextMatches.set(
        excerpt.id,
        (this.matchesByExcerptId.get(excerpt.id) ?? [])
          .map((span) => change.mapRange(span, rangeMapOptions))
          .filter((span) => span.end > span.start),
      );
    }
    this.baseExcerpts = nextExcerpts;
    this.matchesByExcerptId = nextMatches;
  }

  private rebuild(): void {
    this.version++;
    const active = this.applyFold();
    this.snapshotData = MultiBufferSnapshot.fromExcerpts(this.documents, active);
    const matchCountByDoc = new Map<string, number>();
    for (const excerpt of this.baseExcerpts) {
      const count = (this.matchesByExcerptId.get(excerpt.id) ?? []).length;
      matchCountByDoc.set(excerpt.documentId, (matchCountByDoc.get(excerpt.documentId) ?? 0) + count);
    }
    this.metasData = metasFor(this.snapshotData, {
      pathByDoc: this.pathByDoc,
      matchesByExcerptId: this.matchesByExcerptId,
      matchCountByDoc,
      foldedDocs: this.foldedDocs,
      documents: this.documents,
    });
    this.visualsData = multiBufferVisuals(this.snapshotData, this.metasData, this.version);
  }

  /** Derive the active excerpts: folded files collapse to a single row. */
  private applyFold(): Excerpt[] {
    if (this.foldedDocs.size === 0) return this.baseExcerpts;
    const out: Excerpt[] = [];
    for (let i = 0; i < this.baseExcerpts.length; ) {
      const excerpt = this.baseExcerpts[i]!;
      if (!this.foldedDocs.has(excerpt.documentId)) {
        out.push(excerpt);
        i++;
        continue;
      }
      const buffer = this.documents.get(excerpt.documentId);
      const firstLine = buffer?.lineRangeAt(excerpt.sourceRange.start);
      const end = firstLine
        ? Math.max(firstLine.to, Math.min(excerpt.sourceRange.end, firstLine.from + 1))
        : excerpt.sourceRange.end;
      out.push({
        id: excerpt.id,
        documentId: excerpt.documentId,
        sourceRange: { start: excerpt.sourceRange.start, end },
        primaryRange: { start: excerpt.sourceRange.start, end },
      });
      // Skip the rest of this file's consecutive excerpts.
      let j = i + 1;
      while (j < this.baseExcerpts.length && this.baseExcerpts[j]!.documentId === excerpt.documentId)
        j++;
      i = j;
    }
    return out;
  }

  /** Keep an expanded range from overlapping a same-file neighbor. */
  private clampExpansion(index: number, grown: Span): Span {
    const excerpt = this.baseExcerpts[index]!;
    const buffer = this.documents.get(excerpt.documentId)!;
    let start = grown.start;
    let end = grown.end;
    const previous = this.baseExcerpts[index - 1];
    if (previous && previous.documentId === excerpt.documentId) {
      const lastLine = buffer.lineRangeAt(
        Math.max(previous.sourceRange.start, previous.sourceRange.end - 1),
      ).index;
      start = Math.max(start, buffer.lineRange(Math.min(buffer.lineCount - 1, lastLine + 1)).from);
    }
    const next = this.baseExcerpts[index + 1];
    if (next && next.documentId === excerpt.documentId) {
      const firstLine = buffer.lineRangeAt(next.sourceRange.start).index;
      end = Math.min(end, buffer.lineRange(Math.max(0, firstLine - 1)).to);
    }
    return {
      start: Math.min(excerpt.sourceRange.start, start),
      end: Math.max(excerpt.sourceRange.end, end),
    };
  }

  private clampSelection(selection: Selection): Selection {
    const length = this.snapshotData.length;
    const clamp = (pos: number): number => Math.max(0, Math.min(length, pos));
    return Selection.create(
      selection.ranges.map((range) => SelectionRange.create(clamp(range.anchor), clamp(range.head))),
      { primaryIndex: selection.primaryIndex },
    );
  }

  private notifyDocument(): void {
    this.notify(
      [
        invalidate(DocumentInvalidated, { version: this.version, changes: [] }),
        invalidate(VisualsInvalidated, { generation: this.version }),
        invalidate(SelectionInvalidated),
      ],
      "document",
    );
  }

  private notify(invalidations: readonly Invalidation[], source: "document" | "view"): void {
    const notification: EditorNotification = { serial: ++this.serial, invalidations };
    for (const listener of this.listeners) listener(notification, source);
  }
}
