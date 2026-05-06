use std::collections::{
  BTreeMap,
  BTreeSet,
};

use ropey::{
  RopeSlice,
  iter::Chars,
};
use the_stdx::rope::RopeSliceExt;

use crate::{
  chars::{
    CharCategory,
    categorize_char,
    char_is_line_ending,
  },
  grapheme::{
    next_grapheme_boundary,
    prev_grapheme_boundary,
  },
  line_ending::line_end_char_index,
  selection::{
    CursorId,
    CursorRange,
    Range,
    Selection,
  },
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Direction {
  Forward,
  Backward,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MotionMode {
  /// Extend the selection.
  Extend,

  /// Move the selection, collapsing to the moved cursor.
  Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualGoal {
  pub col: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VisualGoals {
  goals: BTreeMap<CursorId, VisualGoal>,
}

impl VisualGoals {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn is_empty(&self) -> bool {
    self.goals.is_empty()
  }

  pub fn get(&self, cursor_id: CursorId) -> Option<VisualGoal> {
    self.goals.get(&cursor_id).copied()
  }

  pub fn insert(&mut self, cursor_id: CursorId, goal: VisualGoal) {
    self.goals.insert(cursor_id, goal);
  }

  pub fn remove(&mut self, cursor_id: CursorId) {
    self.goals.remove(&cursor_id);
  }

  pub fn iter(&self) -> impl Iterator<Item = (CursorId, VisualGoal)> + '_ {
    self
      .goals
      .iter()
      .map(|(cursor_id, goal)| (*cursor_id, *goal))
  }

  pub fn retain_ids(&mut self, ids: impl IntoIterator<Item = CursorId>) {
    let ids = ids.into_iter().collect::<BTreeSet<_>>();
    self.goals.retain(|cursor_id, _| ids.contains(cursor_id));
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionResult {
  pub selection:    Selection,
  pub visual_goals: VisualGoals,
}

#[must_use]
pub fn move_char(
  text: RopeSlice,
  selection: &Selection,
  direction: Direction,
  count: usize,
  mode: MotionMode,
) -> Selection {
  let count = count.max(1);
  let extend = mode == MotionMode::Extend;
  selection.clone().transform(|range| {
    let mut pos = range.cursor(text);
    for _ in 0..count {
      pos = match direction {
        Direction::Forward => next_grapheme_boundary(text, pos),
        Direction::Backward => prev_grapheme_boundary(text, pos),
      };
    }
    range.put_cursor(text, pos, extend)
  })
}

#[must_use]
pub fn move_char_left(text: RopeSlice, selection: &Selection, count: usize) -> Selection {
  move_char(
    text,
    selection,
    Direction::Backward,
    count,
    MotionMode::Move,
  )
}

#[must_use]
pub fn move_char_right(text: RopeSlice, selection: &Selection, count: usize) -> Selection {
  move_char(text, selection, Direction::Forward, count, MotionMode::Move)
}

#[must_use]
pub fn extend_char_left(text: RopeSlice, selection: &Selection, count: usize) -> Selection {
  move_char(
    text,
    selection,
    Direction::Backward,
    count,
    MotionMode::Extend,
  )
}

#[must_use]
pub fn extend_char_right(text: RopeSlice, selection: &Selection, count: usize) -> Selection {
  move_char(
    text,
    selection,
    Direction::Forward,
    count,
    MotionMode::Extend,
  )
}

#[must_use]
pub fn move_visual_line(
  text: RopeSlice,
  selection: &Selection,
  goals: &VisualGoals,
  direction: Direction,
  count: usize,
  mode: MotionMode,
) -> MotionResult {
  let count = count.max(1);
  let extend = mode == MotionMode::Extend;
  let mut next_goals = VisualGoals::new();

  let selection = selection
    .clone()
    .transform_entries(|cursor| {
      let pos = cursor.range.cursor(text);
      let line = text.char_to_line(pos);
      let col = goals
        .get(cursor.id)
        .map(|goal| goal.col as usize)
        .unwrap_or_else(|| pos - text.line_to_char(line));
      let target_line = match direction {
        Direction::Forward => line.saturating_add(count),
        Direction::Backward => line.saturating_sub(count),
      }
      .min(text.len_lines().saturating_sub(1));
      let line_start = text.line_to_char(target_line);
      let line_end = line_content_end(text, target_line);
      let target = line_start + col.min(line_end.saturating_sub(line_start));
      next_goals.insert(cursor.id, VisualGoal { col: col as u32 });
      CursorRange {
        id:    cursor.id,
        range: cursor.range.put_cursor(text, target, extend),
      }
    })
    .unwrap_or_else(|_| selection.clone());

  MotionResult {
    selection,
    visual_goals: next_goals,
  }
}

#[must_use]
pub fn move_visual_line_up(
  text: RopeSlice,
  selection: &Selection,
  goals: &VisualGoals,
  count: usize,
) -> MotionResult {
  move_visual_line(
    text,
    selection,
    goals,
    Direction::Backward,
    count,
    MotionMode::Move,
  )
}

#[must_use]
pub fn move_visual_line_down(
  text: RopeSlice,
  selection: &Selection,
  goals: &VisualGoals,
  count: usize,
) -> MotionResult {
  move_visual_line(
    text,
    selection,
    goals,
    Direction::Forward,
    count,
    MotionMode::Move,
  )
}

#[must_use]
pub fn extend_visual_line_up(
  text: RopeSlice,
  selection: &Selection,
  goals: &VisualGoals,
  count: usize,
) -> MotionResult {
  move_visual_line(
    text,
    selection,
    goals,
    Direction::Backward,
    count,
    MotionMode::Extend,
  )
}

#[must_use]
pub fn extend_visual_line_down(
  text: RopeSlice,
  selection: &Selection,
  goals: &VisualGoals,
  count: usize,
) -> MotionResult {
  move_visual_line(
    text,
    selection,
    goals,
    Direction::Forward,
    count,
    MotionMode::Extend,
  )
}

#[must_use]
pub fn move_next_word_start_selection(
  text: RopeSlice,
  selection: &Selection,
  count: usize,
) -> Selection {
  selection
    .clone()
    .transform(|range| move_next_word_start(text, range, count.max(1)))
}

#[must_use]
pub fn move_prev_word_start_selection(
  text: RopeSlice,
  selection: &Selection,
  count: usize,
) -> Selection {
  selection
    .clone()
    .transform(|range| move_prev_word_start(text, range, count.max(1)))
}

#[must_use]
pub fn move_next_word_end_selection(
  text: RopeSlice,
  selection: &Selection,
  count: usize,
) -> Selection {
  selection
    .clone()
    .transform(|range| move_next_word_end(text, range, count.max(1)))
}

#[must_use]
pub fn move_next_long_word_start_selection(
  text: RopeSlice,
  selection: &Selection,
  count: usize,
) -> Selection {
  selection
    .clone()
    .transform(|range| move_next_long_word_start(text, range, count.max(1)))
}

#[must_use]
pub fn move_prev_long_word_start_selection(
  text: RopeSlice,
  selection: &Selection,
  count: usize,
) -> Selection {
  selection
    .clone()
    .transform(|range| move_prev_long_word_start(text, range, count.max(1)))
}

#[must_use]
pub fn move_next_long_word_end_selection(
  text: RopeSlice,
  selection: &Selection,
  count: usize,
) -> Selection {
  selection
    .clone()
    .transform(|range| move_next_long_word_end(text, range, count.max(1)))
}

#[must_use]
pub fn extend_next_word_start(text: RopeSlice, selection: &Selection, count: usize) -> Selection {
  selection.clone().transform(|range| {
    let word = move_next_word_start(text, range, count.max(1));
    range.put_cursor(text, word.cursor(text), true)
  })
}

#[must_use]
pub fn extend_prev_word_start(text: RopeSlice, selection: &Selection, count: usize) -> Selection {
  selection.clone().transform(|range| {
    let word = move_prev_word_start(text, range, count.max(1));
    range.put_cursor(text, word.cursor(text), true)
  })
}

#[must_use]
pub fn extend_next_word_end(text: RopeSlice, selection: &Selection, count: usize) -> Selection {
  selection.clone().transform(|range| {
    let word = move_next_word_end(text, range, count.max(1));
    range.put_cursor(text, word.cursor(text), true)
  })
}

#[must_use]
pub fn extend_next_long_word_start(
  text: RopeSlice,
  selection: &Selection,
  count: usize,
) -> Selection {
  selection.clone().transform(|range| {
    let word = move_next_long_word_start(text, range, count.max(1));
    range.put_cursor(text, word.cursor(text), true)
  })
}

#[must_use]
pub fn extend_prev_long_word_start(
  text: RopeSlice,
  selection: &Selection,
  count: usize,
) -> Selection {
  selection.clone().transform(|range| {
    let word = move_prev_long_word_start(text, range, count.max(1));
    range.put_cursor(text, word.cursor(text), true)
  })
}

#[must_use]
pub fn extend_next_long_word_end(
  text: RopeSlice,
  selection: &Selection,
  count: usize,
) -> Selection {
  selection.clone().transform(|range| {
    let word = move_next_long_word_end(text, range, count.max(1));
    range.put_cursor(text, word.cursor(text), true)
  })
}

#[must_use]
pub fn goto_line_start(text: RopeSlice, selection: &Selection, mode: MotionMode) -> Selection {
  goto_by_cursor(text, selection, mode, |text, range| {
    text.line_to_char(range.cursor_line(text))
  })
}

#[must_use]
pub fn goto_first_non_whitespace(
  text: RopeSlice,
  selection: &Selection,
  mode: MotionMode,
) -> Selection {
  goto_by_cursor(text, selection, mode, |text, range| {
    let line = range.cursor_line(text);
    let start = text.line_to_char(line);
    text
      .line(line)
      .first_non_whitespace_char()
      .map(|offset| start + offset)
      .unwrap_or(start)
  })
}

#[must_use]
pub fn goto_line_end(text: RopeSlice, selection: &Selection, mode: MotionMode) -> Selection {
  goto_by_cursor(text, selection, mode, |text, range| {
    line_content_end(text, range.cursor_line(text))
  })
}

#[must_use]
pub fn goto_line_end_newline(
  text: RopeSlice,
  selection: &Selection,
  mode: MotionMode,
) -> Selection {
  goto_by_cursor(text, selection, mode, |text, range| {
    let line = range.cursor_line(text);
    let line_start = text.line_to_char(line);
    line_start + text.line(line).len_chars()
  })
}

#[must_use]
pub fn goto_document_start(_text: RopeSlice, selection: &Selection, mode: MotionMode) -> Selection {
  goto_position(selection, 0, mode)
}

#[must_use]
pub fn goto_document_end(text: RopeSlice, selection: &Selection, mode: MotionMode) -> Selection {
  goto_position(selection, text.len_chars(), mode)
}

#[must_use]
pub fn goto_line(
  text: RopeSlice,
  selection: &Selection,
  line: usize,
  mode: MotionMode,
) -> Selection {
  let line = line.min(text.len_lines().saturating_sub(1));
  goto_position(selection, text.line_to_char(line), mode)
}

#[must_use]
pub fn goto_column(
  text: RopeSlice,
  selection: &Selection,
  column: usize,
  mode: MotionMode,
) -> Selection {
  let column = column.saturating_sub(1);
  goto_by_cursor(text, selection, mode, |text, range| {
    let line = range.cursor_line(text);
    let line_start = text.line_to_char(line);
    let line_end = line_content_end(text, line);
    line_start + column.min(line_end.saturating_sub(line_start))
  })
}

#[must_use]
pub fn extend_line_below(text: RopeSlice, selection: &Selection, count: usize) -> Selection {
  extend_line(text, selection, Direction::Forward, count)
}

#[must_use]
pub fn extend_line_above(text: RopeSlice, selection: &Selection, count: usize) -> Selection {
  extend_line(text, selection, Direction::Backward, count)
}

fn goto_by_cursor(
  text: RopeSlice,
  selection: &Selection,
  mode: MotionMode,
  mut f: impl FnMut(RopeSlice, Range) -> usize,
) -> Selection {
  let extend = mode == MotionMode::Extend;
  selection.clone().transform(|range| {
    let pos = f(text, range);
    range.put_cursor(text, pos, extend)
  })
}

fn goto_position(selection: &Selection, pos: usize, mode: MotionMode) -> Selection {
  let extend = mode == MotionMode::Extend;
  selection.clone().transform(|range| {
    if extend {
      Range::new(range.anchor, pos)
    } else {
      Range::point(pos)
    }
  })
}

fn extend_line(
  text: RopeSlice,
  selection: &Selection,
  direction: Direction,
  count: usize,
) -> Selection {
  let count = count.max(1);
  selection.clone().transform(|range| {
    let (start_line, end_line) = range.line_range(text);
    let start = text.line_to_char(start_line);
    let end = text.line_to_char((end_line + 1).min(text.len_lines()));

    let (anchor, head) = if range.from() == start && range.to() == end {
      match direction {
        Direction::Backward => (end, text.line_to_char(start_line.saturating_sub(count))),
        Direction::Forward => {
          (
            start,
            text.line_to_char((end_line + count + 1).min(text.len_lines())),
          )
        },
      }
    } else {
      match direction {
        Direction::Backward => (end, text.line_to_char(start_line.saturating_sub(count - 1))),
        Direction::Forward => {
          (
            start,
            text.line_to_char((end_line + count).min(text.len_lines())),
          )
        },
      }
    };

    Range::new(anchor, head)
  })
}

fn line_content_end(text: RopeSlice, line: usize) -> usize {
  line_end_char_index(&text, line)
}

/// Possible targets of a word motion.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WordMotionTarget {
  NextWordStart,
  NextWordEnd,
  PrevWordStart,
  PrevWordEnd,
  NextLongWordStart,
  NextLongWordEnd,
  PrevLongWordStart,
  PrevLongWordEnd,
}

impl WordMotionTarget {
  #[inline]
  pub fn is_backward(self) -> bool {
    matches!(
      self,
      Self::PrevWordStart | Self::PrevWordEnd | Self::PrevLongWordStart | Self::PrevLongWordEnd
    )
  }
}

#[must_use]
pub fn move_next_word_start(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::NextWordStart)
}

#[must_use]
pub fn move_next_word_end(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::NextWordEnd)
}

#[must_use]
pub fn move_prev_word_start(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::PrevWordStart)
}

#[must_use]
pub fn move_prev_word_end(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::PrevWordEnd)
}

