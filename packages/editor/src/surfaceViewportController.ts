import type { CellGridMetrics, RenderFrame, Viewport } from "@the-editor/contracts/runtime";
import type { BufferDecoration } from "./decorations.js";
import {
  clearCanvasTextSurface,
  configureCanvasTextSurface,
  loadEditorFont,
  paintCanvasTextSurface,
  type CanvasTextSurfaceConfiguration,
  type CanvasTextSurfacePaintOptions,
  type CanvasTextSurfaceStyleOptions,
} from "./textSurfaceCanvas.js";
import type { ThemeSnapshot } from "./themes.js";

export interface SurfaceViewportFrameResult {
  frame: RenderFrame;
  viewport: Viewport;
  lineCount: number;
  syntaxLegend?: Record<number, string>;
}

export interface SurfaceViewportFrameSource {
  key: string;
  readFrame(input: { viewport: Viewport; metrics: CellGridMetrics }): Promise<SurfaceViewportFrameResult>;
}

export interface SurfaceViewportControllerState {
  sourceKey?: string;
  status: "idle" | "loading" | "ready";
  metrics?: CellGridMetrics;
  viewport?: Viewport;
  frameViewport?: Viewport;
  frame?: RenderFrame;
  lineCount: number;
  scrollTop: number;
  scrollLeft: number;
  scrollHeight: number;
  syntaxLegend: Record<number, string>;
}

export interface SurfaceViewportControllerOptions {
  styleOptions?: CanvasTextSurfaceStyleOptions;
  paintOptions?: CanvasTextSurfacePaintOptions;
  overscanRows?: number;
  overscanCols?: number;
}

export class SurfaceViewportController {
  private readonly styleOptions: CanvasTextSurfaceStyleOptions;
  private paintOptions: CanvasTextSurfacePaintOptions;
  private readonly overscanRows: number;
  private readonly overscanCols: number;
  private readonly listeners = new Set<() => void>();
  private readonly onScroll = () => {
    this.updateScrollState();
    this.paint();
    this.requestFrameIfNeeded();
  };

  private scrollElement: HTMLDivElement | null = null;
  private canvas: HTMLCanvasElement | null = null;
  private theme: ThemeSnapshot | undefined;
  private decorations: BufferDecoration[] = [];
  private source: SurfaceViewportFrameSource | null = null;
  private layout: CanvasTextSurfaceConfiguration | null = null;
  private resizeObserver: ResizeObserver | null = null;
  private fontLoadPromise: Promise<void> | null = null;
  private requestToken = 0;
  private pendingViewportKey: string | null = null;

  private state: SurfaceViewportControllerState = {
    status: "idle",
    lineCount: 0,
    scrollTop: 0,
    scrollLeft: 0,
    scrollHeight: 1,
    syntaxLegend: {},
  };

  constructor(options: SurfaceViewportControllerOptions = {}) {
    this.styleOptions = options.styleOptions ?? {};
    this.paintOptions = options.paintOptions ?? {};
    this.overscanRows = options.overscanRows ?? 12;
    this.overscanCols = options.overscanCols ?? 0;
  }

  attach(scrollElement: HTMLDivElement, canvas: HTMLCanvasElement): void {
    if (this.scrollElement === scrollElement && this.canvas === canvas) return;
    this.detach();

    this.scrollElement = scrollElement;
    this.canvas = canvas;
    scrollElement.addEventListener("scroll", this.onScroll, { passive: true });

    const configure = () => {
      if (!this.canvas) return;
      const configuration = configureCanvasTextSurface(this.canvas, this.styleOptions);
      this.layout = configuration;
      this.state = {
        ...this.state,
        metrics: configuration.metrics,
        viewport: this.deriveVisibleViewport(),
      };
      this.syncScrollHeight();
      this.paint();
      this.emit();
      this.requestFrame(true);
    };

    this.fontLoadPromise = loadEditorFont(this.styleOptions).finally(() => {
      configure();
      if (!this.scrollElement) return;
      this.resizeObserver = new ResizeObserver(() => {
        configure();
      });
      this.resizeObserver.observe(this.scrollElement);
    });
  }

  detach(): void {
    this.resizeObserver?.disconnect();
    this.resizeObserver = null;
    if (this.scrollElement) {
      this.scrollElement.removeEventListener("scroll", this.onScroll);
    }
    this.scrollElement = null;
    this.canvas = null;
    this.layout = null;
  }

