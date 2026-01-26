//! Render plan construction.
//!
//! A render plan is a backend-agnostic description of what to draw for a given
//! document and viewport. Clients consume the plan and handle actual drawing.
//!
//! # Example
//!
//! ```no_run
//! use ropey::Rope;
//! use the_lib::{
//!   document::{
//!     Document,
//!     DocumentId,
//!   },
//!   position::Position,
//!   render::{
//!     NoHighlights,
//!     RenderCache,
//!     RenderStyles,
//!     build_plan,
//!     graphics::Rect,
//!     text_annotations::TextAnnotations,
//!     text_format::TextFormat,
//!   },
//!   view::ViewState,
//! };
//!
//! let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
//! let doc = Document::new(id, Rope::from("hello"));
//! let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
//! let text_fmt = TextFormat::default();
//! let mut annotations = TextAnnotations::default();
//! let mut highlights = NoHighlights;
//! let mut cache = RenderCache::default();
//! let styles = RenderStyles::default();
//!
//! let plan = build_plan(
//!   &doc,
//!   view,
//!   &text_fmt,
//!   &mut annotations,
//!   &mut highlights,
//!   &mut cache,
//!   styles,
//! );
//! assert_eq!(plan.lines.len(), 1);
//! ```

use std::collections::BTreeMap;

use the_core::grapheme::Grapheme;

