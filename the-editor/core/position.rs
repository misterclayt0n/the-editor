use std::{
  borrow::Cow,
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
  grapheme::{
    ensure_grapheme_boundary_prev,
    grapheme_width,
  },
  line_ending::line_end_char_index,
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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum VisualOffsetError {
  PosBeforeAnchorRow,
  PosAfterMaxRow,
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
  let line = if pos < text.len_chars() {
    text.char_to_line(pos)
  } else {
    text.len_lines().saturating_sub(1)
  };

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

/// Convert a character index to (line, column) coordinates visually.
///
/// Takes \t, double-width characters (CJK) into account as well as text
/// not in the document in the future.
/// See [`coords_at_pos`] for an "objective" one.
///
/// This function should be used very rarely. Usually
/// `visual_offset_from_anchor` or `visual_offset_from_block` is preferable.
/// However when you want to compute the actual visual row/column in the text
/// (not what is actually shown on screen) then you should use this function.
/// For example aligning text should ignore virtual text and softwrap.
#[deprecated = "Doesn't account for softwrap or decorations, use visual_offset_from_anchor instead"]
pub fn visual_coords_at_pos(text: RopeSlice, pos: usize, tab_width: usize) -> Position {
  let line = if pos < text.len_chars() {
    text.char_to_line(pos)
  } else {
    text.len_lines().saturating_sub(1)
  };

  let line_start = text.line_to_char(line);
  let pos = ensure_grapheme_boundary_prev(text, pos);

  let mut col = 0;

  for grapheme in text.slice(line_start..pos).graphemes() {
    if grapheme == "\t" {
      col += tab_width - (col % tab_width);
    } else {
      let grapheme = Cow::from(grapheme);
      col += grapheme_width(&grapheme);
    }
  }

  Position::new(line, col)
}

/// Convert visual (line, column) coordinates to a character index.
///
/// If the `line` coordinate is beyond the end of the file, the EOF
/// position will be returned.
///
/// If the `column` coordinate is past the end of the given line, the
/// line-end position (in this case, just before the line ending
/// character) will be returned.
/// This function should be used very rarely. Usually
/// `char_idx_at_visual_offset` is preferable. However when you want to compute
/// a char position from the visual row/column in the text (not what is actually
/// shown on screen) then you should use this function. For example aligning
/// text should ignore virtual text and softwrap.
#[deprecated = "Doesn't account for softwrap or decorations, use char_idx_at_visual_offset instead"]
pub fn pos_at_visual_coords(text: RopeSlice, coords: Position, tab_width: usize) -> usize {
  let Position { mut row, col } = coords;
  row = row.min(text.len_lines() - 1);
  let line_start = text.line_to_char(row);
  let line_end = line_end_char_index(&text, row);

  let mut col_char_offset = 0;
  let mut cols_remaining = col;
  for grapheme in text.slice(line_start..line_end).graphemes() {
    let grapheme_width = if grapheme == "\t" {
      tab_width - ((col - cols_remaining) % tab_width)
    } else {
      let grapheme = Cow::from(grapheme);
      grapheme_width(&grapheme)
    };

    // If pos is in the middle of a wider grapheme (tab for example)
    // return the starting offset.
    if grapheme_width > cols_remaining {
      break;
    }

    cols_remaining -= grapheme_width;
    col_char_offset += grapheme.chars().count();
  }

  line_start + col_char_offset
}

/// Returns the height of the given text when softwrapping
pub fn softwrapped_dimensions(text: RopeSlice, text_fmt: &TextFormat) -> (usize, u16) {
  let last_pos =
    visual_offset_from_block(text, 0, usize::MAX, text_fmt, &TextAnnotations::default()).0;
  if last_pos.row == 0 {
    (1, last_pos.col as u16)
  } else {
    (last_pos.row + 1, text_fmt.viewport_width)
  }
}

pub fn visual_offset_from_anchor(
  text: RopeSlice,
  anchor: usize,
  pos: usize,
  text_fmt: &TextFormat,
  annotations: &TextAnnotations,
  max_rows: usize,
) -> Result<(Position, usize), VisualOffsetError> {
  let mut formatter =
    DocumentFormatter::new_at_prev_checkpoint(text, text_fmt, annotations, anchor);
  let mut anchor_line = None;
  let mut found_pos = None;
  let mut last_pos = Position::default();

  let block_start = formatter.next_char_pos();
  if pos < block_start {
    return Err(VisualOffsetError::PosBeforeAnchorRow);
  }

  while let Some(grapheme) = formatter.next() {
    last_pos = grapheme.visual_pos;

    if formatter.next_char_pos() > pos {
      if let Some(anchor_line) = anchor_line {
        last_pos.row -= anchor_line;
        return Ok((last_pos, block_start));
      } else {
        found_pos = Some(last_pos);
      }
    }
    if formatter.next_char_pos() > anchor && anchor_line.is_none() {
      if let Some(mut found_pos) = found_pos {
        return if found_pos.row == last_pos.row {
          found_pos.row = 0;
          Ok((found_pos, block_start))
        } else {
          Err(VisualOffsetError::PosBeforeAnchorRow)
        };
      } else {
        anchor_line = Some(last_pos.row);
      }
    }

    if let Some(anchor_line) = anchor_line
      && grapheme.visual_pos.row >= anchor_line + max_rows
    {
      return Err(VisualOffsetError::PosAfterMaxRow);
    }
  }

  let anchor_line = anchor_line.unwrap_or(last_pos.row);
  last_pos.row -= anchor_line;

  Ok((last_pos, block_start))
}

pub fn char_idx_at_visual_offset(
  text: RopeSlice,
  mut anchor: usize,
  mut row_offset: isize,
  column: usize,
  text_fmt: &TextFormat,
  annotations: &TextAnnotations,
) -> (usize, usize) {
  let mut pos = anchor;
  // convert row relative to visual line containing anchor to row relative to a
  // block containing anchor (anchor may change)
  loop {
    let (visual_pos_in_block, block_char_offset) =
      visual_offset_from_block(text, anchor, pos, text_fmt, annotations);
    row_offset += visual_pos_in_block.row as isize;
    anchor = block_char_offset;
    if row_offset >= 0 {
      break;
    }

    if block_char_offset == 0 {
      row_offset = 0;
      break;
    }
    // the row_offset is negative so we need to look at the previous block
    // set the anchor to the last char before the current block so that we can
    // compute the distance of this block from the start of the previous block
    pos = anchor;
    anchor -= 1;
  }

  char_idx_at_visual_block_offset(
    text,
    anchor,
    row_offset as usize,
    column,
    text_fmt,
    annotations,
  )
}
