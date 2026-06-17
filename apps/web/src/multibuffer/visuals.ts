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

const digits = (n: number): number => String(Math.max(1, n)).length;

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
    afterLine: snapshot.offsetToLine(m.startOffset) - 1,
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

  const gutter: ResolvedGutterBand = {
    id: "mb-line-numbers",
    side: "left",
    widthCh: digits(maxSourceLine) + 1,
    priority: 0,
    owner: MB_OWNER,
    cell: (line) => {
      if (line < 0 || line >= snapshot.lineCount) return null;
      const src = snapshot.offsetToSource(snapshot.lineToOffset(line));
      if (src === undefined) return null;
      const buffer = snapshot.document(src.documentId);
      if (buffer === undefined) return null;
      return {
        text: String(buffer.lineRangeAt(src.offset).index + 1),
        className: "gutter-line-number",
      };
    },
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
    gutterBands: [gutter],
  };
}
