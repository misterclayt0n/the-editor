use std::ops::Range;

use ropey::RopeSlice;
use unicode_segmentation::UnicodeSegmentation;

use super::{
  display_list::{
    RowId,
    RowKind,
    RowMetadata,
    RowPlacement,
    Viewport,
    WidgetId,
  },
  doc_formatter::DocumentFormatter,
  layers::BlockWidget,
  layout_index::{
    LayoutIndex,
    LayoutInputs,
  },
  measurement::CellGridMetrics,
  text_annotations::TextAnnotations,
  text_format::TextFormat,
};
use crate::{
  Tendril,
  grapheme::{
    Grapheme,
    grapheme_width,
  },
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum RowKey {
  Document {
    line_idx:       usize,
    first_doc_char: usize,
  },
  Widget {
    widget_id: WidgetId,
    fragment:  u32,
  },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VisibleRowMetadata {
  pub(super) kind:                 RowKind,
  pub(super) doc_line:             Option<usize>,
  pub(super) starts_logical_line:  bool,
  pub(super) wrapped_continuation: bool,
}

impl VisibleRowMetadata {
  pub(super) fn into_row_metadata(self, row_id: RowId, surface_row: usize) -> RowMetadata {
    RowMetadata {
      row_id,
      surface_row,
      kind: self.kind,
      doc_line: self.doc_line,
      starts_logical_line: self.starts_logical_line,
      wrapped_continuation: self.wrapped_continuation,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct VisibleRow {
  pub(super) key:             RowKey,
  pub(super) text:            Tendril,
  pub(super) anchor_line:     usize,
  pub(super) doc_visual_row:  Option<usize>,
  pub(super) abs_surface_row: usize,
  pub(super) row_end_char:    usize,
  pub(super) row_end_col_abs: usize,
  pub(super) metadata:        VisibleRowMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VisibleWidgetLayout {
  pub(super) widget_id: WidgetId,
  pub(super) start_row: usize,
  pub(super) row_span:  usize,
  pub(super) width_px:  u32,
  pub(super) height_px: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct VisibleLayout {
  pub(super) text_revision:    Option<u64>,
  pub(super) wrap_width:       usize,
  pub(super) window_start_row: usize,
  pub(super) window_end_row:   usize,
  pub(super) rows:             Vec<VisibleRow>,
  pub(super) widget_layouts:   Vec<VisibleWidgetLayout>,
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

  pub(super) fn build_rows(
    &self,
    viewport: Viewport,
    measurements: CellGridMetrics,
  ) -> Vec<BuiltRow> {
    self
      .rows
      .iter()
      .filter(|row| row.abs_surface_row >= viewport.first_row)
      .filter(|row| row.abs_surface_row < viewport.first_row.saturating_add(viewport.row_count))
      .map(|row| {
        BuiltRow {
          key:       row.key.clone(),
          placement: RowPlacement::new(
            row.abs_surface_row - viewport.first_row,
            measurements.row_to_y_px(row.abs_surface_row - viewport.first_row),
            measurements.row_height_px,
          ),
          text:      crop_row_text(&row.text, viewport),
          metadata:  row.metadata,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BuiltRow {
  pub(super) key:       RowKey,
  pub(super) placement: RowPlacement,
  pub(super) text:      Tendril,
  pub(super) metadata:  VisibleRowMetadata,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct VisibleLayoutBuilder<'a> {
  text:          RopeSlice<'a>,
  text_revision: u64,
  format:        &'a TextFormat,
  annotations:   &'a TextAnnotations,
  widgets:       &'a [BlockWidget],
  measurements:  CellGridMetrics,
}

impl<'a> VisibleLayoutBuilder<'a> {
  pub(super) fn new(
    text: RopeSlice<'a>,
    text_revision: u64,
    format: &'a TextFormat,
    annotations: &'a TextAnnotations,
    widgets: &'a [BlockWidget],
    measurements: CellGridMetrics,
  ) -> Self {
    Self {
      text,
      text_revision,
      format,
      annotations,
      widgets,
      measurements,
    }
  }

  pub(super) fn build(self, viewport: Viewport, layout_index: &mut LayoutIndex) -> VisibleLayout {
    let inputs = LayoutInputs {
      text:          self.text,
      text_revision: self.text_revision,
      format:        self.format,
      wrap_width:    viewport.wrap_width,
      annotations:   self.annotations,
      widgets:       self.widgets,
      measurements:  self.measurements,
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
      self.annotations,
      layout_start.checkpoint,
    );

    let mut rows = Vec::new();
    let mut units = Vec::new();
    let mut cursor_positions = Vec::new();
    let mut widget_layouts = Vec::new();
    let last_doc_line = self.text.len_lines().saturating_sub(1);
    let mut next_widget = self
      .widgets
      .partition_point(|widget| widget.after_line < layout_start.line_idx);
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
          last_for_line,
          self.widgets,
          self.measurements,
          last_doc_line,
          window_start_row,
          window_end_row,
          &mut current_surface_row_abs,
          &mut next_widget,
          &mut rows,
          &mut widget_layouts,
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
            true,
            self.widgets,
            self.measurements,
            last_doc_line,
            window_start_row,
            window_end_row,
            &mut current_surface_row_abs,
            &mut next_widget,
            &mut rows,
            &mut widget_layouts,
            &mut cursor_positions,
          );
        } else if grapheme.visual_pos.row > 0
          && current_surface_row_abs >= window_start_row
          && current_surface_row_abs < window_end_row
        {
          rows.push(VisibleRow {
            key:             RowKey::Document {
              line_idx:       grapheme.line_idx,
              first_doc_char: grapheme.char_idx,
            },
            text:            Tendril::new(),
            anchor_line:     grapheme.line_idx,
            doc_visual_row:  Some(abs_doc_visual_row),
            abs_surface_row: current_surface_row_abs,
            row_end_char:    grapheme.char_idx,
            row_end_col_abs: grapheme.visual_pos.col,
            metadata:        VisibleRowMetadata {
              kind:                 RowKind::Document,
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
        key:             RowKey::Document {
          line_idx:       0,
          first_doc_char: 0,
        },
        text:            Tendril::new(),
        anchor_line:     0,
        doc_visual_row:  Some(0),
        abs_surface_row: 0,
        row_end_char:    0,
        row_end_col_abs: 0,
        metadata:        VisibleRowMetadata {
          kind:                 RowKind::Document,
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
      widget_layouts,
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
  first_doc_char:      usize,
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
      first_doc_char,
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
      key: RowKey::Document {
        line_idx:       self.line_idx,
        first_doc_char: self.first_doc_char,
      },
      text: self.text.into(),
      anchor_line: self.line_idx,
      doc_visual_row: Some(self.visual_row),
      abs_surface_row,
      row_end_char: self.row_end_char,
      row_end_col_abs: self.row_end_col_abs,
      metadata: VisibleRowMetadata {
        kind: RowKind::Document,
        doc_line: Some(self.line_idx),
        starts_logical_line,
        wrapped_continuation: !starts_logical_line,
      },
    }
  }
}

#[allow(clippy::too_many_arguments)]
fn finalize_visible_row(
  row: RowBuilder,
  last_for_line: bool,
  widgets: &[BlockWidget],
  measurements: CellGridMetrics,
  last_doc_line: usize,
  window_start_row: usize,
  window_end_row: usize,
  current_surface_row_abs: &mut usize,
  next_widget: &mut usize,
  rows: &mut Vec<VisibleRow>,
  widget_layouts: &mut Vec<VisibleWidgetLayout>,
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

  if !last_for_line {
    return;
  }

  while let Some(widget) = widgets.get(*next_widget) {
    let widget_line = widget.after_line.min(last_doc_line);
    if widget_line != visible_row.anchor_line {
      break;
    }

    let row_span = widget_row_span(widget.measured_height_px, measurements.row_height_px);
    widget_layouts.push(VisibleWidgetLayout {
      widget_id: widget.id,
      start_row: *current_surface_row_abs,
      row_span,
      width_px: widget.measured_width_px,
      height_px: widget.measured_height_px.max(1),
    });

    for fragment in 0..row_span {
      let abs_surface_row = *current_surface_row_abs + fragment;
      if abs_surface_row >= window_start_row && abs_surface_row < window_end_row {
        rows.push(VisibleRow {
          key: RowKey::Widget {
            widget_id: widget.id,
            fragment:  fragment as u32,
          },
          text: Tendril::new(),
          anchor_line: visible_row.anchor_line,
          doc_visual_row: None,
          abs_surface_row,
          row_end_char: visible_row.row_end_char,
          row_end_col_abs: 0,
          metadata: VisibleRowMetadata {
            kind:                 RowKind::Widget {
              widget_id: widget.id,
              fragment:  fragment as u32,
            },
            doc_line:             Some(visible_row.anchor_line),
            starts_logical_line:  fragment == 0,
            wrapped_continuation: false,
          },
        });
      }
    }

    *current_surface_row_abs += row_span;
    *next_widget += 1;
  }
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

fn widget_row_span(height_px: u32, row_height_px: u32) -> usize {
  let row_height_px = row_height_px.max(1);
  height_px.max(1).div_ceil(row_height_px) as usize
}

pub(super) const VISIBLE_LAYOUT_MARGIN_ROWS: usize = 8;
