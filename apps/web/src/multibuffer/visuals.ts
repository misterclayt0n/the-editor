/**
 * Synthesizes a VisualsSnapshot for a multibuffer view directly from its
 * snapshot + excerpt metadata. The multibuffer is read-only and static after
 * construction, so there is no EditorVisuals contribution lifecycle — the
 * snapshot is built once and handed to the renderer.
 *
 * Three contributions:
 *  - excerpt header/divider BLOCK widgets (the cards), with `afterLine = -1`
 *    for the first excerpt so the renderer emits a leading row before line 0;
 *  - a left GUTTER band whose cells show each excerpt's ORIGINAL source line;
 *  - background DECORATIONS over the search matches, mapped to unified coords.
 */

import type {
  AnchoredBlockWidget,
  RangeDecoration,
  ResolvedGutterBand,
  VirtualText,
  VisualsSnapshot,
} from "@the-editor/editor";
import type { MultiBufferSnapshot } from "@the-editor/engine";
import type { ExcerptMeta } from "./builder";
import { lineNumberBand } from "../editor/gutter";

/** A slice of one excerpt overlapping a unified range, with its source mapping. */
export type ExcerptSlice = {
  readonly meta: ExcerptMeta;
  /** Add to a SOURCE offset to get the UNIFIED offset (and vice versa). */
  readonly base: number;
  readonly sourceFrom: number;
  readonly sourceTo: number;
};

/** Excerpts overlapping [from, to) in unified coords, with source sub-ranges. */
export function excerptSlicesInRange(
  metas: readonly ExcerptMeta[],
  from: number,
  to: number,
): ExcerptSlice[] {
  const out: ExcerptSlice[] = [];
  for (const meta of metas) {
    const start = meta.startOffset;
    const end = start + (meta.excerpt.sourceRange.end - meta.excerpt.sourceRange.start);
    if (end <= from || start >= to) continue;
    const base = start - meta.excerpt.sourceRange.start;
    out.push({
      meta,
      base,
      sourceFrom: meta.excerpt.sourceRange.start + Math.max(0, from - start),
      sourceTo: meta.excerpt.sourceRange.start + Math.min(end - start, to - start),
    });
  }
  return out;
}

export const MB_OWNER = "the-editor/multibuffer";
/** Display rows reserved for a file header card vs an intra-file divider. */
export const FILE_HEADER_ROWS = 2;
export const EXCERPT_DIVIDER_ROWS = 1;
const HEADER_PREFIX = "mb-header:";

export const headerId = (excerptId: string): string => `${HEADER_PREFIX}${excerptId}`;
export const excerptIdFromHeader = (id: string): string | undefined =>
  id.startsWith(HEADER_PREFIX) ? id.slice(HEADER_PREFIX.length) : undefined;

