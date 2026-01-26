//! Syntax highlight adapter for render plans.
//!
//! This adapter converts cached tree-sitter highlights into a simple
//! per-grapheme lookup for `render::plan`.
//!
//! # Example
//!
//! ```no_run
//! use ropey::Rope;
//! use the_lib::{
//!   render::{
//!     HighlightProvider,
//!     SyntaxHighlightAdapter,
//!   },
//!   syntax::{
//!     HighlightCache,
//!     Loader,
//!     Syntax,
//!   },
//! };
//!
//! # fn demo(syntax: &Syntax, loader: &Loader) {
//! let text = Rope::from("let x = 1;");
//! let mut cache = HighlightCache::default();
//! let line_range = 0..1;
//! let mut adapter =
//!   SyntaxHighlightAdapter::new(text.slice(..), syntax, loader, &mut cache, line_range, 1, 1);
//! let _ = adapter.highlight_at(0);
//! # }
//! ```

use std::ops::Range;

use ropey::RopeSlice;

use super::HighlightProvider;
use crate::syntax::{
  Highlight,
  HighlightCache,
  Loader,
  Syntax,
};

/// A highlight adapter backed by `Syntax` and `HighlightCache`.
///
/// The adapter expects `highlight_at` calls to be in non-decreasing order of
/// character index for best performance.
pub struct SyntaxHighlightAdapter<'a> {
  text:       RopeSlice<'a>,
  highlights: Vec<(Highlight, Range<usize>)>,
  idx:        usize,
}

impl<'a> SyntaxHighlightAdapter<'a> {
  pub fn new(
    text: RopeSlice<'a>,
    syntax: &'a Syntax,
    loader: &'a Loader,
    cache: &mut HighlightCache,
    line_range: Range<usize>,
    doc_version: u64,
    syntax_version: u64,
  ) -> Self {
    let byte_range = line_range_to_bytes(text, line_range.clone());
    if !byte_range.is_empty()
      && (cache.doc_version() != doc_version
        || cache.syntax_version() != syntax_version
        || !cache.is_range_cached(byte_range.clone()))
    {
      let _ = syntax.requery_and_cache(
        cache,
        text,
        loader,
        line_range.clone(),
        doc_version,
        syntax_version,
      );
    }

    Self::from_cache(text, cache, line_range)
  }

  pub fn from_cache(text: RopeSlice<'a>, cache: &HighlightCache, line_range: Range<usize>) -> Self {
    let highlights = if line_range.start < line_range.end {
      cache.get_line_range(line_range.start, line_range.end - 1)
    } else {
      Vec::new()
    };

    Self {
      text,
      highlights,
      idx: 0,
    }
  }
}

impl HighlightProvider for SyntaxHighlightAdapter<'_> {
  fn highlight_at(&mut self, char_idx: usize) -> Option<Highlight> {
    if self.highlights.is_empty() {
      return None;
    }

    let byte_idx = self.text.char_to_byte(char_idx);
    while let Some((_, range)) = self.highlights.get(self.idx) {
      if byte_idx < range.start {
        return None;
      }
      if byte_idx < range.end {
        return Some(self.highlights[self.idx].0);
      }
      self.idx += 1;
    }
    None
  }
}

fn line_range_to_bytes(text: RopeSlice<'_>, line_range: Range<usize>) -> Range<usize> {
  if line_range.start >= text.len_lines() {
    return 0..0;
  }
  let start_line = line_range.start;
  let end_line = line_range.end.min(text.len_lines());
  let start_byte = text.line_to_byte(start_line);
  let end_byte = if end_line < text.len_lines() {
    text.line_to_byte(end_line)
  } else {
    text.len_bytes()
  };
  start_byte..end_byte
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::*;

  #[test]
  fn adapter_from_cache_resolves_highlights() {
    let text = Rope::from("let x = 1;\n");
    let mut cache = HighlightCache::default();
    cache.update_range(
      0..text.len_bytes(),
      vec![(Highlight::new(1), 0..3)],
      text.slice(..),
      1,
      7,
    );

    let mut adapter = SyntaxHighlightAdapter::from_cache(text.slice(..), &cache, 0..1);
    assert_eq!(adapter.highlight_at(0), Some(Highlight::new(1)));
    assert_eq!(adapter.highlight_at(4), None);
  }
}
