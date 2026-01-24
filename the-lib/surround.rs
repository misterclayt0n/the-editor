//! Surround pair detection for selections.
//!
//! This module computes the positions of surrounding pairs (brackets or
//! quotes) around selections and provides helpers for stepping outward.
//! It supports syntax-aware matching when a `Syntax` is available and falls
//! back to plaintext scanning otherwise.
//!
//! # Example (plaintext)
//!
//! ```
//! use ropey::Rope;
//! use the_lib::selection::{Range, Selection};
//! use the_lib::surround::get_surround_pos;
//! use smallvec::smallvec;
//!
//! let text = Rope::from("(hello) world");
//! let selection = Selection::new(smallvec![Range::point(2)], 0).unwrap();
//! let positions = get_surround_pos(None, text.slice(..), &selection, Some('('), 1).unwrap();
//! assert_eq!(positions.as_slice(), &[(0, 6)]);
//! ```
use std::collections::HashSet;

use ropey::RopeSlice;
use smallvec::SmallVec;
use the_core::grapheme::next_grapheme_boundary;
use thiserror::Error;

use crate::{
  match_brackets::{
    find_matching_bracket, find_matching_bracket_fuzzy, get_pair, is_close_bracket, is_open_bracket,
  },
  movement::Direction,
  search,
  selection::{Range, Selection},
  syntax::Syntax,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
  #[error("surround pair not found around all cursors")]
  PairNotFound,
  #[error("cursors overlap for a single surround pair range")]
  CursorOverlap,
  #[error("cursor range exceeds text length (to={to}, len={len})")]
  RangeExceedsText { to: usize, len: usize },
  #[error("cursor on ambiguous surround pair '{ch}' at {pos}")]
  CursorOnAmbiguousPair { ch: char, pos: usize },
}

type Result<T> = std::result::Result<T, Error>;

pub type SurroundPair = (usize, usize);

/// Finds the position of surround pairs of any
/// [`crate::match_brackets::BRACKETS`] using tree-sitter when possible.
///
/// # Returns
///
/// Tuple `(anchor, head)`, meaning it is not always ordered.
pub fn find_nth_closest_pairs_pos(
  syntax: Option<&Syntax>,
  text: RopeSlice,
  range: Range,
  skip: usize,
) -> Result<SurroundPair> {
  match syntax {
    Some(syntax) => find_nth_closest_pairs_ts(syntax, text, range, skip),
    None => find_nth_closest_pairs_plain(text, range, skip),
  }
}

fn find_nth_closest_pairs_ts(
  syntax: &Syntax,
  text: RopeSlice,
  range: Range,
  mut skip: usize,
) -> Result<SurroundPair> {
  let mut opening = range.from();
  // We want to expand the selection if we are already on the found pair,
  // otherwise we would need to subtract "-1" from "range.to()".
  let mut closing = range.to();

  while skip > 0 {
    closing = find_matching_bracket_fuzzy(syntax, text, closing).ok_or(Error::PairNotFound)?;
    opening = find_matching_bracket(syntax, text, closing).ok_or(Error::PairNotFound)?;
    // If we're already on a closing bracket "find_matching_bracket_fuzzy" will
    // return the position of the opening bracket.
    if closing < opening {
      (opening, closing) = (closing, opening);
    }

    // In case found brackets are partially inside current selection.
    if range.from() < opening || closing < range.to().saturating_sub(1) {
      closing = next_grapheme_boundary(text, closing);
    } else {
      skip -= 1;
      if skip != 0 {
        closing = next_grapheme_boundary(text, closing);
      }
    }
  }

  // Keep the original direction.
  if let Direction::Forward = range.direction() {
    Ok((opening, closing))
  } else {
    Ok((closing, opening))
  }
}

fn find_nth_closest_pairs_plain(
  text: RopeSlice,
  range: Range,
  mut skip: usize,
) -> Result<SurroundPair> {
  let mut stack = Vec::with_capacity(2);
  let pos = range.from();
  let mut close_pos = pos.saturating_sub(1);

  for ch in text.chars_at(pos) {
    close_pos += 1;

    if is_open_bracket(ch) {
      // Track open pairs encountered so that we can step over
      // the corresponding close pairs that will come up further
      // down the loop. We want to find a lone close pair whose
      // open pair is before the cursor position.
      stack.push(ch);
      continue;
    }

    if !is_close_bracket(ch) {
      // We don't care if this character isn't a brace pair item,
      // so short circuit here.
      continue;
    }

    let (open, close) = get_pair(ch);

    if stack.last() == Some(&open) {
      // If we are encountering the closing pair for an opener
      // we just found while traversing, then its inside the
      // selection and should be skipped over.
      stack.pop();
      continue;
    }

    match find_nth_open_pair(text, open, close, close_pos, 1) {
      // Before we accept this pair, we want to ensure that the
      // pair encloses the range rather than just the cursor.
      Some(open_pos)
        if open_pos <= pos.saturating_add(1) && close_pos >= range.to().saturating_sub(1) =>
      {
        // Since we have special conditions for when to
        // accept, we can't just pass the skip parameter on
        // through to the find_nth_*_pair methods, so we
        // track skips manually here.
        if skip > 1 {
          skip -= 1;
          continue;
        }

        return match range.direction() {
          Direction::Forward => Ok((open_pos, close_pos)),
          Direction::Backward => Ok((close_pos, open_pos)),
        };
      },
      _ => continue,
    }
  }

  Err(Error::PairNotFound)
}

