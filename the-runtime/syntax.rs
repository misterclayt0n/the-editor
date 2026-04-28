use std::{
  borrow::Cow,
  fs,
  path::{
    Path,
    PathBuf,
  },
  sync::Arc,
};

use the_lib::{
  document::Document,
  syntax::{
    LanguageAssignment,
    LanguageRegistry,
    QueryKind,
    SyntaxConfiguration,
    SyntaxError,
    SyntaxResources,
    tree_sitter::Grammar,
  },
};

#[derive(Debug)]
pub struct RuntimeSyntax {
  registry: LanguageRegistry,
}

impl RuntimeSyntax {
  pub fn disabled() -> Self {
    Self::new(
      SyntaxConfiguration {
        language: Vec::new(),
      },
      the_lib::syntax::NullResources,
    )
    .expect("empty syntax registry should be valid")
  }

  pub fn new(
    config: SyntaxConfiguration,
    resources: impl SyntaxResources + 'static,
  ) -> Result<Self, SyntaxError> {
    LanguageRegistry::new(config, resources).map(|registry| Self { registry })
  }

  pub fn with_resources(
    config: SyntaxConfiguration,
    resources: Arc<dyn SyntaxResources>,
  ) -> Result<Self, SyntaxError> {
    LanguageRegistry::with_resources(config, resources).map(|registry| Self { registry })
  }

  pub fn from_toml_str(
    text: &str,
    resources: impl SyntaxResources + 'static,
  ) -> Result<Self, RuntimeSyntaxError> {
    let config = toml::from_str(text)?;
    Self::new(config, resources).map_err(Into::into)
  }

  pub fn from_toml_file(
    path: &Path,
    resources: impl SyntaxResources + 'static,
  ) -> Result<Self, RuntimeSyntaxError> {
    let text = fs::read_to_string(path)?;
    Self::from_toml_str(&text, resources)
  }

  pub fn registry(&self) -> &LanguageRegistry {
    &self.registry
  }

  pub fn detect_language_for_path(
    &self,
    path: Option<&Path>,
    document: &Document,
  ) -> Option<LanguageAssignment> {
    self
      .registry
      .detect_language_for_document(path, document.text().slice(..))
  }

  pub fn activate_document(&self, document: &mut Document, path: Option<&Path>) {
    if let Some(assignment) = self.detect_language_for_path(path, document) {
      document.set_syntax(
        self
          .registry
          .activate_document_syntax(document.text().slice(..), assignment),
      );
    } else {
      document.clear_syntax();
    }
  }
}

#[derive(Debug, Clone)]
pub struct FilesystemSyntaxResources {
  grammar_dir: PathBuf,
  query_dir:   PathBuf,
}

impl FilesystemSyntaxResources {
  pub fn new(grammar_dir: impl Into<PathBuf>, query_dir: impl Into<PathBuf>) -> Self {
    Self {
      grammar_dir: grammar_dir.into(),
      query_dir:   query_dir.into(),
    }
  }

  fn grammar_candidates(&self, grammar_name: &str) -> [PathBuf; 5] {
    let ext = std::env::consts::DLL_EXTENSION;
    let dash_name = grammar_name.replace('_', "-");
    let underscore_name = grammar_name.replace('-', "_");
    [
      self.grammar_dir.join(format!("{grammar_name}.{ext}")),
      self.grammar_dir.join(format!("lib{grammar_name}.{ext}")),
      self
        .grammar_dir
        .join(format!("tree-sitter-{dash_name}.{ext}")),
      self
        .grammar_dir
        .join(format!("libtree-sitter-{dash_name}.{ext}")),
      self.grammar_dir.join(format!("{underscore_name}.{ext}")),
    ]
  }
}

impl SyntaxResources for FilesystemSyntaxResources {
  fn grammar(&self, grammar_name: &str) -> Option<Grammar> {
    self
      .grammar_candidates(grammar_name)
      .into_iter()
      .find(|path| path.exists())
      .and_then(|path| unsafe { Grammar::new(grammar_name, &path).ok() })
  }

  fn query(&self, language_id: &str, kind: QueryKind) -> Option<Cow<'_, str>> {
    let path = self.query_dir.join(language_id).join(kind.filename());
    fs::read_to_string(path).ok().map(Cow::Owned)
  }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeSyntaxError {
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error(transparent)]
  Toml(#[from] toml::de::Error),
  #[error(transparent)]
  Syntax(#[from] SyntaxError),
}

#[cfg(test)]
mod tests {
  use tempfile::tempdir;
  use the_lib::syntax::{
    DocumentSyntax,
    DocumentSyntaxState,
  };

  use super::*;

  #[test]
  fn disabled_runtime_syntax_leaves_document_unassigned() {
    let mut document = Document::new("hello".into());

    RuntimeSyntax::disabled().activate_document(&mut document, None);

    assert!(matches!(document.syntax(), DocumentSyntax::Unassigned));
  }

  #[test]
  fn runtime_syntax_detects_path_and_marks_missing_grammar_unavailable() {
    let mut document = Document::new("fn main() {}\n".into());
    let runtime = RuntimeSyntax::from_toml_str(
      r#"
        [[language]]
        name = "rust"
        scope = "source.rust"
        file-types = ["rs"]
      "#,
      the_lib::syntax::NullResources,
    )
    .unwrap();

    runtime.activate_document(&mut document, Some(Path::new("main.rs")));

    assert!(matches!(document.syntax(), DocumentSyntax::Assigned {
      state: DocumentSyntaxState::Unavailable(_),
      ..
    }));
  }

  #[test]
  fn filesystem_syntax_resources_reads_query_files() {
    let grammar_dir = tempdir().unwrap();
    let query_dir = tempdir().unwrap();
    let rust_dir = query_dir.path().join("rust");
    fs::create_dir_all(&rust_dir).unwrap();
    fs::write(rust_dir.join("highlights.scm"), "(identifier) @variable").unwrap();

    let resources = FilesystemSyntaxResources::new(grammar_dir.path(), query_dir.path());
    let text = resources.query("rust", QueryKind::Highlights).unwrap();

    assert_eq!(text, Cow::Borrowed("(identifier) @variable"));
  }
}
