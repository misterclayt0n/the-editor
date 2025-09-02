use std::{
  cmp::Ordering,
  ops::{
    Add,
    AddAssign,
    Sub,
    SubAssign,
  },
};

use ropey::RopeSlice;

use crate::core::{
  doc_formatter::DocumentFormatter,
  text_annotations::TextAnnotations,
  text_format::TextFormat,
};

/// This is a single point in a text buffer.
/// 0-indexed as all things should be.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
  pub row: usize,
  pub col: usize,
}

impl AddAssign for Position {
  fn add_assign(&mut self, rhs: Self) {
    self.row += rhs.row;
    self.col += rhs.col;
  }
}

impl SubAssign for Position {
  fn sub_assign(&mut self, rhs: Self) {
    self.row -= rhs.row;
    self.col -= rhs.col;
  }
}

impl Sub for Position {
  type Output = Position;

  fn sub(mut self, rhs: Self) -> Self::Output {
    self -= rhs;
    self
  }
}

impl Add for Position {
  type Output = Position;

  fn add(mut self, rhs: Self) -> Self::Output {
    self += rhs;
    self
  }
}

impl Position {
  pub fn new(row: usize, col: usize) -> Self {
    Self { row, col }
  }

  pub const fn is_zero(&self) -> bool {
    self.row == 0 && self.col == 0
  }

  pub fn from_char_pos(text: RopeSlice, pos: usize) -> Self {
    let row = text.char_to_line(pos);
    let line_start = text.line_to_char(row);
    let col = pos - line_start;
    Self { row, col }
  }

  pub fn to_char_pos(&self, text: RopeSlice) -> usize {
    if self.row >= text.len_lines() {
      return text.len_chars();
    }

    let line_start = text.line_to_char(self.row);
    let line_end = if self.row + 1 < text.len_lines() {
      text.line_to_char(self.row + 1).saturating_sub(1)
    } else {
      text.len_chars()
    };

    (line_start + self.col).min(line_end)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualOffsets {
  pub first_col: usize,
  pub last_col:  usize,
}

impl VisualOffsets {
  pub fn new(first_col: usize, last_col: usize) -> Self {
    Self {
      first_col,
      last_col,
    }
  }
}

pub fn visual_offset_from_block(
  text: RopeSlice,
  anchor: usize,
  pos: usize,
  text_fmt: &TextFormat,
  annotations: &TextAnnotations,
) -> (Position, usize) {
  let mut last_pos = Position::default();
  let mut formatter =
    DocumentFormatter::new_at_prev_checkpoint(text, text_fmt, annotations, anchor);
  let block_start = formatter.next_char_pos();

  while let Some(grapheme) = formatter.next() {
    last_pos = grapheme.visual_pos;
    if formatter.next_char_pos() > pos {
      return (grapheme.visual_pos, block_start);
    }
  }

  (last_pos, block_start)
}

/// This function behaves the same as `char_idx_at_visual_offset`, except that
/// the vertical offset `row` is always computed relative to the block that
/// contains `anchor` instead of the visual line that contains `anchor`.
/// Usually `char_idx_at_visual_offset` is more useful but this function can be
/// used in some situations as an optimization when `visual_offset_from_block`
/// was used
///
/// # Returns
///
/// `(real_char_idx, virtual_lines)`
///
/// See `char_idx_at_visual_offset` for details
pub fn char_idx_at_visual_block_offset(
  text: RopeSlice,
  anchor: usize,
  row: usize,
  column: usize,
  text_fmt: &TextFormat,
  annotations: &TextAnnotations,
) -> (usize, usize) {
  let mut formatter =
    DocumentFormatter::new_at_prev_checkpoint(text, text_fmt, annotations, anchor);
  let mut last_char_idx = formatter.next_char_pos();
  let mut found_non_virtual_on_row = false;
  let mut last_row = 0;
  for grapheme in &mut formatter {
    match grapheme.visual_pos.row.cmp(&row) {
      Ordering::Equal => {
        if grapheme.visual_pos.col + grapheme.width() > column {
          if !grapheme.is_virtual() {
            return (grapheme.char_idx, 0);
          } else if found_non_virtual_on_row {
            return (last_char_idx, 0);
          }
        } else if !grapheme.is_virtual() {
          found_non_virtual_on_row = true;
          last_char_idx = grapheme.char_idx;
        }
      },
      Ordering::Greater if found_non_virtual_on_row => return (last_char_idx, 0),
      Ordering::Greater => return (last_char_idx, row - last_row),
      Ordering::Less => {
        if !grapheme.is_virtual() {
          last_row = grapheme.visual_pos.row;
          last_char_idx = grapheme.char_idx;
        }
      },
    }
  }

  (formatter.next_char_pos(), 0)
}
