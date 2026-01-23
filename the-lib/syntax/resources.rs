//! Abstract access to tree-sitter grammars and query sources.
//!
//! The syntax core does not assume a runtime filesystem layout. Instead, the
//! caller supplies a `SyntaxResources` implementation that can load grammars
//! and query text from any source (embedded assets, on-disk runtime, etc).
//!
//! # Example: custom in-memory resources
//!
//! ```no_run
//! use std::borrow::Cow;
//! use std::collections::HashMap;
//!
//! use the_lib::syntax::resources::{QueryKind, SyntaxResources};
//! use tree_house::tree_sitter::Grammar;
//!
//! struct InMemoryResources {
//!   grammars: HashMap<String, Grammar>,
//!   queries: HashMap<(String, QueryKind), String>,
//! }
//!
//! impl SyntaxResources for InMemoryResources {
//!   fn grammar(&self, grammar_name: &str) -> Option<Grammar> {
//!     self.grammars.get(grammar_name).copied()
//!   }
//!
//!   fn query(&self, language_id: &str, kind: QueryKind) -> Option<Cow<'_, str>> {
//!     self
//!       .queries
//!       .get(&(language_id.to_string(), kind))
//!       .map(|s| Cow::Borrowed(s.as_str()))
//!   }
//! }
//! ```
use std::borrow::Cow;

use tree_house::tree_sitter::Grammar;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryKind {
  Highlights,
  Injections,
  Locals,
  Indents,
  TextObjects,
  Tags,
  Rainbows,
}

impl QueryKind {
  pub fn filename(self) -> &'static str {
    match self {
      Self::Highlights => "highlights.scm",
      Self::Injections => "injections.scm",
      Self::Locals => "locals.scm",
      Self::Indents => "indents.scm",
      Self::TextObjects => "textobjects.scm",
      Self::Tags => "tags.scm",
      Self::Rainbows => "rainbows.scm",
    }
  }
}

pub trait SyntaxResources: Send + Sync {
  fn grammar(&self, grammar_name: &str) -> Option<Grammar>;
  fn query(&self, language_id: &str, kind: QueryKind) -> Option<Cow<'_, str>>;
}

#[derive(Debug, Default)]
pub struct NullResources;

impl SyntaxResources for NullResources {
  fn grammar(&self, _grammar_name: &str) -> Option<Grammar> {
    None
  }

  fn query(&self, _language_id: &str, _kind: QueryKind) -> Option<Cow<'_, str>> {
    None
  }
}
