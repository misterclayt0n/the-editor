//! Automatic bracket and quote pairing.
//!
//! This module provides functionality for automatically inserting matching
//! pairs (brackets, quotes, etc.) when the user types an opening character,
//! and for deleting pairs together.
//!
//! # How It Works
//!
//! When the user types an opening character like `(`, the module can:
//!
//! 1. **Insert pair** - Insert both `(` and `)`, placing cursor between them
//! 2. **Skip close** - If already at `)`, just move past it instead of
//!    inserting
//! 3. **Insert single** - Insert only the typed character (no pairing)
//!
//! The decision depends on context (what characters surround the cursor).
//!
//! # Default Pairs
//!
//! The [`DEFAULT_PAIRS`] constant provides common bracket/quote pairs:
//!
//! ```ignore
//! pub const DEFAULT_PAIRS: &[(&str, &str)] = &[
//!     ("(", ")"),
//!     ("{", "}"),
//!     ("[", "]"),
//!     ("'", "'"),
//!     ("\"", "\""),
//!     ("`", "`"),
//! ];
//! ```
//!
//! # Multi-Character Pairs
//!
//! Pairs can be multi-character, useful for languages with special syntax:
//!
//! ```ignore
//! let pairs = AutoPairs::new(vec![
//!     ("{{", "}}"),  // Jinja/Handlebars templates
//!     ("{%", "%}"),
//!     ("/*", "*/"),  // Block comments
//! ]);
//! ```
//!
//! # Usage
//!
//! ## Insert Hook
//!
//! Call [`hook`] when a character is typed to potentially transform it into
//! a paired insertion:
//!
//! ```ignore
//! use the_lib::auto_pairs::{AutoPairs, hook};
//!
//! let pairs = AutoPairs::default();
//!
//! // User typed '('
//! if let Some(transaction) = hook(&doc, &selection, '(', &pairs)? {
//!     // Apply the transaction - inserts "()" with cursor between
//!     transaction.apply(&mut doc)?;
//! }
//! ```
//!
//! ## Delete Hook
//!
//! Call [`delete_hook`] on backspace to potentially delete both characters
//! of a pair:
//!
//! ```ignore
//! // Cursor is between "()"
//! if let Some(transaction) = delete_hook(&doc, &selection, &pairs)? {
//!     // Deletes both '(' and ')'
//!     transaction.apply(&mut doc)?;
//! }
//! ```
//!
//! # Pairing Conditions
//!
//! A pair is only closed automatically when:
//!
//! - The character after the cursor is not alphanumeric
//! - For "same" pairs (like quotes), the character before is also not
//!   alphanumeric
//!
//! This prevents unwanted pairing in contexts like `don't` (no pair after `n`).
//!
//! # Skipping Closing Characters
//!
//! When typing a closing character that already exists at the cursor position,
//! the cursor skips over it instead of inserting a duplicate. This allows
//! natural typing flow: `(|)` → type `)` → `()|`

use ropey::Rope;
use smallvec::SmallVec;
use the_core::grapheme;
use thiserror::Error;

use crate::{
  Tendril,
  movement::Direction,
  selection::{
    Range,
    Selection,
    SelectionError,
  },
  transaction::{
    Change,
    Transaction,
    TransactionError,
  },
};

// Heavily based on https://github.com/codemirror/closebrackets/
pub const DEFAULT_PAIRS: &[(&str, &str)] = &[
  ("(", ")"),
  ("{", "}"),
  ("[", "]"),
  ("'", "'"),
  ("\"", "\""),
  ("`", "`"),
];

/// Represents the config for a particular pairing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
  pub open:  Tendril,
  pub close: Tendril,
}

/// The type that represents the collection of auto pairs,
/// kept as a list to allow multi-char matching.
#[derive(Debug, Clone)]
pub struct AutoPairs(Vec<Pair>);

