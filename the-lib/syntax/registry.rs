use std::{
  borrow::Cow,
  collections::HashMap,
  fmt,
  path::Path,
  sync::{
    Arc,
    Mutex,
    OnceLock,
  },
};

use globset::{
  GlobSet,
  GlobSetBuilder,
};
use ropey::RopeSlice;
use the_stdx::rope::{
  RopeSliceExt as _,
  regex_cursor,
};
use thiserror::Error;
use tree_house::{
  InjectionLanguageMarker,
  Language,
  LanguageConfig as SyntaxConfig,
  LanguageLoader,
  highlighter::Highlight as HighlightKindId,
  tree_sitter::{
    Query,
    query::InvalidPredicateError,
  },
};

use super::{
  QueryKind,
  config::{
    FileType,
    LanguageSyntaxConfig,
    SyntaxConfiguration,
  },
  indent_query::IndentQuery,
  rainbow_query::RainbowQuery,
  resources::SyntaxResources,
  state::{
    DocumentSyntax,
    DocumentSyntaxState,
    Syntax,
    SyntaxState,
    SyntaxUnavailableReason,
  },
  tag_query::TagQuery,
  textobject_query::TextObjectQuery,
};

pub type Result<T> = std::result::Result<T, SyntaxError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LanguageId(usize);

impl LanguageId {
  pub const fn new(index: usize) -> Self {
    Self(index)
  }

  pub const fn get(self) -> usize {
    self.0
  }

  pub(crate) fn to_backend(self) -> Language {
    Language(self.0 as u32)
  }

