use std::{
  collections::{
    BTreeMap,
    BTreeSet,
  },
  iter,
};

use ropey::{
  RopeSlice,
  iter::Chars,
};
use smallvec::SmallVec;

use crate::{
  chars::{
    CharCategory,
    categorize_char,
    char_is_line_ending,
  },
  operation::SelectionTransform,
  grapheme::{
    next_grapheme_boundary,
    nth_next_grapheme_boundary,
    nth_prev_grapheme_boundary,
    prev_grapheme_boundary,
  },
  line_ending::{
    get_line_ending,
    rope_is_line_ending,
  },
  render::{
    TextAnnotations,
    TextFormat,
    layout_index::{
      char_idx_at_visual_block_offset,
      char_idx_at_visual_offset,
      visual_offset_from_block,
    },
  },
  selection::{
    CursorId,
    CursorRange,
    Range,
    Selection,
  },
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Direction {
  Forward,
  Backward,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MotionMode {
  /// Extend the selection.
  Extend,

  /// Move the selection, collapsing to the moved cursor.
  Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WordKind {
  Word,
  LongWord,
  SubWord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WordTarget {
  Start,
  End,
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

#[derive(Debug, Clone, Copy)]
pub struct MovementContext<'a> {
  pub text_format: &'a TextFormat,
  pub annotations: &'a TextAnnotations,
  pub wrap_width:  usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionResult {
  pub selection:    Selection,
  pub visual_goals: VisualGoals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VerticalMoveResult {
  range:       Range,
  visual_goal: VisualGoal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CursorMotionResult {
  range:       Range,
  visual_goal: Option<VisualGoal>,
}

#[derive(Copy, Clone, Debug)]
enum InternalWordMotionTarget {
  NextWordStart,
  NextWordEnd,
  PrevWordStart,
  PrevWordEnd,
  NextLongWordStart,
  NextLongWordEnd,
  PrevLongWordStart,
  PrevLongWordEnd,
  NextSubWordStart,
  NextSubWordEnd,
  PrevSubWordStart,
  PrevSubWordEnd,
}

impl InternalWordMotionTarget {
  #[inline]
  fn is_backward(self) -> bool {
    matches!(
      self,
      Self::PrevWordStart
        | Self::PrevWordEnd
        | Self::PrevLongWordStart
        | Self::PrevLongWordEnd
        | Self::PrevSubWordStart
        | Self::PrevSubWordEnd
    )
  }
}

trait CharHelpers {
  fn range_to_target(&mut self, target: InternalWordMotionTarget, origin: Range) -> Range;
}

impl CharHelpers for Chars<'_> {
  fn range_to_target(&mut self, target: InternalWordMotionTarget, origin: Range) -> Range {
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
    #[allow(clippy::while_let_on_iterator)]
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

pub fn transform_selection(
  text: RopeSlice,
  selection: Selection,
  visual_goals: &VisualGoals,
  transform: SelectionTransform,
  cx: &MovementContext<'_>,
) -> MotionResult {
  match transform {
    SelectionTransform::Horizontal {
      direction,
      count,
      mode,
    } => apply_selection_motion(text, selection, visual_goals, |range, _| {
      CursorMotionResult {
        range:       move_horizontally_range(text, range, direction, count, mode),
        visual_goal: None,
      }
    }),
    SelectionTransform::Vertical {
      direction,
      count,
      mode,
    } => apply_selection_motion(text, selection, visual_goals, |range, visual_goal| {
      let result = move_vertically_range(text, range, visual_goal, direction, count, mode, cx);
      CursorMotionResult {
        range:       result.range,
        visual_goal: Some(result.visual_goal),
      }
    }),
    SelectionTransform::VisualVertical {
      direction,
      count,
      mode,
    } => apply_selection_motion(text, selection, visual_goals, |range, visual_goal| {
      let result = move_vertically_visual_range(text, range, visual_goal, direction, count, mode, cx);
      CursorMotionResult {
        range:       result.range,
        visual_goal: Some(result.visual_goal),
      }
    }),
    SelectionTransform::Word {
      direction,
      kind,
      target,
      count,
      mode,
    } => apply_selection_motion(text, selection, visual_goals, |range, _| {
      CursorMotionResult {
        range:       move_to_pos(text, range, word_target_pos(text, range, count, word_target(direction, kind, target)), mode),
        visual_goal: None,
      }
    }),
    SelectionTransform::Paragraph {
      direction,
      count,
      mode,
    } => apply_selection_motion(text, selection, visual_goals, |range, _| {
      CursorMotionResult {
        range:       match direction {
          Direction::Backward => move_prev_paragraph_range(text, range, count, mode),
          Direction::Forward => move_next_paragraph_range(text, range, count, mode),
        },
        visual_goal: None,
      }
    }),
    SelectionTransform::LineStart { mode } => apply_selection_motion(text, selection, visual_goals, |range, _| {
      CursorMotionResult {
        range:       move_to_pos(text, range, line_start(text, range.cursor_line(text)), mode),
        visual_goal: None,
      }
    }),
    SelectionTransform::LineEnd { mode } => apply_selection_motion(text, selection, visual_goals, |range, _| {
      CursorMotionResult {
        range:       move_to_pos(text, range, line_content_end(text, range.cursor_line(text)), mode),
        visual_goal: None,
      }
    }),
    SelectionTransform::FirstNonWhitespace { mode } => {
      apply_selection_motion(text, selection, visual_goals, |range, _| CursorMotionResult {
        range:       move_to_pos(text, range, first_non_whitespace(text, range.cursor_line(text)), mode),
        visual_goal: None,
      })
    },
    SelectionTransform::DocumentStart { mode } => apply_selection_motion(text, selection, visual_goals, |range, _| {
      CursorMotionResult {
        range:       move_to_pos(text, range, 0, mode),
        visual_goal: None,
      }
    }),
    SelectionTransform::DocumentEnd { mode } => apply_selection_motion(text, selection, visual_goals, |range, _| {
      CursorMotionResult {
        range:       move_to_pos(text, range, text.len_chars(), mode),
        visual_goal: None,
      }
    }),
  }
}

fn apply_selection_motion<F>(
  text: RopeSlice,
  selection: Selection,
  visual_goals: &VisualGoals,
  move_cursor: F,
) -> MotionResult
where
  F: Fn(Range, Option<VisualGoal>) -> CursorMotionResult,
{
  let mut cursors = SmallVec::<[CursorRange; 1]>::new();
  let mut next_visual_goals = VisualGoals::new();

  for cursor in selection.iter() {
    let old_goal = visual_goals.get(cursor.id);
    let result = move_cursor(*cursor.range, old_goal);

    cursors.push(CursorRange {
      id:    cursor.id,
      range: result.range,
    });

    match result.visual_goal {
      Some(goal) => next_visual_goals.insert(cursor.id, goal),
      None if result.range == *cursor.range => {
        if let Some(goal) = old_goal {
          next_visual_goals.insert(cursor.id, goal);
        }
      },
      None => {},
    }
  }

  let selection =
    Selection::from_cursors(cursors).expect("movement must preserve non-empty unique cursor IDs");
  next_visual_goals.retain_ids(selection.iter().map(|cursor| cursor.id));
  let _ = text;

  MotionResult {
    selection,
    visual_goals: next_visual_goals,
  }
}

fn move_to_pos(text: RopeSlice, range: Range, pos: usize, mode: MotionMode) -> Range {
  range.put_cursor(text, pos.min(text.len_chars()), mode == MotionMode::Extend)
}

fn move_horizontally_range(
  text: RopeSlice,
  range: Range,
  direction: Direction,
  count: usize,
  mode: MotionMode,
) -> Range {
  let pos = range.cursor(text);
  let count = count.max(1);

  let new_pos = match direction {
    Direction::Forward => nth_next_grapheme_boundary(text, pos, count),
    Direction::Backward => nth_prev_grapheme_boundary(text, pos, count),
  };

  move_to_pos(text, range, new_pos, mode)
}

fn move_vertically_visual_range(
  text: RopeSlice,
  range: Range,
  visual_goal: Option<VisualGoal>,
  direction: Direction,
  count: usize,
  mode: MotionMode,
  cx: &MovementContext<'_>,
) -> VerticalMoveResult {
  let pos = range.cursor(text);
  let (visual_pos, _block_start) = visual_offset_from_block(
    text,
    pos,
    pos,
    cx.text_format,
    cx.wrap_width,
    cx.annotations,
  );
  let goal = VisualGoal {
    col: visual_goal.map_or(visual_pos.col as u32, |goal| goal.col),
  };
  let count = count.max(1);

  let row_offset = match direction {
    Direction::Forward => count as isize,
    Direction::Backward => -(count as isize),
  };
  let (new_pos, _) = char_idx_at_visual_offset(
    text,
    pos,
    row_offset,
    goal.col as usize,
    cx.text_format,
    cx.wrap_width,
    cx.annotations,
  );

  VerticalMoveResult {
    range:       move_to_pos(text, range, new_pos, mode),
    visual_goal: goal,
  }
}

fn move_vertically_range(
  text: RopeSlice,
  range: Range,
  visual_goal: Option<VisualGoal>,
  direction: Direction,
  count: usize,
  mode: MotionMode,
  cx: &MovementContext<'_>,
) -> VerticalMoveResult {
  let pos = range.cursor(text);
  let line_idx = if pos < text.len_chars() {
    text.char_to_line(pos)
  } else {
    text.len_lines().saturating_sub(1)
  };
  let line_start = text.line_to_char(line_idx);
  let current = visual_offset_from_block(
    text,
    line_start,
    pos,
    cx.text_format,
    cx.wrap_width,
    cx.annotations,
  )
  .0;

  let goal = VisualGoal {
    col: visual_goal.map_or(current.col as u32, |goal| goal.col),
  };
  let count = count.max(1);

  let mut new_line_idx = match direction {
    Direction::Forward => line_idx.saturating_add(count),
    Direction::Backward => line_idx.saturating_sub(count),
  };

  let line_slice = if new_line_idx >= text.len_lines().saturating_sub(1) {
    new_line_idx = text.len_lines().saturating_sub(1);
    text
  } else {
    let new_line_end = prev_grapheme_boundary(text, text.line_to_char(new_line_idx + 1));
    text.slice(..new_line_end)
  };
  let new_line_start = line_slice.line_to_char(new_line_idx);

  let (new_pos, _) = char_idx_at_visual_block_offset(
    line_slice,
    new_line_start,
    current.row,
    goal.col as usize,
    cx.text_format,
    cx.wrap_width,
    cx.annotations,
  );

  VerticalMoveResult {
    range:       move_to_pos(text, range, new_pos, mode),
    visual_goal: goal,
  }
}

fn word_target(
  direction: Direction,
  kind: WordKind,
  target: WordTarget,
) -> InternalWordMotionTarget {
  match (direction, kind, target) {
    (Direction::Forward, WordKind::Word, WordTarget::Start) => {
      InternalWordMotionTarget::NextWordStart
    },
    (Direction::Forward, WordKind::Word, WordTarget::End) => InternalWordMotionTarget::NextWordEnd,
    (Direction::Backward, WordKind::Word, WordTarget::Start) => {
      InternalWordMotionTarget::PrevWordStart
    },
    (Direction::Backward, WordKind::Word, WordTarget::End) => InternalWordMotionTarget::PrevWordEnd,

    (Direction::Forward, WordKind::LongWord, WordTarget::Start) => {
      InternalWordMotionTarget::NextLongWordStart
    },
    (Direction::Forward, WordKind::LongWord, WordTarget::End) => {
      InternalWordMotionTarget::NextLongWordEnd
    },
    (Direction::Backward, WordKind::LongWord, WordTarget::Start) => {
      InternalWordMotionTarget::PrevLongWordStart
    },
    (Direction::Backward, WordKind::LongWord, WordTarget::End) => {
      InternalWordMotionTarget::PrevLongWordEnd
    },

    (Direction::Forward, WordKind::SubWord, WordTarget::Start) => {
      InternalWordMotionTarget::NextSubWordStart
    },
    (Direction::Forward, WordKind::SubWord, WordTarget::End) => {
      InternalWordMotionTarget::NextSubWordEnd
    },
    (Direction::Backward, WordKind::SubWord, WordTarget::Start) => {
      InternalWordMotionTarget::PrevSubWordStart
    },
    (Direction::Backward, WordKind::SubWord, WordTarget::End) => {
      InternalWordMotionTarget::PrevSubWordEnd
    },
  }
}

fn word_target_pos(
  text: RopeSlice,
  range: Range,
  count: usize,
  target: InternalWordMotionTarget,
) -> usize {
  word_move(text, range, count.max(1), target).cursor(text)
}

fn word_move(
  text: RopeSlice,
  range: Range,
  count: usize,
  target: InternalWordMotionTarget,
) -> Range {
  let is_prev = target.is_backward();

  if (is_prev && range.head == 0) || (!is_prev && range.head == text.len_chars()) {
    return range;
  }

  #[allow(clippy::collapsible_else_if)]
  let start_range = if is_prev {
    if range.anchor < range.head {
      Range::new(range.head, prev_grapheme_boundary(text, range.head))
    } else {
      Range::new(next_grapheme_boundary(text, range.head), range.head)
    }
  } else if range.anchor < range.head {
    Range::new(prev_grapheme_boundary(text, range.head), range.head)
  } else {
    Range::new(range.head, next_grapheme_boundary(text, range.head))
  };

  let mut range = start_range;
  for _ in 0..count {
    let next_range = text.chars_at(range.head).range_to_target(target, range);
    if range == next_range {
      break;
    }
    range = next_range;
  }

  range
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

fn is_sub_word_boundary(a: char, b: char, direction: Direction) -> bool {
  match (categorize_char(a), categorize_char(b)) {
    (CharCategory::Word, CharCategory::Word) => {
      if (a == '_') != (b == '_') {
        return true;
      }

      match direction {
        Direction::Forward => a.is_lowercase() && b.is_uppercase(),
        Direction::Backward => a.is_uppercase() && b.is_lowercase(),
      }
    },
    (a, b) if a != b => true,
    _ => false,
  }
}

fn reached_target(target: InternalWordMotionTarget, prev_ch: char, next_ch: char) -> bool {
  match target {
    InternalWordMotionTarget::NextWordStart | InternalWordMotionTarget::PrevWordEnd => {
      is_word_boundary(prev_ch, next_ch)
        && (char_is_line_ending(next_ch) || !next_ch.is_whitespace())
    },
    InternalWordMotionTarget::NextWordEnd | InternalWordMotionTarget::PrevWordStart => {
      is_word_boundary(prev_ch, next_ch)
        && (!prev_ch.is_whitespace() || char_is_line_ending(next_ch))
    },
    InternalWordMotionTarget::NextLongWordStart | InternalWordMotionTarget::PrevLongWordEnd => {
      is_long_word_boundary(prev_ch, next_ch)
        && (char_is_line_ending(next_ch) || !next_ch.is_whitespace())
    },
    InternalWordMotionTarget::NextLongWordEnd | InternalWordMotionTarget::PrevLongWordStart => {
      is_long_word_boundary(prev_ch, next_ch)
        && (!prev_ch.is_whitespace() || char_is_line_ending(next_ch))
    },
    InternalWordMotionTarget::NextSubWordStart => {
      is_sub_word_boundary(prev_ch, next_ch, Direction::Forward)
        && (char_is_line_ending(next_ch) || !(next_ch.is_whitespace() || next_ch == '_'))
    },
    InternalWordMotionTarget::PrevSubWordEnd => {
      is_sub_word_boundary(prev_ch, next_ch, Direction::Backward)
        && (char_is_line_ending(next_ch) || !(next_ch.is_whitespace() || next_ch == '_'))
    },
    InternalWordMotionTarget::NextSubWordEnd => {
      is_sub_word_boundary(prev_ch, next_ch, Direction::Forward)
        && (!(prev_ch.is_whitespace() || prev_ch == '_') || char_is_line_ending(next_ch))
    },
    InternalWordMotionTarget::PrevSubWordStart => {
      is_sub_word_boundary(prev_ch, next_ch, Direction::Backward)
        && (!(prev_ch.is_whitespace() || prev_ch == '_') || char_is_line_ending(next_ch))
    },
  }
}

fn line_start(text: RopeSlice, line_idx: usize) -> usize {
  text.line_to_char(line_idx.min(text.len_lines().saturating_sub(1)))
}

fn line_content_end(text: RopeSlice, line_idx: usize) -> usize {
  let line_idx = line_idx.min(text.len_lines().saturating_sub(1));
  let line_start = text.line_to_char(line_idx);
  let line = text.line(line_idx);
  let line_ending_chars = get_line_ending(&line)
    .map(|line_ending| line_ending.len_chars())
    .unwrap_or(0);
  line_start + line.len_chars().saturating_sub(line_ending_chars)
}

fn first_non_whitespace(text: RopeSlice, line_idx: usize) -> usize {
  let line_idx = line_idx.min(text.len_lines().saturating_sub(1));
  let line_start = line_start(text, line_idx);
  let line_end = line_content_end(text, line_idx);

  let mut pos = line_start;
  while pos < line_end {
    let ch = text.char(pos);
    if !ch.is_whitespace() {
      return pos;
    }
    pos += 1;
  }

  line_end
}

fn move_prev_paragraph_range(
  text: RopeSlice,
  range: Range,
  count: usize,
  mode: MotionMode,
) -> Range {
  let mut line = range.cursor_line(text);
  let first_char = text.line_to_char(line) == range.cursor(text);
  let prev_line_empty = rope_is_line_ending(text.line(line.saturating_sub(1)));
  let curr_line_empty = rope_is_line_ending(text.line(line));
  let prev_empty_to_line = prev_line_empty && !curr_line_empty;

  if prev_empty_to_line && !first_char {
    line += 1;
  }
  let mut lines = text.lines_at(line);
  lines.reverse();
  let mut lines = lines.map(rope_is_line_ending).peekable();
  let mut last_line = line;
  for _ in 0..count.max(1) {
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

  let head = text.line_to_char(line);
  let anchor = if mode == MotionMode::Move {
    if prev_empty_to_line && first_char {
      range.cursor(text)
    } else {
      range.head
    }
  } else {
    range.put_cursor(text, head, true).anchor
  };
  Range::new(anchor, head)
}

fn move_next_paragraph_range(
  text: RopeSlice,
  range: Range,
  count: usize,
  mode: MotionMode,
) -> Range {
  let mut line = range.cursor_line(text);
  let last_char = prev_grapheme_boundary(text, text.line_to_char(line + 1)) == range.cursor(text);
  let curr_line_empty = rope_is_line_ending(text.line(line));
  let next_line_empty =
    rope_is_line_ending(text.line(text.len_lines().saturating_sub(1).min(line + 1)));
  let curr_empty_to_line = curr_line_empty && !next_line_empty;

  if curr_empty_to_line && last_char {
    line += 1;
  }
  let mut lines = text.lines_at(line).map(rope_is_line_ending).peekable();
  let mut last_line = line;
  for _ in 0..count.max(1) {
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

  let head = text.line_to_char(line);
  let anchor = if mode == MotionMode::Move {
    if curr_empty_to_line && last_char {
      range.head
    } else {
      range.cursor(text)
    }
  } else {
    range.put_cursor(text, head, true).anchor
  };
  Range::new(anchor, head)
}

#[inline]
pub fn skip_while<F>(text: RopeSlice, pos: usize, predicate: F) -> Option<usize>
where
  F: Fn(char) -> bool,
{
  let mut chars = text.chars_at(pos).enumerate();
  chars.find_map(|(idx, ch)| (!predicate(ch)).then_some(pos + idx))
}

#[inline]
pub fn backwards_skip_while<F>(text: RopeSlice, pos: usize, predicate: F) -> Option<usize>
where
  F: Fn(char) -> bool,
{
  let mut chars_starting_from_next = text.chars_at(pos);
  let mut backwards = iter::from_fn(|| chars_starting_from_next.prev()).enumerate();
  backwards.find_map(|(idx, ch)| (!predicate(ch)).then_some(pos.saturating_sub(idx)))
}

#[cfg(test)]
mod tests {
  use std::num::NonZeroU64;

  use ropey::Rope;
  use smallvec::SmallVec;

  use super::{
    Direction,
    MotionMode,
    MovementContext,
    VisualGoal,
    VisualGoals,
    transform_selection,
  };
  use crate::operation::SelectionTransform;
  use crate::{
    render::{
      TextAnnotations,
      TextFormat,
    },
    selection::{
      CursorId,
      CursorRange,
      Range,
      Selection,
    },
  };

  fn cursor(id: u64, anchor: usize, head: usize) -> CursorRange {
    CursorRange {
      id:    CursorId::new(NonZeroU64::new(id).unwrap()),
      range: Range::new(anchor, head),
    }
  }

  fn selection(cursors: &[(u64, usize, usize)]) -> Selection {
    Selection::from_cursors(
      cursors
        .iter()
        .map(|&(id, anchor, head)| cursor(id, anchor, head))
        .collect::<SmallVec<[CursorRange; 1]>>(),
    )
    .unwrap()
  }

  fn context(format: &TextFormat) -> MovementContext<'_> {
    static ANNOTATIONS: std::sync::LazyLock<TextAnnotations> =
      std::sync::LazyLock::new(TextAnnotations::default);
    MovementContext {
      text_format: format,
      annotations: &ANNOTATIONS,
      wrap_width:  4,
    }
  }

  #[test]
  fn horizontal_motion_is_selection_centric_and_clears_visual_goals() {
    let rope = Rope::from("abcd efgh");
    let selection = selection(&[(1, 1, 1), (2, 5, 5)]);
    let mut visual_goals = VisualGoals::new();
    visual_goals.insert(CursorId::new(NonZeroU64::new(1).unwrap()), VisualGoal {
      col: 9,
    });
    visual_goals.insert(CursorId::new(NonZeroU64::new(2).unwrap()), VisualGoal {
      col: 7,
    });

    let result = transform_selection(
      rope.slice(..),
      selection,
      &visual_goals,
      SelectionTransform::Horizontal {
        direction: Direction::Forward,
        count: 2,
        mode: MotionMode::Move,
      },
      &context(&TextFormat::default()),
    );

    let cursors = result
      .selection
      .iter()
      .map(|cursor| (cursor.id.get(), *cursor.range))
      .collect::<Vec<_>>();
    assert_eq!(cursors, vec![(1, Range::new(3, 3)), (2, Range::new(7, 7))]);
    assert!(result.visual_goals.is_empty());
  }

  #[test]
  fn vertical_motion_updates_per_cursor_visual_goals() {
    let rope = Rope::from("ab\nxyz\nqrst");
    let selection = selection(&[(1, 1, 1), (2, 5, 5)]);

    let result = transform_selection(
      rope.slice(..),
      selection,
      &VisualGoals::new(),
      SelectionTransform::Vertical {
        direction: Direction::Forward,
        count: 1,
        mode: MotionMode::Move,
      },
      &context(&TextFormat::default()),
    );

    let cursors = result
      .selection
      .iter()
      .map(|cursor| (cursor.id.get(), *cursor.range))
      .collect::<Vec<_>>();
    assert_eq!(cursors, vec![(1, Range::new(4, 4)), (2, Range::new(9, 9))]);
    assert_eq!(result.visual_goals.iter().collect::<Vec<_>>(), vec![
      (CursorId::new(NonZeroU64::new(1).unwrap()), VisualGoal {
        col: 1,
      }),
      (CursorId::new(NonZeroU64::new(2).unwrap()), VisualGoal {
        col: 2,
      }),
    ]);
  }

  #[test]
  fn visual_motion_uses_wrapped_rows() {
    let rope = Rope::from("abcd efgh");
    let mut format = TextFormat::default();
    format.soft_wrap = true;
    let selection = selection(&[(1, 1, 1)]);

    let result = transform_selection(
      rope.slice(..),
      selection,
      &VisualGoals::new(),
      SelectionTransform::VisualVertical {
        direction: Direction::Forward,
        count: 1,
        mode: MotionMode::Move,
      },
      &context(&format),
    );

    let cursor = result.selection.get(0).unwrap();
    assert_eq!(*cursor.range, Range::new(4, 4));
    assert_eq!(
      result.visual_goals.get(cursor.id),
      Some(VisualGoal { col: 1 })
    );
  }

  #[test]
  fn visual_up_motion_moves_from_wrapped_continuation_rows() {
    let rope = Rope::from("0123456789abcdef");
    let mut format = TextFormat::default();
    format.soft_wrap = true;

    let result = transform_selection(
      rope.slice(..),
      selection(&[(1, 7, 7)]),
      &VisualGoals::new(),
      SelectionTransform::VisualVertical {
        direction: Direction::Backward,
        count: 1,
        mode: MotionMode::Move,
      },
      &context(&format),
    );

    let cursor = result.selection.get(0).unwrap();
    assert_eq!(*cursor.range, Range::new(3, 3));
    assert_eq!(
      result.visual_goals.get(cursor.id),
      Some(VisualGoal { col: 3 })
    );
  }

  #[test]
  fn word_motion_moves_to_next_word_start() {
    let rope = Rope::from("alpha beta gamma");
    let selection = selection(&[(1, 0, 0)]);

    let result = transform_selection(
      rope.slice(..),
      selection,
      &VisualGoals::new(),
      SelectionTransform::Word {
        direction: Direction::Forward,
        kind: crate::movement::WordKind::Word,
        target: crate::movement::WordTarget::Start,
        count: 2,
        mode: MotionMode::Move,
      },
      &context(&TextFormat::default()),
    );

    assert_eq!(*result.selection.get(0).unwrap().range, Range::new(10, 10));
  }

  #[test]
  fn line_and_document_anchor_motions_use_logical_lines() {
    let rope = Rope::from("  abc\nxyz\n");
    let selection = selection(&[(1, 4, 4)]);

    let first_non_whitespace = transform_selection(
      rope.slice(..),
      selection.clone(),
      &VisualGoals::new(),
      SelectionTransform::FirstNonWhitespace { mode: MotionMode::Move },
      &context(&TextFormat::default()),
    );
    assert_eq!(
      *first_non_whitespace.selection.get(0).unwrap().range,
      Range::new(2, 2)
    );

    let line_end = transform_selection(
      rope.slice(..),
      selection.clone(),
      &VisualGoals::new(),
      SelectionTransform::LineEnd { mode: MotionMode::Move },
      &context(&TextFormat::default()),
    );
    assert_eq!(*line_end.selection.get(0).unwrap().range, Range::new(5, 5));

    let document_end = transform_selection(
      rope.slice(..),
      selection,
      &VisualGoals::new(),
      SelectionTransform::DocumentEnd { mode: MotionMode::Move },
      &context(&TextFormat::default()),
    );
    assert_eq!(
      *document_end.selection.get(0).unwrap().range,
      Range::new(rope.len_chars(), rope.len_chars())
    );
  }
}
