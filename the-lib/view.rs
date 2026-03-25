//! View state owned by clients.
//!
//! This module models per-view UI state (scroll/viewport, an optional active
//! cursor, and per-cursor visual goal state) without baking any view behavior
//! into core selection logic.
//!
//! # Example
//!
//! ```no_run
//! use the_lib::{
//!   position::Position,
//!   render::graphics::Rect,
//!   view::ViewState,
//! };
//!
//! let viewport = Rect::new(0, 0, 80, 24);
//! let scroll = Position::new(0, 0);
//! let view = ViewState::new(viewport, scroll);
//! # let _ = view;
//! ```

use std::collections::BTreeMap;

use crate::{
  position::Position,
  render::graphics::Rect,
  selection::{
    CursorId,
    Range,
  },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorVisualGoal {
  pub anchor: usize,
  pub head:   usize,
  pub row:    u32,
  pub col:    u32,
}

impl CursorVisualGoal {
  pub fn for_range(range: Range, row: u32, col: u32) -> Self {
    Self {
      anchor: range.anchor,
      head: range.head,
      row,
      col,
    }
  }

  pub fn matches(self, range: Range) -> bool {
    self.anchor == range.anchor && self.head == range.head
  }
}

/// Per-view state owned by the client.
///
/// The core library stays cursor-agnostic; the client chooses which cursor
/// (if any) is "active" for viewport following or collapse actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewState {
  pub viewport:        Rect,
  /// Visual scroll offset (row/col) in rendered space.
  pub scroll:          Position,
  /// Optional active cursor selected by the client.
  pub active_cursor:   Option<CursorId>,
  cursor_visual_goals: BTreeMap<CursorId, CursorVisualGoal>,
}

impl ViewState {
  pub fn new(viewport: Rect, scroll: Position) -> Self {
    Self {
      viewport,
      scroll,
      active_cursor: None,
      cursor_visual_goals: BTreeMap::new(),
    }
  }

  pub fn with_active_cursor(mut self, cursor_id: CursorId) -> Self {
    self.active_cursor = Some(cursor_id);
    self
  }

  pub fn cursor_visual_goal(&self, cursor_id: CursorId, range: Range) -> Option<(u32, u32)> {
    self
      .cursor_visual_goals
      .get(&cursor_id)
      .copied()
      .filter(|goal| goal.matches(range))
      .map(|goal| (goal.row, goal.col))
  }

  pub fn set_cursor_visual_goal(&mut self, cursor_id: CursorId, range: Range, row: u32, col: u32) {
    self
      .cursor_visual_goals
      .insert(cursor_id, CursorVisualGoal::for_range(range, row, col));
  }

  pub fn clear_cursor_visual_goal(&mut self, cursor_id: CursorId) {
    self.cursor_visual_goals.remove(&cursor_id);
  }

  pub fn clear_cursor_visual_goals(&mut self) {
    self.cursor_visual_goals.clear();
  }

  pub fn retain_cursor_visual_goals(&mut self, cursor_ids: &[CursorId]) {
    self
      .cursor_visual_goals
      .retain(|cursor_id, _| cursor_ids.contains(cursor_id));
  }
}

/// Compute adjusted scroll to keep cursor visible with scrolloff padding.
///
/// Returns `Some(new_scroll)` when scroll must change, `None` when cursor
/// is already within the padded viewport.
pub fn scroll_to_keep_visible(
  cursor_line: usize,
  cursor_col: usize,
  scroll: Position,
  viewport_height: usize,
  viewport_width: usize,
  scrolloff: usize,
) -> Option<Position> {
  let mut new_scroll = scroll;

  if let Some(new_row) =
    scroll_row_to_keep_visible(cursor_line, scroll.row, viewport_height, scrolloff)
  {
    new_scroll.row = new_row;
  }

  let h_off = scrolloff.min(viewport_width / 2);

  // Horizontal
  if cursor_col < scroll.col + h_off {
    new_scroll.col = cursor_col.saturating_sub(h_off);
  } else if cursor_col + h_off >= scroll.col + viewport_width {
    new_scroll.col = cursor_col + h_off + 1 - viewport_width;
  }

  if new_scroll != scroll {
    Some(new_scroll)
  } else {
    None
  }
}

/// Compute adjusted vertical scroll row to keep cursor visible with scrolloff
/// padding.
///
/// Returns `Some(new_row)` when the row must change, `None` when cursor
/// is already within the padded viewport.
pub fn scroll_row_to_keep_visible(
  cursor_line: usize,
  scroll_row: usize,
  viewport_height: usize,
  scrolloff: usize,
) -> Option<usize> {
  let v_off = scrolloff.min(viewport_height / 2);
  let mut new_row = scroll_row;

  if cursor_line < scroll_row + v_off {
    new_row = cursor_line.saturating_sub(v_off);
  } else if cursor_line + v_off >= scroll_row + viewport_height {
    new_row = cursor_line + v_off + 1 - viewport_height;
  }

  if new_row != scroll_row {
    Some(new_row)
  } else {
    None
  }
}

/// Compute the greatest vertical scroll origin that still keeps content in
/// view.
pub fn max_scroll_row_for_content(last_visual_row: usize, viewport_height: usize) -> usize {
  last_visual_row.saturating_sub(viewport_height.saturating_sub(1))
}
