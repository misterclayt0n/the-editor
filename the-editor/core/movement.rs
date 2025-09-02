use ropey::RopeSlice;

use crate::core::{
  grapheme::{
    nth_next_grapheme_boundary,
    nth_prev_grapheme_boundary,
  },
  line_ending::line_end_char_index,
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

  let mut new_range = range.put_cursor(slice, new_pos, behavior == Movement::Extend);
  new_range.old_visual_pos = None;
  new_range
}

// TODO: Add text format and text annotations for movement based on different
// rendering styles + better separation of concerns.
// THIS IS WRONG!!!
pub fn move_vertically(
  slice: RopeSlice,
  range: Range,
  dir: Direction,
  count: usize,
  behavior: Movement,
) -> Range {
  let pos = range.cursor(slice);
  let line_idx = slice.char_to_line(pos);
  let line_start = slice.line_to_char(line_idx);
  let col_in_line = pos - line_start;

  let target_col = range
    .old_visual_pos
    .map(|(_, col)| col)
    .unwrap_or(col_in_line);

  let new_line_idx = match dir {
    Direction::Forward => line_idx.saturating_add(count),
    Direction::Backward => line_idx.saturating_sub(count),
  }
  .min(slice.len_lines().saturating_sub(1));

  let new_line_start = slice.line_to_char(new_line_idx);
  let new_line_end = line_end_char_index(&slice, new_line_idx);
  let content_len = new_line_end.saturating_sub(new_line_start);

  let new_col = target_col.min(content_len);
  let new_pos = new_line_start + new_col;

  let mut new_range = range.put_cursor(slice, new_pos, behavior == Movement::Extend);
  
  new_range.old_visual_pos = Some((new_line_idx, target_col));
  new_range
}
