//! Utility functions to traverse the unicode graphemes of a `Rope`'s text
//! contents.

use core::slice;
use std::{
  borrow::Cow,
  fmt::{self, Debug, Display},
  marker::PhantomData,
  ops::Deref,
  ptr::NonNull,
};

use ropey::{RopeSlice, str_utils::byte_to_char_idx};
use unicode_properties::UnicodeEmoji;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};
use unicode_width::UnicodeWidthStr;

use crate::{
  chars::WhitespaceProperties,
  line_ending::{LineEnding, char_can_break_after},
};

#[inline]
pub fn tab_width_at(visual_x: usize, tab_width: u16) -> usize {
  tab_width as usize - (visual_x % tab_width as usize)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grapheme<'a> {
  Newline,
  Tab { width: usize },
  Other { g: GraphemeStr<'a> },
}

impl<'a> Grapheme<'a> {
  pub fn new_decoration(g: &'static str) -> Grapheme<'a> {
    assert_ne!(g, "\t");
    Grapheme::new(g.into(), 0, 0)
  }

  pub fn new(g: GraphemeStr<'a>, visual_x: usize, tab_width: u16) -> Grapheme<'a> {
    match g {
      g if g == "\t" => Grapheme::Tab {
        width: tab_width_at(visual_x, tab_width),
      },
      _ if LineEnding::from_str(&g).is_some() => Grapheme::Newline,
      _ => Grapheme::Other { g },
    }
  }

  pub fn change_position(&mut self, visual_x: usize, tab_width: u16) {
    if let Grapheme::Tab { width } = self {
      *width = tab_width_at(visual_x, tab_width)
    }
  }

  /// Returns the visual width of this grapheme.
  #[inline]
  pub fn width(&self) -> usize {
    match *self {
      // PERF: width is not cached because we are dealing with
      // ASCII almost all of the time, which already has a
      // fastpath.
      // It's ok to convert to u16 here because no codepoint has a width largert
      // than 2 and graphemes are usually atmost 2 visible codepoints wide.
      Grapheme::Other { ref g } => grapheme_width(g),
      Grapheme::Tab { width } => width,
      Grapheme::Newline => 1,
    }
  }

  pub fn is_whitespace(&self) -> bool {
    match self {
      Grapheme::Newline | Grapheme::Tab { .. } => true,
      Grapheme::Other { g } => g
        .chars()
        .next()
        .is_some_and(|ch| WhitespaceProperties::of(ch).is_some()),
    }
  }

  /// Returns true if a soft line break (for wrapping) is allowed after this grapheme.
  ///
  /// This uses the Unicode Line Breaking Algorithm (UAX #14) for proper handling
  /// of all scripts and character types, including:
  /// - Spaces and whitespace (break allowed)
  /// - CJK characters (break allowed between them)
  /// - Hyphens (break allowed after)
  /// - Non-breaking spaces (break prohibited)
  /// - Word joiners (break prohibited)
  ///
  /// # Note
  /// This method examines only this grapheme without context from surrounding
  /// graphemes. For more accurate line breaking with full context, use
  /// `line_ending::soft_breaks()` on the complete string.
  ///
  /// # Naming
  /// The name `is_word_boundary` is kept for API compatibility (TO BE CHANGED). In the context
  /// of softwrapping, it means "can break after this grapheme".
  pub fn is_word_boundary(&self) -> bool {
    match self {
      Grapheme::Newline | Grapheme::Tab { .. } => true,
      Grapheme::Other { g } => {
        // Use the last character of the grapheme to determine break opportunity,
        // since we're basically asking "can we break AFTER this grapheme?"
        g.chars().last().map(char_can_break_after).unwrap_or(true)
      },
    }
  }
}

impl Display for Grapheme<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match *self {
      Grapheme::Newline => write!(f, " "),
      Grapheme::Tab { width } => {
        for _ in 0..width {
          write!(f, " ")?;
        }
        Ok(())
      },
      Grapheme::Other { ref g } => write!(f, "{g}"),
    }
  }
}

/// A highly compressed `Cow<'a, str>` that holds
/// atmost u31::MAX bytes and is readonly.
pub struct GraphemeStr<'a> {
  ptr: NonNull<u8>,
  len: u32,
  phantom: PhantomData<&'a str>,
}

