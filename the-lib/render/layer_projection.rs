use ropey::RopeSlice;

use super::{
  display_list::Viewport,
  layers::{
    CursorPlacement,
    CursorSemantics,
    SelectionSpan,
    SyntaxSpan,
  },
  visible_layout::VisibleLayout,
};
use crate::{
  selection::{
    CursorId,
    Selection,
  },
  syntax::SyntaxHighlightRun,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct LayerProjector;

impl LayerProjector {
  pub(super) fn project_syntax_runs(
    visible_layout: &VisibleLayout,
    viewport: Viewport,
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
            kind: runs[probe].kind,
          };

          if let Some(last) = spans.last_mut()
            && last.surface_row == span.surface_row
            && last.kind == span.kind
            && last.end_col == span.start_col
          {
            last.end_col = span.end_col;
          } else {
            spans.push(span);
          }
        }
        probe += 1;
      }
    }

    spans
  }

  pub(super) fn project_selections(
    visible_layout: &VisibleLayout,
    selection: Option<&Selection>,
    active_cursor: Option<CursorId>,
    viewport: Viewport,
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
        active: active_cursor == Some(cursor.id),
      };

      if let Some(last) = spans.last_mut()
        && last.cursor_id == span.cursor_id
        && last.surface_row == span.surface_row
        && last.end_col == span.start_col
      {
        last.end_col = span.end_col;
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
    active_cursor: Option<CursorId>,
    viewport: Viewport,
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
        .filter(|position| position.char_idx == char_idx)
        .find(|position| cursor_position_visible(**position, viewport))
        .or_else(|| {
          visible_layout
            .cursor_positions
            .iter()
            .find(|position| position.char_idx == char_idx)
        })
        .copied()
      else {
        continue;
      };

      if !cursor_position_visible(position, viewport) {
        continue;
      }
      let col = position.col_abs - viewport.first_col;
      let surface_row = position.surface_row_abs - viewport.first_row;
      cursors.push(CursorPlacement {
        cursor_id: cursor.id,
        surface_row,
        col,
        semantics,
        active: active_cursor == Some(cursor.id),
      });
    }

    cursors
  }
}

fn project_visible_cols(
  start_col_abs: usize,
  end_col_abs: usize,
  viewport: Viewport,
) -> Option<(usize, usize)> {
  let visible_start = start_col_abs.max(viewport.first_col);
  let visible_end = end_col_abs.min(viewport.first_col.saturating_add(viewport.wrap_width));
  (visible_start < visible_end).then_some((
    visible_start - viewport.first_col,
    visible_end - viewport.first_col,
  ))
}

fn cursor_position_visible(
  position: super::visible_layout::CursorPosition,
  viewport: Viewport,
) -> bool {
  position.surface_row_abs >= viewport.first_row
    && position.surface_row_abs < viewport.first_row.saturating_add(viewport.row_count)
    && position.col_abs >= viewport.first_col
    && position.col_abs <= viewport.first_col.saturating_add(viewport.wrap_width)
}
