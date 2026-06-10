/**
 * EditorRenderer: the vanilla-DOM rendering hot path, with OWNED SCROLLING.
 *
 * There is no native scroller. A scroll model (plain numbers) is driven by a
 * non-passive wheel listener and a custom scrollbar; the rendered window of
 * rows is positioned with a transform. This is the Monaco/VSCode model, and
 * it is the conclusion of a long investigation (chrome traces + compositor
 * video of real input):
 *
 * - With NATIVE scrolling, the compositor scrolls ahead while Chromium
 *   throttles main-thread commits to ~100-160ms during the gesture (and
 *   macOS native-scrollbar drags run a nested event loop that can starve
 *   the main thread for seconds). Every commit then races raster, drawing
 *   50-220ms of missing-tile (checkerboard) frames — perceived as flicker
 *   and blank text. Clamping tricks (pierre's inverse sticky) require
 *   compositor-promoted layers, whose tiles die on every content shift; we
 *   exhausted that whole design space.
 * - With OWNED scrolling, the visual scroll position only moves when we
 *   draw: content and offset update atomically in the same frame, commits
 *   run at full display rate (nothing throttles them — there is no native
 *   scroll), and a raster gap cannot exceed one frame. Every synthetic test
 *   we ever ran (which is exactly this regime: JS-paced scrolling) was
 *   artifact-free at any speed; this makes real input take that path.
 * - Wheel momentum needs no simulation: macOS delivers momentum as wheel
 *   events. The custom scrollbar uses ordinary pointer capture, so thumb
 *   drags are plain mousemove events at full rate — no nested run loop.
 *
 * Rendering: a window of lines around the viewport, rows in NORMAL FLOW
 * (children of the content element; child i = line windowStart + i),
 * edge-trimmed on slides so retained rows get zero writes. The content
 * element is translated to (windowTop - scrollY). Caret/selection overlays
 * are window-local absolute elements. Per-line dirty tracking from
 * DocumentInvalidated change ranges rewrites only touched rows; structural
 * edits rebuild the window. Edits above the viewport adjust the scroll
 * model so visible text holds still.
 *
 * Syntax highlighting is intentionally absent: it will be a worker-owned
 * tree-sitter document (Zed's parse-async/query-windowed model, see
 * packages/syntax SyntaxState) feeding TextStyleRuns into layoutViewport's
 * visuals — applied by rewriting affected rows.
 */

import {
  DocumentInvalidated,
  hasInvalidation,
  hitTestTextPosition,
  layoutContentHeight,
  layoutViewport,
  payloadsOf,
  SelectionInvalidated,
  type DocumentChangeRange,
  type EditorNotification,
  type EditorSnapshot,
  type LayoutInlineSegment,
  type ViewportLayoutSnapshot,
} from "@the-editor/editor";
import { Selection } from "@the-editor/engine";
import type { Chord } from "@the-editor/keymaps";
import type { AppRuntime } from "../runtime";
import { cancelRender, queueRender } from "./frame";
import { RenderDebug } from "./debug";

const LINE_HEIGHT = 20;
const TAB_WIDTH = 4;
const PADDING_LEFT = 12;
const PADDING_TOP = 8;
/** Pixels of content kept rendered beyond each viewport edge. */
const OVERSCAN_PX = 800;
/** Re-center the window when the viewport gets this close to its edge. */
const WINDOW_SLACK_PX = 300;
/** Extra horizontal room past the longest rendered line. */
const H_SCROLL_PAD = 48;
const SCROLLBAR_MIN_THUMB = 32;

export type EditorRendererOptions = {
  readonly onSnapshot?: (snapshot: EditorSnapshot) => void;
  readonly onPending?: (chords: ReadonlyArray<Chord>) => void;
};

/** Map an offset forward through one transaction's change ranges. */
function mapOffset(changes: ReadonlyArray<DocumentChangeRange>, pos: number): number {
  let diff = 0;
  for (const range of changes) {
    if (pos < range.fromA) break;
    if (pos >= range.toA) {
      diff += range.toB - range.fromB - (range.toA - range.fromA);
      continue;
    }
    return range.fromB;
  }
  return pos + diff;
}