  pub(crate) fn from_backend(language: Language) -> Self {
    Self(language.idx())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageAssignmentSource {
  UserOverride,
  Name,
  Scope,
  Filename,
  Shebang,
  Glob,
  InjectionRegex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageAssignment {
  pub language_id: LanguageId,
  pub source:      LanguageAssignmentSource,
}

#[derive(Debug, Error)]
pub enum SyntaxError {
  #[error("duplicate language id '{language_id}'")]
  DuplicateLanguageId { language_id: String },
  #[error("duplicate language scope '{scope}'")]
  DuplicateScope { scope: String },
  #[error("duplicate language extension '{extension}'")]
  DuplicateExtension { extension: String },
  #[error("duplicate language shebang '{shebang}'")]
  DuplicateShebang { shebang: String },
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
  #[error("failed to initialize syntax for '{language_id}'")]
  BackendInit {
    language_id: String,
    #[source]
    source:      Box<dyn std::error::Error + Send + Sync>,
  },
}

#[derive(Debug)]
pub struct LanguageDefinition {
  config:           Arc<LanguageSyntaxConfig>,
  syntax:           OnceLock<Result<Option<SyntaxConfig>>>,
  indent_query:     OnceLock<Result<Option<IndentQuery>>>,
  textobject_query: OnceLock<Result<Option<TextObjectQuery>>>,
  tag_query:        OnceLock<Result<Option<TagQuery>>>,
  rainbow_query:    OnceLock<Result<Option<RainbowQuery>>>,
}

impl LanguageDefinition {
  fn new(config: LanguageSyntaxConfig) -> Self {
    Self {
      config:           Arc::new(config),
      syntax:           OnceLock::new(),
      indent_query:     OnceLock::new(),
      textobject_query: OnceLock::new(),
      tag_query:        OnceLock::new(),
      rainbow_query:    OnceLock::new(),
    }
  }

  pub fn config(&self) -> &Arc<LanguageSyntaxConfig> {
    &self.config
  }

  fn compile_syntax_config(&self, registry: &LanguageRegistry) -> Result<Option<SyntaxConfig>> {
    let syntax = &self.config;
    let Some(grammar) = registry.resources.grammar(syntax.grammar_name()) else {
      return Ok(None);
    };

    let highlights = registry.query_text(&syntax.language_id, QueryKind::Highlights);
    let injections = registry.query_text(&syntax.language_id, QueryKind::Injections);
    let locals = registry.query_text(&syntax.language_id, QueryKind::Locals);
    let config = SyntaxConfig::new(
      grammar,
      highlights.as_ref(),
      injections.as_ref(),
      locals.as_ref(),
    )
    .map_err(|err| {
      SyntaxError::SyntaxConfig {
        language_id: syntax.language_id.clone(),
        source:      Box::new(err),
      }
    })?;

    registry.configure_highlights(&config);
    Ok(Some(config))
  }

  pub(crate) fn syntax_config<'a>(
    &'a self,
    registry: &LanguageRegistry,
  ) -> Result<Option<&'a SyntaxConfig>> {
    match self
      .syntax
      .get_or_init(|| self.compile_syntax_config(registry))
    {
      Ok(Some(config)) => Ok(Some(config)),
      Ok(None) => Ok(None),
      Err(err) => Err(clone_syntax_error(err)),
    }
  }

  fn compile_indent_query(&self, registry: &LanguageRegistry) -> Result<Option<IndentQuery>> {
    let Some(grammar) = self.syntax_config(registry)?.map(|config| config.grammar) else {
      return Ok(None);
    };
    let text = registry.query_text(&self.config.language_id, QueryKind::Indents);
    if text.is_empty() {
      return Ok(None);
    }
    IndentQuery::new(grammar, text.as_ref())
      .map(Some)
      .map_err(|err| {
        SyntaxError::Query {
          language_id: self.config.language_id.clone(),
          kind:        QueryKind::Indents,
          source:      Box::new(err),
        }
      })
  }

  pub(crate) fn indent_query<'a>(
    &'a self,
    registry: &LanguageRegistry,
  ) -> Result<Option<&'a IndentQuery>> {
    match self
      .indent_query
      .get_or_init(|| self.compile_indent_query(registry))
    {
      Ok(Some(query)) => Ok(Some(query)),
      Ok(None) => Ok(None),
      Err(err) => Err(clone_syntax_error(err)),
    }
  }

  fn compile_textobject_query(
    &self,
    registry: &LanguageRegistry,
  ) -> Result<Option<TextObjectQuery>> {
    let Some(grammar) = self.syntax_config(registry)?.map(|config| config.grammar) else {
      return Ok(None);
    };
    let text = registry.query_text(&self.config.language_id, QueryKind::TextObjects);
    if text.is_empty() {
      return Ok(None);
    }
    let query = Query::new(grammar, text.as_ref(), |_, _| Ok(())).map_err(|err| {
      SyntaxError::Query {
        language_id: self.config.language_id.clone(),
        kind:        QueryKind::TextObjects,
        source:      Box::new(err),
      }
    })?;
    Ok(Some(TextObjectQuery::new(query)))
  }

  pub(crate) fn textobject_query<'a>(
    &'a self,
    registry: &LanguageRegistry,
  ) -> Result<Option<&'a TextObjectQuery>> {
    match self
      .textobject_query
      .get_or_init(|| self.compile_textobject_query(registry))
    {
      Ok(Some(query)) => Ok(Some(query)),
      Ok(None) => Ok(None),
      Err(err) => Err(clone_syntax_error(err)),
    }
  }

  fn compile_tag_query(&self, registry: &LanguageRegistry) -> Result<Option<TagQuery>> {
    let Some(grammar) = self.syntax_config(registry)?.map(|config| config.grammar) else {
      return Ok(None);
    };
    let text = registry.query_text(&self.config.language_id, QueryKind::Tags);
    if text.is_empty() {
      return Ok(None);
    }
    let query = Query::new(grammar, text.as_ref(), |_pattern, predicate| {
      match predicate {
        tree_house::tree_sitter::query::UserPredicate::IsPropertySet { key: "local", .. } => Ok(()),
        tree_house::tree_sitter::query::UserPredicate::Other(pred) => {
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
        language_id: self.config.language_id.clone(),
        kind:        QueryKind::Tags,
        source:      Box::new(err),
      }
    })?;
    Ok(Some(TagQuery { query }))
  }

  pub(crate) fn tag_query<'a>(
    &'a self,
    registry: &LanguageRegistry,
  ) -> Result<Option<&'a TagQuery>> {
    match self
      .tag_query
      .get_or_init(|| self.compile_tag_query(registry))
    {
      Ok(Some(query)) => Ok(Some(query)),
      Ok(None) => Ok(None),
      Err(err) => Err(clone_syntax_error(err)),
    }
  }

  fn compile_rainbow_query(&self, registry: &LanguageRegistry) -> Result<Option<RainbowQuery>> {
    let Some(grammar) = self.syntax_config(registry)?.map(|config| config.grammar) else {
      return Ok(None);
    };
    let text = registry.query_text(&self.config.language_id, QueryKind::Rainbows);
    if text.is_empty() {
      return Ok(None);
    }
    RainbowQuery::new(grammar, text.as_ref())
      .map(Some)
      .map_err(|err| {
        SyntaxError::Query {
          language_id: self.config.language_id.clone(),
          kind:        QueryKind::Rainbows,
          source:      Box::new(err),
        }
      })
  }

  pub(crate) fn rainbow_query<'a>(
    &'a self,
    registry: &LanguageRegistry,
  ) -> Result<Option<&'a RainbowQuery>> {
    match self
      .rainbow_query
      .get_or_init(|| self.compile_rainbow_query(registry))
    {
      Ok(Some(query)) => Ok(Some(query)),
      Ok(None) => Ok(None),
      Err(err) => Err(clone_syntax_error(err)),
    }
  }
}

