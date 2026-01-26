//! Runtime-backed implementation of `SyntaxResources`.
//!
//! This adapter loads grammars and query files from the runtime asset layout
//! used by `the-editor`. It is feature-gated (`runtime-loader`) so the core
//! syntax crate can stay pure and embeddable.
//!
//! # Example
//!
//! ```no_run
//! # #[cfg(feature = "runtime-loader")] {
//! use std::collections::HashMap;
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
//!   runtime_loader::RuntimeLoader,
//! };
//!
//! let language = LanguageConfiguration {
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
//!   language:        vec![language],
//!   language_server: HashMap::new(),
//! };
//!
//! let _loader = Loader::new(config, RuntimeLoader::new()).expect("loader");
//! # }
//! ```

#[cfg(feature = "runtime-loader")] use std::borrow::Cow;

#[cfg(feature = "runtime-loader")]
use the_loader::grammar::{
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