use crate::{
  Tendril,
  document::Document,
  position::Position,
  render::{
    FormattedGrapheme,
    GraphemeSource,
    doc_formatter::{
      DocumentFormatter,
      prev_checkpoint,
    },
    graphics::{
      CursorKind,
      Rect,
      Style,
    },
    text_annotations::TextAnnotations,
    text_format::TextFormat,
    visual_position,
  },
  syntax::Highlight,
  view::ViewState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderSpan {
  pub col:        u16,
  pub cols:       u16,
  pub text:       Tendril,
  pub highlight:  Option<Highlight>,
  pub is_virtual: bool,
}

impl RenderSpan {
  fn end_col(&self) -> u16 {
    self.col.saturating_add(self.cols)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderLine {
  pub row:   u16,
  pub spans: Vec<RenderSpan>,
}

impl RenderLine {
  fn new(row: u16) -> Self {
    Self {
      row,
      spans: Vec::new(),
    }
  }

  fn push_span(&mut self, span: RenderSpan) {
    if let Some(last) = self.spans.last_mut() {
      if last.is_virtual == span.is_virtual
        && last.highlight == span.highlight
        && last.end_col() == span.col
      {
        last.text.push_str(&span.text);
        last.cols = last.cols.saturating_add(span.cols);
        return;
      }
    }
    self.spans.push(span);
  }

  #[cfg(test)]
  fn text(&self) -> String {
    let mut s = String::new();
    for span in &self.spans {
      s.push_str(&span.text);
    }
    s
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderCursor {
  pub id:    crate::selection::CursorId,
  pub pos:   Position,
  pub kind:  CursorKind,
  pub style: Style,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderSelection {
  pub rect:  Rect,
  pub style: Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderStyles {
  pub selection:     Style,
  pub cursor:        Style,
  pub active_cursor: Style,
}

impl Default for RenderStyles {
  fn default() -> Self {
    Self {
      selection:     Style::default(),
      cursor:        Style::default(),
      active_cursor: Style::default(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct RenderPlan {
  pub viewport:   Rect,
  pub scroll:     Position,
  pub lines:      Vec<RenderLine>,
  pub cursors:    Vec<RenderCursor>,
  pub selections: Vec<RenderSelection>,
}

impl RenderPlan {
  pub fn empty(viewport: Rect, scroll: Position) -> Self {
    Self {
      viewport,
      scroll,
      lines: Vec::new(),
      cursors: Vec::new(),
      selections: Vec::new(),
    }
  }
}

#[derive(Debug, Default)]
pub struct RenderCache {
  text_version:           Option<u64>,
  annotations_generation: Option<u64>,
  format_signature:       Option<crate::render::text_format::TextFormatSignature>,
  by_char:                BTreeMap<usize, Position>,
  by_pos:                 BTreeMap<Position, usize>,
}

impl RenderCache {
  pub(crate) fn reset_if_stale(
    &mut self,
    text_version: u64,
    annotations_generation: u64,
    format_signature: crate::render::text_format::TextFormatSignature,
  ) {
    let stale = self.text_version != Some(text_version)
      || self.annotations_generation != Some(annotations_generation)
      || self.format_signature.as_ref() != Some(&format_signature);
    if stale {
      self.text_version = Some(text_version);
      self.annotations_generation = Some(annotations_generation);
      self.format_signature = Some(format_signature);
      self.by_char.clear();
      self.by_pos.clear();
    }
  }

  pub(crate) fn insert_origin(&mut self, char_idx: usize, pos: Position) {
    if let Some(prev) = self.by_char.insert(char_idx, pos) {
      self.by_pos.remove(&prev);
    }
    if let Some(prev) = self.by_pos.insert(pos, char_idx) {
      self.by_char.remove(&prev);
    }
  }

  pub(crate) fn nearest_origin(&self, target: Position) -> Option<(usize, Position)> {
    let (pos, char_idx) = self.by_pos.range(..=target).next_back()?;
    Some((*char_idx, *pos))
  }
}

pub trait HighlightProvider {
  fn highlight_at(&mut self, char_idx: usize) -> Option<Highlight>;
}

#[derive(Debug, Default)]
pub struct NoHighlights;

impl HighlightProvider for NoHighlights {
  fn highlight_at(&mut self, _char_idx: usize) -> Option<Highlight> {
    None
  }
}

/// Build a render plan for the given document and view state.
///
/// This uses `DocumentFormatter` to produce visual positions. When soft wrap
/// and line annotations are disabled it fast-starts at the scroll position via
/// `visual_position`. When soft wrap is enabled it uses `RenderCache` to start
/// from the nearest cached block origin, avoiding full rescans in steady-state.
#[allow(clippy::too_many_arguments)]
pub fn build_plan<'a, 't, H: HighlightProvider>(
  doc: &'a Document,
  view: ViewState,
  text_fmt: &'a TextFormat,
  annotations: &'t mut TextAnnotations<'a>,
  highlights: &mut H,
  cache: &mut RenderCache,
  styles: RenderStyles,
) -> RenderPlan {
  let mut plan = RenderPlan::empty(view.viewport, view.scroll);
  let text = doc.text().slice(..);

  cache.reset_if_stale(
    doc.version(),
    annotations.generation(),
    text_fmt.signature(),
  );

  let row_start = view.scroll.row;
  let row_end = row_start + view.viewport.height as usize;
  let col_start = view.scroll.col;

  let use_fast_start = !text_fmt.soft_wrap && !annotations.has_line_annotations();
  let (block_char_idx, origin) = if use_fast_start {
    let start_char =
      visual_position::char_at_visual_pos(text, text_fmt, annotations, view.scroll).unwrap_or(0);
    let (block_char_idx, _) = prev_checkpoint(text, start_char);
    let origin = if start_char == 0 {
      Position::new(0, 0)
    } else {
      visual_position::visual_pos_at_char(text, text_fmt, annotations, block_char_idx)
        .unwrap_or_else(|| Position::new(0, 0))
    };
    (block_char_idx, origin)
  } else if let Some((char_idx, pos)) = cache.nearest_origin(view.scroll) {
    (char_idx, pos)
  } else {
    (0, Position::new(0, 0))
  };
  cache.insert_origin(block_char_idx, origin);

  {
    let mut formatter = DocumentFormatter::new_at_prev_checkpoint(
      doc.text().slice(..),
      text_fmt,
      annotations,
      block_char_idx,
    );

    let mut current_row: Option<usize> = None;
    let mut current_line = RenderLine::new(0);

    for grapheme in &mut formatter {
      if grapheme.source.is_eof() {
        break;
      }

      let rel_pos = grapheme.visual_pos;
      let abs_row = origin.row + rel_pos.row;
      let abs_col = if rel_pos.row == 0 {
        origin.col + rel_pos.col
      } else {
        rel_pos.col
      };

      if grapheme.raw == Grapheme::Newline {
        if let Some(row) = current_row {
          if row >= row_start && row < row_end {
            plan.lines.push(current_line);
          }
        }
        current_row = None;
        current_line = RenderLine::new(0);
        continue;
      }

      if abs_row < row_start {
        continue;
      }
      if abs_row >= row_end {
        break;
      }

      if abs_col < col_start {
        continue;
      }

      let col = abs_col - col_start;
      if col >= view.viewport.width as usize {
        continue;
      }

      let row = abs_row - row_start;
      if current_row != Some(abs_row) {
        if current_row.is_some() {
          plan.lines.push(current_line);
        }
        current_row = Some(abs_row);
        current_line = RenderLine::new(row as u16);
      }

      if let Some((text, cols)) = grapheme_text(&grapheme) {
        let highlight = match grapheme.source {
          GraphemeSource::VirtualText { highlight } => highlight,
          _ => highlights.highlight_at(grapheme.char_idx),
        };

        current_line.push_span(RenderSpan {
          col: col as u16,
          cols: cols as u16,
          text,
          highlight,
          is_virtual: grapheme.source.is_virtual(),
        });
      }
    }

    if current_row.is_some() {
      plan.lines.push(current_line);
    }
  }

  add_selections_and_cursor(&mut plan, doc, text_fmt, annotations, view, styles);

  plan
}

fn grapheme_text(grapheme: &FormattedGrapheme<'_>) -> Option<(Tendril, usize)> {
  match grapheme.raw {
    Grapheme::Newline => None,
    Grapheme::Tab { width } => {
      let spaces = " ".repeat(width);
      Some((spaces.into(), width))
    },
    Grapheme::Other { ref g } => {
      let s = g.to_string();
      let width = grapheme.raw.width();
      Some((s.into(), width))
    },
  }
}

fn add_selections_and_cursor<'a>(
  plan: &mut RenderPlan,
  doc: &'a Document,
  text_fmt: &'a TextFormat,
  annotations: &mut TextAnnotations<'a>,
  view: ViewState,
  styles: RenderStyles,
) {
  let selection = doc.selection();
  let cursor_kind = CursorKind::Block;

  for (cursor_id, range) in selection.iter_with_ids() {
    let from = range.from();
    let to = range.to();
    if from == to {
      // Even when empty, still render a cursor below.
    } else {
      let start =
        visual_position::visual_pos_at_char(doc.text().slice(..), text_fmt, annotations, from);
      let end =
        visual_position::visual_pos_at_char(doc.text().slice(..), text_fmt, annotations, to);
      let (Some(start), Some(end)) = (start, end) else {
        continue;
      };

      push_selection_rects(plan, start, end, styles.selection);
    }

    let cursor_pos = range.cursor(doc.text().slice(..));
    if let Some(pos) =
      visual_position::visual_pos_at_char(doc.text().slice(..), text_fmt, annotations, cursor_pos)
    {
      if let Some(pos) = clamp_position(plan, pos) {
        let cursor_style = if view.active_cursor == Some(cursor_id) {
          styles.active_cursor
        } else {
          styles.cursor
        };
        plan.cursors.push(RenderCursor {
          id: cursor_id,
          pos,
          kind: cursor_kind,
          style: cursor_style,
        });
      }
    }
  }
}

fn clamp_position(plan: &RenderPlan, pos: Position) -> Option<Position> {
  let row_start = plan.scroll.row;
  let row_end = row_start + plan.viewport.height as usize;
  let col_start = plan.scroll.col;
  let col_end = col_start + plan.viewport.width as usize;

  if pos.row < row_start || pos.row >= row_end {
    return None;
  }
  if pos.col < col_start || pos.col >= col_end {
    return None;
  }

  Some(Position::new(pos.row - row_start, pos.col - col_start))
}

fn push_selection_rects(plan: &mut RenderPlan, start: Position, end: Position, style: Style) {
  let row_start = plan.scroll.row;
  let row_end = row_start + plan.viewport.height as usize;
  let col_start = plan.scroll.col;
  let col_end = col_start + plan.viewport.width as usize;

  let start_row = start.row;
  let end_row = end.row;

  if start_row == end_row {
    let row = start_row;
    if row < row_start || row >= row_end {
      return;
    }
    let from = start.col.min(end.col);
    let to = start.col.max(end.col);
    let from = from.max(col_start);
    let to = to.min(col_end);
    if to <= from {
      return;
    }
    plan.selections.push(RenderSelection {
      rect: Rect::new(
        (from - col_start) as u16,
        (row - row_start) as u16,
        (to - from) as u16,
        1,
      ),
      style,
    });
    return;
  }

  for row in start_row..=end_row {
    if row < row_start || row >= row_end {
      continue;
    }

    let (from, to) = if row == start_row {
      (start.col, col_end)
    } else if row == end_row {
      (col_start, end.col)
    } else {
      (col_start, col_end)
    };

    let from = from.max(col_start);
    let to = to.min(col_end);
    if to <= from {
      continue;
    }

    plan.selections.push(RenderSelection {
      rect: Rect::new(
        (from - col_start) as u16,
        (row - row_start) as u16,
        (to - from) as u16,
        1,
      ),
      style,
    });
  }
}

#[cfg(test)]
mod tests {
  use ropey::Rope;
  use smallvec::smallvec;

  use super::*;
  use crate::{
    document::{
      Document,
      DocumentId,
    },
    render::SyntaxHighlightAdapter,
    selection::{
      Range,
      Selection,
    },
    syntax::HighlightCache,
  };

  #[test]
  fn build_plan_simple_text() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("abc"));
    let view = ViewState::new(Rect::new(0, 0, 10, 1), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;

    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    assert_eq!(plan.lines.len(), 1);
    assert_eq!(plan.lines[0].text(), "abc");
  }

  #[test]
  fn build_plan_scrolls_rows() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("a\nb\nc"));
    let view = ViewState::new(Rect::new(0, 0, 10, 1), Position::new(1, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;

    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    assert_eq!(plan.lines.len(), 1);
    assert_eq!(plan.lines[0].text(), "b");
  }

  #[test]
  fn build_plan_selection_and_cursor_rects() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let mut doc = Document::new(id, Rope::from("abcd\nefgh\nijkl\n"));
    let selection = Selection::new(smallvec![Range::new(7, 12), Range::point(6)]).unwrap();
    doc.set_selection(selection).unwrap();

    let view = ViewState::new(Rect::new(0, 0, 8, 2), Position::new(1, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();

    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    assert_eq!(plan.selections.len(), 2);
    assert_eq!(plan.selections[0].rect, Rect::new(2, 0, 6, 1));
    assert_eq!(plan.selections[1].rect, Rect::new(0, 1, 2, 1));

    assert_eq!(plan.cursors.len(), 2);
    let cursor_positions: Vec<_> = plan.cursors.iter().map(|c| c.pos).collect();
    assert!(cursor_positions.contains(&Position::new(0, 1)));
    assert!(cursor_positions.contains(&Position::new(1, 1)));
  }

  #[test]
  fn build_plan_applies_highlight_spans() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("abc"));
    let view = ViewState::new(Rect::new(0, 0, 10, 1), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();

    let mut highlight_cache = HighlightCache::default();
    highlight_cache.update_range(
      0..doc.text().len_bytes(),
      vec![(crate::syntax::Highlight::new(1), 1..2)],
      doc.text().slice(..),
      doc.version(),
      1,
    );
    let mut highlights =
      SyntaxHighlightAdapter::from_cache(doc.text().slice(..), &highlight_cache, 0..1);

    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();

    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    let span_highlights: Vec<_> = plan.lines[0]
      .spans
      .iter()
      .filter_map(|span| span.highlight)
      .collect();
    assert!(span_highlights.contains(&crate::syntax::Highlight::new(1)));
  }
}
