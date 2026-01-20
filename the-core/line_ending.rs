use ropey::{Rope, RopeSlice};
pub use unicode_linebreak::BreakOpportunity;

#[cfg(target_os = "windows")]
pub const NATIVE_LINE_ENDING: LineEnding = LineEnding::Crlf;

#[cfg(not(target_os = "windows"))]
pub const NATIVE_LINE_ENDING: LineEnding = LineEnding::LF;

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum LineEnding {
  /// CarriageReturn followed by LineFeed.
  Crlf,

  /// U+000A -- LineFeed
  LF,

  #[cfg(feature = "unicode-lines")]
  /// U+000B -- VerticalTab
  VT,

  #[cfg(feature = "unicode-lines")]
  /// U+000C -- FormFeed
  FF,

  #[cfg(feature = "unicode-lines")]
  /// U+000D -- CarriageReturn
  CR,

  #[cfg(feature = "unicode-lines")]
  /// U+0085 -- NextLine
  Nel,

  /// U+2028 -- Line Separator
  #[cfg(feature = "unicode-lines")]
  LS,

  /// U+2029 -- ParagraphSeparator
  #[cfg(feature = "unicode-lines")]
  PS,
}

impl LineEnding {
  #[inline]
  pub const fn len_chars(&self) -> usize {
    match self {
      Self::Crlf => 2,
      _ => 1,
    }
  }

  #[inline]
  pub const fn as_str(&self) -> &'static str {
    match self {
      Self::Crlf => "\u{000D}\u{000A}",
      Self::LF => "\u{000A}",
      #[cfg(feature = "unicode-lines")]
      Self::VT => "\u{000B}",
      #[cfg(feature = "unicode-lines")]
      Self::FF => "\u{000C}",
      #[cfg(feature = "unicode-lines")]
      Self::CR => "\u{000D}",
      #[cfg(feature = "unicode-lines")]
      Self::Nel => "\u{0085}",
      #[cfg(feature = "unicode-lines")]
      Self::LS => "\u{2028}",
      #[cfg(feature = "unicode-lines")]
      Self::PS => "\u{2029}",
    }
  }

  #[inline]
  pub const fn from_char(ch: char) -> Option<LineEnding> {
    match ch {
      '\u{000A}' => Some(LineEnding::LF),
      #[cfg(feature = "unicode-lines")]
      '\u{000B}' => Some(LineEnding::VT),
      #[cfg(feature = "unicode-lines")]
      '\u{000C}' => Some(LineEnding::FF),
      #[cfg(feature = "unicode-lines")]
      '\u{000D}' => Some(LineEnding::CR),
      #[cfg(feature = "unicode-lines")]
      '\u{0085}' => Some(LineEnding::Nel),
      #[cfg(feature = "unicode-lines")]
      '\u{2028}' => Some(LineEnding::LS),
      #[cfg(feature = "unicode-lines")]
      '\u{2029}' => Some(LineEnding::PS),
      _ => None,
    }
  }

  // Normally we'd want to implement the FromStr trait, but in this case
  // that would force us into a different return type than from_char or
  // or from_rope_slice, which would be weird.
  #[allow(clippy::should_implement_trait)]
  #[inline]
  pub fn from_str(g: &str) -> Option<LineEnding> {
    match g {
      "\u{000D}\u{000A}" => Some(LineEnding::Crlf),
      "\u{000A}" => Some(LineEnding::LF),
      #[cfg(feature = "unicode-lines")]
      "\u{000B}" => Some(LineEnding::VT),
      #[cfg(feature = "unicode-lines")]
      "\u{000C}" => Some(LineEnding::FF),
      #[cfg(feature = "unicode-lines")]
      "\u{000D}" => Some(LineEnding::CR),
      #[cfg(feature = "unicode-lines")]
      "\u{0085}" => Some(LineEnding::Nel),
      #[cfg(feature = "unicode-lines")]
      "\u{2028}" => Some(LineEnding::LS),
      #[cfg(feature = "unicode-lines")]
      "\u{2029}" => Some(LineEnding::PS),
      _ => None,
    }
  }

  #[inline]
  pub fn from_rope_slice(g: &RopeSlice) -> Option<LineEnding> {
    if let Some(text) = g.as_str() {
      LineEnding::from_str(text)
    } else {
      // Non-contiguous, so it can't be a line ending.
      // Specifically, Ropey guarantees that CRLF is always
      // contiguous. And the remaining line endings are all
      // single `char`s, and therefore trivially contiguous.
      None
    }
  }
}

#[inline]
pub fn str_is_line_ending(s: &str) -> bool {
  LineEnding::from_str(s).is_some()
}

