use std::{
  borrow::Cow,
  collections::HashSet,
  env,
  fs,
  path::{
    Path,
    PathBuf,
  },
  sync::Arc,
  time::Duration,
};

use rust_embed::RustEmbed;
use serde::Deserialize;
use the_lib::{
  document::Document,
  surface::SurfaceSyntaxInterest,
  syntax::{
    DocumentSyntax,
    DocumentSyntaxState,
    FileType,
    LanguageAssignment,
    LanguageRegistry,
    LanguageSyntaxConfig,
    QueryKind,
    SyntaxConfiguration,
    SyntaxError,
    SyntaxResources,
    SyntaxState,
    tree_sitter::Grammar,
  },
};

#[derive(Debug, Clone)]
pub struct RuntimeSyntax {
  registry: Arc<LanguageRegistry>,
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

  pub fn builtin() -> Result<Self, RuntimeSyntaxError> {
    let config = builtin_syntax_configuration()?;
    Self::new(config, BuiltinSyntaxResources::default()).map_err(Into::into)
  }

  pub fn new(
    config: SyntaxConfiguration,
    resources: impl SyntaxResources + 'static,
  ) -> Result<Self, SyntaxError> {
    LanguageRegistry::new(config, resources).map(|registry| {
      Self {
        registry: Arc::new(registry),
      }
    })
  }

  pub fn with_resources(
    config: SyntaxConfiguration,
    resources: Arc<dyn SyntaxResources>,
  ) -> Result<Self, SyntaxError> {
    LanguageRegistry::with_resources(config, resources).map(|registry| {
      Self {
        registry: Arc::new(registry),
      }
    })
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
    self.registry.as_ref()
  }

