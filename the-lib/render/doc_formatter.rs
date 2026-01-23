//! Document formatting and visual layout traversal.
//!
//! This module turns a document `RopeSlice` plus `TextFormat` and
//! `TextAnnotations` into a stream of [`FormattedGrapheme`] values with
//! visual positions. The formatter is deterministic, allocation-light,
//! and keeps all rendering logic in one place so clients can remain dumb.
//!
//! # Usage
//!
//! ```no_run
//! use ropey::Rope;
//! use the_lib::render::{doc_formatter::DocumentFormatter, text_format::TextFormat};
//! use the_lib::render::text_annotations::TextAnnotations;
//!
//! let rope = Rope::from("hello\nworld");
//! let text_fmt = TextFormat::default();
//! let mut annotations = TextAnnotations::default();
//!
//! let formatter =
//!   DocumentFormatter::new_at_prev_checkpoint(rope.slice(..), &text_fmt, &mut annotations, 0);
//! for g in formatter {
//!   if g.source.is_eof() { break; }
//!   // render g.raw at g.visual_pos...
//! }
//! ```
use std::{borrow::Cow, cmp::Ordering, mem::replace};

use ropey::RopeSlice;
use the_core::grapheme::{Grapheme, GraphemeStr};
use the_stdx::rope::{RopeGraphemes, RopeSliceExt};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
  position::Position,
  render::{
    FormattedGrapheme,
    GraphemeSource,
    text_annotations::{TextAnnotations, TextAnnotationsCursor},
    text_format::TextFormat,
  },
  syntax::Highlight,
};

// Limit block size for very long lines to avoid pathological traversal costs.
const MAX_BLOCK_CHARS: usize = 4096;

#[derive(Debug, Clone)]
struct GraphemeWithSource<'a> {
  grapheme: Grapheme<'a>,
  source: GraphemeSource,
}

impl<'a> GraphemeWithSource<'a> {
  fn new(
    g: GraphemeStr<'a>,
    visual_x: usize,
    tab_width: u16,
    source: GraphemeSource,
  ) -> GraphemeWithSource<'a> {
    GraphemeWithSource {
      grapheme: Grapheme::new(g, visual_x, tab_width),
      source,
    }
  }
  fn placeholder() -> Self {
    GraphemeWithSource {
      grapheme: Grapheme::Other { g: " ".into() },
      source: GraphemeSource::Document { codepoints: 0 },
    }
  }

  fn doc_chars(&self) -> usize {
    self.source.doc_chars()
  }

  fn is_whitespace(&self) -> bool {
    self.grapheme.is_whitespace()
  }

  fn is_newline(&self) -> bool {
    matches!(self.grapheme, Grapheme::Newline)
  }

  fn is_eof(&self) -> bool {
    self.source.is_eof()
  }

  fn width(&self) -> usize {
    self.grapheme.width()
  }

  fn is_word_boundary(&self) -> bool {
    self.grapheme.is_word_boundary()
  }
}

#[derive(Debug)]
pub struct DocumentFormatter<'a, 't> {
  text_fmt: &'a TextFormat,
  annotations: TextAnnotationsCursor<'a, 't>,
  wrap_indicator_graphemes: Vec<GraphemeStr<'a>>,

  /// The visual position at the end of the last yielded word boundary.
  visual_pos: Position,
  graphemes: RopeGraphemes<'a>,
  /// The character pos of the `graphemes` iter used for inserting annotations.
  char_pos: usize,
  /// The line pos of the `graphemes` iter used for inserting annotations.
  line_pos: usize,
  exhausted: bool,

  inline_annotation_buf: Vec<GraphemeStr<'a>>,
  inline_annotation_idx: usize,
  inline_annotation_highlight: Option<Highlight>,

  // Softwrap specific.
  /// The indentation of the current line.
  /// Is set to `None` if the indentation level is not yet known
  /// because no non-whitespace graphemes have been encountered yet.
  indent_level: Option<usize>,
  /// In case a long word needs to be split a single grapheme might need to be
  /// wrapped while the rest of the word stays on the same line.
  peeked_grapheme: Option<GraphemeWithSource<'a>>,
  /// A first-in first-out (fifo) buffer for the Graphemes of any given word.
  word_buf: Vec<GraphemeWithSource<'a>>,
  /// The index of the next grapheme that will be yielded from the `word_buf`.
  word_i: usize,
}

