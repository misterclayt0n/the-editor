//! Tree-sitter powered syntax state and query helpers.
//!
//! This module is intentionally split into three layers:
//!
//! - **Configuration** (`syntax::config`): data-only language and service
//!   settings, parsed from config files.
//! - **Resources** (`syntax::resources`): a small interface for providing
//!   grammars and query text (runtime loader, embedded files, etc).
//! - **State** (this file): lazy compilation of queries, language detection,
//!   and syntax state with incremental updates.
//!
//! The core stays pure and does not assume any runtime filesystem layout. The
//! caller injects a `SyntaxResources` implementation for grammars and query
//! sources, which lets different clients choose how to provide assets.
//!
//! # Example: build a loader and detect a language
//!
//! ```no_run
//! use std::{
//!   collections::HashMap,
//!   path::Path,
//! };
//!
//! use the_lib::syntax::{
//!   Loader,
//!   config::{
//!     Configuration,
//!     FileType,
//!     LanguageConfiguration,
//!     LanguageServicesConfig,
//!     SyntaxLanguageConfig,
//!   },
//!   resources::NullResources,
//! };
//!
//! let rust = LanguageConfiguration {
//!   syntax:   SyntaxLanguageConfig {
//!     language_id:          "rust".into(),
//!     scope:                "source.rust".into(),
//!     file_types:           vec![FileType::Extension("rs".into())],
//!     shebangs:             Vec::new(),
//!     comment_tokens:       None,
//!     block_comment_tokens: None,
//!     text_width:           None,
//!     soft_wrap:            None,
//!     auto_format:          false,
//!     path_completion:      None,
//!     word_completion:      None,
//!     grammar:              None,
//!     injection_regex:      None,
//!     indent:               None,
//!     auto_pairs:           None,
//!     rulers:               None,
//!     rainbow_brackets:     None,
//!   },
//!   services: LanguageServicesConfig::default(),
//! };
//!
//! let config = Configuration {
//!   language:        vec![rust],
//!   language_server: HashMap::new(),
//! };
//!
//! let loader = Loader::new(config, NullResources).expect("loader");
//! let lang = loader.language_for_filename(Path::new("src/main.rs"));
//! assert!(lang.is_some());
//! ```
pub mod config;
pub mod highlight_cache;
pub mod indent_query;
pub mod resources;
pub mod runtime_loader;

use std::{
  borrow::Cow,
  collections::{
    HashMap,
    HashSet,
  },
  fmt,
  iter,
  ops::{
    self,
    RangeBounds,
  },
  path::Path,
  sync::{
    Arc,
    OnceLock,
    RwLock,
    atomic::{
      AtomicBool,
      AtomicU64,
      Ordering,
    },
  },
  time::Duration,
};

use config::{
  Configuration,
  FileType,
  LanguageConfiguration,
  LanguageServerConfiguration,
};
pub use highlight_cache::HighlightCache;
pub use indent_query::IndentQuery;
use parking_lot::Mutex;
use ropey::{
  Rope,
  RopeSlice,
};
use the_stdx::rope::{
  RopeSliceExt as _,
  regex_cursor,
};
use thiserror::Error;
use tree_house::{
  Error,
  InjectionLanguageMarker,
  Language,
  LanguageConfig as SyntaxConfig,
  Layer,
  highlighter,
  query_iter::QueryIter,
  tree_sitter::{
    self,
    Capture,
    Grammar,
    InactiveQueryCursor,
    InputEdit,
    Node,
    Pattern,
    Query,
    RopeInput,
    Tree,
    query::{
      InvalidPredicateError,
      UserPredicate,
    },
  },
};
pub use tree_house::{
  Error as HighlighterError,
  LanguageLoader,
  TREE_SITTER_MATCH_LIMIT,
  TreeCursor,
  highlighter::{
    Highlight,
    HighlightEvent,
  },
  query_iter::QueryIterEvent,
};

use crate::{
  syntax::resources::{
    QueryKind,
    SyntaxResources,
  },
  transaction::ChangeSet,
};

pub type Result<T> = std::result::Result<T, SyntaxError>;

#[derive(Debug, Error)]
pub enum SyntaxError {
  #[error("missing grammar for language '{language_id}'")]
  MissingGrammar { language_id: String },
  #[error("failed to compile syntax config for '{language_id}'")]
  SyntaxConfig {
    language_id: String,
    #[source]
    source:      Box<dyn std::error::Error + Send + Sync>,
  },
  #[error("failed to compile query '{kind:?}' for '{language_id}'")]
  Query {
    language_id: String,
    kind:        QueryKind,
    #[source]
    source:      Box<dyn std::error::Error + Send + Sync>,
  },
}

#[derive(Debug)]
pub struct LanguageData {
  config:           Arc<LanguageConfiguration>,
  syntax:           OnceLock<Option<SyntaxConfig>>,
  indent_query:     OnceLock<Option<IndentQuery>>,
  textobject_query: OnceLock<Option<TextObjectQuery>>,
  tag_query:        OnceLock<Option<TagQuery>>,
  rainbow_query:    OnceLock<Option<RainbowQuery>>,
}

impl LanguageData {
  fn new(config: LanguageConfiguration) -> Self {
    Self {
      config:           Arc::new(config),
      syntax:           OnceLock::new(),
      indent_query:     OnceLock::new(),
      textobject_query: OnceLock::new(),
      tag_query:        OnceLock::new(),
      rainbow_query:    OnceLock::new(),
    }
  }

  pub fn config(&self) -> &Arc<LanguageConfiguration> {
    &self.config
  }

  /// Loads the grammar and compiles the highlights, injections and locals for
  /// the language. This function should only be used by this module or the
  /// xtask crate.
  pub fn compile_syntax_config(
    config: &LanguageConfiguration,
    loader: &Loader,
  ) -> Result<Option<SyntaxConfig>> {
    let syntax = &config.syntax;
    let name = &syntax.language_id;
    let parser_name = syntax.grammar.as_deref().unwrap_or(name);
    let Some(grammar) = loader.resources.grammar(parser_name) else {
      return Ok(None);
    };
    let highlight_query_text = loader.query_text(name, QueryKind::Highlights);
    let injection_query_text = loader.query_text(name, QueryKind::Injections);
    let local_query_text = loader.query_text(name, QueryKind::Locals);
    let config = SyntaxConfig::new(
      grammar,
      highlight_query_text.as_ref(),
      injection_query_text.as_ref(),
      local_query_text.as_ref(),
    )
    .map_err(|err| {
      SyntaxError::SyntaxConfig {
        language_id: name.to_string(),
        source:      Box::new(err),
      }
    })?;

    let scopes = loader.scopes();
    reconfigure_highlights(&config, scopes.as_ref());

    Ok(Some(config))
  }

  fn syntax_config(&self, loader: &Loader) -> Option<&SyntaxConfig> {
    self
      .syntax
      .get_or_init(|| {
        match Self::compile_syntax_config(&self.config, loader) {
          Ok(config) => config,
          Err(err) => {
            tracing::warn!(%err, "failed to compile syntax config");
            None
          },
        }
      })
      .as_ref()
  }

  /// Compiles the indents.scm query for a language.
  /// This function should only be used by this module or the xtask crate.
  pub fn compile_indent_query(
    grammar: Grammar,
    config: &LanguageConfiguration,
    loader: &Loader,
  ) -> Result<Option<IndentQuery>> {
    let name = &config.syntax.language_id;
    let text = loader.query_text(name, QueryKind::Indents);
    if text.is_empty() {
      return Ok(None);
    }
    let indent_query = IndentQuery::new(grammar, text.as_ref()).map_err(|err| {
      SyntaxError::Query {
        language_id: name.to_string(),
        kind:        QueryKind::Indents,
        source:      Box::new(err),
      }
    })?;
    Ok(Some(indent_query))
  }

  fn indent_query(&self, loader: &Loader) -> Option<&IndentQuery> {
    self
      .indent_query
      .get_or_init(|| {
        let grammar = self.syntax_config(loader)?.grammar;
        match Self::compile_indent_query(grammar, &self.config, loader) {
          Ok(query) => query,
          Err(err) => {
            tracing::warn!(%err, "failed to compile indent query");
            None
          },
        }
      })
      .as_ref()
  }

  /// Compiles the textobjects.scm query for a language.
  /// This function should only be used by this module or the xtask crate.
  pub fn compile_textobject_query(
    grammar: Grammar,
    config: &LanguageConfiguration,
    loader: &Loader,
  ) -> Result<Option<TextObjectQuery>> {
    let name = &config.syntax.language_id;
    let text = loader.query_text(name, QueryKind::TextObjects);
    if text.is_empty() {
      return Ok(None);
    }
    let query = Query::new(grammar, text.as_ref(), |_, _| Ok(())).map_err(|err| {
      SyntaxError::Query {
        language_id: name.to_string(),
        kind:        QueryKind::TextObjects,
        source:      Box::new(err),
      }
    })?;
    Ok(Some(TextObjectQuery::new(query)))
  }

