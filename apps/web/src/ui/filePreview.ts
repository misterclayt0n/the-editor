/**
 * File preview: load a bounded window of a file and shape it into highlighted
 * lines, without touching the document/LSP/VCS machinery `openPath` drags in.
 * This module is the pure core (no React, no Effect) so it unit-tests with
 * fake read/highlight deps; the panel wires the real platform ports in.
 */

import type { FileTextRange } from "@the-editor/editor";
import type { CodeSpan } from "../editor/highlightSpans";

/** Window byte cap: matches the syntax engine's synchronous-parse ceiling. */
export const PREVIEW_WINDOW_BYTES = 131_072;
/** Rendered-line cap: bounds DOM cost regardless of how dense the window is. */
export const PREVIEW_MAX_LINES = 800;
/** Absolute size sanity ceiling above which we don't even peek. */
export const PREVIEW_MAX_FILE_BYTES = 10 * 1024 * 1024;
/** Extra lines mounted outside the visible preview viewport. */
export const PREVIEW_RENDER_OVERSCAN_LINES = 8;

export type PreviewTarget = {
  readonly path: string;
  readonly size: number;
  /** 0-based line to reveal/highlight when present (e.g. `file.ts:42`). */
  readonly focusLine?: number;
};

export type PreviewState =
  | { readonly status: "empty" }
  | { readonly status: "loading" }
  | { readonly status: "skipped"; readonly reason: "too-large" | "binary" }
  | { readonly status: "error"; readonly message: string }
  | {
      readonly status: "ready";
      readonly lines: readonly (readonly CodeSpan[])[];
      readonly truncated: boolean;
      readonly focusLine?: number;
    };

export type PreviewReader = (
  path: string,
  options: { readonly offset?: number; readonly maxBytes: number },
) => Promise<FileTextRange>;

/** Per-line highlighted spans for `code` as `ext`, or null to render plain. */
export type PreviewHighlighter = (code: string, ext: string) => (readonly CodeSpan[])[] | null;

export type PreviewLineWindow = {
  readonly start: number;
  readonly end: number;
  readonly offsetTop: number;
  readonly totalHeight: number;
};

export function previewLineWindow(input: {
  readonly lineCount: number;
  readonly scrollTop: number;
  readonly viewportHeight: number;
  readonly rowHeight: number;
  readonly overscan?: number;
}): PreviewLineWindow {
  const lineCount = Math.max(0, input.lineCount);
  const rowHeight = Number.isFinite(input.rowHeight) && input.rowHeight > 0 ? input.rowHeight : 18;
  const scrollTop = Math.max(0, input.scrollTop);
  const viewportHeight = Math.max(0, input.viewportHeight);
  const overscan = Math.max(0, input.overscan ?? PREVIEW_RENDER_OVERSCAN_LINES);
  const start = Math.min(lineCount, Math.max(0, Math.floor(scrollTop / rowHeight) - overscan));
  const end = Math.min(
    lineCount,
    Math.max(start, Math.ceil((scrollTop + viewportHeight) / rowHeight) + overscan),
  );
  return {
    start,
    end,
    offsetTop: start * rowHeight,
    totalHeight: lineCount * rowHeight,
  };
}

const extensionOf = (path: string): string => {
  const slash = path.lastIndexOf("/");
  const dot = path.lastIndexOf(".");
  return dot > slash + 1 ? path.slice(dot + 1).toLowerCase() : "";
};

const shapeLines = (
  text: string,
  ext: string,
  highlight?: PreviewHighlighter,
): { readonly lines: (readonly CodeSpan[])[]; readonly truncated: boolean } => {
  const rawLines = text.split("\n");
  const truncated = rawLines.length > PREVIEW_MAX_LINES;
  const kept = truncated ? rawLines.slice(0, PREVIEW_MAX_LINES) : rawLines;
  const highlighted =
    ext.length > 0 && highlight !== undefined ? highlight(kept.join("\n"), ext) : null;
  const lines = kept.map((line, i): readonly CodeSpan[] => {
    const spans = highlighted?.[i];
    if (spans !== undefined) return spans;
    return line.length > 0 ? [{ text: line }] : [];
  });
  return { lines, truncated };
};

/** The raw, read-but-unshaped preview window — lets a caller render plain
 *  instantly and run the (synchronous) highlight later, off the typing frame. */
export type PreviewWindow =
  | { readonly status: "skipped"; readonly reason: "too-large" | "binary" }
  | { readonly status: "error"; readonly message: string }
  | {
      readonly status: "ready";
      readonly text: string;
      readonly ext: string;
      readonly eof: boolean;
      readonly focusLine?: number;
    };

/** Read the bounded window. No highlighting (no syntax parse) happens here. */
export async function readPreviewWindow(
  target: PreviewTarget,
  read: PreviewReader,
): Promise<PreviewWindow> {
  if (target.size > PREVIEW_MAX_FILE_BYTES) return { status: "skipped", reason: "too-large" };
  let range: FileTextRange;
  try {
    range = await read(target.path, { offset: 0, maxBytes: PREVIEW_WINDOW_BYTES });
  } catch (caught) {
    return { status: "error", message: caught instanceof Error ? caught.message : String(caught) };
  }
  if (range.binary) return { status: "skipped", reason: "binary" };
  return {
    status: "ready",
    text: range.text,
    ext: extensionOf(target.path),
    eof: range.eof,
    ...(target.focusLine !== undefined ? { focusLine: target.focusLine } : {}),
  };
}

/** Shape a read window into renderable lines. `highlight` omitted → plain text
 *  (no tree-sitter parse), which is what the panel renders first. */
export function shapePreview(
  window: Extract<PreviewWindow, { status: "ready" }>,
  highlight?: PreviewHighlighter,
): PreviewState {
  const { lines, truncated } = shapeLines(window.text, window.ext, highlight);
  return {
    status: "ready",
    lines,
    truncated: truncated || !window.eof,
    // Keep focus only when it lands inside the rendered window; a query like
    // `file.ts:1000` against an 800-line head must not highlight an unrelated row.
    ...(window.focusLine !== undefined && window.focusLine >= 0 && window.focusLine < lines.length
      ? { focusLine: window.focusLine }
      : {}),
  };
}

export async function loadFilePreview(
  target: PreviewTarget,
  deps: { readonly read: PreviewReader; readonly highlight: PreviewHighlighter },
): Promise<PreviewState> {
  const window = await readPreviewWindow(target, deps.read);
  return window.status === "ready" ? shapePreview(window, deps.highlight) : window;
}
