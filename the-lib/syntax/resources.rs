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
