use ropey::{
  Rope,
  RopeSlice,
};
use smallvec::SmallVec;
use thiserror::Error;

use crate::{
  Tendril,
  grapheme,
  movement::Direction,
  operation::Edit,
  selection::{
    CursorRange,
    Range,
    Selection,
    SelectionError,
  },
  transaction::{
    Assoc,
    DeletionOverlapPolicy,
    Replacement,
    ReplacementOverlapPolicy,
    Span,
    Transaction,
    TransactionError,
  },
};

pub const DEFAULT_PAIRS: &[(&str, &str)] = &[
  ("(", ")"),
  ("{", "}"),
  ("[", "]"),
  ("'", "'"),
  ("\"", "\""),
  ("`", "`"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
  pub open:  Tendril,
  pub close: Tendril,
}

#[derive(Debug, Clone)]
pub struct AutoPairs(Vec<Pair>);

pub type Result<T> = std::result::Result<T, AutoPairsError>;

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum AutoPairsError {
  #[error("auto-pair open string must not be empty")]
  EmptyOpenPair,

  #[error("auto-pair close string must not be empty")]
  EmptyClosePair,

  #[error(transparent)]
  Selection(#[from] SelectionError),

  #[error(transparent)]
  Transaction(#[from] TransactionError),
}

impl Pair {
  pub fn same(&self) -> bool {
    self.open == self.close
  }

  pub fn open_len(&self) -> usize {
    self.open.chars().count()
  }

  pub fn close_len(&self) -> usize {
    self.close.chars().count()
  }

  pub fn open_last_char(&self) -> Option<char> {
    self.open.chars().last()
  }

  pub fn close_first_char(&self) -> Option<char> {
    self.close.chars().next()
  }

  pub fn should_close(&self, doc: &Rope, range: &Range) -> bool {
    let cursor = range.head;
    next_is_not_alpha(doc, cursor) && (!self.same() || prev_is_not_alpha(doc, cursor))
  }
}

impl From<(&str, &str)> for Pair {
  fn from((open, close): (&str, &str)) -> Self {
    Self {
      open:  Tendril::from(open),
      close: Tendril::from(close),
    }
  }
}

impl From<(char, char)> for Pair {
  fn from((open, close): (char, char)) -> Self {
    Self {
      open:  open.to_string().into(),
      close: close.to_string().into(),
    }
  }
}

impl From<&(char, char)> for Pair {
  fn from(pair: &(char, char)) -> Self {
    Self::from(*pair)
  }
}

impl From<(&char, &char)> for Pair {
  fn from((open, close): (&char, &char)) -> Self {
    Self::from((*open, *close))
  }
}

impl AutoPairs {
  pub fn new<V, A>(pairs: V) -> Result<Self>
  where
    V: IntoIterator<Item = A>,
    A: Into<Pair>,
  {
    let iter = pairs.into_iter();
    let (lower, upper) = iter.size_hint();
    let mut out = Vec::with_capacity(upper.unwrap_or(lower));

    for pair in iter {
      let pair = pair.into();
      if pair.open.is_empty() {
        return Err(AutoPairsError::EmptyOpenPair);
      }
      if pair.close.is_empty() {
        return Err(AutoPairsError::EmptyClosePair);
      }
      out.push(pair);
    }

    Ok(Self(out))
  }

  pub fn pairs(&self) -> &[Pair] {
    &self.0
  }

  pub fn matches_char(&self, ch: char) -> bool {
    self
      .0
      .iter()
      .any(|pair| pair.open_last_char() == Some(ch) || pair.close_first_char() == Some(ch))
  }
}

impl Default for AutoPairs {
  fn default() -> Self {
    Self::new(DEFAULT_PAIRS.iter().copied()).expect("DEFAULT_PAIRS must be valid")
  }
}

#[derive(Debug, Clone)]
enum CursorPlan {
  Replace {
    replacement: Replacement,
    range:       Range,
    delta:       usize,
  },
  Move {
    range: Range,
  },
}

impl CursorPlan {
  fn range(&self) -> Range {
    match self {
      Self::Replace { range, .. } | Self::Move { range } => *range,
    }
  }

  fn delta(&self) -> usize {
    match self {
      Self::Replace { delta, .. } => *delta,
      Self::Move { .. } => 0,
    }
  }
}

pub fn edit_for_typed_char(
  doc: &Rope,
  selection: &Selection,
  ch: char,
  pairs: &AutoPairs,
) -> Result<Option<Edit>> {
  if !pairs.matches_char(ch) {
    return Ok(None);
  }

  let slice = doc.slice(..);
  let mut plans = Vec::with_capacity(selection.ranges().len());
  let mut offset = 0;

  for cursor in selection.iter() {
    let Some(plan) = plan_typed_char_for_cursor(doc, slice, *cursor.range, ch, pairs, offset)
    else {
      return Ok(None);
    };
    offset += plan.delta();
    plans.push((cursor.id, plan));
  }

  let replacements = plans.iter().filter_map(|(_, plan)| {
    match plan {
      CursorPlan::Replace { replacement, .. } => Some(replacement.clone()),
      CursorPlan::Move { .. } => None,
    }
  });

  let transaction = if plans
    .iter()
    .any(|(_, plan)| matches!(plan, CursorPlan::Replace { .. }))
  {
    Transaction::from_replacements(
      doc.len_chars(),
      replacements,
      ReplacementOverlapPolicy::Reject,
    )?
  } else {
    Transaction::empty(doc.len_chars())
  };

  let selection = selection_from_plans(plans)?;
  Ok(Some(Edit {
    transaction,
    selection,
  }))
}

pub fn edit_for_backspace(
  doc: &Rope,
  selection: &Selection,
  pairs: &AutoPairs,
) -> Result<Option<Edit>> {
  let slice = doc.slice(..);
  let mut spans = Vec::with_capacity(selection.ranges().len());
  let mut cursor_ids: SmallVec<[crate::selection::CursorId; 1]> =
    SmallVec::with_capacity(selection.ranges().len());

  for cursor in selection.iter() {
    if !cursor.range.is_empty() {
      return Ok(None);
    }

    let Some(span) = delete_pair_span(slice, cursor.range.head, pairs) else {
      return Ok(None);
    };

    cursor_ids.push(cursor.id);
    spans.push(span);
  }

  if spans.is_empty() {
    return Ok(None);
  }

  let transaction = Transaction::delete_spans(
    doc.len_chars(),
    spans.clone(),
    DeletionOverlapPolicy::Reject,
  )?;
  let cursors = spans
    .into_iter()
    .zip(cursor_ids)
    .map(|(span, id)| {
      let pos = transaction.changes().map_pos(span.start, Assoc::Before)?;
      Ok(CursorRange {
        id,
        range: Range::point(pos),
      })
    })
    .collect::<std::result::Result<SmallVec<[CursorRange; 1]>, TransactionError>>()?;
  let selection = Selection::from_cursors(cursors)?;
  Ok(Some(Edit {
    transaction,
    selection,
  }))
}

fn plan_typed_char_for_cursor(
  doc: &Rope,
  slice: RopeSlice,
  range: Range,
  ch: char,
  pairs: &AutoPairs,
  offset: usize,
) -> Option<CursorPlan> {
  let cursor = range.head;

  if let Some(pair) = match_close_pair(slice, cursor, ch, pairs) {
    return Some(CursorPlan::Move {
      range: next_range_after_typed_edit(doc, &range, offset, 0, pair.close_len()),
    });
  }

  let pair = match_open_pair(slice, cursor, ch, pairs)?;
  let close = pair.should_close(doc, &range);
  let selection_len = if close { 2 } else { 1 };
  let mut text = Tendril::new();
  text.push(ch);
  if close {
    text.push_str(pair.close.as_str());
  }

  let inserted_len = text.chars().count();
  let replacement = Replacement::insert(cursor, text);
  let range = next_range_after_typed_edit(doc, &range, offset, selection_len, 0);

  Some(CursorPlan::Replace {
    replacement,
    range,
    delta: inserted_len,
  })
}

fn selection_from_plans(plans: Vec<(crate::selection::CursorId, CursorPlan)>) -> Result<Selection> {
  let cursors = plans
    .into_iter()
    .map(|(id, plan)| {
      CursorRange {
        id,
        range: plan.range(),
      }
    })
    .collect();
  Ok(Selection::from_cursors(cursors)?)
}

fn advance_graphemes(slice: RopeSlice, mut pos: usize, count: usize) -> usize {
  for _ in 0..count {
    pos = grapheme::next_grapheme_boundary(slice, pos);
  }
  pos
}

fn next_range_after_typed_edit(
  doc: &Rope,
  start_range: &Range,
  offset: usize,
  selection_len: usize,
  advance: usize,
) -> Range {
  if start_range.head == doc.len_chars() && start_range.anchor == doc.len_chars() {
    return Range::new(
      start_range.anchor + offset + 1,
      start_range.head + offset + 1,
    );
  }

  let slice = doc.slice(..);
  let single_grapheme = start_range.is_single_grapheme(slice);

  if selection_len == 0 {
    let end = advance_graphemes(slice, start_range.head, advance) + offset;
    return Range::point(end);
  }

  if selection_len == 1 {
    if start_range.is_empty() {
      let end = start_range.head + offset + 1;
      return Range::point(end);
    }

    let anchor = if single_grapheme || start_range.direction() == Direction::Backward {
      start_range.anchor + offset + 1
    } else {
      start_range.anchor + offset
    };

    return Range::new(anchor, start_range.head + offset + 1);
  }

  let head = if start_range.head == 0 || start_range.direction() == Direction::Backward {
    start_range.head + offset + 1
  } else {
    grapheme::prev_grapheme_boundary(slice, start_range.head) + offset + selection_len
  };

  let anchor = match (start_range.len(), start_range.direction()) {
    (0, _) => head,
    (1, Direction::Forward) => head - 1,
    (1, Direction::Backward) => head + 1,
    (_, Direction::Forward) => {
      if single_grapheme {
        grapheme::prev_grapheme_boundary(slice, start_range.head) + 1
      } else {
        start_range.anchor + offset
      }
    },
    (_, Direction::Backward) => {
      if single_grapheme {
        grapheme::prev_grapheme_boundary(slice, start_range.anchor) + selection_len + offset
      } else {
        start_range.anchor + offset + selection_len
      }
    },
  };

  Range::new(anchor, head)
}

fn match_open_pair<'a>(
  slice: RopeSlice,
  cursor: usize,
  ch: char,
  pairs: &'a AutoPairs,
) -> Option<&'a Pair> {
  pairs
    .pairs()
    .iter()
    .enumerate()
    .filter(|(_, pair)| pair.open_last_char() == Some(ch))
    .filter(|(_, pair)| matches_open_prefix(slice, cursor, pair))
    .max_by(|(left_idx, left), (right_idx, right)| {
      left
        .open_len()
        .cmp(&right.open_len())
        .then_with(|| right_idx.cmp(left_idx))
    })
    .map(|(_, pair)| pair)
}

fn match_close_pair<'a>(
  slice: RopeSlice,
  cursor: usize,
  ch: char,
  pairs: &'a AutoPairs,
) -> Option<&'a Pair> {
  pairs
    .pairs()
    .iter()
    .enumerate()
    .filter(|(_, pair)| pair.close_first_char() == Some(ch))
    .filter(|(_, pair)| matches_close_at(slice, cursor, pair))
    .max_by(|(left_idx, left), (right_idx, right)| {
      left
        .close_len()
        .cmp(&right.close_len())
        .then_with(|| right_idx.cmp(left_idx))
    })
    .map(|(_, pair)| pair)
}