pub type Result<T> = std::result::Result<T, AutoPairsError>;

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum AutoPairsError {
  #[error(transparent)]
  Selection(#[from] SelectionError),
  #[error(transparent)]
  Transaction(#[from] TransactionError),
}

impl Pair {
  /// true if open == close
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

  pub fn next_is_not_alpha(doc: &Rope, range: &Range) -> bool {
    let cursor = range.cursor(doc.slice(..));
    let next_char = doc.get_char(cursor);
    next_char.map(|c| !c.is_alphanumeric()).unwrap_or(true)
  }

  pub fn prev_is_not_alpha(doc: &Rope, range: &Range) -> bool {
    let cursor = range.cursor(doc.slice(..));
    let prev_char = prev_char(doc, cursor);
    prev_char.map(|c| !c.is_alphanumeric()).unwrap_or(true)
  }

  /// true if all of the pair's conditions hold for the given document and range
  pub fn should_close(&self, doc: &Rope, range: &Range) -> bool {
    Self::next_is_not_alpha(doc, range) && (!self.same() || Self::prev_is_not_alpha(doc, range))
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
    let mut open_tendril = Tendril::new();
    open_tendril.push(open);
    let mut close_tendril = Tendril::new();
    close_tendril.push(close);
    Self {
      open:  open_tendril,
      close: close_tendril,
    }
  }
}

impl From<&(char, char)> for Pair {
  fn from(&(open, close): &(char, char)) -> Self {
    Self::from((open, close))
  }
}

impl From<(&char, &char)> for Pair {
  fn from((open, close): (&char, &char)) -> Self {
    Self::from((*open, *close))
  }
}

impl AutoPairs {
  /// Make a new AutoPairs set with the given pairs and default conditions.
  pub fn new<V, A>(pairs: V) -> Self
  where
    V: IntoIterator<Item = A>,
    A: Into<Pair>,
  {
    let iter = pairs.into_iter();
    let (lower, upper) = iter.size_hint();
    let mut auto_pairs = Vec::with_capacity(upper.unwrap_or(lower));

    for pair in iter {
      auto_pairs.push(pair.into());
    }

    Self(auto_pairs)
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
    AutoPairs::new(DEFAULT_PAIRS.iter().copied())
  }
}

// insert hook:
// Fn(doc, selection, char) => Option<Transaction>
// problem is, we want to do this per range, so we can call default handler for
// some ranges so maybe ret Vec<Option<Change>>
// but we also need to be able to return transactions...
//
// to simplify, maybe return Option<Transaction> and just reimplement the
// default

pub fn hook(
  doc: &Rope,
  selection: &Selection,
  ch: char,
  pairs: &AutoPairs,
) -> Result<Option<Transaction>> {
  tracing::trace!("autopairs hook selection: {:#?}", selection);

  if !pairs.matches_char(ch) {
    return Ok(None);
  }

  let doc_slice = doc.slice(..);
  let transaction = build_transaction(doc, selection, |range| {
    make_change_for_range(doc, doc_slice, range, ch, pairs)
  })?;

  Ok(Some(transaction))
}

/// Delete hook for removing matching auto-paired characters around the cursor.
pub fn delete_hook(
  doc: &Rope,
  selection: &Selection,
  pairs: &AutoPairs,
) -> Result<Option<Transaction>> {
  let doc_slice = doc.slice(..);
  let mut deletions = Vec::with_capacity(selection.ranges().len());

  for range in selection.iter() {
    if !range.is_empty() {
      return Ok(None);
    }

    let cursor = range.cursor(doc_slice);
    let Some((from, to)) = delete_pair_range(doc_slice, cursor, pairs) else {
      return Ok(None);
    };
    deletions.push((from, to));
  }

  if deletions.is_empty() {
    return Ok(None);
  }

  let transaction = Transaction::delete(doc, deletions.into_iter())?;
  let new_selection = selection.clone().map(transaction.changes())?;
  Ok(Some(transaction.with_selection(new_selection)))
}

struct ChangeOutcome {
  change:        Change,
  inserted_len:  usize,
  selection_len: usize,
  advance:       usize,
}

fn insert_text(cursor: usize, text: Tendril, selection_len: usize) -> ChangeOutcome {
  let inserted_len = text.chars().count();
  ChangeOutcome {
    change: (cursor, cursor, Some(text)),
    inserted_len,
    selection_len,
    advance: 0,
  }
}

fn insert_char(cursor: usize, ch: char) -> ChangeOutcome {
  let mut text = Tendril::new();
  text.push(ch);
  insert_text(cursor, text, 1)
}

fn skip(cursor: usize, advance: usize) -> ChangeOutcome {
  ChangeOutcome {
    change: (cursor, cursor, None),
    inserted_len: 0,
    selection_len: 0,
    advance,
  }
}

fn make_change_for_range(
  doc: &Rope,
  doc_slice: ropey::RopeSlice,
  range: &Range,
  ch: char,
  pairs: &AutoPairs,
) -> ChangeOutcome {
  let cursor = range.cursor(doc_slice);

  if let Some(pair) = match_close_pair(doc_slice, cursor, ch, pairs) {
    return skip(cursor, pair.close_len());
  }

  if let Some(pair) = match_open_pair(doc_slice, cursor, ch, pairs) {
    let selection_len = if pair.should_close(doc, range) { 2 } else { 1 };
    let mut text = Tendril::new();
    text.push(ch);

    if selection_len == 2 {
      text.push_str(pair.close.as_str());
    }

    return insert_text(cursor, text, selection_len);
  }

  insert_char(cursor, ch)
}

fn match_open_pair<'a>(
  doc_slice: ropey::RopeSlice,
  cursor: usize,
  ch: char,
  pairs: &'a AutoPairs,
) -> Option<&'a Pair> {
  pairs
    .pairs()
    .iter()
    .filter(|pair| pair.open_last_char() == Some(ch))
    .filter(|pair| matches_open_prefix(doc_slice, cursor, pair))
    .max_by_key(|pair| pair.open_len())
}