impl<'a, 't> DocumentFormatter<'a, 't> {
  /// Creates a new formatter at the last block before `char_idx`.
  /// A block is a chunk which always ends with a linebreak.
  /// This is usually just a normal line break.
  /// However very long lines are always wrapped at constant intervals that can
  /// be cheaply calculated to avoid pathological behaviour.
  pub fn new_at_prev_checkpoint(
    text: RopeSlice<'a>,
    text_fmt: &'a TextFormat,
    annotations: &'t mut TextAnnotations<'a>,
    char_idx: usize,
  ) -> Self {
    let block_line_idx = text.char_to_line(char_idx.min(text.len_chars()));
    let line_start = text.line_to_char(block_line_idx);
    let line_len = text.line(block_line_idx).len_chars();
    let block_char_idx = if line_len > MAX_BLOCK_CHARS {
      let in_line = char_idx.saturating_sub(line_start);
      let block_offset = (in_line / MAX_BLOCK_CHARS) * MAX_BLOCK_CHARS;
      line_start + block_offset
    } else {
      line_start
    };
    let annotations = annotations.cursor(block_char_idx);
    let wrap_indicator_graphemes =
      UnicodeSegmentation::graphemes(&*text_fmt.wrap_indicator, true)
        .map(GraphemeStr::from)
        .collect();

    DocumentFormatter {
      text_fmt,
      annotations,
      wrap_indicator_graphemes,
      visual_pos: Position { row: 0, col: 0 },
      graphemes: text.slice(block_char_idx..).graphemes(),
      char_pos: block_char_idx,
      exhausted: false,
      indent_level: None,
      peeked_grapheme: None,
      word_buf: Vec::with_capacity(64),
      word_i: 0,
      line_pos: block_line_idx,
      inline_annotation_buf: Vec::new(),
      inline_annotation_idx: 0,
      inline_annotation_highlight: None,
    }
  }

  /// Returns the char index at the end of the last yielded grapheme.
  pub fn next_char_pos(&self) -> usize {
    self.char_pos
  }

  fn next_inline_annotation_grapheme(
    &mut self,
    char_pos: usize,
  ) -> Option<(GraphemeStr<'a>, Option<Highlight>)> {
    loop {
      if self.inline_annotation_idx < self.inline_annotation_buf.len() {
        let idx = self.inline_annotation_idx;
        self.inline_annotation_idx += 1;
        let grapheme =
          replace(&mut self.inline_annotation_buf[idx], GraphemeStr::from(""));
        return Some((grapheme, self.inline_annotation_highlight));
      }

      self.inline_annotation_highlight = None;
      self.inline_annotation_buf.clear();
      self.inline_annotation_idx = 0;
      if let Some((annotation, highlight)) = self.annotations.next_inline_annotation_at(char_pos) {
        self.inline_annotation_highlight = highlight;
        self.inline_annotation_buf =
          UnicodeSegmentation::graphemes(&*annotation.text, true)
            .map(GraphemeStr::from)
            .collect();
      } else {
        return None;
      }
    }
  }
  fn advance_grapheme(&mut self, col: usize, char_pos: usize) -> Option<GraphemeWithSource<'a>> {
    let (grapheme, source) =
      if let Some((grapheme, highlight)) = self.next_inline_annotation_grapheme(char_pos) {
        (grapheme, GraphemeSource::VirtualText { highlight })
      } else if let Some(grapheme) = self.graphemes.next_grapheme() {
        let codepoints = grapheme.len_chars() as u32;

        let overlay = self.annotations.overlay_at(char_pos);
        let grapheme = match overlay {
          Some((overlay, _)) => GraphemeStr::from(overlay.grapheme.as_str()),
          None => Cow::from(grapheme).into(),
        };

        (grapheme, GraphemeSource::Document { codepoints })
      } else {
        if self.exhausted {
          return None;
        }
        self.exhausted = true;
        // EOF grapheme is required for rendering
        // and correct position computations.
        return Some(GraphemeWithSource {
          grapheme: Grapheme::Other { g: " ".into() },
          source: GraphemeSource::Document { codepoints: 0 },
        });
      };

    let grapheme = GraphemeWithSource::new(grapheme, col, self.text_fmt.tab_width, source);

