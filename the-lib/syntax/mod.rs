pub mod config;
pub mod edits;
pub mod indent_query;
pub mod rainbow_query;
pub mod registry;
pub mod resources;
pub mod state;
pub mod tag_query;
pub mod textobject_query;

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
  Syntax,
  SyntaxSnapshot,
  SyntaxState,
  SyntaxUnavailableReason,
  child_for_byte_range,
  pretty_print_tree,
};
pub use tag_query::TagQuery;
pub use textobject_query::{
  CapturedNode,
  TextObjectQuery,
};
pub use tree_house::{
  Error as HighlighterError,
  LanguageLoader,
  TREE_SITTER_MATCH_LIMIT,
  TreeCursor,
  highlighter::{
    Highlight as HighlightKindId,
    HighlightEvent,
  },
  query_iter::QueryIterEvent,
  tree_sitter,
};