#[inline]
pub fn rope_is_line_ending(r: RopeSlice) -> bool {
  r.chunks().all(str_is_line_ending)
}

/// Attempts to detect what line ending the passed document uses.
pub fn auto_detect_line_ending(doc: &Rope) -> Option<LineEnding> {
  // Return first matched line ending. Not all possible line endings
  // are being matched, as they might be special-use only.
  for line in doc.lines().take(100) {
    match get_line_ending(&line) {
      None => {},
      #[cfg(feature = "unicode-lines")]
      Some(LineEnding::VT) | Some(LineEnding::FF) | Some(LineEnding::PS) => {},
      ending => return ending,
    }
  }

  None
}

/// Returns the passed line's line ending, if any.
pub fn get_line_ending(line: &RopeSlice) -> Option<LineEnding> {
  // Last char as str.
  let g1 = line
    .slice(line.len_chars().saturating_sub(1)..)
    .as_str()
    .unwrap();

  // Last 2 chars as str, or empty str if they're not contiguous.
  // It's fine to punt on the non-contiguous case, because Ropey guarantees
  // that CRLF is always contiguous.
  let g2 = line
    .slice(line.len_chars().saturating_sub(2)..)
    .as_str()
    .unwrap_or("");

  // First check the 2-character case for CRLF, then check the single-character
  // case.
  LineEnding::from_str(g2).or_else(|| LineEnding::from_str(g1))
}

#[cfg(not(feature = "unicode-lines"))]
/// Returns the passed line's line ending, if any.
pub fn get_line_ending_of_str(line: &str) -> Option<LineEnding> {
  if line.ends_with("\u{000D}\u{000A}") {
    Some(LineEnding::Crlf)
  } else if line.ends_with('\u{000A}') {
    Some(LineEnding::LF)
  } else {
    None
  }
}

#[cfg(feature = "unicode-lines")]
/// Returns the passed line's line ending, if any.
pub fn get_line_ending_of_str(line: &str) -> Option<LineEnding> {
  if line.ends_with("\u{000D}\u{000A}") {
    Some(LineEnding::Crlf)
  } else if line.ends_with('\u{000A}') {
    Some(LineEnding::LF)
  } else if line.ends_with('\u{000B}') {
    Some(LineEnding::VT)
  } else if line.ends_with('\u{000C}') {
    Some(LineEnding::FF)
  } else if line.ends_with('\u{000D}') {
    Some(LineEnding::CR)
  } else if line.ends_with('\u{0085}') {
    Some(LineEnding::Nel)
  } else if line.ends_with('\u{2028}') {
    Some(LineEnding::LS)
  } else if line.ends_with('\u{2029}') {
    Some(LineEnding::PS)
  } else {
    None
  }
}

/// Returns the char index of the end of the given line, not including its line
/// ending.
pub fn line_end_char_index(slice: &RopeSlice, line: usize) -> usize {
  slice.line_to_char(line + 1)
    - get_line_ending(&slice.line(line))
      .map(|le| le.len_chars())
      .unwrap_or(0)
}

pub fn line_end_byte_index(slice: &RopeSlice, line: usize) -> usize {
  slice.line_to_byte(line + 1)
    - get_line_ending(&slice.line(line))
      .map(|le| le.as_str().len())
      .unwrap_or(0)
}

/// Get line `line_idx` from the passed rope slice, sans any line ending.
pub fn line_without_line_ending<'a>(slice: &'a RopeSlice, line_idx: usize) -> RopeSlice<'a> {
  let start = slice.line_to_char(line_idx);
  let end = line_end_char_index(slice, line_idx);
  slice.slice(start..end)
}

/// Returns the char index of the end of the given RopeSlice, not including any
/// final line ending.
pub fn rope_end_without_line_ending(slice: &RopeSlice) -> usize {
  slice.len_chars() - get_line_ending(slice).map(|le| le.len_chars()).unwrap_or(0)
}

/// Returns an iterator over soft line break opportunities in the given string.
///
/// Each item is a `(byte_index, BreakOpportunity)` pair indicating where a line
/// break may or must occur.
///
/// # Example
/// ```ignore
/// use the_core::line_ending::{soft_breaks, BreakOpportunity};
///
/// let text = "hello world";
/// for (idx, opportunity) in soft_breaks(text) {
///   match opportunity {
///     BreakOpportunity::Mandatory => println!("Must break at byte {idx}"),
///     BreakOpportunity::Allowed => println!("May break at byte {idx}"),
///   }
/// }
/// ```
#[inline]
pub fn soft_breaks(text: &str) -> impl Iterator<Item = (usize, BreakOpportunity)> + '_ {
  unicode_linebreak::linebreaks(text)
}

