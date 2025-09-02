use ropey::RopeSlice;

use crate::core::{
  grapheme::{
    nth_next_grapheme_boundary,
    nth_prev_grapheme_boundary,
  },
  selection::Range,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Direction {
  Forward,
  Backward,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Movement {
  /// Extend the selection.
  Extend,

  /// Move the selection (set anchor == head)
  Move,
}

pub fn move_horizontally(
  slice: RopeSlice,
  range: Range,
  dir: Direction,
  count: usize,
  behavior: Movement,
) -> Range {
  let pos = range.cursor(slice);

  let new_pos = match dir {
    Direction::Forward => nth_next_grapheme_boundary(slice, pos, count),
    Direction::Backward => nth_prev_grapheme_boundary(slice, pos, count),
  };

  range.put_cursor(slice, new_pos, behavior == Movement::Extend)
}
