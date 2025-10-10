use std::{
  borrow::Cow,
  cmp::Reverse,
  iter,
};

use ropey::{
  RopeSlice,
  iter::Chars,
};
use tree_house::tree_sitter::Node;

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
  line_ending::rope_is_line_ending,
  position::{
    char_idx_at_visual_block_offset,
    char_idx_at_visual_offset,
    visual_offset_from_block,
  },
  selection::{
    Range,
    Selection,
  },
  syntax,
  syntax::Syntax,
  text_annotations::TextAnnotations,
  text_format::TextFormat,
  textobject::TextObject,
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

  if behavior == Movement::Extend
    && new_pos < slice.len_chars()
    && slice.line(slice.char_to_line(new_pos)).len_chars() == 0
  {
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
  let line_idx = if pos < slice.len_chars() {
    slice.char_to_line(pos)
  } else {
    slice.len_lines().saturating_sub(1)
  };
  let line_start = slice.line_to_char(line_idx);

  // Compute the current positions's 2d coordinates.
  let visual_pos = visual_offset_from_block(slice, line_start, pos, text_fmt, annotations).0;

  let (mut new_row, new_col) = range
    .old_visual_pos
    .map_or((visual_pos.row as u32, visual_pos.col as u32), |pos| pos);
  new_row = new_row.max(visual_pos.row as u32);
  let line_idx = if pos < slice.len_chars() {
    slice.char_to_line(pos)
  } else {
    slice.len_lines().saturating_sub(1)
  };

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

  range
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

pub fn move_parent_node_end(
  syntax: &Syntax,
  text: RopeSlice,
  selection: Selection,
  dir: Direction,
  movement: Movement,
) -> Selection {
  selection.transform(|range| {
    let start_from = text.char_to_byte(range.from()) as u32;
    let start_to = text.char_to_byte(range.to()) as u32;

    let mut node = match syntax.named_descendant_for_byte_range(start_from, start_to) {
      Some(node) => node,
      None => {
        log::debug!(
          "no descendant found for byte range: {} - {}",
          start_from,
          start_to
        );
        return range;
      },
    };

    let mut end_head = match dir {
      // moving forward, we always want to move one past the end of the
      // current node, so use the end byte of the current node, which is an exclusive
      // end of the range
      Direction::Forward => text.byte_to_char(node.end_byte() as usize),

      // moving backward, we want the cursor to land on the start char of
      // the current node, or if it is already at the start of a node, to traverse up to
      // the parent
      Direction::Backward => {
        let end_head = text.byte_to_char(node.start_byte() as usize);

        // if we're already on the beginning, look up to the parent
        if end_head == range.cursor(text) {
          node = find_parent_start(&node).unwrap_or(node);
          text.byte_to_char(node.start_byte() as usize)
        } else {
          end_head
        }
      },
    };

    if movement == Movement::Move {
      // preserve direction of original range
      if range.direction() == Direction::Forward {
        Range::new(end_head, end_head + 1)
      } else {
        Range::new(end_head + 1, end_head)
      }
    } else {
      // if we end up with a forward range, then adjust it to be one past
      // where we want
      if end_head >= range.anchor {
        end_head += 1;
      }

      Range::new(range.anchor, end_head)
    }
  })
}

fn find_parent_start<'tree>(node: &Node<'tree>) -> Option<Node<'tree>> {
  let start = node.start_byte();
  let mut node = Cow::Borrowed(node);

  while node.start_byte() >= start || !node.is_named() {
    node = Cow::Owned(node.parent()?);
  }

  Some(node.into_owned())
}

pub fn goto_treesitter_object(
  slice: RopeSlice,
  range: Range,
  object_name: &str,
  dir: Direction,
  slice_tree: &Node,
  syntax: &Syntax,
  loader: &syntax::Loader,
  count: usize,
) -> Range {
  let textobject_query = loader.textobject_query(syntax.root_language());
  let get_range = move |range: Range| -> Option<Range> {
    let byte_pos = slice.char_to_byte(range.cursor(slice));

    let movement_cap = format!("{}.{}", object_name, TextObject::Movement);
    let around_cap = format!("{}.{}", object_name, TextObject::Around);
    let inside_cap = format!("{}.{}", object_name, TextObject::Inside);
    let nodes = textobject_query?.capture_nodes_any(
      [
        movement_cap.as_str(),
        around_cap.as_str(),
        inside_cap.as_str(),
      ],
      slice_tree,
      slice,
    )?;

    let node = match dir {
      Direction::Forward => {
        nodes
          .filter(|n| n.start_byte() > byte_pos)
          .min_by_key(|n| (n.start_byte(), Reverse(n.end_byte())))?
      },
      Direction::Backward => {
        nodes
          .filter(|n| n.end_byte() < byte_pos)
          .max_by_key(|n| (n.end_byte(), Reverse(n.start_byte())))?
      },
    };

    let len = slice.len_bytes();
    let start_byte = node.start_byte();
    let end_byte = node.end_byte();
    if start_byte >= len || end_byte >= len {
      return None;
    }

    let start_char = slice.byte_to_char(start_byte);
    let end_char = slice.byte_to_char(end_byte);

    // head of range should be at beginning
    Some(Range::new(start_char, end_char))
  };
  let mut last_range = range;
  for _ in 0..count {
    match get_range(last_range) {
      Some(r) if r != last_range => last_range = r,
      _ => break,
    }
  }
  last_range
}