function renderRowContent(el: HTMLDivElement, segments: readonly LayoutInlineSegment[]): void {
  if (segments.length === 1) {
    const only = segments[0]!;
    if (only.kind === "text" && only.className === undefined && only.style === undefined) {
      el.textContent = only.text;
      return;
    }
  }
  const fragment = document.createDocumentFragment();
  for (const segment of segments) {
    if (segment.kind === "text" && segment.className === undefined && segment.style === undefined) {
      fragment.append(segment.text);
      continue;
    }
    const span = document.createElement("span");
    span.textContent = segment.text;
    const className =
      segment.kind === "virtual-text" ? (segment.className ?? "editor-virtual-text") : segment.className;
    if (className) span.className = className;
    if (segment.style) {
      for (const [key, value] of Object.entries(segment.style)) {
        (span.style as unknown as Record<string, string>)[key] = value;
      }
    }
    fragment.append(span);
  }
  el.replaceChildren(fragment);
}

export class EditorRenderer {
  private readonly viewport: HTMLDivElement;
  private readonly contentEl: HTMLDivElement;
  private readonly rowLayer: HTMLDivElement;
  private readonly selectionLayer: HTMLDivElement;
  private readonly caretEl: HTMLDivElement;
  private readonly scrollbar: HTMLDivElement;
  private readonly thumb: HTMLDivElement;
  private readonly measureEl: HTMLSpanElement;
  private readonly resizeObserver: ResizeObserver;
  private readonly unsubscribe: () => void;
  private readonly renderPass = () => this.render();
  private readonly handleKeyDown = (event: KeyboardEvent) => {
    const outcome = this.runtime.handleKeyboardEvent(event);
    this.options.onPending?.(this.runtime.pendingChords());
    if (outcome !== "passthrough") event.preventDefault();
  };
  private readonly handleMouseDown = (event: MouseEvent) => {
    if (event.target === this.thumb || this.scrollbar.contains(event.target as Node)) return;
    const rect = this.viewport.getBoundingClientRect();
    const hit = hitTestTextPosition({
      text: this.snapshot.text,
      x: event.clientX - rect.left + this.scrollX,
      y: event.clientY - rect.top + this.scrollY,
      lineHeight: LINE_HEIGHT,
      charWidth: this.charWidth,
      paddingLeft: PADDING_LEFT,
      paddingTop: PADDING_TOP,
      tabWidth: TAB_WIDTH,
      bias: "nearest",
    });
    this.runtime.setSelection(Selection.point(hit.offset));
  };
  private readonly handleFocus = () => this.caretEl.classList.remove("editor-caret-idle");
  private readonly handleBlur = () => this.caretEl.classList.add("editor-caret-idle");
  // Owned scroll: consume wheel deltas (momentum arrives as events from the
  // OS). Non-passive so the page never natively scrolls.
  private readonly handleWheel = (event: WheelEvent) => {
    if (event.ctrlKey || event.metaKey) return; // zoom gestures stay native
    event.preventDefault();
    const scale =
      event.deltaMode === 1 ? LINE_HEIGHT : event.deltaMode === 2 ? this.viewportHeight : 1;
    this.setScroll(this.scrollY + event.deltaY * scale, this.scrollX + event.deltaX * scale);
  };
  private readonly handleThumbDown = (event: PointerEvent) => {
    event.preventDefault();
    this.thumb.setPointerCapture(event.pointerId);
    this.dragOffset = event.clientY - this.thumb.getBoundingClientRect().top;
  };
  private readonly handleThumbMove = (event: PointerEvent) => {
    if (this.dragOffset === null) return;
    this.scrollToThumbPosition(event.clientY - this.dragOffset);
  };
  private readonly handleThumbUp = (event: PointerEvent) => {
    this.dragOffset = null;
    this.thumb.releasePointerCapture(event.pointerId);
  };
  private readonly handleTrackDown = (event: PointerEvent) => {
    if (event.target === this.thumb) return;
    event.preventDefault();
    // Jump so the thumb centers on the pointer, then drag from there.
    this.dragOffset = this.thumbHeight() / 2;
    this.thumb.setPointerCapture(event.pointerId);
    this.scrollToThumbPosition(event.clientY - this.dragOffset);
  };

