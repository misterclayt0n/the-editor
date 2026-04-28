use smartstring::{
  LazyCompact,
  SmartString,
};

pub mod auto_pairs;
pub mod chars;
pub mod comment;
pub mod document;
pub mod edit;
pub mod grapheme;
pub mod history;
pub mod indent;
pub mod line_ending;
pub mod match_brackets;
pub mod movement;
pub mod object;
pub mod render;
pub mod search;
pub mod selection;
pub mod surround;
pub mod syntax;
pub mod text_object;
pub mod transaction;
pub mod uri;

/// A small-string-optimized string type.
///
/// Strings up to ~23 bytes are stored inline without heap allocation.
/// This is the primary string type used throughout the library for
/// text fragments, insertions, and other small strings.
pub type Tendril = SmartString<LazyCompact>;