fn matches_open_prefix(slice: RopeSlice, cursor: usize, pair: &Pair) -> bool {
  let open_len = pair.open_len();
  if open_len == 1 {
    return true;
  }

  let prefix_len = open_len - 1;
  let Some(start) = cursor.checked_sub(prefix_len) else {
    return false;
  };
  matches_chars(slice, start, pair.open.chars().take(prefix_len))
}

fn matches_close_at(slice: RopeSlice, cursor: usize, pair: &Pair) -> bool {
  if cursor + pair.close_len() > slice.len_chars() {
    return false;
  }
  matches_chars(slice, cursor, pair.close.chars())
}

fn matches_chars(slice: RopeSlice, start: usize, expected: impl Iterator<Item = char>) -> bool {
  for (idx, expected) in expected.enumerate() {
    if slice.get_char(start + idx) != Some(expected) {
      return false;
    }
  }
  true
}

fn delete_pair_span(slice: RopeSlice, cursor: usize, pairs: &AutoPairs) -> Option<Span> {
  pairs
    .pairs()
    .iter()
    .enumerate()
    .filter_map(|(idx, pair)| {
      let open_len = pair.open_len();
      let close_len = pair.close_len();
      let start = cursor.checked_sub(open_len)?;
      let end = cursor + close_len;
      if end > slice.len_chars() {
        return None;
      }
      if matches_chars(slice, start, pair.open.chars())
        && matches_chars(slice, cursor, pair.close.chars())
      {
        Some((idx, Span { start, end }))
      } else {
        None
      }
    })
    .max_by(|(left_idx, left), (right_idx, right)| {
      span_len(left)
        .cmp(&span_len(right))
        .then_with(|| right_idx.cmp(left_idx))
    })
    .map(|(_, span)| span)
}