#[must_use]
pub fn move_next_long_word_start(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::NextLongWordStart)
}

#[must_use]
pub fn move_next_long_word_end(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::NextLongWordEnd)
}

#[must_use]
pub fn move_prev_long_word_start(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::PrevLongWordStart)
}

#[must_use]
pub fn move_prev_long_word_end(slice: RopeSlice, range: Range, count: usize) -> Range {
  word_move(slice, range, count, WordMotionTarget::PrevLongWordEnd)
}

fn word_move(slice: RopeSlice, range: Range, count: usize, target: WordMotionTarget) -> Range {
  let is_prev = target.is_backward();

  if (is_prev && range.head == 0) || (!is_prev && range.head == slice.len_chars()) {
    return range;
  }

  let start_range = if is_prev {
    if range.anchor < range.head {
      Range::new(range.head, prev_grapheme_boundary(slice, range.head))
    } else {
      Range::new(next_grapheme_boundary(slice, range.head), range.head)
    }
  } else if range.anchor < range.head {
    Range::new(prev_grapheme_boundary(slice, range.head), range.head)
  } else {
    Range::new(range.head, next_grapheme_boundary(slice, range.head))
  };

  let mut range = start_range;
  for _ in 0..count.max(1) {
    let next_range = slice.chars_at(range.head).range_to_target(target, range);
    if range == next_range {
      break;
    }
    range = next_range;
  }

  range
}

