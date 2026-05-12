import type { CellGridMetrics, RenderFrame, SurfaceId } from "@the-editor/contracts/runtime";
import type { GutterSnapshot } from "./gutters.js";
import type { CanvasTextSurfaceStyleOptions } from "./textSurfaceCanvas.js";

export interface SurfaceCursorAnchor {
  surfaceId: SurfaceId;
  left: number;
  top: number;
  bottom: number;
  height: number;
  row: number;
  col: number;
}

export interface RegisteredSurfaceCursorLayout {
  surfaceId: SurfaceId;
  canvas: HTMLCanvasElement;
  frame?: RenderFrame;
  metrics?: CellGridMetrics;
  gutters?: GutterSnapshot[];
  styleOptions?: CanvasTextSurfaceStyleOptions;
}

export interface SurfaceCursorAnchorSnapshot {
  revision: number;
  layout?: RegisteredSurfaceCursorLayout;
  anchor?: SurfaceCursorAnchor;
}

class SurfaceCursorAnchorRegistry {
  private revision = 0;
  private readonly snapshots = new Map<SurfaceId, SurfaceCursorAnchorSnapshot>();
  private readonly listeners = new Set<() => void>();

  set(layout: RegisteredSurfaceCursorLayout): void {
    this.revision += 1;
    this.snapshots.set(layout.surfaceId, {
      revision: this.revision,
      layout,
      anchor: computeSurfaceCursorAnchor(layout),
    });
    this.emit();
  }

  delete(surfaceId: SurfaceId): void {
    if (!this.snapshots.has(surfaceId)) return;
    this.revision += 1;
    this.snapshots.delete(surfaceId);
    this.emit();
  }

  anchorForSurface(surfaceId: SurfaceId): SurfaceCursorAnchor | undefined {
    return this.snapshots.get(surfaceId)?.anchor;
  }

  snapshotForSurface(surfaceId: SurfaceId): SurfaceCursorAnchorSnapshot {
    return this.snapshots.get(surfaceId) ?? { revision: this.revision };
  }

  activeAnchor(surfaceIds: readonly SurfaceId[]): SurfaceCursorAnchor | undefined {
    for (const surfaceId of surfaceIds) {
      const anchor = this.anchorForSurface(surfaceId);
      if (anchor) return anchor;
    }
    return undefined;
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  private emit(): void {
    for (const listener of this.listeners) listener();
  }
}

export const surfaceCursorAnchors = new SurfaceCursorAnchorRegistry();

export function computeSurfaceCursorAnchor(layout: RegisteredSurfaceCursorLayout): SurfaceCursorAnchor | undefined {
  const cursor = layout.frame?.cursors.find((entry) => entry.active) ?? layout.frame?.cursors[0];
  const metrics = layout.metrics;
  if (!cursor || !metrics) return undefined;

  const rect = layout.canvas.getBoundingClientRect();
  const padding = layout.styleOptions?.paddingPx ?? 18;
  const gutterWidth = leftGutterWidthPx(layout.gutters ?? [], metrics.cellWidthPx);
  const left = rect.left + padding + gutterWidth + cursor.col * metrics.cellWidthPx;
  const top = rect.top + padding + cursor.surfaceRow * metrics.rowHeightPx;
  const height = metrics.rowHeightPx;

  return {
    surfaceId: layout.surfaceId,
    left,
    top,
    bottom: top + height,
    height,
    row: cursor.surfaceRow,
    col: cursor.col,
  };
}

function leftGutterWidthPx(gutters: readonly GutterSnapshot[], cellWidthPx: number): number {
  const lanes = new Map<string, { order: number; width: number }>();
  for (const gutter of gutters) {
    if (gutter.lane.side === "right") continue;
    const width = (gutter.lane.widthPx ?? ((gutter.lane.widthCells ?? 0) * cellWidthPx))
      + gutter.lane.paddingLeftPx
      + gutter.lane.paddingRightPx;
    const current = lanes.get(gutter.lane.id);
    if (!current || current.order > gutter.lane.order) {
      lanes.set(gutter.lane.id, { order: gutter.lane.order, width });
    }
  }
  return [...lanes.values()].sort((a, b) => a.order - b.order).reduce((sum, lane) => sum + lane.width, 0);
}