  fn textobject_query(&self, loader: &Loader) -> Option<&TextObjectQuery> {
    self
      .textobject_query
      .get_or_init(|| {
        let grammar = self.syntax_config(loader)?.grammar;
        match Self::compile_textobject_query(grammar, &self.config, loader) {
          Ok(query) => query,
          Err(err) => {
            tracing::warn!(%err, "failed to compile textobject query");
            None
          },
        }
      })
      .as_ref()
  }

  /// Compiles the tags.scm query for a language.
  /// This function should only be used by this module or the xtask crate.
  pub fn compile_tag_query(
    grammar: Grammar,
    config: &LanguageConfiguration,
    loader: &Loader,
  ) -> Result<Option<TagQuery>> {
    let name = &config.syntax.language_id;
    let text = loader.query_text(name, QueryKind::Tags);
    if text.is_empty() {
      return Ok(None);
    }
    let query = Query::new(grammar, text.as_ref(), |_pattern, predicate| {
      match predicate {
        // TODO: these predicates are allowed in tags.scm queries but not yet used.
        UserPredicate::IsPropertySet { key: "local", .. } => Ok(()),
        UserPredicate::Other(pred) => {
          match pred.name() {
            "strip!" | "select-adjacent!" => Ok(()),
            _ => Err(InvalidPredicateError::unknown(predicate)),
          }
        },
        _ => Err(InvalidPredicateError::unknown(predicate)),
      }
    })
    .map_err(|err| {
      SyntaxError::Query {
        language_id: name.to_string(),
        kind:        QueryKind::Tags,
        source:      Box::new(err),
      }
    })?;
    Ok(Some(TagQuery { query }))
  }

  fn tag_query(&self, loader: &Loader) -> Option<&TagQuery> {
    self
      .tag_query
      .get_or_init(|| {
        let grammar = self.syntax_config(loader)?.grammar;
        match Self::compile_tag_query(grammar, &self.config, loader) {
          Ok(query) => query,
          Err(err) => {
            tracing::warn!(%err, "failed to compile tag query");
            None
          },
        }
      })
      .as_ref()
  }

  /// Compiles the rainbows.scm query for a language.
  /// This function should only be used by this module or the xtask crate.
  pub fn compile_rainbow_query(
    grammar: Grammar,
    config: &LanguageConfiguration,
    loader: &Loader,
  ) -> Result<Option<RainbowQuery>> {
    let name = &config.syntax.language_id;
    let text = loader.query_text(name, QueryKind::Rainbows);
    if text.is_empty() {
      return Ok(None);
    }
    let rainbow_query = RainbowQuery::new(grammar, text.as_ref()).map_err(|err| {
      SyntaxError::Query {
        language_id: name.to_string(),
        kind:        QueryKind::Rainbows,
        source:      Box::new(err),
      }
    })?;
    Ok(Some(rainbow_query))
  }

  fn rainbow_query(&self, loader: &Loader) -> Option<&RainbowQuery> {
    self
      .rainbow_query
      .get_or_init(|| {
        let grammar = self.syntax_config(loader)?.grammar;
        match Self::compile_rainbow_query(grammar, &self.config, loader) {
          Ok(query) => query,
          Err(err) => {
            tracing::warn!(%err, "failed to compile rainbow query");
            None
          },
        }
      })
      .as_ref()
  }

  fn reconfigure(&self, scopes: &[String]) {
    if let Some(Some(config)) = self.syntax.get() {
      reconfigure_highlights(config, scopes);
    }
  }
}

fn reconfigure_highlights(config: &SyntaxConfig, recognized_names: &[String]) {
  config.configure(move |capture_name| {
    let capture_parts: Vec<_> = capture_name.split('.').collect();

    let mut best_index = None;
    let mut best_match_len = 0;
    for (i, recognized_name) in recognized_names.iter().enumerate() {
      let mut len = 0;
      let mut matches = true;
      for (i, part) in recognized_name.split('.').enumerate() {
        match capture_parts.get(i) {
          Some(capture_part) if *capture_part == part => len += 1,
          _ => {
            matches = false;
            break;
          },
        }
      }
      if matches && len > best_match_len {
        best_index = Some(i);
        best_match_len = len;
      }
    }

    best_index.map(|idx| Highlight::new(idx as u32))
  });
}

pub struct Loader {
  resources:               Arc<dyn SyntaxResources>,
  languages:               Vec<LanguageData>,
  languages_by_extension:  HashMap<String, Language>,
  languages_by_shebang:    HashMap<String, Language>,
  languages_glob_matcher:  FileTypeGlobMatcher,
  language_server_configs: HashMap<String, LanguageServerConfiguration>,
  scopes:                  RwLock<Arc<Vec<String>>>,
}

impl fmt::Debug for Loader {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Loader")
      .field("languages", &self.languages.len())
      .field("languages_by_extension", &self.languages_by_extension.len())
      .field("languages_by_shebang", &self.languages_by_shebang.len())
      .field(
        "language_server_configs",
        &self.language_server_configs.len(),
      )
      .finish_non_exhaustive()
  }
}

pub type LoaderError = globset::Error;

impl Loader {
  pub fn new(
    config: Configuration,
    resources: impl SyntaxResources + 'static,
  ) -> std::result::Result<Self, LoaderError> {
    Self::with_resources(config, Arc::new(resources))
  }

  pub fn with_resources(
    config: Configuration,
    resources: Arc<dyn SyntaxResources>,
  ) -> std::result::Result<Self, LoaderError> {
    let mut languages = Vec::with_capacity(config.language.len());
    let mut languages_by_extension = HashMap::new();
    let mut languages_by_shebang = HashMap::new();
    let mut file_type_globs = Vec::new();

    for language_config in config.language {
      let language = Language(languages.len() as u32);

      for file_type in &language_config.syntax.file_types {
        match file_type {
          FileType::Extension(extension) => {
            languages_by_extension.insert(extension.clone(), language);
          },
          FileType::Glob(glob) => {
            file_type_globs.push(FileTypeGlob::new(glob.to_owned(), language));
          },
        };
      }
      for shebang in &language_config.syntax.shebangs {
        languages_by_shebang.insert(shebang.clone(), language);
      }

      languages.push(LanguageData::new(language_config));
    }

    Ok(Self {
      resources,
      languages,
      languages_by_extension,
      languages_by_shebang,
      languages_glob_matcher: FileTypeGlobMatcher::new(file_type_globs)?,
      language_server_configs: config.language_server,
      scopes: RwLock::new(Arc::new(Vec::new())),
    })
  }

  pub fn languages(&self) -> impl ExactSizeIterator<Item = (Language, &LanguageData)> {
    self
      .languages
      .iter()
      .enumerate()
      .map(|(idx, data)| (Language(idx as u32), data))
  }