/// Find the position of surround pairs of `ch` which can be either a closing
/// or opening pair. `n` will skip n - 1 pairs (eg. n=2 will discard (only)
/// the first pair found and keep looking)
pub fn find_nth_pairs_pos(
  text: RopeSlice,
  ch: char,
  range: Range,
  n: usize,
) -> Result<SurroundPair> {
  if text.len_chars() < 2 {
    return Err(Error::PairNotFound);
  }
  let len = text.len_chars();
  if range.to() > len {
    return Err(Error::RangeExceedsText {
      to: range.to(),
      len,
    });
  }

  let (open, close) = get_pair(ch);
  let pos = range.cursor(text);

  let (open, close) = if open == close {
    if Some(open) == text.get_char(pos) {
      // Cursor is directly on match char. We return no match
      // because there's no way to know which side of the char
      // we should be searching on.
      return Err(Error::CursorOnAmbiguousPair { ch: open, pos });
    }
    (
      search::find_nth_prev(text, open, pos, n),
      search::find_nth_next(text, close, pos, n),
    )
  } else {
    (
      find_nth_open_pair(text, open, close, pos, n),
      find_nth_close_pair(text, open, close, pos, n),
    )
  };

  // preserve original direction
  match range.direction() {
    Direction::Forward => Option::zip(open, close).ok_or(Error::PairNotFound),
    Direction::Backward => Option::zip(close, open).ok_or(Error::PairNotFound),
  }
}

fn find_nth_open_pair(
  text: RopeSlice,
  open: char,
  close: char,
  mut pos: usize,
  n: usize,
) -> Option<usize> {
  if pos >= text.len_chars() {
    return None;
  }

  let mut chars = text.chars_at(pos + 1);

  // Adjusts pos for the first iteration, and handles the case of the
  // cursor being *on* the close character which will get falsely stepped over
  // if not skipped here
  if chars.prev()? == open {
    return Some(pos);
  }

  for _ in 0..n {
    let mut step_over: usize = 0;

    loop {
      let c = chars.prev()?;
      pos = pos.saturating_sub(1);

      // ignore other surround pairs that are enclosed *within* our search scope
      if c == close {
        step_over += 1;
      } else if c == open {
        if step_over == 0 {
          break;
        }

        step_over = step_over.saturating_sub(1);
      }
    }
  }

  Some(pos)
}

fn find_nth_close_pair(
  text: RopeSlice,
  open: char,
  close: char,
  mut pos: usize,
  n: usize,
) -> Option<usize> {
  if pos >= text.len_chars() {
    return None;
  }

  let mut chars = text.chars_at(pos);

  if chars.next()? == close {
    return Some(pos);
  }

  for _ in 0..n {
    let mut step_over: usize = 0;

    loop {
      let c = chars.next()?;
      pos += 1;

      if c == open {
        step_over += 1;
      } else if c == close {
        if step_over == 0 {
          break;
        }

        step_over = step_over.saturating_sub(1);
      }
    }
  }

  Some(pos)
}

/// Find position of surround characters around every cursor. Returns an
/// ordered pair `(open, close)` for each selection, normalized so `open <= close`.
/// Returns an error if any positions overlap across selections.
/// positions. `ch` can be either closing or opening pair. If `ch` is None,
/// surround pairs are automatically detected around each cursor (note that this
/// may result in them selecting different surround characters for each
/// selection).
pub fn get_surround_pos(
  syntax: Option<&Syntax>,
  text: RopeSlice,
  selection: &Selection,
  ch: Option<char>,
  skip: usize,
) -> Result<SmallVec<[SurroundPair; 1]>> {
  let mut pairs = SmallVec::with_capacity(selection.ranges().len());
  let mut seen = HashSet::new();

  for &range in selection {
    let (open_pos, close_pos) = match ch {
      Some(ch) => find_nth_pairs_pos(text, ch, range, skip)?,
      None => find_nth_closest_pairs_pos(syntax, text, range, skip)?,
    };
    let range = Range::new(open_pos, close_pos);
    let open_pos = range.from();
    let close_pos = range.to();

    if !seen.insert(open_pos) || !seen.insert(close_pos) {
      return Err(Error::CursorOverlap);
    }
    pairs.push((open_pos, close_pos));
  }
  Ok(pairs)
}

#[cfg(test)]
mod test {
  use ropey::Rope;
  use smallvec::SmallVec;

