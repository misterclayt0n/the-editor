use std::ops::Range;

use ropey::RopeSlice;
use unicode_segmentation::UnicodeSegmentation;

use super::{
  display_list::{
    RenderRow,
    Viewport,
  },
  doc_formatter::DocumentFormatter,
  layout_index::{
    LayoutIndex,
    LayoutInputs,
  },
  text_format::TextFormat,
};
use crate::{
  Tendril,
  grapheme::{
    Grapheme,
    grapheme_width,
  },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VisibleRowMetadata {
  pub(super) doc_line:             Option<usize>,
  pub(super) starts_logical_line:  bool,
  pub(super) wrapped_continuation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct VisibleRow {
  pub(super) text:            Tendril,
  pub(super) anchor_line:     usize,
  pub(super) doc_visual_row:  Option<usize>,
  pub(super) abs_surface_row: usize,
  pub(super) row_end_char:    usize,
  pub(super) row_end_col_abs: usize,
  pub(super) metadata:        VisibleRowMetadata,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct VisibleLayout {
  pub(super) text_revision:    Option<u64>,
  pub(super) wrap_width:       usize,
  pub(super) window_start_row: usize,
  pub(super) window_end_row:   usize,
  pub(super) rows:             Vec<VisibleRow>,
  pub(super) units:            Vec<VisibleUnit>,
  pub(super) cursor_positions: Vec<CursorPosition>,
}

impl VisibleLayout {
  pub(super) fn document_ranges(&self) -> (Range<usize>, Range<usize>) {
    let char_range = self
      .units
      .first()
      .map(|unit| unit.char_start)
      .zip(self.units.last().map(|unit| unit.char_end))
      .map(|(start, end)| start..end)
      .unwrap_or(0..0);
    let byte_range = self
      .units
      .first()
      .map(|unit| unit.byte_start)
      .zip(self.units.last().map(|unit| unit.byte_end))
      .map(|(start, end)| start..end)
      .unwrap_or(0..0);

    (char_range, byte_range)
  }

  pub(super) fn build_rows(&self, viewport: Viewport) -> Vec<RenderRow> {
    self
      .rows
      .iter()
      .filter(|row| row.abs_surface_row >= viewport.first_row)
      .filter(|row| row.abs_surface_row < viewport.first_row.saturating_add(viewport.row_count))
      .map(|row| {
        RenderRow {
          surface_row:          row.abs_surface_row - viewport.first_row,
          text:                 crop_row_text(&row.text, viewport),
          doc_line:             row.metadata.doc_line,
          starts_logical_line:  row.metadata.starts_logical_line,
          wrapped_continuation: row.metadata.wrapped_continuation,
        }
      })
      .collect()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VisibleUnit {
  pub(super) surface_row_abs: usize,
  pub(super) char_start:      usize,
  pub(super) char_end:        usize,
  pub(super) byte_start:      usize,
  pub(super) byte_end:        usize,
  pub(super) start_col_abs:   usize,
  pub(super) end_col_abs:     usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CursorPosition {
  pub(super) char_idx:        usize,
  pub(super) surface_row_abs: usize,
  pub(super) col_abs:         usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct VisibleLayoutBuilder<'a> {
  text:          RopeSlice<'a>,
  text_revision: u64,
  format:        &'a TextFormat,
}

impl<'a> VisibleLayoutBuilder<'a> {
  pub(super) fn new(text: RopeSlice<'a>, text_revision: u64, format: &'a TextFormat) -> Self {
    Self {
      text,
      text_revision,
      format,
    }
  }

  pub(super) fn build(self, viewport: Viewport, layout_index: &mut LayoutIndex) -> VisibleLayout {
    let inputs = LayoutInputs {
      text:          self.text,
      text_revision: self.text_revision,
      format:        self.format,
      wrap_width:    viewport.wrap_width,
    };
    let window_start_row = viewport
      .first_row
      .saturating_sub(VISIBLE_LAYOUT_MARGIN_ROWS);
    let window_end_row = viewport
      .first_row
      .saturating_add(viewport.row_count)
      .saturating_add(VISIBLE_LAYOUT_MARGIN_ROWS);
    let layout_start = layout_index.visible_layout_start(inputs, window_start_row);
    let mut formatter = DocumentFormatter::new_at_checkpoint(
      self.text,
      self.format,
      viewport.wrap_width,
      layout_start.checkpoint,
    );

    let mut rows = Vec::new();
    let mut units = Vec::new();
    let mut cursor_positions = Vec::new();
    let mut current_surface_row_abs = layout_start.surface_row;
    let mut current = None::<RowBuilder>;

    while let Some(grapheme) = formatter.next() {
      let abs_doc_visual_row = layout_start.line_doc_visual_row + grapheme.visual_pos.row;

      if current
        .as_ref()
        .is_some_and(|row| row.visual_row != abs_doc_visual_row)
      {
        let previous = current.take().unwrap();
        let wrapped_row_boundary_char = previous.row_end_char;
        let last_for_line = previous.line_idx != grapheme.line_idx;
        finalize_visible_row(
          previous,
          window_start_row,
          window_end_row,
          &mut current_surface_row_abs,
          &mut rows,
          &mut cursor_positions,
        );
        if !last_for_line
          && current_surface_row_abs >= window_start_row
          && current_surface_row_abs < window_end_row
        {
          cursor_positions.push(CursorPosition {
            char_idx:        wrapped_row_boundary_char,
            surface_row_abs: current_surface_row_abs,
            col_abs:         grapheme.visual_pos.col,
          });
        }
        if current_surface_row_abs >= window_end_row {
          break;
        }
      }

      if grapheme.source.is_eof() {
        if let Some(row) = current.take() {
          finalize_visible_row(
            row,
            window_start_row,
            window_end_row,
            &mut current_surface_row_abs,
            &mut rows,
            &mut cursor_positions,
          );
        } else if grapheme.visual_pos.row > 0
          && current_surface_row_abs >= window_start_row
          && current_surface_row_abs < window_end_row
        {
          rows.push(VisibleRow {
            text:            Tendril::new(),
            anchor_line:     grapheme.line_idx,
            doc_visual_row:  Some(abs_doc_visual_row),
            abs_surface_row: current_surface_row_abs,
            row_end_char:    grapheme.char_idx,
            row_end_col_abs: grapheme.visual_pos.col,
            metadata:        VisibleRowMetadata {
              doc_line:             Some(grapheme.line_idx),
              starts_logical_line:  false,
              wrapped_continuation: true,
            },
          });
        }
        break;
      }

      if current.is_none() {
        current = Some(RowBuilder::new(
          abs_doc_visual_row,
          grapheme.line_idx,
          grapheme.char_idx,
          self.text.line_to_char(grapheme.line_idx) == grapheme.char_idx,
        ));
      }

      current.as_mut().unwrap().push(&grapheme);

      if grapheme.raw != Grapheme::Newline {
        let surface_row_abs = current_surface_row_abs;
        if surface_row_abs >= window_start_row && surface_row_abs < window_end_row {
          units.push(VisibleUnit {
            surface_row_abs,
            char_start: grapheme.char_idx,
            char_end: grapheme.char_idx + grapheme.doc_chars(),
            byte_start: self.text.char_to_byte(grapheme.char_idx),
            byte_end: self
              .text
              .char_to_byte(grapheme.char_idx + grapheme.doc_chars()),
            start_col_abs: grapheme.visual_pos.col,
            end_col_abs: grapheme.visual_pos.col + grapheme.width(),
          });
          cursor_positions.push(CursorPosition {
            char_idx: grapheme.char_idx,
            surface_row_abs,
            col_abs: grapheme.visual_pos.col,
          });
        }
      } else if current_surface_row_abs >= window_start_row
        && current_surface_row_abs < window_end_row
      {
        let row_end_col_abs = current.as_ref().map(|row| row.row_end_col_abs).unwrap_or(0);
        cursor_positions.push(CursorPosition {
          char_idx:        grapheme.char_idx,
          surface_row_abs: current_surface_row_abs,
          col_abs:         row_end_col_abs,
        });
      }
    }

    if rows.is_empty() {
      rows.push(VisibleRow {
        text:            Tendril::new(),
        anchor_line:     0,
        doc_visual_row:  Some(0),
        abs_surface_row: 0,
        row_end_char:    0,
        row_end_col_abs: 0,
        metadata:        VisibleRowMetadata {
          doc_line:             Some(0),
          starts_logical_line:  true,
          wrapped_continuation: false,
        },
      });
    }

    rows.sort_by_key(|row| row.abs_surface_row);

    VisibleLayout {
      text_revision: Some(self.text_revision),
      wrap_width: viewport.wrap_width,
      window_start_row,
      window_end_row,
      rows,
      units,
      cursor_positions,
    }
  }
}

#[derive(Debug)]
struct RowBuilder {
  visual_row:          usize,
  text:                String,
  line_idx:            usize,
  row_end_char:        usize,
  row_end_col_abs:     usize,
  starts_logical_line: bool,
}

impl RowBuilder {
  fn new(
    visual_row: usize,
    line_idx: usize,
    first_doc_char: usize,
    starts_logical_line: bool,
  ) -> Self {
    Self {
      visual_row,
      text: String::new(),
      line_idx,
      row_end_char: first_doc_char,
      row_end_col_abs: 0,
      starts_logical_line,
    }
  }

  fn push(&mut self, grapheme: &super::doc_formatter::FormattedGrapheme<'_>) {
    if grapheme.raw != Grapheme::Newline {
      self.row_end_char = grapheme.char_idx + grapheme.doc_chars();
      self.row_end_col_abs = grapheme.visual_pos.col + grapheme.width();
    }

    match grapheme.raw {
      Grapheme::Newline => {},
      Grapheme::Tab { width } => self.text.push_str(&" ".repeat(width)),
      Grapheme::Other { ref g } => self.text.push_str(g.as_ref()),
    }
  }

  fn into_visible_row(self, abs_surface_row: usize) -> VisibleRow {
    let starts_logical_line = self.starts_logical_line;
    VisibleRow {
      text: self.text.into(),
      anchor_line: self.line_idx,
      doc_visual_row: Some(self.visual_row),
      abs_surface_row,
      row_end_char: self.row_end_char,
      row_end_col_abs: self.row_end_col_abs,
      metadata: VisibleRowMetadata {
        doc_line: Some(self.line_idx),
        starts_logical_line,
        wrapped_continuation: !starts_logical_line,
      },
    }
  }
}

fn finalize_visible_row(
  row: RowBuilder,
  window_start_row: usize,
  window_end_row: usize,
  current_surface_row_abs: &mut usize,
  rows: &mut Vec<VisibleRow>,
  cursor_positions: &mut Vec<CursorPosition>,
) {
  let visible_row = row.into_visible_row(*current_surface_row_abs);

  if *current_surface_row_abs >= window_start_row && *current_surface_row_abs < window_end_row {
    rows.push(visible_row.clone());
    cursor_positions.push(CursorPosition {
      char_idx:        visible_row.row_end_char,
      surface_row_abs: *current_surface_row_abs,
      col_abs:         visible_row.row_end_col_abs,
    });
  }

  *current_surface_row_abs += 1;
}

fn crop_row_text(text: &str, viewport: Viewport) -> Tendril {
  if viewport.first_col == 0 {
    let width = row_display_width(text);
    if width <= viewport.wrap_width {
      return text.into();
    }
  }

  let mut result = String::new();
  let visible_start = viewport.first_col;
  let visible_end = viewport.first_col.saturating_add(viewport.wrap_width);
  let mut col = 0;

  for grapheme in UnicodeSegmentation::graphemes(text, true) {
    let width = grapheme_width(grapheme);
    let end_col = col + width;

    if end_col <= visible_start {
      col = end_col;
      continue;
    }
    if col >= visible_end {
      break;
    }

    result.push_str(grapheme);
    col = end_col;
  }

  result.into()
}

fn row_display_width(text: &str) -> usize {
  UnicodeSegmentation::graphemes(text, true)
    .map(grapheme_width)
    .sum()
}

pub(super) const VISIBLE_LAYOUT_MARGIN_ROWS: usize = 8;
