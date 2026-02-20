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
//!     GutterConfig,
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
//! let gutter = GutterConfig::default();
//!
//! let plan = build_plan(
//!   &doc,
//!   view,
//!   &text_fmt,
//!   &gutter,
//!   &mut annotations,
//!   &mut highlights,
//!   &mut cache,
//!   styles,
//! );
//! assert_eq!(plan.lines.len(), 1);
//! ```

use std::collections::BTreeMap;

use the_core::grapheme::{
  Grapheme,
  GraphemeStr,
};
use the_stdx::rope::RopeSliceExt;

use crate::{
  Tendril,
  diagnostics::DiagnosticSeverity,
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
    gutter::{
      GutterConfig,
      GutterType,
      LineNumberMode,
    },
    overlay::OverlayNode,
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
pub struct RenderVisibleRow {
  pub row:               u16,
  pub doc_line:          usize,
  pub first_visual_line: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderGutterSpan {
  pub col:   u16,
  pub text:  Tendril,
  pub style: Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderGutterColumn {
  pub kind:  GutterType,
  pub col:   u16,
  pub width: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderGutterLine {
  pub row:   u16,
  pub spans: Vec<RenderGutterSpan>,
}

impl RenderGutterLine {
  fn new(row: u16) -> Self {
    Self {
      row,
      spans: Vec::new(),
    }
  }

  fn push_span(&mut self, span: RenderGutterSpan) {
    if span.text.is_empty() {
      return;
    }
    self.spans.push(span);
  }

  fn sort_spans(&mut self) {
    self.spans.sort_by_key(|span| span.col);
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
  pub gutter:        Style,
  pub gutter_active: Style,
}

impl Default for RenderStyles {
  fn default() -> Self {
    Self {
      selection:     Style::default(),
      cursor:        Style::default(),
      active_cursor: Style::default(),
      gutter:        Style::default(),
      gutter_active: Style::default(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct RenderPlan {
  pub viewport:         Rect,
  pub scroll:           Position,
  pub content_offset_x: u16,
  pub gutter_columns:   Vec<RenderGutterColumn>,
  pub visible_rows:     Vec<RenderVisibleRow>,
  pub gutter_lines:     Vec<RenderGutterLine>,
  pub lines:            Vec<RenderLine>,
  pub cursors:          Vec<RenderCursor>,
  pub selections:       Vec<RenderSelection>,
  pub overlays:         Vec<OverlayNode>,
}

impl RenderPlan {
  pub fn empty(viewport: Rect, scroll: Position) -> Self {
    Self {
      viewport,
      scroll,
      content_offset_x: 0,
      gutter_columns: Vec::new(),
      visible_rows: Vec::new(),
      gutter_lines: Vec::new(),
      lines: Vec::new(),
      cursors: Vec::new(),
      selections: Vec::new(),
      overlays: Vec::new(),
    }
  }

  pub fn content_width(&self) -> usize {
    self.viewport.width.saturating_sub(self.content_offset_x) as usize
  }

  pub fn gutter_column(&self, kind: GutterType) -> Option<RenderGutterColumn> {
    self
      .gutter_columns
      .iter()
      .copied()
      .find(|column| column.kind == kind)
  }
}

impl Default for RenderPlan {
  fn default() -> Self {
    Self::empty(Rect::default(), Position::default())
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
  gutter: &GutterConfig,
  annotations: &'t mut TextAnnotations<'a>,
  highlights: &mut H,
  cache: &mut RenderCache,
  styles: RenderStyles,
) -> RenderPlan {
  let mut plan = RenderPlan::empty(view.viewport, view.scroll);
  let text = doc.text().slice(..);

  let line_number_width = line_number_column_width(doc, gutter);
  plan.gutter_columns = build_gutter_columns(gutter, line_number_width);
  plan.content_offset_x = if view.viewport.width == 0 {
    0
  } else {
    gutter_columns_width(&plan.gutter_columns).min(view.viewport.width.saturating_sub(1))
  };
  let content_width = if text_fmt.viewport_width == 0 {
    view.viewport.width.max(1) as usize
  } else {
    text_fmt.viewport_width as usize
  };

  cache.reset_if_stale(
    doc.version(),
    annotations.generation(),
    text_fmt.signature(),
  );

  let row_start = view.scroll.row;
  let row_end = row_start + view.viewport.height as usize;
  let col_start = view.scroll.col;

  let has_line_annotations = annotations.has_line_annotations();
  let use_fast_start = !text_fmt.soft_wrap && !has_line_annotations;
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
  } else if has_line_annotations {
    (0, Position::new(0, 0))
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
    let mut visible_rows: Vec<Option<RenderVisibleRow>> = vec![None; view.viewport.height as usize];

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

      if abs_row >= row_start && abs_row < row_end {
        let row = abs_row - row_start;
        if row < visible_rows.len() {
          let first_visual_line = grapheme.char_idx == text.line_to_char(grapheme.line_idx);
          match &mut visible_rows[row] {
            Some(meta) => {
              meta.first_visual_line |= first_visual_line;
            },
            None => {
              visible_rows[row] = Some(RenderVisibleRow {
                row: row as u16,
                doc_line: grapheme.line_idx,
                first_visual_line,
              });
            },
          }
        }
      }

      if grapheme.raw == Grapheme::Newline {
        // Render the newline as a space character, matching Helix behavior
        // where newlines are selectable/visible graphemes occupying 1 cell.
        if abs_row >= row_start && abs_row < row_end && abs_col >= col_start {
          let col = abs_col - col_start;
          if col < content_width {
            let row = abs_row - row_start;
            if current_row != Some(abs_row) {
              if let Some(prev_row) = current_row {
                if prev_row >= row_start && prev_row < row_end {
                  plan.lines.push(current_line);
                }
              }
              current_row = Some(abs_row);
              current_line = RenderLine::new(row as u16);
            }
            let highlight = match grapheme.source {
              GraphemeSource::VirtualText { highlight } => highlight,
              GraphemeSource::Document {
                highlight: Some(highlight),
                ..
              } => Some(highlight),
              GraphemeSource::Document {
                highlight: None, ..
              } => highlights.highlight_at(grapheme.char_idx),
            };
            current_line.push_span(RenderSpan {
              col: col as u16,
              cols: 1,
              text: " ".into(),
              highlight,
              is_virtual: false,
            });
          }
        }

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
      if col >= content_width {
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
          GraphemeSource::Document {
            highlight: Some(highlight),
            ..
          } => Some(highlight),
          GraphemeSource::Document {
            highlight: None, ..
          } => highlights.highlight_at(grapheme.char_idx),
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
    plan.visible_rows = visible_rows.into_iter().flatten().collect();
    plan.gutter_lines = build_gutter_lines(
      &plan.visible_rows,
      doc,
      view,
      gutter,
      &plan.gutter_columns,
      styles,
    );
  }

  add_selections_and_cursor(&mut plan, doc, text_fmt, annotations, view, styles);

  plan
}

fn line_number_column_width(doc: &Document, gutter: &GutterConfig) -> usize {
  if !gutter.layout.contains(&GutterType::LineNumbers) {
    return 0;
  }
  let lines = doc.text().len_lines().max(1);
  gutter.line_numbers.min_width.max(lines.to_string().len())
}

fn build_gutter_columns(
  gutter: &GutterConfig,
  line_number_width: usize,
) -> Vec<RenderGutterColumn> {
  let mut out = Vec::with_capacity(gutter.layout.len());
  let mut col = 0u16;
  for kind in &gutter.layout {
    let width = match kind {
      GutterType::Diagnostics | GutterType::Diff | GutterType::Spacer => 1,
      GutterType::LineNumbers => line_number_width as u16,
    };
    if width == 0 {
      continue;
    }
    out.push(RenderGutterColumn {
      kind: *kind,
      col,
      width,
    });
    col = col.saturating_add(width);
  }
  out
}

fn gutter_columns_width(columns: &[RenderGutterColumn]) -> u16 {
  columns
    .iter()
    .map(|column| column.col.saturating_add(column.width))
    .max()
    .unwrap_or(0)
}

pub fn gutter_width_for_document(
  doc: &Document,
  viewport_width: u16,
  gutter: &GutterConfig,
) -> u16 {
  let columns = build_gutter_columns(gutter, line_number_column_width(doc, gutter));
  if viewport_width == 0 {
    return 0;
  }
  gutter_columns_width(&columns).min(viewport_width.saturating_sub(1))
}

fn build_gutter_lines(
  visible_rows: &[RenderVisibleRow],
  doc: &Document,
  view: ViewState,
  gutter: &GutterConfig,
  columns: &[RenderGutterColumn],
  styles: RenderStyles,
) -> Vec<RenderGutterLine> {
  if columns.is_empty() {
    return Vec::new();
  }

  let active_line = active_doc_line(doc, view);
  let mut out = Vec::with_capacity(visible_rows.len());

  for row in visible_rows {
    let mut line = RenderGutterLine::new(row.row);
    for column in columns {
      match column.kind {
        GutterType::LineNumbers => {
          if row.first_visual_line
            && let Some(text) = line_number_text(
              gutter.line_numbers.mode,
              row.doc_line,
              active_line,
              column.width as usize,
            )
          {
            let style = if active_line.is_some_and(|line| line == row.doc_line) {
              styles.gutter_active
            } else {
              styles.gutter
            };
            line.push_span(RenderGutterSpan {
              col: column.col,
              text: text.into(),
              style,
            });
          }
        },
        GutterType::Diagnostics | GutterType::Diff | GutterType::Spacer => {},
      }
    }
    line.sort_spans();
    out.push(line);
  }

  out
}

fn line_number_text(
  mode: LineNumberMode,
  doc_line: usize,
  active_line: Option<usize>,
  width: usize,
) -> Option<String> {
  if width == 0 {
    return None;
  }

  let absolute = doc_line.saturating_add(1);
  let value = match mode {
    LineNumberMode::Absolute => absolute,
    LineNumberMode::Relative => {
      match active_line {
        Some(active) if active == doc_line => absolute,
        Some(active) => active.abs_diff(doc_line),
        None => absolute,
      }
    },
  };
  Some(format!("{value:>width$}"))
}

fn active_doc_line(doc: &Document, view: ViewState) -> Option<usize> {
  let text = doc.text().slice(..);
  let selection = doc.selection();
  if let Some(active_cursor) = view.active_cursor
    && let Some((_, range)) = selection
      .iter_with_ids()
      .find(|(cursor_id, _)| *cursor_id == active_cursor)
  {
    return Some(text.char_to_line(range.cursor(text)));
  }
  let range = selection.ranges().first()?;
  Some(text.char_to_line(range.cursor(text)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderDiagnosticGutterStyles {
  pub error:   Style,
  pub warning: Style,
  pub info:    Style,
  pub hint:    Style,
}

impl Default for RenderDiagnosticGutterStyles {
  fn default() -> Self {
    Self {
      error:   Style::default(),
      warning: Style::default(),
      info:    Style::default(),
      hint:    Style::default(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderGutterDiffKind {
  Added,
  Modified,
  Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderDiffGutterStyles {
  pub added:    Style,
  pub modified: Style,
  pub removed:  Style,
}

impl Default for RenderDiffGutterStyles {
  fn default() -> Self {
    Self {
      added:    Style::default(),
      modified: Style::default(),
      removed:  Style::default(),
    }
  }
}

pub fn apply_diagnostic_gutter_markers(
  plan: &mut RenderPlan,
  diagnostics_by_line: &BTreeMap<usize, DiagnosticSeverity>,
  styles: RenderDiagnosticGutterStyles,
) {
  let Some(column) = plan.gutter_column(GutterType::Diagnostics) else {
    return;
  };

  for (meta, line) in plan.visible_rows.iter().zip(plan.gutter_lines.iter_mut()) {
    line
      .spans
      .retain(|span| span.col < column.col || span.col >= column.col.saturating_add(column.width));

    if !meta.first_visual_line {
      continue;
    }

    let Some(severity) = diagnostics_by_line.get(&meta.doc_line).copied() else {
      continue;
    };

    let style = match severity {
      DiagnosticSeverity::Error => styles.error,
      DiagnosticSeverity::Warning => styles.warning,
      DiagnosticSeverity::Information => styles.info,
      DiagnosticSeverity::Hint => styles.hint,
    };
    line.push_span(RenderGutterSpan {
      col: column.col,
      text: "●".into(),
      style,
    });
    line.sort_spans();
  }
}

pub fn apply_diff_gutter_markers(
  plan: &mut RenderPlan,
  diff_by_line: &BTreeMap<usize, RenderGutterDiffKind>,
  styles: RenderDiffGutterStyles,
) {
  let Some(column) = plan.gutter_column(GutterType::Diff) else {
    return;
  };

  for (meta, line) in plan.visible_rows.iter().zip(plan.gutter_lines.iter_mut()) {
    line
      .spans
      .retain(|span| span.col < column.col || span.col >= column.col.saturating_add(column.width));

    if !meta.first_visual_line {
      continue;
    }

    let Some(kind) = diff_by_line.get(&meta.doc_line).copied() else {
      continue;
    };

    let (text, style) = match kind {
      RenderGutterDiffKind::Added => ("+", styles.added),
      RenderGutterDiffKind::Modified => ("~", styles.modified),
      RenderGutterDiffKind::Removed => ("-", styles.removed),
    };
    line.push_span(RenderGutterSpan {
      col: column.col,
      text: text.into(),
      style,
    });
    line.sort_spans();
  }
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
  let row_visible_end_cols = visible_line_end_cols(plan, doc, text_fmt, annotations);
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

      push_selection_rects(plan, start, end, styles.selection, &row_visible_end_cols);
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
  let col_end = col_start + plan.content_width();

  if pos.row < row_start || pos.row >= row_end {
    return None;
  }
  if pos.col < col_start || pos.col >= col_end {
    return None;
  }

  Some(Position::new(pos.row - row_start, pos.col - col_start))
}

fn visible_line_end_cols<'a>(
  plan: &RenderPlan,
  doc: &'a Document,
  text_fmt: &'a TextFormat,
  annotations: &mut TextAnnotations<'a>,
) -> Vec<usize> {
  if !text_fmt.soft_wrap && !annotations.has_line_annotations() {
    let mut row_end_cols = vec![plan.scroll.col; plan.viewport.height as usize];
    let text = doc.text().slice(..);

    for (row, end_col) in row_end_cols.iter_mut().enumerate() {
      let abs_row = plan.scroll.row + row;
      if abs_row >= text.len_lines() {
        break;
      }

      let line = text.line(abs_row);
      let mut visual_col = 0usize;
      let mut has_line_ending = false;
      for grapheme in line.graphemes() {
        let g = grapheme_str(grapheme);
        let g = Grapheme::new(g, visual_col, text_fmt.tab_width);
        if matches!(g, Grapheme::Newline) {
          has_line_ending = true;
        }
        visual_col += g.width();
      }

      // Make newline/eof selection visible, matching Helix behavior where
      // both line endings and EOF are selectable graphemes.
      if !has_line_ending {
        visual_col = visual_col.saturating_add(1);
      }

      *end_col = plan.scroll.col + visual_col;
    }

    return row_end_cols;
  }

  let mut row_end_cols = vec![plan.scroll.col; plan.viewport.height as usize];
  for line in &plan.lines {
    let row = line.row as usize;
    if row >= row_end_cols.len() {
      continue;
    }

    let end_col = line
      .spans
      .iter()
      .map(|span| plan.scroll.col + span.end_col() as usize)
      .max()
      .unwrap_or(plan.scroll.col);
    row_end_cols[row] = row_end_cols[row].max(end_col);
  }
  row_end_cols
}

fn grapheme_str<'a>(grapheme: ropey::RopeSlice<'a>) -> GraphemeStr<'a> {
  match grapheme.as_str() {
    Some(slice) => GraphemeStr::from(slice),
    None => GraphemeStr::from(grapheme.to_string()),
  }
}

fn row_visible_end_col(plan: &RenderPlan, row: usize, row_visible_end_cols: &[usize]) -> usize {
  let row_start = plan.scroll.row;
  let col_start = plan.scroll.col;
  let col_end = col_start + plan.content_width();
  let relative = row.saturating_sub(row_start);
  row_visible_end_cols
    .get(relative)
    .copied()
    .unwrap_or(col_start)
    .min(col_end)
}

fn push_selection_rects(
  plan: &mut RenderPlan,
  start: Position,
  end: Position,
  style: Style,
  row_visible_end_cols: &[usize],
) {
  let row_start = plan.scroll.row;
  let row_end = row_start + plan.viewport.height as usize;
  let col_start = plan.scroll.col;
  let col_end = col_start + plan.content_width();

  let start_row = start.row;
  let end_row = end.row;

  if start_row == end_row {
    let row = start_row;
    if row < row_start || row >= row_end {
      return;
    }
    let from = start.col.min(end.col);
    let mut to = start.col.max(end.col);
    let from = from.max(col_start);
    to = to.min(row_visible_end_col(plan, row, row_visible_end_cols));
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

    let row_end_col = row_visible_end_col(plan, row, row_visible_end_cols);
    let (from, to) = if row == start_row {
      (start.col, row_end_col)
    } else if row == end_row {
      (col_start, end.col.min(row_end_col))
    } else {
      (col_start, row_end_col)
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
    diagnostics::DiagnosticSeverity,
    document::{
      Document,
      DocumentId,
    },
    render::{
      GutterConfig,
      SyntaxHighlightAdapter,
      text_annotations::Overlay,
    },
    selection::{
      Range,
      Selection,
    },
    syntax::HighlightCache,
  };

  fn no_gutter() -> GutterConfig {
    GutterConfig {
      layout: Vec::new(),
      ..GutterConfig::default()
    }
  }

  #[test]
  fn build_plan_simple_text() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("abc"));
    let view = ViewState::new(Rect::new(0, 0, 10, 1), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let gutter = no_gutter();

    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
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
    let gutter = no_gutter();

    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    assert_eq!(plan.lines.len(), 1);
    assert_eq!(plan.lines[0].text(), "b ");
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
    let gutter = no_gutter();
    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();

    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    assert_eq!(plan.selections.len(), 2);
    assert_eq!(plan.selections[0].rect, Rect::new(2, 0, 3, 1));
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
    let gutter = no_gutter();

    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
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

  #[test]
  fn build_plan_applies_overlay_annotation_highlight() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("abc"));
    let view = ViewState::new(Rect::new(0, 0, 10, 1), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let overlay = vec![Overlay::new(1, "X")];
    let mut annotations = TextAnnotations::default();
    let _ = annotations.add_overlay(&overlay, Some(crate::syntax::Highlight::new(7)));

    let mut highlights = NoHighlights;
    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let gutter = no_gutter();

    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    let has_overlay_highlight = plan.lines[0].spans.iter().any(|span| {
      span.highlight == Some(crate::syntax::Highlight::new(7)) && span.text.as_str().contains('X')
    });
    assert!(has_overlay_highlight);
  }

  #[test]
  fn build_plan_exposes_visible_row_metadata() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("a\n\nabcdef"));
    let view = ViewState::new(Rect::new(0, 0, 4, 4), Position::new(0, 0));
    let mut text_fmt = TextFormat::default();
    text_fmt.soft_wrap = true;
    text_fmt.viewport_width = 4;

    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let gutter = no_gutter();
    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    assert_eq!(plan.visible_rows.len(), 4);
    assert_eq!(plan.visible_rows[0].doc_line, 0);
    assert!(plan.visible_rows[0].first_visual_line);
    assert_eq!(plan.visible_rows[1].doc_line, 1);
    assert!(plan.visible_rows[1].first_visual_line);
    assert_eq!(plan.visible_rows[2].doc_line, 2);
    assert!(plan.visible_rows[2].first_visual_line);
    assert_eq!(plan.visible_rows[3].doc_line, 2);
    assert!(!plan.visible_rows[3].first_visual_line);
  }

  #[test]
  fn build_plan_generates_line_number_gutter_payload() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("a\nb\n"));
    let view = ViewState::new(Rect::new(0, 0, 20, 2), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let gutter = GutterConfig::default();

    let plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );

    assert!(plan.content_offset_x > 0);
    assert_eq!(plan.gutter_lines.len(), 2);
    let line0_text = plan.gutter_lines[0]
      .spans
      .iter()
      .map(|span| span.text.as_str())
      .collect::<Vec<_>>()
      .join("");
    let line1_text = plan.gutter_lines[1]
      .spans
      .iter()
      .map(|span| span.text.as_str())
      .collect::<Vec<_>>()
      .join("");
    assert!(line0_text.contains('1'));
    assert!(line1_text.contains('2'));
  }

  #[test]
  fn apply_diagnostic_markers_to_gutter_column() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("a\nb\n"));
    let view = ViewState::new(Rect::new(0, 0, 20, 2), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let gutter = GutterConfig::default();

    let mut plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );
    let mut diagnostics = BTreeMap::new();
    diagnostics.insert(1, DiagnosticSeverity::Warning);
    apply_diagnostic_gutter_markers(
      &mut plan,
      &diagnostics,
      RenderDiagnosticGutterStyles::default(),
    );

    let row1 = plan
      .gutter_lines
      .iter()
      .find(|line| line.row == 1)
      .expect("row 1 exists");
    assert!(row1.spans.iter().any(|span| span.text == "●"));
  }

  #[test]
  fn apply_diff_markers_to_gutter_column() {
    let id = DocumentId::new(std::num::NonZeroUsize::new(1).unwrap());
    let doc = Document::new(id, Rope::from("a\nb\n"));
    let view = ViewState::new(Rect::new(0, 0, 20, 2), Position::new(0, 0));
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let mut cache = RenderCache::default();
    let styles = RenderStyles::default();
    let gutter = GutterConfig::default();

    let mut plan = build_plan(
      &doc,
      view,
      &text_fmt,
      &gutter,
      &mut annotations,
      &mut highlights,
      &mut cache,
      styles,
    );
    let mut diff = BTreeMap::new();
    diff.insert(0, RenderGutterDiffKind::Modified);
    apply_diff_gutter_markers(&mut plan, &diff, RenderDiffGutterStyles::default());

    let row0 = plan
      .gutter_lines
      .iter()
      .find(|line| line.row == 0)
      .expect("row 0 exists");
    assert!(row0.spans.iter().any(|span| span.text == "~"));
  }
}