#[derive(Default)]
struct HighlightVocabulary {
  names:   Vec<String>,
  by_name: HashMap<String, HighlightKindId>,
}

impl HighlightVocabulary {
  fn intern(&mut self, name: &str) -> HighlightKindId {
    if let Some(id) = self.by_name.get(name).copied() {
      return id;
    }

    let id = HighlightKindId::new(self.names.len() as u32);
    let owned = name.to_string();
    self.names.push(owned.clone());
    self.by_name.insert(owned, id);
    id
  }

  fn name(&self, id: HighlightKindId) -> Option<String> {
    self.names.get(id.get() as usize).cloned()
  }

  fn names(&self) -> Vec<String> {
    self.names.clone()
  }
}

pub struct LanguageRegistry {
  pub(crate) resources:   Arc<dyn SyntaxResources>,
  languages:              Vec<LanguageDefinition>,
  languages_by_name:      HashMap<String, LanguageId>,
  languages_by_scope:     HashMap<String, LanguageId>,
  languages_by_extension: HashMap<String, LanguageId>,
  languages_by_shebang:   HashMap<String, LanguageId>,
  language_glob_matcher:  FileTypeGlobMatcher,
  highlight_vocabulary:   Mutex<HighlightVocabulary>,
}

impl fmt::Debug for LanguageRegistry {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("LanguageRegistry")
      .field("languages", &self.languages.len())
      .field("languages_by_name", &self.languages_by_name.len())
      .field("languages_by_scope", &self.languages_by_scope.len())
      .field("languages_by_extension", &self.languages_by_extension.len())
      .field("languages_by_shebang", &self.languages_by_shebang.len())
      .finish_non_exhaustive()
  }
}

impl LanguageRegistry {
  pub fn new(
    config: SyntaxConfiguration,
    resources: impl SyntaxResources + 'static,
  ) -> Result<Self> {
    Self::with_resources(config, Arc::new(resources))
  }

  pub fn with_resources(
    config: SyntaxConfiguration,
    resources: Arc<dyn SyntaxResources>,
  ) -> Result<Self> {
    let mut languages = Vec::with_capacity(config.language.len());
    let mut languages_by_name = HashMap::new();
    let mut languages_by_scope = HashMap::new();
    let mut languages_by_extension = HashMap::new();
    let mut languages_by_shebang = HashMap::new();
    let mut globs = Vec::new();

    for language_config in config.language {
      let language_id = LanguageId::new(languages.len());

      if languages_by_name
        .insert(language_config.language_id.clone(), language_id)
        .is_some()
      {
        return Err(SyntaxError::DuplicateLanguageId {
          language_id: language_config.language_id,
        });
      }

      if languages_by_scope
        .insert(language_config.scope.clone(), language_id)
        .is_some()
      {
        return Err(SyntaxError::DuplicateScope {
          scope: language_config.scope,
        });
      }

      for shebang in &language_config.shebangs {
        if languages_by_shebang
          .insert(shebang.clone(), language_id)
          .is_some()
        {
          return Err(SyntaxError::DuplicateShebang {
            shebang: shebang.clone(),
          });
        }
      }

      for file_type in &language_config.file_types {
        match file_type {
          FileType::Extension(extension) => {
            if languages_by_extension
              .insert(extension.clone(), language_id)
              .is_some()
            {
              return Err(SyntaxError::DuplicateExtension {
                extension: extension.clone(),
              });
            }
          },
          FileType::Glob(glob) => globs.push(FileTypeGlob::new(glob.clone(), language_id)),
        }
      }

      languages.push(LanguageDefinition::new(language_config));
    }

    Ok(Self {
      resources,
      languages,
      languages_by_name,
      languages_by_scope,
      languages_by_extension,
      languages_by_shebang,
      language_glob_matcher: FileTypeGlobMatcher::new(globs).map_err(|err| {
        SyntaxError::SyntaxConfig {
          language_id: "<registry>".into(),
          source:      Box::new(err),
        }
      })?,
      highlight_vocabulary: Mutex::new(HighlightVocabulary::default()),
    })
  }

  pub fn len(&self) -> usize {
    self.languages.len()
  }

  pub fn is_empty(&self) -> bool {
    self.languages.is_empty()
  }

