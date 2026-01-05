//! Text wrapping utilities for popup content.
//!
//! This module provides word-aware text wrapping for `TextSegment`-based
//! content, similar to Helix's `reflow.rs` but adapted for our GPU-based
//! renderer.
//!
//! The key insight from Helix is that text should be wrapped at **render time**
//! using the actual available width, not pre-wrapped with estimated widths.

use the_editor_renderer::{
  Color,
  TextSegment,
  TextStyle,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A styled grapheme with its display width.
#[derive(Clone)]
pub struct StyledGrapheme<'a> {
  pub symbol: &'a str,
  pub style:  TextStyle,
  pub width:  u16,
}

/// Word wrapper that wraps styled text segments at word boundaries.
///
/// This is modeled after Helix's `WordWrapper` but works with our `TextSegment`
/// type. It wraps text at render time using the actual available width.
pub struct WordWrapper<'a> {
  /// Iterator over styled graphemes
  graphemes:      Vec<StyledGrapheme<'a>>,
  /// Current position in graphemes
  pos:            usize,
  /// Maximum line width in display cells
  max_line_width: u16,
  /// Whether to trim leading whitespace on wrapped lines
  trim:           bool,
}

impl<'a> WordWrapper<'a> {
  /// Create a new word wrapper from text segments.
  ///
  /// # Arguments
  /// * `segments` - The text segments to wrap
  /// * `max_line_width` - Maximum width in display cells (characters for
  ///   monospace)
  /// * `trim` - Whether to trim leading whitespace on continuation lines
  pub fn new(segments: &'a [TextSegment], max_line_width: u16, trim: bool) -> Self {
    let graphemes = segments_to_graphemes(segments);
    Self {
      graphemes,
      pos: 0,
      max_line_width,
      trim,
    }
  }

  /// Get the next wrapped line.
  ///
  /// Returns `None` when all content has been consumed.
  /// Returns `Some((segments, width))` with the line's segments and actual
  /// width.
  pub fn next_line(&mut self) -> Option<(Vec<TextSegment>, u16)> {
    if self.max_line_width == 0 {
      return None;
    }

    if self.pos >= self.graphemes.len() {
      return None;
    }

    let mut current_line: Vec<StyledGrapheme<'a>> = Vec::new();
    let mut current_width: u16 = 0;
    let mut last_word_end: usize = 0;
    let mut width_at_word_end: u16 = 0;
    let mut prev_whitespace = false;
    let mut first_non_whitespace_seen = false;

    while self.pos < self.graphemes.len() {
      let grapheme = &self.graphemes[self.pos];

      // Skip leading whitespace if trimming
      if self.trim && !first_non_whitespace_seen {
        if is_whitespace(grapheme.symbol) && !is_line_ending(grapheme.symbol) {
          self.pos += 1;
          continue;
        }
        first_non_whitespace_seen = true;
      }

      // Break on newline
      if is_line_ending(grapheme.symbol) {
        self.pos += 1;
        break;
      }

      // Skip graphemes wider than the max width
      if grapheme.width > self.max_line_width {
        self.pos += 1;
        continue;
      }

      // Track word boundaries (transition from whitespace to non-whitespace)
      let is_ws = is_whitespace(grapheme.symbol);
      if is_ws && !prev_whitespace && !current_line.is_empty() {
        last_word_end = current_line.len();
        width_at_word_end = current_width;
      }
      prev_whitespace = is_ws;

      // Check if adding this grapheme would exceed max width
      if current_width + grapheme.width > self.max_line_width {
        // Need to wrap - determine break point
        if last_word_end > 0 {
          // Break at last word boundary
          // Push remaining graphemes back
          let remainder: Vec<_> = current_line.drain(last_word_end..).collect();

          // Skip leading whitespace in remainder
          let first_non_ws = remainder
            .iter()
            .position(|g| !is_whitespace(g.symbol))
            .unwrap_or(remainder.len());

          // Prepend non-whitespace remainder to graphemes for next iteration
          let mut new_graphemes =
            Vec::with_capacity(remainder.len() - first_non_ws + self.graphemes.len() - self.pos);
          new_graphemes.extend(remainder.into_iter().skip(first_non_ws));
          new_graphemes.extend(self.graphemes.drain(self.pos..));
          self.graphemes = new_graphemes;
          self.pos = 0;

          current_width = width_at_word_end;
        } else {
          // No word boundary - break at character level
          // Current grapheme goes to next line
          break;
        }
        break;
      }

      current_line.push(grapheme.clone());
      current_width += grapheme.width;
      self.pos += 1;
    }

    // Convert graphemes back to segments
    if current_line.is_empty() && self.pos >= self.graphemes.len() {
      return None;
    }

    let segments = graphemes_to_segments(&current_line);
    Some((segments, current_width))
  }
}