    Some(grapheme)
  }

  fn peek_grapheme(&mut self, col: usize, char_pos: usize) -> Option<&GraphemeWithSource<'a>> {
    if self.peeked_grapheme.is_none() {
      self.peeked_grapheme = self.advance_grapheme(col, char_pos);
    }
    self.peeked_grapheme.as_ref()
  }

  fn next_grapheme(&mut self, col: usize, char_pos: usize) -> Option<GraphemeWithSource<'a>> {
    self.peek_grapheme(col, char_pos);
    self.peeked_grapheme.take()
  }

  fn allow_exact_fit(&mut self, col: usize, char_pos: usize) -> bool {
    self.text_fmt.soft_wrap_at_text_width
      && self
        .peek_grapheme(col, char_pos)
        .is_some_and(|grapheme| grapheme.is_newline() || grapheme.is_eof())
  }

  fn hard_wrap_limit_exceeded(&self, word_width: usize) -> bool {
    word_width > self.text_fmt.max_wrap as usize
  }

  /// Move a word to the next visual line.
  fn wrap_word(&mut self) -> usize {
    // Softwrap this word to the next line.
    let indent_carry_over = if let Some(indent) = self.indent_level {
      if indent as u16 <= self.text_fmt.max_indent_retain {
        indent as u16
      } else {
        0
      }
    } else {
      // ensure the indent stays 0
      self.indent_level = Some(0);
      0
    };

    let virtual_lines =
      self
        .annotations
        .virtual_lines_at(self.char_pos, self.visual_pos, self.line_pos);
    self.visual_pos.col = indent_carry_over as usize;
    self.visual_pos.row += 1 + virtual_lines;
    let mut word_width = 0;
    let indicator_len = self.wrap_indicator_graphemes.len();
    let wrap_indicator = self.wrap_indicator_graphemes.iter().map(|g| {
      let grapheme = GraphemeWithSource::new(
        g.clone(),
        self.visual_pos.col + word_width,
        self.text_fmt.tab_width,
        GraphemeSource::VirtualText {
          highlight: self.text_fmt.wrap_indicator_highlight,
        },
      );
      word_width += grapheme.width();
      grapheme
    });
    self.word_buf.splice(0..0, wrap_indicator);

    for grapheme in &mut self.word_buf[indicator_len..] {
      let visual_x = self.visual_pos.col + word_width;
      grapheme
        .grapheme
        .change_position(visual_x, self.text_fmt.tab_width);
      word_width += grapheme.width();
    }
    if let Some(grapheme) = &mut self.peeked_grapheme {
      let visual_x = self.visual_pos.col + word_width;
      grapheme
        .grapheme
        .change_position(visual_x, self.text_fmt.tab_width);
    }
    word_width
  }

  fn advance_to_next_word(&mut self) {
    self.word_buf.clear();
    let mut word_width = 0;
    let mut word_chars = 0;

    if self.exhausted {
      return;
    }

    loop {
      let mut col = self.visual_pos.col + word_width;
      let char_pos = self.char_pos + word_chars;
      match col.cmp(&(self.text_fmt.viewport_width as usize)) {
        // The EOF char and newline chars are always selectable in helix. That means
        // that wrapping happens "too-early" if a word fits a line perfectly. This
        // is intentional so that all selectable graphemes are always visible (and
        // therefore the cursor never disappears). However if the user manually set a
        // lower softwrap width then this is undesirable. Just increasing the viewport-
        // width by one doesn't work because if a line is wrapped multiple times then
        // some words may extend past the specified width.
        //
        // So we special case a word that ends exactly at line bounds and is followed
        // by a newline/eof character here.
        Ordering::Equal if self.allow_exact_fit(col, char_pos) => {},
        Ordering::Equal if self.hard_wrap_limit_exceeded(word_width) => return,
        Ordering::Greater if self.hard_wrap_limit_exceeded(word_width) => {
          self.peeked_grapheme = self.word_buf.pop();
          return;
        },
        Ordering::Equal | Ordering::Greater => {
          word_width = self.wrap_word();
          col = self.visual_pos.col + word_width;
        },
        Ordering::Less => (),
      }

      let Some(grapheme) = self.next_grapheme(col, char_pos) else {
        return;
      };
      word_chars += grapheme.doc_chars();

      // Track indentation
      if !grapheme.is_whitespace() && self.indent_level.is_none() {
        self.indent_level = Some(self.visual_pos.col);
      } else if grapheme.grapheme == Grapheme::Newline {
        self.indent_level = None;
      }

      let is_word_boundary = grapheme.is_word_boundary();
      word_width += grapheme.width();
      self.word_buf.push(grapheme);

      if is_word_boundary {
        return;
      }
    }
  }
}

impl<'a, 't> Iterator for DocumentFormatter<'a, 't> {
  type Item = FormattedGrapheme<'a>;

