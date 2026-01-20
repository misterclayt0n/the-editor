use crate::line_ending::
  LineEnding
;

#[derive(Debug, Eq, PartialEq)]
pub enum CharCategory {
  Whitespace,
  Eol,
  Word,
  Punctuation,
  Unknown,
}

pub fn categorize_char(ch: char) -> CharCategory {
  match ch {
    c if char_is_line_ending(c) => CharCategory::Eol,
    c if c.is_whitespace() => CharCategory::Whitespace,
    c if char_is_word(c) => CharCategory::Word,
    c if char_is_punctuation(c) => CharCategory::Punctuation,
    _ => CharCategory::Unknown,
  }
}

#[inline]
pub fn char_is_line_ending(ch: char) -> bool {
  LineEnding::from_char(ch).is_some()
}

#[inline]
pub fn char_is_whitespace(ch: char) -> bool {
  // TODO: This is a naive binary categorization of whitespace
  // characters. For display, word wrapping, etc. we'll need a better
  // categorization based on e.g. breaking vs non-breaking spaces
  // and whether they're zero-width or not.
  match ch {
      //'\u{1680}' | // Ogham Space Mark (here for completeness, but usually displayed as a dash, not as whitespace)
      '\u{0009}' | // Character Tabulation
      '\u{0020}' | // Space
      '\u{00A0}' | // No-break Space
      '\u{180E}' | // Mongolian Vowel Separator
      '\u{202F}' | // Narrow No-break Space
      '\u{205F}' | // Medium Mathematical Space
      '\u{3000}' | // Ideographic Space
      '\u{FEFF}'   // Zero Width No-break Space
      => true,

      // En Quad, Em Quad, En Space, Em Space, Three-per-em Space,
      // Four-per-em Space, Six-per-em Space, Figure Space,
      // Punctuation Space, Thin Space, Hair Space, Zero Width Space.
      ch if ('\u{2000}' ..= '\u{200B}').contains(&ch) => true,

      _ => false,
    }
}

#[inline]
pub fn char_is_punctuation(ch: char) -> bool {
  use unicode_general_category::{
    GeneralCategory,
    get_general_category,
  };

  matches!(
    get_general_category(ch),
    GeneralCategory::OtherPunctuation
      | GeneralCategory::OpenPunctuation
      | GeneralCategory::ClosePunctuation
      | GeneralCategory::InitialPunctuation
      | GeneralCategory::FinalPunctuation
      | GeneralCategory::ConnectorPunctuation
      | GeneralCategory::DashPunctuation
      | GeneralCategory::MathSymbol
      | GeneralCategory::CurrencySymbol
      | GeneralCategory::ModifierSymbol
  )
}

#[inline]
pub fn char_is_word(ch: char) -> bool {
  ch.is_alphanumeric() || ch == '_'
}