  pub fn languages(&self) -> impl ExactSizeIterator<Item = (LanguageId, &LanguageDefinition)> {
    self
      .languages
      .iter()
      .enumerate()
      .map(|(idx, definition)| (LanguageId::new(idx), definition))
  }

  pub fn language(&self, language_id: LanguageId) -> Option<&LanguageDefinition> {
    self.languages.get(language_id.get())
  }

  pub fn language_for_name(&self, name: &str) -> Option<LanguageId> {
    self.languages_by_name.get(name).copied()
  }

  pub fn language_for_scope(&self, scope: &str) -> Option<LanguageId> {
    self.languages_by_scope.get(scope).copied()
  }

  pub fn language_for_extension(&self, extension: &str) -> Option<LanguageId> {
    self.languages_by_extension.get(extension).copied()
  }

  pub fn language_for_glob(&self, path: &Path) -> Option<LanguageId> {
    self.language_glob_matcher.language_for_path(path)
  }

  pub fn language_for_filename(&self, path: &Path) -> Option<LanguageId> {
    self.language_for_glob(path).or_else(|| {
      path
        .extension()
        .and_then(|ext| ext.to_str())
        .and_then(|ext| self.language_for_extension(ext))
    })
  }

  pub fn language_for_shebang(&self, text: RopeSlice) -> Option<LanguageId> {
    use the_stdx::rope::Regex;
    const SHEBANG: &str = r"^#!\s*(?:\S*[/\\](?:env\s+(?:\-\S+\s+)*)?)?([^\s\.\d]+)";
    static SHEBANG_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = SHEBANG_REGEX.get_or_init(|| Regex::new(SHEBANG).unwrap());

    let marker = regex
      .captures_iter(regex_cursor::Input::new(text))
      .map(|cap| text.byte_slice(cap.get_group(1).unwrap().range()))
      .next()?;
    let shebang: Cow<'_, str> = marker.into();
    self.languages_by_shebang.get(shebang.as_ref()).copied()
  }

  pub fn language_for_match(&self, text: RopeSlice) -> Option<LanguageId> {
    let name: Cow<'_, str> = text.into();
    if let Some(language) = self.language_for_name(name.as_ref()) {
      return Some(language);
    }

    let mut best = None;
    let mut best_len = 0;
    for (language_id, definition) in self.languages() {
      if let Some(regex) = &definition.config.injection_regex
        && let Some(mat) = regex.find(text.regex_input())
      {
        let len = mat.end() - mat.start();
        if len > best_len {
          best = Some(language_id);
          best_len = len;
        }
      }
    }
    best
  }

  pub fn assign_by_name(&self, name: &str) -> Option<LanguageAssignment> {
    self.language_for_name(name).map(|language_id| {
      LanguageAssignment {
        language_id,
        source: LanguageAssignmentSource::Name,
      }
    })
  }

  pub fn assign_by_scope(&self, scope: &str) -> Option<LanguageAssignment> {
    self.language_for_scope(scope).map(|language_id| {
      LanguageAssignment {
        language_id,
        source: LanguageAssignmentSource::Scope,
      }
    })
  }

  pub fn assign_by_filename(&self, path: &Path) -> Option<LanguageAssignment> {
    path
      .extension()
      .and_then(|ext| ext.to_str())
      .and_then(|ext| self.language_for_extension(ext))
      .map(|language_id| {
        LanguageAssignment {
          language_id,
          source: LanguageAssignmentSource::Filename,
        }
      })
      .or_else(|| {
        self.language_for_glob(path).map(|language_id| {
          LanguageAssignment {
            language_id,
            source: LanguageAssignmentSource::Glob,
          }
        })
      })
  }

  pub fn assign_by_shebang(&self, text: RopeSlice) -> Option<LanguageAssignment> {
    self.language_for_shebang(text).map(|language_id| {
      LanguageAssignment {
        language_id,
        source: LanguageAssignmentSource::Shebang,
      }
    })
  }

  pub fn assign_by_match(&self, text: RopeSlice) -> Option<LanguageAssignment> {
    self.language_for_match(text).map(|language_id| {
      LanguageAssignment {
        language_id,
        source: LanguageAssignmentSource::InjectionRegex,
      }
    })
  }

  pub fn detect_language_for_document(
    &self,
    path: Option<&Path>,
    text: RopeSlice,
  ) -> Option<LanguageAssignment> {
    path
      .and_then(|path| {
        path
          .extension()
          .and_then(|ext| ext.to_str())
          .and_then(|ext| self.language_for_extension(ext))
          .map(|language_id| {
            LanguageAssignment {
              language_id,
              source: LanguageAssignmentSource::Filename,
            }
          })
          .or_else(|| self.assign_by_shebang(text))
          .or_else(|| {
            self.language_for_glob(path).map(|language_id| {
              LanguageAssignment {
                language_id,
                source: LanguageAssignmentSource::Glob,
              }
            })
          })
          .or_else(|| self.assign_by_match(text))
      })
      .or_else(|| self.assign_by_shebang(text))
      .or_else(|| self.assign_by_match(text))
  }