  pub fn registry_arc(&self) -> Arc<LanguageRegistry> {
    Arc::clone(&self.registry)
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

  pub fn advance_document_syntax(
    &self,
    document: &mut Document,
    _interest: Option<&SurfaceSyntaxInterest>,
    budget: Duration,
  ) -> Result<bool, RuntimeSyntaxError> {
    let Some(state) = active_syntax_state(document.syntax()) else {
      return Ok(false);
    };

    if !state.needs_reparse() && !state.is_parse_pending() {
      return Ok(false);
    }

    let mut snapshot = state.snapshot(document.text().clone());
    let parsed = snapshot
      .syntax
      .try_reparse_with_timeout(snapshot.source.slice(..), self.registry(), budget)
      .map_err(|err| RuntimeSyntaxError::Backend(err.to_string()))?;
    if !parsed {
      state.set_parse_pending(true);
      return Ok(false);
    }

    Ok(state.apply_parsed(snapshot))
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

#[derive(RustEmbed)]
#[folder = "assets/"]
struct EmbeddedSyntaxAssets;

#[derive(Debug, Default, Clone, Copy)]
pub struct EmbeddedSyntaxResources;

impl EmbeddedSyntaxResources {
  fn query_path(language_id: &str, kind: QueryKind) -> String {
    format!("queries/{language_id}/{}", kind.filename())
  }
}

impl SyntaxResources for EmbeddedSyntaxResources {
  fn grammar(&self, _grammar_name: &str) -> Option<Grammar> {
    None
  }

  fn query(&self, language_id: &str, kind: QueryKind) -> Option<Cow<'_, str>> {
    let path = Self::query_path(language_id, kind);
    EmbeddedSyntaxAssets::get(&path)
      .and_then(|file| String::from_utf8(file.data.into()).ok())
      .map(Cow::Owned)
  }
}

#[derive(Debug, Clone)]
pub struct BuiltinSyntaxResources {
  embedded: EmbeddedSyntaxResources,
  runtime:  Option<FilesystemSyntaxResources>,
}

impl Default for BuiltinSyntaxResources {
  fn default() -> Self {
    Self {
      embedded: EmbeddedSyntaxResources,
      runtime:  builtin_runtime_root().map(FilesystemSyntaxResources::from_runtime_root),
    }
  }
}

impl SyntaxResources for BuiltinSyntaxResources {
  fn grammar(&self, grammar_name: &str) -> Option<Grammar> {
    self
      .runtime
      .as_ref()
      .and_then(|resources| resources.grammar(grammar_name))
  }

  fn query(&self, language_id: &str, kind: QueryKind) -> Option<Cow<'_, str>> {
    self.embedded.query(language_id, kind)
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

  pub fn from_runtime_root(runtime_root: impl Into<PathBuf>) -> Self {
    let runtime_root = runtime_root.into();
    Self::new(runtime_root.join("grammars"), runtime_root.join("queries"))
  }

  fn grammar_candidates(&self, grammar_name: &str) -> Vec<PathBuf> {
    let dash_name = grammar_name.replace('_', "-");
    let underscore_name = grammar_name.replace('-', "_");
    let extensions = [std::env::consts::DLL_EXTENSION, "so", "dylib", "dll"];
    let mut candidates = Vec::new();

    for ext in extensions {
      candidates.push(self.grammar_dir.join(format!("{grammar_name}.{ext}")));
      candidates.push(self.grammar_dir.join(format!("lib{grammar_name}.{ext}")));
      candidates.push(
        self
          .grammar_dir
          .join(format!("tree-sitter-{dash_name}.{ext}")),
      );
      candidates.push(
        self
          .grammar_dir
          .join(format!("libtree-sitter-{dash_name}.{ext}")),
      );
      candidates.push(self.grammar_dir.join(format!("{underscore_name}.{ext}")));
    }

    candidates.sort();
    candidates.dedup();
    candidates
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

const BUILTIN_LANGUAGES_TOML: &str = include_str!("../languages.toml");

fn builtin_runtime_default_root(xdg_config_home: Option<PathBuf>) -> PathBuf {
  let config_root = xdg_config_home
    .unwrap_or_else(|| PathBuf::from(the_stdx::env::expand("$HOME/.config").into_owned()));
  config_root.join("the-editor/runtime")
}

fn builtin_runtime_root() -> Option<PathBuf> {
  let env_root = env::var_os("THE_EDITOR_RUNTIME").map(PathBuf::from);
  let default_root = builtin_runtime_default_root(
    env::var_os("XDG_CONFIG_HOME")
      .filter(|value| !value.is_empty())
      .map(PathBuf::from),
  );

  env_root
    .into_iter()
    .chain(std::iter::once(default_root))
    .find(|path| path.is_dir())
}

fn builtin_syntax_configuration() -> Result<SyntaxConfiguration, RuntimeSyntaxError> {
  #[derive(Debug, Deserialize)]
  struct RawLanguageList {
    #[serde(default)]
    language: Vec<toml::Table>,
  }

  let raw = toml::from_str::<RawLanguageList>(BUILTIN_LANGUAGES_TOML)?;
  let mut languages = Vec::new();
  let mut language_ids = HashSet::new();
  let mut scopes = HashSet::new();
  let mut extensions = HashSet::new();
  let mut shebangs = HashSet::new();

  for table in raw.language {
    match sanitize_language_table(table)
      .and_then(|table| table.try_into::<LanguageSyntaxConfig>().map_err(Into::into))
    {
      Ok(language)
        if register_language_uniqueness(
          &language,
          &mut language_ids,
          &mut scopes,
          &mut extensions,
          &mut shebangs,
        ) =>
      {
        languages.push(language)
      },
      Ok(language) => {
        tracing::warn!(
          "skipping conflicting builtin language config: {}",
          language.language_id
        );
      },
      Err(err) => {
        tracing::warn!("skipping builtin language config: {err}");
      },
    }
  }

  Ok(SyntaxConfiguration {
    language: languages,
  })
}

fn sanitize_language_table(mut table: toml::Table) -> Result<toml::Table, RuntimeSyntaxError> {
  const ALLOWED_KEYS: &[&str] = &[
    "name",
    "scope",
    "file-types",
    "shebangs",
    "comment-token",
    "comment-tokens",
    "block-comment-tokens",
    "text-width",
    "soft-wrap",
    "grammar",
    "injection-regex",
    "indent",
    "auto-pairs",
    "rulers",
    "rainbow-brackets",
  ];

  table.retain(|key, _| ALLOWED_KEYS.contains(&key.as_ref()));

  if let Some(file_types) = table.get_mut("file-types")
    && let toml::Value::Array(entries) = file_types
  {
    for entry in entries {
      normalize_file_type_entry(entry);
    }
  }

  Ok(table)
}

fn normalize_file_type_entry(entry: &mut toml::Value) {
  let toml::Value::Table(table) = entry else {
    return;
  };

  if let Some(glob) = table.remove("glob") {
    table.clear();
    table.insert(String::from("glob"), glob);
  } else if let Some(pattern) = table.remove("pattern") {
    table.clear();
    table.insert(String::from("glob"), pattern);
  }
}

fn register_language_uniqueness(
  language: &LanguageSyntaxConfig,
  language_ids: &mut HashSet<String>,
  scopes: &mut HashSet<String>,
  extensions: &mut HashSet<String>,
  shebangs: &mut HashSet<String>,
) -> bool {
  if language_ids.contains(&language.language_id) || scopes.contains(&language.scope) {
    return false;
  }

  let file_extensions = language
    .file_types
    .iter()
    .filter_map(|file_type| {
      if let FileType::Extension(extension) = file_type {
        Some(extension)
      } else {
        None
      }
    })
    .collect::<Vec<_>>();
  if file_extensions
    .iter()
    .any(|extension| extensions.contains(*extension))
  {
    return false;
  }

  if language
    .shebangs
    .iter()
    .any(|shebang| shebangs.contains(shebang))
  {
    return false;
  }

  language_ids.insert(language.language_id.clone());
  scopes.insert(language.scope.clone());
  extensions.extend(file_extensions.into_iter().cloned());
  shebangs.extend(language.shebangs.iter().cloned());
  true
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
  fn builtin_runtime_default_root_uses_xdg_when_present() {
    assert_eq!(
      builtin_runtime_default_root(Some(PathBuf::from("/tmp/xdg"))),
      PathBuf::from("/tmp/xdg/the-editor/runtime")
    );
  }

  #[test]
  fn builtin_syntax_includes_rust() {
    let runtime = RuntimeSyntax::builtin().unwrap();
    assert!(runtime.registry().language_for_name("rust").is_some());
  }

  #[test]
  fn builtin_resources_return_rust_highlight_query() {
    let query = EmbeddedSyntaxResources
      .query("rust", QueryKind::Highlights)
      .expect("rust highlights query");
    assert!(query.contains("@keyword"));
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
  fn baseline_highlight_runs_return_byte_ranges() {
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

    let runs = document
      .active_syntax()
      .expect("active syntax")
      .syntax()
      .highlight_runs_in_byte_range(
        document.text().slice(..),
        runtime.registry(),
        0..document.text().len_bytes(),
      );
    let fragments = runs
      .iter()
      .filter_map(|run| {
        runtime.registry().highlight_name(run.kind).map(|name| {
          (
            name,
            document
              .text()
              .byte_slice(run.byte_range.clone())
              .to_string(),
          )
        })
      })
      .collect::<Vec<_>>();

    assert!(fragments.contains(&(String::from("string"), String::from("\"name\""))));
    assert!(fragments.contains(&(String::from("number"), String::from("1"))));
  }

  #[test]
  fn builtin_runtime_highlights_rust_source() {
    let Some(runtime_root) = builtin_runtime_root() else {
      return;
    };
    if !runtime_root.join("grammars/rust.so").exists() {
      return;
    }

    let runtime = RuntimeSyntax::builtin().unwrap();
    let mut document = Document::new("fn main() { let x = 1; }\n".into());
    runtime.activate_document(&mut document, Some(Path::new("main.rs")));

    assert!(matches!(document.syntax(), DocumentSyntax::Assigned {
      state: DocumentSyntaxState::Active(_),
      ..
    }));

    let runs = document
      .active_syntax()
      .expect("active syntax")
      .syntax()
      .highlight_runs_in_byte_range(
        document.text().slice(..),
        runtime.registry(),
        0..document.text().len_bytes(),
      );
    let names = runs
      .iter()
      .filter_map(|run| runtime.registry().highlight_name(run.kind))
      .collect::<Vec<_>>();

    assert!(names.iter().any(|name| name.starts_with("keyword")));
    assert!(names.iter().any(|name| {
      name.starts_with("function") || name.starts_with("variable") || name.starts_with("constant")
    }));
  }

  #[test]
  fn builtin_runtime_highlights_real_rust_source() {
    let Some(runtime_root) = builtin_runtime_root() else {
      return;
    };
    if !runtime_root.join("grammars/rust.so").exists() {
      return;
    }

    let runtime = RuntimeSyntax::builtin().unwrap();
    let mut document = Document::new(
      r#"
fn main() {
  let answer = 42;
  println!("{answer}");
}
"#
      .into(),
    );
    runtime.activate_document(&mut document, Some(Path::new("main.rs")));

    let runs = document
      .active_syntax()
      .expect("active syntax")
      .syntax()
      .highlight_runs_in_byte_range(
        document.text().slice(..),
        runtime.registry(),
        0..document.text().len_bytes(),
      );
    assert!(
      !runs.is_empty(),
      "expected highlights for real rust source, got none"
    );
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