pub fn move_prev_paragraph(
  slice: RopeSlice,
  range: Range,
  count: usize,
  behavior: Movement,
) -> Range {
  let mut line = range.cursor_line(slice);
  let first_char = slice.line_to_char(line) == range.cursor(slice);
  let prev_line_empty = rope_is_line_ending(slice.line(line.saturating_sub(1)));
  let curr_line_empty = rope_is_line_ending(slice.line(line));
  let prev_empty_to_line = prev_line_empty && !curr_line_empty;

  // skip character before paragraph boundary
  if prev_empty_to_line && !first_char {
    line += 1;
  }
  let mut lines = slice.lines_at(line);
  lines.reverse();
  let mut lines = lines.map(rope_is_line_ending).peekable();
  let mut last_line = line;
  for _ in 0..count {
    while lines.next_if(|&e| e).is_some() {
      line -= 1;
    }
    while lines.next_if(|&e| !e).is_some() {
      line -= 1;
    }
    if line == last_line {
      break;
    }
    last_line = line;
  }

  let head = slice.line_to_char(line);
  let anchor = if behavior == Movement::Move {
    // exclude first character after paragraph boundary
    if prev_empty_to_line && first_char {
      range.cursor(slice)
    } else {
      range.head
    }
  } else {
    range.put_cursor(slice, head, true).anchor
  };
  Range::new(anchor, head)
}

pub fn move_next_paragraph(
  slice: RopeSlice,
  range: Range,
  count: usize,
  behavior: Movement,
) -> Range {
  let mut line = range.cursor_line(slice);
  let last_char =
    prev_grapheme_boundary(slice, slice.line_to_char(line + 1)) == range.cursor(slice);
  let curr_line_empty = rope_is_line_ending(slice.line(line));
  let next_line_empty =
    rope_is_line_ending(slice.line(slice.len_lines().saturating_sub(1).min(line + 1)));
  let curr_empty_to_line = curr_line_empty && !next_line_empty;

  // skip character after paragraph boundary
  if curr_empty_to_line && last_char {
    line += 1;
  }
  let mut lines = slice.lines_at(line).map(rope_is_line_ending).peekable();
  let mut last_line = line;
  for _ in 0..count {
    while lines.next_if(|&e| !e).is_some() {
      line += 1;
    }
    while lines.next_if(|&e| e).is_some() {
      line += 1;
    }
    if line == last_line {
      break;
    }
    last_line = line;
  }
  let head = slice.line_to_char(line);
  let anchor = if behavior == Movement::Move {
    if curr_empty_to_line && last_char {
      range.head
    } else {
      range.cursor(slice)
    }
  } else {
    range.put_cursor(slice, head, true).anchor
  };
  Range::new(anchor, head)
}

#[inline]
/// Returns first index that doesn't satisfy a given predicate when
/// advancing the character index.
///
/// Returns none if all characters satisfy the predicate.
pub fn skip_while<F>(slice: RopeSlice, pos: usize, fun: F) -> Option<usize>
where
  F: Fn(char) -> bool,
{
  let mut chars = slice.chars_at(pos).enumerate();
  chars.find_map(|(i, c)| if !fun(c) { Some(pos + i) } else { None })
}

#[inline]
/// Returns first index that doesn't satisfy a given predicate when
/// retreating the character index, saturating if all elements satisfy
/// the condition.
pub fn backwards_skip_while<F>(slice: RopeSlice, pos: usize, fun: F) -> Option<usize>
where
  F: Fn(char) -> bool,
{
  let mut chars_starting_from_next = slice.chars_at(pos);
  let mut backwards = iter::from_fn(|| chars_starting_from_next.prev()).enumerate();
  backwards.find_map(|(i, c)| {
    if !fun(c) {
      Some(pos.saturating_sub(i))
    } else {
      None
    }
  })
}
