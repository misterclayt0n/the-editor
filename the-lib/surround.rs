use ropey::RopeSlice;
use smallvec::SmallVec;
use thiserror::Error;

use crate::{
  grapheme::next_grapheme_boundary,
  match_brackets::{
    find_matching_bracket,
    find_matching_bracket_fuzzy,
  },
  search,
  selection::{
    Range,
    Selection,
  },
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

pub fn find_nth_closest_pairs_pos(
  syntax: Option<&Syntax>,
  text: RopeSlice,
  range: Range,
  skip: usize,
) -> Result<SurroundPair> {
  validate_range(text, range)?;
  match syntax {
    Some(syntax) => find_nth_closest_pairs_with_syntax(syntax, text, range, skip),
    None => find_nth_closest_pairs_plain(text, range, skip),
  }
}

pub fn find_nth_pairs_pos(
  text: RopeSlice,
  ch: char,
  range: Range,
  n: usize,
) -> Result<SurroundPair> {
  validate_range(text, range)?;
  if text.len_chars() < 2 || n == 0 {
    return Err(Error::PairNotFound);
  }

  let (open, close) = pair_chars(ch);
  let pos = range.cursor(text);

  let pair = if open == close {
    if Some(open) == text.get_char(pos) {
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

  pair
    .0
    .zip(pair.1)
    .map(normalize_pair)
    .ok_or(Error::PairNotFound)
}

pub fn get_surround_pos(
  syntax: Option<&Syntax>,
  text: RopeSlice,
  selection: &Selection,
  ch: Option<char>,
  skip: usize,
) -> Result<SmallVec<[SurroundPair; 1]>> {
  let mut pairs = SmallVec::with_capacity(selection.ranges().len());

  for cursor in selection.iter() {
    let pair = match ch {
      Some(ch) => find_nth_pairs_pos(text, ch, *cursor.range, skip)?,
      None => find_nth_closest_pairs_pos(syntax, text, *cursor.range, skip)?,
    };

    if pairs
      .iter()
      .copied()
      .any(|existing| pairs_overlap(existing, pair))
    {
      return Err(Error::CursorOverlap);
    }

    pairs.push(pair);
  }

  Ok(pairs)
}

fn validate_range(text: RopeSlice, range: Range) -> Result<()> {
  let len = text.len_chars();
  if range.to() > len {
    return Err(Error::RangeExceedsText {
      to: range.to(),
      len,
    });
  }
  Ok(())
}

fn find_nth_closest_pairs_with_syntax(
  syntax: &Syntax,
  text: RopeSlice,
  range: Range,
  mut skip: usize,
) -> Result<SurroundPair> {
  let mut opening = range.from();
  let mut closing = range.to();

  while skip > 0 {
    closing = find_matching_bracket_fuzzy(syntax, text, closing).ok_or(Error::PairNotFound)?;
    opening = find_matching_bracket(syntax, text, closing).ok_or(Error::PairNotFound)?;
    (opening, closing) = normalize_pair((opening, closing));

    if range.from() < opening || closing < range.to().saturating_sub(1) {
      closing = next_grapheme_boundary(text, closing);
      continue;
    }

    skip -= 1;
    if skip != 0 {
      closing = next_grapheme_boundary(text, closing);
    }
  }

  Ok((opening, closing))
}

fn find_nth_closest_pairs_plain(
  text: RopeSlice,
  range: Range,
  mut skip: usize,
) -> Result<SurroundPair> {
  if text.len_chars() < 2 || skip == 0 {
    return Err(Error::PairNotFound);
  }

  let mut stack = Vec::with_capacity(2);
  let pos = range.from();
  let mut close_pos = pos.saturating_sub(1);

  for ch in text.chars_at(pos) {
    close_pos += 1;

    if is_open_bracket(ch) {
      stack.push(ch);
      continue;
    }

    if !is_close_bracket(ch) {
      continue;
    }

    let (open, close) = pair_chars(ch);

    if stack.last() == Some(&open) {
      stack.pop();
      continue;
    }

    let Some(open_pos) = find_nth_open_pair(text, open, close, close_pos, 1) else {
      continue;
    };

    if open_pos > pos.saturating_add(1) || close_pos < range.to().saturating_sub(1) {
      continue;
    }

    if skip > 1 {
      skip -= 1;
      continue;
    }

    return Ok(normalize_pair((open_pos, close_pos)));
  }

  Err(Error::PairNotFound)
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
  if chars.prev()? == open {
    return Some(pos);
  }

  for _ in 0..n {
    let mut step_over = 0usize;

    loop {
      let ch = chars.prev()?;
      pos = pos.saturating_sub(1);

      if ch == close {
        step_over += 1;
      } else if ch == open {
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
    let mut step_over = 0usize;

    loop {
      let ch = chars.next()?;
      pos += 1;

      if ch == open {
        step_over += 1;
      } else if ch == close {
        if step_over == 0 {
          break;
        }
        step_over = step_over.saturating_sub(1);
      }
    }
  }

  Some(pos)
}

fn pairs_overlap(a: SurroundPair, b: SurroundPair) -> bool {
  a.0 <= b.1 && b.0 <= a.1
}

fn normalize_pair((a, b): SurroundPair) -> SurroundPair {
  (a.min(b), a.max(b))
}

fn pair_chars(ch: char) -> (char, char) {
  match ch {
    '(' | ')' => ('(', ')'),
    '{' | '}' => ('{', '}'),
    '[' | ']' => ('[', ']'),
    '<' | '>' => ('<', '>'),
    '‘' | '’' => ('‘', '’'),
    '“' | '”' => ('“', '”'),
    '«' | '»' => ('«', '»'),
    '「' | '」' => ('「', '」'),
    '（' | '）' => ('（', '）'),
    _ => (ch, ch),
  }
}

fn is_open_bracket(ch: char) -> bool {
  matches!(ch, '(' | '{' | '[' | '<' | '‘' | '“' | '«' | '「' | '（')
}

fn is_close_bracket(ch: char) -> bool {
  matches!(ch, ')' | '}' | ']' | '>' | '’' | '”' | '»' | '」' | '）')
}

#[cfg(test)]
mod test {
  use std::borrow::Cow;

  use ropey::Rope;

  use super::*;
  use crate::syntax::{
    FileType,
    LanguageId,
    LanguageRegistry,
    LanguageSyntaxConfig,
    QueryKind,
    SyntaxConfiguration,
    SyntaxResources,
    tree_sitter::Grammar,
  };

  #[derive(Debug, Clone, Copy)]
  struct JsonResources {
    grammar: Grammar,
  }

  impl SyntaxResources for JsonResources {
    fn grammar(&self, grammar_name: &str) -> Option<Grammar> {
      (grammar_name == "json").then_some(self.grammar)
    }

    fn query(&self, _language_id: &str, _kind: QueryKind) -> Option<Cow<'_, str>> {
      None
    }
  }

  fn json_syntax(text: &str) -> (Rope, Syntax) {
    let grammar = Grammar::try_from(tree_sitter_json::LANGUAGE).unwrap();
    let registry = LanguageRegistry::new(
      SyntaxConfiguration {
        language: vec![LanguageSyntaxConfig {
          language_id:          "json".into(),
          scope:                "source.json".into(),
          file_types:           vec![FileType::Extension("json".into())],
          shebangs:             Vec::new(),
          comment_tokens:       None,
          block_comment_tokens: None,
          text_width:           None,
          soft_wrap:            None,
          grammar:              Some("json".into()),
          injection_regex:      None,
          indent:               None,
          auto_pairs:           None,
          rulers:               None,
          rainbow_brackets:     None,
        }],
      },
      JsonResources { grammar },
    )
    .unwrap();

    let rope = Rope::from(text);
    let syntax = Syntax::new(rope.slice(..), LanguageId::new(0), &registry).unwrap();
    (rope, syntax)
  }

  #[test]
  fn explicit_pair_lookup_returns_normalized_bounds_even_for_backward_ranges() {
    let text = Rope::from("(abc)");

    assert_eq!(
      find_nth_pairs_pos(text.slice(..), '(', Range::new(3, 1), 1),
      Ok((0, 4))
    );
  }

  #[test]
  fn explicit_pair_lookup_supports_nested_symmetric_pairs_with_skip() {
    let text = Rope::from("some 'nested 'quoted' text' on this line");

    assert_eq!(
      find_nth_pairs_pos(text.slice(..), '\'', Range::point(18), 2),
      Ok((5, 26))
    );
  }

  #[test]
  fn explicit_symmetric_pair_lookup_rejects_cursor_on_an_ambiguous_pair_char() {
    let text = Rope::from("'quoted'");

    assert!(matches!(
      find_nth_pairs_pos(text.slice(..), '\'', Range::point(0), 1),
      Err(Error::CursorOnAmbiguousPair { .. })
    ));
  }

  #[test]
  fn closest_pair_lookup_works_without_syntax() {
    let text = Rope::from("(outer [inner])");

    assert_eq!(
      find_nth_closest_pairs_pos(None, text.slice(..), Range::point(8), 1),
      Ok((7, 13))
    );
    assert_eq!(
      find_nth_closest_pairs_pos(None, text.slice(..), Range::point(8), 2),
      Ok((0, 14))
    );
  }

  #[test]
  fn closest_pair_lookup_can_use_syntax_when_available() {
    let (text, syntax) = json_syntax("[1,[2,3]]");

    assert_eq!(
      find_nth_closest_pairs_pos(Some(&syntax), text.slice(..), Range::point(5), 1),
      Ok((3, 7))
    );
  }

  #[test]
  fn get_surround_pos_is_all_or_nothing_across_the_selection() {
    let text = Rope::from("[some]\n(chars)xx\n(newline)");
    let selection = Selection::new(smallvec::smallvec![Range::point(2), Range::point(10)]).unwrap();

    assert_eq!(
      get_surround_pos(None, text.slice(..), &selection, Some('('), 1),
      Err(Error::PairNotFound)
    );
  }

  #[test]
  fn get_surround_pos_rejects_overlapping_detected_pairs() {
    let text = Rope::from("[some]");
    let selection = Selection::new(smallvec::smallvec![Range::point(2), Range::point(3)]).unwrap();

    assert_eq!(
      get_surround_pos(None, text.slice(..), &selection, Some('['), 1),
      Err(Error::CursorOverlap)
    );
  }
}