fn next_is_not_alpha(doc: &Rope, cursor: usize) -> bool {
  doc
    .get_char(cursor)
    .map(|ch| !ch.is_alphanumeric())
    .unwrap_or(true)
}

fn prev_is_not_alpha(doc: &Rope, cursor: usize) -> bool {
  cursor == 0
    || doc
      .get_char(cursor - 1)
      .map(|ch| !ch.is_alphanumeric())
      .unwrap_or(true)
}

fn span_len(span: &Span) -> usize {
  span.end - span.start
}

#[cfg(test)]
mod test {
  use smallvec::smallvec;

  use super::*;

  fn pairs() -> AutoPairs {
    AutoPairs::new([
      ("\"\"\"", "\"\"\""),
      ("{%", "%}"),
      ("{{", "}}"),
      ("\"", "\""),
      ("(", ")"),
    ])
    .unwrap()
  }

  fn apply(doc: &Rope, edit: &Edit) -> String {
    edit.transaction.apply_to(doc).unwrap().to_string()
  }

  fn heads(selection: &Selection) -> Vec<usize> {
    selection.ranges().iter().map(|range| range.head).collect()
  }

  #[test]
  fn rejects_empty_pairs() {
    assert!(matches!(
      AutoPairs::new([("", ")")]),
      Err(AutoPairsError::EmptyOpenPair)
    ));
    assert!(matches!(
      AutoPairs::new([("(", "")]),
      Err(AutoPairsError::EmptyClosePair)
    ));
  }

