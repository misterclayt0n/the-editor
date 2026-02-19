use smartstring::{
  LazyCompact,
  SmartString,
};

pub mod app;
pub mod auto_pairs;
pub mod case_convention;
pub mod clipboard;
pub mod command_line;
pub mod comment;
pub mod diagnostics;
pub mod diff;
pub mod docs_markdown;
pub mod document;
pub mod editor;
pub mod fuzzy;
pub mod history;
pub mod indent;
pub mod match_brackets;
pub mod messages;
pub mod movement;
pub mod object;
pub mod position;
pub mod registers;
pub mod render;
pub mod search;
pub mod selection;
pub mod surround;
pub mod syntax;
pub mod syntax_async;
pub mod text_object;
pub mod transaction;
pub mod view;

/// A small-string-optimized string type.
///
/// Strings up to ~23 bytes are stored inline without heap allocation.
/// This is the primary string type used throughout the library for
/// text fragments, insertions, and other small strings.
pub type Tendril = SmartString<LazyCompact>;