  private snapshot: EditorSnapshot;
  private scrollY = 0;
  private scrollX = 0;
  private windowStart = 0;
  private windowEnd = 0;
  private maxRowChars = 0;
  private contentHeightPx = 0;
  private charWidth = 8;
  private viewportHeight = 0;
  private viewportWidth = 0;
  private lastLineCount: number;
  private dragOffset: number | null = null;
  private lastTransform = "";
  private lastThumbCss = "";
  private anchor: { line: number; offset: number } | null = null;
  private lastSerial = 0;
  private docDirty = false;
  private selDirty = false;
  /** Change sets since the last frame; null = unknown, treat window as dirty. */
  private pendingChangeSets: DocumentChangeRange[][] | null = [];
  private revealCaretPending = false;
  private overlaysDirty = true;
  private disposed = false;
  private readonly debug: RenderDebug | null;

  constructor(
    private readonly root: HTMLElement,
    private readonly runtime: AppRuntime,
    private readonly options: EditorRendererOptions = {},
  ) {
    this.snapshot = runtime.snapshot();
    this.lastLineCount = this.snapshot.text.lineCount;

    this.measureEl = document.createElement("span");
    this.measureEl.className = "editor-measure";
    this.measureEl.setAttribute("aria-hidden", "true");
    this.measureEl.textContent = "0".repeat(100);

    this.viewport = document.createElement("div");
    this.viewport.className = "editor-viewport";
    this.contentEl = document.createElement("div");
    this.contentEl.className = "editor-content";
    this.rowLayer = document.createElement("div");
    this.rowLayer.className = "editor-rows";
    this.selectionLayer = document.createElement("div");
    this.selectionLayer.className = "editor-layer";
    this.caretEl = document.createElement("div");
    this.caretEl.className = "editor-caret editor-caret-idle";
    this.caretEl.style.display = "none";
    this.scrollbar = document.createElement("div");
    this.scrollbar.className = "editor-scrollbar";
    this.thumb = document.createElement("div");
    this.thumb.className = "editor-scrollbar-thumb";

    this.contentEl.append(this.selectionLayer, this.rowLayer, this.caretEl);
    this.scrollbar.append(this.thumb);
    this.viewport.append(this.contentEl);
    root.append(this.measureEl, this.viewport, this.scrollbar);

    this.measureCharWidth();
    // Re-measure once webfonts settle; a wrong width skews hit testing.
    document.fonts?.ready.then(() => {
      if (this.disposed) return;
      this.measureCharWidth();
      this.overlaysDirty = true;
      queueRender(this.renderPass);
    });

    this.viewport.addEventListener("wheel", this.handleWheel, { passive: false });
    this.viewport.addEventListener("mousedown", this.handleMouseDown);
    this.thumb.addEventListener("pointerdown", this.handleThumbDown);
    this.thumb.addEventListener("pointermove", this.handleThumbMove);
    this.thumb.addEventListener("pointerup", this.handleThumbUp);
    this.scrollbar.addEventListener("pointerdown", this.handleTrackDown);
    this.scrollbar.addEventListener("pointermove", this.handleThumbMove);
    this.scrollbar.addEventListener("pointerup", this.handleThumbUp);
    root.addEventListener("keydown", this.handleKeyDown);
    root.addEventListener("focus", this.handleFocus);
    root.addEventListener("blur", this.handleBlur);

    this.resizeObserver = new ResizeObserver(() => queueRender(this.renderPass));
    this.resizeObserver.observe(this.viewport);

    this.unsubscribe = runtime.subscribe((notification) => this.onNotification(notification));

    const isDev =
      typeof location !== "undefined" &&
      (location.hostname === "localhost" || location.hostname === "127.0.0.1");
    this.debug = isDev
      ? new RenderDebug(
          this.viewport,
          () => [this.contentEl],
          () => ({
            windowStart: this.windowStart,
            windowEnd: this.windowEnd,
            windowTopPx: PADDING_TOP + this.windowStart * LINE_HEIGHT,
            windowHeightPx: (this.windowEnd - this.windowStart) * LINE_HEIGHT,
          }),
          () => this.scrollY,
        )
      : null;
    this.debug?.start();
    if (isDev) {
      (window as unknown as Record<string, unknown>).__editorScroll = {
        get: () => ({ y: this.scrollY, x: this.scrollX }),
        set: (y: number, x = 0) => this.setScroll(y, x),
        max: () => this.maxScrollY(),
      };
    }

    queueRender(this.renderPass);
  }

