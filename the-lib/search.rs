//! Single-character search within a rope slice.
//!
//! This module provides character-level search functionality for navigating
//! within text. It supports both forward and backward searching with
//! configurable start behavior (inclusive or exclusive).
//!
//! # Overview
//!
//! The primary function is [`find_nth`], which searches for the nth occurrence
//! of a character (or character matching a predicate) in either direction.
//! Convenience wrappers [`find_nth_next`] and [`find_nth_prev`] provide simpler
//! APIs for common cases.
//!
//! # Character Matching
//!
//! Search targets are specified via the [`CharMatcher`] trait, which is
//! implemented for:
//! - `char` - matches a specific character
//! - `FnMut(char) -> bool` - matches characters satisfying a predicate
//!
//! # Examples
//!
//! ```ignore
//! use ropey::Rope;
//! use the_lib::search::{find_nth_next, find_nth_prev};
//!
//! let text = Rope::from("hello world");
//! let slice = text.slice(..);
//!
//! // Find the first 'o' from the start
//! assert_eq!(find_nth_next(slice, 'o', 0, 1), Some(4));
//!
//! // Find the second 'o' from the start
//! assert_eq!(find_nth_next(slice, 'o', 0, 2), Some(7));
//!
//! // Find first vowel using a predicate
//! assert_eq!(find_nth_next(slice, |c| "aeiou".contains(c), 0, 1), Some(1));
//!
//! // Search backwards from end
//! assert_eq!(find_nth_prev(slice, 'o', 11, 1), Some(7));
//! ```
//!
//! # Design Notes
//!
//! - **1-based count**: The `n` parameter is 1-based; `n == 0` always returns
//!   `None`
//! - **Position semantics**: Positions are character indices (not byte indices)
//! - **Inclusive vs Exclusive**: Controls whether the character at `pos` is
//!   considered as a candidate in the search

use ropey::RopeSlice;
use the_core::grapheme::{
  ensure_grapheme_boundary_next,
  ensure_grapheme_boundary_prev,
};
use the_stdx::rope::{
  Config,
  Regex,
  RegexBuilder,
  RopeSliceExt,
};

use crate::{
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
/// additional matchers (e.g., `CharSet`, `CaseInsensitive`) if needed.
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
  mut char_matcher: M,
  mut pos: usize,
  n: usize,
  direction: SearchDirection,
  start: SearchStart,
) -> Option<usize> {
  let len = text.len_chars();
  if n == 0 || pos > len {
    return None;
  }

  match direction {
    SearchDirection::Next => {
      if pos >= len {
        return None;
      }

      let mut chars = text.chars_at(pos);
      if start == SearchStart::Exclusive {
        let _ = chars.next()?;
        pos += 1;
      }

      for _ in 0..n {
        loop {
          let c = chars.next()?;
          pos += 1;
          if char_matcher.char_match(c) {
            break;
          }
        }
      }

      Some(pos - 1)
    },
    SearchDirection::Prev => {
      if pos == 0 {
        return None;
      }

      let mut chars = text.chars_at(pos);
      if start == SearchStart::Exclusive {
        let _ = chars.prev()?;
        pos -= 1;
      }

      for _ in 0..n {
        loop {
          let c = chars.prev()?;
          pos -= 1;
          if char_matcher.char_match(c) {
            break;
          }
        }
      }

      Some(pos)
    },
  }
}

/// Find the nth match forward from `pos`, including the character at `pos`.
///
/// `n` is 1-based; `n == 0` returns `None`.
pub fn find_nth_next<M: CharMatcher>(
  text: RopeSlice,
  char_matcher: M,
  pos: usize,
  n: usize,
) -> Option<usize> {
  find_nth(
    text,
    char_matcher,
    pos,
    n,
    SearchDirection::Next,
    SearchStart::Inclusive,
  )
}