  pub fn syntax_config(&self, language_id: LanguageId) -> Result<Option<&SyntaxConfig>> {
    let definition = self.language(language_id).expect("invalid LanguageId");
    definition.syntax_config(self)
  }

  pub fn indent_query(&self, language_id: LanguageId) -> Result<Option<&IndentQuery>> {
    let definition = self.language(language_id).expect("invalid LanguageId");
    definition.indent_query(self)
  }

  pub fn textobject_query(&self, language_id: LanguageId) -> Result<Option<&TextObjectQuery>> {
    let definition = self.language(language_id).expect("invalid LanguageId");
    definition.textobject_query(self)
  }

  pub fn tag_query(&self, language_id: LanguageId) -> Result<Option<&TagQuery>> {
    let definition = self.language(language_id).expect("invalid LanguageId");
    definition.tag_query(self)
  }

  pub fn rainbow_query(&self, language_id: LanguageId) -> Result<Option<&RainbowQuery>> {
    let definition = self.language(language_id).expect("invalid LanguageId");
    definition.rainbow_query(self)
  }

  pub fn query_text(&self, language_id: &str, kind: QueryKind) -> String {
    tree_house::read_query(language_id, |language| {
      self
        .resources
        .query(language, kind)
        .map_or_else(String::new, Cow::into_owned)
    })
  }

  pub fn highlight_name(&self, kind: HighlightKindId) -> Option<String> {
    self.highlight_vocabulary.lock().unwrap().name(kind)
  }

  pub fn highlight_names(&self) -> Vec<String> {
    self.highlight_vocabulary.lock().unwrap().names()
  }

  pub(crate) fn configure_highlights(&self, config: &SyntaxConfig) {
    config.configure(|capture_name| Some(self.intern_highlight_kind(capture_name)));
  }

  fn intern_highlight_kind(&self, capture_name: &str) -> HighlightKindId {
    self
      .highlight_vocabulary
      .lock()
      .unwrap()
      .intern(capture_name)
  }

  pub fn create_syntax(
    &self,
    source: RopeSlice,
    language_id: LanguageId,
  ) -> Result<Option<Syntax>> {
    let Some(_config) = self.syntax_config(language_id)? else {
      return Ok(None);
    };

    Syntax::new(source, language_id, self)
      .map(Some)
      .map_err(|err| {
        SyntaxError::BackendInit {
          language_id: self
            .language(language_id)
            .expect("invalid LanguageId")
            .config
            .language_id
            .clone(),
          source:      Box::new(std::io::Error::other(err.to_string())),
        }
      })
  }

  pub fn activate_syntax(
    &self,
    source: RopeSlice,
    assignment: LanguageAssignment,
  ) -> DocumentSyntaxState {
    match self.create_syntax(source, assignment.language_id) {
      Ok(Some(syntax)) => {
        DocumentSyntaxState::Active(SyntaxState::new(assignment.language_id, syntax))
      },
      Ok(None) => DocumentSyntaxState::Unavailable(SyntaxUnavailableReason::MissingGrammar),
      Err(SyntaxError::MissingGrammar { .. }) => {
        DocumentSyntaxState::Unavailable(SyntaxUnavailableReason::MissingGrammar)
      },
      Err(SyntaxError::SyntaxConfig { .. }) => {
        DocumentSyntaxState::Unavailable(SyntaxUnavailableReason::InvalidSyntaxConfig)
      },
      Err(SyntaxError::Query { kind, .. }) => {
        DocumentSyntaxState::Unavailable(SyntaxUnavailableReason::InvalidQuery { kind })
      },
      Err(SyntaxError::BackendInit { .. }) => {
        DocumentSyntaxState::Unavailable(SyntaxUnavailableReason::BackendInit)
      },
      Err(
        SyntaxError::DuplicateLanguageId { .. }
        | SyntaxError::DuplicateScope { .. }
        | SyntaxError::DuplicateExtension { .. }
        | SyntaxError::DuplicateShebang { .. },
      ) => DocumentSyntaxState::Unavailable(SyntaxUnavailableReason::InvalidSyntaxConfig),
    }
  }

  pub fn deferred_document_syntax(&self, assignment: LanguageAssignment) -> DocumentSyntax {
    DocumentSyntax::Assigned {
      assignment,
      state: DocumentSyntaxState::Deferred,
    }
  }

