//! Visual row/column mapping helpers built on top of [`DocumentFormatter`].

use std::cmp::Ordering;

use ropey::RopeSlice;
use the_stdx::rope::RopeSliceExt;

use crate::{
  grapheme::Grapheme,
  line_ending::get_line_ending,
  render::{
    doc_formatter::{
      DocumentFormatter,
      Position,
    },
    text_annotations::TextAnnotations,
    text_format::TextFormat,
  },
};

pub fn visual_pos_at_char(
  text: RopeSlice<'_>,
  text_format: &TextFormat,
  wrap_width: usize,
  annotations: &TextAnnotations,
  char_idx: usize,
) -> Option<Position> {
  let char_idx = char_idx.min(text.len_chars());

  if !text_format.soft_wrap && annotations.is_empty() {
    let line = text.char_to_line(char_idx);
    let line_start = text.line_to_char(line);
    let slice = text.slice(line_start..char_idx);
    let mut col = 0;
    for grapheme in slice.graphemes() {
      col += Grapheme::new(grapheme_str(grapheme), col, text_format.tab_width).width();
    }
    return Some(Position::new(line, col));
  }

  for grapheme in DocumentFormatter::new(text, text_format, wrap_width, annotations) {
    if grapheme.char_idx == char_idx && !grapheme.is_virtual() {
      return Some(grapheme.visual_pos);
    }
    if grapheme.source.is_eof() {
      return Some(grapheme.visual_pos);
    }
  }

  None
}

pub fn char_at_visual_pos(
  text: RopeSlice<'_>,
  text_format: &TextFormat,
  wrap_width: usize,
  annotations: &TextAnnotations,
  target: Position,
) -> Option<usize> {
  if !text_format.soft_wrap && annotations.is_empty() {
    if text.len_lines() == 0 {
      return Some(0);
    }

    let line = target.row.min(text.len_lines().saturating_sub(1));
    let line_start = text.line_to_char(line);
    let line_slice = text.line(line);
    let line_ending_chars = get_line_ending(&line_slice)
      .map(|line_ending| line_ending.len_chars())
      .unwrap_or(0);
    let line_content_end = line_start + line_slice.len_chars().saturating_sub(line_ending_chars);
    let line_content = text.slice(line_start..line_content_end);

    let mut col = 0;
    let mut char_pos = line_start;
    for grapheme_slice in line_content.graphemes() {
      let grapheme = Grapheme::new(grapheme_str(grapheme_slice), col, text_format.tab_width);
      let width = grapheme.width();
      if col + width > target.col {
        return Some(char_pos);
      }
      col += width;
      char_pos += grapheme_slice.len_chars();
    }

    return Some(line_content_end);
  }

  for grapheme in DocumentFormatter::new(text, text_format, wrap_width, annotations) {
    if grapheme.source.is_eof() {
      return Some(text.len_chars());
    }

    match grapheme.visual_pos.row.cmp(&target.row) {
      Ordering::Greater => return Some(grapheme.char_idx),
      Ordering::Equal if grapheme.visual_pos.col + grapheme.width() > target.col => {
        return Some(grapheme.char_idx);
      },
      _ => {},
    }
  }

  None
}

#[allow(dead_code)]
pub(crate) fn visual_offset_from_block(
  text: RopeSlice<'_>,
  anchor: usize,
  pos: usize,
  text_format: &TextFormat,
  wrap_width: usize,
  annotations: &TextAnnotations,
) -> (Position, usize) {
  let mut last_pos = Position::default();
  let mut formatter =
    DocumentFormatter::new_at_char(text, text_format, wrap_width, annotations, anchor);
  let block_start = formatter.next_char_pos();

  while let Some(grapheme) = formatter.next() {
    last_pos = grapheme.visual_pos;
    if formatter.next_char_pos() > pos {
      return (grapheme.visual_pos, block_start);
    }
  }

  (last_pos, block_start)
}

