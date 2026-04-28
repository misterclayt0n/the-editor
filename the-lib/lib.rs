use smartstring::{
  LazyCompact,
  SmartString,
};

pub mod auto_pairs;
pub mod chars;
pub mod grapheme;
pub mod history;
pub mod line_ending;
pub mod movement;
pub mod selection;
pub mod syntax;
pub mod transaction;
pub mod uri;
pub mod object;
pub mod match_brackets;

/// A small-string-optimized string type.
///
/// Strings up to ~23 bytes are stored inline without heap allocation.
/// This is the primary string type used throughout the library for
/// text fragments, insertions, and other small strings.
pub type Tendril = SmartString<LazyCompact>;