  fn next(&mut self) -> Option<Self::Item> {
    let grapheme = if self.text_fmt.soft_wrap {
      if self.word_i >= self.word_buf.len() {
        self.advance_to_next_word();
        self.word_i = 0;
      }
      let grapheme = replace(
        self.word_buf.get_mut(self.word_i)?,
        GraphemeWithSource::placeholder(),
      );
      self.word_i += 1;
      grapheme
    } else {
      self.advance_grapheme(self.visual_pos.col, self.char_pos)?
    };

    let grapheme = FormattedGrapheme {
      raw: grapheme.grapheme,
      source: grapheme.source,
      visual_pos: self.visual_pos,
      line_idx: self.line_pos,
      char_idx: self.char_pos,
    };

    self.char_pos += grapheme.doc_chars();
    if !grapheme.is_virtual() {
      self.annotations.process_virtual_text_anchors(&grapheme);
    }
    if grapheme.raw == Grapheme::Newline {
      // move to end of newline char
      self.visual_pos.col += 1;
      let virtual_lines =
        self
          .annotations
          .virtual_lines_at(self.char_pos, self.visual_pos, self.line_pos);
      self.visual_pos.row += 1 + virtual_lines;
      self.visual_pos.col = 0;
      if !grapheme.is_virtual() {
        self.line_pos += 1;
      }
    } else {
      self.visual_pos.col += grapheme.width();
    }
    Some(grapheme)
  }
}

#[cfg(test)]
mod doc_formatter_tests {
  use ropey::Rope;

  use super::*;
  use crate::{position::Position};

  #[test]
  fn grapheme_source_is_virtual() {
    let doc_source = GraphemeSource::Document { codepoints: 5 };
    let virtual_source = GraphemeSource::VirtualText { highlight: None };

    assert!(!doc_source.is_virtual());
    assert!(virtual_source.is_virtual());
  }

  #[test]
  fn grapheme_source_is_eof() {
    let eof_source = GraphemeSource::Document { codepoints: 0 };
    let non_eof_source = GraphemeSource::Document { codepoints: 1 };
    let virtual_source = GraphemeSource::VirtualText { highlight: None };

    assert!(eof_source.is_eof());
    assert!(!non_eof_source.is_eof());
    assert!(!virtual_source.is_eof());
  }

  #[test]
  fn grapheme_source_doc_chars() {
    let doc_source = GraphemeSource::Document { codepoints: 42 };
    let virtual_source = GraphemeSource::VirtualText { highlight: None };

    assert_eq!(doc_source.doc_chars(), 42);
    assert_eq!(virtual_source.doc_chars(), 0);
  }

  #[test]
  fn grapheme_with_source_new() {
    let grapheme =
      GraphemeWithSource::new("a".into(), 0, 4, GraphemeSource::Document { codepoints: 1 });

    assert_eq!(grapheme.doc_chars(), 1);
    assert_eq!(grapheme.width(), 1);
    assert!(!grapheme.is_whitespace());
    assert!(!grapheme.is_newline());
    assert!(!grapheme.is_eof());
  }

  #[test]
  fn grapheme_with_source_placeholder() {
    let placeholder = GraphemeWithSource::placeholder();

    assert_eq!(placeholder.doc_chars(), 0);
    assert!(placeholder.is_eof());
    assert!(placeholder.is_whitespace());
  }

  #[test]
  fn grapheme_with_source_whitespace() {
    let space =
      GraphemeWithSource::new(" ".into(), 0, 4, GraphemeSource::Document { codepoints: 1 });
    let tab = GraphemeWithSource::new(
      "\t".into(),
      0,
      4,
      GraphemeSource::Document { codepoints: 1 },
    );

    assert!(space.is_whitespace());
    assert!(tab.is_whitespace());
  }

  #[test]
  fn grapheme_with_source_newline() {
    let newline = GraphemeWithSource::new(
      "\n".into(),
      0,
      4,
      GraphemeSource::Document { codepoints: 1 },
    );

    assert!(newline.is_newline());
    assert!(!newline.is_eof());
  }

  #[test]
  fn formatted_grapheme_methods() {
    let formatted = FormattedGrapheme {
      raw: Grapheme::Other { g: "test".into() },
      source: GraphemeSource::Document { codepoints: 4 },
      visual_pos: Position { row: 1, col: 5 },
      line_idx: 0,
      char_idx: 0,
    };

    assert!(!formatted.is_virtual());
    assert_eq!(formatted.doc_chars(), 4);
    assert!(!formatted.is_whitespace());
    assert_eq!(formatted.width(), 4);
  }