export function multiBufferVisuals(
  snapshot: MultiBufferSnapshot,
  metas: readonly ExcerptMeta[],
  /** Live per-source-document visuals (LSP diagnostics + inlay hints). */
  sourceVisuals?: (documentId: string) => VisualsSnapshot | undefined,
): VisualsSnapshot {
  const widgets: AnchoredBlockWidget[] = metas.map((m) => ({
    id: headerId(m.excerpt.id),
    atOffset: m.startOffset,
    // afterLine -1 => leading block before the first line (see layoutViewport).
    afterLine: m.headerAfterLine,
    heightRows: m.isFirstOfFile ? FILE_HEADER_ROWS : EXCERPT_DIVIDER_ROWS,
    owner: MB_OWNER,
  }));
  const totalWidgetRows = widgets.reduce((n, w) => n + w.heightRows, 0);

  const decorations: RangeDecoration[] = [];
  let maxSourceLine = 1;
  for (const m of metas) {
    const buffer = snapshot.document(m.excerpt.documentId);
    if (buffer !== undefined) {
      maxSourceLine = Math.max(maxSourceLine, buffer.lineRangeAt(m.excerpt.sourceRange.end).index + 1);
    }
    const base = m.startOffset - m.excerpt.sourceRange.start;
    for (const match of m.matches) {
      if (match.end <= match.start) continue;
      decorations.push({
        from: base + match.start,
        to: base + match.end,
        layer: "background",
        className: "mb-match",
        owner: MB_OWNER,
      });
    }
  }
  decorations.sort((a, b) => a.from - b.from || a.to - b.to);

  // Line numbers are synthesized here (not consumed from the source bands) so
  // they are present immediately, before per-excerpt intel attaches — but the
  // band's shape/width/styling come from the SAME factory the editor's
  // lineNumbers package uses, so gutter changes mirror to both.
  const lineNumbers: ResolvedGutterBand = {
    ...lineNumberBand(maxSourceLine, (line) => {
      if (line < 0 || line >= snapshot.lineCount) return null;
      const src = snapshot.offsetToSource(snapshot.lineToOffset(line));
      if (src === undefined) return null;
      const buffer = snapshot.document(src.documentId);
      if (buffer === undefined) return null;
      return buffer.lineRangeAt(src.offset).index + 1;
    }),
    owner: MB_OWNER,
  };

  return {
    generation: 0, // the pane owns the generation (bumps on rebuild + diagnostics)
    textStylesBetween: () => [],
    decorationsBetween: (from, to) => {
      const out = decorations.filter((d) => d.to > from && d.from < to);
      if (sourceVisuals !== undefined) {
        for (const slice of excerptSlicesInRange(metas, from, to)) {
          const source = sourceVisuals(slice.meta.excerpt.documentId);
          if (source === undefined) continue;
          for (const d of source.decorationsBetween(slice.sourceFrom, slice.sourceTo)) {
            out.push({ ...d, from: slice.base + d.from, to: slice.base + d.to });
          }
        }
      }
      return out;
    },
    virtualTextBetween: (from, to) => {
      if (sourceVisuals === undefined) return [];
      const out: VirtualText[] = [];
      for (const slice of excerptSlicesInRange(metas, from, to)) {
        const source = sourceVisuals(slice.meta.excerpt.documentId);
        if (source === undefined) continue;
        for (const vt of source.virtualTextBetween(slice.sourceFrom, slice.sourceTo)) {
          out.push({ ...vt, at: slice.base + vt.at });
        }
      }
      return out;
    },
    widgets: () => widgets,
    totalWidgetRows,
    gutterBands: [lineNumbers, ...mapSourceGutterBands(snapshot, metas, sourceVisuals)],
  };
}

/**
 * Map every NON-line-number gutter band contributed to the on-screen source
 * documents (today: the VCS diff strip) into unified coordinates, so the
 * multibuffer gutter carries the same bands a plain editor view would. Bands
 * are grouped by id across documents; each unified cell maps its row back to
 * the owning source line and defers to that document's own cell. Attach-gated
 * like diagnostics — a band appears once its excerpt's intel is live.
 */
function mapSourceGutterBands(
  snapshot: MultiBufferSnapshot,
  metas: readonly ExcerptMeta[],
  sourceVisuals?: (documentId: string) => VisualsSnapshot | undefined,
): ResolvedGutterBand[] {
  if (sourceVisuals === undefined) return [];
  const byId = new Map<string, { sample: ResolvedGutterBand; perDoc: Map<string, ResolvedGutterBand> }>();
  for (const docId of new Set(metas.map((m) => m.excerpt.documentId))) {
    const visuals = sourceVisuals(docId);
    if (visuals === undefined) continue;
    for (const band of visuals.gutterBands) {
      if (band.id === "line-numbers") continue; // synthesized separately, always present
      const entry = byId.get(band.id) ?? { sample: band, perDoc: new Map<string, ResolvedGutterBand>() };
      entry.perDoc.set(docId, band);
      byId.set(band.id, entry);
    }
  }
  const out: ResolvedGutterBand[] = [];
  for (const [id, { sample, perDoc }] of byId) {
    out.push({
      id,
      side: sample.side,
      widthCh: sample.widthCh,
      ...(sample.priority !== undefined ? { priority: sample.priority } : {}),
      owner: MB_OWNER,
      ...(sample.widthPx !== undefined ? { widthPx: sample.widthPx } : {}),
      cell: (line) => {
        if (line < 0 || line >= snapshot.lineCount) return null;
        const src = snapshot.offsetToSource(snapshot.lineToOffset(line));
        if (src === undefined) return null;
        const band = perDoc.get(src.documentId);
        const buffer = snapshot.document(src.documentId);
        if (band?.cell === undefined || buffer === undefined) return null;
        return band.cell(buffer.lineRangeAt(src.offset).index);
      },
    });
  }
  return out;
}
