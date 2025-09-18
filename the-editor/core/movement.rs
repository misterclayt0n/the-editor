use ropey::{
  RopeSlice,
  iter::Chars,
};

use crate::core::{
  chars::{
    CharCategory,
    categorize_char,
    char_is_line_ending,
  },
  grapheme::{
    next_grapheme_boundary,
    nth_next_grapheme_boundary,
    nth_prev_grapheme_boundary,
    prev_grapheme_boundary,
  },
  position::{
    char_idx_at_visual_block_offset,
    char_idx_at_visual_offset,
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

/// Possible targets of a word motion
#[derive(Copy, Clone, Debug)]
pub enum WordMotionTarget {
  NextWordStart,
  NextWordEnd,
  PrevWordStart,
  PrevWordEnd,
  // A "Long word" (also known as a WORD in Vim/Kakoune) is strictly
  // delimited by whitespace, and can consist of punctuation as well
  // as alphanumerics.
  NextLongWordStart,
  NextLongWordEnd,
  PrevLongWordStart,
  PrevLongWordEnd,

  // A sub word is similar to a regular word, except it is also delimited by
  // underscores and transitions from lowercase to uppercase.
  NextSubWordStart,
  NextSubWordEnd,
  PrevSubWordStart,
  PrevSubWordEnd,
}

pub trait CharHelpers {
  fn range_to_target(&mut self, target: WordMotionTarget, origin: Range) -> Range;
}

impl CharHelpers for Chars<'_> {
  /// Note: this only changes the anchor of the range if the head is effectively
  /// starting on a boundary (either directly or after skipping newline
  /// characters). Any other changes to the anchor should be handled by the
  /// calling code.
  fn range_to_target(&mut self, target: WordMotionTarget, origin: Range) -> Range {
    let is_prev = matches!(
      target,
      WordMotionTarget::PrevWordStart
        | WordMotionTarget::PrevLongWordStart
        | WordMotionTarget::PrevSubWordStart
        | WordMotionTarget::PrevWordEnd
        | WordMotionTarget::PrevLongWordEnd
        | WordMotionTarget::PrevSubWordEnd
    );

    // Reverse the iterator if needed for the motion direction.
    if is_prev {
      self.reverse();
    }

    // Function to advance index in the appropriate motion direction.
    let advance: &dyn Fn(&mut usize) = if is_prev {
      &|idx| *idx = idx.saturating_sub(1)
    } else {
      &|idx| *idx += 1
    };

    // Initialize state variables.
    let mut anchor = origin.anchor;
    let mut head = origin.head;
    let mut prev_ch = {
      let ch = self.prev();
      if ch.is_some() {
        self.next();
      }
      ch
    };

    // Skip any initial newline characters.
    while let Some(ch) = self.next() {
      if char_is_line_ending(ch) {
        prev_ch = Some(ch);
        advance(&mut head);
      } else {
        self.prev();
        break;
      }
    }
    if prev_ch.map(char_is_line_ending).unwrap_or(false) {
      anchor = head;
    }

    // Find our target position(s).
    let head_start = head;
    #[allow(clippy::while_let_on_iterator)] // Clippy's suggestion to fix doesn't work here.
    while let Some(next_ch) = self.next() {
      if prev_ch.is_none() || reached_target(target, prev_ch.unwrap(), next_ch) {
        if head == head_start {
          anchor = head;
        } else {
          break;
        }
      }
      prev_ch = Some(next_ch);
      advance(&mut head);
    }

    // Un-reverse the iterator if needed.
    if is_prev {
      self.reverse();
    }

    Range::new(anchor, head)
  }
}

fn is_word_boundary(a: char, b: char) -> bool {
  categorize_char(a) != categorize_char(b)
}

fn is_long_word_boundary(a: char, b: char) -> bool {
  match (categorize_char(a), categorize_char(b)) {
    (CharCategory::Word, CharCategory::Punctuation)
    | (CharCategory::Punctuation, CharCategory::Word) => false,
    (a, b) if a != b => true,
    _ => false,
  }
}

fn is_sub_word_boundary(a: char, b: char, dir: Direction) -> bool {
  match (categorize_char(a), categorize_char(b)) {
    (CharCategory::Word, CharCategory::Word) => {
      if (a == '_') != (b == '_') {
        return true;
      }

      // Subword boundaries are directional: in 'fooBar', there is a
      // boundary between 'o' and 'B', but not between 'B' and 'a'.
      match dir {
        Direction::Forward => a.is_lowercase() && b.is_uppercase(),
        Direction::Backward => a.is_uppercase() && b.is_lowercase(),
      }
    },
    (a, b) if a != b => true,
    _ => false,
  }
}

fn reached_target(target: WordMotionTarget, prev_ch: char, next_ch: char) -> bool {
  match target {
    WordMotionTarget::NextWordStart | WordMotionTarget::PrevWordEnd => {
      is_word_boundary(prev_ch, next_ch)
        && (char_is_line_ending(next_ch) || !next_ch.is_whitespace())
    },
    WordMotionTarget::NextWordEnd | WordMotionTarget::PrevWordStart => {
      is_word_boundary(prev_ch, next_ch)
        && (!prev_ch.is_whitespace() || char_is_line_ending(next_ch))
    },
    WordMotionTarget::NextLongWordStart | WordMotionTarget::PrevLongWordEnd => {
      is_long_word_boundary(prev_ch, next_ch)
        && (char_is_line_ending(next_ch) || !next_ch.is_whitespace())
    },
    WordMotionTarget::NextLongWordEnd | WordMotionTarget::PrevLongWordStart => {
      is_long_word_boundary(prev_ch, next_ch)
        && (!prev_ch.is_whitespace() || char_is_line_ending(next_ch))
    },
    WordMotionTarget::NextSubWordStart => {
      is_sub_word_boundary(prev_ch, next_ch, Direction::Forward)
        && (char_is_line_ending(next_ch) || !(next_ch.is_whitespace() || next_ch == '_'))
    },
    WordMotionTarget::PrevSubWordEnd => {
      is_sub_word_boundary(prev_ch, next_ch, Direction::Backward)
        && (char_is_line_ending(next_ch) || !(next_ch.is_whitespace() || next_ch == '_'))
    },
    WordMotionTarget::NextSubWordEnd => {
      is_sub_word_boundary(prev_ch, next_ch, Direction::Forward)
        && (!(prev_ch.is_whitespace() || prev_ch == '_') || char_is_line_ending(next_ch))
    },
    WordMotionTarget::PrevSubWordStart => {
      is_sub_word_boundary(prev_ch, next_ch, Direction::Backward)
        && (!(prev_ch.is_whitespace() || prev_ch == '_') || char_is_line_ending(next_ch))
    },
  }
}

pub fn move_horizontally(
  slice: RopeSlice,
  range: Range,
  dir: Direction,
  count: usize,
  behavior: Movement,
  _: &TextFormat,
  _: &mut TextAnnotations,
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

pub fn move_vertically_visual(
  slice: RopeSlice,
  range: Range,
  dir: Direction,
  count: usize,
  behavior: Movement,
  text_fmt: &TextFormat,
  annotations: &mut TextAnnotations,
) -> Range {
  if !text_fmt.soft_wrap {
    return move_vertically(slice, range, dir, count, behavior, text_fmt, annotations);
  }

  annotations.clear_line_annotations();
  let pos = range.cursor(slice);

  let (visual_pos, block_off) = visual_offset_from_block(slice, pos, pos, text_fmt, annotations);
  let new_col = range
    .old_visual_pos
    .map_or(visual_pos.col as u32, |(_, col)| col);

  let mut row_off = match dir {
    Direction::Forward => count as isize,
    Direction::Backward => -(count as isize),
  };
  row_off += visual_pos.row as isize;

  let (mut new_pos, virtual_rows) = char_idx_at_visual_offset(
    slice,
    block_off,
    row_off,
    new_col as usize,
    text_fmt,
    annotations,
  );
  if dir == Direction::Forward {
    new_pos += (virtual_rows != 0) as usize;
  }

  if behavior == Movement::Extend && slice.line(slice.char_to_line(new_pos)).len_chars() == 0 {
    return range;
  }

  let mut new_range = range.put_cursor(slice, new_pos, behavior == Movement::Extend);
  new_range.old_visual_pos = Some((0, new_col));
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

fn word_move(slice: RopeSlice, range: Range, count: usize, target: WordMotionTarget) -> Range {
  let is_prev = matches!(
    target,
    WordMotionTarget::PrevWordStart
      | WordMotionTarget::PrevLongWordStart
      | WordMotionTarget::PrevSubWordStart
      | WordMotionTarget::PrevWordEnd
      | WordMotionTarget::PrevLongWordEnd
      | WordMotionTarget::PrevSubWordEnd
  );

  // Special case early-out.
  if (is_prev && range.head == 0) || (!is_prev && range.head == slice.len_chars()) {
    return range;
  }

  // Prepare the range appropriately based on the target movement
  // direction. This is addressing 2 things at once:
  //
  // 1. Block-cursor semantics.
  // 2. The anchor position being irrelevant to the output result.
  #[allow(clippy::collapsible_else_if)]
  let start_range = if is_prev {
    if range.anchor < range.head {
      Range::new(range.head, prev_grapheme_boundary(slice, range.head))
    } else {
      Range::new(next_grapheme_boundary(slice, range.head), range.head)
    }
  } else {
    if range.anchor < range.head {
      Range::new(prev_grapheme_boundary(slice, range.head), range.head)
    } else {
      Range::new(range.head, next_grapheme_boundary(slice, range.head))
    }
  };

  let mut range = start_range;
  for _ in 0..count {
    let next_range = slice.chars_at(range.head).range_to_target(target, range);
    if range == next_range {
      break;
    }

    range = next_range;
  }

  return range;
}

pub fn move_next_word_start(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::NextWordStart)
}

pub fn move_next_word_end(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::NextWordEnd)
}

pub fn move_prev_word_start(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::PrevWordStart)
}

pub fn move_prev_word_end(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::PrevWordEnd)
}

pub fn move_next_long_word_start(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::NextLongWordStart)
}

pub fn move_next_long_word_end(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::NextLongWordEnd)
}

pub fn move_prev_long_word_start(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::PrevLongWordStart)
}

pub fn move_prev_long_word_end(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::PrevLongWordEnd)
}

pub fn move_next_sub_word_start(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::NextSubWordStart)
}

pub fn move_next_sub_word_end(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::NextSubWordEnd)
}

pub fn move_prev_sub_word_start(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::PrevSubWordStart)
}

pub fn move_prev_sub_word_end(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::PrevSubWordEnd)
}