fn match_close_pair<'a>(
  doc_slice: ropey::RopeSlice,
  cursor: usize,
  ch: char,
  pairs: &'a AutoPairs,
) -> Option<&'a Pair> {
  pairs
    .pairs()
    .iter()
    .filter(|pair| pair.close_first_char() == Some(ch))
    .filter(|pair| matches_close_at(doc_slice, cursor, pair))
    .max_by_key(|pair| pair.close_len())
}

fn matches_open_prefix(doc_slice: ropey::RopeSlice, cursor: usize, pair: &Pair) -> bool {
  let open_len = pair.open_len();
  if open_len <= 1 {
    return true;
  }

  let prefix_len = open_len - 1;
  let Some(start) = cursor.checked_sub(prefix_len) else {
    return false;
  };
  matches_chars(
    doc_slice,
    start,
    prefix_len,
    pair.open.chars().take(prefix_len),
  )
}

fn matches_close_at(doc_slice: ropey::RopeSlice, cursor: usize, pair: &Pair) -> bool {
  let close_len = pair.close_len();
  if close_len == 0 {
    return false;
  }
  if cursor + close_len > doc_slice.len_chars() {
    return false;
  }
  matches_chars(doc_slice, cursor, close_len, pair.close.chars())
}

fn matches_chars(
  doc_slice: ropey::RopeSlice,
  start: usize,
  len: usize,
  expected: impl Iterator<Item = char>,
) -> bool {
  let end = start + len;
  let slice = doc_slice.slice(start..end);
  iter_chars_eq(slice.chars(), expected)
}

fn iter_chars_eq(
  mut left: impl Iterator<Item = char>,
  mut right: impl Iterator<Item = char>,
) -> bool {
  loop {
    match (left.next(), right.next()) {
      (None, None) => return true,
      (Some(left), Some(right)) if left == right => continue,
      _ => return false,
    }
  }
}

