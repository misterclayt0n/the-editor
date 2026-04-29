use std::ops::Range;

use ropey::RopeSlice;

use super::{
  DocumentFormatter,
  Position,
  SurfaceMeasurements,
  SyntaxSpan,
  TextAnnotations,
  TextFormat,
  Viewport,
  char_at_visual_pos,
};
use crate::{
  grapheme::Grapheme,
  syntax::SyntaxHighlightRun,
};

pub fn viewport_char_range(
  text: RopeSlice<'_>,
  format: &TextFormat,
  annotations: &TextAnnotations,
  viewport: Viewport,
  row_margin: usize,
) -> Range<usize> {
  if text.len_chars() == 0 {
    return 0..0;
  }

  let start_row = viewport.first_row.saturating_sub(row_margin);
  let end_row = viewport
    .first_row
    .saturating_add(viewport.row_count)
    .saturating_add(row_margin);
  let start_col = viewport.first_col;
  let end_col = viewport
    .first_col
    .saturating_add(viewport.wrap_width.max(1));

  let start = char_at_visual_pos(text, format, viewport.wrap_width, annotations, Position {
    row: start_row,
    col: start_col,
  })
  .unwrap_or(0);

  let end = char_at_visual_pos(text, format, viewport.wrap_width, annotations, Position {
    row: end_row,
    col: end_col,
  })
  .unwrap_or(text.len_chars());

  start.min(text.len_chars())..end.max(start).min(text.len_chars())
}

pub fn project_visible_syntax_spans(
  text: RopeSlice<'_>,
  format: &TextFormat,
  annotations: &TextAnnotations,
  runs: &[SyntaxHighlightRun],
  viewport: Viewport,
  measurements: SurfaceMeasurements,
) -> Vec<SyntaxSpan> {
  if runs.is_empty() {
    return Vec::new();
  }

  let mut spans: Vec<SyntaxSpan> = Vec::new();
  let mut next_run = 0;
  let mut active: Vec<(&SyntaxHighlightRun, usize)> = Vec::new();
  let mut formatter = DocumentFormatter::new(text, format, viewport.wrap_width, annotations);

  for grapheme in &mut formatter {
    if grapheme.source.is_eof() {
      break;
    }
    if grapheme.is_virtual() || grapheme.raw == Grapheme::Newline {
      continue;
    }

    let visual_row = grapheme.visual_pos.row;
    if visual_row < viewport.first_row || visual_row >= viewport.first_row + viewport.row_count {
      continue;
    }

    let byte_start = text.char_to_byte(grapheme.char_idx);
    let byte_end = text.char_to_byte(grapheme.char_idx + grapheme.doc_chars());

    while let Some(run) = runs.get(next_run) {
      if run.byte_range.start > byte_start {
        break;
      }
      if run.byte_range.end > byte_start {
        active.push((run, run.byte_range.end));
      }
      next_run += 1;
    }

    active.retain(|(_, end)| *end > byte_start);
    let Some((run, _)) = active.last().copied() else {
      continue;
    };

    let Some((start_col, end_col)) = project_visible_cols(
      grapheme.visual_pos.col,
      grapheme.visual_pos.col + grapheme.width(),
      viewport,
    ) else {
      continue;
    };

    let span = SyntaxSpan {
      surface_row: visual_row - viewport.first_row,
      start_col,
      end_col,
      x_px: start_col as u32 * measurements.cell_width_px,
      width_px: (end_col - start_col) as u32 * measurements.cell_width_px,
      kind: run.kind,
    };

    if let Some(last) = spans.last_mut()
      && last.surface_row == span.surface_row
      && last.kind == span.kind
      && last.end_col == span.start_col
    {
      last.end_col = span.end_col;
      last.width_px = (last.end_col - last.start_col) as u32 * measurements.cell_width_px;
    } else {
      spans.push(span);
    }

    if byte_end > byte_start {
      active.retain(|(_, end)| *end > byte_end.saturating_sub(1));
    }
  }

  spans
}

fn project_visible_cols(
  start_col: usize,
  end_col: usize,
  viewport: Viewport,
) -> Option<(usize, usize)> {
  let visible_start = viewport.first_col;
  let visible_end = viewport.first_col.saturating_add(viewport.wrap_width);

  if end_col <= visible_start || start_col >= visible_end {
    return None;
  }

  Some((
    start_col.max(visible_start) - visible_start,
    end_col.min(visible_end) - visible_start,
  ))
}
