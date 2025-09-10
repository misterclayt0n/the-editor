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
pub mod view;
pub mod case_conversion;
pub mod indent;
pub mod config;
pub mod editor_config;
pub mod history;
pub mod diagnostics;
pub mod clipboard;
pub mod graphics;
pub mod uri;
pub mod diff;
pub mod command_line;
pub mod auto_pairs;
pub mod theme;
pub mod tree;
pub mod registers;
pub mod macros;
pub mod info;
pub mod fuzzy;

/// This type basically optimizes small string operations by doing expensive
/// operations in heap, and using inline storage for small strings to avoid heap
/// allocations.
/// Hence why it's called `SmartString`.
pub type Tendril = SmartString<LazyCompact>;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DocumentId(pub NonZeroUsize);

impl Default for DocumentId {
  fn default() -> Self {
    // SAFETY: 1 is non-zero.
    DocumentId(unsafe { NonZeroUsize::new_unchecked(1) })
  }
}

impl std::fmt::Display for DocumentId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!("{}", self.0))
  }
}

slotmap::new_key_type! {
  pub struct ViewId;
}
