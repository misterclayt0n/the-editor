use ropey::RopeSlice;

use super::{
  damage::Damage,
  display_list::Viewport,
  layers::{
    CursorPlacement,
    CursorSemantics,
    SelectionSpan,
    SyntaxSpan,
    WidgetPlacement,
  },
  measurement::CellGridMetrics,
  visible_layout::VisibleLayout,
};
use crate::{
  selection::Selection,
  syntax::SyntaxHighlightRun,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct LayerProjector;

impl LayerProjector {
  pub(super) fn project_syntax_runs(
    visible_layout: &VisibleLayout,
    viewport: Viewport,
    measurements: CellGridMetrics,
    runs: &[SyntaxHighlightRun],
  ) -> Vec<SyntaxSpan> {
    if runs.is_empty() {
      return Vec::new();
    }

    let mut spans: Vec<SyntaxSpan> = Vec::new();
    let mut run_idx = 0;

    for unit in &visible_layout.units {
      if unit.surface_row_abs < viewport.first_row
        || unit.surface_row_abs >= viewport.first_row + viewport.row_count
      {
        continue;
      }

      while run_idx < runs.len() && runs[run_idx].byte_range.end <= unit.byte_start {
        run_idx += 1;
      }

      let mut probe = run_idx;
      while probe < runs.len() && runs[probe].byte_range.start < unit.byte_end {
        if unit.byte_start < runs[probe].byte_range.end {
          let Some((start_col, end_col)) =
            project_visible_cols(unit.start_col_abs, unit.end_col_abs, viewport)
          else {
            probe += 1;
            continue;
          };
          let span = SyntaxSpan {
            surface_row: unit.surface_row_abs - viewport.first_row,
            start_col,
            end_col,
            x_px: measurements.col_to_x_px(start_col),
            width_px: measurements.col_span_width_px(start_col, end_col),
            kind: runs[probe].kind,
          };

          if let Some(last) = spans.last_mut()
            && last.surface_row == span.surface_row
            && last.kind == span.kind
            && last.end_col == span.start_col
          {
            last.end_col = span.end_col;
            last.width_px = measurements.col_span_width_px(last.start_col, last.end_col);
          } else {
            spans.push(span);
          }
        }
        probe += 1;
      }
    }

    spans
  }

  pub(super) fn project_widgets(
    visible_layout: &VisibleLayout,
    viewport: Viewport,
    measurements: CellGridMetrics,
  ) -> Vec<WidgetPlacement> {
    let viewport_end = viewport.first_row.saturating_add(viewport.row_count);
    let viewport_width_px = measurements.col_span_width_px(0, viewport.wrap_width) as i64;
    let x_px = -(measurements.col_to_x_px(viewport.first_col) as i64);

    visible_layout
      .widget_layouts
      .iter()
      .filter_map(|widget| {
        let widget_end = widget.start_row.saturating_add(widget.row_span);
        let visible_start = widget.start_row.max(viewport.first_row);
        let visible_end = widget_end.min(viewport_end);
        if visible_start >= visible_end {
          return None;
        }
        if x_px >= viewport_width_px || x_px + widget.width_px as i64 <= 0 {
          return None;
        }

        Some(WidgetPlacement {
          widget_id: widget.widget_id,
          surface_row: visible_start - viewport.first_row,
          row_span: visible_end - visible_start,
          x_px,
          y_px: (measurements.row_to_y_px(widget.start_row) as i64)
            - (measurements.row_to_y_px(viewport.first_row) as i64),
          width_px: widget.width_px,
          height_px: widget.height_px,
        })
      })
      .collect()
  }

  pub(super) fn project_selections(
    visible_layout: &VisibleLayout,
    selection: Option<&Selection>,
    viewport: Viewport,
    measurements: CellGridMetrics,
  ) -> Vec<SelectionSpan> {
    let Some(selection) = selection else {
      return Vec::new();
    };

    let mut spans: Vec<SelectionSpan> = Vec::new();

    for unit in &visible_layout.units {
      if unit.surface_row_abs < viewport.first_row
        || unit.surface_row_abs >= viewport.first_row + viewport.row_count
      {
        continue;
      }

      let Some(cursor) = selection
        .iter()
        .find(|cursor| !cursor.range.is_empty() && cursor.range.contains(unit.char_start))
      else {
        continue;
      };

      let Some((start_col, end_col)) =
        project_visible_cols(unit.start_col_abs, unit.end_col_abs, viewport)
      else {
        continue;
      };
      let span = SelectionSpan {
        cursor_id: cursor.id,
        surface_row: unit.surface_row_abs - viewport.first_row,
        start_col,
        end_col,
        x_px: measurements.col_to_x_px(start_col),
        width_px: measurements.col_span_width_px(start_col, end_col),
      };

      if let Some(last) = spans.last_mut()
        && last.cursor_id == span.cursor_id
        && last.surface_row == span.surface_row
        && last.end_col == span.start_col
      {
        last.end_col = span.end_col;
        last.width_px = measurements.col_span_width_px(last.start_col, last.end_col);
      } else {
        spans.push(span);
      }
    }

    spans
  }

  pub(super) fn project_cursors(
    text: RopeSlice<'_>,
    visible_layout: &VisibleLayout,
    selection: Option<&Selection>,
    viewport: Viewport,
    measurements: CellGridMetrics,
    semantics: CursorSemantics,
  ) -> Vec<CursorPlacement> {
    let Some(selection) = selection else {
      return Vec::new();
    };

    let mut cursors = Vec::new();
    for cursor in selection.iter() {
      let char_idx = match semantics {
        CursorSemantics::InsertionPoint => cursor.range.head,
        CursorSemantics::Cell => cursor.range.cursor(text),
      };
      let Some(position) = visible_layout
        .cursor_positions
        .iter()
        .find(|position| position.char_idx == char_idx)
        .copied()
      else {
        continue;
      };

      if position.surface_row_abs < viewport.first_row
        || position.surface_row_abs >= viewport.first_row + viewport.row_count
      {
        continue;
      }

      if position.col_abs < viewport.first_col
        || position.col_abs >= viewport.first_col.saturating_add(viewport.wrap_width)
      {
        continue;
      }
      let col = position.col_abs - viewport.first_col;
      let surface_row = position.surface_row_abs - viewport.first_row;
      let width_cols = match semantics {
        CursorSemantics::InsertionPoint => 0,
        CursorSemantics::Cell => {
          visible_layout
            .units
            .iter()
            .find(|unit| {
              unit.surface_row_abs == position.surface_row_abs && unit.char_start == char_idx
            })
            .map_or(1, |unit| {
              unit.end_col_abs.saturating_sub(unit.start_col_abs).max(1)
            })
        },
      };
      cursors.push(CursorPlacement {
        cursor_id: cursor.id,
        surface_row,
        col,
        x_px: measurements.col_to_x_px(col),
        y_px: measurements.row_to_y_px(surface_row),
        width_px: measurements.col_span_width_px(0, width_cols),
        height_px: measurements.row_height_px,
        semantics,
      });
    }

    cursors
  }

  pub(super) fn damage_for_selection_change(
    old: &[SelectionSpan],
    new: &[SelectionSpan],
  ) -> Damage {
    Damage::from_rows(
      old
        .iter()
        .map(|span| span.surface_row)
        .chain(new.iter().map(|span| span.surface_row)),
    )
  }

  pub(super) fn damage_for_cursor_change(
    old: &[CursorPlacement],
    new: &[CursorPlacement],
  ) -> Damage {
    Damage::from_rows(
      old
        .iter()
        .map(|cursor| cursor.surface_row)
        .chain(new.iter().map(|cursor| cursor.surface_row)),
    )
  }

  pub(super) fn damage_for_syntax_change(old: &[SyntaxSpan], new: &[SyntaxSpan]) -> Damage {
    Damage::from_rows(
      old
        .iter()
        .map(|span| span.surface_row)
        .chain(new.iter().map(|span| span.surface_row)),
    )
  }

  pub(super) fn damage_for_widget_change(
    old: &[WidgetPlacement],
    new: &[WidgetPlacement],
  ) -> Damage {
    Damage::from_rows(
      old
        .iter()
        .flat_map(widget_visible_rows)
        .chain(new.iter().flat_map(widget_visible_rows)),
    )
  }
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

fn widget_visible_rows(widget: &WidgetPlacement) -> impl Iterator<Item = usize> {
  widget.surface_row..widget.surface_row + widget.row_span
}
