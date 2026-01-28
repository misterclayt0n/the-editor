//! Cached syntax highlight spans for render backends.
//!
//! This cache keeps highlight ranges indexed by line to avoid re-running
//! tree-sitter queries on every frame. It is intentionally optional and
//! separate from the parsing state so clients can opt into or ignore it.
//!
//! # Example
//!
//! ```no_run
//! use ropey::Rope;
//! use the_lib::syntax::{
//!   Highlight,
//!   HighlightCache,
//! };
//!
//! let mut cache = HighlightCache::default();
//! let text = Rope::from("let x = 1;\n");
//! cache.update_range(
//!   0..text.len_bytes(),
//!   vec![(Highlight::new(0), 0..5)],
//!   text.slice(..),
//!   1,
//!   1,
//! );
//! assert!(cache.is_range_cached(0..text.len_bytes()));
//! ```
use std::{
  cmp::Reverse,
  collections::HashMap,
  ops,
};

use ropey::RopeSlice;

use super::Highlight;

/// Cache for syntax highlighting results to avoid re-querying tree-sitter on
/// every frame.
///
/// Maximum number of lines to keep in the highlight cache.
/// This prevents unbounded memory growth while being generous enough for most
/// use cases. At ~100 bytes per line of highlights, this is roughly 1MB of
/// cache per document.
const MAX_CACHED_LINES: usize = 10_000;

/// Cache for syntax highlight results to avoid re-querying tree-sitter on every
/// frame.
///
/// This cache stores highlight spans indexed by line number for fast lookup
/// during rendering. It tracks which ranges have been queried and invalidates
/// entries when the document changes.
#[derive(Debug, Clone, Default)]
pub struct HighlightCache {
  /// Cached highlight spans indexed by line number.
  /// Key: line number, Value: highlights that overlap or start on that line
  by_line: HashMap<usize, Vec<(Highlight, ops::Range<usize>)>>,

  /// Document version when cache was last updated
  doc_version:    u64,
  syntax_version: u64,

  /// Byte range that has been queried and cached
  cached_range: ops::Range<usize>,
}

impl HighlightCache {
  /// Get cached highlights for a specific line range
  pub fn get_line_range(
    &self,
    start_line: usize,
    end_line: usize,
  ) -> Vec<(Highlight, ops::Range<usize>)> {
    let mut highlights = Vec::new();

    for line in start_line..=end_line {
      if let Some(line_highlights) = self.by_line.get(&line) {
        highlights.extend_from_slice(line_highlights);
      }
    }

    // Sort by (start, Reverse(end)) so longer ranges come first.
    // Since rendering applies highlights via patch() (last wins), this ensures
    // shorter/more-specific highlights take precedence over container highlights.
    highlights.sort_by_key(|(_, range)| (range.start, Reverse(range.end)));
    highlights
  }

  /// Store highlights for the given byte range
  pub fn update_range(
    &mut self,
    byte_range: ops::Range<usize>,
    highlights: Vec<(Highlight, ops::Range<usize>)>,
    text: RopeSlice,
    doc_version: u64,
    syntax_version: u64,
  ) {
    // Clear old highlights in this range
    let start_line = text.byte_to_line(byte_range.start);
    let end_line = text.byte_to_line(byte_range.end.min(text.len_bytes()));

    for line in start_line..=end_line {
      self.by_line.remove(&line);
    }

    // If cache is getting too large, clear it entirely to prevent unbounded growth
    if self.by_line.len() > MAX_CACHED_LINES {
      self.by_line.clear();
    }

    // Group highlights by the line they start on
    for (highlight, range) in highlights {
      let start_line = text.byte_to_char(range.start);
      let start_line = text.char_to_line(start_line);

      self
        .by_line
        .entry(start_line)
        .or_default()
        .push((highlight, range.clone()));
    }

    // Update metadata
    self.doc_version = doc_version;
    self.syntax_version = syntax_version;
    self.cached_range = byte_range;
  }

  /// Invalidate highlights in the given line range
  pub fn invalidate_line_range(&mut self, start_line: usize, end_line: usize) {
    for line in start_line..=end_line {
      self.by_line.remove(&line);
    }
  }

  /// Check if a byte range is fully cached
  pub fn is_range_cached(&self, byte_range: ops::Range<usize>) -> bool {
    byte_range.start >= self.cached_range.start && byte_range.end <= self.cached_range.end
  }

  /// Get the cached document version
  pub fn version(&self) -> usize {
    self.doc_version as usize
  }

  pub fn doc_version(&self) -> u64 {
    self.doc_version
  }

  pub fn syntax_version(&self) -> u64 {
    self.syntax_version
  }

  /// Clear all cached highlights
  pub fn clear(&mut self) {
    self.by_line.clear();
    self.cached_range = 0..0;
    self.doc_version = 0;
    self.syntax_version = 0;
  }

  /// Get total number of cached highlight entries
  pub fn len(&self) -> usize {
    self.by_line.values().map(|v| v.len()).sum()
  }