  dispose(): void {
    this.disposed = true;
    this.debug?.stop();
    cancelRender(this.renderPass);
    this.unsubscribe();
    this.resizeObserver.disconnect();
    this.viewport.removeEventListener("wheel", this.handleWheel);
    this.viewport.removeEventListener("mousedown", this.handleMouseDown);
    this.root.removeEventListener("keydown", this.handleKeyDown);
    this.root.removeEventListener("focus", this.handleFocus);
    this.root.removeEventListener("blur", this.handleBlur);
    this.measureEl.remove();
    this.viewport.remove();
    this.scrollbar.remove();
  }

  private measureCharWidth(): void {
    const width = this.measureEl.getBoundingClientRect().width / 100;
    if (width > 0) this.charWidth = width;
  }

  private maxScrollY(): number {
    return Math.max(0, this.contentHeightPx - this.viewportHeight);
  }

  private maxScrollX(): number {
    const contentWidth = PADDING_LEFT + this.maxRowChars * this.charWidth + H_SCROLL_PAD;
    return Math.max(0, contentWidth - this.viewportWidth);
  }

  /** The single entry point for scroll movement (wheel, scrollbar, reveal). */
  private setScroll(y: number, x = this.scrollX): void {
    const ny = Math.max(0, Math.min(this.maxScrollY(), y));
    const nx = Math.max(0, Math.min(this.maxScrollX(), x));
    if (ny === this.scrollY && nx === this.scrollX) return;
    this.scrollY = ny;
    this.scrollX = nx;
    this.debug?.log("scroll", { st: Math.round(ny) });
    queueRender(this.renderPass);
  }

  private thumbHeight(): number {
    const track = this.viewportHeight;
    if (this.contentHeightPx <= track) return 0;
    return Math.max(SCROLLBAR_MIN_THUMB, (track * track) / this.contentHeightPx);
  }

  private scrollToThumbPosition(thumbTopClient: number): void {
    const trackRect = this.scrollbar.getBoundingClientRect();
    const trackSpan = trackRect.height - this.thumbHeight();
    if (trackSpan <= 0) return;
    const ratio = Math.max(0, Math.min(1, (thumbTopClient - trackRect.top) / trackSpan));
    this.setScroll(ratio * this.maxScrollY());
  }

  private onNotification(notification: EditorNotification): void {
    const doc = hasInvalidation(notification, DocumentInvalidated);
    const sel = hasInvalidation(notification, SelectionInvalidated);
    if (!doc && !sel) return;
    if (doc) {
      this.docDirty = true;
      if (this.pendingChangeSets !== null) {
        const gap = this.lastSerial !== 0 && notification.serial > this.lastSerial + 1;
        const payloads = payloadsOf(notification, DocumentInvalidated);
        if (gap || payloads.some((payload) => payload.changes === undefined)) {
          this.pendingChangeSets = null;
        } else {
          this.pendingChangeSets.push(payloads.flatMap((payload) => [...payload.changes!]));
        }
      }
    }
    if (sel) {
      this.selDirty = true;
      this.revealCaretPending = true;
    }
    this.lastSerial = notification.serial;
    queueRender(this.renderPass);
  }