#[allow(dead_code)]
pub(crate) fn char_idx_at_visual_block_offset(
  text: RopeSlice<'_>,
  anchor: usize,
  row: usize,
  column: usize,
  text_format: &TextFormat,
  wrap_width: usize,
  annotations: &TextAnnotations,
) -> (usize, usize) {
  let mut formatter =
    DocumentFormatter::new_at_char(text, text_format, wrap_width, annotations, anchor);
  let mut last_char_idx = formatter.next_char_pos();
  let mut found_non_virtual_on_row = false;
  let mut last_row = 0;

  for grapheme in &mut formatter {
    match grapheme.visual_pos.row.cmp(&row) {
      Ordering::Equal => {
        if grapheme.visual_pos.col + grapheme.width() > column {
          if !grapheme.is_virtual() {
            return (grapheme.char_idx, 0);
          }
          if found_non_virtual_on_row {
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

#[allow(dead_code)]
pub(crate) fn char_idx_at_visual_offset(
  text: RopeSlice<'_>,
  mut anchor: usize,
  mut row_offset: isize,
  column: usize,
  text_format: &TextFormat,
  wrap_width: usize,
  annotations: &TextAnnotations,
) -> (usize, usize) {
  let mut pos = anchor;

  loop {
    let (visual_pos_in_block, block_char_offset) =
      visual_offset_from_block(text, anchor, pos, text_format, wrap_width, annotations);
    row_offset += visual_pos_in_block.row as isize;
    anchor = block_char_offset;

    if row_offset >= 0 {
      break;
    }
    if block_char_offset == 0 {
      row_offset = 0;
      break;
    }

    pos = anchor;
    anchor -= 1;
  }

  char_idx_at_visual_block_offset(
    text,
    anchor,
    row_offset as usize,
    column,
    text_format,
    wrap_width,
    annotations,
  )
}

fn grapheme_str<'a>(grapheme: RopeSlice<'a>) -> crate::grapheme::GraphemeStr<'a> {
  match grapheme.as_str() {
    Some(slice) => slice.into(),
    None => grapheme.to_string().into(),
  }
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::{
    Position,
    char_at_visual_pos,
    visual_pos_at_char,
  };
  use crate::render::{
    text_annotations::{
      TextAnnotations,
      VirtualLineSpec,
    },
    text_format::TextFormat,
  };

  #[test]
  fn visual_pos_at_char_without_soft_wrap() {
    let rope = Rope::from("hello\nworld");
    let format = TextFormat::default();
    let annotations = TextAnnotations::default();

    let pos = visual_pos_at_char(rope.slice(..), &format, 20, &annotations, 7).unwrap();

    assert_eq!(pos, Position::new(1, 1));
  }

  #[test]
  fn char_at_visual_pos_clips_past_line_end_before_newline() {
    let rope = Rope::from("hello\nworld");
    let format = TextFormat::default();
    let annotations = TextAnnotations::default();

    let char_idx = char_at_visual_pos(
      rope.slice(..),
      &format,
      20,
      &annotations,
      Position::new(0, 20),
    )
    .unwrap();

    assert_eq!(char_idx, 5);
  }

  #[test]
  fn visual_mapping_accounts_for_virtual_lines() {
    let rope = Rope::from("one\ntwo");
    let format = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    annotations.add_virtual_line(VirtualLineSpec::after(0).text("ghost").single_line());

    let pos = visual_pos_at_char(rope.slice(..), &format, 20, &annotations, 4).unwrap();
    let char_idx = char_at_visual_pos(
      rope.slice(..),
      &format,
      20,
      &annotations,
      Position::new(1, 2),
    )
    .unwrap();

    assert_eq!(pos, Position::new(2, 0));
    assert_eq!(char_idx, 4);
  }

  #[test]
  fn char_at_visual_pos_handles_tabs_in_formatter_path() {
    let rope = Rope::from("a\tb");
    let mut format = TextFormat::default();
    format.soft_wrap = true;
    let annotations = TextAnnotations::default();

    let char_idx = char_at_visual_pos(
      rope.slice(..),
      &format,
      20,
      &annotations,
      Position::new(0, 3),
    )
    .unwrap();

    assert_eq!(char_idx, 1);
  }
}