/// Convert text segments to a flat list of styled graphemes.
fn segments_to_graphemes(segments: &[TextSegment]) -> Vec<StyledGrapheme<'_>> {
  let mut graphemes = Vec::new();

  for segment in segments {
    for grapheme in segment.content.graphemes(true) {
      let width = UnicodeWidthStr::width(grapheme) as u16;
      graphemes.push(StyledGrapheme {
        symbol: grapheme,
        style: segment.style.clone(),
        width,
      });
    }
  }

  graphemes
}

/// Convert styled graphemes back to text segments, merging adjacent same-style
/// graphemes.
fn graphemes_to_segments(graphemes: &[StyledGrapheme<'_>]) -> Vec<TextSegment> {
  if graphemes.is_empty() {
    return Vec::new();
  }

  let mut segments = Vec::new();
  let mut current_content = String::new();
  let mut current_style = graphemes[0].style.clone();

  for grapheme in graphemes {
    if styles_equal(&grapheme.style, &current_style) {
      current_content.push_str(grapheme.symbol);
    } else {
      if !current_content.is_empty() {
        segments.push(TextSegment {
          content: std::mem::take(&mut current_content),
          style:   current_style.clone(),
        });
      }
      current_content = grapheme.symbol.to_string();
      current_style = grapheme.style.clone();
    }
  }

  if !current_content.is_empty() {
    segments.push(TextSegment {
      content: current_content,
      style:   current_style,
    });
  }

  segments
}

/// Check if two styles are equal (for merging segments).
fn styles_equal(a: &TextStyle, b: &TextStyle) -> bool {
  (a.size - b.size).abs() < f32::EPSILON && colors_equal(&a.color, &b.color)
}

/// Check if two colors are equal.
fn colors_equal(a: &Color, b: &Color) -> bool {
  (a.r - b.r).abs() < f32::EPSILON
    && (a.g - b.g).abs() < f32::EPSILON
    && (a.b - b.b).abs() < f32::EPSILON
    && (a.a - b.a).abs() < f32::EPSILON
}

/// Check if a string is a line ending.
fn is_line_ending(s: &str) -> bool {
  matches!(
    s,
    "\n" | "\r\n" | "\r" | "\u{000B}" | "\u{000C}" | "\u{0085}" | "\u{2028}" | "\u{2029}"
  )
}

/// Check if a string is whitespace (but not a line ending).
fn is_whitespace(s: &str) -> bool {
  // Non-breaking spaces should not be treated as word boundaries
  const NBSP: &str = "\u{00a0}";
  const NNBSP: &str = "\u{202f}";

  s != NBSP && s != NNBSP && s.chars().all(|c| c.is_whitespace())
}

/// Wrap a single line of text segments to fit within max_width.
///
/// This is a convenience function for wrapping pre-parsed content.
/// Returns a vector of lines, where each line is a vector of segments.
pub fn wrap_line(segments: &[TextSegment], max_width: u16, trim: bool) -> Vec<Vec<TextSegment>> {
  if max_width == 0 {
    return vec![segments.to_vec()];
  }

  let mut wrapper = WordWrapper::new(segments, max_width, trim);
  let mut lines = Vec::new();

  while let Some((line_segments, _width)) = wrapper.next_line() {
    lines.push(line_segments);
  }

  if lines.is_empty() {
    lines.push(Vec::new());
  }

  lines
}

/// Wrap multiple lines of text segments.
///
/// Each input line is wrapped independently, preserving intentional line
/// breaks.
pub fn wrap_lines(lines: &[Vec<TextSegment>], max_width: u16, trim: bool) -> Vec<Vec<TextSegment>> {
  let mut result = Vec::new();

  for line in lines {
    if line.is_empty() {
      result.push(Vec::new());
    } else {
      let wrapped = wrap_line(line, max_width, trim);
      result.extend(wrapped);
    }
  }

  result
}

/// Calculate the required size for displaying wrapped text.
///
/// Returns `(max_line_width, total_lines)` where widths are in display cells.
pub fn required_size(lines: &[Vec<TextSegment>], max_width: u16) -> (u16, u16) {
  if max_width == 0 {
    return (0, 0);
  }

  let mut text_width: u16 = 0;
  let mut total_lines: u16 = 0;

  for line in lines {
    if line.is_empty() {
      total_lines += 1;
      continue;
    }

    let mut wrapper = WordWrapper::new(line, max_width, false);
    while let Some((_, line_width)) = wrapper.next_line() {
      text_width = text_width.max(line_width);
      total_lines += 1;
    }
  }

  (text_width, total_lines)
}

/// Calculate the display width of a line of segments in cells.
pub fn line_width(segments: &[TextSegment]) -> u16 {
  segments
    .iter()
    .map(|seg| UnicodeWidthStr::width(seg.content.as_str()) as u16)
    .sum()
}

/// Calculate the display width of a line of segments in pixels.
pub fn line_width_pixels(segments: &[TextSegment], cell_width: f32) -> f32 {
  line_width(segments) as f32 * cell_width
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_segment(content: &str) -> TextSegment {
    TextSegment {
      content: content.to_string(),
      style:   TextStyle {
        size:  14.0,
        color: Color::WHITE,
      },
    }
  }

  #[test]
  fn test_wrap_short_line() {
    let segments = vec![make_segment("hello world")];
    let wrapped = wrap_line(&segments, 80, true);
    assert_eq!(wrapped.len(), 1);
    assert_eq!(wrapped[0][0].content, "hello world");
  }

  #[test]
  fn test_wrap_at_word_boundary() {
    let segments = vec![make_segment("hello world foo bar")];
    let wrapped = wrap_line(&segments, 12, true);
    assert_eq!(wrapped.len(), 2);
    assert_eq!(wrapped[0][0].content, "hello world");
    assert_eq!(wrapped[1][0].content, "foo bar");
  }

  #[test]
  fn test_wrap_long_word() {
    let segments = vec![make_segment("superlongwordthatdoesnotfit")];
    let wrapped = wrap_line(&segments, 10, true);
    assert!(wrapped.len() >= 2);
  }

  #[test]
  fn test_wrap_preserves_style() {
    let segments = vec![
      TextSegment {
        content: "hello ".to_string(),
        style:   TextStyle {
          size:  UI_FONT_SIZE,
          color: Color::RED,
        },
      },
      TextSegment {
        content: "world".to_string(),
        style:   TextStyle {
          size:  UI_FONT_SIZE,
          color: Color::BLUE,
        },
      },
    ];
    let wrapped = wrap_line(&segments, 80, true);
    assert_eq!(wrapped.len(), 1);
    assert_eq!(wrapped[0].len(), 2);
    assert!(colors_equal(&wrapped[0][0].style.color, &Color::RED));
    assert!(colors_equal(&wrapped[0][1].style.color, &Color::BLUE));
  }

  #[test]
  fn test_required_size() {
    let lines = vec![vec![make_segment("short")], vec![make_segment(
      "a longer line that needs wrapping",
    )]];
    let (width, height) = required_size(&lines, 15);
    assert!(width <= 15);
    assert!(height >= 3); // first line + wrapped second line
  }

  #[test]
  fn test_line_width() {
    let segments = vec![make_segment("hello"), make_segment(" world")];
    assert_eq!(line_width(&segments), 11);
  }
}