/// Find the nth match backward from `pos`, starting before `pos`.
///
/// `n` is 1-based; `n == 0` returns `None`.
pub fn find_nth_prev<M: CharMatcher>(
  text: RopeSlice,
  char_matcher: M,
  pos: usize,
  n: usize,
) -> Option<usize> {
  find_nth(
    text,
    char_matcher,
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

/// Find the next regex match and return an updated selection.
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
  let (cursor_id, primary) = selection.pick(pick).ok()?;
  let idx = selection.index_of(cursor_id)?;

  let start = match direction {
    Direction::Forward => text.char_to_byte(ensure_grapheme_boundary_next(text, primary.to())),
    Direction::Backward => text.char_to_byte(ensure_grapheme_boundary_prev(text, primary.from())),
  };

  let mut mat = match direction {
    Direction::Forward => regex.find(text.regex_input_at_bytes(start..)),
    Direction::Backward => regex.find_iter(text.regex_input_at_bytes(..start)).last(),
  };

  if mat.is_none() && wrap_around {
    mat = match direction {
      Direction::Forward => regex.find(text.regex_input()),
      Direction::Backward => regex.find_iter(text.regex_input_at_bytes(start..)).last(),
    };
  }

  let mat = mat?;
  let start = text.byte_to_char(mat.start());
  let end = text.byte_to_char(mat.end());
  if end == 0 {
    return None;
  }

  let range = Range::new(start, end).with_direction(primary.direction());
  let next = match movement {
    Movement::Extend => selection.clone().push(range),
    Movement::Move => selection.clone().replace(idx, range).ok()?,
  };
  Some(next)
}

#[cfg(test)]
mod test {
  use ropey::Rope;
  use smallvec::smallvec;

  use super::*;
  use crate::selection::{
    Range,
    Selection,
  };

  #[test]
  fn find_next_char() {
    let text = Rope::from("hello world");
    let slice = text.slice(..);

    // Find first 'o' from start
    assert_eq!(find_nth_next(slice, 'o', 0, 1), Some(4));
    // Find second 'o' from start
    assert_eq!(find_nth_next(slice, 'o', 0, 2), Some(7));
    // Find 'o' starting after the first one
    assert_eq!(find_nth_next(slice, 'o', 5, 1), Some(7));
    // Character not found
    assert_eq!(find_nth_next(slice, 'z', 0, 1), None);
  }

  #[test]
  fn find_prev_char() {
    let text = Rope::from("hello world");
    let slice = text.slice(..);

    // Find first 'o' backwards from end
    assert_eq!(find_nth_prev(slice, 'o', 11, 1), Some(7));
    // Find second 'o' backwards from end
    assert_eq!(find_nth_prev(slice, 'o', 11, 2), Some(4));
    // Find 'o' backwards from position 6
    assert_eq!(find_nth_prev(slice, 'o', 6, 1), Some(4));
    // Character not found
    assert_eq!(find_nth_prev(slice, 'z', 11, 1), None);
  }

  #[test]
  fn find_with_predicate() {
    let text = Rope::from("hello world");
    let slice = text.slice(..);

    // Find first whitespace forward
    assert_eq!(
      find_nth_next(slice, |c: char| c.is_whitespace(), 0, 1),
      Some(5)
    );
    // Find first vowel forward
    assert_eq!(
      find_nth_next(slice, |c: char| "aeiou".contains(c), 0, 1),
      Some(1)
    );
    // Find first whitespace backward
    assert_eq!(
      find_nth_prev(slice, |c: char| c.is_whitespace(), 11, 1),
      Some(5)
    );
  }

  #[test]
  fn edge_cases() {
    let text = Rope::from("hello");
    let slice = text.slice(..);

    // n=0 always returns None
    assert_eq!(find_nth_next(slice, 'e', 0, 0), None);
    assert_eq!(find_nth_prev(slice, 'e', 5, 0), None);
    // pos at end for forward search
    assert_eq!(find_nth_next(slice, 'o', 5, 1), None);
    // pos at start for backward search
    assert_eq!(find_nth_prev(slice, 'h', 0, 1), None);
    // Asking for more matches than exist
    assert_eq!(find_nth_next(slice, 'l', 0, 3), None);
  }

  #[test]
  fn inclusive_vs_exclusive() {
    let text = Rope::from("aaa");
    let slice = text.slice(..);

    // Inclusive: includes char at pos
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
    // Exclusive: skips char at pos
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
  fn regex_build_smart_case() {
    let regex = build_regex("abc", true).unwrap();
    let uppercase = Rope::from("ABC");
    let text = uppercase.slice(..);
    assert!(regex.find(text.regex_input()).is_some());

    let regex = build_regex("Abc", true).unwrap();
    let lowercase = Rope::from("abc");
    let text = lowercase.slice(..);
    assert!(regex.find(text.regex_input()).is_none());
  }

  #[test]
  fn regex_search_wrap() {
    let text = Rope::from("abc abc");
    let slice = text.slice(..);
    let regex = build_regex("abc", true).unwrap();
    let selection = Selection::point(7);

    let next = search_regex(
      slice,
      &selection,
      CursorPick::First,
      &regex,
      Movement::Move,
      Direction::Forward,
      true,
    )
    .unwrap();
    assert_eq!(next.ranges()[0].from(), 0);
    assert_eq!(next.ranges()[0].to(), 3);
  }

  #[test]
  fn regex_search_backward() {
    let text = Rope::from("abc abc");
    let slice = text.slice(..);
    let regex = build_regex("abc", true).unwrap();
    let selection = Selection::point(0);

    let next = search_regex(
      slice,
      &selection,
      CursorPick::First,
      &regex,
      Movement::Move,
      Direction::Backward,
      true,
    )
    .unwrap();
    assert_eq!(next.ranges()[0].from(), 4);
    assert_eq!(next.ranges()[0].to(), 7);
  }

  #[test]
  fn regex_search_updates_picked_range() {
    let text = Rope::from("abc abc");
    let slice = text.slice(..);
    let regex = build_regex("abc", true).unwrap();
    let selection = Selection::new(smallvec![Range::point(0), Range::point(4)]).unwrap();

    let next = search_regex(
      slice,
      &selection,
      CursorPick::Last,
      &regex,
      Movement::Move,
      Direction::Forward,
      true,
    )
    .unwrap();
    assert_eq!(next.ranges()[0].from(), 0);
    assert_eq!(next.ranges()[0].to(), 0);
    assert_eq!(next.ranges()[1].from(), 4);
    assert_eq!(next.ranges()[1].to(), 7);
  }
}
