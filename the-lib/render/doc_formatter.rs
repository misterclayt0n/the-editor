//! Low-level visual document traversal.
//!
//! `DocumentFormatter` turns document text plus layout policy into a stream of
//! formatted graphemes with visual `(row, col)` positions.

use std::{
  cmp::Ordering,
  mem::replace,
};

use ropey::RopeSlice;
use the_stdx::rope::{
  RopeGraphemes,
  RopeSliceExt,
};

use crate::{
  grapheme::{
    Grapheme,
    GraphemeStr,
  },
  render::text_format::TextFormat,
};

const MAX_BLOCK_CHARS: usize = 4096;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
  pub row: usize,
  pub col: usize,
}

impl Position {
  pub const fn new(row: usize, col: usize) -> Self {
    Self { row, col }
  }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FormatterCheckpoint {
  pub(crate) char_idx:     usize,
  pub(crate) line_idx:     usize,
  pub(crate) visual_pos:   Position,
  pub(crate) indent_level: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphemeSource {
  Document { codepoints: u32 },
  VirtualText,
}

impl GraphemeSource {
  pub fn is_virtual(self) -> bool {
    matches!(self, Self::VirtualText)
  }

  pub fn is_eof(self) -> bool {
    matches!(self, Self::Document { codepoints: 0 })
  }

  pub fn doc_chars(self) -> usize {
    match self {
      Self::Document { codepoints } => codepoints as usize,
      Self::VirtualText => 0,
    }
  }
}

#[derive(Debug, Clone)]
pub struct FormattedGrapheme<'a> {
  pub raw:        Grapheme<'a>,
  pub source:     GraphemeSource,
  pub visual_pos: Position,
  pub line_idx:   usize,
  pub char_idx:   usize,
}

impl FormattedGrapheme<'_> {
  pub fn is_virtual(&self) -> bool {
    self.source.is_virtual()
  }

  pub fn doc_chars(&self) -> usize {
    self.source.doc_chars()
  }

  pub fn is_whitespace(&self) -> bool {
    self.raw.is_whitespace()
  }

  pub fn width(&self) -> usize {
    self.raw.width()
  }

  pub fn is_word_boundary(&self) -> bool {
    self.raw.is_word_boundary()
  }
}

pub(crate) fn prev_checkpoint(text: RopeSlice<'_>, char_idx: usize) -> (usize, usize) {
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
  (block_char_idx, block_line_idx)
}

#[derive(Debug, Clone)]
struct GraphemeWithSource<'a> {
  grapheme: Grapheme<'a>,
  source:   GraphemeSource,
}

impl<'a> GraphemeWithSource<'a> {
  fn new(
    grapheme: GraphemeStr<'a>,
    visual_x: usize,
    tab_width: u16,
    source: GraphemeSource,
  ) -> Self {
    Self {
      grapheme: Grapheme::new(grapheme, visual_x, tab_width),
      source,
    }
  }

  fn eof() -> Self {
    Self {
      grapheme: Grapheme::Other { g: " ".into() },
      source:   GraphemeSource::Document { codepoints: 0 },
    }
  }

  fn placeholder() -> Self {
    Self::eof()
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
pub struct DocumentFormatter<'a> {
  text_format: &'a TextFormat,
  wrap_width:  usize,

  visual_pos: Position,
  graphemes:  RopeGraphemes<'a>,
  char_pos:   usize,
  line_pos:   usize,
  exhausted:  bool,

  peeked_grapheme: Option<GraphemeWithSource<'a>>,
  word_buf:        Vec<GraphemeWithSource<'a>>,
  word_i:          usize,
  indent_level:    Option<usize>,
}

impl<'a> DocumentFormatter<'a> {
  pub fn new(text: RopeSlice<'a>, text_format: &'a TextFormat, wrap_width: usize) -> Self {
    Self::new_at_char(text, text_format, wrap_width, 0)
  }