  private render(): void {
    if (this.disposed) return;
    const t0 = this.debug !== null ? performance.now() : 0;
    const docDirty = this.docDirty;
    const selDirty = this.selDirty;
    const changeSets = this.pendingChangeSets;
    this.docDirty = false;
    this.selDirty = false;
    this.pendingChangeSets = [];

    if (docDirty || selDirty) this.snapshot = this.runtime.snapshot();
    const { text, version, selection } = this.snapshot;
    const lineCount = text.lineCount;

    this.contentHeightPx = layoutContentHeight({
      text,
      lineHeight: LINE_HEIGHT,
      paddingTop: PADDING_TOP,
    });
    this.viewportHeight = this.viewport.clientHeight;
    this.viewportWidth = this.viewport.clientWidth;

    // Scroll anchoring: when an edit lands above the first visible line,
    // shift the scroll model so the viewport text holds still.
    if (docDirty && this.anchor !== null && changeSets !== null && changeSets.length > 0) {
      let offset = this.anchor.offset;
      for (const set of changeSets) offset = mapOffset(set, offset);
      const line = text.lineRangeAt(Math.min(offset, text.length)).index;
      this.scrollY += (line - this.anchor.line) * LINE_HEIGHT;
    }

    if (this.revealCaretPending) {
      this.revealCaretPending = false;
      const line = text.lineRangeAt(selection.primary.head).index;
      const caretTop = PADDING_TOP + line * LINE_HEIGHT;
      if (caretTop < this.scrollY) {
        this.scrollY = caretTop;
      } else if (caretTop + LINE_HEIGHT > this.scrollY + this.viewportHeight) {
        this.scrollY = caretTop + LINE_HEIGHT - this.viewportHeight;
      }
    }
    this.scrollY = Math.max(0, Math.min(this.maxScrollY(), this.scrollY));
    this.scrollX = Math.max(0, Math.min(this.maxScrollX(), this.scrollX));

    const scrollY = this.scrollY;
    const visStart = Math.max(0, Math.floor((scrollY - PADDING_TOP) / LINE_HEIGHT));
    const visEnd = Math.min(
      lineCount,
      Math.ceil((scrollY + this.viewportHeight - PADDING_TOP) / LINE_HEIGHT) + 1,
    );

    const slackLines = Math.ceil(WINDOW_SLACK_PX / LINE_HEIGHT);
    const needStart = Math.max(0, visStart - slackLines);
    const needEnd = Math.min(lineCount, visEnd + slackLines);
    const windowOk =
      this.windowEnd > this.windowStart &&
      this.windowStart <= needStart &&
      this.windowEnd >= needEnd &&
      this.windowEnd <= lineCount;

    let stats: { mode: string; added: number; removed: number; rewritten: number } | null = null;
    if (!windowOk || docDirty) {
      let start = this.windowStart;
      let end = this.windowEnd;
      if (!windowOk) {
        const overscanLines = Math.ceil(OVERSCAN_PX / LINE_HEIGHT);
        const targetLines = visEnd - visStart + 2 * overscanLines;
        end = Math.min(lineCount, Math.floor((visStart + visEnd) / 2) + Math.ceil(targetLines / 2));
        start = Math.max(0, end - targetLines);
        end = Math.min(lineCount, start + targetLines);
      }

      const layout = layoutViewport({
        text,
        version,
        startLine: start,
        lineCount: end - start,
        tabWidth: TAB_WIDTH,
        lineHeight: LINE_HEIGHT,
        charWidth: this.charWidth,
        paddingTop: PADDING_TOP,
        paddingLeft: PADDING_LEFT,
      });

      let structureUnknown = false;
      const dirtySpans: Array<readonly [number, number]> = [];
      if (docDirty) {
        if (changeSets === null || changeSets.length > 1 || lineCount !== this.lastLineCount) {
          structureUnknown = true;
        } else if (changeSets.length === 1) {
          for (const range of changeSets[0]!) {
            dirtySpans.push([
              text.lineRangeAt(Math.min(range.fromB, text.length)).index,
              text.lineRangeAt(Math.min(range.toB, text.length)).index,
            ]);
          }
        }
      }

      stats = this.patchRows(layout, structureUnknown, dirtySpans, this.windowStart, this.windowEnd);
      this.windowStart = start;
      this.windowEnd = end;
    }
    this.lastLineCount = lineCount;

    // Position the window: content translates to (windowTop - scroll).
    const windowTop = PADDING_TOP + this.windowStart * LINE_HEIGHT;
    const dpr = window.devicePixelRatio || 1;
    const ty = Math.round((windowTop - scrollY) * dpr) / dpr;
    const tx = Math.round(-this.scrollX * dpr) / dpr;
    const transform = `translate(${tx}px, ${ty}px)`;
    if (transform !== this.lastTransform) {
      this.lastTransform = transform;
      this.contentEl.style.transform = transform;
    }

    // Scrollbar thumb.
    const th = this.thumbHeight();
    const maxY = this.maxScrollY();
    const trackSpan = this.viewportHeight - th;
    const thumbTop = th > 0 && maxY > 0 ? (scrollY / maxY) * trackSpan : 0;
    const thumbCss = `${Math.round(thumbTop)}:${Math.round(th)}`;
    if (thumbCss !== this.lastThumbCss) {
      this.lastThumbCss = thumbCss;
      this.thumb.style.height = `${th}px`;
      this.thumb.style.top = `${thumbTop}px`;
      this.thumb.style.display = th > 0 ? "" : "none";
    }

    if (docDirty || selDirty || this.overlaysDirty || (stats !== null && stats.mode !== "none")) {
      this.renderOverlays(windowTop, selDirty);
      this.overlaysDirty = false;
    }

    // Remember the first fully visible line as the scroll anchor for the
    // next document change.
    const anchorLine = Math.min(
      Math.max(0, Math.ceil((scrollY - PADDING_TOP) / LINE_HEIGHT)),
      lineCount - 1,
    );
    this.anchor = { line: anchorLine, offset: text.lineRange(anchorLine).from };

    if (stats !== null || docDirty || selDirty) {
      this.debug?.log("render", {
        mode: stats?.mode ?? "state",
        st: Math.round(scrollY),
        win: `${this.windowStart}-${this.windowEnd}`,
        added: stats?.added ?? 0,
        removed: stats?.removed ?? 0,
        rewritten: stats?.rewritten ?? 0,
        dur: Math.round((performance.now() - t0) * 10) / 10,
      });
    }

    if (docDirty || selDirty) this.options.onSnapshot?.(this.snapshot);
  }

