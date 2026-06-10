/**
 * Render incident recorder (dev only). Runs a per-frame monitor that records
 * scroll events, render passes, and frame-by-frame exposure measurements
 * into a ring buffer, and dumps ONE compact report per "incident" — the
 * deterministic moment a flicker can exist — instead of spamming logs.
 *
 * Per frame it measures:
 * - gap: viewport pixels NOT covered by the window's VISUAL rect (after any
 *   sticky clamping). gap > 0 means a blank region was genuinely on screen
 *   this frame — a logic failure we can localize.
 * - clamp: how far sticky displaced the window from its flow position
 *   (non-zero = the clamp is engaged, scroll is outrunning rendering).
 * - dt: rAF delta; > 25ms means the main thread missed frames.
 *
 * If the user sees blank text while every frame logs gap=0, the DOM/clamp
 * were provably correct and the artifact is compositor raster, not logic.
 *
 * Console API: window.__editorDebug.{dump(), clear()}
 */

export type RenderDebugState = {
  readonly windowStart: number;
  readonly windowEnd: number;
  readonly windowTopPx: number;
  readonly windowHeightPx: number;
};

type Entry = {
  readonly seq: number;
  readonly t: number;
  readonly k: "frame" | "scroll" | "render";
  readonly [key: string]: unknown;
};

const RING_CAPACITY = 12_000; // ~45s of 120Hz frames + events
const GAP_TOLERANCE_PX = 10; // top padding + subpixel slop
const HITCH_MS = 25;
const QUIET_FRAMES_TO_CLOSE = 20;
const CONTEXT_ENTRIES_BEFORE = 40;
const MAX_INCIDENTS = 12;

export class RenderDebug {
  private entries: Entry[] = [];
  private seq = 0;
  private rafId: number | null = null;
  private lastFrameTs = 0;
  private incidentStartSeq: number | null = null;
  private incidentMaxGap = 0;
  private incidentMaxDt = 0;
  private quietFrames = 0;
  private incidentCount = 0;

  constructor(
    private readonly scroller: HTMLElement,
    private readonly getVisibleEls: () => HTMLElement[],
    private readonly state: () => RenderDebugState,
    private readonly getScrollTop: () => number = () => this.scroller.scrollTop,
  ) {}

  start(): void {
    if (this.rafId !== null) return;
    this.rafId = requestAnimationFrame(this.frame);
    (window as unknown as Record<string, unknown>).__editorDebug = {
      dump: () => console.table(this.entries),
      clear: () => {
        this.entries = [];
      },
    };
    console.info(
      "[editor-debug] incident recorder armed. Reproduce the flicker; " +
        "incidents dump automatically. Full ring: __editorDebug.dump()",
    );
  }

  stop(): void {
    if (this.rafId !== null) cancelAnimationFrame(this.rafId);
    this.rafId = null;
  }

  private lastScrollTs = 0;

  log(k: "scroll" | "render", data: Record<string, unknown>): void {
    const t = performance.now();
    if (k === "scroll") this.lastScrollTs = t;
    this.push({ seq: this.seq++, t: Math.round(t), k, ...data });
  }

  private push(entry: Entry): void {
    this.entries.push(entry);
    if (this.entries.length > RING_CAPACITY) {
      this.entries.splice(0, this.entries.length - RING_CAPACITY);
    }
  }

  private readonly frame = (ts: number): void => {
    const dt = this.lastFrameTs === 0 ? 16.7 : ts - this.lastFrameTs;
    this.lastFrameTs = ts;

    const scrollTop = this.getScrollTop();
    const viewportHeight = this.scroller.clientHeight;
    const scrollerTop = this.scroller.getBoundingClientRect().top;
    const s = this.state();

    // Uncovered viewport pixels, measured on the union of the VISUAL rects
    // of the visible page elements (post-clamp).
    const rects = this.getVisibleEls()
      .map((el) => el.getBoundingClientRect())
      .map((r) => [Math.max(0, r.top - scrollerTop), Math.min(viewportHeight, r.bottom - scrollerTop)] as const)
      .filter(([a, b]) => b > a)
      .sort((a, b) => a[0] - b[0]);
    let coveredPx = 0;
    let cursor = 0;
    for (const [a, b] of rects) {
      coveredPx += Math.max(0, b - Math.max(a, cursor));
      cursor = Math.max(cursor, b);
    }
    const gap = Math.round(viewportHeight - coveredPx);
    // Sticky displacement of the first visible page: visual vs flow top.
    const firstRect = this.getVisibleEls()[0]?.getBoundingClientRect();
    const flowTop = scrollerTop + s.windowTopPx - scrollTop;
    const clamp = firstRect ? Math.round(firstRect.top - flowTop) : 0;

    this.push({
      seq: this.seq++,
      t: Math.round(ts),
      k: "frame",
      st: Math.round(scrollTop),
      dt: Math.round(dt),
      win: `${s.windowStart}-${s.windowEnd}`,
      gap,
      clamp,
    });

    // Frame hitches only count near scroll activity; rAF pauses from tab
    // switches and idle periods are noise, not flicker.
    const nearScroll = ts - this.lastScrollTs < 500;
    const bad = gap > GAP_TOLERANCE_PX || (dt > HITCH_MS && nearScroll);
    if (bad) {
      if (this.incidentStartSeq === null) this.incidentStartSeq = this.seq - 1;
      this.incidentMaxGap = Math.max(this.incidentMaxGap, gap);
      this.incidentMaxDt = Math.max(this.incidentMaxDt, Math.round(dt));
      this.quietFrames = 0;
    } else if (this.incidentStartSeq !== null) {
      this.quietFrames++;
      if (this.quietFrames >= QUIET_FRAMES_TO_CLOSE) this.closeIncident();
    }

    this.rafId = requestAnimationFrame(this.frame);
  };

  private closeIncident(): void {
    const startSeq = this.incidentStartSeq!;
    this.incidentStartSeq = null;
    this.quietFrames = 0;
    const maxGap = this.incidentMaxGap;
    const maxDt = this.incidentMaxDt;
    this.incidentMaxGap = 0;
    this.incidentMaxDt = 0;
    if (this.incidentCount >= MAX_INCIDENTS) return;
    this.incidentCount++;

    const fromSeq = startSeq - CONTEXT_ENTRIES_BEFORE;
    const toSeq = this.seq - QUIET_FRAMES_TO_CLOSE + 4;
    const slice = this.entries
      .filter((entry) => entry.seq >= fromSeq && entry.seq <= toSeq)
      .slice(0, 120);
    const label =
      `[editor-debug] incident #${this.incidentCount}: maxGap=${maxGap}px maxFrameDt=${maxDt}ms ` +
      `(gap>0 = real uncovered viewport; gap=0 = compositor raster, not logic)`;
    // Defer the dump: console.table is expensive and must not stall the
    // frames it is reporting on.
    setTimeout(() => {
      console.groupCollapsed(label);
      console.table(slice);
      console.groupEnd();
    }, 250);
  }
}
