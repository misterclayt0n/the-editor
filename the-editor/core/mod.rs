use std::num::NonZeroUsize;

use smartstring::{
  LazyCompact,
  SmartString,
};

pub mod chars;
pub mod commands;
pub mod doc_formatter;
pub mod document;
pub mod grapheme;
pub mod line_ending;
pub mod movement;
pub mod position;
pub mod selection;
pub mod syntax;
pub mod text_annotations;
pub mod text_format;
pub mod transaction;

/// This type basically optimizes small string operations by doing expensive
/// operations in heap, and using inline storage for small strings to avoid heap
/// allocations.
/// Hence why it's called `SmartString`.
pub type Tendril = SmartString<LazyCompact>;