/// Check if a soft line break is allowed after the given character.
///
/// This is a simplified single-character check. For accurate results with full
/// context, use `soft_breaks()` on the complete string instead.
///
/// # Note
/// Without context from surrounding characters, this function uses conservative
/// defaults based on the character's Unicode line break class. It will return
/// `true` for characters that typically allow breaks after them (spaces,
/// hyphens, CJK characters, etc.).
#[inline]
pub fn char_can_break_after(ch: char) -> bool {
  use unicode_linebreak::BreakClass;

  match unicode_linebreak::break_property(ch as u32) {
    // Spaces and breaking whitespace
    BreakClass::Space |
    BreakClass::Mandatory |
    BreakClass::CarriageReturn |
    BreakClass::LineFeed |
    BreakClass::NextLine |
    BreakClass::ZeroWidthSpace
      => true,

    // Hyphens and breaking punctuation
    BreakClass::Hyphen |
    BreakClass::After  // Break after
      => true,

    // CJK ideographs (can break between them)
    BreakClass::Ideographic |
    BreakClass::ConditionalJapaneseStarter
      => true,

    // Characters that prohibit breaks
    BreakClass::NonBreakingGlue |
    BreakClass::WordJoiner |
    BreakClass::Inseparable |
    BreakClass::Before |  // Break before (not after)
    BreakClass::NonStarter
      => false,

    // Alphabetic and numeric - generally don't break between them
    BreakClass::Alphabetic |
    BreakClass::Numeric |
    BreakClass::HebrewLetter
      => false,

    // Default: don't allow break (conservative)
    _ => false,
  }
}

/// Check if a soft line break is allowed between two characters.
///
/// This provides more accurate results than `char_can_break_after()` by
/// considering both characters in the pair.
///
/// # Note
/// For the most accurate line breaking, use `soft_breaks()` on the complete
/// string, as the full Unicode Line Breaking Algorithm considers more context.
#[inline]
pub fn can_break_between(before: char, after: char) -> bool {
  use unicode_linebreak::BreakClass;

  let before_class = unicode_linebreak::break_property(before as u32);
  let after_class = unicode_linebreak::break_property(after as u32);

  // Mandatory breaks
  if matches!(
    before_class,
    BreakClass::Mandatory
      | BreakClass::CarriageReturn
      | BreakClass::LineFeed
      | BreakClass::NextLine
  ) {
    return true;
  }

  // Never break before certain characters
  if matches!(
    after_class,
    BreakClass::NonBreakingGlue
      | BreakClass::WordJoiner
      | BreakClass::ClosePunctuation
      | BreakClass::CloseParenthesis
      | BreakClass::Exclamation
      | BreakClass::InfixSeparator
      | BreakClass::Symbol
  ) {
    return false;
  }

  // Never break after certain characters
  if matches!(
    before_class,
    BreakClass::NonBreakingGlue
      | BreakClass::WordJoiner
      | BreakClass::OpenPunctuation
      | BreakClass::Quotation
  ) {
    return false;
  }

  // Break after spaces
  if before_class == BreakClass::Space {
    return true;
  }

  // Break after hyphens
  if matches!(before_class, BreakClass::Hyphen | BreakClass::After) {
    return true;
  }

  // CJK: can break between ideographs
  if matches!(
    before_class,
    BreakClass::Ideographic | BreakClass::ConditionalJapaneseStarter
  ) || matches!(
    after_class,
    BreakClass::Ideographic | BreakClass::ConditionalJapaneseStarter
  ) {
    return true;
  }

  // Default: don't break between alphabetic/numeric sequences
  false
}

#[cfg(test)]
mod line_ending_tests {
  use super::*;

  #[test]
  fn line_ending_autodetect() {
    assert_eq!(
      auto_detect_line_ending(&Rope::from_str("\n")),
      Some(LineEnding::LF)
    );
    assert_eq!(
      auto_detect_line_ending(&Rope::from_str("\r\n")),
      Some(LineEnding::Crlf)
    );
    assert_eq!(auto_detect_line_ending(&Rope::from_str("hello")), None);
    assert_eq!(auto_detect_line_ending(&Rope::from_str("")), None);
    assert_eq!(
      auto_detect_line_ending(&Rope::from_str("hello\nhelix\r\n")),
      Some(LineEnding::LF)
    );
    assert_eq!(
      auto_detect_line_ending(&Rope::from_str("a formfeed\u{000C}")),
      None
    );
    assert_eq!(
      auto_detect_line_ending(&Rope::from_str("\n\u{000A}\n \u{000A}")),
      Some(LineEnding::LF)
    );
    assert_eq!(
      auto_detect_line_ending(&Rope::from_str(
        "a formfeed\u{000C} with a\u{000C} linefeed\u{000A}"
      )),
      Some(LineEnding::LF)
    );
    assert_eq!(
      auto_detect_line_ending(&Rope::from_str(
        "a formfeed\u{000C} with a\u{000C} carriage return linefeed\u{000D}\u{000A} and a \
         linefeed\u{000A}"
      )),
      Some(LineEnding::Crlf)
    );
  }