  dispose(): void {
    this.requestToken += 1;
    this.detach();
    this.listeners.clear();
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  snapshot(): SurfaceViewportControllerState {
    return this.state;
  }

  setSource(source: SurfaceViewportFrameSource | null): void {
    if (this.source?.key === source?.key) return;
    this.source = source;
    this.pendingViewportKey = null;
    this.requestToken += 1;

    const resetScroll = Boolean(this.scrollElement && source?.key !== this.state.sourceKey);
    if (resetScroll && this.scrollElement) {
      this.scrollElement.scrollTop = 0;
      this.scrollElement.scrollLeft = 0;
    }

    this.state = {
      ...this.state,
      sourceKey: source?.key,
      status: source ? "loading" : "idle",
      frame: undefined,
      frameViewport: undefined,
      lineCount: 0,
      scrollTop: 0,
      scrollLeft: 0,
      scrollHeight: 1,
      syntaxLegend: {},
      viewport: this.deriveVisibleViewport(),
    };
    this.syncScrollHeight();
    this.paint();
    this.emit();
    if (source) this.requestFrame(true);
  }

  setTheme(theme: ThemeSnapshot | undefined): void {
    this.theme = theme;
    this.paint();
  }

  setDecorations(decorations: BufferDecoration[]): void {
    this.decorations = decorations;
    this.paint();
  }

  setPaintOptions(options: CanvasTextSurfacePaintOptions): void {
    this.paintOptions = options;
    this.paint();
  }

  private emit(): void {
    for (const listener of this.listeners) listener();
  }

  private updateScrollState(): void {
    this.state = {
      ...this.state,
      scrollTop: this.scrollElement?.scrollTop ?? 0,
      scrollLeft: this.scrollElement?.scrollLeft ?? 0,
      viewport: this.deriveVisibleViewport(),
    };
    this.emit();
  }

  private deriveVisibleViewport(): Viewport | undefined {
    const metrics = this.state.metrics;
    const scrollElement = this.scrollElement;
    const layout = this.layout;
    if (!metrics || !scrollElement || !layout) return undefined;
    return {
      firstRow: Math.max(0, Math.floor(scrollElement.scrollTop / metrics.rowHeightPx)),
      rowCount: layout.viewport.rowCount,
      firstCol: Math.max(0, Math.floor(scrollElement.scrollLeft / metrics.cellWidthPx)),
      wrapWidth: layout.viewport.wrapWidth,
    };
  }

  private desiredFrameViewport(): Viewport | undefined {
    const visible = this.state.viewport;
    if (!visible) return undefined;
    return {
      firstRow: Math.max(0, visible.firstRow - this.overscanRows),
      rowCount: visible.rowCount + this.overscanRows * 2 + 1,
      firstCol: Math.max(0, visible.firstCol - this.overscanCols),
      wrapWidth: visible.wrapWidth,
    };
  }

  private requestFrameIfNeeded(): void {
    const desired = this.desiredFrameViewport();
    if (!desired) return;
    const current = this.state.frameViewport;
    if (current && viewportContains(current, this.state.viewport)) return;
    this.requestFrame(false);
  }

  private requestFrame(force: boolean): void {
    const source = this.source;
    const metrics = this.state.metrics;
    const viewport = this.desiredFrameViewport();
    if (!source || !metrics || !viewport) return;

    const viewportKey = `${source.key}:${viewport.firstRow}:${viewport.rowCount}:${viewport.firstCol}:${viewport.wrapWidth}`;
    if (!force && this.pendingViewportKey === viewportKey) return;
    this.pendingViewportKey = viewportKey;

    const token = ++this.requestToken;
    this.state = {
      ...this.state,
      status: this.state.frame ? "ready" : "loading",
    };
    this.emit();

    void source.readFrame({ viewport, metrics }).then((result) => {
      if (token !== this.requestToken || source !== this.source) return;
      this.pendingViewportKey = null;
      this.state = {
        ...this.state,
        status: "ready",
        frame: result.frame,
        frameViewport: result.viewport,
        lineCount: result.lineCount,
        syntaxLegend: result.syntaxLegend ?? {},
      };
      this.syncScrollHeight();
      this.paint();
      this.emit();
      this.requestFrameIfNeeded();
    }).catch(() => {
      if (token !== this.requestToken || source !== this.source) return;
      this.pendingViewportKey = null;
      this.state = {
        ...this.state,
        status: this.state.frame ? "ready" : "idle",
      };
      this.emit();
    });
  }

  private syncScrollHeight(): void {
    const scrollElement = this.scrollElement;
    const metrics = this.state.metrics;
    if (!scrollElement || !metrics) return;
    const padding = this.styleOptions.paddingPx ?? 18;
    const lineCount = Math.max(this.state.lineCount, this.state.viewport?.rowCount ?? 0);
    const scrollHeight = Math.max(scrollElement.clientHeight, padding * 2 + lineCount * metrics.rowHeightPx);
    this.state = {
      ...this.state,
      scrollHeight,
    };

    const maxScrollTop = Math.max(0, scrollHeight - scrollElement.clientHeight);
    if (scrollElement.scrollTop > maxScrollTop) {
      scrollElement.scrollTop = maxScrollTop;
      this.state = {
        ...this.state,
        scrollTop: scrollElement.scrollTop,
        viewport: this.deriveVisibleViewport(),
      };
    }
  }

  private paint(): void {
    const canvas = this.canvas;
    const frame = this.state.frame;
    const frameViewport = this.state.frameViewport;
    const metrics = this.state.metrics;
    if (!canvas || !metrics) return;
    if (!frame || !frameViewport) {
      clearCanvasTextSurface(canvas, this.theme, this.styleOptions);
      return;
    }

    const offsetXPx = Math.max(0, this.state.scrollLeft - frameViewport.firstCol * metrics.cellWidthPx);
    const offsetYPx = Math.max(0, this.state.scrollTop - frameViewport.firstRow * metrics.rowHeightPx);
    paintCanvasTextSurface(
      canvas,
      frame,
      this.decorations,
      this.theme,
      this.state.syntaxLegend,
      {
        ...this.paintOptions,
        offsetXPx,
        offsetYPx,
      },
      this.styleOptions,
    );
  }
}

function viewportContains(container: Viewport, value: Viewport | undefined): boolean {
  if (!value) return true;
  const containerLastRow = container.firstRow + container.rowCount;
  const valueLastRow = value.firstRow + value.rowCount;
  const containerLastCol = container.firstCol + container.wrapWidth;
  const valueLastCol = value.firstCol + value.wrapWidth;
  return container.firstRow <= value.firstRow
    && containerLastRow >= valueLastRow
    && container.firstCol <= value.firstCol
    && containerLastCol >= valueLastCol;
}
