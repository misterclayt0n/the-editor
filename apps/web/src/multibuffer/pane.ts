/**
 * A multibuffer view as a PaneHost, painted by the existing EditorRenderer.
 *
 * Scale model (mirrors Zed):
 *  - EDITS are incremental: a keystroke commits to the source document and the
 *    snapshot is patched via `mapDocumentChanges` (which reuses unchanged
 *    excerpts' summaries), never re-scanning every excerpt.
 *  - INTEL (LSP + syntax) is attached only for documents with a VISIBLE excerpt,
 *    ref-counted, debounced on scroll, and detached when scrolled away/closed —
 *    so a 1000-match search never opens 150 language-server documents at once.
 *  - DIAGNOSTICS are read per-document, windowed and cached, and mapped into the
 *    unified coordinate space (no global decoration set, no full-visuals merge).
 *
 * State is canonical -> derived: the live buffers, the unfolded `baseExcerpts`
 * (ranges that edits/expands mutate), the folded-doc set, and per-excerpt match
 * spans; `rebuild()`/`recomputeDerived()` derive the snapshot, metas, visuals.
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
  type DocumentChangeRange,
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
import type { IntelLocation, PaneHighlights, PaneHost } from "../runtime";
import { metasFor, type ExcerptMeta, type MultiBufferBuild } from "./builder";
import { excerptSlicesInRange, multiBufferVisuals } from "./visuals";

/** Surface kind for multibuffer panels (its own keymap/command scope). */
export const MultiBufferSurface = SurfaceKind("the-editor/multibuffer");

/** Lines added per expand-context click. */
const EXPAND_LINES = 5;
/** Debounce before (de)attaching intel as the viewport settles. */
const VIEWPORT_DEBOUNCE_MS = 50;

const rangeMapOptions = { startAssoc: "before", endAssoc: "after" } as const;

