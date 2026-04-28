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
  render::SyntaxHighlight,
  syntax::{
    DocumentSyntax,
    DocumentSyntaxState,
    LanguageAssignment,
    LanguageRegistry,
    QueryKind,
    SyntaxConfiguration,
    SyntaxError,
    SyntaxResources,
    SyntaxState,
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

  pub fn reparse_document(&self, document: &mut Document) -> Result<bool, RuntimeSyntaxError> {
    let Some(state) = active_syntax_state(document.syntax()) else {
      return Ok(false);
    };

    if !state.needs_reparse() && !state.is_parse_pending() {
      return Ok(false);
    }

    let mut snapshot = state.snapshot(document.text().clone());
    snapshot
      .syntax
      .reparse(snapshot.source.slice(..), &self.registry)
      .map_err(|err| RuntimeSyntaxError::Backend(err.to_string()))?;
    Ok(state.apply_parsed(snapshot))
  }

  pub fn collect_document_highlights(&self, document: &Document) -> Vec<SyntaxHighlight> {
    let Some(state) = active_syntax_state(document.syntax()) else {
      return Vec::new();
    };

    let syntax = state.syntax();
    let text = document.text();
    let byte_highlights =
      syntax.collect_highlights(text.slice(..), &self.registry, 0..text.len_bytes());

    byte_highlights
      .into_iter()
      .filter_map(|(kind, range)| {
        let start = text.byte_to_char(range.start);
        let end = text.byte_to_char(range.end);
        (start < end).then_some(SyntaxHighlight {
          range: start..end,
          kind,
        })
      })
      .collect()
  }
}

fn active_syntax_state(syntax: &DocumentSyntax) -> Option<SyntaxState> {
  match syntax {
    DocumentSyntax::Assigned {
      state: DocumentSyntaxState::Active(state),
      ..
    } => Some(state.clone()),
    DocumentSyntax::Unassigned | DocumentSyntax::Assigned { .. } => None,
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
  #[error("syntax backend error: {0}")]
  Backend(String),
}

#[cfg(test)]
mod tests {
  use std::borrow::Cow;

  use tempfile::tempdir;
  use the_lib::{
    syntax::{
      DocumentSyntax,
      DocumentSyntaxState,
      QueryKind,
      SyntaxResources,
      tree_sitter::Grammar,
    },
    transaction::Transaction,
  };

  use super::*;

  #[derive(Debug, Clone, Copy)]
  struct JsonResources {
    grammar: Grammar,
    query:   &'static str,
  }

  impl SyntaxResources for JsonResources {
    fn grammar(&self, grammar_name: &str) -> Option<Grammar> {
      (grammar_name == "json").then_some(self.grammar)
    }

    fn query(&self, language_id: &str, kind: QueryKind) -> Option<Cow<'_, str>> {
      (language_id == "json" && kind == QueryKind::Highlights).then_some(Cow::Borrowed(self.query))
    }
  }

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

  #[test]
  fn collect_document_highlights_returns_char_ranges() {
    let grammar = Grammar::try_from(tree_sitter_json::LANGUAGE).unwrap();
    let runtime = RuntimeSyntax::from_toml_str(
      r#"
        [[language]]
        name = "json"
        scope = "source.json"
        file-types = ["json"]
      "#,
      JsonResources {
        grammar,
        query: "(string) @string\n(number) @number",
      },
    )
    .unwrap();
    let mut document = Document::new("{\"name\": 1}\n".into());
    runtime.activate_document(&mut document, Some(Path::new("data.json")));

    let highlights = runtime.collect_document_highlights(&document);
    let fragments = highlights
      .iter()
      .filter_map(|highlight| {
        runtime
          .registry()
          .highlight_name(highlight.kind)
          .map(|name| {
            (
              name,
              document.text().slice(highlight.range.clone()).to_string(),
            )
          })
      })
      .collect::<Vec<_>>();

    assert!(fragments.contains(&(String::from("string"), String::from("\"name\""))));
    assert!(fragments.contains(&(String::from("number"), String::from("1"))));
  }

  #[test]
  fn reparse_document_clears_parse_pending_after_edit() {
    let grammar = Grammar::try_from(tree_sitter_json::LANGUAGE).unwrap();
    let runtime = RuntimeSyntax::from_toml_str(
      r#"
        [[language]]
        name = "json"
        scope = "source.json"
        file-types = ["json"]
      "#,
      JsonResources {
        grammar,
        query: "(string) @string\n(number) @number",
      },
    )
    .unwrap();
    let mut document = Document::new("{\"name\": 1}\n".into());
    runtime.activate_document(&mut document, Some(Path::new("data.json")));

    let tx = Transaction::insert_at(document.text().len_chars(), 10, "2").unwrap();
    document.apply_transaction(&tx).unwrap();

    let DocumentSyntax::Assigned {
      state: DocumentSyntaxState::Active(state),
      ..
    } = document.syntax()
    else {
      panic!("expected active syntax state");
    };
    assert!(state.is_parse_pending());

    assert!(runtime.reparse_document(&mut document).unwrap());

    let DocumentSyntax::Assigned {
      state: DocumentSyntaxState::Active(state),
      ..
    } = document.syntax()
    else {
      panic!("expected active syntax state");
    };
    assert!(!state.is_parse_pending());
  }
}