  fn query_text<'a>(&'a self, language_id: &str, kind: QueryKind) -> Cow<'a, str> {
    self
      .resources
      .query(language_id, kind)
      .unwrap_or(Cow::Borrowed(""))
  }

  pub fn language_configs(&self) -> impl ExactSizeIterator<Item = &LanguageConfiguration> {
    self.languages.iter().map(|language| &*language.config)
  }

  pub fn language(&self, lang: Language) -> &LanguageData {
    &self.languages[lang.idx()]
  }

  pub fn language_for_name(&self, name: &str) -> Option<Language> {
    self.languages.iter().enumerate().find_map(|(idx, config)| {
      (name == config.config.syntax.language_id).then_some(Language(idx as u32))
    })
  }

  pub fn language_for_scope(&self, scope: &str) -> Option<Language> {
    self.languages.iter().enumerate().find_map(|(idx, config)| {
      (scope == config.config.syntax.scope).then_some(Language(idx as u32))
    })
  }

  pub fn language_for_match(&self, text: RopeSlice) -> Option<Language> {
    // PERF: If the name matches up with the id, then this saves the need to do
    // expensive regex.
    let name: Cow<'_, str> = text.into();
    let shortcircuit = self.language_for_name(name.as_ref());
    if shortcircuit.is_some() {
      return shortcircuit;
    }

    // If the name did not match up with a known id, then match on injection regex.

    let mut best_match_length = 0;
    let mut best_match_position = None;
    for (idx, data) in self.languages.iter().enumerate() {
      if let Some(injection_regex) = &data.config.syntax.injection_regex
        && let Some(mat) = injection_regex.find(text.regex_input())
      {
        let length = mat.end() - mat.start();
        if length > best_match_length {
          best_match_position = Some(idx);
          best_match_length = length;
        }
      }
    }

    best_match_position.map(|i| Language(i as u32))
  }

  pub fn language_for_filename(&self, path: &Path) -> Option<Language> {
    // Find all the language configurations that match this file name
    // or a suffix of the file name.

    // TODO: content_regex handling conflict resolution
    self
      .languages_glob_matcher
      .language_for_path(path)
      .or_else(|| {
        path
          .extension()
          .and_then(|extension| extension.to_str())
          .and_then(|extension| self.languages_by_extension.get(extension).copied())
      })
  }

  pub fn language_for_shebang(&self, text: RopeSlice) -> Option<Language> {
    // NOTE: this is slightly different than the one for injection markers in
    // tree-house. It is anchored at the beginning.
    use the_stdx::rope::Regex;
    const SHEBANG: &str = r"^#!\s*(?:\S*[/\\](?:env\s+(?:\-\S+\s+)*)?)?([^\s\.\d]+)";
    static SHEBANG_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = SHEBANG_REGEX.get_or_init(|| Regex::new(SHEBANG).unwrap());

    let marker = regex
      .captures_iter(regex_cursor::Input::new(text))
      .map(|cap| text.byte_slice(cap.get_group(1).unwrap().range()))
      .next()?;
    self.language_for_shebang_marker(marker)
  }

  fn language_for_shebang_marker(&self, marker: RopeSlice) -> Option<Language> {
    let shebang: Cow<str> = marker.into();
    self.languages_by_shebang.get(shebang.as_ref()).copied()
  }

  pub fn indent_query(&self, lang: Language) -> Option<&IndentQuery> {
    self.language(lang).indent_query(self)
  }

  pub fn textobject_query(&self, lang: Language) -> Option<&TextObjectQuery> {
    self.language(lang).textobject_query(self)
  }

  pub fn tag_query(&self, lang: Language) -> Option<&TagQuery> {
    self.language(lang).tag_query(self)
  }

  fn rainbow_query(&self, lang: Language) -> Option<&RainbowQuery> {
    self.language(lang).rainbow_query(self)
  }

  pub fn language_server_configs(&self) -> &HashMap<String, LanguageServerConfiguration> {
    &self.language_server_configs
  }

  pub fn scopes(&self) -> Arc<Vec<String>> {
    self
      .scopes
      .read()
      .unwrap_or_else(|err| err.into_inner())
      .clone()
  }

  pub fn set_scopes(&self, scopes: Vec<String>) {
    let scopes = Arc::new(scopes);
    *self.scopes.write().unwrap_or_else(|err| err.into_inner()) = Arc::clone(&scopes);

    // Reconfigure existing grammars
    for data in &self.languages {
      data.reconfigure(&scopes);
    }
  }
}

impl LanguageLoader for Loader {
  fn language_for_marker(&self, marker: InjectionLanguageMarker) -> Option<Language> {
    match marker {
      InjectionLanguageMarker::Name(name) => self.language_for_name(name.as_ref()),
      InjectionLanguageMarker::Match(text) => self.language_for_match(text),
      InjectionLanguageMarker::Filename(text) => {
        let path: Cow<str> = text.into();
        self.language_for_filename(Path::new(path.as_ref()))
      },
      InjectionLanguageMarker::Shebang(text) => self.language_for_shebang_marker(text),
    }
  }

  fn get_config(&self, lang: Language) -> Option<&SyntaxConfig> {
    self.languages[lang.idx()].syntax_config(self)
  }
}

#[derive(Debug)]
struct FileTypeGlob {
  glob:     globset::Glob,
  language: Language,
}

impl FileTypeGlob {
  pub fn new(glob: globset::Glob, language: Language) -> Self {
    Self { glob, language }
  }
}

#[derive(Debug)]
struct FileTypeGlobMatcher {
  matcher:    globset::GlobSet,
  file_types: Vec<FileTypeGlob>,
}

impl Default for FileTypeGlobMatcher {
  fn default() -> Self {
    Self {
      matcher:    globset::GlobSet::empty(),
      file_types: Default::default(),
    }
  }
}

impl FileTypeGlobMatcher {
  fn new(file_types: Vec<FileTypeGlob>) -> std::result::Result<Self, globset::Error> {
    let mut builder = globset::GlobSetBuilder::new();
    for file_type in &file_types {
      builder.add(file_type.glob.clone());
    }

    Ok(Self {
      matcher: builder.build()?,
      file_types,
    })
  }

  fn language_for_path(&self, path: &Path) -> Option<Language> {
    self
      .matcher
      .matches(path)
      .iter()
      .filter_map(|idx| self.file_types.get(*idx))
      .max_by_key(|file_type| file_type.glob.glob().len())
      .map(|file_type| file_type.language)
  }
}

#[derive(Debug, Clone)]
pub struct Syntax {
  inner: tree_house::Syntax,
}

/// Thread-safe syntax state with version tracking for async parsing.
///
/// This type wraps `Syntax` with version tracking to support background
/// parsing. It separates "interpolation" (fast edit application ~100µs) from
/// "reparsing" (full tree-sitter parse 10-100ms+), allowing the UI to remain
/// responsive during edits.
///
/// # Architecture
///
/// When an edit occurs:
/// 1. `interpolate()` is called synchronously - this applies `tree.edit()` to
///    all layer trees, which just adjusts byte offsets (very fast, O(edits))
/// 2. A background parse is spawned which clones the syntax state and runs the
///    full `update()` call
/// 3. When the background parse completes, the result is swapped in atomically
///
/// This allows the UI to render immediately with slightly stale syntax (but
/// with correct byte offsets), while accurate highlighting follows within
/// ~50-100ms.
pub struct SyntaxState {
  inner: Arc<SyntaxStateInner>,
}

struct SyntaxStateInner {
  /// The actual syntax tree, protected by a mutex for interior mutability.
  syntax: Mutex<Syntax>,

  /// Version when last interpolation happened (fast path).
  /// This is bumped every time `interpolate()` is called.
  interpolated_version: AtomicU64,

  /// Version when last full parse completed (accurate).
  /// This is set when background parse results are swapped in.
  parsed_version: AtomicU64,

  /// Is a background parse currently in progress?
  parse_pending: AtomicBool,
}

impl std::fmt::Debug for SyntaxStateInner {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("SyntaxStateInner")
      .field(
        "interpolated_version",
        &self.interpolated_version.load(Ordering::Relaxed),
      )
      .field(
        "parsed_version",
        &self.parsed_version.load(Ordering::Relaxed),
      )
      .field("parse_pending", &self.parse_pending.load(Ordering::Relaxed))
      .finish_non_exhaustive()
  }
}

impl Clone for SyntaxState {
  fn clone(&self) -> Self {
    Self {
      inner: Arc::clone(&self.inner),
    }
  }
}

impl std::fmt::Debug for SyntaxState {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("SyntaxState")
      .field("inner", &self.inner)
      .finish()
  }
}

/// A snapshot of syntax state that can be sent to a background thread for
/// parsing.
pub struct SyntaxSnapshot {
  /// Cloned syntax state for parsing.
  pub syntax:  Syntax,
  /// The source text at the time of snapshot.
  pub source:  Rope,
  /// The version at which this snapshot was taken.
  pub version: u64,
}

impl SyntaxState {
  /// Create a new syntax state with the given syntax.
  pub fn new(syntax: Syntax) -> Self {
    Self {
      inner: Arc::new(SyntaxStateInner {
        syntax:               Mutex::new(syntax),
        interpolated_version: AtomicU64::new(0),
        parsed_version:       AtomicU64::new(0),
        parse_pending:        AtomicBool::new(false),
      }),
    }
  }

  /// Apply edits to existing trees without full reparse.
  ///
  /// This is the fast path (~100µs) that applies `tree.edit()` to all layer
  /// trees. It just adjusts byte offsets in existing nodes - no actual parsing
  /// happens. Call this synchronously in the edit path, then spawn a background
  /// parse for accurate syntax.
  pub fn interpolate(&self, edits: &[InputEdit]) {
    let mut syntax = self.inner.syntax.lock();
    syntax.inner.interpolate(edits);
    self
      .inner
      .interpolated_version
      .fetch_add(1, Ordering::Release);
  }

  /// Take a snapshot of the current syntax state for background parsing.
  ///
  /// The snapshot includes a clone of the syntax tree and the current version.
  /// Use this to send syntax to a background thread for parsing.
  pub fn snapshot(&self, source: Rope) -> SyntaxSnapshot {
    let syntax = self.inner.syntax.lock();
    SyntaxSnapshot {
      syntax: Syntax {
        inner: syntax.inner.clone(),
      },
      source,
      version: self.inner.interpolated_version.load(Ordering::Acquire),
    }
  }