  #[test]
  fn inserts_single_char_pair_and_selection() {
    let doc = Rope::from("");
    let edit = edit_for_typed_char(&doc, &Selection::point(0), '(', &AutoPairs::default())
      .unwrap()
      .unwrap();
    assert_eq!(apply(&doc, &edit), "()");
    assert_eq!(edit.selection.ranges(), &[Range::point(1)]);
  }

  #[test]
  fn quote_heuristic_inside_and_outside_words() {
    let pairs = AutoPairs::default();

    let doc = Rope::from("hello ");
    let edit = edit_for_typed_char(&doc, &Selection::point(6), '"', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply(&doc, &edit), "hello \"\"");

    let doc = Rope::from("hello");
    let edit = edit_for_typed_char(&doc, &Selection::point(2), '"', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply(&doc, &edit), "he\"llo");
    assert_eq!(edit.selection.ranges(), &[Range::point(3)]);

    let doc = Rope::from("ab");
    let edit = edit_for_typed_char(&doc, &Selection::point(1), '(', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply(&doc, &edit), "a(b");
    assert_eq!(edit.selection.ranges(), &[Range::point(2)]);
  }

  #[test]
  fn skips_existing_closer() {
    let doc = Rope::from("()");
    let edit = edit_for_typed_char(&doc, &Selection::point(1), ')', &AutoPairs::default())
      .unwrap()
      .unwrap();
    assert_eq!(apply(&doc, &edit), "()");
    assert_eq!(edit.selection.ranges(), &[Range::point(2)]);
  }

  #[test]
  fn multi_char_pairs_insert_skip_and_backspace() {
    let pairs = pairs();

    let doc = Rope::from("{");
    let edit = edit_for_typed_char(&doc, &Selection::point(1), '%', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply(&doc, &edit), "{%%}");
    assert_eq!(edit.selection.ranges(), &[Range::point(2)]);

    let doc = Rope::from("{%%}");
    let edit = edit_for_typed_char(&doc, &Selection::point(2), '%', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply(&doc, &edit), "{%%}");
    assert_eq!(edit.selection.ranges(), &[Range::point(4)]);

    let edit = edit_for_backspace(&doc, &Selection::point(2), &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply(&doc, &edit), "");
    assert_eq!(edit.selection.ranges(), &[Range::point(0)]);
  }

