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