/// Returns the visual width of a grapheme cluster.
///
/// # Width Rules
/// - ASCII control characters (0x00-0x1F, 0x7F): width 2 (rendered as `^X`)
/// - ASCII printable characters: width 1
/// - Zero-width whitespace (ZWSP, BOM, etc.): width 0
/// - Emoji: width 2
/// - Other Unicode: determined by `unicode-width` crate
#[must_use]
pub fn grapheme_width(g: &str) -> usize {
  let mut chars = g.chars();
  let Some(first) = chars.next() else {
    return 0;
  };

  if chars.next().is_none() && first.is_ascii() {
    return match first as u8 {
      0x00..=0x1F | 0x7F => 2,
      _ => 1,
    };
  }

  if let Some(props) = WhitespaceProperties::of(first) {
    if props.is_zero_width() {
      return 0;
    }
  }

  if first.is_emoji_char() {
    return 2;
  }

  UnicodeWidthStr::width(g)
}

#[must_use]
pub fn nth_prev_grapheme_boundary(slice: RopeSlice, char_idx: usize, n: usize) -> usize {
  assert!(char_idx <= slice.len_chars());

  let mut byte_idx = slice.char_to_byte(char_idx);
  let (mut chunk, mut chunk_byte_idx, mut chunk_char_idx, _) = slice.chunk_at_byte(byte_idx);
  let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);

  for _ in 0..n {
    loop {
      match gc.prev_boundary(chunk, chunk_byte_idx) {
        Ok(None) => return 0,
        Ok(Some(n)) => {
          byte_idx = n;
          break;
        },
        Err(GraphemeIncomplete::PrevChunk) => {
          let (a, b, c, _) = slice.chunk_at_byte(chunk_byte_idx - 1);
          chunk = a;
          chunk_byte_idx = b;
          chunk_char_idx = c;
        },
        Err(GraphemeIncomplete::PreContext(n)) => {
          let ctx_chunk = slice.chunk_at_byte(n - 1).0;
          gc.provide_context(ctx_chunk, n - ctx_chunk.len());
        },
        _ => unreachable!(),
      }
    }
  }
  let tmp = byte_to_char_idx(chunk, byte_idx - chunk_byte_idx);
  chunk_char_idx + tmp
}

#[must_use]
pub fn nth_next_grapheme_boundary(slice: RopeSlice, char_idx: usize, n: usize) -> usize {
  assert!(char_idx <= slice.len_chars());

  let mut byte_idx = slice.char_to_byte(char_idx);
  let (mut chunk, mut chunk_byte_idx, mut chunk_char_idx, _) = slice.chunk_at_byte(byte_idx);
  let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);

  for _ in 0..n {
    loop {
      match gc.next_boundary(chunk, chunk_byte_idx) {
        Ok(None) => return slice.len_chars(),
        Ok(Some(n)) => {
          byte_idx = n;
          break;
        },
        Err(GraphemeIncomplete::NextChunk) => {
          chunk_byte_idx += chunk.len();
          let (a, _, c, _) = slice.chunk_at_byte(chunk_byte_idx);
          chunk = a;
          chunk_char_idx = c;
        },
        Err(GraphemeIncomplete::PreContext(n)) => {
          let ctx_chunk = slice.chunk_at_byte(n - 1).0;
          gc.provide_context(ctx_chunk, n - ctx_chunk.len());
        },
        _ => unreachable!(),
      }
    }
  }

  let tmp = byte_to_char_idx(chunk, byte_idx - chunk_byte_idx);
  chunk_char_idx + tmp
}

/// Finds the next grapheme boundary after the given char position.
#[must_use]
#[inline(always)]
pub fn next_grapheme_boundary(slice: RopeSlice, char_idx: usize) -> usize {
  nth_next_grapheme_boundary(slice, char_idx, 1)
}

/// Finds the previous grapheme boundary after the given char position.
#[must_use]
#[inline(always)]
pub fn prev_grapheme_boundary(slice: RopeSlice, char_idx: usize) -> usize {
  nth_prev_grapheme_boundary(slice, char_idx, 1)
}

/// Returns the passed char index if it's already a grapheme boundary,
/// or the next grapheme boundary char index if not.
#[must_use]
#[inline]
pub fn ensure_grapheme_boundary_next(slice: RopeSlice, char_idx: usize) -> usize {
  // Clamp to valid range
  let char_idx = char_idx.min(slice.len_chars());

  if char_idx == 0 {
    char_idx
  } else {
    next_grapheme_boundary(slice, char_idx - 1)
  }
}

/// Returns the passed char index if it's already a grapheme boundary,
/// or the prev grapheme boundary char index if not.
#[must_use]
#[inline]
pub fn ensure_grapheme_boundary_prev(slice: RopeSlice, char_idx: usize) -> usize {
  // Clamp to valid range
  let char_idx = char_idx.min(slice.len_chars());

  if char_idx == slice.len_chars() {
    char_idx
  } else {
    prev_grapheme_boundary(slice, char_idx + 1)
  }
}