fn delete_pair_range(
  doc_slice: ropey::RopeSlice,
  cursor: usize,
  pairs: &AutoPairs,
) -> Option<(usize, usize)> {
  let mut best: Option<(usize, usize, usize)> = None;

  for pair in pairs.pairs() {
    let open_len = pair.open_len();
    let close_len = pair.close_len();
    if open_len == 0 || close_len == 0 {
      continue;
    }
    if cursor < open_len {
      continue;
    }

    let from = cursor - open_len;
    let to = cursor + close_len;
    if to > doc_slice.len_chars() {
      continue;
    }

    if matches_chars(doc_slice, from, open_len, pair.open.chars())
      && matches_chars(doc_slice, cursor, close_len, pair.close.chars())
    {
      let total_len = open_len + close_len;
      match best {
        Some((best_len, ..)) if best_len >= total_len => {},
        _ => best = Some((total_len, from, to)),
      }
    }
  }

  best.map(|(_, from, to)| (from, to))
}

fn prev_char(doc: &Rope, pos: usize) -> Option<char> {
  if pos == 0 {
    return None;
  }

  doc.get_char(pos - 1)
}

fn advance_graphemes(doc_slice: ropey::RopeSlice, mut pos: usize, count: usize) -> usize {
  for _ in 0..count {
    pos = grapheme::next_grapheme_boundary(doc_slice, pos);
  }
  pos
}

/// calculate what the resulting range should be for an auto pair insertion
fn get_next_range(
  doc: &Rope,
  start_range: &Range,
  offset: usize,
  selection_len: usize,
  advance: usize,
) -> Range {
  // When the character under the cursor changes due to complete pair
  // insertion, we must look backward a grapheme and then add the length
  // of the insertion to put the resulting cursor in the right place, e.g.
  //
  // foo[\r\n] - anchor: 3, head: 5
  // foo([)]\r\n - anchor: 4, head: 5
  //
  // foo[\r\n] - anchor: 3, head: 5
  // foo'[\r\n] - anchor: 4, head: 6
  //
  // foo([)]\r\n - anchor: 4, head: 5
  // foo()[\r\n] - anchor: 5, head: 7
  //
  // [foo]\r\n - anchor: 0, head: 3
  // [foo(])\r\n - anchor: 0, head: 5

  // inserting at the very end of the document after the last newline
  if start_range.head == doc.len_chars() && start_range.anchor == doc.len_chars() {
    return Range::new(
      start_range.anchor + offset + 1,
      start_range.head + offset + 1,
    );
  }

  let doc_slice = doc.slice(..);
  let single_grapheme = start_range.is_single_grapheme(doc_slice);

  // just skip over graphemes; always collapse to a point
  if selection_len == 0 {
    let end = advance_graphemes(doc_slice, start_range.head, advance) + offset;
    return Range::new(end, end);
  }

  // trivial case: only inserted a single-char opener, just move the selection
  if selection_len == 1 {
    if start_range.len() == 0 {
      let end = start_range.head + offset + 1;
      return Range::new(end, end);
    }

    let end_anchor = if single_grapheme || start_range.direction() == Direction::Backward {
      start_range.anchor + offset + 1
    } else {
      start_range.anchor + offset
    };

    return Range::new(end_anchor, start_range.head + offset + 1);
  }

  // If the head = 0, then we must be in insert mode with a backward
  // cursor, which implies the head will just move
  let end_head = if start_range.head == 0 || start_range.direction() == Direction::Backward {
    start_range.head + offset + 1
  } else {
    // We must have a forward cursor, which means we must move to the
    // other end of the grapheme to get to where the new characters
    // are inserted, then move the head to where it should be
    let prev_bound = grapheme::prev_grapheme_boundary(doc_slice, start_range.head);
    tracing::trace!(
      "prev_bound: {}, offset: {}, selection_len: {}",
      prev_bound,
      offset,
      selection_len
    );
    prev_bound + offset + selection_len
  };

  let end_anchor = match (start_range.len(), start_range.direction()) {
    // if we have a zero width cursor, it shifts to the same number
    (0, _) => end_head,

    // If we are inserting for a regular one-width cursor, the anchor
    // moves with the head. This is the fast path for ASCII.
    (1, Direction::Forward) => end_head - 1,
    (1, Direction::Backward) => end_head + 1,

    (_, Direction::Forward) => {
      if single_grapheme {
        grapheme::prev_grapheme_boundary(doc_slice, start_range.head) + 1

      // if we are appending, the anchor stays where it is; only offset
      // for multiple range insertions
      } else {
        start_range.anchor + offset
      }
    },

    (_, Direction::Backward) => {
      if single_grapheme {
        // if we're backward, then the head is at the first char
        // of the typed char, so we need to add the length of
        // the closing char
        grapheme::prev_grapheme_boundary(doc_slice, start_range.anchor) + selection_len + offset
      } else {
        // when we are inserting in front of a selection, we need to move
        // the anchor over by however many characters were inserted overall
        start_range.anchor + offset + selection_len
      }
    },
  };

  Range::new(end_anchor, end_head)
}