  pub(crate) fn new_at_char(
    text: RopeSlice<'a>,
    text_format: &'a TextFormat,
    wrap_width: usize,
    char_idx: usize,
  ) -> Self {
    let (block_char_idx, block_line_idx) = prev_checkpoint(text, char_idx);
    Self::new_exact_at_char(
      text,
      text_format,
      wrap_width,
      block_char_idx,
      block_line_idx,
    )
  }

  pub(crate) fn new_exact_at_char(
    text: RopeSlice<'a>,
    text_format: &'a TextFormat,
    wrap_width: usize,
    char_idx: usize,
    line_idx: usize,
  ) -> Self {
    Self::new_exact_with_state(
      text,
      text_format,
      wrap_width,
      char_idx,
      line_idx,
      Position::default(),
      None,
    )
  }

  pub(crate) fn new_at_checkpoint(
    text: RopeSlice<'a>,
    text_format: &'a TextFormat,
    wrap_width: usize,
    checkpoint: FormatterCheckpoint,
  ) -> Self {
    Self::new_exact_with_state(
      text,
      text_format,
      wrap_width,
      checkpoint.char_idx,
      checkpoint.line_idx,
      checkpoint.visual_pos,
      checkpoint.indent_level,
    )
  }

  fn new_exact_with_state(
    text: RopeSlice<'a>,
    text_format: &'a TextFormat,
    wrap_width: usize,
    char_idx: usize,
    line_idx: usize,
    visual_pos: Position,
    indent_level: Option<usize>,
  ) -> Self {
    Self {
      text_format,
      wrap_width: wrap_width.max(1),
      visual_pos,
      graphemes: text.slice(char_idx..).graphemes(),
      char_pos: char_idx,
      line_pos: line_idx,
      exhausted: false,
      peeked_grapheme: None,
      word_buf: Vec::with_capacity(64),
      word_i: 0,
      indent_level,
    }
  }

  pub fn next_char_pos(&self) -> usize {
    self.char_pos
  }

  pub(crate) fn checkpoint(&self) -> Option<FormatterCheckpoint> {
    if self.word_i < self.word_buf.len() || self.peeked_grapheme.is_some() {
      return None;
    }

    Some(FormatterCheckpoint {
      char_idx:     self.char_pos,
      line_idx:     self.line_pos,
      visual_pos:   self.visual_pos,
      indent_level: self.indent_level,
    })
  }

  fn advance_grapheme(&mut self, col: usize, _char_pos: usize) -> Option<GraphemeWithSource<'a>> {
    if let Some(grapheme) = self.graphemes.next_grapheme() {
      let codepoints = grapheme.len_chars() as u32;
      return Some(GraphemeWithSource::new(
        grapheme_str(grapheme),
        col,
        self.text_format.tab_width,
        GraphemeSource::Document { codepoints },
      ));
    }

    if self.exhausted {
      None
    } else {
      self.exhausted = true;
      Some(GraphemeWithSource::eof())
    }
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
    self.text_format.soft_wrap_at_text_width
      && self
        .peek_grapheme(col, char_pos)
        .is_some_and(|grapheme| grapheme.is_newline() || grapheme.is_eof())
  }

  fn hard_wrap_limit_exceeded(&self, word_width: usize) -> bool {
    word_width > self.text_format.max_wrap as usize
  }