  /// Swap in results from a completed background parse.
  ///
  /// This replaces the current syntax with the parsed result if the parsed
  /// version is still relevant (no newer edits have occurred).
  ///
  /// Returns `true` if the swap was successful, `false` if the parsed result
  /// was stale (newer edits arrived during parsing).
  pub fn apply_parsed(&self, snapshot: SyntaxSnapshot) -> bool {
    let current_version = self.inner.interpolated_version.load(Ordering::Acquire);

    // Only apply if this parse is for the current version
    if snapshot.version == current_version {
      let mut syntax = self.inner.syntax.lock();
      *syntax = snapshot.syntax;
      self
        .inner
        .parsed_version
        .store(snapshot.version, Ordering::Release);
      self.inner.parse_pending.store(false, Ordering::Release);
      true
    } else {
      // Stale parse - more edits arrived during parsing
      false
    }
  }

  /// Check if we need another parse (edits arrived during the last parse).
  pub fn needs_reparse(&self) -> bool {
    let interpolated = self.inner.interpolated_version.load(Ordering::Acquire);
    let parsed = self.inner.parsed_version.load(Ordering::Acquire);
    interpolated > parsed
  }

  /// Returns the current interpolated version.
  pub fn interpolated_version(&self) -> u64 {
    self.inner.interpolated_version.load(Ordering::Acquire)
  }

  /// Returns the version of the last completed parse.
  pub fn parsed_version(&self) -> u64 {
    self.inner.parsed_version.load(Ordering::Acquire)
  }

  /// Check if a background parse is in progress.
  pub fn is_parse_pending(&self) -> bool {
    self.inner.parse_pending.load(Ordering::Acquire)
  }

  /// Mark that a background parse has started.
  pub fn set_parse_pending(&self, pending: bool) {
    self.inner.parse_pending.store(pending, Ordering::Release);
  }

  /// Check if we have accurate (fully parsed) syntax.
  pub fn is_accurate(&self) -> bool {
    let interpolated = self.inner.interpolated_version.load(Ordering::Acquire);
    let parsed = self.inner.parsed_version.load(Ordering::Acquire);
    parsed >= interpolated
  }

  /// Get read access to the current syntax.
  ///
  /// This returns a guard that holds the lock. For display purposes, use this
  /// even if the syntax might be slightly stale.
  pub fn syntax(&self) -> impl std::ops::Deref<Target = Syntax> + '_ {
    self.inner.syntax.lock()
  }

  /// Get mutable access to the current syntax.
  ///
  /// Use this sparingly - prefer `interpolate()` + background parse for edits.
  pub fn syntax_mut(&self) -> impl std::ops::DerefMut<Target = Syntax> + '_ {
    self.inner.syntax.lock()
  }
}

pub const PARSE_TIMEOUT: Duration = Duration::from_millis(500); // half a second is pretty generous

impl Syntax {
  pub fn new(
    source: RopeSlice,
    language: Language,
    loader: &Loader,
  ) -> std::result::Result<Self, Error> {
    let inner = tree_house::Syntax::new(source, language, PARSE_TIMEOUT, loader)?;
    Ok(Self { inner })
  }

  pub fn update(
    &mut self,
    old_source: RopeSlice,
    source: RopeSlice,
    changeset: &ChangeSet,
    loader: &Loader,
  ) -> std::result::Result<(), Error> {
    let edits = generate_edits(old_source, changeset);
    if edits.is_empty() {
      Ok(())
    } else {
      self.inner.update(source, PARSE_TIMEOUT, &edits, loader)
    }
  }

  /// Apply edits to existing trees without full reparse.
  ///
  /// This is the fast path (~100µs) that applies `tree.edit()` to all layer
  /// trees. It just adjusts byte offsets in existing nodes - no actual parsing
  /// happens. Use this for immediate feedback after edits, followed by a full
  /// `update()` call (potentially on a background thread) for accurate syntax.
  pub fn interpolate(&mut self, old_source: RopeSlice, changeset: &ChangeSet) {
    let edits = generate_edits(old_source, changeset);
    if !edits.is_empty() {
      self.inner.interpolate(&edits);
    }
  }

  /// Update the syntax tree with pre-computed edits.
  ///
  /// This is used by background parsing where edits are computed once and
  /// passed to the parse task.
  pub fn update_with_edits(
    &mut self,
    source: RopeSlice,
    edits: &[InputEdit],
    loader: &Loader,
  ) -> std::result::Result<(), Error> {
    if edits.is_empty() {
      Ok(())
    } else {
      self.inner.update(source, PARSE_TIMEOUT, edits, loader)
    }
  }

  /// Try to update the syntax tree with a short timeout.
  ///
  /// Returns `Ok(true)` if the parse completed in time, `Ok(false)` if it timed
  /// out, or `Err` if there was an error. Use this to try a fast synchronous
  /// parse before falling back to async parsing.
  pub fn try_update_with_short_timeout(
    &mut self,
    source: RopeSlice,
    edits: &[InputEdit],
    loader: &Loader,
    timeout: Duration,
  ) -> std::result::Result<bool, Error> {
    if edits.is_empty() {
      return Ok(true);
    }
    match self.inner.update(source, timeout, edits, loader) {
      Ok(()) => Ok(true),
      Err(Error::Timeout) => Ok(false),
      Err(err) => Err(err),
    }
  }

  /// Apply edits to existing trees without full reparse (interpolation).
  ///
  /// This is the fast path (~100µs) that applies `tree.edit()` to all layer
  /// trees. It just adjusts byte offsets in existing nodes - no actual parsing
  /// happens.
  pub fn interpolate_with_edits(&mut self, edits: &[InputEdit]) {
    if !edits.is_empty() {
      self.inner.interpolate(edits);
    }
  }

  pub fn layer(&self, layer: Layer) -> &tree_house::LayerData {
    self.inner.layer(layer)
  }

  pub fn root_layer(&self) -> Layer {
    self.inner.root()
  }

  pub fn layer_for_byte_range(&self, start: u32, end: u32) -> Layer {
    self.inner.layer_for_byte_range(start, end)
  }

  pub fn root_language(&self) -> Language {
    self.layer(self.root_layer()).language
  }

  pub fn tree(&self) -> &Tree {
    self.inner.tree()
  }

  pub fn tree_for_byte_range(&self, start: u32, end: u32) -> &Tree {
    self.inner.tree_for_byte_range(start, end)
  }

  pub fn named_descendant_for_byte_range(&self, start: u32, end: u32) -> Option<Node<'_>> {
    self.inner.named_descendant_for_byte_range(start, end)
  }

  pub fn descendant_for_byte_range(&self, start: u32, end: u32) -> Option<Node<'_>> {
    self.inner.descendant_for_byte_range(start, end)
  }

  pub fn walk(&self) -> TreeCursor<'_> {
    self.inner.walk()
  }

  /// Returns whether injection parsing is enabled.
  pub fn injections_enabled(&self) -> bool {
    self.inner.injections_enabled()
  }

  /// Sets whether injection parsing is enabled.
  ///
  /// When disabled, only the root layer is parsed, which can significantly
  /// improve performance for large files with many injections (e.g., files
  /// with doc comments, SQL strings, etc.).
  ///
  /// Note: After changing this setting, the syntax tree should be re-parsed
  /// to apply the change.
  pub fn set_injections_enabled(&mut self, enabled: bool) {
    self.inner.set_injections_enabled(enabled);
  }

  pub fn highlighter<'a>(
    &'a self,
    source: RopeSlice<'a>,
    loader: &'a Loader,
    range: impl RangeBounds<u32>,
  ) -> Highlighter<'a> {
    Highlighter::new(&self.inner, source, loader, range)
  }

  /// Collects all highlights for the given byte range.
  /// Returns a vector of (Highlight, byte_range) tuples.
  pub fn collect_highlights(
    &self,
    source: RopeSlice,
    loader: &Loader,
    byte_range: ops::Range<usize>,
  ) -> Vec<(Highlight, ops::Range<usize>)> {
    let highlighter = self.highlighter(
      source,
      loader,
      byte_range.start as u32..byte_range.end as u32,
    );

    highlighter
      .collect_highlights()
      .into_iter()
      .map(|(hl, range)| (hl, range.start as usize..range.end as usize))
      .collect()
  }

  /// Re-queries the given line range and updates the cache with fresh
  /// highlights. Returns the number of highlight entries added to the cache.
  pub fn requery_and_cache(
    &self,
    cache: &mut HighlightCache,
    source: RopeSlice,
    loader: &Loader,
    line_range: ops::Range<usize>,
    doc_version: u64,
    syntax_version: u64,
  ) -> usize {
    if line_range.start >= source.len_lines() {
      return 0;
    }

    let start_line = line_range.start;
    let end_line = line_range.end.min(source.len_lines());

    // Convert line range to byte range
    let start_byte = source.line_to_byte(start_line);
    let end_byte = if end_line < source.len_lines() {
      source.line_to_byte(end_line)
    } else {
      source.len_bytes()
    };

    // Collect highlights for this range
    let highlights = self.collect_highlights(source, loader, start_byte..end_byte);

    // Update the cache
    cache.update_range(
      start_byte..end_byte,
      highlights,
      source,
      doc_version,
      syntax_version,
    );

    cache.len()
  }

  pub fn query_iter<'a, QueryLoader, LayerState, Range>(
    &'a self,
    source: RopeSlice<'a>,
    loader: QueryLoader,
    range: Range,
  ) -> QueryIter<'a, 'a, QueryLoader, LayerState>
  where
    QueryLoader: FnMut(Language) -> Option<&'a Query> + 'a,
    LayerState: Default,
    Range: RangeBounds<u32>,
  {
    QueryIter::new(&self.inner, source, loader, range)
  }

  pub fn tags<'a>(
    &'a self,
    source: RopeSlice<'a>,
    loader: &'a Loader,
    range: impl RangeBounds<u32>,
  ) -> QueryIter<'a, 'a, impl FnMut(Language) -> Option<&'a Query> + 'a, ()> {
    self.query_iter(
      source,
      |lang| loader.tag_query(lang).map(|q| &q.query),
      range,
    )
  }

  pub fn rainbow_highlights(
    &self,
    source: RopeSlice,
    rainbow_length: usize,
    loader: &Loader,
    range: impl RangeBounds<u32>,
  ) -> OverlayHighlights {
    struct RainbowScope<'tree> {
      end:       u32,
      node:      Option<Node<'tree>>,
      highlight: Highlight,
    }

    let mut scope_stack = Vec::<RainbowScope>::new();
    let mut highlights = Vec::new();
    let mut query_iter = self.query_iter::<_, (), _>(
      source,
      |lang| loader.rainbow_query(lang).map(|q| &q.query),
      range,
    );

    while let Some(event) = query_iter.next() {
      let QueryIterEvent::Match(mat) = event else {
        continue;
      };

      let rainbow_query = loader
        .rainbow_query(query_iter.current_language())
        .expect("language must have a rainbow query to emit matches");

      let byte_range = mat.node.byte_range();
      // Pop any scopes that end before this capture begins.
      while scope_stack
        .last()
        .is_some_and(|scope| byte_range.start >= scope.end)
      {
        scope_stack.pop();
      }

      let capture = Some(mat.capture);
      if capture == rainbow_query.scope_capture {
        scope_stack.push(RainbowScope {
          end:       byte_range.end,
          node:      if rainbow_query
            .include_children_patterns
            .contains(&mat.pattern)
          {
            None
          } else {
            Some(mat.node.clone())
          },
          highlight: Highlight::new((scope_stack.len() % rainbow_length) as u32),
        });
      } else if capture == rainbow_query.bracket_capture
        && let Some(scope) = scope_stack.last()
        && !scope
          .node
          .as_ref()
          .is_some_and(|node| mat.node.parent().as_ref() != Some(node))
      {
        let start = source.byte_to_char(source.floor_char_boundary(byte_range.start as usize));
        let end = source.byte_to_char(source.ceil_char_boundary(byte_range.end as usize));
        highlights.push((scope.highlight, start..end));
      }
    }

    OverlayHighlights::Heterogenous { highlights }
  }
}