  /**
   * Reconcile the row elements (in-flow children of rowLayer; child i shows
   * line windowStart + i) with the new layout. Edge trimming: when the
   * window slides, retained rows are untouched — elements are removed from
   * the shrinking edge and fragments inserted at the growing edge.
   */
  private patchRows(
    layout: ViewportLayoutSnapshot<unknown>,
    structureUnknown: boolean,
    dirtySpans: ReadonlyArray<readonly [number, number]>,
    prevStart: number,
    prevEnd: number,
  ): { mode: string; added: number; removed: number; rewritten: number } {
    const layer = this.rowLayer;
    const rows = layout.rows.filter((row) => row.kind === "text");
    const start = layout.startLine;
    const end = start + rows.length;
    for (const row of rows) {
      if (row.text.length > this.maxRowChars) this.maxRowChars = row.text.length;
    }
    const overlapStart = Math.max(start, prevStart);
    const overlapEnd = Math.min(end, prevEnd);
    const canPatch =
      !structureUnknown &&
      overlapEnd > overlapStart &&
      layer.childElementCount === prevEnd - prevStart;

    if (!canPatch) {
      while (layer.childElementCount > rows.length) layer.lastElementChild!.remove();
      for (let i = 0; i < rows.length; i++) {
        let el = layer.children[i] as HTMLDivElement | undefined;
        if (el === undefined) {
          el = this.buildRowEl(rows[i]!.segments);
          layer.append(el);
        } else {
          renderRowContent(el, rows[i]!.segments);
        }
      }
      return { mode: "rebuild", added: rows.length, removed: 0, rewritten: 0 };
    }

    for (let i = overlapStart - prevStart; i > 0; i--) layer.firstElementChild!.remove();
    for (let i = prevEnd - overlapEnd; i > 0; i--) layer.lastElementChild!.remove();
    if (start < overlapStart) {
      const fragment = document.createDocumentFragment();
      for (let line = start; line < overlapStart; line++) {
        fragment.append(this.buildRowEl(rows[line - start]!.segments));
      }
      layer.insertBefore(fragment, layer.firstElementChild);
    }
    if (end > overlapEnd) {
      const fragment = document.createDocumentFragment();
      for (let line = overlapEnd; line < end; line++) {
        fragment.append(this.buildRowEl(rows[line - start]!.segments));
      }
      layer.append(fragment);
    }
    let rewritten = 0;
    for (const [firstLine, lastLine] of dirtySpans) {
      for (let line = Math.max(firstLine, start); line <= Math.min(lastLine, end - 1); line++) {
        renderRowContent(layer.children[line - start] as HTMLDivElement, rows[line - start]!.segments);
        rewritten++;
      }
    }
    const added = Math.max(0, overlapStart - start) + Math.max(0, end - overlapEnd);
    const removed = Math.max(0, overlapStart - prevStart) + Math.max(0, prevEnd - overlapEnd);
    return {
      mode: added + removed + rewritten > 0 ? "patch" : "none",
      added,
      removed,
      rewritten,
    };
  }

