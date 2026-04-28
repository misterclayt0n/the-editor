use ropey::RopeSlice;
use the_stdx::rope::{
  Config,
  Regex,
  RegexBuilder,
  RopeSliceExt,
};

use crate::{
  grapheme::{
    ensure_grapheme_boundary_next,
    ensure_grapheme_boundary_prev,
  },
  movement::{
    Direction,
    Movement,
  },
  selection::{
    CursorPick,
    Range,
    Selection,
  },
};

/// Trait for matching characters during search operations.
///
/// Note: `std::str::Pattern` (rust-lang/rust#27721) is designed for `&str`, not
/// `RopeSlice`, so even when stabilized it won't directly apply here. This
/// trait serves the same purpose for rope-based text and could be extended with
/// additional matchers if needed.
pub trait CharMatcher {
  fn char_match(&mut self, ch: char) -> bool;
}

impl CharMatcher for char {
  fn char_match(&mut self, ch: char) -> bool {
    *self == ch
  }
}

impl<F: FnMut(char) -> bool> CharMatcher for F {
  fn char_match(&mut self, ch: char) -> bool {
    (*self)(ch)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDirection {
  Next,
  Prev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchStart {
  /// Include the first candidate in the given direction.
  Inclusive,
  /// Skip the first candidate in the given direction.
  Exclusive,
}

/// Find the nth match in the given direction starting from `pos`.
///
/// `n` is 1-based; `n == 0` returns `None`.
pub fn find_nth<M: CharMatcher>(
  text: RopeSlice,
  mut matcher: M,
  pos: usize,
  n: usize,
  direction: SearchDirection,
  start: SearchStart,
) -> Option<usize> {
  if n == 0 {
    return None;
  }

  match direction {
    SearchDirection::Next => find_nth_next_impl(text, &mut matcher, pos, n, start),
    SearchDirection::Prev => find_nth_prev_impl(text, &mut matcher, pos, n, start),
  }
}

/// Find the nth match forward from `pos`, including the character at `pos`.
///
/// `n` is 1-based; `n == 0` returns `None`.
pub fn find_nth_next<M: CharMatcher>(
  text: RopeSlice,
  matcher: M,
  pos: usize,
  n: usize,
) -> Option<usize> {
  find_nth(
    text,
    matcher,
    pos,
    n,
    SearchDirection::Next,
    SearchStart::Inclusive,
  )
}

/// Find the nth match backward from `pos`, including the character before or at
/// `pos` according to the chosen start mode.
///
/// `n` is 1-based; `n == 0` returns `None`.
pub fn find_nth_prev<M: CharMatcher>(
  text: RopeSlice,
  matcher: M,
  pos: usize,
  n: usize,
) -> Option<usize> {
  find_nth(
    text,
    matcher,
    pos,
    n,
    SearchDirection::Prev,
    SearchStart::Inclusive,
  )
}

/// Build a regex for interactive search with optional smart-case.
///
/// If `smart_case` is true, the regex is case-insensitive unless the query
/// contains an uppercase letter.
pub fn build_regex(query: &str, smart_case: bool) -> Result<Regex, String> {
  let case_insensitive = smart_case && !query.chars().any(char::is_uppercase);
  RegexBuilder::new()
    .syntax(
      Config::new()
        .case_insensitive(case_insensitive)
        .multi_line(true),
    )
    .build(query)
    .map_err(|err| err.to_string())
}

/// Find one regex match and return an updated selection.
///
/// Uses `pick` to choose which cursor/range acts as the anchor. If no match is
/// found, returns `None`.
pub fn search_regex(
  text: RopeSlice,
  selection: &Selection,
  pick: CursorPick,
  regex: &Regex,
  movement: Movement,
  direction: Direction,
  wrap_around: bool,
) -> Option<Selection> {
  let picked = selection.pick(pick).ok()?;
  let search_start = regex_search_start(text, picked.range, direction);
  let matched_range = regex_match_range(text, regex, search_start, direction, wrap_around)?;
  let idx = selection.index_of(picked.id)?;

  match movement {
    Movement::Move => selection.clone().replace(idx, matched_range).ok(),
    Movement::Extend => Some(selection.clone().push(matched_range)),
  }
}

fn find_nth_next_impl<M: CharMatcher>(
  text: RopeSlice,
  matcher: &mut M,
  mut pos: usize,
  n: usize,
  start: SearchStart,
) -> Option<usize> {
  let len = text.len_chars();
  if pos > len || pos >= len {
    return None;
  }

  let mut chars = text.chars_at(pos);
  if start == SearchStart::Exclusive {
    let _ = chars.next()?;
    pos += 1;
  }

  for _ in 0..n {
    loop {
      let ch = chars.next()?;
      pos += 1;
      if matcher.char_match(ch) {
        break;
      }
    }
  }

  Some(pos - 1)
}

fn find_nth_prev_impl<M: CharMatcher>(
  text: RopeSlice,
  matcher: &mut M,
  mut pos: usize,
  n: usize,
  start: SearchStart,
) -> Option<usize> {
  let len = text.len_chars();
  if pos > len || pos == 0 {
    return None;
  }

  let mut chars = text.chars_at(pos);
  if start == SearchStart::Exclusive {
    let _ = chars.prev()?;
    pos -= 1;
  }

  for _ in 0..n {
    loop {
      let ch = chars.prev()?;
      pos -= 1;
      if matcher.char_match(ch) {
        break;
      }
    }
  }

  Some(pos)
}

fn regex_search_start(text: RopeSlice, range: Range, direction: Direction) -> usize {
  match direction {
    Direction::Forward => text.char_to_byte(ensure_grapheme_boundary_next(text, range.to())),
    Direction::Backward => text.char_to_byte(ensure_grapheme_boundary_prev(text, range.from())),
  }
}

fn regex_match_range(
  text: RopeSlice,
  regex: &Regex,
  byte_start: usize,
  direction: Direction,
  wrap_around: bool,
) -> Option<Range> {
  let (start, end) = find_regex_match(text, regex, byte_start, direction).or_else(|| {
    wrap_around
      .then(|| find_wrapped_regex_match(text, regex, byte_start, direction))
      .flatten()
  })?;

  let start = text.byte_to_char(start);
  let end = text.byte_to_char(end);
  (end > 0).then(|| Range::new(start, end))
}

fn find_regex_match(
  text: RopeSlice,
  regex: &Regex,
  byte_start: usize,
  direction: Direction,
) -> Option<(usize, usize)> {
  match direction {
    Direction::Forward => {
      regex
        .find(text.regex_input_at_bytes(byte_start..))
        .map(|mat| (mat.start(), mat.end()))
    },
    Direction::Backward => {
      regex
        .find_iter(text.regex_input_at_bytes(..byte_start))
        .last()
        .map(|mat| (mat.start(), mat.end()))
    },
  }
}

fn find_wrapped_regex_match(
  text: RopeSlice,
  regex: &Regex,
  byte_start: usize,
  direction: Direction,
) -> Option<(usize, usize)> {
  match direction {
    Direction::Forward => {
      regex
        .find(text.regex_input())
        .map(|mat| (mat.start(), mat.end()))
    },
    Direction::Backward => {
      regex
        .find_iter(text.regex_input_at_bytes(byte_start..))
        .last()
        .map(|mat| (mat.start(), mat.end()))
    },
  }
}

#[cfg(test)]
mod test {
  use ropey::Rope;
  use smallvec::smallvec;

  use super::*;
  use crate::selection::{
    CursorRange,
    Range,
    Selection,
  };

  fn cursor(id: u64, anchor: usize, head: usize) -> CursorRange {
    use std::num::NonZeroU64;
    crate::selection::CursorRange {
      id:    crate::selection::CursorId::new(NonZeroU64::new(id).unwrap()),
      range: Range::new(anchor, head),
    }
  }

  #[test]
  fn find_nth_searches_forward_and_backward() {
    let text = Rope::from("hello world");
    let slice = text.slice(..);

    assert_eq!(find_nth_next(slice, 'o', 0, 1), Some(4));
    assert_eq!(find_nth_next(slice, 'o', 0, 2), Some(7));
    assert_eq!(find_nth_prev(slice, 'o', 11, 1), Some(7));
    assert_eq!(find_nth_prev(slice, 'o', 11, 2), Some(4));
  }

  #[test]
  fn find_nth_supports_predicates_and_inclusive_or_exclusive_start() {
    let text = Rope::from("aaa b");
    let slice = text.slice(..);

    assert_eq!(
      find_nth(
        slice,
        |c: char| c.is_whitespace(),
        1,
        1,
        SearchDirection::Next,
        SearchStart::Inclusive,
      ),
      Some(3)
    );
    assert_eq!(
      find_nth(
        slice,
        'a',
        1,
        1,
        SearchDirection::Next,
        SearchStart::Inclusive
      ),
      Some(1)
    );
    assert_eq!(
      find_nth(
        slice,
        'a',
        1,
        1,
        SearchDirection::Next,
        SearchStart::Exclusive
      ),
      Some(2)
    );
  }

  #[test]
  fn find_nth_returns_none_for_invalid_requests() {
    let text = Rope::from("hello");
    let slice = text.slice(..);

    assert_eq!(find_nth_next(slice, 'e', 0, 0), None);
    assert_eq!(find_nth_prev(slice, 'e', 5, 0), None);
    assert_eq!(find_nth_next(slice, 'o', 5, 1), None);
    assert_eq!(find_nth_prev(slice, 'h', 0, 1), None);
    assert_eq!(find_nth_next(slice, 'l', 0, 3), None);
    assert_eq!(find_nth_next(slice, 'h', 6, 1), None);
  }

  #[test]
  fn build_regex_uses_smart_case() {
    let regex = build_regex("abc", true).unwrap();
    let uppercase = Rope::from("ABC");
    assert!(regex.find(uppercase.slice(..).regex_input()).is_some());

    let regex = build_regex("Abc", true).unwrap();
    let lowercase = Rope::from("abc");
    assert!(regex.find(lowercase.slice(..).regex_input()).is_none());
  }

  #[test]
  fn search_regex_wraps_forward_and_backward() {
    let text = Rope::from("abc abc");
    let regex = build_regex("abc", true).unwrap();

    let forward = search_regex(
      text.slice(..),
      &Selection::point(7),
      CursorPick::First,
      &regex,
      Movement::Move,
      Direction::Forward,
      true,
    )
    .unwrap();
    assert_eq!(forward.ranges(), &[Range::new(0, 3)]);

    let backward = search_regex(
      text.slice(..),
      &Selection::point(0),
      CursorPick::First,
      &regex,
      Movement::Move,
      Direction::Backward,
      true,
    )
    .unwrap();
    assert_eq!(backward.ranges(), &[Range::new(4, 7)]);
  }

  #[test]
  fn search_regex_updates_only_the_picked_cursor() {
    let text = Rope::from("abc abc");
    let regex = build_regex("abc", true).unwrap();
    let selection = Selection::from_cursors(smallvec![cursor(1, 0, 0), cursor(2, 4, 4)]).unwrap();

    let next = search_regex(
      text.slice(..),
      &selection,
      CursorPick::Last,
      &regex,
      Movement::Move,
      Direction::Forward,
      true,
    )
    .unwrap();

    assert_eq!(
      next.range_by_id(cursor(1, 0, 0).id),
      Some(&Range::new(0, 0))
    );
    assert_eq!(
      next.range_by_id(cursor(2, 4, 4).id),
      Some(&Range::new(4, 7))
    );
  }

  #[test]
  fn search_regex_extend_adds_a_new_match_range() {
    let text = Rope::from("abc abc");
    let regex = build_regex("abc", true).unwrap();
    let selection = Selection::point(3);

    let next = search_regex(
      text.slice(..),
      &selection,
      CursorPick::First,
      &regex,
      Movement::Extend,
      Direction::Forward,
      true,
    )
    .unwrap();

    assert_eq!(next.ranges(), &[Range::new(3, 3), Range::new(4, 7)]);
  }

  #[test]
  fn search_regex_returns_none_when_no_match_exists_and_wrap_is_disabled() {
    let text = Rope::from("abc abc");
    let regex = build_regex("zzz", true).unwrap();

    assert_eq!(
      search_regex(
        text.slice(..),
        &Selection::point(0),
        CursorPick::First,
        &regex,
        Movement::Move,
        Direction::Forward,
        false,
      ),
      None
    );
  }
}