  #[test]
  fn str_to_line_ending() {
    #[cfg(feature = "unicode-lines")]
    assert_eq!(LineEnding::from_str("\r"), Some(LineEnding::CR));
    assert_eq!(LineEnding::from_str("\n"), Some(LineEnding::LF));
    assert_eq!(LineEnding::from_str("\r\n"), Some(LineEnding::Crlf));
    assert_eq!(LineEnding::from_str("hello\n"), None);
  }

  #[test]
  fn rope_slice_to_line_ending() {
    let r = Rope::from_str("hello\r\n");

    #[cfg(feature = "unicode-lines")]
    assert_eq!(
      LineEnding::from_rope_slice(&r.slice(5..6)),
      Some(LineEnding::CR)
    );
    assert_eq!(
      LineEnding::from_rope_slice(&r.slice(6..7)),
      Some(LineEnding::LF)
    );
    assert_eq!(
      LineEnding::from_rope_slice(&r.slice(5..7)),
      Some(LineEnding::Crlf)
    );
    assert_eq!(LineEnding::from_rope_slice(&r.slice(..)), None);
  }

  #[test]
  fn get_line_ending_rope_slice() {
    let r = Rope::from_str("Hello\rworld\nhow\r\nare you?");

    #[cfg(feature = "unicode-lines")]
    assert_eq!(get_line_ending(&r.slice(..6)), Some(LineEnding::CR));
    assert_eq!(get_line_ending(&r.slice(..12)), Some(LineEnding::LF));
    assert_eq!(get_line_ending(&r.slice(..17)), Some(LineEnding::Crlf));
    assert_eq!(get_line_ending(&r.slice(..)), None);
  }

  #[test]
  fn get_line_ending_str() {
    let text = "Hello\rworld\nhow\r\nare you?";

    #[cfg(feature = "unicode-lines")]
    assert_eq!(get_line_ending_of_str(&text[..6]), Some(LineEnding::CR));
    assert_eq!(get_line_ending_of_str(&text[..12]), Some(LineEnding::LF));
    assert_eq!(get_line_ending_of_str(&text[..17]), Some(LineEnding::Crlf));
    assert_eq!(get_line_ending_of_str(text), None);
  }

  #[test]
  fn line_end_char_index_rope_slice() {
    let r = Rope::from_str("Hello\rworld\nhow\r\nare you?");
    let s = &r.slice(..);

    #[cfg(not(feature = "unicode-lines"))]
    {
      assert_eq!(line_end_char_index(s, 0), 11);
      assert_eq!(line_end_char_index(s, 1), 15);
      assert_eq!(line_end_char_index(s, 2), 25);
    }
    #[cfg(feature = "unicode-lines")]
    {
      assert_eq!(line_end_char_index(s, 0), 5);
      assert_eq!(line_end_char_index(s, 1), 11);
      assert_eq!(line_end_char_index(s, 2), 15);
    }
  }

  #[test]
  fn test_soft_breaks() {
    // Space allows break after
    assert!(char_can_break_after(' '));

    // Newline is mandatory break
    assert!(char_can_break_after('\n'));

    // Letters don't allow breaks
    assert!(!char_can_break_after('a'));
    assert!(!char_can_break_after('Z'));

    // Hyphen allows break after
    assert!(char_can_break_after('-'));

    // CJK characters allow breaks
    assert!(char_can_break_after('漢'));
    assert!(char_can_break_after('字'));

    // Non-breaking space does NOT allow break
    assert!(!char_can_break_after('\u{00A0}'));
  }

  #[test]
  fn test_can_break_between() {
    // Space before letter: can break
    assert!(can_break_between(' ', 'a'));

    // Letter to letter: no break
    assert!(!can_break_between('a', 'b'));

    // CJK to CJK: can break
    assert!(can_break_between('漢', '字'));

    // Hyphen to letter: can break
    assert!(can_break_between('-', 'a'));

    // Letter before close paren: no break
    assert!(!can_break_between('a', ')'));

    // Open paren to letter: no break
    assert!(!can_break_between('(', 'a'));
  }

  #[test]
  fn test_soft_breaks_iterator() {
    let text = "hello world";
    let breaks: Vec<_> = soft_breaks(text).collect();

    // Should have break opportunity after space (at byte 6) and at end
    assert!(breaks.iter().any(|(idx, _)| *idx == 6));
  }
}
