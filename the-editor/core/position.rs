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
use the_editor_stdx::rope::RopeSliceExt;

use crate::core::{
  Tendril,
  chars::char_is_line_ending,
  doc_formatter::DocumentFormatter,
  grapheme::ensure_grapheme_boundary_prev,
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

  pub fn traverse(self, text: Tendril) -> Self {
    let Self { mut row, mut col } = self;
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
      if char_is_line_ending(ch) && !(ch == '\r' && chars.peek() == Some(&'\n')) {
        row += 1;
        col = 0;
      } else {
        col += 1;
      }
    }

    Self { row, col }
  }
}

impl From<(usize, usize)> for Position {
  fn from(value: (usize, usize)) -> Self {
    Position::new(value.0, value.1)
  }
}

/// Converts a character index into a `Position`.
///
/// Column in `char` count which can be used for row:col display in
/// status line. See [`visual_coords_at_pos`] for a visual one.
pub fn coords_at_pos(text: RopeSlice, pos: usize) -> Position {
  let line = text.char_to_line(pos);

  let line_start = text.line_to_char(line);
  let pos = ensure_grapheme_boundary_prev(text, pos);
  let col = text.slice(line_start..pos).graphemes().count();

  Position::new(line, col)
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
