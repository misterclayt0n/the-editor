use std::num::NonZeroUsize;

use smartstring::{
  LazyCompact,
  SmartString,
};

pub mod animation;
pub mod auto_pairs;
pub mod case_conversion;
pub mod chars;
pub mod clipboard;
pub mod command_line;
pub mod command_registry;
pub mod commands;
pub mod comment;
pub mod config;
pub mod context_fade;
pub mod diagnostics;
pub mod diff;
pub mod doc_formatter;
pub mod document;
pub mod editor_config;
pub mod expansion;
pub mod fuzzy;
pub mod global_search;
pub mod grapheme;
pub mod graphics;
pub mod history;
pub mod indent;
pub mod info;
pub mod layout;
pub mod line_ending;
pub mod lsp_commands;
pub mod macros;
pub mod match_brackets;
pub mod movement;
pub mod object;
pub mod position;
pub mod registers;
pub mod search;
pub mod selection;
pub mod special_buffer;
pub mod surround;
pub mod syntax;
pub mod text_annotations;
pub mod text_format;
pub mod textobject;
pub mod theme;
pub mod transaction;
pub mod tree;
pub mod uri;
pub mod view;

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