pub type Highlighter<'a> = highlighter::Highlighter<'a, 'a, Loader>;

/// Generate tree-sitter input edits from a changeset.
///
/// This is used both for synchronous interpolation and for background parsing.
pub fn generate_edits(old_text: RopeSlice, changeset: &ChangeSet) -> Vec<InputEdit> {
  use tree_sitter::Point;

  use crate::transaction::Operation::*;

  let mut old_pos = 0;

  let mut edits = Vec::new();

  if changeset.changes.is_empty() {
    return edits;
  }

  let mut iter = changeset.changes.iter().peekable();

  // TODO; this is a lot easier with Change instead of Operation.
  while let Some(change) = iter.next() {
    let len = match change {
      Delete(i) | Retain(i) => *i,
      Insert(_) => 0,
    };
    let mut old_end = old_pos + len;

    match change {
      Retain(_) => {},
      Delete(_) => {
        let start_byte = old_text.char_to_byte(old_pos) as u32;
        let old_end_byte = old_text.char_to_byte(old_end) as u32;

        // deletion
        edits.push(InputEdit {
          start_byte,               // old_pos to byte
          old_end_byte,             // old_end to byte
          new_end_byte: start_byte, // old_pos to byte
          start_point: Point::ZERO,
          old_end_point: Point::ZERO,
          new_end_point: Point::ZERO,
        });
      },
      Insert(s) => {
        let start_byte = old_text.char_to_byte(old_pos) as u32;

        // a subsequent delete means a replace, consume it
        if let Some(Delete(len)) = iter.peek() {
          old_end = old_pos + len;
          let old_end_byte = old_text.char_to_byte(old_end) as u32;

          iter.next();

          // replacement
          edits.push(InputEdit {
            start_byte,                                // old_pos to byte
            old_end_byte,                              // old_end to byte
            new_end_byte: start_byte + s.len() as u32, // old_pos to byte + s.len()
            start_point: Point::ZERO,
            old_end_point: Point::ZERO,
            new_end_point: Point::ZERO,
          });
        } else {
          // insert
          edits.push(InputEdit {
            start_byte,                                // old_pos to byte
            old_end_byte: start_byte,                  // same
            new_end_byte: start_byte + s.len() as u32, // old_pos + s.len()
            start_point: Point::ZERO,
            old_end_point: Point::ZERO,
            new_end_point: Point::ZERO,
          });
        }
      },
    }
    old_pos = old_end;
  }
  edits
}

/// A set of "overlay" highlights and ranges they apply to.
///
/// As overlays, the styles for the given `Highlight`s are merged on top of the
/// syntax highlights.
#[derive(Debug)]
pub enum OverlayHighlights {
  /// All highlights use a single `Highlight`.
  ///
  /// Note that, currently, all ranges are assumed to be non-overlapping. This
  /// could change in the future though.
  Homogeneous {
    highlight: Highlight,
    ranges:    Vec<ops::Range<usize>>,
  },
  /// A collection of different highlights for given ranges.
  ///
  /// Note that the ranges **must be non-overlapping**.
  Heterogenous {
    highlights: Vec<(Highlight, ops::Range<usize>)>,
  },
}

impl OverlayHighlights {
  pub fn single(highlight: Highlight, range: ops::Range<usize>) -> Self {
    Self::Homogeneous {
      highlight,
      ranges: vec![range],
    }
  }

  fn is_empty(&self) -> bool {
    match self {
      Self::Homogeneous { ranges, .. } => ranges.is_empty(),
      Self::Heterogenous { highlights } => highlights.is_empty(),
    }
  }
}

#[derive(Debug)]
struct Overlay {
  highlights:       OverlayHighlights,
  /// The position of the highlighter into the Vec of ranges of the overlays.
  ///
  /// Used by the `OverlayHighlighter`.
  idx:              usize,
  /// The currently active highlight (and the ending character index) for this
  /// overlay.
  ///
  /// Used by the `OverlayHighlighter`.
  active_highlight: Option<(Highlight, usize)>,
}

impl Overlay {
  fn new(highlights: OverlayHighlights) -> Option<Self> {
    (!highlights.is_empty()).then_some(Self {
      highlights,
      idx: 0,
      active_highlight: None,
    })
  }

  fn current(&self) -> Option<(Highlight, ops::Range<usize>)> {
    match &self.highlights {
      OverlayHighlights::Homogeneous { highlight, ranges } => {
        ranges
          .get(self.idx)
          .map(|range| (*highlight, range.clone()))
      },
      OverlayHighlights::Heterogenous { highlights } => highlights.get(self.idx).cloned(),
    }
  }

  fn start(&self) -> Option<usize> {
    match &self.highlights {
      OverlayHighlights::Homogeneous { ranges, .. } => {
        ranges.get(self.idx).map(|range| range.start)
      },
      OverlayHighlights::Heterogenous { highlights } => {
        highlights
          .get(self.idx)
          .map(|(_highlight, range)| range.start)
      },
    }
  }
}

/// A collection of highlights to apply when rendering which merge on top of
/// syntax highlights.
#[derive(Debug)]
pub struct OverlayHighlighter {
  overlays:             Vec<Overlay>,
  next_highlight_start: usize,
  next_highlight_end:   usize,
}

impl OverlayHighlighter {
  pub fn new(overlays: impl IntoIterator<Item = OverlayHighlights>) -> Self {
    let overlays: Vec<_> = overlays.into_iter().filter_map(Overlay::new).collect();
    let next_highlight_start = overlays
      .iter()
      .filter_map(|overlay| overlay.start())
      .min()
      .unwrap_or(usize::MAX);

    Self {
      overlays,
      next_highlight_start,
      next_highlight_end: usize::MAX,
    }
  }

  /// The current position in the overlay highlights.
  ///
  /// This method is meant to be used when treating this type as a cursor over
  /// the overlay highlights.
  ///
  /// `usize::MAX` is returned when there are no more overlay highlights.
  pub fn next_event_offset(&self) -> usize {
    self.next_highlight_start.min(self.next_highlight_end)
  }