trait CharHelpers {
  fn range_to_target(&mut self, target: WordMotionTarget, origin: Range) -> Range;
}

impl CharHelpers for Chars<'_> {
  /// Note: this only changes the anchor if the head is effectively starting on
  /// a boundary (either directly or after skipping newline characters). Any
  /// other anchor changes should be handled by the caller.
  fn range_to_target(&mut self, target: WordMotionTarget, origin: Range) -> Range {
    let is_prev = target.is_backward();

    if is_prev {
      self.reverse();
    }

    let advance: &dyn Fn(&mut usize) = if is_prev {
      &|idx| *idx = idx.saturating_sub(1)
    } else {
      &|idx| *idx += 1
    };

    let mut anchor = origin.anchor;
    let mut head = origin.head;
    let mut prev_ch = {
      let ch = self.prev();
      if ch.is_some() {
        self.next();
      }
      ch
    };

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

    let head_start = head;
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
  }
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::*;

  #[test]
  fn word_motions_use_helix_block_selection_semantics() {
    let doc = Rope::from("hello world");
    let text = doc.slice(..);

    assert_eq!(
      move_next_word_start(text, Range::point(0), 1),
      Range::new(0, 6)
    );
    assert_eq!(
      move_next_word_end(text, Range::point(0), 1),
      Range::new(0, 5)
    );
    assert_eq!(
      move_prev_word_start(text, Range::point(6), 1),
      Range::new(6, 0)
    );
  }
}
