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
  pub viewport: Rect,
  /// Visual scroll offset (row/col) in rendered space.
  pub scroll: Position,
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