fn build_transaction(
  doc: &Rope,
  selection: &Selection,
  mut make_change: impl FnMut(&Range) -> ChangeOutcome,
) -> Result<Transaction> {
  let mut end_ranges = SmallVec::with_capacity(selection.ranges().len());
  let mut offset = 0;

  let transaction = Transaction::change_by_selection(doc, selection, |start_range| {
    let outcome = make_change(start_range);
    let next_range = get_next_range(
      doc,
      start_range,
      offset,
      outcome.selection_len,
      outcome.advance,
    );
    end_ranges.push(next_range);
    offset += outcome.inserted_len;
    outcome.change
  })?;

  let cursor_ids: SmallVec<[crate::selection::CursorId; 1]> =
    selection.cursor_ids().iter().copied().collect();
  let selection = if cursor_ids.len() == end_ranges.len() {
    Selection::new_with_ids(end_ranges, cursor_ids)?
  } else {
    Selection::new(end_ranges)?
  };
  let transaction = transaction.with_selection(selection);
  tracing::debug!("auto pair transaction: {:#?}", transaction);
  Ok(transaction)
}

#[cfg(test)]
mod test {
  use super::*;

  fn make_multi_char_pairs() -> AutoPairs {
    AutoPairs::new([
      ("\"\"\"", "\"\"\""), // triple-quote
      ("{%", "%}"),         // jinja block
      ("{{", "}}"),         // jinja expression
      ("\"", "\""),         // double-quote
      ("(", ")"),           // parens
    ])
  }

  fn apply_transaction(doc: &Rope, tx: &Transaction) -> String {
    let mut doc = doc.clone();
    tx.changes().apply(&mut doc).unwrap();
    doc.to_string()
  }

  #[test]
  fn test_prefix_and_close_matching_helpers() {
    // matches_open_prefix: single-char pairs always match
    let doc = Rope::from("hello");
    let paren: Pair = ("(", ")").into();
    assert!(matches_open_prefix(doc.slice(..), 0, &paren));
    assert!(matches_open_prefix(doc.slice(..), 5, &paren));

    // matches_open_prefix: multi-char requires correct prefix
    let triple: Pair = ("\"\"\"", "\"\"\"").into();
    let jinja: Pair = ("{%", "%}").into();

    assert!(matches_open_prefix(
      Rope::from("\"\"").slice(..),
      2,
      &triple
    ));
    assert!(!matches_open_prefix(Rope::from("\"").slice(..), 1, &triple));
    assert!(!matches_open_prefix(Rope::from("ab").slice(..), 2, &triple));

    assert!(matches_open_prefix(Rope::from("{").slice(..), 1, &jinja));
    assert!(!matches_open_prefix(Rope::from("a").slice(..), 1, &jinja));
    assert!(!matches_open_prefix(Rope::from("").slice(..), 0, &jinja));

    // matches_close_at: single-char
    assert!(matches_close_at(Rope::from(")").slice(..), 0, &paren));
    assert!(!matches_close_at(Rope::from("x").slice(..), 0, &paren));

    // matches_close_at: multi-char
    assert!(matches_close_at(Rope::from("\"\"\"").slice(..), 0, &triple));
    assert!(!matches_close_at(Rope::from("\"\"").slice(..), 0, &triple));
    assert!(!matches_close_at(Rope::from("\"\"x").slice(..), 0, &triple));

    assert!(matches_close_at(Rope::from("%}").slice(..), 0, &jinja));
    assert!(!matches_close_at(Rope::from("%x").slice(..), 0, &jinja));
    assert!(!matches_close_at(Rope::from("%").slice(..), 0, &jinja));
    assert!(!matches_close_at(Rope::from("x").slice(..), 1, &jinja)); // past end
  }

