/**
 * EditorRenderer: the vanilla-DOM rendering hot path, with OWNED SCROLLING
 * and the visuals pipeline.
 *
 * Scrolling (the result of a long traced investigation): there is no native
 * scroller. A scroll model is driven by a non-passive wheel listener and a
 * custom scrollbar (pointer capture); the rendered window of in-flow rows is
 * positioned with a transform. Scroll offset and content commit atomically
 * each frame, so nothing can race raster — every artifact class we measured
 * with native scrolling (throttled commits, checkerboard, nested-runloop
 * starvation) is structurally absent. Never reintroduce native scrolling,
 * sticky clamps, or scroll throttles here.
 *
 * Visuals (the extensibility contract): everything anchored to text — text
 * styles, decorations, virtual text, block widgets, gutter bands — comes
 * from the EditorVisuals service as windowed queries over engine RangeSets.
 * The per-frame budget is O(window + log totalVisuals) regardless of how
 * much extensions contribute:
 *
 * - Pure scrolling renders nothing new and queries nothing.
 * - Window slides query only the new window's offsets.
 * - VisualsInvalidated{changes} feeds the same per-row dirty machinery as
 *   document edits: only touched rows rewrite.
 * - Block widgets reserve integer rows; the renderer creates a mount element
 *   of exactly that size when the widget enters the window and announces it
 *   on the MountHub. Extension content portals in on its own schedule — a
 *   slow extension shows a stale/empty box, never scroll jank.
 * - Gutter bands are width reservations; the band layer is x-pinned (it
 *   never gets the horizontal transform) and its cells rebuild only when
 *   the window or the bands change.
 * - ALL vertical geometry goes through GeometryModel (line↔y with widget
 *   prefix sums). No line-arithmetic shortcuts: they break the moment one
 *   widget exists.
 */

import {
  GeometryModel,
  DocumentInvalidated,
  hasInvalidation,
  hitTestTextPosition,
  hitTestTextPositionExact,
  layoutViewport,
  virtualColsBefore,
  payloadsOf,
  SelectionInvalidated,
  FileSyncInvalidated,
  VisualsInvalidated,
  type AnchoredBlockWidget,
  type DocumentChangeRange,
  type EditorNotification,
  type EditorSnapshot,
  type LayoutInlineSegment,
  type ResolvedGutterBand,
  type TextStyleRun,
  type ViewportLayoutSnapshot,
  type VisualsSnapshot,
} from "@the-editor/editor";
import { resolveSyntaxStyle } from "../theme/dom";
import { Selection, type RenderableText } from "@the-editor/engine";
import type { PaneHost } from "../runtime";
import { cancelRender, queueRender } from "./frame";

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
/** Inner padding inside gutter band cells, px. */
const BAND_CELL_PAD = 8;

export type EditorRendererOptions = {
  readonly onSnapshot?: (snapshot: EditorSnapshot) => void;
  readonly focused?: boolean;
};

type OffsetRange = { readonly from: number; readonly to: number };

type ResolvedBand = ResolvedGutterBand & { readonly x: number; readonly widthPx: number };

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

function visualColumn(text: string): number {
  let column = 0;
  for (const ch of text) {
    if (ch === "\t") {
      column += TAB_WIDTH - (column % TAB_WIDTH);
    } else {
      column++;
    }
  }
  return column;
}

// Mouse selection helpers --------------------------------------------------

const WORD_CHAR = /[\p{L}\p{N}_$]/u;
type CharKind = "word" | "space" | "punct";
const charKind = (ch: string): CharKind =>
  /\s/.test(ch) ? "space" : WORD_CHAR.test(ch) ? "word" : "punct";
const kindRank = (kind: CharKind): number => (kind === "word" ? 2 : kind === "punct" ? 1 : 0);

/**
 * The "word" surrounding an offset for double-click selection — a run of the
 * same character kind (word/punct), Zed's `surrounding_word`. When the offset
 * sits between two kinds, the higher-priority side wins (word > punct); never
 * crosses a line break.
 */
function wordRangeAt(text: RenderableText, offset: number): OffsetRange {
  const len = text.length;
  const before = offset > 0 ? text.sliceString(offset - 1, offset) : "";
  const after = offset < len ? text.sliceString(offset, offset + 1) : "";
  const beforeKind = before === "" || before === "\n" ? null : charKind(before);
  const afterKind = after === "" || after === "\n" ? null : charKind(after);
  let kind: CharKind | null;
  if (beforeKind !== null && afterKind !== null) {
    kind = kindRank(beforeKind) >= kindRank(afterKind) ? beforeKind : afterKind;
  } else {
    kind = beforeKind ?? afterKind;
  }
  if (kind === null) return { from: offset, to: offset };
  let from = offset;
  let to = offset;
  while (from > 0) {
    const ch = text.sliceString(from - 1, from);
    if (ch === "\n" || charKind(ch) !== kind) break;
    from--;
  }
  while (to < len) {
    const ch = text.sliceString(to, to + 1);
    if (ch === "\n" || charKind(ch) !== kind) break;
    to++;
  }
  return { from, to };
}

/** The whole line including its trailing break (so triple-click selects the
 * line and the selection wraps to the next line's start), clamped at EOF. */
