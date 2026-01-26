//! Visual position helpers.
//!
//! These helpers map between document character indices and visual positions
//! (row/column) under the current [`TextFormat`] and [`TextAnnotations`].

use std::cmp::Ordering;

use ropey::RopeSlice;
use the_core::grapheme::{
  Grapheme,
  GraphemeStr,
};
use the_stdx::rope::RopeSliceExt;

use crate::{
  position::Position,
  render::{
    doc_formatter::DocumentFormatter,
    text_annotations::TextAnnotations,
    text_format::TextFormat,
  },
};

/// Map a character index to a visual position.
pub fn visual_pos_at_char<'a>(
  text: RopeSlice<'a>,
  text_fmt: &'a TextFormat,
  annotations: &mut TextAnnotations<'a>,
  char_idx: usize,
) -> Option<Position> {
  let char_idx = char_idx.min(text.len_chars());

  if !text_fmt.soft_wrap && !annotations.has_line_annotations() {
    let line = text.char_to_line(char_idx);
    let line_start = text.line_to_char(line);
    let slice = text.slice(line_start..char_idx);
    let mut col = 0;
    for grapheme in slice.graphemes() {
      let g = Grapheme::new(grapheme_str(grapheme), col, text_fmt.tab_width);
      col += g.width();
    }
    return Some(Position::new(line, col));
  }

  let mut formatter = DocumentFormatter::new_at_prev_checkpoint(text, text_fmt, annotations, 0);
  for g in &mut formatter {
    if g.char_idx == char_idx {
      return Some(g.visual_pos);
    }
    if g.source.is_eof() {
      break;
    }
  }
  None
}

/// Map a visual position to a character index.
///
/// For non-soft-wrapped text this is fast and uses rope line slices.
/// For soft wrapping this performs a linear scan using [`DocumentFormatter`].
pub fn char_at_visual_pos<'a>(
  text: RopeSlice<'a>,
  text_fmt: &'a TextFormat,
  annotations: &mut TextAnnotations<'a>,
  target: Position,
) -> Option<usize> {
  if !text_fmt.soft_wrap && !annotations.has_line_annotations() {
    if text.len_lines() == 0 {
      return Some(0);
    }

    let line = target.row.min(text.len_lines().saturating_sub(1));
    let line_start = text.line_to_char(line);
    let line_slice = text.line(line);

    let mut col = 0;
    let mut char_pos = line_start;
    for grapheme in line_slice.graphemes() {
      let g = Grapheme::new(grapheme_str(grapheme), col, text_fmt.tab_width);
      let width = g.width();
      if col + width > target.col {
        return Some(char_pos);
      }
      col += width;
      char_pos += grapheme.len_chars();
    }

    return Some(line_start + line_slice.len_chars());
  }

  let mut formatter = DocumentFormatter::new_at_prev_checkpoint(text, text_fmt, annotations, 0);
  for g in &mut formatter {
    if g.source.is_eof() {
      return Some(text.len_chars());
    }
    if g.visual_pos.row > target.row
      || (g.visual_pos.row == target.row && g.visual_pos.col >= target.col)
    {
      return Some(g.char_idx);
    }
  }
  None
}

fn grapheme_str<'a>(grapheme: RopeSlice<'a>) -> GraphemeStr<'a> {
  match grapheme.as_str() {
    Some(slice) => GraphemeStr::from(slice),
    None => GraphemeStr::from(grapheme.to_string()),
  }
}

/// Compute the visual position of `pos` relative to the block containing
/// `anchor`.
///
/// Returns `(visual_position, block_start_char_idx)`.
///
/// This is essential for soft-wrap aware vertical movement since a single
/// logical line can span multiple visual rows.
pub fn visual_offset_from_block<'a>(
  text: RopeSlice<'a>,
  anchor: usize,
  pos: usize,
  text_fmt: &'a TextFormat,
  annotations: &mut TextAnnotations<'a>,
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

/// Convert a visual block offset to a character index.
///
/// This function computes the character index at the given `row` and `column`
/// relative to the block containing `anchor`. Unlike
/// `char_idx_at_visual_offset`, the row is always relative to the block start,
/// not the visual line containing the anchor.
///
/// # Returns
///
/// `(char_idx, virtual_rows)` where `virtual_rows` is non-zero if the target
/// position is beyond the end of the document.
pub fn char_idx_at_visual_block_offset<'a>(
  text: RopeSlice<'a>,
  anchor: usize,
  row: usize,
  column: usize,
  text_fmt: &'a TextFormat,
  annotations: &mut TextAnnotations<'a>,
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

/// Convert a visual offset (potentially negative) to a character index.
///
/// This is the main entry point for visual vertical movement. It handles:
/// - Positive row offsets (moving down)
/// - Negative row offsets (moving up, crossing block boundaries)
/// - Virtual rows beyond EOF
///
/// # Returns
///
/// `(char_idx, virtual_rows)` where `virtual_rows` is non-zero if the target
/// position is beyond the end of the document.
pub fn char_idx_at_visual_offset<'a>(
  text: RopeSlice<'a>,
  mut anchor: usize,
  mut row_offset: isize,
  column: usize,
  text_fmt: &'a TextFormat,
  annotations: &mut TextAnnotations<'a>,
) -> (usize, usize) {
  let mut pos = anchor;

  // Convert row relative to visual line containing anchor to row relative to a
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

    // The row_offset is negative so we need to look at the previous block.
    // Set the anchor to the last char before the current block so that we can
    // compute the distance of this block from the start of the previous block.
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

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::*;

  #[test]
  fn visual_pos_at_char_no_soft_wrap() {
    let text = Rope::from("a\tb");
    let mut fmt = TextFormat::default();
    fmt.soft_wrap = false;
    fmt.tab_width = 4;
    fmt.rebuild_wrap_indicator();
    let mut annotations = TextAnnotations::default();

    let pos = visual_pos_at_char(text.slice(..), &fmt, &mut annotations, 2).unwrap();
    assert_eq!(pos.row, 0);
    assert_eq!(pos.col, 4);
  }

  #[test]
  fn char_at_visual_pos_no_soft_wrap() {
    let text = Rope::from("a\tb");
    let mut fmt = TextFormat::default();
    fmt.soft_wrap = false;
    fmt.tab_width = 4;
    fmt.rebuild_wrap_indicator();
    let mut annotations = TextAnnotations::default();

    let pos =
      char_at_visual_pos(text.slice(..), &fmt, &mut annotations, Position::new(0, 3)).unwrap();
    assert_eq!(pos, 1);
  }
}
