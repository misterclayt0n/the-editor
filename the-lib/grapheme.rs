//! Utility functions to traverse the unicode graphemes of a `Rope`'s text
//! contents.

use core::slice;
use std::{
  borrow::Cow,
  fmt::{
    self,
    Debug,
    Display,
  },
  marker::PhantomData,
  ops::Deref,
  ptr::NonNull,
};

use ropey::RopeSlice;
use unicode_properties::UnicodeEmoji;
use unicode_segmentation::{
  GraphemeCursor,
  GraphemeIncomplete,
};
use unicode_width::UnicodeWidthStr;

use crate::{
  chars::WhitespaceProperties,
  line_ending::{
    LineEnding,
    char_can_break_after,
  },
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
      g if g == "\t" => {
        Grapheme::Tab {
          width: tab_width_at(visual_x, tab_width),
        }
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
      Grapheme::Other { g } => {
        g.chars()
          .next()
          .is_some_and(|ch| WhitespaceProperties::of(ch).is_some())
      },
    }
  }

  pub fn is_word_boundary(&self) -> bool {
    match self {
      Grapheme::Newline | Grapheme::Tab { .. } => true,
      Grapheme::Other { g } => char_can_break_after(g.chars().last().unwrap()),
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
  ptr:     NonNull<u8>,
  len:     u32,
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
  let Some(first) = g.chars().next() else {
    return 0;
  };

  if first.is_ascii() {
    return match first as u8 {
      0x00..=0x1F | 0x7F => 2,
      _ => 1,
    };
  }

  if WhitespaceProperties::of(first).is_some_and(|p| p.is_zero_width()) {
    return 0;
  }

  if first.is_emoji_char() {
    return 2;
  }

  UnicodeWidthStr::width(g)
}

#[must_use]
pub fn nth_prev_grapheme_boundary(slice: RopeSlice, char_idx: usize, n: usize) -> usize {
  assert!(char_idx <= slice.len_chars());

  if n == 0 {
    return char_idx;
  }

  let mut byte_idx = slice.char_to_byte(char_idx);
  let (mut chunk, mut chunk_byte_idx, ..) = slice.chunk_at_byte(byte_idx);
  let mut cursor = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);

  for _ in 0..n {
    loop {
      match cursor.prev_boundary(chunk, chunk_byte_idx) {
        Ok(Some(boundary)) => {
          byte_idx = boundary;
          break;
        },
        Ok(None) => return 0,
        Err(GraphemeIncomplete::PrevChunk) => {
          let (new_chunk, new_byte_idx, ..) = slice.chunk_at_byte(chunk_byte_idx - 1);
          chunk = new_chunk;
          chunk_byte_idx = new_byte_idx;
        },
        Err(GraphemeIncomplete::PreContext(byte_idx)) => {
          let (ctx_chunk, ctx_byte_idx, ..) = slice.chunk_at_byte(byte_idx - 1);
          cursor.provide_context(ctx_chunk, ctx_byte_idx);
        },
        err => unreachable!("{err:?}"),
      }
    }
  }

  slice.byte_to_char(byte_idx)
}

#[must_use]
pub fn nth_next_grapheme_boundary(slice: RopeSlice, char_idx: usize, n: usize) -> usize {
  assert!(char_idx <= slice.len_chars());

  if n == 0 {
    return char_idx;
  }

  let mut byte_idx = slice.char_to_byte(char_idx);
  let (mut chunk, mut chunk_byte_idx, ..) = slice.chunk_at_byte(byte_idx);
  let mut cursor = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);

  for _ in 0..n {
    loop {
      match cursor.next_boundary(chunk, chunk_byte_idx) {
        Ok(Some(boundary)) => {
          byte_idx = boundary;
          break;
        },
        Ok(None) => return slice.len_chars(),
        Err(GraphemeIncomplete::NextChunk) => {
          chunk_byte_idx += chunk.len();
          let (new_chunk, new_byte_idx, ..) = slice.chunk_at_byte(chunk_byte_idx);
          chunk = new_chunk;
          chunk_byte_idx = new_byte_idx;
        },
        Err(GraphemeIncomplete::PreContext(byte_idx)) => {
          let (ctx_chunk, ctx_byte_idx, ..) = slice.chunk_at_byte(byte_idx - 1);
          cursor.provide_context(ctx_chunk, ctx_byte_idx);
        },
        err => unreachable!("{err:?}"),
      }
    }
  }

  slice.byte_to_char(byte_idx)
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
  assert!(char_idx <= slice.len_chars());

  if char_idx == 0 {
    return 0;
  }

  next_grapheme_boundary(slice, char_idx - 1)
}