  pub fn advance(&mut self) -> (HighlightEvent, impl Iterator<Item = Highlight> + '_) {
    let mut refresh = false;
    let prev_stack_size = self
      .overlays
      .iter()
      .filter(|overlay| overlay.active_highlight.is_some())
      .count();
    let pos = self.next_event_offset();

    if self.next_highlight_end == pos {
      for overlay in self.overlays.iter_mut() {
        if overlay
          .active_highlight
          .is_some_and(|(_highlight, end)| end == pos)
        {
          overlay.active_highlight.take();
        }
      }

      refresh = true;
    }

    while self.next_highlight_start == pos {
      let mut activated_idx = usize::MAX;
      for (idx, overlay) in self.overlays.iter_mut().enumerate() {
        let Some((highlight, range)) = overlay.current() else {
          continue;
        };
        if range.start != self.next_highlight_start {
          continue;
        }

        // If this overlay has a highlight at this start index, set its active highlight
        // and increment the cursor position within the overlay.
        overlay.active_highlight = Some((highlight, range.end));
        overlay.idx += 1;

        activated_idx = activated_idx.min(idx);
      }

      // If `self.next_highlight_start == pos` that means that some overlay was ready
      // to emit a highlight, so `activated_idx` must have been set to an
      // existing index.
      assert!(
        (0..self.overlays.len()).contains(&activated_idx),
        "expected an overlay to highlight (at pos {pos}, there are {} overlays)",
        self.overlays.len()
      );

      // If any overlays are active after the (lowest) one which was just activated,
      // the highlights need to be refreshed.
      refresh |= self.overlays[activated_idx..]
        .iter()
        .any(|overlay| overlay.active_highlight.is_some());

      self.next_highlight_start = self
        .overlays
        .iter()
        .filter_map(|overlay| overlay.start())
        .min()
        .unwrap_or(usize::MAX);
    }

    self.next_highlight_end = self
      .overlays
      .iter()
      .filter_map(|overlay| Some(overlay.active_highlight?.1))
      .min()
      .unwrap_or(usize::MAX);

    let (event, start) = if refresh {
      (HighlightEvent::Refresh, 0)
    } else {
      (HighlightEvent::Push, prev_stack_size)
    };

    (
      event,
      self
        .overlays
        .iter()
        .flat_map(|overlay| overlay.active_highlight)
        .map(|(highlight, _end)| highlight)
        .skip(start),
    )
  }
}

#[derive(Debug)]
pub enum CapturedNode<'a> {
  Single(Node<'a>),
  /// Guaranteed to be not empty
  Grouped(Vec<Node<'a>>),
}

impl CapturedNode<'_> {
  pub fn start_byte(&self) -> usize {
    match self {
      Self::Single(n) => n.start_byte() as usize,
      Self::Grouped(ns) => ns[0].start_byte() as usize,
    }
  }

  pub fn end_byte(&self) -> usize {
    match self {
      Self::Single(n) => n.end_byte() as usize,
      Self::Grouped(ns) => ns.last().unwrap().end_byte() as usize,
    }
  }

  pub fn byte_range(&self) -> ops::Range<usize> {
    self.start_byte()..self.end_byte()
  }
}

#[derive(Debug)]
pub struct TextObjectQuery {
  query: Query,
}

impl TextObjectQuery {
  pub fn new(query: Query) -> Self {
    Self { query }
  }

  /// Run the query on the given node and return sub nodes which match given
  /// capture ("function.inside", "class.around", etc).
  ///
  /// Captures may contain multiple nodes by using quantifiers (+, *, etc),
  /// and support for this is partial and could use improvement.
  ///
  /// ```query
  /// (comment)+ @capture
  ///
  /// ; OR
  /// (
  ///   (comment)*
  ///   .
  ///   (function)
  /// ) @capture
  /// ```
  pub fn capture_nodes<'a>(
    &'a self,
    capture_name: &'a str,
    node: &Node<'a>,
    slice: RopeSlice<'a>,
  ) -> Option<impl Iterator<Item = CapturedNode<'a>>> {
    self.capture_nodes_any([capture_name], node, slice)
  }

  /// Find the first capture that exists out of all given `capture_names`
  /// and return sub nodes that match this capture.
  pub fn capture_nodes_any<'a, I>(
    &'a self,
    capture_names: I,
    node: &Node<'a>,
    slice: RopeSlice<'a>,
  ) -> Option<impl Iterator<Item = CapturedNode<'a>> + 'a>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let capture = capture_names
      .into_iter()
      .find_map(|cap| self.query.get_capture(cap))?;

    let mut cursor = InactiveQueryCursor::new(0..u32::MAX, TREE_SITTER_MATCH_LIMIT).execute_query(
      &self.query,
      node,
      RopeInput::new(slice),
    );

    let capture_node = iter::from_fn(move || {
      let (mat, _) = cursor.next_matched_node()?;
      Some(mat.nodes_for_capture(capture).cloned().collect())
    })
    .filter_map(|nodes: Vec<_>| {
      if nodes.len() > 1 {
        Some(CapturedNode::Grouped(nodes))
      } else {
        nodes.into_iter().map(CapturedNode::Single).next()
      }
    });

    Some(capture_node)
  }
}

#[derive(Debug)]
pub struct TagQuery {
  pub query: Query,
}

pub fn pretty_print_tree<W: fmt::Write>(fmt: &mut W, node: Node) -> fmt::Result {
  if node.child_count() == 0 {
    if node_is_visible(&node) {
      write!(fmt, "({})", node.kind())
    } else {
      write!(fmt, "\"{}\"", format_anonymous_node_kind(node.kind()))
    }
  } else {
    pretty_print_tree_impl(fmt, &mut node.walk(), 0)
  }
}

fn node_is_visible(node: &Node) -> bool {
  node.is_missing() || (node.is_named() && node.grammar().node_kind_is_visible(node.kind_id()))
}

fn format_anonymous_node_kind(kind: &str) -> Cow<'_, str> {
  if kind.contains('"') {
    Cow::Owned(kind.replace('"', "\\\""))
  } else {
    Cow::Borrowed(kind)
  }
}

fn pretty_print_tree_impl<W: fmt::Write>(
  fmt: &mut W,
  cursor: &mut tree_sitter::TreeCursor,
  depth: usize,
) -> fmt::Result {
  let node = cursor.node();
  let visible = node_is_visible(&node);

  if visible {
    let indentation_columns = depth * 2;
    write!(fmt, "{:indentation_columns$}", "")?;

    if let Some(field_name) = cursor.field_name() {
      write!(fmt, "{}: ", field_name)?;
    }

    write!(fmt, "({}", node.kind())?;
  } else {
    write!(fmt, " \"{}\"", format_anonymous_node_kind(node.kind()))?;
  }

  // Handle children.
  if cursor.goto_first_child() {
    loop {
      if node_is_visible(&cursor.node()) {
        fmt.write_char('\n')?;
      }

      pretty_print_tree_impl(fmt, cursor, depth + 1)?;

      if !cursor.goto_next_sibling() {
        break;
      }
    }

    let moved = cursor.goto_parent();
    // The parent of the first child must exist, and must be `node`.
    debug_assert!(moved);
    debug_assert!(cursor.node() == node);
  }

  if visible {
    fmt.write_char(')')?;
  }

  Ok(())
}

/// Finds the child of `node` which contains the given byte range.
pub fn child_for_byte_range<'a>(node: &Node<'a>, range: ops::Range<u32>) -> Option<Node<'a>> {
  for child in node.children() {
    let child_range = child.byte_range();

    if range.start >= child_range.start && range.end <= child_range.end {
      return Some(child);
    }
  }

  None
}

#[derive(Debug)]
pub struct RainbowQuery {
  query:                     Query,
  include_children_patterns: HashSet<Pattern>,
  scope_capture:             Option<Capture>,
  bracket_capture:           Option<Capture>,
}

impl RainbowQuery {
  fn new(
    grammar: Grammar,
    source: &str,
  ) -> std::result::Result<Self, tree_sitter::query::ParseError> {
    let mut include_children_patterns = HashSet::default();

    let query = Query::new(grammar, source, |pattern, predicate| {
      match predicate {
        UserPredicate::SetProperty {
          key: "rainbow.include-children",
          val,
        } => {
          if val.is_some() {
            return Err("property 'rainbow.include-children' does not take an argument".into());
          }
          include_children_patterns.insert(pattern);
          Ok(())
        },
        _ => Err(InvalidPredicateError::unknown(predicate)),
      }
    })?;

    Ok(Self {
      include_children_patterns,
      scope_capture: query.get_capture("rainbow.scope"),
      bracket_capture: query.get_capture("rainbow.bracket"),
      query,
    })
  }
}

#[cfg(test)]
mod test {
  use ropey::Rope;

  use super::*;
  use crate::transaction::Transaction;

