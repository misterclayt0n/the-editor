//! Fuzzy matching for filtering and ranking string collections.
//!
//! This module provides fuzzy matching functionality powered by the [`nucleo`]
//! crate. It's designed for filtering small-to-medium collections synchronously
//! on the current thread.
//!
//! # Overview
//!
//! Fuzzy matching allows finding strings that approximately match a pattern,
//! even with typos, abbreviations, or partial matches. Results are ranked by
//! match quality.
//!
//! # Match Modes
//!
//! Two matching modes are available via [`MatchMode`]:
//!
//! - **Plain**: Standard fuzzy matching, suitable for general text
//! - **Path**: Optimized for file paths, giving preference to path separators
//!   and filename matches
//!
//! # Case Sensitivity
//!
//! Smart case matching is used by default:
//! - Lowercase patterns match case-insensitively
//! - Patterns containing uppercase characters match case-sensitively
//!
//! # Examples
//!
//! ```ignore
//! use the_lib::fuzzy::{fuzzy_match, MatchMode};
//!
//! let items = vec!["foo.rs", "bar.rs", "foobar.rs", "baz.txt"];
//!
//! // Find items matching "foo"
//! let results = fuzzy_match("foo", items, MatchMode::Plain);
//! // Returns: [("foo.rs", score), ("foobar.rs", score)]
//!
//! // Results are sorted by score (best match first)
//! let (best_match, _score) = &results[0];
//! assert_eq!(*best_match, "foo.rs");
//! ```
//!
//! # Performance Considerations
//!
//! This module is suitable for:
//! - Command palette filtering
//! - Buffer/file picker with modest file counts
//! - Any synchronous filtering of small collections
//!
//! For large collections or responsive UIs, consider using `nucleo` directly
//! with its async worker API to avoid blocking the main thread.
//!
//! # Thread-Local Matcher
//!
//! [`fuzzy_match`] uses a thread-local matcher instance to avoid allocation
//! overhead. For explicit control over matcher lifetime, use
//! [`fuzzy_match_with`] with your own [`Matcher`] instance.

use std::cell::RefCell;

use nucleo::{
  Config,
  Matcher,
  pattern::{
    Atom,
    AtomKind,
    CaseMatching,
    Normalization,
  },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatchMode {
  #[default]
  Plain,
  Path,
}

thread_local! {
  static MATCHER: RefCell<Matcher> = RefCell::new(Matcher::default());
}

/// convenience function to easily fuzzy match
/// on a (relatively small list of inputs). This is not recommended for building
/// a full tui application that can match large numbers of matches as all
/// matching is done on the current thread, effectively blocking the UI
pub fn fuzzy_match<T: AsRef<str>>(
  pattern: &str,
  items: impl IntoIterator<Item = T>,
  mode: MatchMode,
) -> Vec<(T, u16)> {
  MATCHER.with(|matcher| fuzzy_match_with(&mut matcher.borrow_mut(), pattern, items, mode))
}

/// Fuzzy match using a caller-provided matcher to avoid global state.
pub fn fuzzy_match_with<T: AsRef<str>>(
  matcher: &mut Matcher,
  pattern: &str,
  items: impl IntoIterator<Item = T>,
  mode: MatchMode,
) -> Vec<(T, u16)> {
  matcher.config = Config::DEFAULT;
  if mode == MatchMode::Path {
    matcher.config.set_match_paths();
  }

  let pattern = Atom::new(
    pattern,
    CaseMatching::Smart,
    Normalization::Smart,
    AtomKind::Fuzzy,
    false,
  );
  pattern.match_list(items, matcher)
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn basic_fuzzy_match() {
    let items = vec!["foo", "bar", "baz", "foobar"];
    let results = fuzzy_match("foo", items, MatchMode::Plain);

    // Should match "foo" and "foobar"
    let matched: Vec<_> = results.iter().map(|(s, _)| *s).collect();
    assert!(matched.contains(&"foo"));
    assert!(matched.contains(&"foobar"));
    assert!(!matched.contains(&"bar"));
  }

  #[test]
  fn empty_pattern_matches_all() {
    let items = vec!["foo", "bar", "baz"];
    let results = fuzzy_match("", items, MatchMode::Plain);

    // Empty pattern should match everything
    assert_eq!(results.len(), 3);
  }

  #[test]
  fn no_matches() {
    let items = vec!["foo", "bar", "baz"];
    let results = fuzzy_match("xyz", items, MatchMode::Plain);

    assert!(results.is_empty());
  }

  #[test]
  fn smart_case_matching() {
    let items = vec!["FooBar", "foobar", "FOOBAR"];

    // Lowercase pattern: case-insensitive
    let results = fuzzy_match("foo", items.clone(), MatchMode::Plain);
    assert_eq!(results.len(), 3);

    // Uppercase in pattern: case-sensitive
    let results = fuzzy_match("Foo", items, MatchMode::Plain);
    // Should prefer "FooBar" but may still match others with lower score
    let top_match = &results.first().unwrap().0;
    assert_eq!(*top_match, "FooBar");
  }

  #[test]
  fn results_are_sorted_by_score() {
    let items = vec!["xfoo", "foo", "foox"];
    let results = fuzzy_match("foo", items, MatchMode::Plain);

    // Exact match "foo" should have highest score (first in results)
    assert!(!results.is_empty());
    let scores: Vec<_> = results.iter().map(|(_, score)| *score).collect();
    // Scores should be in descending order
    for i in 1..scores.len() {
      assert!(
        scores[i - 1] >= scores[i],
        "Results should be sorted by score"
      );
    }
  }
}