  pub fn activate_document_syntax(
    &self,
    source: RopeSlice,
    assignment: LanguageAssignment,
  ) -> DocumentSyntax {
    DocumentSyntax::Assigned {
      assignment,
      state: self.activate_syntax(source, assignment),
    }
  }
}

impl LanguageLoader for LanguageRegistry {
  fn language_for_marker(&self, marker: InjectionLanguageMarker) -> Option<Language> {
    match marker {
      InjectionLanguageMarker::Name(name) => {
        self
          .language_for_name(name.as_ref())
          .map(LanguageId::to_backend)
      },
      InjectionLanguageMarker::Match(text) => {
        self.language_for_match(text).map(LanguageId::to_backend)
      },
      InjectionLanguageMarker::Filename(text) => {
        let path: Cow<'_, str> = text.into();
        self
          .language_for_filename(Path::new(path.as_ref()))
          .map(LanguageId::to_backend)
      },
      InjectionLanguageMarker::Shebang(text) => {
        let shebang: Cow<'_, str> = text.into();
        self
          .languages_by_shebang
          .get(shebang.as_ref())
          .copied()
          .map(LanguageId::to_backend)
      },
    }
  }

  fn get_config(&self, lang: Language) -> Option<&SyntaxConfig> {
    let language_id = LanguageId::from_backend(lang);
    self.syntax_config(language_id).ok().flatten()
  }
}

#[derive(Debug)]
struct FileTypeGlob {
  glob:        globset::Glob,
  language_id: LanguageId,
}

impl FileTypeGlob {
  fn new(glob: globset::Glob, language_id: LanguageId) -> Self {
    Self { glob, language_id }
  }
}

#[derive(Debug, Default)]
struct FileTypeGlobMatcher {
  matcher:    GlobSet,
  file_types: Vec<FileTypeGlob>,
}

impl FileTypeGlobMatcher {
  fn new(file_types: Vec<FileTypeGlob>) -> std::result::Result<Self, globset::Error> {
    let mut builder = GlobSetBuilder::new();
    for file_type in &file_types {
      builder.add(file_type.glob.clone());
    }

    Ok(Self {
      matcher: builder.build()?,
      file_types,
    })
  }

  fn language_for_path(&self, path: &Path) -> Option<LanguageId> {
    self
      .matcher
      .matches(path)
      .iter()
      .filter_map(|idx| self.file_types.get(*idx))
      .max_by_key(|file_type| file_type.glob.glob().len())
      .map(|file_type| file_type.language_id)
  }
}

fn clone_syntax_error(error: &SyntaxError) -> SyntaxError {
  match error {
    SyntaxError::DuplicateLanguageId { language_id } => {
      SyntaxError::DuplicateLanguageId {
        language_id: language_id.clone(),
      }
    },
    SyntaxError::DuplicateScope { scope } => {
      SyntaxError::DuplicateScope {
        scope: scope.clone(),
      }
    },
    SyntaxError::DuplicateExtension { extension } => {
      SyntaxError::DuplicateExtension {
        extension: extension.clone(),
      }
    },
    SyntaxError::DuplicateShebang { shebang } => {
      SyntaxError::DuplicateShebang {
        shebang: shebang.clone(),
      }
    },
    SyntaxError::MissingGrammar { language_id } => {
      SyntaxError::MissingGrammar {
        language_id: language_id.clone(),
      }
    },
    SyntaxError::SyntaxConfig {
      language_id,
      source,
    } => {
      SyntaxError::SyntaxConfig {
        language_id: language_id.clone(),
        source:      Box::new(std::io::Error::other(source.to_string())),
      }
    },
    SyntaxError::Query {
      language_id,
      kind,
      source,
    } => {
      SyntaxError::Query {
        language_id: language_id.clone(),
        kind:        *kind,
        source:      Box::new(std::io::Error::other(source.to_string())),
      }
    },
    SyntaxError::BackendInit {
      language_id,
      source,
    } => {
      SyntaxError::BackendInit {
        language_id: language_id.clone(),
        source:      Box::new(std::io::Error::other(source.to_string())),
      }
    },
  }
}

#[cfg(test)]
mod test {
  use std::{
    borrow::Cow,
    path::Path,
  };

  use ropey::Rope;
  use the_stdx::rope;

  use super::*;
  use crate::syntax::{
    resources::SyntaxResources,
    tree_sitter::Grammar,
  };

  #[derive(Debug, Default)]
  struct InMemoryResources {
    queries: HashMap<(String, QueryKind), String>,
  }