  fn wrap_word(&mut self) -> usize {
    let indent_carry_over = if let Some(indent) = self.indent_level {
      if indent as u16 <= self.text_format.max_indent_retain {
        indent as u16
      } else {
        0
      }
    } else {
      self.indent_level = Some(0);
      0
    };

    self.visual_pos.row += 1;
    self.visual_pos.col = indent_carry_over as usize;

    let mut word_width = 0;
    let indicator_len = self.text_format.wrap_indicator_graphemes().len();
    let wrap_indicator = self
      .text_format
      .wrap_indicator_graphemes()
      .iter()
      .map(|grapheme| {
        let grapheme = GraphemeWithSource::new(
          grapheme.clone(),
          self.visual_pos.col + word_width,
          self.text_format.tab_width,
          GraphemeSource::VirtualText,
        );
        word_width += grapheme.width();
        grapheme
      });
    self.word_buf.splice(0..0, wrap_indicator);

    for grapheme in &mut self.word_buf[indicator_len..] {
      let visual_x = self.visual_pos.col + word_width;
      grapheme
        .grapheme
        .change_position(visual_x, self.text_format.tab_width);
      word_width += grapheme.width();
    }

    if let Some(grapheme) = &mut self.peeked_grapheme {
      let visual_x = self.visual_pos.col + word_width;
      grapheme
        .grapheme
        .change_position(visual_x, self.text_format.tab_width);
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
      match col.cmp(&self.wrap_width) {
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
        Ordering::Less => {},
      }

      let Some(grapheme) = self.next_grapheme(col, char_pos) else {
        return;
      };
      word_chars += grapheme.doc_chars();

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

impl<'a> Iterator for DocumentFormatter<'a> {
  type Item = FormattedGrapheme<'a>;

  fn next(&mut self) -> Option<Self::Item> {
    let grapheme = if self.text_format.soft_wrap {
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

    let formatted = FormattedGrapheme {
      raw:        grapheme.grapheme,
      source:     grapheme.source,
      visual_pos: self.visual_pos,
      line_idx:   self.line_pos,
      char_idx:   self.char_pos,
    };

    self.char_pos += formatted.doc_chars();

    if formatted.raw == Grapheme::Newline {
      self.visual_pos.col += 1;
      self.visual_pos.row += 1;
      self.visual_pos.col = 0;
      if !formatted.is_virtual() {
        self.line_pos += 1;
      }
    } else {
      self.visual_pos.col += formatted.width();
    }

    Some(formatted)
  }
}

fn grapheme_str<'a>(grapheme: RopeSlice<'a>) -> GraphemeStr<'a> {
  match grapheme.as_str() {
    Some(slice) => GraphemeStr::from(slice),
    None => GraphemeStr::from(grapheme.to_string()),
  }
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::{
    DocumentFormatter,
    FormattedGrapheme,
    GraphemeSource,
    Position,
  };
  use crate::{
    grapheme::Grapheme,
    render::text_format::TextFormat,
  };

  fn collect_visible(formatter: DocumentFormatter<'_>) -> Vec<FormattedGrapheme<'_>> {
    formatter
      .filter(|grapheme| !grapheme.source.is_eof())
      .collect()
  }

  #[test]
  fn grapheme_source_reports_virtual_and_eof() {
    assert!(GraphemeSource::VirtualText.is_virtual());
    assert!(!GraphemeSource::Document { codepoints: 1 }.is_virtual());
    assert!(GraphemeSource::Document { codepoints: 0 }.is_eof());
    assert_eq!(GraphemeSource::Document { codepoints: 3 }.doc_chars(), 3);
  }

  #[test]
  fn formatter_emits_eof_sentinel() {
    let rope = Rope::from("hi");
    let format = TextFormat::default();
    let items = DocumentFormatter::new(rope.slice(..), &format, 20).collect::<Vec<_>>();

    assert!(
      items
        .last()
        .is_some_and(|grapheme| grapheme.source.is_eof())
    );
  }

  #[test]
  fn formatter_tracks_visual_positions() {
    let rope = Rope::from("a\nb");
    let format = TextFormat::default();
    let items = collect_visible(DocumentFormatter::new(rope.slice(..), &format, 20));

    assert_eq!(items[0].visual_pos, Position::new(0, 0));
    assert_eq!(items[1].raw, Grapheme::Newline);
    assert_eq!(items[1].visual_pos, Position::new(0, 1));
    assert_eq!(items[2].visual_pos, Position::new(1, 0));
  }

  #[test]
  fn formatter_emits_wrap_indicator() {
    let rope = Rope::from("hello world");
    let mut format = TextFormat::default();
    format.soft_wrap = true;
    format.set_wrap_indicator("↪");

    let items = collect_visible(DocumentFormatter::new(rope.slice(..), &format, 5));

    assert!(items.iter().any(|grapheme| {
      grapheme.is_virtual() && grapheme.raw == Grapheme::Other { g: "↪".into() }
    }));
  }
}