impl GraphemeStr<'_> {
  const MASK_OWNED: u32 = 1 << 31;

  fn len(&self) -> usize {
    (self.len & !Self::MASK_OWNED) as usize
  }
}

impl Deref for GraphemeStr<'_> {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    unsafe {
      let bytes = slice::from_raw_parts(self.ptr.as_ptr(), self.len());
      str::from_utf8_unchecked(bytes)
    }
  }
}

impl Drop for GraphemeStr<'_> {
  fn drop(&mut self) {
    if self.len & Self::MASK_OWNED != 0 {
      // Free allocation.
      unsafe {
        drop(Box::from_raw(slice::from_raw_parts_mut(
          self.ptr.as_ptr(),
          self.len(),
        )))
      }
    }
  }
}

impl<'a> From<&'a str> for GraphemeStr<'a> {
  fn from(value: &'a str) -> Self {
    GraphemeStr {
      ptr: unsafe { NonNull::new_unchecked(value.as_bytes().as_ptr() as *mut u8) },
      len: i32::try_from(value.len()).unwrap() as u32,
      phantom: PhantomData,
    }
  }
}

impl From<String> for GraphemeStr<'_> {
  fn from(value: String) -> Self {
    let len = value.len();
    let ptr = Box::into_raw(value.into_bytes().into_boxed_slice()) as *mut u8;
    GraphemeStr {
      ptr: unsafe { NonNull::new_unchecked(ptr) },
      len: (i32::try_from(len).unwrap() as u32) | Self::MASK_OWNED,
      phantom: PhantomData,
    }
  }
}

impl<'a> From<Cow<'a, str>> for GraphemeStr<'a> {
  fn from(value: Cow<'a, str>) -> Self {
    match value {
      Cow::Borrowed(value) => value.into(),
      Cow::Owned(value) => value.into(),
    }
  }
}

impl<T: Deref<Target = str>> PartialEq<T> for GraphemeStr<'_> {
  fn eq(&self, other: &T) -> bool {
    self.deref() == other.deref()
  }
}

impl PartialEq<str> for GraphemeStr<'_> {
  fn eq(&self, other: &str) -> bool {
    self.deref() == other
  }
}

impl Eq for GraphemeStr<'_> {}

impl Debug for GraphemeStr<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    Debug::fmt(self.deref(), f)
  }
}

impl Display for GraphemeStr<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    Display::fmt(self.deref(), f)
  }
}

