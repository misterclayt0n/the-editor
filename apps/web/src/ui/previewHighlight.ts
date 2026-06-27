import { highlightToSpans, splitSpansByLine } from "../editor/highlightSpans";
import { PREVIEW_WINDOW_BYTES, type PreviewHighlighter } from "./filePreview";

/** Shared detached highlighter for read-only file previews. */
export const highlightPreview: PreviewHighlighter = (code, ext) => {
  const flat = highlightToSpans(code, ext, { maxChars: PREVIEW_WINDOW_BYTES, budgetMs: 8 });
  return flat ? splitSpansByLine(flat) : null;
};
