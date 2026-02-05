use crate::line_ending::LineEnding;

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

/// Return the previous UTF-8 character boundary at or before `idx`.
///
/// If `idx` is already at a boundary, this returns the start of the
/// previous character (or 0 if at the start).
pub fn prev_char_boundary(s: &str, idx: usize) -> usize {
  if s.is_empty() || idx == 0 {
    return 0;
  }
  let idx = idx.min(s.len());
  let mut prev = 0;
  for (i, _) in s.char_indices() {
    if i >= idx {
      break;
    }
    prev = i;
  }
  prev
}

/// Return the next UTF-8 character boundary after `idx`.
///
/// If `idx` is not on a boundary, this moves to the next valid boundary.
pub fn next_char_boundary(s: &str, idx: usize) -> usize {
  if s.is_empty() {
    return 0;
  }
  let idx = idx.min(s.len());
  let mut iter = s.char_indices();
  while let Some((i, _)) = iter.next() {
    if i == idx {
      return iter.next().map(|(n, _)| n).unwrap_or(s.len());
    }
    if i > idx {
      return i;
    }
  }
  s.len()
}

/// Convert a UTF-8 byte index to a character index (count of chars before it).
pub fn byte_to_char_idx(s: &str, byte_idx: usize) -> usize {
  if s.is_empty() {
    return 0;
  }
  let mut idx = byte_idx.min(s.len());
  while idx > 0 && !s.is_char_boundary(idx) {
    idx -= 1;
  }
  s[..idx].chars().count()
}

#[inline]
pub fn char_is_line_ending(ch: char) -> bool {
  LineEnding::from_char(ch).is_some()
}

/// Whether a whitespace character allows line breaking at its position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakingBehavior {
  Breaking,
  NonBreaking,
}

/// Display width category for whitespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceWidth {
  /// Zero visual width (ZWSP, BOM, etc.)
  Zero,
  /// Standard single-column width.
  Single,
  /// Wider than single column (em space, ideographic space).
  Wide,
  /// Context-dependent width (tab).
  Variable,
}

/// Complete whitespace classification.
///
/// This struct provides detailed information about whitespace characters
/// for use in text rendering, word wrapping, and cursor positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WhitespaceProperties {
  pub breaking: BreakingBehavior,
  pub width:    WhitespaceWidth,
}

impl WhitespaceProperties {
  const fn new(breaking: BreakingBehavior, width: WhitespaceWidth) -> Self {
    Self { breaking, width }
  }

  /// Classify a character's whitespace properties.
  ///
  /// Returns `Some(WhitespaceProperties)` if the character is whitespace,
  /// `None` otherwise.
  #[inline]
  pub const fn of(ch: char) -> Option<Self> {
    use BreakingBehavior::*;
    use WhitespaceWidth::*;

    match ch {
      // Tab - breaking, variable width
      '\u{0009}' => Some(Self::new(Breaking, Variable)),

      // Regular space - breaking, single width
      '\u{0020}' => Some(Self::new(Breaking, Single)),

      // No-Break Space - non-breaking, single width
      '\u{00A0}' => Some(Self::new(NonBreaking, Single)),

      // Mongolian Vowel Separator - non-breaking, zero width
      '\u{180E}' => Some(Self::new(NonBreaking, Zero)),

      // En Quad - breaking, wide
      '\u{2000}' => Some(Self::new(Breaking, Wide)),

      // Em Quad - breaking, wide
      '\u{2001}' => Some(Self::new(Breaking, Wide)),

      // En Space - breaking, wide
      '\u{2002}' => Some(Self::new(Breaking, Wide)),

      // Em Space - breaking, wide
      '\u{2003}' => Some(Self::new(Breaking, Wide)),

      // Three-per-em Space - breaking, single
      '\u{2004}' => Some(Self::new(Breaking, Single)),

      // Four-per-em Space - breaking, single
      '\u{2005}' => Some(Self::new(Breaking, Single)),

      // Six-per-em Space - breaking, single
      '\u{2006}' => Some(Self::new(Breaking, Single)),

      // Figure Space - non-breaking, single (used for aligning digits)
      '\u{2007}' => Some(Self::new(NonBreaking, Single)),

      // Punctuation Space - breaking, single
      '\u{2008}' => Some(Self::new(Breaking, Single)),

      // Thin Space - breaking, single
      '\u{2009}' => Some(Self::new(Breaking, Single)),

      // Hair Space - breaking, single
      '\u{200A}' => Some(Self::new(Breaking, Single)),

      // Zero Width Space - breaking, zero width
      '\u{200B}' => Some(Self::new(Breaking, Zero)),

      // Narrow No-Break Space - non-breaking, single
      '\u{202F}' => Some(Self::new(NonBreaking, Single)),

      // Medium Mathematical Space - breaking, single
      '\u{205F}' => Some(Self::new(Breaking, Single)),

      // Ideographic Space - breaking, wide (full-width space in CJK)
      '\u{3000}' => Some(Self::new(Breaking, Wide)),

      // Zero Width No-Break Space (BOM) - non-breaking, zero width
      '\u{FEFF}' => Some(Self::new(NonBreaking, Zero)),

      // Not whitespace
      _ => None,
    }
  }