  /// Check if cache is empty
  pub fn is_empty(&self) -> bool {
    self.by_line.is_empty()
  }

  /// Check if a specific line is cached
  pub fn is_line_cached(&self, line: usize) -> bool {
    self.by_line.contains_key(&line)
  }
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::*;

  #[test]
  fn test_highlight_cache_basic() {
    let mut cache = HighlightCache::default();
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.version(), 0);

    let text = Rope::from("line 1\nline 2\nline 3\nline 4\n");
    // Text is 28 bytes: "line 1\n" (7) + "line 2\n" (7) + "line 3\n" (7) + "line
    // 4\n" (7)

    // Create test highlights within valid byte ranges
    let highlights = vec![
      (Highlight::new(0), 0..6),   // "line 1" on line 0
      (Highlight::new(1), 7..13),  // "line 2" on line 1
      (Highlight::new(2), 14..20), // "line 3" on line 2
    ];

    // Update cache with highlights
    cache.update_range(
      0..text.len_bytes(),
      highlights.clone(),
      text.slice(..),
      1,
      1,
    );

    // Verify cache state
    assert!(!cache.is_empty());
    assert_eq!(cache.len(), 3);
    assert_eq!(cache.version(), 1);
    assert!(cache.is_range_cached(0..text.len_bytes()));
  }

  #[test]
  fn test_highlight_cache_line_range_lookup() {
    let mut cache = HighlightCache::default();
    let text = Rope::from("line 1\nline 2\nline 3\n");

    // Highlights on different lines
    // Line 0: bytes 0-6
    // Line 1: bytes 7-13
    // Line 2: bytes 14-20
    let highlights = vec![
      (Highlight::new(0), 0..5),   // On line 0
      (Highlight::new(1), 8..12),  // On line 1
      (Highlight::new(2), 15..19), // On line 2
    ];

    cache.update_range(0..text.len_bytes(), highlights, text.slice(..), 1, 1);

    // Get highlights for line 1
    let line1_highlights = cache.get_line_range(1, 1);
    assert_eq!(line1_highlights.len(), 1);
    assert_eq!(line1_highlights[0].0, Highlight::new(1));
    assert_eq!(line1_highlights[0].1, 8..12);

    // Get highlights for lines 0-1
    let multi_line = cache.get_line_range(0, 1);
    assert_eq!(multi_line.len(), 2);
  }

  #[test]
  fn test_highlight_cache_invalidation() {
    let mut cache = HighlightCache::default();
    let text = Rope::from("line 1\nline 2\nline 3\nline 4\n");

    let highlights = vec![
      (Highlight::new(0), 0..5),
      (Highlight::new(1), 8..12),
      (Highlight::new(2), 15..19),
      (Highlight::new(3), 22..26),
    ];

    cache.update_range(0..text.len_bytes(), highlights, text.slice(..), 1, 1);
    assert_eq!(cache.len(), 4);

    // Invalidate line 1
    cache.invalidate_line_range(1, 1);

    // Line 1 highlights should be gone
    let line1_after = cache.get_line_range(1, 1);
    assert_eq!(line1_after.len(), 0);

    // Other lines should still be cached
    let line0 = cache.get_line_range(0, 0);
    assert_eq!(line0.len(), 1);
    let line2 = cache.get_line_range(2, 2);
    assert_eq!(line2.len(), 1);
  }

  #[test]
  fn test_highlight_cache_clear() {
    let mut cache = HighlightCache::default();
    let text = Rope::from("line 1\nline 2\n");

    let highlights = vec![(Highlight::new(0), 0..5), (Highlight::new(1), 8..12)];

    cache.update_range(0..text.len_bytes(), highlights, text.slice(..), 1, 1);
    assert!(!cache.is_empty());

    cache.clear();
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.cached_range, 0..0);
  }

  #[test]
  fn test_highlight_cache_version_tracking() {
    let mut cache = HighlightCache::default();
    let text = Rope::from("test\n");

    assert_eq!(cache.version(), 0);

    cache.update_range(0..5, vec![(Highlight::new(0), 0..4)], text.slice(..), 5, 1);
    assert_eq!(cache.version(), 5);

    cache.update_range(0..5, vec![(Highlight::new(1), 0..4)], text.slice(..), 10, 1);
    assert_eq!(cache.version(), 10);
  }

  #[test]
  fn test_highlight_cache_range_checking() {
    let mut cache = HighlightCache::default();
    let text = Rope::from("line 1\nline 2\nline 3\n");

    // Cache bytes 0-14 (first two lines)
    cache.update_range(0..14, vec![(Highlight::new(0), 0..5)], text.slice(..), 1, 1);

    // Check various ranges
    assert!(cache.is_range_cached(0..14)); // Exact match
    assert!(cache.is_range_cached(5..10)); // Within range
    assert!(!cache.is_range_cached(0..20)); // Beyond cached range
    assert!(!cache.is_range_cached(15..20)); // Completely outside
  }
}
