use std::ops::{
  Add,
  AddAssign,
  Sub,
  SubAssign,
};

use ropey::RopeSlice;
use the_core::{
  chars::char_is_line_ending,
  grapheme::ensure_grapheme_boundary_prev,
};
use the_stdx::rope::RopeSliceExt;

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

  pub const fn zero() -> Self {
    Self { row: 0, col: 0 }
  }

  pub const fn is_zero(&self) -> bool {
    self.row == 0 && self.col == 0
  }

  pub fn traverse(self, text: impl AsRef<str>) -> Self {
    let Self { mut row, mut col } = self;
    let mut chars = text.as_ref().chars().peekable();

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
/// status line.
pub fn coords_at_pos(text: RopeSlice, pos: usize) -> Position {
  let len = text.len_chars();
  let pos = pos.min(len);
  let line = text.char_to_line(pos);

  let line_start = text.line_to_char(line);
  let pos = ensure_grapheme_boundary_prev(text, pos);
  let col = text.slice(line_start..pos).graphemes().count();

  Position::new(line, col)
}

/// Convert a `(row, column)` (grapheme counts) to a character index.
///
/// If `row` exceeds the number of lines, the last line is used.
/// If `col` exceeds the number of graphemes on the line, the line end is used.
pub fn char_idx_at_coords(text: RopeSlice, coords: Position) -> usize {
  let line = coords.row.min(text.len_lines().saturating_sub(1));
  let line_start = text.line_to_char(line);
  let line_end = if line + 1 < text.len_lines() {
    text.line_to_char(line + 1)
  } else {
    text.len_chars()
  };

  let mut remaining = coords.col;
  let mut char_idx = line_start;
  for grapheme in text.slice(line_start..line_end).graphemes() {
    if remaining == 0 {
      return char_idx;
    }
    remaining -= 1;
    char_idx += grapheme.chars().count();
  }

  line_end
}