  impl SyntaxResources for InMemoryResources {
    fn grammar(&self, _grammar_name: &str) -> Option<Grammar> {
      None
    }

    fn query(&self, language_id: &str, kind: QueryKind) -> Option<Cow<'_, str>> {
      self
        .queries
        .get(&(language_id.to_string(), kind))
        .map(|query| Cow::Borrowed(query.as_str()))
    }
  }

  fn language(language_id: &str, file_types: Vec<FileType>) -> LanguageSyntaxConfig {
    LanguageSyntaxConfig {
      language_id: language_id.to_string(),
      scope: format!("source.{language_id}"),
      file_types,
      shebangs: Vec::new(),
      comment_tokens: None,
      block_comment_tokens: None,
      text_width: None,
      soft_wrap: None,
      grammar: None,
      injection_regex: None,
      indent: None,
      auto_pairs: None,
      rulers: None,
      rainbow_brackets: None,
    }
  }

  #[test]
  fn registry_rejects_duplicate_exact_language_detectors() {
    assert!(matches!(
      LanguageRegistry::new(
        SyntaxConfiguration {
          language: vec![
            language("rust", vec![FileType::Extension("rs".into())]),
            language("rust", vec![FileType::Extension("rb".into())]),
          ],
        },
        InMemoryResources::default(),
      ),
      Err(SyntaxError::DuplicateLanguageId { .. })
    ));

    let mut ruby = language("ruby", vec![FileType::Extension("rb".into())]);
    ruby.scope = "source.rust".into();
    assert!(matches!(
      LanguageRegistry::new(
        SyntaxConfiguration {
          language: vec![
            language("rust", vec![FileType::Extension("rs".into())]),
            ruby
          ],
        },
        InMemoryResources::default(),
      ),
      Err(SyntaxError::DuplicateScope { .. })
    ));

    assert!(matches!(
      LanguageRegistry::new(
        SyntaxConfiguration {
          language: vec![
            language("rust", vec![FileType::Extension("rs".into())]),
            language("ruby", vec![FileType::Extension("rs".into())]),
          ],
        },
        InMemoryResources::default(),
      ),
      Err(SyntaxError::DuplicateExtension { .. })
    ));

    let mut python = language("python", vec![FileType::Extension("py".into())]);
    python.shebangs = vec!["python".into()];
    let mut ruby = language("ruby", vec![FileType::Extension("rb".into())]);
    ruby.shebangs = vec!["python".into()];
    assert!(matches!(
      LanguageRegistry::new(
        SyntaxConfiguration {
          language: vec![python, ruby],
        },
        InMemoryResources::default(),
      ),
      Err(SyntaxError::DuplicateShebang { .. })
    ));
  }

  #[test]
  fn registry_detects_languages_by_exact_and_heuristic_sources() {
    let mut python = language("python", vec![FileType::Extension("py".into())]);
    python.shebangs = vec!["python".into()];

    let mut markdown = language("markdown", vec![FileType::Glob(
      globset::Glob::new("*/docs/*.md").unwrap(),
    )]);
    markdown.injection_regex = Some(rope::Regex::new("```(markdown|md)").unwrap());

    let registry = LanguageRegistry::new(
      SyntaxConfiguration {
        language: vec![
          language("rust", vec![FileType::Extension("rs".into())]),
          python,
          markdown,
        ],
      },
      InMemoryResources::default(),
    )
    .unwrap();

    assert_eq!(registry.language_for_name("rust"), Some(LanguageId::new(0)));
    assert_eq!(
      registry.language_for_scope("source.python"),
      Some(LanguageId::new(1))
    );
    assert_eq!(
      registry.assign_by_filename(Path::new("src/lib.rs")),
      Some(LanguageAssignment {
        language_id: LanguageId::new(0),
        source:      LanguageAssignmentSource::Filename,
      })
    );
    assert_eq!(
      registry.assign_by_shebang(Rope::from("#!/usr/bin/env python\nprint()\n").slice(..)),
      Some(LanguageAssignment {
        language_id: LanguageId::new(1),
        source:      LanguageAssignmentSource::Shebang,
      })
    );
    assert_eq!(
      registry.assign_by_filename(Path::new("/tmp/docs/guide.md")),
      Some(LanguageAssignment {
        language_id: LanguageId::new(2),
        source:      LanguageAssignmentSource::Glob,
      })
    );
    assert_eq!(
      registry.assign_by_match(Rope::from("```markdown\n# title\n```").slice(..)),
      Some(LanguageAssignment {
        language_id: LanguageId::new(2),
        source:      LanguageAssignmentSource::InjectionRegex,
      })
    );
  }

  #[test]
  fn registry_prefers_extension_then_shebang_then_glob_then_match_when_detecting_documents() {
    let mut shell = language("shell", vec![FileType::Extension("sh".into())]);
    shell.shebangs = vec!["bash".into()];

    let mut markdown = language("markdown", vec![FileType::Glob(
      globset::Glob::new("*/docs/*.txt").unwrap(),
    )]);
    markdown.injection_regex = Some(rope::Regex::new("markdown").unwrap());

    let registry = LanguageRegistry::new(
      SyntaxConfiguration {
        language: vec![shell, markdown],
      },
      InMemoryResources::default(),
    )
    .unwrap();

    let text = Rope::from("#!/usr/bin/env bash\nmarkdown\n");
    assert_eq!(
      registry.detect_language_for_document(Some(Path::new("notes.sh")), text.slice(..)),
      Some(LanguageAssignment {
        language_id: LanguageId::new(0),
        source:      LanguageAssignmentSource::Filename,
      })
    );

    assert_eq!(
      registry
        .detect_language_for_document(Some(Path::new("/tmp/docs/readme.txt")), text.slice(..)),
      Some(LanguageAssignment {
        language_id: LanguageId::new(0),
        source:      LanguageAssignmentSource::Shebang,
      })
    );

    let text = Rope::from("plain markdown body");
    assert_eq!(
      registry
        .detect_language_for_document(Some(Path::new("/tmp/docs/readme.txt")), text.slice(..)),
      Some(LanguageAssignment {
        language_id: LanguageId::new(1),
        source:      LanguageAssignmentSource::Glob,
      })
    );
    assert_eq!(
      registry.detect_language_for_document(None, text.slice(..)),
      Some(LanguageAssignment {
        language_id: LanguageId::new(1),
        source:      LanguageAssignmentSource::InjectionRegex,
      })
    );
  }

  #[test]
  fn registry_uses_longest_glob_and_longest_regex_match() {
    let mut general = language("general", vec![FileType::Glob(
      globset::Glob::new("*/docs/*.md").unwrap(),
    )]);
    general.injection_regex = Some(rope::Regex::new("md").unwrap());
    let mut specific = language("specific", vec![FileType::Glob(
      globset::Glob::new("*/docs/reference/*.md").unwrap(),
    )]);
    specific.injection_regex = Some(rope::Regex::new("markdown").unwrap());

    let registry = LanguageRegistry::new(
      SyntaxConfiguration {
        language: vec![general, specific],
      },
      InMemoryResources::default(),
    )
    .unwrap();

    assert_eq!(
      registry.language_for_filename(Path::new("/tmp/docs/reference/guide.md")),
      Some(LanguageId::new(1))
    );
    assert_eq!(
      registry.language_for_match(Rope::from("markdown").slice(..)),
      Some(LanguageId::new(1))
    );
  }

  #[test]
  fn activate_document_syntax_records_missing_grammar_as_unavailable() {
    let registry = LanguageRegistry::new(
      SyntaxConfiguration {
        language: vec![language("rust", vec![FileType::Extension("rs".into())])],
      },
      InMemoryResources::default(),
    )
    .unwrap();
    let assignment = LanguageAssignment {
      language_id: LanguageId::new(0),
      source:      LanguageAssignmentSource::Filename,
    };

    let deferred = registry.deferred_document_syntax(assignment);
    assert!(matches!(deferred, DocumentSyntax::Assigned {
      assignment: _,
      state:      DocumentSyntaxState::Deferred,
    }));

    let active =
      registry.activate_document_syntax(Rope::from("fn main() {}\n").slice(..), assignment);
    assert!(matches!(active, DocumentSyntax::Assigned {
      assignment: _,
      state:      DocumentSyntaxState::Unavailable(SyntaxUnavailableReason::MissingGrammar),
    }));
  }

  #[test]
  fn query_text_expands_inherits_directives() {
    let mut resources = InMemoryResources::default();
    resources.queries.insert(
      ("rust".into(), QueryKind::Highlights),
      "(identifier) @variable".into(),
    );
    resources.queries.insert(
      ("rust-format-args-macro".into(), QueryKind::Highlights),
      "; inherits: rust".into(),
    );

    let registry = LanguageRegistry::new(
      SyntaxConfiguration {
        language: vec![
          language("rust", vec![FileType::Extension("rs".into())]),
          language("rust-format-args-macro", vec![FileType::Extension(
            "rfam".into(),
          )]),
        ],
      },
      resources,
    )
    .unwrap();

    let text = registry.query_text("rust-format-args-macro", QueryKind::Highlights);
    assert!(text.contains("(identifier) @variable"));
  }
}
