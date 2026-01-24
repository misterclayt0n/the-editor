//! Visual position helpers.
//!
//! These helpers map between document character indices and visual positions
//! (row/column) under the current [`TextFormat`] and [`TextAnnotations`].

use ropey::RopeSlice;

use the_core::grapheme::{Grapheme, GraphemeStr};
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

#[cfg(test)]
mod tests {
  use super::*;
  use ropey::Rope;

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

    let pos = char_at_visual_pos(text.slice(..), &fmt, &mut annotations, Position::new(0, 3))
      .unwrap();
    assert_eq!(pos, 1);
  }
}