  private buildRowEl(segments: readonly LayoutInlineSegment[]): HTMLDivElement {
    const el = document.createElement("div");
    el.className = "editor-row";
    renderRowContent(el, segments);
    return el;
  }

  /** Caret + selection rects, window-local, above the rows. */
  private renderOverlays(windowTop: number, selDirty: boolean): void {
    const { text, version, selection } = this.snapshot;
    const layout = layoutViewport({
      text,
      version,
      startLine: this.windowStart,
      lineCount: this.windowEnd - this.windowStart,
      tabWidth: TAB_WIDTH,
      lineHeight: LINE_HEIGHT,
      charWidth: this.charWidth,
      paddingTop: PADDING_TOP,
      paddingLeft: PADDING_LEFT,
      anchors: [
        { kind: "overlay", id: "caret", at: selection.primary.head, payload: null },
        ...selection.ranges
          .filter((range) => !range.isCollapsed)
          .map((range, index) => ({
            kind: "range" as const,
            id: `selection-${index}`,
            from: range.from,
            to: range.to,
            payload: null,
          })),
      ],
    });

    const selectionRects: HTMLDivElement[] = [];
    let caretPlaced = false;
    for (const placement of layout.placements) {
      if (placement.kind === "range") {
        for (const rect of placement.rects) {
          const el = document.createElement("div");
          el.className = "editor-selection";
          el.style.top = `${rect.top - windowTop}px`;
          el.style.left = `${rect.left}px`;
          el.style.width = `${Math.max(rect.width, 4)}px`;
          el.style.height = `${rect.height}px`;
          selectionRects.push(el);
        }
      } else if (placement.kind === "overlay" && placement.id === "caret") {
        caretPlaced = true;
        this.caretEl.style.display = "";
        this.caretEl.style.top = `${placement.rect.top - windowTop}px`;
        this.caretEl.style.left = `${placement.rect.left}px`;
        this.caretEl.style.height = `${placement.rect.height}px`;
        if (selDirty) {
          // Re-inserting restarts the blink animation, like a fresh caret.
          this.caretEl.remove();
          this.contentEl.append(this.caretEl);
        }
      }
    }
    if (!caretPlaced) this.caretEl.style.display = "none";
    this.selectionLayer.replaceChildren(...selectionRects);
  }
}