/// Returns the passed char index if it's already a grapheme boundary,
/// or the prev grapheme boundary char index if not.
#[must_use]
#[inline]
pub fn ensure_grapheme_boundary_prev(slice: RopeSlice, char_idx: usize) -> usize {
  assert!(char_idx <= slice.len_chars());

  if char_idx == slice.len_chars() {
    return char_idx;
  }

  prev_grapheme_boundary(slice, char_idx + 1)
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
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
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
      ptr:     unsafe { NonNull::new_unchecked(value.as_bytes().as_ptr() as *mut u8) },
      len:     i32::try_from(value.len()).unwrap() as u32,
      phantom: PhantomData,
    }
  }
}

impl From<String> for GraphemeStr<'_> {
  fn from(value: String) -> Self {
    let len = value.len();
    let ptr = Box::into_raw(value.into_bytes().into_boxed_slice()) as *mut u8;
    GraphemeStr {
      ptr:     unsafe { NonNull::new_unchecked(ptr) },
      len:     (i32::try_from(len).unwrap() as u32) | Self::MASK_OWNED,
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
  fn tabs_render_to_the_spaces_left_until_the_next_tab_stop() {
    let mut tab = Grapheme::new("\t".into(), 2, 4);
    assert_eq!(tab.width(), 2);
    assert_eq!(tab.to_string(), "  ");

    tab.change_position(4, 4);
    assert_eq!(tab.width(), 4);
    assert_eq!(tab.to_string(), "    ");
  }

  #[test]
  fn newlines_render_as_one_space() {
    let newline = Grapheme::new("\n".into(), 0, 4);

    assert_eq!(newline.width(), 1);
    assert_eq!(newline.to_string(), " ");
    assert!(newline.is_whitespace());
    assert!(newline.is_word_boundary());
  }

  #[test]
  fn words_punctuation_and_cjk_report_soft_wrap_boundaries() {
    assert!(!Grapheme::new("a".into(), 0, 4).is_word_boundary());
    assert!(!Grapheme::new(".".into(), 0, 4).is_word_boundary());
    assert!(Grapheme::new("-".into(), 0, 4).is_word_boundary());
    assert!(Grapheme::new("漢".into(), 0, 4).is_word_boundary());
  }

  #[test]
  fn grapheme_width_matches_terminal_columns() {
    assert_eq!(grapheme_width("a"), 1);
    assert_eq!(grapheme_width("\u{0000}"), 2);
    assert_eq!(grapheme_width("\u{200B}"), 0);
    assert_eq!(grapheme_width("😀"), 2);
    assert_eq!(grapheme_width("漢"), 2);
    assert_eq!(grapheme_width("a\u{0301}"), 1);
  }

  #[test]
  fn grapheme_boundaries_keep_combining_marks_with_their_base_character() {
    let rope = Rope::from_str("a\u{0301}b");
    let text = rope.slice(..);

    assert_eq!(next_grapheme_boundary(text, 0), 2);
    assert_eq!(prev_grapheme_boundary(text, 3), 2);
    assert_eq!(ensure_grapheme_boundary_next(text, 1), 2);
    assert_eq!(ensure_grapheme_boundary_prev(text, 1), 0);
  }

  #[test]
  fn grapheme_boundaries_treat_crlf_as_one_newline() {
    let rope = Rope::from_str("x\r\ny");
    let text = rope.slice(..);

    assert_eq!(next_grapheme_boundary(text, 1), 3);
    assert_eq!(prev_grapheme_boundary(text, 4), 3);
    assert_eq!(nth_prev_grapheme_boundary(text, 4, 2), 1);
  }

  #[test]
  fn grapheme_strings_compare_by_text() {
    let borrowed: GraphemeStr = "hello".into();
    let owned: GraphemeStr = String::from("hello").into();

    assert_eq!(borrowed, "hello");
    assert_eq!(borrowed, owned);
  }
}