  #[test]
  fn test_triple_quote_behavior() {
    let pairs = make_multi_char_pairs();

    // Insert: `""` + type `"` -> `""""""` (triple-quote pair)
    let doc = Rope::from("\"\"");
    let tx = hook(&doc, &Selection::point(2), '"', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "\"\"\"\"\"\"");

    // Insert with trailing content
    let doc = Rope::from("\"\" world");
    let tx = hook(&doc, &Selection::point(2), '"', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "\"\"\"\"\"\" world");

    // Skip: cursor inside `""""""`, typing `"` skips over closing `"""`
    let doc = Rope::from("\"\"\"\"\"\"");
    let tx = hook(&doc, &Selection::point(3), '"', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "\"\"\"\"\"\"");
    assert_eq!(tx.selection().unwrap().ranges()[0].head, 6);

    // Single quote skip (not triple): at pos 1 in `""`, skip single `"`
    let doc = Rope::from("\"\"");
    let tx = hook(&doc, &Selection::point(1), '"', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "\"\"");
    assert_eq!(tx.selection().unwrap().ranges()[0].head, 2);

    // Delete: cursor at pos 3 in `""""""` removes all 6 chars
    let doc = Rope::from("\"\"\"\"\"\"");
    let tx = delete_hook(&doc, &Selection::point(3), &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "");

    // Delete with surrounding content
    let doc = Rope::from("a\"\"\"\"\"\"b");
    let tx = delete_hook(&doc, &Selection::point(4), &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "ab");
  }

  #[test]
  fn test_jinja_pair_behavior() {
    let pairs = make_multi_char_pairs();

    // Insert `{%`: `{` + type `%` -> `{%%}`
    let doc = Rope::from("{");
    let tx = hook(&doc, &Selection::point(1), '%', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "{%%}");

    // Insert `{{`: `{` + type `{` -> `{{}}`
    let doc = Rope::from("{");
    let tx = hook(&doc, &Selection::point(1), '{', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "{{}}");

    // Skip: cursor at pos 2 in `{%%}`, typing `%` skips over `%}`
    let doc = Rope::from("{%%}");
    let tx = hook(&doc, &Selection::point(2), '%', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "{%%}");
    assert_eq!(tx.selection().unwrap().ranges()[0].head, 4);

    // Delete: cursor at pos 2 in `{%%}` removes all 4 chars
    let doc = Rope::from("{%%}");
    let tx = delete_hook(&doc, &Selection::point(2), &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "");

    // Delete with surrounding content
    let doc = Rope::from("a{%%}b");
    let tx = delete_hook(&doc, &Selection::point(3), &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "ab");
  }

  #[test]
  fn test_longest_pair_match_priority() {
    let pairs = make_multi_char_pairs();

    // Open match: `""` before cursor -> triple `"""` wins over single `"`
    let doc = Rope::from("\"\"");
    let pair = match_open_pair(doc.slice(..), 2, '"', &pairs).unwrap();
    assert_eq!(pair.open.as_str(), "\"\"\"");

    // Close match: `"""` ahead -> triple wins
    let doc = Rope::from("\"\"\"");
    let pair = match_close_pair(doc.slice(..), 0, '"', &pairs).unwrap();
    assert_eq!(pair.close.as_str(), "\"\"\"");

    // Fallback: only one `"` before -> single `"` pair matches
    let doc = Rope::from("\"");
    let pair = match_open_pair(doc.slice(..), 1, '"', &pairs).unwrap();
    assert_eq!(pair.open.as_str(), "\"");
  }