  #[test]
  fn longest_match_wins_and_config_order_breaks_ties() {
    let pairs = pairs();
    let doc = Rope::from("\"\"");
    let edit = edit_for_typed_char(&doc, &Selection::point(2), '"', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply(&doc, &edit), "\"\"\"\"\"\"");

    let ordered = AutoPairs::new([("ax", "yz"), ("ax", "yw")]).unwrap();
    let doc = Rope::from("a");
    let edit = edit_for_typed_char(&doc, &Selection::point(1), 'x', &ordered)
      .unwrap()
      .unwrap();
    assert_eq!(apply(&doc, &edit), "axyz");
    assert_eq!(edit.selection.ranges(), &[Range::point(2)]);
  }

  #[test]
  fn multi_cursor_is_all_or_nothing_and_preserves_ids() {
    let pairs = AutoPairs::default();
    let doc = Rope::from("a b");
    let selection = Selection::new(smallvec![Range::point(1), Range::point(3)]).unwrap();
    let ids: Vec<_> = selection.cursor_ids().to_vec();

    let edit = edit_for_typed_char(&doc, &selection, '(', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply(&doc, &edit), "a() b()");
    assert_eq!(heads(&edit.selection), vec![2, 6]);
    assert_eq!(edit.selection.cursor_ids(), ids.as_slice());

    let doc = Rope::from("() ()");
    let selection = Selection::new(smallvec![Range::point(1), Range::point(4)]).unwrap();
    let ids: Vec<_> = selection.cursor_ids().to_vec();
    let edit = edit_for_backspace(&doc, &selection, &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply(&doc, &edit), " ");
    assert_eq!(heads(&edit.selection), vec![0, 1]);
    assert_eq!(edit.selection.cursor_ids(), ids.as_slice());

    let doc = Rope::from("() x");
    let selection = Selection::new(smallvec![Range::point(1), Range::point(4)]).unwrap();
    assert!(
      edit_for_backspace(&doc, &selection, &pairs)
        .unwrap()
        .is_none()
    );
  }

  #[test]
  fn non_empty_selection_uses_insertion_edge_not_surround() {
    let doc = Rope::from("printf");
    let selection = Selection::single(0, 6);
    let edit = edit_for_typed_char(&doc, &selection, '[', &AutoPairs::default())
      .unwrap()
      .unwrap();
    assert_eq!(apply(&doc, &edit), "printf[]");
    assert_eq!(edit.selection.ranges(), &[Range::new(0, 7)]);
  }

  #[test]
  fn crlf_single_grapheme_selection_preserves_old_backward_landing_behavior() {
    let doc = Rope::from("foo\r\n");
    let selection = Selection::single(5, 3);
    let pairs = AutoPairs::default();

    let edit = edit_for_typed_char(&doc, &selection, '(', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply(&doc, &edit), "foo()\r\n");
    assert_eq!(edit.selection.ranges(), &[Range::new(5, 4)]);

    let edit = edit_for_typed_char(&doc, &selection, '\'', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply(&doc, &edit), "foo'\r\n");
    assert_eq!(edit.selection.ranges(), &[Range::new(6, 4)]);
  }

  #[test]
  fn irrelevant_char_declines() {
    let doc = Rope::from("");
    assert!(
      edit_for_typed_char(&doc, &Selection::point(0), 'a', &AutoPairs::default())
        .unwrap()
        .is_none()
    );
  }

  #[test]
  fn overlapping_backspace_pair_spans_return_an_error_instead_of_panicking() {
    let doc = Rope::from("\"\"\"\"\"\"");
    let selection = Selection::new(smallvec![Range::point(1), Range::point(3)]).unwrap();
    let pairs = AutoPairs::new([("\"\"\"", "\"\"\""), ("\"", "\"")]).unwrap();

    assert!(matches!(
      edit_for_backspace(&doc, &selection, &pairs),
      Err(AutoPairsError::Transaction(
        TransactionError::OverlappingReplacement { .. }
      ))
    ));
  }
}
