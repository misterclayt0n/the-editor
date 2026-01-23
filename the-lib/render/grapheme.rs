use crate::{position::Position, syntax::Highlight};
use the_core::grapheme::Grapheme;

/// Source information for a formatted grapheme.
#[derive(Debug, Clone, Copy)]
pub enum GraphemeSource {
  Document {
    codepoints: u32,
  },
  /// Inline virtual text cannot be highlighted with a `Highlight` iterator
  /// because it's not part of the document. Instead the `Highlight` is emitted
  /// directly by the document formatter.
  VirtualText {
    highlight: Option<Highlight>,
  },
}

impl GraphemeSource {
  /// Returns whether this grapheme is virtual inline text.
  pub fn is_virtual(self) -> bool {
    matches!(self, GraphemeSource::VirtualText { .. })
  }

  pub fn is_eof(self) -> bool {
    // All doc chars except the EOF char have non-zero codepoints.
    matches!(self, GraphemeSource::Document { codepoints: 0 })
  }

  pub fn doc_chars(self) -> usize {
    match self {
      GraphemeSource::Document { codepoints } => codepoints as usize,
      GraphemeSource::VirtualText { .. } => 0,
    }
  }
}

/// A grapheme that has been formatted and placed on a visual grid.
#[derive(Debug, Clone)]
pub struct FormattedGrapheme<'a> {
  pub raw: Grapheme<'a>,
  pub source: GraphemeSource,
  pub visual_pos: Position,
  /// Document line at the start of the grapheme.
  pub line_idx: usize,
  /// Document char position at the start of the grapheme.
  pub char_idx: usize,
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
