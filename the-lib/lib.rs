use smartstring::{
  LazyCompact,
  SmartString,
};

pub mod auto_pairs;
pub mod case_convention;
pub mod command_line;
pub mod history;
pub mod movement;
pub mod selection;
pub mod transaction;
pub mod search;

/// A small-string-optimized string type.
///
/// Strings up to ~23 bytes are stored inline without heap allocation.
/// This is the primary string type used throughout the library for
/// text fragments, insertions, and other small strings.
pub type Tendril = SmartString<LazyCompact>;
