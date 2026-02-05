//! View state owned by clients.
//!
//! This module models per-view UI state (scroll/viewport and an optional active
//! cursor) without baking any view behavior into core selection logic.
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

use crate::{
  position::Position,
  render::graphics::Rect,
  selection::CursorId,
};

/// Per-view state owned by the client.
///
/// The core library stays cursor-agnostic; the client chooses which cursor
/// (if any) is "active" for viewport following or collapse actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewState {
  pub viewport:      Rect,
  /// Visual scroll offset (row/col) in rendered space.
  pub scroll:        Position,
  /// Optional active cursor selected by the client.
  pub active_cursor: Option<CursorId>,
}

impl ViewState {
  pub fn new(viewport: Rect, scroll: Position) -> Self {
    Self {
      viewport,
      scroll,
      active_cursor: None,
    }
  }

  pub fn with_active_cursor(mut self, cursor_id: CursorId) -> Self {
    self.active_cursor = Some(cursor_id);
    self
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
  let v_off = scrolloff.min(viewport_height / 2);
  let h_off = scrolloff.min(viewport_width / 2);

  let mut new_scroll = scroll;

  // Vertical
  if cursor_line < scroll.row + v_off {
    new_scroll.row = cursor_line.saturating_sub(v_off);
  } else if cursor_line + v_off >= scroll.row + viewport_height {
    new_scroll.row = cursor_line + v_off + 1 - viewport_height;
  }

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