  #[inline]
  pub const fn is_breaking(&self) -> bool {
    matches!(self.breaking, BreakingBehavior::Breaking)
  }

  #[inline]
  pub const fn is_zero_width(&self) -> bool {
    matches!(self.width, WhitespaceWidth::Zero)
  }

  #[inline]
  pub const fn is_variable_width(&self) -> bool {
    matches!(self.width, WhitespaceWidth::Variable)
  }

  #[inline]
  pub const fn is_wide(&self) -> bool {
    matches!(self.width, WhitespaceWidth::Wide)
  }
}

#[deprecated(
  since = "0.1.0",
  note = "use WhitespaceProperties::of() for detailed whitespace classification"
)]
#[inline]
pub fn char_is_whitespace(ch: char) -> bool {
  WhitespaceProperties::of(ch).is_some()
}

#[inline]
pub fn char_is_breaking_whitespace(ch: char) -> bool {
  WhitespaceProperties::of(ch)
    .map(|p| p.is_breaking())
    .unwrap_or(false)
}

#[inline]
pub fn char_is_non_breaking_whitespace(ch: char) -> bool {
  WhitespaceProperties::of(ch)
    .map(|p| !p.is_breaking())
    .unwrap_or(false)
}

#[inline]
pub fn char_is_zero_width_whitespace(ch: char) -> bool {
  WhitespaceProperties::of(ch)
    .map(|p| p.is_zero_width())
    .unwrap_or(false)
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

#[cfg(test)]
mod tests {
  use super::{
    byte_to_char_idx,
    next_char_boundary,
    prev_char_boundary,
  };

  #[test]
  fn char_boundaries_utf8() {
    let s = "aé😊";
    assert_eq!(next_char_boundary(s, 0), 1);
    assert_eq!(next_char_boundary(s, 1), 3);
    assert_eq!(next_char_boundary(s, 3), 7);
    assert_eq!(next_char_boundary(s, 7), 7);

    assert_eq!(prev_char_boundary(s, 7), 3);
    assert_eq!(prev_char_boundary(s, 3), 1);
    assert_eq!(prev_char_boundary(s, 1), 0);
    assert_eq!(prev_char_boundary(s, 0), 0);
  }

  #[test]
  fn byte_to_char_index() {
    let s = "aé😊";
    assert_eq!(byte_to_char_idx(s, 0), 0);
    assert_eq!(byte_to_char_idx(s, 1), 1);
    assert_eq!(byte_to_char_idx(s, 3), 2);
    assert_eq!(byte_to_char_idx(s, 7), 3);
    assert_eq!(byte_to_char_idx(s, 8), 3);
  }
}

#[inline]
pub fn char_is_word(ch: char) -> bool {
  ch.is_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod whitespace_tests {
  use super::*;

  #[test]
  fn test_whitespace_classification() {
    // Breaking, single width
    let space = WhitespaceProperties::of(' ').unwrap();
    assert!(space.is_breaking());
    assert_eq!(space.width, WhitespaceWidth::Single);

    // Breaking, variable width
    let tab = WhitespaceProperties::of('\t').unwrap();
    assert!(tab.is_breaking());
    assert!(tab.is_variable_width());

    // Non-breaking
    assert!(!WhitespaceProperties::of('\u{00A0}').unwrap().is_breaking());
    assert!(!WhitespaceProperties::of('\u{2007}').unwrap().is_breaking());

    // Zero-width
    let zwsp = WhitespaceProperties::of('\u{200B}').unwrap();
    assert!(zwsp.is_zero_width() && zwsp.is_breaking());
    let bom = WhitespaceProperties::of('\u{FEFF}').unwrap();
    assert!(bom.is_zero_width() && !bom.is_breaking());

    // Wide
    assert!(WhitespaceProperties::of('\u{2003}').unwrap().is_wide());
    assert!(WhitespaceProperties::of('\u{3000}').unwrap().is_wide());

    // Non-whitespace
    assert!(WhitespaceProperties::of('a').is_none());
    assert!(WhitespaceProperties::of('\n').is_none());
  }

  #[test]
  fn test_convenience_functions() {
    assert!(char_is_breaking_whitespace(' '));
    assert!(!char_is_breaking_whitespace('\u{00A0}'));

    assert!(char_is_non_breaking_whitespace('\u{00A0}'));
    assert!(!char_is_non_breaking_whitespace(' '));

    assert!(char_is_zero_width_whitespace('\u{200B}'));
    assert!(!char_is_zero_width_whitespace(' '));

    assert!(!char_is_breaking_whitespace('a'));
  }
}
