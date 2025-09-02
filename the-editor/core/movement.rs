use ropey::RopeSlice;

use crate::core::{
  grapheme::{
    nth_next_grapheme_boundary,
    nth_prev_grapheme_boundary,
    prev_grapheme_boundary,
  },
  position::{
    char_idx_at_visual_block_offset,
    visual_offset_from_block,
  },
  selection::Range,
  text_annotations::TextAnnotations,
  text_format::TextFormat,
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

pub fn move_vertically(
  slice: RopeSlice,
  range: Range,
  dir: Direction,
  count: usize,
  behavior: Movement,
  text_fmt: &TextFormat,
  annotations: &mut TextAnnotations,
) -> Range {
  let pos = range.cursor(slice);
  let line_idx = slice.char_to_line(pos);
  let line_start = slice.line_to_char(line_idx);

  // Compute the current positions's 2d coordinates.
  let visual_pos = visual_offset_from_block(slice, line_start, pos, text_fmt, annotations).0;

  let (mut new_row, new_col) = range
    .old_visual_pos
    .map_or((visual_pos.row as u32, visual_pos.col as u32), |pos| pos);
  new_row = new_row.max(visual_pos.row as u32);
  let line_idx = slice.char_to_line(pos);

  // Compute the new position.
  let mut new_line_idx = match dir {
    Direction::Forward => line_idx.saturating_add(count),
    Direction::Backward => line_idx.saturating_sub(count),
  };

  let line = if new_line_idx >= slice.len_lines() - 1 {
    // There is no line terminator for the last line
    // so the logic below is not necessary here.
    new_line_idx = slice.len_lines() - 1;
    slice
  } else {
    // `char_idx_at_visual_block_offset` returns a one-past-the-end index
    // in case it reaches the end of the slice
    // to avoid moving to the nextline in that case the line terminator is removed
    // from the line
    let new_line_end = prev_grapheme_boundary(slice, slice.line_to_char(new_line_idx + 1));
    slice.slice(..new_line_end)
  };

  let new_line_start = line.line_to_char(new_line_idx);

  let (new_pos, _) = char_idx_at_visual_block_offset(
    line,
    new_line_start,
    new_row as usize,
    new_col as usize,
    text_fmt,
    annotations,
  );

  // Special-case to avoid moving to the end of the last non-empty line.
  if behavior == Movement::Extend && slice.line(new_line_idx).len_chars() == 0 {
    return range;
  }

  let mut new_range = range.put_cursor(slice, new_pos, behavior == Movement::Extend);
  new_range.old_visual_pos = Some((new_row, new_col));
  new_range
}