function lineSelectRange(text: RenderableText, offset: number): OffsetRange {
  const line = text.lineRangeAt(offset);
  const to = line.index < text.lineCount - 1 ? text.lineRange(line.index + 1).from : line.to;
  return { from: line.from, to };
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
      segment.kind === "virtual-text"
        ? (segment.className ?? "editor-virtual-text")
        : segment.className;
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
  private readonly decoBgLayer: HTMLDivElement;
  private readonly selectionLayer: HTMLDivElement;
  private readonly rowLayer: HTMLDivElement;
  private readonly decoFgLayer: HTMLDivElement;
  private readonly caretEl: HTMLDivElement;
  private readonly bandHost: HTMLDivElement;
  private readonly vScrollbar: HTMLDivElement;
  private readonly vThumb: HTMLDivElement;
  private readonly hScrollbar: HTMLDivElement;
  private readonly hThumb: HTMLDivElement;
  private readonly measureEl: HTMLSpanElement;
  private readonly resizeObserver: ResizeObserver;
  private readonly unsubscribe: () => void;
  private readonly unsubscribeHighlights: () => void;
  private readonly unsubscribeFloats: () => void;
  private readonly unsubscribeTheme: () => void;
  private readonly unsubscribeFont: () => void;
  private readonly renderPass = () => this.render();
  /** Active mouse selection: the fixed "anchor unit" and the click mode. The
   * anchor is a single offset for char mode, or the clicked word/line range
   * for word/line mode (so drag extends by whole words/lines, Zed-style). */
  private drag: { mode: "char" | "word" | "line"; anchor: OffsetRange } | null = null;
  private readonly handleMouseDown = (event: MouseEvent) => {
    if (event.button !== 0) return; // left button only
    if (
      this.vScrollbar.contains(event.target as Node) ||
      this.hScrollbar.contains(event.target as Node)
    )
      return;
    // Clicks inside a float (hover card, completion list) belong to the
    // float's own UI — moving the caret would kill text selection in it.
    if ((event.target as HTMLElement).closest?.(".editor-float") !== null) return;
    // Block widgets own their own controls. A header chevron lives inside the
    // gutter x-range, but it must fold the multibuffer entry, not trigger the
    // gutter's "open source line" action or move the caret.
    if ((event.target as HTMLElement).closest?.(".editor-widget") !== null) return;
    // Own the gesture: stop the browser starting a native selection over our
    // row DOM (we render our own selection from the engine model).
    event.preventDefault();
    // Put the keyboard in this pane so the workbench knows the editor was
    // activated; preventScroll keeps the viewport from jumping on focus.
    this.root.focus({ preventScroll: true });
    // A click in the gutter (left of the text) can be claimed by the pane
    // (the multibuffer jumps to that source line); fall through otherwise.
    if (this.runtime.gutterClick !== undefined) {
      const rect = this.viewport.getBoundingClientRect();
      if (event.clientX - rect.left < this.textOriginX) {
        const line = this.geometry.lineAt(event.clientY - rect.top + this.scrollY);
        if (this.runtime.gutterClick(line)) return;
      }
    }
    const offset = this.offsetAt(event.clientX, event.clientY);
    const text = this.snapshot.text;

    // Shift+click extends the current selection from its existing anchor.
    if (event.shiftKey) {
      const anchor = this.snapshot.selection.primary.anchor;
      this.drag = { mode: "char", anchor: { from: anchor, to: anchor } };
      this.runtime.setSelection(Selection.single(anchor, offset));
      this.beginDragTracking();
      return;
    }

    // event.detail is the platform click count (1 single, 2 double, …).
    const clickCount = event.detail;
    if (clickCount >= 4) {
      this.drag = null; // quadruple-click selects all; nothing to drag
      this.runtime.setSelection(Selection.single(0, text.length));
      return;
    }
    if (clickCount === 3) {
      const line = lineSelectRange(text, offset);
      this.drag = { mode: "line", anchor: line };
      this.runtime.setSelection(Selection.single(line.from, line.to));
    } else if (clickCount === 2) {
      const word = wordRangeAt(text, offset);
      this.drag = { mode: "word", anchor: word };
      this.runtime.setSelection(Selection.single(word.from, word.to));
    } else {
      this.drag = { mode: "char", anchor: { from: offset, to: offset } };
      this.runtime.setSelection(Selection.point(offset));
    }
    this.beginDragTracking();
  };

  private beginDragTracking(): void {
    // Window-level so the drag keeps extending and releases even when the
    // pointer leaves the viewport. Same ref → addEventListener de-dupes.
    window.addEventListener("mousemove", this.handleDragMove);
    window.addEventListener("mouseup", this.handleDragEnd);
  }

  /** Pixel → document offset, clamped to a valid position ("nearest" bias). */
  private offsetAt(clientX: number, clientY: number): number {
    const rect = this.viewport.getBoundingClientRect();
    // Vertical geometry goes through the model (widget-aware); the column
    // resolver gets a synthetic widget-less y for the resolved line.
    const line = this.geometry.lineAt(clientY - rect.top + this.scrollY);
    const hit = hitTestTextPosition({
      text: this.snapshot.text,
      x: clientX - rect.left + this.scrollX,
      y: PADDING_TOP + line * LINE_HEIGHT + 1,
      lineHeight: LINE_HEIGHT,
      charWidth: this.charWidth,
      paddingLeft: this.textOriginX,
      paddingTop: PADDING_TOP,
      tabWidth: TAB_WIDTH,
      bias: "nearest",
      virtualText: this.lineVirtuals(line),
    });
    return hit.offset;
  }

  private readonly handleDragMove = (event: MouseEvent) => {
    if (this.drag === null) return;
    this.lastDragClientX = event.clientX;
    this.lastDragClientY = event.clientY;
    this.updateDragSelection(event.clientX, event.clientY);
    this.updateAutoscroll(event.clientY);
  };

  private readonly handleDragEnd = () => {
    this.drag = null;
    this.autoscrollVel = 0;
    if (this.autoscrollRaf !== 0) {
      cancelAnimationFrame(this.autoscrollRaf);
      this.autoscrollRaf = 0;
    }
    window.removeEventListener("mousemove", this.handleDragMove);
    window.removeEventListener("mouseup", this.handleDragEnd);
  };

  /** Extend the live selection to the pointer. The pointer is clamped into the
   * viewport so dragging past an edge resolves the edge line (autoscroll then
   * advances it), instead of jumping straight to the document end. */
  private updateDragSelection(clientX: number, clientY: number): void {
    if (this.drag === null) return;
    const rect = this.viewport.getBoundingClientRect();
    const x = Math.max(rect.left, Math.min(rect.right - 1, clientX));
    const y = Math.max(rect.top, Math.min(rect.bottom - 1, clientY));
    const offset = this.offsetAt(x, y);
    const { mode, anchor } = this.drag;
    if (mode === "char") {
      this.runtime.setSelection(Selection.single(anchor.from, offset));
      return;
    }
    // Word/line: extend by whole units. The fixed end is the anchor unit; the
    // head snaps to the unit boundary on the side the pointer is dragging to.
    const unit = mode === "word" ? wordRangeAt(this.snapshot.text, offset) : lineSelectRange(this.snapshot.text, offset);
    if (unit.from < anchor.from) {
      this.runtime.setSelection(Selection.single(anchor.to, unit.from)); // reversed
    } else {
      this.runtime.setSelection(Selection.single(anchor.from, unit.to));
    }
  }

  private autoscrollRaf = 0;
  private autoscrollVel = 0;
  private lastDragClientX = 0;
  private lastDragClientY = 0;
  /** Scroll speed (px/frame) for a pointer `dist` px past the autoscroll
   * margin — a gentle ramp, capped so it never outruns the eye. */
  private static autoscrollSpeed(dist: number): number {
    return Math.min(LINE_HEIGHT * 2, 2 + dist * 0.4);
  }

  private updateAutoscroll(clientY: number): void {
    const rect = this.viewport.getBoundingClientRect();
    const margin = Math.min(LINE_HEIGHT, rect.height / 3);
    let vel = 0;
    if (clientY < rect.top + margin) {
      vel = -EditorRenderer.autoscrollSpeed(rect.top + margin - clientY);
    } else if (clientY > rect.bottom - margin) {
      vel = EditorRenderer.autoscrollSpeed(clientY - (rect.bottom - margin));
    }
    this.autoscrollVel = vel;
    if (vel !== 0 && this.autoscrollRaf === 0) {
      this.autoscrollRaf = requestAnimationFrame(this.autoscrollTick);
    }
  }

  private readonly autoscrollTick = () => {
    this.autoscrollRaf = 0;
    if (this.drag === null || this.autoscrollVel === 0) return;
    const before = this.scrollY;
    this.setScroll(this.scrollY + this.autoscrollVel);
    // Re-extend at the (held) pointer; the newly revealed edge line is now
    // under the clamped pointer, so the selection grows as we scroll.
    this.updateDragSelection(this.lastDragClientX, this.lastDragClientY);
    if (this.scrollY !== before) this.autoscrollRaf = requestAnimationFrame(this.autoscrollTick);
  };
  private hoverRaf = 0;
  private lastPointerClient: { x: number; y: number } | null = null;
  private readonly handleMouseMove = (event: MouseEvent) => {
    if (this.runtime.pointerHover === undefined) return;
    // Dragging a selection — the window-level handler owns the pointer; don't
    // also arm hover.
    if (this.drag !== null) return;
    // Chromium re-synthesizes mousemove at the SAME client position when
    // the DOM changes under a stationary cursor (scrolling rebuilds rows).
    // Those are not user movement: consecutive identical coordinates never
    // arm or re-arm hover. Zed gets this for free (OS-level moves only).
    if (
      this.lastPointerClient !== null &&
      this.lastPointerClient.x === event.clientX &&
      this.lastPointerClient.y === event.clientY
    ) {
      return;
    }
    this.lastPointerClient = { x: event.clientX, y: event.clientY };
    // Inside a float the pointer is interacting with the float's own UI
    // (selecting hover-card text); freeze hover state rather than
    // re-hit-testing the text underneath it.
    if ((event.target as HTMLElement).closest?.(".editor-float") !== null) return;
    if (this.hoverRaf !== 0) return;
    const x = event.clientX;
    const y = event.clientY;
    this.hoverRaf = requestAnimationFrame(() => {
      this.hoverRaf = 0;
      if (this.disposed) return;
      const rect = this.viewport.getBoundingClientRect();
      if (x < rect.left || x > rect.right || y < rect.top || y > rect.bottom) {
        this.runtime.pointerHover?.(null);
        return;
      }
      // Hover is EXACT: empty space right of a line, widget rows, and the
      // void past the last line report null instead of the nearest offset.
      const contentY = y - rect.top + this.scrollY;
      const line = this.geometry.lineAt(contentY);
      const lineTop = this.geometry.lineTop(line);
      if (contentY < lineTop || contentY >= lineTop + LINE_HEIGHT) {
        this.runtime.pointerHover?.(null);
        return;
      }
      const hit = hitTestTextPositionExact({
        text: this.snapshot.text,
        x: x - rect.left + this.scrollX,
        y: PADDING_TOP + line * LINE_HEIGHT + 1,
        lineHeight: LINE_HEIGHT,
        charWidth: this.charWidth,
        paddingLeft: this.textOriginX,
        paddingTop: PADDING_TOP,
        tabWidth: TAB_WIDTH,
        virtualText: this.lineVirtuals(line),
      });
      this.runtime.pointerHover?.(hit?.offset ?? null);
    });
  };
  private readonly handleMouseLeave = () => {
    this.runtime.pointerHover?.(null);
  };
  private readonly handleNativeScrollLeak = () => {
    if (this.viewport.scrollLeft !== 0) this.viewport.scrollLeft = 0;
    if (this.viewport.scrollTop !== 0) this.viewport.scrollTop = 0;
  };
  private readonly handleWheel = (event: WheelEvent) => {
    if (event.ctrlKey || event.metaKey) return; // zoom gestures stay native
    event.preventDefault();
    const scale =
      event.deltaMode === 1 ? LINE_HEIGHT : event.deltaMode === 2 ? this.viewportHeight : 1;
    this.setScroll(this.scrollY + event.deltaY * scale, this.scrollX + event.deltaX * scale);
  };
  private readonly handleVThumbDown = (event: PointerEvent) => {
    event.preventDefault();
    this.vThumb.setPointerCapture(event.pointerId);
    this.vDragOffset = event.clientY - this.vThumb.getBoundingClientRect().top;
  };
  private readonly handleVThumbMove = (event: PointerEvent) => {
    if (this.vDragOffset === null) return;
    this.scrollToVThumbPosition(event.clientY - this.vDragOffset);
  };
  private readonly handleVThumbUp = (event: PointerEvent) => {
    this.vDragOffset = null;
    this.vThumb.releasePointerCapture(event.pointerId);
  };
  private readonly handleVTrackDown = (event: PointerEvent) => {
    if (event.target === this.vThumb) return;
    event.preventDefault();
    this.vDragOffset = this.vThumbHeight() / 2;
    this.vThumb.setPointerCapture(event.pointerId);
    this.scrollToVThumbPosition(event.clientY - this.vDragOffset);
  };
  private readonly handleHThumbDown = (event: PointerEvent) => {
    event.preventDefault();
    this.hThumb.setPointerCapture(event.pointerId);
    this.hDragOffset = event.clientX - this.hThumb.getBoundingClientRect().left;
  };
  private readonly handleHThumbMove = (event: PointerEvent) => {
    if (this.hDragOffset === null) return;
    this.scrollToHThumbPosition(event.clientX - this.hDragOffset);
  };
  private readonly handleHThumbUp = (event: PointerEvent) => {
    this.hDragOffset = null;
    this.hThumb.releasePointerCapture(event.pointerId);
  };
  private readonly handleHTrackDown = (event: PointerEvent) => {
    if (event.target === this.hThumb) return;
    event.preventDefault();
    this.hDragOffset = this.hThumbWidth() / 2;
    this.hThumb.setPointerCapture(event.pointerId);
    this.scrollToHThumbPosition(event.clientX - this.hDragOffset);
  };

  private snapshot: EditorSnapshot;
  private visuals: VisualsSnapshot | null = null;
  private widgets: AnchoredBlockWidget[] = [];
  private geometry: GeometryModel;
  private bands: ResolvedBand[] = [];
  private bandsKey = "";
  private textOriginX = PADDING_LEFT;
  private scrollY = 0;
  private scrollX = 0;
  private windowStart = 0;
  private windowEnd = 0;
  /** line → rowLayer child index; null means identity (no widgets in window). */
  private rowIndexByLine: Map<number, number> | null = null;
  private windowHasWidgets = false;
  /** Live widget mount elements by widget id (identity survives rebuilds). */
  private readonly mountEls = new Map<string, HTMLDivElement>();
  private maxRowChars = 0;
  private contentHeightPx = 0;
  private charWidth = 8;
  private viewportHeight = 0;
  private viewportWidth = 0;
  private lastLineCount: number;
  private vDragOffset: number | null = null;
  private hDragOffset: number | null = null;
  private lastTransform = "";
  private lastBandTransform = "";
  private lastViewportVars = "";
  private lastVThumbCss = "";
  private lastHThumbCss = "";
  private anchor: { offset: number; topPx: number } | null = null;
  private lastSerial = 0;
  private docDirty = false;
  private selDirty = false;
  private visualsDirty = true;
  /** Offset ranges from VisualsInvalidated; null = unknown, rebuild window. */
  private pendingVisualChanges: OffsetRange[] | null = [];
  /** Change sets since the last frame; null = unknown, treat window as dirty. */
  private pendingChangeSets: DocumentChangeRange[][] | null = [];
  private revealCaretPending = false;
  private overlaysDirty = true;
  private disposed = false;

  constructor(
    private readonly root: HTMLElement,
    private readonly runtime: PaneHost,
    private readonly options: EditorRendererOptions = {},
  ) {
    this.snapshot = runtime.snapshot();
    this.lastLineCount = this.snapshot.text.lineCount;
    this.geometry = new GeometryModel(this.lastLineCount, LINE_HEIGHT, PADDING_TOP);

    this.measureEl = document.createElement("span");
    this.measureEl.className = "editor-measure";
    this.measureEl.setAttribute("aria-hidden", "true");
    this.measureEl.textContent = "0".repeat(100);

    this.viewport = document.createElement("div");
    this.viewport.className = "editor-viewport";
    this.contentEl = document.createElement("div");
    this.contentEl.className = "editor-content";
    this.decoBgLayer = document.createElement("div");
    this.decoBgLayer.className = "editor-layer";
    this.selectionLayer = document.createElement("div");
    this.selectionLayer.className = "editor-layer";
    this.rowLayer = document.createElement("div");
    this.rowLayer.className = "editor-rows";
    this.root.style.setProperty("--editor-text-origin-x", `${this.textOriginX}px`);
    this.decoFgLayer = document.createElement("div");
    this.decoFgLayer.className = "editor-layer editor-layer-fg";
    this.caretEl = document.createElement("div");
    this.caretEl.className = this.options.focused === true
      ? "editor-caret"
      : "editor-caret editor-caret-idle";
    this.caretEl.style.display = "none";
    this.bandHost = document.createElement("div");
    this.bandHost.className = "editor-bands";
    this.vScrollbar = document.createElement("div");
    this.vScrollbar.className = "editor-scrollbar editor-scrollbar-vertical";
    this.vThumb = document.createElement("div");
    this.vThumb.className = "editor-scrollbar-thumb editor-scrollbar-thumb-vertical";
    this.hScrollbar = document.createElement("div");
    this.hScrollbar.className = "editor-scrollbar editor-scrollbar-horizontal";
    this.hThumb = document.createElement("div");
    this.hThumb.className = "editor-scrollbar-thumb editor-scrollbar-thumb-horizontal";

    this.contentEl.append(
      this.decoBgLayer,
      this.selectionLayer,
      this.rowLayer,
      this.decoFgLayer,
      this.caretEl,
    );
    this.vScrollbar.append(this.vThumb);
    this.hScrollbar.append(this.hThumb);
    this.viewport.append(this.contentEl, this.bandHost);
    root.append(this.measureEl, this.viewport, this.vScrollbar, this.hScrollbar);

    this.measureCharWidth();
    document.fonts?.ready.then(() => {
      if (this.disposed) return;
      this.measureCharWidth();
      this.overlaysDirty = true;
      this.visualsDirty = true; // band widths derive from charWidth
      queueRender(this.renderPass);
    });

    this.viewport.addEventListener("wheel", this.handleWheel, { passive: false });
    this.viewport.addEventListener("mousedown", this.handleMouseDown);
    this.vThumb.addEventListener("pointerdown", this.handleVThumbDown);
    this.vThumb.addEventListener("pointermove", this.handleVThumbMove);
    this.vThumb.addEventListener("pointerup", this.handleVThumbUp);
    this.vScrollbar.addEventListener("pointerdown", this.handleVTrackDown);
    this.vScrollbar.addEventListener("pointermove", this.handleVThumbMove);
    this.vScrollbar.addEventListener("pointerup", this.handleVThumbUp);
    this.hThumb.addEventListener("pointerdown", this.handleHThumbDown);
    this.hThumb.addEventListener("pointermove", this.handleHThumbMove);
    this.hThumb.addEventListener("pointerup", this.handleHThumbUp);
    this.hScrollbar.addEventListener("pointerdown", this.handleHTrackDown);
    this.hScrollbar.addEventListener("pointermove", this.handleHThumbMove);
    this.hScrollbar.addEventListener("pointerup", this.handleHThumbUp);

    this.resizeObserver = new ResizeObserver(() => queueRender(this.renderPass));
    this.resizeObserver.observe(this.viewport);

    this.viewport.addEventListener("mousemove", this.handleMouseMove);
    this.viewport.addEventListener("mouseleave", this.handleMouseLeave);
    // The viewport is overflow:hidden but the browser can still scroll it
    // NATIVELY (scrollIntoView walking ancestors, focus reveal heuristics)
    // — invisible to the owned-scroll model. Snap any such drift back.
    this.viewport.addEventListener("scroll", this.handleNativeScrollLeak);
    this.unsubscribeFloats = runtime.floats.onChanged(() => {
      this.syncFloats();
      queueRender(this.renderPass);
    });
    this.syncFloats();

    this.unsubscribe = runtime.subscribe((notification, source) =>
      this.onNotification(notification, source),
    );
    // Fresh highlight data: rebuild the window (cheap, parse-paced).
    this.unsubscribeHighlights =
      runtime.highlights?.subscribe(() => {
        this.visualsDirty = true;
        this.pendingVisualChanges = null;
        queueRender(this.renderPass);
      }) ?? (() => {});
    this.unsubscribeTheme = runtime.subscribeTheme(() => {
      this.visualsDirty = true;
      this.pendingVisualChanges = null;
      queueRender(this.renderPass);
    });
    this.unsubscribeFont = runtime.subscribeFont(() => {
      document.fonts?.ready.then(() => {
        if (this.disposed) return;
        this.measureCharWidth();
        this.visualsDirty = true;
        this.pendingVisualChanges = null;
        this.overlaysDirty = true;
        queueRender(this.renderPass);
      });
    });

    queueRender(this.renderPass);
  }

  setFocused(focused: boolean): void {
    this.caretEl.classList.toggle("editor-caret-idle", !focused);
  }

  dispose(): void {
    this.disposed = true;
    cancelRender(this.renderPass);
    this.unsubscribe();
    this.unsubscribeHighlights();
    this.unsubscribeTheme();
    this.unsubscribeFont();
    this.unsubscribeFloats();
    if (this.hoverRaf !== 0) cancelAnimationFrame(this.hoverRaf);
    this.handleDragEnd(); // drop any in-flight selection drag + window listeners
    this.viewport.removeEventListener("mousemove", this.handleMouseMove);
    this.viewport.removeEventListener("mouseleave", this.handleMouseLeave);
    this.viewport.removeEventListener("scroll", this.handleNativeScrollLeak);
    this.resizeObserver.disconnect();
    this.viewport.removeEventListener("wheel", this.handleWheel);
    this.viewport.removeEventListener("mousedown", this.handleMouseDown);
    for (const id of this.mountEls.keys()) this.runtime.mounts.unmounted(id);
    this.mountEls.clear();
    this.measureEl.remove();
    this.viewport.remove();
    this.vScrollbar.remove();
    this.hScrollbar.remove();
  }

  /** Attach float elements (popovers) to the editor root. */
  private syncFloats(): void {
    for (const entry of this.runtime.floats.all()) {
      if (entry.element.parentElement !== this.root) this.root.append(entry.element);
    }
  }

  /** Anchor floats to their text offsets; flip when out of room. */
  private positionFloats(): void {
    const text = this.snapshot.text;
    for (const entry of this.runtime.floats.all()) {
      const anchor = entry.anchor;
      if (anchor === null) continue;
      const offset = Math.max(0, Math.min(anchor.offset, text.length));
      const lineRange = text.lineRangeAt(offset);
      const column = visualColumn(text.sliceString(lineRange.from, offset));
      const lineTop = this.geometry.lineTop(lineRange.index) - this.scrollY;
      let left = this.textOriginX + column * this.charWidth - this.scrollX;
      const height = entry.element.offsetHeight;
      const width = entry.element.offsetWidth;
      let top =
        anchor.placement === "below" ? lineTop + LINE_HEIGHT + 4 : lineTop - height - 4;
      // Flip when the preferred side runs out of viewport.
      if (anchor.placement === "below" && top + height > this.viewportHeight && lineTop > height) {
        top = lineTop - height - 4;
      } else if (anchor.placement === "above" && top < 0) {
        top = lineTop + LINE_HEIGHT + 4;
      }
      left = Math.max(4, Math.min(left, this.viewportWidth - width - 4));
      entry.element.style.left = `${Math.round(left)}px`;
      entry.element.style.top = `${Math.round(top)}px`;
    }
  }

  /** Cached worker highlight runs as low-priority inline color styles. */
  private syntaxStyles(from: number, to: number): TextStyleRun[] {
    const highlights = this.runtime.highlights;
    if (highlights === undefined) return [];
    const out: TextStyleRun[] = [];
    for (const run of highlights.runsBetween(from, to)) {
      const style = resolveSyntaxStyle(run.kind);
      if (style?.color === undefined) continue;
      out.push({ from: run.from, to: run.to, style: { color: style.color }, priority: -100 });
    }
    return out;
  }

  /** Virtual text anchored on a line (caret/hit geometry must see it). */
  private lineVirtuals(line: number) {
    if (this.visuals === null) return [];
    const text = this.snapshot.text;
    const range = text.lineRange(Math.max(0, Math.min(line, text.lineCount - 1)));
    return this.visuals.virtualTextBetween(range.from, range.to);
  }

  private measureCharWidth(): void {
    const width = this.measureEl.getBoundingClientRect().width / 100;
    if (width > 0) this.charWidth = width;
  }

  private maxScrollY(): number {
    return Math.max(0, this.contentHeightPx - this.viewportHeight);
  }

  private maxScrollX(): number {
    const contentWidth = this.textOriginX + this.maxRowChars * this.charWidth + H_SCROLL_PAD;
    return Math.max(0, contentWidth - this.viewportWidth);
  }

  private setScroll(y: number, x = this.scrollX): void {
    const ny = Math.max(0, Math.min(this.maxScrollY(), y));
    const nx = Math.max(0, Math.min(this.maxScrollX(), x));
    if (ny === this.scrollY && nx === this.scrollX) return;
    this.scrollY = ny;
    this.scrollX = nx;
    // Every scroll position change hides hover immediately (Zed's model);
    // it re-arms only on a REAL mouse move afterwards.
    this.runtime.pointerScrolled?.();
    queueRender(this.renderPass);
  }

  private vThumbHeight(): number {
    const track = this.viewportHeight;
    if (this.contentHeightPx <= track) return 0;
    return Math.max(SCROLLBAR_MIN_THUMB, (track * track) / this.contentHeightPx);
  }

  private hThumbWidth(): number {
    const contentWidth = this.textOriginX + this.maxRowChars * this.charWidth + H_SCROLL_PAD;
    const track = this.viewportWidth;
    if (contentWidth <= track) return 0;
    return Math.max(SCROLLBAR_MIN_THUMB, (track * track) / contentWidth);
  }

  private scrollToVThumbPosition(thumbTopClient: number): void {
    const trackRect = this.vScrollbar.getBoundingClientRect();
    const trackSpan = trackRect.height - this.vThumbHeight();
    if (trackSpan <= 0) return;
    const ratio = Math.max(0, Math.min(1, (thumbTopClient - trackRect.top) / trackSpan));
    this.setScroll(ratio * this.maxScrollY());
  }

  private scrollToHThumbPosition(thumbLeftClient: number): void {
    const trackRect = this.hScrollbar.getBoundingClientRect();
    const trackSpan = trackRect.width - this.hThumbWidth();
    if (trackSpan <= 0) return;
    const ratio = Math.max(0, Math.min(1, (thumbLeftClient - trackRect.left) / trackSpan));
    this.setScroll(this.scrollY, ratio * this.maxScrollX());
  }

  private onNotification(notification: EditorNotification, source: "document" | "view"): void {
    const doc = hasInvalidation(notification, DocumentInvalidated);
    const sel = hasInvalidation(notification, SelectionInvalidated);
    const vis = hasInvalidation(notification, VisualsInvalidated);
    if (!doc && !sel && !vis) {
      // Not render-relevant, but file sync transitions refresh host
      // surfaces (status bar) through the snapshot callback. Fresh object:
      // hosts may bail on identical references.
      if (hasInvalidation(notification, FileSyncInvalidated)) {
        this.options.onSnapshot?.(this.runtime.snapshot());
      }
      return;
    }
    if (doc) {
      this.docDirty = true;
      if (this.pendingChangeSets !== null) {
        // Serials are per bus; document/visuals invalidations ride the
        // document bus, so the gap check tracks that bus only.
        const gap = this.lastSerial !== 0 && notification.serial > this.lastSerial + 1;
        const payloads = payloadsOf(notification, DocumentInvalidated);
        if (gap || payloads.some((payload) => payload.changes === undefined)) {
          this.pendingChangeSets = null;
        } else {
          this.pendingChangeSets.push(payloads.flatMap((payload) => [...payload.changes!]));
        }
      }
    }
    if (vis) {
      this.visualsDirty = true;
      if (this.pendingVisualChanges !== null) {
        const payloads = payloadsOf(notification, VisualsInvalidated);
        if (payloads.some((payload) => payload.changes === undefined)) {
          this.pendingVisualChanges = null;
        } else {
          this.pendingVisualChanges.push(...payloads.flatMap((payload) => [...payload.changes!]));
        }
      }
    }
    if (sel) {
      this.selDirty = true;
      // Reveal only follows deliberate caret moves on this view's own bus,
      // not selection drift mapped through other views' edits.
      if (source === "view") this.revealCaretPending = true;
    }
    if (source === "document") this.lastSerial = notification.serial;
    queueRender(this.renderPass);
  }

  private resolveBands(): void {
    const specs = [...(this.visuals?.gutterBands ?? [])].sort(
      (a, b) => (a.priority ?? 0) - (b.priority ?? 0),
    );
    const key = specs
      .map((band) => `${band.id}:${band.side}:${band.widthCh}:${band.widthPx ?? ""}`)
      .join("|");
    if (key === this.bandsKey) {
      // Same geometry, but contributions may have replaced their CELL
      // closures (content-mutable bands like VCS diff markers) — refresh
      // them without rebuilding columns.
      this.bands = this.bands.map((band) => {
        const spec = specs.find((s) => s.id === band.id && s.side === band.side);
        return spec !== undefined ? { ...spec, x: band.x, widthPx: band.widthPx } : band;
      });
      return;
    }
    this.bandsKey = key;
    let x = 0;
    this.bands = specs
      .filter((band) => band.side === "left")
      .map((band) => {
        const widthPx =
          band.widthPx ?? Math.ceil(band.widthCh * this.charWidth) + BAND_CELL_PAD * 2;
        const resolved = { ...band, x, widthPx };
        x += widthPx;
        return resolved;
      });
    this.textOriginX = x + PADDING_LEFT;
    this.root.style.setProperty("--editor-text-origin-x", `${this.textOriginX}px`);
    // Rebuild band columns.
    const columns = this.bands.map((band) => {
      const el = document.createElement("div");
      el.className = "editor-band";
      el.dataset.bandId = band.id;
      el.style.left = `${band.x}px`;
      el.style.width = `${band.widthPx}px`;
      return el;
    });
    this.bandHost.replaceChildren(...columns);
    // Geometry of overlays and rows changed.
    this.windowEnd = this.windowStart; // force window rebuild
    this.overlaysDirty = true;
  }

  private render(): void {
    if (this.disposed) return;
    const docDirty = this.docDirty;
    const selDirty = this.selDirty;
    const visualsDirty = this.visualsDirty;
    const changeSets = this.pendingChangeSets;
    const visualChanges = this.pendingVisualChanges;
    this.docDirty = false;
    this.selDirty = false;
    this.visualsDirty = false;
    this.pendingChangeSets = [];
    this.pendingVisualChanges = [];

    if (docDirty || selDirty) this.snapshot = this.runtime.snapshot();
    const { text, version, selection } = this.snapshot;
    const lineCount = text.lineCount;
    // The syntax engine must be at THIS version before the layout below
    // reads highlight runs: the frame that paints an edit is the frame
    // that has its colors (synchronous, in-process — no flash possible).
    this.runtime.highlights?.ensureSynced(version, changeSets);

    if (this.visuals === null || visualsDirty || docDirty) {
      this.visuals = this.runtime.visualsSnapshot();
      this.widgets = this.visuals.widgets(text);
      this.geometry = new GeometryModel(lineCount, LINE_HEIGHT, PADDING_TOP, this.widgets);
      this.resolveBands();
    }
    this.contentHeightPx = this.geometry.contentHeight();
    this.viewportHeight = this.viewport.clientHeight;
    this.viewportWidth = this.viewport.clientWidth;

    // Scroll anchoring: when an edit lands above the first visible line,
    // shift the scroll model so the viewport text holds still.
    if (docDirty && this.anchor !== null && changeSets !== null && changeSets.length > 0) {
      let offset = this.anchor.offset;
      for (const set of changeSets) offset = mapOffset(set, offset);
      const line = text.lineRangeAt(Math.min(offset, text.length)).index;
      this.scrollY += this.geometry.lineTop(line) - this.anchor.topPx;
    }

    if (this.revealCaretPending) {
      this.revealCaretPending = false;
      const line = text.lineRangeAt(selection.primary.head).index;
      const caretTop = this.geometry.lineTop(line);
      if (caretTop < this.scrollY) {
        this.scrollY = caretTop;
      } else if (caretTop + LINE_HEIGHT > this.scrollY + this.viewportHeight) {
        this.scrollY = caretTop + LINE_HEIGHT - this.viewportHeight;
      }

      const lineRange = text.lineRange(line);
      const caretVirtualCols = virtualColsBefore(
        this.visuals?.virtualTextBetween(lineRange.from, lineRange.to) ?? [],
        text,
        line,
        selection.primary.head,
      );
      const beforeCaret = text.sliceString(lineRange.from, selection.primary.head);
      // The caret's line bounds content width from below. maxRowChars is
      // otherwise updated by the window patch AFTER this clamp runs, so a
      // bulk edit (paste, fast typing) would clamp the reveal against a
      // stale width and strand the caret off-screen.
      const caretLineCols = visualColumn(text.sliceString(lineRange.from, lineRange.to));
      if (caretLineCols > this.maxRowChars) this.maxRowChars = caretLineCols;
      const caretLeft =
        this.textOriginX + (visualColumn(beforeCaret) + caretVirtualCols) * this.charWidth;
      const leftMargin = this.textOriginX;
      const rightMargin = 24;
      if (caretLeft < this.scrollX + leftMargin) {
        this.scrollX = caretLeft - leftMargin;
      } else if (caretLeft + this.charWidth > this.scrollX + this.viewportWidth - rightMargin) {
        this.scrollX = caretLeft + this.charWidth - this.viewportWidth + rightMargin;
      }
    }
    this.scrollY = Math.max(0, Math.min(this.maxScrollY(), this.scrollY));
    this.scrollX = Math.max(0, Math.min(this.maxScrollX(), this.scrollX));
    this.syncViewportCssVars();

    const scrollY = this.scrollY;
    const visStart = this.geometry.lineAt(scrollY);
    const visEnd = Math.min(
      lineCount,
      this.geometry.lineAt(scrollY + Math.max(0, this.viewportHeight - 1)) + 2,
    );

    const slackLines = Math.ceil(WINDOW_SLACK_PX / LINE_HEIGHT);
    const needStart = Math.max(0, visStart - slackLines);
    const needEnd = Math.min(lineCount, visEnd + slackLines);
    const windowOk =
      this.windowEnd > this.windowStart &&
      this.windowStart <= needStart &&
      this.windowEnd >= needEnd &&
      this.windowEnd <= lineCount;

    if (!windowOk || docDirty || visualsDirty) {
      let start = this.windowStart;
      let end = this.windowEnd;
      if (!windowOk) {
        const overscanLines = Math.ceil(OVERSCAN_PX / LINE_HEIGHT);
        const targetLines = visEnd - visStart + 2 * overscanLines;
        end = Math.min(lineCount, Math.floor((visStart + visEnd) / 2) + Math.ceil(targetLines / 2));
        start = Math.max(0, end - targetLines);
        end = Math.min(lineCount, start + targetLines);
      }

      const winFrom = text.lineRange(start).from;
      const winTo = end > start ? text.lineRange(end - 1).to : winFrom;
      const layout = layoutViewport({
        text,
        version,
        startLine: start,
        lineCount: end - start,
        tabWidth: TAB_WIDTH,
        lineHeight: LINE_HEIGHT,
        charWidth: this.charWidth,
        paddingTop: PADDING_TOP,
        paddingLeft: this.textOriginX,
        visuals: {
          // Syntax runs merge under product styles (lowest priority).
          textStyles: [
            ...this.syntaxStyles(winFrom, winTo),
            ...this.visuals!.textStylesBetween(winFrom, winTo),
          ],
          rangeDecorations: this.visuals!.decorationsBetween(winFrom, winTo),
          virtualText: this.visuals!.virtualTextBetween(winFrom, winTo),
          // All widgets: layout's absolute geometry must agree with the
          // GeometryModel about rows above the window too.
          blockWidgets: this.widgets,
        },
      });

      // Dirtiness: document structure changes rebuild; in-line edits and
      // localized visual changes rewrite only their rows.
      let structureUnknown = visualChanges === null;
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
      if (visualChanges !== null) {
        for (const range of visualChanges) {
          dirtySpans.push([
            text.lineRangeAt(Math.max(0, Math.min(range.from, text.length))).index,
            text.lineRangeAt(Math.max(0, Math.min(range.to, text.length))).index,
          ]);
        }
      }

      this.patchRows(layout, structureUnknown, dirtySpans, this.windowStart, this.windowEnd);
      // Content width derives from the window (line widths aren't indexed,
      // so it cannot be exact like the vertical side). Document edits
      // RECOMPUTE it — deleting the long line shrinks the scrollbar — while
      // scroll-driven rebuilds only grow it, keeping the thumb stable while
      // panning past the widest line.
      const windowCols = layout.rows.reduce(
        (max, row) => (row.kind === "text" ? Math.max(max, visualColumn(row.text)) : max),
        0,
      );
      this.maxRowChars = docDirty ? windowCols : Math.max(this.maxRowChars, windowCols);
      this.scrollX = Math.max(0, Math.min(this.maxScrollX(), this.scrollX));
      this.syncViewportCssVars();
      this.windowStart = start;
      this.windowEnd = end;
      this.renderBands(layout);
      this.renderDecorations(layout);
      this.reconcileMounts(layout);
      this.overlaysDirty = true;
    }
    this.lastLineCount = lineCount;

    const windowTop = this.windowTop();
    const dpr = window.devicePixelRatio || 1;
    const ty = Math.round((windowTop - scrollY) * dpr) / dpr;
    const tx = Math.round(-this.scrollX * dpr) / dpr;
    const transform = `translate(${tx}px, ${ty}px)`;
    if (transform !== this.lastTransform) {
      this.lastTransform = transform;
      this.contentEl.style.transform = transform;
    }
    // Bands pin horizontally: vertical channel only.
    const bandTransform = `translateY(${ty}px)`;
    if (bandTransform !== this.lastBandTransform) {
      this.lastBandTransform = bandTransform;
      this.bandHost.style.transform = bandTransform;
    }

    const vThumbHeight = this.vThumbHeight();
    const maxY = this.maxScrollY();
    const vTrackSpan = this.viewportHeight - vThumbHeight;
    const vThumbTop = vThumbHeight > 0 && maxY > 0 ? (scrollY / maxY) * vTrackSpan : 0;
    const vThumbCss = `${Math.round(vThumbTop)}:${Math.round(vThumbHeight)}`;
    if (vThumbCss !== this.lastVThumbCss) {
      this.lastVThumbCss = vThumbCss;
      this.vThumb.style.height = `${vThumbHeight}px`;
      this.vThumb.style.top = `${vThumbTop}px`;
      this.vThumb.style.display = vThumbHeight > 0 ? "" : "none";
      this.vScrollbar.style.display = vThumbHeight > 0 ? "" : "none";
    }

    const hThumbWidth = this.hThumbWidth();
    const maxX = this.maxScrollX();
    const hTrackSpan = this.viewportWidth - hThumbWidth;
    const hThumbLeft = hThumbWidth > 0 && maxX > 0 ? (this.scrollX / maxX) * hTrackSpan : 0;
    const hThumbCss = `${Math.round(hThumbLeft)}:${Math.round(hThumbWidth)}`;
    if (hThumbCss !== this.lastHThumbCss) {
      this.lastHThumbCss = hThumbCss;
      this.hThumb.style.width = `${hThumbWidth}px`;
      this.hThumb.style.left = `${hThumbLeft}px`;
      this.hThumb.style.display = hThumbWidth > 0 ? "" : "none";
      this.hScrollbar.style.display = hThumbWidth > 0 ? "" : "none";
    }

    if (docDirty || selDirty || this.overlaysDirty) {
      this.renderOverlays(windowTop, selDirty);
      this.overlaysDirty = false;
    }

    const anchorLine = Math.min(Math.max(0, visStart), lineCount - 1);
    this.anchor = {
      offset: text.lineRange(anchorLine).from,
      topPx: this.geometry.lineTop(anchorLine),
    };

    this.positionFloats();
    this.runtime.viewportChanged?.(this.geometry.lineAt(this.scrollY), this.windowStart, this.windowEnd);

    if (docDirty || selDirty) this.options.onSnapshot?.(this.snapshot);
  }

  /**
   * Reconcile row elements with the new layout. Text-only windows keep the
   * edge-trim fast path (retained rows untouched); windows containing block
   * widgets rebuild, preserving widget mount element identity.
   */
  private patchRows(
    layout: ViewportLayoutSnapshot<unknown>,
    structureUnknown: boolean,
    dirtySpans: ReadonlyArray<readonly [number, number]>,
    prevStart: number,
    prevEnd: number,
  ): { mode: string; added: number; removed: number; rewritten: number } {
    const layer = this.rowLayer;
    const hasBlocks = layout.rows.some((row) => row.kind === "block");

    if (hasBlocks || this.windowHasWidgets) {
      // Mixed window: rebuild in document order, reusing mount elements.
      const widgetByRow = new Map<number, string>();
      for (const widget of layout.blockWidgets) widgetByRow.set(widget.row, widget.id);
      const fragment = document.createDocumentFragment();
      const index = new Map<number, number>();
      let childIndex = 0;
      for (const row of layout.rows) {
        if (row.kind === "text") {
          const el = document.createElement("div");
          el.className = "editor-row";
          renderRowContent(el, row.segments);
          index.set(row.line, childIndex);
          fragment.append(el);
        } else {
          const id = widgetByRow.get(row.row);
          let el = id !== undefined ? this.mountEls.get(id) : undefined;
          if (el === undefined) {
            el = document.createElement("div");
            el.className = "editor-widget";
            if (id !== undefined) {
              el.dataset.widgetId = id;
              this.mountEls.set(id, el);
              this.runtime.mounts.mounted(id, el);
            }
          }
          el.style.height = `${row.heightRows * LINE_HEIGHT}px`;
          fragment.append(el);
        }
        childIndex++;
      }
      layer.replaceChildren(fragment);
      this.rowIndexByLine = index;
      this.windowHasWidgets = hasBlocks;
      return { mode: "rebuild-mixed", added: layout.rows.length, removed: 0, rewritten: 0 };
    }

    // Text-only fast path.
    const rows = layout.rows.filter((row) => row.kind === "text");
    const start = layout.startLine;
    const end = start + rows.length;
    this.rowIndexByLine = null;
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
        renderRowContent(
          layer.children[line - start] as HTMLDivElement,
          rows[line - start]!.segments,
        );
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

  private syncViewportCssVars(): void {
    const key = `${this.scrollX}:${this.viewportWidth}`;
    if (key === this.lastViewportVars) return;
    this.lastViewportVars = key;
    this.root.style.setProperty("--editor-scroll-x", `${this.scrollX}px`);
    this.root.style.setProperty("--editor-viewport-width", `${this.viewportWidth}px`);
  }

  private buildRowEl(segments: readonly LayoutInlineSegment[]): HTMLDivElement {
    const el = document.createElement("div");
    el.className = "editor-row";
    renderRowContent(el, segments);
    return el;
  }


  /** Gutter band cells for the current window (cheap full rebuild). */
  private renderBands(layout: ViewportLayoutSnapshot<unknown>): void {
    if (this.bands.length === 0) return;
    for (let i = 0; i < this.bands.length; i++) {
      const band = this.bands[i]!;
      const column = this.bandHost.children[i] as HTMLDivElement | undefined;
      if (column === undefined) continue;
      const fragment = document.createDocumentFragment();
      for (const row of layout.rows) {
        const cellEl = document.createElement("div");
        if (row.kind === "text") {
          const cell = band.cell?.(row.line) ?? null;
          cellEl.className = cell?.className
            ? `editor-band-cell ${cell.className}`
            : "editor-band-cell";
          if (cell) cellEl.textContent = cell.text;
        } else {
          cellEl.className = "editor-band-spacer";
          cellEl.style.height = `${row.heightRows * LINE_HEIGHT}px`;
        }
        fragment.append(cellEl);
      }
      column.replaceChildren(fragment);
    }
  }

  /**
   * Document-absolute top of the window's first RENDERED row. Normally that is
   * windowStart's text row, but a leading block (afterLine < 0, rendered above
   * line 0) shifts the window origin up by its height — without this the flow
   * row layer and the absolutely-positioned decoration/overlay layers disagree
   * by exactly the leading block's height.
   */
  private windowTop(): number {
    const base = this.geometry.lineTop(this.windowStart);
    if (this.windowStart !== 0) return base;
    let leadingRows = 0;
    for (const widget of this.widgets) if (widget.afterLine < 0) leadingRows += widget.heightRows;
    return base - leadingRows * LINE_HEIGHT;
  }

  /** Decoration rect layers (window-local), rebuilt with the window. */
  private renderDecorations(layout: ViewportLayoutSnapshot<unknown>): void {
    const windowTop = this.windowTop();
    const bg: HTMLDivElement[] = [];
    const fg: HTMLDivElement[] = [];
    for (const decoration of layout.decorations) {
      for (const rect of decoration.rects) {
        const el = document.createElement("div");
        el.className = decoration.className
          ? `editor-decoration editor-decoration-${decoration.layer} ${decoration.className}`
          : `editor-decoration editor-decoration-${decoration.layer}`;
        el.style.top = `${rect.top - windowTop}px`;
        el.style.left = `${rect.left}px`;
        el.style.width = `${rect.width}px`;
        el.style.height = `${rect.height}px`;
        if (decoration.style) {
          for (const [key, value] of Object.entries(decoration.style)) {
            (el.style as unknown as Record<string, string>)[key] = value;
          }
        }
        (decoration.layer === "background" ? bg : fg).push(el);
      }
    }
    this.decoBgLayer.replaceChildren(...bg);
    this.decoFgLayer.replaceChildren(...fg);
  }

  /** Withdraw mounts for widgets that left the window. */
  private reconcileMounts(layout: ViewportLayoutSnapshot<unknown>): void {
    const live = new Set(layout.blockWidgets.map((widget) => widget.id));
    for (const id of [...this.mountEls.keys()]) {
      if (!live.has(id)) {
        this.mountEls.delete(id);
        this.runtime.mounts.unmounted(id);
      }
    }
  }

  /** Caret + selection rects, window-local, above the rows. */
  private renderOverlays(windowTop: number, selDirty: boolean): void {
    const { text, version, selection } = this.snapshot;
    // Virtual text shifts every column right of it: caret and selection
    // geometry must see the same virtuals the rows render.
    const winFrom = this.windowEnd > this.windowStart ? text.lineRange(this.windowStart).from : 0;
    const winTo =
      this.windowEnd > this.windowStart ? text.lineRange(this.windowEnd - 1).to : 0;
    const layout = layoutViewport({
      text,
      version,
      startLine: this.windowStart,
      lineCount: this.windowEnd - this.windowStart,
      tabWidth: TAB_WIDTH,
      lineHeight: LINE_HEIGHT,
      charWidth: this.charWidth,
      paddingTop: PADDING_TOP,
      paddingLeft: this.textOriginX,
      visuals: {
        blockWidgets: this.widgets,
        virtualText: this.visuals?.virtualTextBetween(winFrom, winTo) ?? [],
      },
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