  #[test]
  fn test_document_boundary_edge_cases() {
    let pairs = make_multi_char_pairs();

    // Insert at doc start: empty doc, type `"` -> `""`
    let doc = Rope::from("");
    let tx = hook(&doc, &Selection::point(0), '"', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "\"\"");

    // Build triple-quote from scratch
    let doc = Rope::from("\"\"");
    let tx = hook(&doc, &Selection::point(2), '"', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "\"\"\"\"\"\"");

    // Delete at doc start: `()` with cursor at 1
    let doc = Rope::from("()");
    let tx = delete_hook(&doc, &Selection::point(1), &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "");

    // Delete fails at pos 0 (nothing before cursor)
    let doc = Rope::from("()");
    assert!(
      delete_hook(&doc, &Selection::point(0), &pairs)
        .unwrap()
        .is_none()
    );

    // Skip at doc end: cursor at 1 in `()`, type `)` -> skip to pos 2
    let doc = Rope::from("()");
    let tx = hook(&doc, &Selection::point(1), ')', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "()");
    assert_eq!(tx.selection().unwrap().ranges()[0].head, 2);

    // Insert at doc end: cursor at 2 in `()`, type `)` -> insert and move to pos 3
    let doc = Rope::from("()");
    let tx = hook(&doc, &Selection::point(2), ')', &pairs)
      .unwrap()
      .unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "())");
    let range = &tx.selection().unwrap().ranges()[0];
    assert_eq!(range.head, 3);
    assert_eq!(range.anchor, 3);
  }

  #[test]
  fn test_multiple_cursors() {
    let pairs = make_multi_char_pairs();

    // Insert at multiple positions: `a b c` with cursors at 1, 3, 5
    let doc = Rope::from("a b c");
    let sel = Selection::new(smallvec::smallvec![
      Range::point(1),
      Range::point(3),
      Range::point(5)
    ])
    .unwrap();
    let tx = hook(&doc, &sel, '(', &pairs).unwrap().unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "a() b() c()");

    // Skip at multiple positions: `()()()` with cursors at 1, 3, 5
    let doc = Rope::from("()()()");
    let sel = Selection::new(smallvec::smallvec![
      Range::point(1),
      Range::point(3),
      Range::point(5)
    ])
    .unwrap();
    let tx = hook(&doc, &sel, ')', &pairs).unwrap().unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "()()()");
    let heads: Vec<_> = tx
      .selection()
      .unwrap()
      .ranges()
      .iter()
      .map(|r| r.head)
      .collect();
    assert_eq!(heads, vec![2, 4, 6]);

    // Delete at multiple positions
    let doc = Rope::from("()()()");
    let sel = Selection::new(smallvec::smallvec![
      Range::point(1),
      Range::point(3),
      Range::point(5)
    ])
    .unwrap();
    let tx = delete_hook(&doc, &sel, &pairs).unwrap().unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "");

    // Multi-char pairs with multiple cursors: jinja
    let doc = Rope::from("{ {");
    let sel = Selection::new(smallvec::smallvec![Range::point(1), Range::point(3)]).unwrap();
    let tx = hook(&doc, &sel, '%', &pairs).unwrap().unwrap();
    assert_eq!(apply_transaction(&doc, &tx), "{%%} {%%}");

    // Triple-quote delete with multiple cursors
    let doc = Rope::from("\"\"\"\"\"\" \"\"\"\"\"\"");
    let sel = Selection::new(smallvec::smallvec![Range::point(3), Range::point(10)]).unwrap();
    let tx = delete_hook(&doc, &sel, &pairs).unwrap().unwrap();
    assert_eq!(apply_transaction(&doc, &tx), " ");
  }
}
