pub mod config;
pub mod edits;
pub mod indent_query;
pub mod rainbow_query;
pub mod registry;
pub mod resources;
pub mod state;
pub mod tag_query;
pub mod textobject_query;

pub const TREE_SITTER_MATCH_LIMIT: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HighlightKindId(u32);

impl HighlightKindId {
  pub const fn new(id: u32) -> Self {
    Self(id)
  }

  pub const fn get(self) -> u32 {
    self.0
  }
}

pub use config::{
  BlockCommentToken,
  FileType,
  IndentationConfiguration,
  IndentationHeuristic,
  LanguageSyntaxConfig,
  SoftWrap,
  SyntaxConfiguration,
  WordCompletion,
};
pub use edits::generate_edits;
pub use indent_query::IndentQuery;
pub use rainbow_query::RainbowQuery;
pub use registry::{
  LanguageAssignment,
  LanguageAssignmentSource,
  LanguageDefinition,
  LanguageId,
  LanguageRegistry,
  SyntaxError,
};
pub use resources::{
  NullResources,
  QueryKind,
  SyntaxResources,
};
pub use state::{
  DocumentSyntax,
  DocumentSyntaxState,
  InvalidatedByteRanges,
  Syntax,
  SyntaxBackendError,
  SyntaxGeneration,
  SyntaxHighlightRun,
  SyntaxInvalidation,
  SyntaxSnapshot,
  SyntaxState,
  SyntaxUnavailableReason,
  child_for_byte_range,
  pretty_print_tree,
};
pub use textobject_query::{
  CapturedNode,
  TextObjectQuery,
};
pub use tree_sitter_bindings as tree_sitter;
