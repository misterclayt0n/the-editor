#[cfg(feature = "runtime-loader")] use std::borrow::Cow;

#[cfg(feature = "runtime-loader")]
use the_editor_loader::grammar::{
  get_language,
  load_runtime_file,
};

#[cfg(feature = "runtime-loader")]
use crate::syntax::resources::{
  QueryKind,
  SyntaxResources,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct RuntimeLoader;

impl RuntimeLoader {
  pub fn new() -> Self {
    Self
  }
}

#[cfg(feature = "runtime-loader")]
impl SyntaxResources for RuntimeLoader {
  fn grammar(&self, grammar_name: &str) -> Option<tree_house::tree_sitter::Grammar> {
    get_language(grammar_name).ok().flatten()
  }

  fn query(&self, language_id: &str, kind: QueryKind) -> Option<Cow<'_, str>> {
    load_runtime_file(language_id, kind.filename())
      .ok()
      .map(Cow::Owned)
  }
}