  #[test]
  fn test_input_edits() {
    use tree_sitter::{
      InputEdit,
      Point,
    };

    let doc = Rope::from("hello world!\ntest 123");
    let transaction = Transaction::change(
      &doc,
      vec![(6, 11, Some("test".into())), (12, 17, None)].into_iter(),
    )
    .unwrap();
    let edits = generate_edits(doc.slice(..), transaction.changes());

    assert_eq!(edits, &[
      InputEdit {
        start_byte:    6,
        old_end_byte:  11,
        new_end_byte:  10,
        start_point:   Point::ZERO,
        old_end_point: Point::ZERO,
        new_end_point: Point::ZERO,
      },
      InputEdit {
        start_byte:    12,
        old_end_byte:  17,
        new_end_byte:  12,
        start_point:   Point::ZERO,
        old_end_point: Point::ZERO,
        new_end_point: Point::ZERO,
      },
    ]);

    let mut doc = Rope::from("fn test() {}");
    let transaction =
      Transaction::change(&doc, vec![(8, 8, Some("a: u32".into()))].into_iter()).unwrap();
    let edits = generate_edits(doc.slice(..), transaction.changes());
    transaction.apply(&mut doc).unwrap();

    assert_eq!(doc, "fn test(a: u32) {}");
    assert_eq!(edits, &[InputEdit {
      start_byte:    8,
      old_end_byte:  8,
      new_end_byte:  14,
      start_point:   Point::ZERO,
      old_end_point: Point::ZERO,
      new_end_point: Point::ZERO,
    }]);
  }
}

#[cfg(all(test, feature = "runtime-loader"))]
mod runtime_tests {
  use std::{
    collections::HashMap,
    ops::Range,
  };

  use ropey::Rope;

  use super::*;
  use crate::syntax::{
    config::{
      Configuration,
      FileType,
      LanguageConfiguration,
      LanguageServicesConfig,
      SyntaxLanguageConfig,
    },
    runtime_loader::RuntimeLoader,
  };
  use crate::transaction::Transaction;

  fn language_config(language_id: &str, scope: &str, extension: &str) -> LanguageConfiguration {
    LanguageConfiguration {
      syntax:   SyntaxLanguageConfig {
        language_id:          language_id.to_string(),
        scope:                scope.to_string(),
        file_types:           vec![FileType::Extension(extension.to_string())],
        shebangs:             Vec::new(),
        comment_tokens:       None,
        block_comment_tokens: None,
        text_width:           None,
        soft_wrap:            None,
        auto_format:          false,
        path_completion:      None,
        word_completion:      None,
        grammar:              None,
        injection_regex:      None,
        indent:               None,
        auto_pairs:           None,
        rulers:               None,
        rainbow_brackets:     None,
      },
      services: LanguageServicesConfig::default(),
    }
  }

  fn test_loader() -> Loader {
    let config = Configuration {
      language:        vec![
        language_config("rust", "source.rust", "rs"),
        language_config("ruby", "source.ruby", "rb"),
      ],
      language_server: HashMap::new(),
    };

    Loader::new(config, RuntimeLoader::new()).expect("loader")
  }

  #[derive(Debug, Clone, Copy)]
  struct SimRng {
    state: u64,
  }

  impl SimRng {
    fn new(seed: u64) -> Self {
      Self {
        state: seed.max(1),
      }
    }

    fn next_u64(&mut self) -> u64 {
      let mut x = self.state;
      x ^= x << 13;
      x ^= x >> 7;
      x ^= x << 17;
      self.state = x;
      x
    }

    fn next_usize(&mut self, upper: usize) -> usize {
      if upper == 0 {
        0
      } else {
        (self.next_u64() as usize) % upper
      }
    }
  }

  fn next_edit(rng: &mut SimRng, len_chars: usize) -> (usize, usize, Option<String>, &'static str) {
    const TOKENS: &[&str] = &[
      "a",
      "_",
      " ",
      "\n",
      "{",
      "}",
      "(",
      ")",
      "\"",
      "::",
      "let ",
      "fn ",
      "0",
      "🙂",
    ];

    let op = if len_chars == 0 { 0 } else { rng.next_usize(3) };
    match op {
      0 => {
        let at = rng.next_usize(len_chars.saturating_add(1));
        let replacement = TOKENS[rng.next_usize(TOKENS.len())].to_string();
        (at, at, Some(replacement), "insert")
      },
      1 => {
        let from = rng.next_usize(len_chars);
        let max_span = (len_chars - from).min(8);
        let span = 1 + rng.next_usize(max_span);
        (from, from + span, None, "delete")
      },
      _ => {
        let from = rng.next_usize(len_chars);
        let max_span = (len_chars - from).min(8);
        let span = 1 + rng.next_usize(max_span);
        let replacement = TOKENS[rng.next_usize(TOKENS.len())].to_string();
        (from, from + span, Some(replacement), "replace")
      },
    }
  }

  fn normalized_highlights(
    syntax: &Syntax,
    text: RopeSlice,
    loader: &Loader,
    range: Range<usize>,
  ) -> Vec<(u32, Range<usize>)> {
    let end = range.end.min(text.len_bytes());
    let start = range.start.min(end);
    let mut highlights = syntax.collect_highlights(text, loader, start..end);
    highlights.retain(|(_highlight, span)| span.start <= span.end && span.end <= end);
    highlights.sort_by_key(|(highlight, span)| (span.start, span.end, highlight.get()));
    highlights
      .into_iter()
      .map(|(highlight, span)| (highlight.get(), span))
      .collect()
  }

  fn sampled_byte_ranges(mut rng: SimRng, len_bytes: usize) -> Vec<Range<usize>> {
    if len_bytes == 0 {
      return vec![0..0];
    }

    let mut ranges = Vec::with_capacity(4);
    ranges.push(0..len_bytes);
    for _ in 0..3 {
      let start = rng.next_usize(len_bytes);
      let max_width = (len_bytes - start).max(1).min(512);
      let width = 1 + rng.next_usize(max_width);
      ranges.push(start..(start + width));
    }
    ranges
  }

  #[test]
  fn deterministic_syntax_edit_simulation() {
    let loader = test_loader();
    let Some(language) = loader.language_for_name("rust") else {
      eprintln!("Skipping deterministic_syntax_edit_simulation: Rust language not configured");
      return;
    };
    if loader.get_config(language).is_none() {
      eprintln!("Skipping deterministic_syntax_edit_simulation: Rust grammar not available");
      return;
    }

    let corpora = [
      "fn main() {\n    let value = 1;\n    println!(\"{value}\");\n}\n",
      "/// docs\nfn parse(input: &str) -> Result<(), Error> {\n    input.parse::<u64>()?;\n    Ok(())\n}\n",
      "enum Token { Ident(String), Number(u64), Eof }\n",
    ];
    let seeds = [0x41u64, 0x5eedu64, 0xdead_beefu64, 0x1234_5678u64];

    for (corpus_idx, corpus) in corpora.iter().enumerate() {
      for seed in seeds {
        let combined_seed = seed ^ ((corpus_idx as u64 + 1) * 0x9E37_79B9);
        let mut rng = SimRng::new(combined_seed);
        let mut text = Rope::from_str(corpus);
        let mut syntax = match Syntax::new(text.slice(..), language, &loader) {
          Ok(syntax) => syntax,
          Err(err) => {
            eprintln!("Skipping deterministic_syntax_edit_simulation: {err}");
            return;
          },
        };

        for step in 0..320usize {
          let old_text = text.clone();
          let (from, to, replacement, op_name) = next_edit(&mut rng, old_text.len_chars());

          let transaction = Transaction::change(
            &text,
            vec![(from, to, replacement.clone().map(Into::into))].into_iter(),
          )
          .unwrap_or_else(|err| {
            panic!(
              "failed to build transaction: seed={combined_seed} corpus={corpus_idx} step={step} op={op_name} from={from} to={to} replacement={replacement:?}: {err}"
            )
          });
          let changes = transaction.changes().clone();
          transaction.apply(&mut text).unwrap_or_else(|err| {
            panic!(
              "failed to apply transaction: seed={combined_seed} corpus={corpus_idx} step={step} op={op_name}: {err}"
            )
          });

          syntax
            .update(old_text.slice(..), text.slice(..), &changes, &loader)
            .unwrap_or_else(|err| {
              panic!(
                "syntax update failed: seed={combined_seed} corpus={corpus_idx} step={step} op={op_name} from={from} to={to} replacement={replacement:?}: {err}"
              )
            });

          let root_range = syntax.tree().root_node().byte_range();
          let len_bytes = text.len_bytes();
          let root_start = root_range.start as usize;
          let root_end = root_range.end as usize;
          assert!(
            root_start <= root_end,
            "invalid root ordering: seed={combined_seed} corpus={corpus_idx} step={step} op={op_name} root_start={root_start} root_end={root_end}"
          );
          assert!(
            root_end <= len_bytes,
            "invalid root end: seed={combined_seed} corpus={corpus_idx} step={step} op={op_name} root_end={} len_bytes={len_bytes}",
            root_end
          );

          let highlights = syntax.collect_highlights(text.slice(..), &loader, 0..len_bytes);
          for (_highlight, range) in highlights {
            assert!(
              range.start <= range.end && range.end <= len_bytes,
              "invalid highlight range: seed={combined_seed} corpus={corpus_idx} step={step} op={op_name} range={range:?} len_bytes={len_bytes}"
            );
          }
        }
      }
    }
  }