  use super::*;
  use crate::selection::Range;

  #[test]
  fn test_get_surround_pos() {
    #[rustfmt::skip]
        let (doc, selection, expectations) =
            rope_with_selections_and_expectations(
                "(some) (chars)\n(newline)",
                "_ ^  _ _ ^   _\n_    ^  _"
            );

    let positions = get_surround_pos(None, doc.slice(..), &selection, Some('('), 1).unwrap();
    assert_eq!(positions.as_slice(), expectations.as_slice());
  }

  #[test]
  fn test_get_surround_pos_bail_different_surround_chars() {
    #[rustfmt::skip]
        let (doc, selection, _) =
            rope_with_selections_and_expectations(
                "[some]\n(chars)xx\n(newline)",
                "  ^   \n  ^      \n         "
            );

    assert_eq!(
      get_surround_pos(None, doc.slice(..), &selection, Some('('), 1),
      Err(Error::PairNotFound)
    );
  }

  #[test]
  fn test_get_surround_pos_bail_overlapping_surround_chars() {
    #[rustfmt::skip]
        let (doc, selection, _) =
            rope_with_selections_and_expectations(
                "[some]\n(chars)xx\n(newline)",
                "      \n       ^ \n      ^  "
            );

    assert_eq!(
      get_surround_pos(None, doc.slice(..), &selection, Some('('), 1),
      Err(Error::PairNotFound) // overlapping surround chars
    );
  }

  #[test]
  fn test_get_surround_pos_bail_cursor_overlap() {
    #[rustfmt::skip]
        let (doc, selection, _) =
            rope_with_selections_and_expectations(
                "[some]\n(chars)xx\n(newline)",
                "  ^^  \n         \n         "
            );

    assert_eq!(
      get_surround_pos(None, doc.slice(..), &selection, Some('['), 1),
      Err(Error::CursorOverlap)
    );
  }

  #[test]
  fn test_find_nth_pairs_pos_quote_success() {
    #[rustfmt::skip]
        let (doc, selection, expectations) =
            rope_with_selections_and_expectations(
                "some 'quoted text' on this 'line'\n'and this one'",
                "     _        ^  _               \n              "
            );

    assert_eq!(1, expectations.len());
    assert_eq!(
      find_nth_pairs_pos(doc.slice(..), '\'', selection.primary(), 1).expect("find should succeed"),
      expectations[0]
    )
  }

  #[test]
  fn test_find_nth_pairs_pos_nested_quote_success() {
    #[rustfmt::skip]
        let (doc, selection, expectations) =
            rope_with_selections_and_expectations(
                "some 'nested 'quoted' text' on this 'line'\n'and this one'",
                "     _           ^        _               \n              "
            );

    assert_eq!(1, expectations.len());
    assert_eq!(
      find_nth_pairs_pos(doc.slice(..), '\'', selection.primary(), 2).expect("find should succeed"),
      expectations[0]
    )
  }

  #[test]
  fn test_find_nth_pairs_pos_inside_quote_ambiguous() {
    #[rustfmt::skip]
        let (doc, selection, _) =
            rope_with_selections_and_expectations(
                "some 'nested 'quoted' text' on this 'line'\n'and this one'",
                "                    ^                     \n              "
            );

    assert_eq!(
      matches!(
        find_nth_pairs_pos(doc.slice(..), '\'', selection.primary(), 1),
        Err(Error::CursorOnAmbiguousPair { .. })
      ),
      true
    )
  }

  #[test]
  fn test_find_nth_closest_pairs_pos_index_range_panic() {
    #[rustfmt::skip]
        let (doc, selection, _) =
            rope_with_selections_and_expectations(
                "(a)c)",
                "^^^^^"
            );

    assert_eq!(
      find_nth_closest_pairs_pos(None, doc.slice(..), selection.primary(), 1),
      Err(Error::PairNotFound)
    )
  }

  // Create a Rope and a matching Selection using a specification language.
  // ^ is a single-point selection.
  // _ is an expected index. These are returned as a Vec<usize> for use in
  // assertions.
  fn rope_with_selections_and_expectations(
    text: &str,
    spec: &str,
  ) -> (Rope, Selection, Vec<SurroundPair>) {
    if text.len() != spec.len() {
      panic!("specification must match text length -- are newlines aligned?");
    }

    let rope = Rope::from(text);

    let selections: SmallVec<[Range; 1]> = spec
      .match_indices('^')
      .map(|(i, _)| Range::point(i))
      .collect();

    let expected_positions: Vec<usize> = spec.match_indices('_').map(|(i, _)| i).collect();
    assert_eq!(
      expected_positions.len() % 2,
      0,
      "expected positions must be provided in pairs"
    );
    let expectations = expected_positions
      .chunks(2)
      .map(|pair| (pair[0], pair[1]))
      .collect();

    (rope, Selection::new(selections, 0).unwrap(), expectations)
  }
}