  #[test]
  fn formatted_grapheme_virtual() {
    let virtual_formatted = FormattedGrapheme {
      raw: Grapheme::Other { g: "virt".into() },
      source: GraphemeSource::VirtualText { highlight: None },
      visual_pos: Position { row: 0, col: 0 },
      line_idx: 0,
      char_idx: 0,
    };

    assert!(virtual_formatted.is_virtual());
    assert_eq!(virtual_formatted.doc_chars(), 0);
  }

  fn create_test_text_format() -> TextFormat {
    TextFormat {
      soft_wrap: false,
      tab_width: 4,
      max_wrap: 3,
      max_indent_retain: 4,
      wrap_indicator: "↪".into(),
      wrap_indicator_highlight: None,
      viewport_width: 80,
      soft_wrap_at_text_width: false,
    }
  }

  #[test]
  fn document_formatter_new_at_prev_checkpoint() {
    let rope = Rope::from_str("Hello\nWorld\nTest");
    let text_fmt = create_test_text_format();
    let mut annotations = TextAnnotations::default();

    let formatter = DocumentFormatter::new_at_prev_checkpoint(
      rope.slice(..),
      &text_fmt,
      &mut annotations,
      6, // Position after "Hello\n"
    );

    assert_eq!(formatter.next_char_pos(), 6);
  }

  #[test]
  fn document_formatter_simple_iteration() {
    let rope = Rope::from_str("Hi");
    let text_fmt = create_test_text_format();
    let mut annotations = TextAnnotations::default();

    let mut formatter =
      DocumentFormatter::new_at_prev_checkpoint(rope.slice(..), &text_fmt, &mut annotations, 0);

    let first = formatter.next().unwrap();
    assert_eq!(first.char_idx, 0);
    assert_eq!(first.visual_pos, Position { row: 0, col: 0 });

    let second = formatter.next().unwrap();
    assert_eq!(second.char_idx, 1);
    assert_eq!(second.visual_pos, Position { row: 0, col: 1 });

    // EOF grapheme
    let eof = formatter.next().unwrap();
    assert!(eof.source.is_eof());
  }

  #[test]
  fn document_formatter_newline_handling() {
    let rope = Rope::from_str("A\nB");
    let text_fmt = create_test_text_format();
    let mut annotations = TextAnnotations::default();

    let mut formatter =
      DocumentFormatter::new_at_prev_checkpoint(rope.slice(..), &text_fmt, &mut annotations, 0);

    let a = formatter.next().unwrap();
    assert_eq!(a.visual_pos, Position { row: 0, col: 0 });

    let newline = formatter.next().unwrap();
    assert_eq!(newline.visual_pos, Position { row: 0, col: 1 });

    let b = formatter.next().unwrap();
    assert_eq!(b.visual_pos, Position { row: 1, col: 0 });
  }

  #[test]
  fn document_formatter_soft_wrap_enabled() {
    let rope = Rope::from_str("This is a very long line that should wrap");
    let mut text_fmt = create_test_text_format();
    text_fmt.soft_wrap = true;
    text_fmt.viewport_width = 10;
    let mut annotations = TextAnnotations::default();

    let formatter =
      DocumentFormatter::new_at_prev_checkpoint(rope.slice(..), &text_fmt, &mut annotations, 0);

    // Should wrap at word boundaries when soft wrap is enabled
    let mut graphemes = Vec::new();
    for g in formatter {
      if g.source.is_eof() {
        break;
      }
      graphemes.push(g);
    }

    // Should have some graphemes that moved to new visual lines due to wrapping
    let has_wrapped = graphemes.iter().any(|g| g.visual_pos.row > 0);
    assert!(
      has_wrapped,
      "Expected soft wrap to create multiple visual lines"
    );
  }

  #[test]
  fn document_formatter_tab_width() {
    let rope = Rope::from_str("A\tB");
    let mut text_fmt = create_test_text_format();
    text_fmt.tab_width = 8;
    let mut annotations = TextAnnotations::default();

    let mut formatter =
      DocumentFormatter::new_at_prev_checkpoint(rope.slice(..), &text_fmt, &mut annotations, 0);

    let a = formatter.next().unwrap();
    assert_eq!(a.visual_pos, Position { row: 0, col: 0 });

    let tab = formatter.next().unwrap();
    assert_eq!(tab.visual_pos, Position { row: 0, col: 1 });

    let b = formatter.next().unwrap();
    assert_eq!(b.visual_pos, Position { row: 0, col: 8 }); // Tab expanded to 8 spaces
  }