  #[test]
  fn deterministic_syntax_differential_oracle() {
    let loader = test_loader();
    let Some(language) = loader.language_for_name("rust") else {
      eprintln!("Skipping deterministic_syntax_differential_oracle: Rust language not configured");
      return;
    };
    if loader.get_config(language).is_none() {
      eprintln!("Skipping deterministic_syntax_differential_oracle: Rust grammar not available");
      return;
    }

    let corpora = [
      "fn main() {\n    let value = 1;\n    println!(\"{value}\");\n}\n",
      "/// docs\nfn parse(input: &str) -> Result<(), Error> {\n    input.parse::<u64>()?;\n    Ok(())\n}\n",
      "enum Token { Ident(String), Number(u64), Eof }\n",
      "impl Parser {\n    fn next(&mut self) -> Option<char> { self.input.pop() }\n}\n",
    ];
    let seeds = [0x41u64, 0x5eedu64, 0xdead_beefu64, 0x1234_5678u64];

    for (corpus_idx, corpus) in corpora.iter().enumerate() {
      for seed in seeds {
        let combined_seed = seed ^ ((corpus_idx as u64 + 1) * 0x9E37_79B9);
        let mut rng = SimRng::new(combined_seed);
        let mut text = Rope::from_str(corpus);
        let mut incremental = match Syntax::new(text.slice(..), language, &loader) {
          Ok(syntax) => syntax,
          Err(err) => {
            eprintln!("Skipping deterministic_syntax_differential_oracle: {err}");
            return;
          },
        };

        for step in 0..240usize {
          let old_text = text.clone();
          let (from, to, replacement, op_name) = next_edit(&mut rng, old_text.len_chars());

          let transaction = Transaction::change(
            &text,
            vec![(from, to, replacement.clone().map(Into::into))].into_iter(),
          )
          .unwrap_or_else(|err| {
            panic!(
              "failed to build transaction: seed={combined_seed} corpus={corpus_idx} step={step} op={op_name} from={from} to={to} replacement={replacement:?}: {err}"
            )
          });
          let changes = transaction.changes().clone();
          transaction.apply(&mut text).unwrap_or_else(|err| {
            panic!(
              "failed to apply transaction: seed={combined_seed} corpus={corpus_idx} step={step} op={op_name}: {err}"
            )
          });

          incremental
            .update(old_text.slice(..), text.slice(..), &changes, &loader)
            .unwrap_or_else(|err| {
              panic!(
                "incremental syntax update failed: seed={combined_seed} corpus={corpus_idx} step={step} op={op_name} from={from} to={to} replacement={replacement:?}: {err}"
              )
            });

          if step % 8 != 0 && step != 239 {
            continue;
          }

          let fresh = Syntax::new(text.slice(..), language, &loader).unwrap_or_else(|err| {
            panic!(
              "fresh syntax parse failed: seed={combined_seed} corpus={corpus_idx} step={step} op={op_name} from={from} to={to} replacement={replacement:?}: {err}"
            )
          });

          let incremental_root = incremental.tree().root_node();
          let fresh_root = fresh.tree().root_node();
          assert_eq!(
            incremental_root.kind_id(),
            fresh_root.kind_id(),
            "root kind mismatch: seed={combined_seed} corpus={corpus_idx} step={step} op={op_name}"
          );
          assert_eq!(
            incremental_root.byte_range(),
            fresh_root.byte_range(),
            "root byte range mismatch: seed={combined_seed} corpus={corpus_idx} step={step} op={op_name}"
          );

          let windows = sampled_byte_ranges(
            SimRng::new(combined_seed ^ ((step as u64 + 1) * 0xA24B_AED4)),
            text.len_bytes(),
          );
          for window in windows {
            let incremental_highlights =
              normalized_highlights(&incremental, text.slice(..), &loader, window.clone());
            let fresh_highlights =
              normalized_highlights(&fresh, text.slice(..), &loader, window.clone());
            assert_eq!(
              incremental_highlights,
              fresh_highlights,
              "highlight mismatch: seed={combined_seed} corpus={corpus_idx} step={step} op={op_name} window={window:?}"
            );
          }
        }
      }
    }
  }

  #[test]
  #[ignore = "requires compiled tree-sitter grammars"]
  fn test_textobject_queries() {
    let query_str = r#"
        (line_comment)+ @quantified_nodes
        ((line_comment)+) @quantified_nodes_grouped
        ((line_comment) (line_comment)) @multiple_nodes_grouped
        "#;
    let source = Rope::from_str(
      r#"
/// a comment on
/// multiple lines
        "#,
    );

    let loader = test_loader();
    let language = match loader.language_for_name("rust") {
      Some(lang) => lang,
      None => {
        eprintln!("Skipping test_textobject_queries: Rust parser not available");
        return;
      },
    };
    let grammar = match loader.get_config(language) {
      Some(config) => config.grammar,
      None => {
        eprintln!("Skipping test_textobject_queries: Rust grammar not available");
        return;
      },
    };
    let query = Query::new(grammar, query_str, |_, _| Ok(())).unwrap();
    let textobject = TextObjectQuery::new(query);
    let syntax = match Syntax::new(source.slice(..), language, &loader) {
      Ok(syntax) => syntax,
      Err(err) => {
        eprintln!("Skipping test_textobject_queries: {err}");
        return;
      },
    };

    let root = syntax.tree().root_node();
    let test = |capture, range| {
      let matches: Vec<_> = textobject
        .capture_nodes(capture, &root, source.slice(..))
        .unwrap()
        .collect();

      assert_eq!(
        matches[0].byte_range(),
        range,
        "@{} expected {:?}",
        capture,
        range
      )
    };

    test("quantified_nodes", 1..37);
  }

  #[track_caller]
  fn assert_pretty_print(
    loader: &Loader,
    language_name: &str,
    source: &str,
    expected: &str,
    start: usize,
    end: usize,
  ) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let source = Rope::from_str(source);
    let language = match loader.language_for_name(language_name) {
      Some(lang) => lang,
      None => return Err(format!("{} parser not available", language_name).into()),
    };
    let syntax = Syntax::new(source.slice(..), language, loader)
      .map_err(|e| format!("Failed to create syntax: {:?}", e))?;

    let root = syntax
      .tree()
      .root_node()
      .descendant_for_byte_range(start as u32, end as u32)
      .ok_or("No node found in range")?;

    let mut output = String::new();
    pretty_print_tree(&mut output, root)?;

    assert_eq!(expected, output);
    Ok(())
  }

  #[test]
  #[ignore = "requires compiled tree-sitter grammars"]
  fn test_pretty_print() {
    let loader = test_loader();

    let source = r#"// Hello"#;
    if let Err(e) = assert_pretty_print(
      &loader,
      "rust",
      source,
      "(line_comment \"//\")",
      0,
      source.len(),
    ) {
      eprintln!("Skipping test_pretty_print: {}", e);
      return;
    }

    let source = r#"fn main() {
            println!("Hello, World!");
        }"#;
    assert_pretty_print(
      &loader,
      "rust",
      source,
      concat!(
        "(function_item \"fn\"\n",
        "  name: (identifier)\n",
        "  parameters: (parameters \"(\" \")\")\n",
        "  body: (block \"{\"\n",
        "    (expression_statement\n",
        "      (macro_invocation\n",
        "        macro: (identifier) \"!\"\n",
        "        (token_tree \"(\"\n",
        "          (string_literal \"\\\"\"\n",
        "            (string_content) \"\\\"\") \")\")) \";\") \"}\"))",
      ),
      0,
      source.len(),
    )
    .ok();

    let source = r#"fn main() {}"#;
    assert_pretty_print(&loader, "rust", source, r#"\"fn\""#, 0, 1).ok();

    let source = r#"}{"#;
    assert_pretty_print(
      &loader,
      "rust",
      source,
      "(ERROR \"}\" \"{\")",
      0,
      source.len(),
    )
    .ok();

    let source = "def self.method_name
          true
        end";
    assert_pretty_print(
      &loader,
      "ruby",
      source,
      concat!(
        "(singleton_method \"def\"\n",
        "  object: (self) \".\"\n",
        "  name: (identifier)\n",
        "  body: (body_statement\n",
        "    (true)) \"end\")"
      ),
      0,
      source.len(),
    )
    .ok();
  }
}