/** The slice of the app runtime a multibuffer pane needs. */
export interface MultiBufferHost {
  subscribeTheme(listener: () => void): () => void;
  jumpToLocation(path: string, position: lsp.Position): Promise<void>;
  /** Commit edits to a source document; returns its change + new buffer. */
  commitDocumentEdits(
    docId: string,
    edits: readonly TextEdit[],
  ): { readonly change: ChangeDesc; readonly buffer: TextBuffer } | undefined;
  /** The source document's current text buffer. */
  documentBuffer(docId: string): TextBuffer | undefined;
  /** The source document's syntax highlighter, if one is attached. */
  documentHighlights(docId: string): PaneHighlights | undefined;
  /** The source document's current version (for syntax sync). */
  documentVersion(docId: string): number;
  /** The source document's EditorVisuals (LSP diagnostics + inlay hints). */
  documentVisuals(docId: string): VisualsSnapshot | undefined;
  /** Subscribe to a source document's changes (diagnostics, edits); repaint. */
  subscribeDocument(docId: string, listener: () => void): () => void;
  /** Attach LSP + syntax + VCS to a source document (idempotent, ref-counted). */
  attachDocumentIntel(docId: string, path: string): void;
  /** Release intel for a source document when no longer visible anywhere. */
  detachDocumentIntel(docId: string): void;
}

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

  // Derived state (recomputed by rebuild()/recomputeDerived()).
  private snapshotData!: MultiBufferSnapshot;
  private metasData!: ExcerptMeta[];
  private visualsData!: VisualsSnapshot;
  private visualsGeneration = 0;

  // Viewport-scoped intel: LSP/syntax attached only for visible documents.
  private readonly attachedDocs = new Set<string>();
  private readonly docSubscriptions = new Map<string, () => void>();
  private readonly sourceVisualsCache = new Map<string, VisualsSnapshot>();
  private viewportScheduled = false;
  private disposed = false;
  private pendingViewport: { start: number; end: number } | null = null;

  private readonly editorId: string;
  private selection = Selection.single(0, 0);
  private version = 1;
  private serial = 0;
  private readonly listeners = new Set<
    (notification: EditorNotification, source: "document" | "view") => void
  >();
  private readonly highlightListeners = new Set<() => void>();
  private stickyListener: ((meta: ExcerptMeta | null) => void) | null = null;

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

  /** Every source document referenced by the multibuffer (for cleanup). */
  sourceDocumentIds(): readonly string[] {
    return [...this.documents.keys()];
  }

  /** Whether the document currently has a visible (intel-attached) excerpt. */
  usesDocument(docId: string): boolean {
    return this.attachedDocs.has(docId);
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
    return { ...this.visualsData, generation: this.visualsGeneration };
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

  /** Resolve a unified offset to the underlying source document + offset. The
   * common ground for LSP/intel: the IntelController treats this exactly like a
   * single-file pane (which returns itself). */
  resolveSource(offset: number): IntelLocation | null {
    const clamped = Math.max(0, Math.min(this.snapshotData.length, offset));
    const source = this.snapshotData.offsetToSource(clamped);
    if (source === undefined) return null;
    const text = this.documents.get(source.documentId);
    if (text === undefined) return null;
    return {
      docId: source.documentId,
      path: this.pathByDoc.get(source.documentId) ?? null,
      text,
      offset: source.offset,
    };
  }

  /** Per-excerpt syntax: map each visible source doc's runs into unified coords. */
  readonly highlights: PaneHighlights = {
    ensureSynced: () => {
      for (const docId of this.attachedDocs) {
        this.host.documentHighlights(docId)?.ensureSynced(this.host.documentVersion(docId), null);
      }
    },
    runsBetween: (from: number, to: number): readonly HighlightRun[] => {
      const out: HighlightRun[] = [];
      for (const slice of excerptSlicesInRange(this.metasData, from, to)) {
        if (!this.attachedDocs.has(slice.meta.excerpt.documentId)) continue;
        const docHighlights = this.host.documentHighlights(slice.meta.excerpt.documentId);
        if (docHighlights === undefined) continue;
        for (const run of docHighlights.runsBetween(slice.sourceFrom, slice.sourceTo)) {
          out.push({ from: slice.base + run.from, to: slice.base + run.to, kind: run.kind });
        }
      }
      return out;
    },
    subscribe: (listener: () => void): (() => void) => {
      this.highlightListeners.add(listener);
      return () => {
        this.highlightListeners.delete(listener);
      };
    },
  };

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

  // The view registers here to render a sticky header for the top excerpt.
  onViewport(listener: (meta: ExcerptMeta | null) => void): () => void {
    this.stickyListener = listener;
    return () => {
      if (this.stickyListener === listener) this.stickyListener = null;
    };
  }

  /** Renderer reports the top visible line (sticky) + the rendered line window
   * (intel attach). Attach is debounced so scrolling doesn't thrash servers. */
  readonly viewportChanged = (topLine: number, startLine: number, endLine: number): void => {
    this.stickyListener?.(this.stickyHeaderForLine(topLine));
    this.pendingViewport = { start: startLine, end: endLine };
    if (this.viewportScheduled) return;
    this.viewportScheduled = true;
    setTimeout(() => {
      this.viewportScheduled = false;
      const pending = this.pendingViewport;
      this.pendingViewport = null;
      if (!this.disposed && pending !== null) this.updateAttachments(pending.start, pending.end);
    }, VIEWPORT_DEBOUNCE_MS);
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

  /** Apply LSP/completion edits (in SOURCE coordinates) to a source document,
   * incrementally remap the multibuffer, and place the caret after the edit.
   * The completion accept routes here so the menu can edit through the view. */
  commitToSource(docId: string, replacements: readonly TextEdit[], sourceCaret: number): void {
    if (replacements.length === 0) return;
    const result = this.host.commitDocumentEdits(docId, replacements);
    if (result === undefined) return;
    this.documents.set(docId, result.buffer);
    const changes = new Map<string, ChangeDesc>([[docId, result.change]]);
    this.snapshotData = this.snapshotData.mapDocumentChanges(changes, this.documents);
    this.remapCanonical(changes);
    this.recomputeDerived();
    const caret = this.snapshotData.sourceToOffset(
      docId,
      Math.max(0, Math.min(result.buffer.length, sourceCaret)),
    );
    this.selection =
      caret === undefined
        ? this.clampSelection(this.selection)
        : Selection.single(caret, caret);
    this.notifyDocument();
  }

  /** Release viewport intel subscriptions (the runtime then frees the docs). */
  dispose(): void {
    this.disposed = true;
    for (const docId of [...this.attachedDocs]) this.detachIntel(docId);
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
    if (documentEdits.size === 0) return; // e.g. deleting a synthetic separator
    const changes = new Map<string, ChangeDesc>();
    for (const [docId, docEdits] of documentEdits) {
      const result = this.host.commitDocumentEdits(docId, docEdits);
      if (result === undefined) return;
      this.documents.set(docId, result.buffer);
      changes.set(docId, result.change);
    }
    // Incremental: patch only the changed documents' excerpts (reuse summaries).
    this.snapshotData = this.snapshotData.mapDocumentChanges(changes, this.documents);
    this.remapCanonical(changes);
    this.recomputeDerived();
    this.selection = this.clampSelection(editsToSelection(effective));
    // Report unified change spans so the controller can spot a typed char and
    // auto-trigger completion/signature. effective edits are already unified
    // coords; accumulate the shift across a multi-cursor batch.
    const changeRanges: DocumentChangeRange[] = [];
    let shift = 0;
    for (const edit of [...effective].sort((a, b) => a.from - b.from)) {
      const fromB = edit.from + shift;
      changeRanges.push({ fromA: edit.from, toA: edit.to, fromB, toB: fromB + edit.insert.length });
      shift += edit.insert.length - (edit.to - edit.from);
    }
    this.notifyDocument(changeRanges);
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

  /** Full structural rebuild (fold/expand/initial): re-derives the snapshot. */
  private rebuild(): void {
    this.snapshotData = MultiBufferSnapshot.fromExcerpts(this.documents, this.applyFold());
    this.recomputeDerived();
  }

  /** Recompute metas + visuals from the CURRENT snapshot (no text scan). */
  private recomputeDerived(): void {
    this.version++;
    this.visualsGeneration++;
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
    this.visualsData = multiBufferVisuals(this.snapshotData, this.metasData, this.sourceVisualsFor);
  }

  /** Cached, viewport-gated source-document visuals (LSP diagnostics + inlays). */
  private readonly sourceVisualsFor = (docId: string): VisualsSnapshot | undefined => {
    if (!this.attachedDocs.has(docId)) return undefined;
    let cached = this.sourceVisualsCache.get(docId);
    if (cached === undefined) {
      cached = this.host.documentVisuals(docId);
      if (cached !== undefined) this.sourceVisualsCache.set(docId, cached);
    }
    return cached;
  };

  private updateAttachments(startLine: number, endLine: number): void {
    if (this.snapshotData.length === 0) return;
    const lastLine = this.snapshotData.lineCount - 1;
    const from = this.snapshotData.lineToOffset(Math.max(0, Math.min(lastLine, startLine)));
    const to = this.snapshotData.lineToOffset(Math.max(0, Math.min(lastLine, endLine + 1)));
    const visible = new Set<string>();
    for (const slice of excerptSlicesInRange(this.metasData, from, to)) {
      visible.add(slice.meta.excerpt.documentId);
    }
    let changed = false;
    for (const docId of visible) {
      if (!this.attachedDocs.has(docId)) {
        this.attachIntel(docId);
        changed = true;
      }
    }
    for (const docId of [...this.attachedDocs]) {
      if (!visible.has(docId)) {
        this.detachIntel(docId);
        changed = true;
      }
    }
    if (changed) this.invalidateVisuals();
  }


  private attachIntel(docId: string): void {
    const path = this.pathByDoc.get(docId);
    if (path === undefined) return;
    this.attachedDocs.add(docId);
    this.host.attachDocumentIntel(docId, path);
    this.docSubscriptions.set(
      docId,
      this.host.subscribeDocument(docId, () => this.onSourceChanged(docId)),
    );
  }

  private detachIntel(docId: string): void {
    this.attachedDocs.delete(docId);
    this.docSubscriptions.get(docId)?.();
    this.docSubscriptions.delete(docId);
    this.sourceVisualsCache.delete(docId);
    this.host.detachDocumentIntel(docId);
  }

  private onSourceChanged(docId: string): void {
    this.sourceVisualsCache.delete(docId);
    const next = this.host.documentBuffer(docId);
    if (next !== undefined && this.documents.get(docId) !== next) {
      this.documents.set(docId, next);
      this.rebuild();
      this.selection = this.clampSelection(this.selection);
      this.notifyDocument();
    } else {
      this.invalidateVisuals();
    }
  }

  /** A source document's diagnostics/syntax changed (or attach set changed): repaint. */
  private invalidateVisuals(): void {
    this.visualsGeneration++;
    this.notify([invalidate(VisualsInvalidated, { generation: this.visualsGeneration })], "document");
    for (const listener of this.highlightListeners) listener();
  }

  /** The file header to pin while scrolled into a file's body (null at its top). */
  private stickyHeaderForLine(line: number): ExcerptMeta | null {
    if (line <= 0 || line >= this.snapshotData.lineCount) return null;
    const source = this.snapshotData.offsetToSource(this.snapshotData.lineToOffset(line));
    if (source === undefined) return null;
    const header = this.metasData.find(
      (meta) => meta.isFirstOfFile && meta.excerpt.documentId === source.documentId,
    );
    if (header === undefined) return null;
    return line > this.snapshotData.offsetToLine(header.startOffset) ? header : null;
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

  private notifyDocument(changes: readonly DocumentChangeRange[] = []): void {
    this.notify(
      [
        invalidate(DocumentInvalidated, { version: this.version, changes }),
        invalidate(VisualsInvalidated, { generation: this.visualsGeneration }),
        invalidate(SelectionInvalidated),
      ],
      "document",
    );
    for (const listener of this.highlightListeners) listener();
  }

  private notify(invalidations: readonly Invalidation[], source: "document" | "view"): void {
    const notification: EditorNotification = { serial: ++this.serial, invalidations };
    for (const listener of this.listeners) listener(notification, source);
  }
}