  #[test]
  fn document_formatter_empty_string() {
    let rope = Rope::from_str("");
    let text_fmt = create_test_text_format();
    let mut annotations = TextAnnotations::default();

    let mut formatter =
      DocumentFormatter::new_at_prev_checkpoint(rope.slice(..), &text_fmt, &mut annotations, 0);

    let eof = formatter.next().unwrap();
    assert!(eof.source.is_eof());
    assert_eq!(eof.visual_pos, Position { row: 0, col: 0 });

    // Should return None after EOF
    assert!(formatter.next().is_none());
  }

  #[test]
  fn document_formatter_unicode_graphemes() {
    let rope = Rope::from_str("café 👨‍👩‍👧‍👦");
    let text_fmt = create_test_text_format();
    let mut annotations = TextAnnotations::default();

    let formatter =
      DocumentFormatter::new_at_prev_checkpoint(rope.slice(..), &text_fmt, &mut annotations, 0);

    let mut graphemes = Vec::new();
    for g in formatter {
      if g.source.is_eof() {
        break;
      }
      graphemes.push(g);
    }

    // Should handle multi-byte UTF-8 characters and emoji sequences properly
    assert!(!graphemes.is_empty());

    // The family emoji is a single grapheme cluster
    let family_grapheme = graphemes
      .iter()
      .find(|g| matches!(g.raw, Grapheme::Other { ref g } if g.contains("👨‍👩‍👧‍👦")));
    assert!(
      family_grapheme.is_some(),
      "Should find family emoji as single grapheme"
    );
  }

  #[test]
  fn document_formatter_start_mid_document() {
    let rope = Rope::from_str("First line\nSecond line\nThird line");
    let text_fmt = create_test_text_format();
    let mut annotations = TextAnnotations::default();

    // Start at beginning of second line
    let second_line_start = rope.line_to_char(1);
    let formatter = DocumentFormatter::new_at_prev_checkpoint(
      rope.slice(..),
      &text_fmt,
      &mut annotations,
      second_line_start,
    );

    assert_eq!(formatter.next_char_pos(), second_line_start);
  }

  #[test]
  fn document_formatter_word_boundary_detection() {
    let rope = Rope::from_str("hello world test");
    let text_fmt = create_test_text_format();
    let mut annotations = TextAnnotations::default();

    let formatter =
      DocumentFormatter::new_at_prev_checkpoint(rope.slice(..), &text_fmt, &mut annotations, 0);

    let mut word_boundaries = Vec::new();
    for g in formatter {
      if g.source.is_eof() {
        break;
      }
      if g.is_word_boundary() {
        word_boundaries.push(g.char_idx);
      }
    }

    // Should detect word boundaries (spaces in this case).
    assert!(!word_boundaries.is_empty(), "Should find word boundaries");
  }

  #[test]
  fn document_formatter_wrap_indicator_emitted() {
    let rope = Rope::from_str("hello world");
    let mut text_fmt = create_test_text_format();
    text_fmt.soft_wrap = true;
    text_fmt.viewport_width = 4;
    text_fmt.wrap_indicator = "↪".into();
    let mut annotations = TextAnnotations::default();

    let formatter =
      DocumentFormatter::new_at_prev_checkpoint(rope.slice(..), &text_fmt, &mut annotations, 0);

    let mut saw_indicator = false;
    for g in formatter {
      if g.source.is_eof() {
        break;
      }
      if g.source.is_virtual() && matches!(g.raw, Grapheme::Other { ref g } if g == "↪") {
        saw_indicator = true;
        break;
      }
    }

    assert!(saw_indicator, "Expected wrap indicator grapheme to be emitted");
  }

  #[test]
  fn document_formatter_inline_annotation_inserts_virtual_text() {
    let rope = Rope::from_str("hello");
    let text_fmt = create_test_text_format();
    let inline = [crate::render::text_annotations::InlineAnnotation::new(0, ">")];
    let mut annotations = TextAnnotations::default();
    let _ = annotations.add_inline_annotations(&inline, None);

    let mut formatter =
      DocumentFormatter::new_at_prev_checkpoint(rope.slice(..), &text_fmt, &mut annotations, 0);

    let first = formatter.next().unwrap();
    assert!(first.source.is_virtual());
    assert!(matches!(first.raw, Grapheme::Other { ref g } if g == ">"));
  }
}