impl Clone for GraphemeStr<'_> {
  fn clone(&self) -> Self {
    self.deref().to_owned().into()
  }
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::*;

  #[test]
  fn test_tab_width_at() {
    assert_eq!(tab_width_at(0, 4), 4);
    assert_eq!(tab_width_at(1, 4), 3);
    assert_eq!(tab_width_at(3, 4), 1);
    assert_eq!(tab_width_at(4, 4), 4);
  }

  #[test]
  fn test_grapheme_new_variants_and_width() {
    // Tab
    let mut g = Grapheme::new("\t".into(), 3, 4);
    match g {
      Grapheme::Tab { width } => assert_eq!(width, 1),
      _ => panic!("expected Tab variant"),
    }
    // Changing position updates tab width
    g.change_position(4, 4);
    match g {
      Grapheme::Tab { width } => assert_eq!(width, 4),
      _ => panic!("expected Tab variant"),
    }

    // Newline (LF)
    let g = Grapheme::new("\n".into(), 0, 4);
    match g {
      Grapheme::Newline => assert_eq!(g.width(), 1),
      _ => panic!("expected Newline variant"),
    }

    // Other
    let g = Grapheme::new("a".into(), 0, 4);
    match g {
      Grapheme::Other { ref g } => {
        assert_eq!(g.deref(), "a");
        assert_eq!(grapheme_width(g), 1);
      },
      _ => panic!("expected Other variant"),
    }

    // Wide character
    let g = Grapheme::new("漢".into(), 0, 4);
    match g {
      Grapheme::Other { ref g } => {
        assert_eq!(grapheme_width(g), 2);
      },
      _ => panic!("expected Other variant for wide char"),
    }
  }

  #[test]
  fn test_whitespace_and_word_boundary() {
    // Spaces allow break after
    let space = Grapheme::new(" ".into(), 0, 4);
    assert!(space.is_whitespace());
    assert!(space.is_word_boundary());

    // Letters don't allow break after (within words)
    let ch = Grapheme::new("a".into(), 0, 4);
    assert!(!ch.is_whitespace());
    assert!(!ch.is_word_boundary());

    // Period is InfixSeparator in UAX #14, doesn't allow break (think "3.14")
    let period = Grapheme::new(".".into(), 0, 4);
    assert!(!period.is_whitespace());
    assert!(!period.is_word_boundary()); // Changed: UAX #14 correct behavior

    // Hyphen allows break after
    let hyphen = Grapheme::new("-".into(), 0, 4);
    assert!(!hyphen.is_whitespace());
    assert!(hyphen.is_word_boundary());

    // Tab allows break after
    let tab = Grapheme::new("\t".into(), 2, 4);
    assert!(tab.is_whitespace());
    assert!(tab.is_word_boundary());

    // Newline allows break after
    let nl = Grapheme::new("\n".into(), 0, 4);
    assert!(nl.is_whitespace());
    assert!(nl.is_word_boundary());

    // CJK characters allow break between them
    let cjk = Grapheme::new("漢".into(), 0, 4);
    assert!(!cjk.is_whitespace());
    assert!(cjk.is_word_boundary());
  }

  #[test]
  fn test_display_impl() {
    let other = Grapheme::new("ab".into(), 0, 4);
    assert_eq!(other.to_string(), "ab");

    let tab = Grapheme::new("\t".into(), 2, 4); // width = 2
    assert_eq!(tab.to_string(), "  ");

    let nl = Grapheme::new("\n".into(), 0, 4);
    assert_eq!(nl.to_string(), " ");
  }

  #[test]
  fn test_grapheme_str_conversions_and_eq() {
    let g: GraphemeStr = ("hello").into();
    assert_eq!(&*g, "hello");

    let g2 = g.clone();
    assert_eq!(g, g2);
    assert_eq!(g, "hello");

    let owned: GraphemeStr = String::from("world").into();
    assert_eq!(owned, "world");
  }

  #[test]
  fn test_grapheme_boundaries_combining() {
    // "a\u{0301}" (a + combining acute) is one grapheme cluster of 2 chars
    let text = String::from("a\u{0301}b");
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);

    // Next boundaries
    assert_eq!(next_grapheme_boundary(slice, 0), 2);
    assert_eq!(nth_next_grapheme_boundary(slice, 0, 2), 3);

    // Prev boundaries
    assert_eq!(prev_grapheme_boundary(slice, 3), 2);
    assert_eq!(nth_prev_grapheme_boundary(slice, 3, 2), 0);

    // Ensure functions from within a cluster
    assert_eq!(ensure_grapheme_boundary_next(slice, 1), 2);
    assert_eq!(ensure_grapheme_boundary_prev(slice, 1), 0);
  }

  #[test]
  fn test_grapheme_boundaries_crlf() {
    // CRLF should be treated as a single grapheme cluster by unicode rules
    let text = String::from("x\r\ny");
    let rope = Rope::from_str(&text);
    let slice = rope.slice(..);

    // Start at index after 'x' (1), next boundary should skip CRLF to index 3
    assert_eq!(next_grapheme_boundary(slice, 1), 3);
    // Prev boundary from end (4) should go back to 3 first, then 1
    assert_eq!(prev_grapheme_boundary(slice, 4), 3);
    assert_eq!(nth_prev_grapheme_boundary(slice, 4, 2), 1);
  }

  #[test]
  fn test_grapheme_width_function() {
    // ASCII printable
    assert_eq!(grapheme_width("a"), 1);
    assert_eq!(grapheme_width("Z"), 1);
    assert_eq!(grapheme_width("!"), 1);

    // ASCII control characters -> width 2 (rendered as ^X)
    assert_eq!(grapheme_width("\u{0000}"), 2); // NUL -> ^@
    assert_eq!(grapheme_width("\u{0007}"), 2); // BEL -> ^G
    assert_eq!(grapheme_width("\u{001B}"), 2); // ESC -> ^[
    assert_eq!(grapheme_width("\u{007F}"), 2); // DEL -> ^?

    // Zero-width whitespace -> width 0
    assert_eq!(grapheme_width("\u{200B}"), 0); // Zero-width space
    assert_eq!(grapheme_width("\u{FEFF}"), 0); // BOM / ZWNBSP

    // Emoji -> width 2
    assert_eq!(grapheme_width("😀"), 2);
    assert_eq!(grapheme_width("🇺🇸"), 2); // Flag emoji

    // Wide CJK
    assert_eq!(grapheme_width("漢"), 2);

    // Combining sequences
    assert_eq!(grapheme_width("a\u{0301}"), 1); // a + combining acute
  }
}
