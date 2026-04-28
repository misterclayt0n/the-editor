use std::{
  borrow::Cow,
  fmt,
  ops::{
    self,
    RangeBounds,
  },
  sync::{
    Arc,
    atomic::{
      AtomicBool,
      AtomicU64,
      Ordering,
    },
  },
  time::Duration,
};

use parking_lot::Mutex;
use ropey::{
  Rope,
  RopeSlice,
};
use tree_house::{
  Error,
  Layer,
  query_iter::QueryIter,
  tree_sitter::{
    self,
    InputEdit,
    Node,
    Query,
    Tree,
  },
};

use super::{
  HighlightKindId,
  LanguageId,
  LanguageRegistry,
  QueryKind,
  edits::generate_edits,
  tag_query::TagQuery,
};
use crate::transaction::ChangeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxUnavailableReason {
  MissingGrammar,
  InvalidSyntaxConfig,
  InvalidQuery { kind: QueryKind },
  BackendInit,
}

#[derive(Debug, Clone)]
pub enum DocumentSyntaxState {
  Deferred,
  Unavailable(SyntaxUnavailableReason),
  Active(SyntaxState),
}

#[derive(Debug, Clone)]
pub enum DocumentSyntax {
  Unassigned,
  Assigned {
    assignment: super::LanguageAssignment,
    state:      DocumentSyntaxState,
  },
}

impl DocumentSyntax {
  pub fn assignment(&self) -> Option<super::LanguageAssignment> {
    match self {
      Self::Unassigned => None,
      Self::Assigned { assignment, .. } => Some(*assignment),
    }
  }

  pub fn state(&self) -> Option<&DocumentSyntaxState> {
    match self {
      Self::Unassigned => None,
      Self::Assigned { state, .. } => Some(state),
    }
  }
}

#[derive(Debug, Clone)]
pub struct Syntax {
  inner: tree_house::Syntax,
}

pub struct SyntaxState {
  language_id: LanguageId,
  inner:       Arc<SyntaxStateInner>,
}

struct SyntaxStateInner {
  syntax:               Mutex<Syntax>,
  interpolated_version: AtomicU64,
  parsed_version:       AtomicU64,
  parse_pending:        AtomicBool,
}

impl fmt::Debug for SyntaxStateInner {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
      language_id: self.language_id,
      inner:       Arc::clone(&self.inner),
    }
  }
}

impl fmt::Debug for SyntaxState {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("SyntaxState")
      .field("language_id", &self.language_id)
      .field("inner", &self.inner)
      .finish()
  }
}

pub struct SyntaxSnapshot {
  pub language_id: LanguageId,
  pub syntax:      Syntax,
  pub source:      Rope,
  pub version:     u64,
}

pub const PARSE_TIMEOUT: Duration = Duration::from_millis(500);

impl SyntaxState {
  pub fn new(language_id: LanguageId, syntax: Syntax) -> Self {
    Self {
      language_id,
      inner: Arc::new(SyntaxStateInner {
        syntax:               Mutex::new(syntax),
        interpolated_version: AtomicU64::new(0),
        parsed_version:       AtomicU64::new(0),
        parse_pending:        AtomicBool::new(false),
      }),
    }
  }

  pub fn language_id(&self) -> LanguageId {
    self.language_id
  }

  pub fn interpolate(&self, edits: &[InputEdit]) {
    let mut syntax = self.inner.syntax.lock();
    syntax.inner.interpolate(edits);
    self
      .inner
      .interpolated_version
      .fetch_add(1, Ordering::Release);
  }

  pub fn snapshot(&self, source: Rope) -> SyntaxSnapshot {
    let syntax = self.inner.syntax.lock();
    SyntaxSnapshot {
      language_id: self.language_id,
      syntax: syntax.clone(),
      source,
      version: self.inner.interpolated_version.load(Ordering::Acquire),
    }
  }

  pub fn apply_parsed(&self, snapshot: SyntaxSnapshot) -> bool {
    let current_version = self.inner.interpolated_version.load(Ordering::Acquire);
    if snapshot.language_id != self.language_id || snapshot.version != current_version {
      return false;
    }

    let mut syntax = self.inner.syntax.lock();
    *syntax = snapshot.syntax;
    self
      .inner
      .parsed_version
      .store(snapshot.version, Ordering::Release);
    self.inner.parse_pending.store(false, Ordering::Release);
    true
  }

  pub fn needs_reparse(&self) -> bool {
    self.interpolated_version() > self.parsed_version()
  }

  pub fn interpolated_version(&self) -> u64 {
    self.inner.interpolated_version.load(Ordering::Acquire)
  }

  pub fn parsed_version(&self) -> u64 {
    self.inner.parsed_version.load(Ordering::Acquire)
  }

  pub fn is_parse_pending(&self) -> bool {
    self.inner.parse_pending.load(Ordering::Acquire)
  }

  pub fn set_parse_pending(&self, pending: bool) {
    self.inner.parse_pending.store(pending, Ordering::Release)
  }

  pub fn is_accurate(&self) -> bool {
    self.parsed_version() >= self.interpolated_version()
  }

  pub fn syntax(&self) -> impl std::ops::Deref<Target = Syntax> + '_ {
    self.inner.syntax.lock()
  }

  pub fn syntax_mut(&self) -> impl std::ops::DerefMut<Target = Syntax> + '_ {
    self.inner.syntax.lock()
  }
}

impl Syntax {
  pub fn new(
    source: RopeSlice,
    language_id: LanguageId,
    registry: &LanguageRegistry,
  ) -> std::result::Result<Self, Error> {
    let inner = tree_house::Syntax::new(source, language_id.to_backend(), PARSE_TIMEOUT, registry)?;
    Ok(Self { inner })
  }

  pub fn update(
    &mut self,
    old_source: RopeSlice,
    source: RopeSlice,
    changeset: &ChangeSet,
    registry: &LanguageRegistry,
  ) -> std::result::Result<(), Error> {
    let edits = generate_edits(old_source, changeset);
    self.update_with_edits(source, &edits, registry)
  }

  pub fn update_with_edits(
    &mut self,
    source: RopeSlice,
    edits: &[InputEdit],
    registry: &LanguageRegistry,
  ) -> std::result::Result<(), Error> {
    if edits.is_empty() {
      Ok(())
    } else {
      self.inner.update(source, PARSE_TIMEOUT, edits, registry)
    }
  }

  pub fn reparse(
    &mut self,
    source: RopeSlice,
    registry: &LanguageRegistry,
  ) -> std::result::Result<(), Error> {
    self.inner.update(source, PARSE_TIMEOUT, &[], registry)
  }

  pub fn try_reparse_with_timeout(
    &mut self,
    source: RopeSlice,
    registry: &LanguageRegistry,
    timeout: Duration,
  ) -> std::result::Result<bool, Error> {
    match self.inner.update(source, timeout, &[], registry) {
      Ok(()) => Ok(true),
      Err(Error::Timeout) => Ok(false),
      Err(err) => Err(err),
    }
  }

  pub fn try_update_with_short_timeout(
    &mut self,
    source: RopeSlice,
    edits: &[InputEdit],
    registry: &LanguageRegistry,
    timeout: Duration,
  ) -> std::result::Result<bool, Error> {
    if edits.is_empty() {
      return Ok(true);
    }
    match self.inner.update(source, timeout, edits, registry) {
      Ok(()) => Ok(true),
      Err(Error::Timeout) => Ok(false),
      Err(err) => Err(err),
    }
  }

  pub fn interpolate(&mut self, old_source: RopeSlice, changeset: &ChangeSet) {
    let edits = generate_edits(old_source, changeset);
    self.interpolate_with_edits(&edits);
  }

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

  pub fn root_language(&self) -> LanguageId {
    LanguageId::from_backend(self.layer(self.root_layer()).language)
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

  pub fn walk(&self) -> tree_house::TreeCursor<'_> {
    self.inner.walk()
  }

  pub fn scopes_at<'a>(&'a self, text: RopeSlice<'a>, pos: usize) -> Vec<&'a str> {
    let mut scopes = Vec::new();
    let byte_pos = text.char_to_byte(pos) as u32;
    let Some(mut node) = self
      .tree()
      .root_node()
      .descendant_for_byte_range(byte_pos, byte_pos)
    else {
      return scopes;
    };

    scopes.push(node.kind());
    while let Some(parent) = node.parent() {
      scopes.push(parent.kind());
      node = parent;
    }
    scopes.reverse();
    scopes
  }

  pub fn injections_enabled(&self) -> bool {
    self.inner.injections_enabled()
  }

  pub fn set_injections_enabled(&mut self, enabled: bool) {
    self.inner.set_injections_enabled(enabled)
  }

  pub fn highlighter<'a>(
    &'a self,
    source: RopeSlice<'a>,
    registry: &'a LanguageRegistry,
    range: impl RangeBounds<u32>,
  ) -> tree_house::highlighter::Highlighter<'a, 'a, LanguageRegistry> {
    tree_house::highlighter::Highlighter::new(&self.inner, source, registry, range)
  }

  pub fn collect_highlights(
    &self,
    source: RopeSlice,
    registry: &LanguageRegistry,
    byte_range: ops::Range<usize>,
  ) -> Vec<(HighlightKindId, ops::Range<usize>)> {
    let len_bytes = source.len_bytes();
    if len_bytes == 0 {
      return Vec::new();
    }

    let start = byte_range.start.min(len_bytes);
    let end = byte_range.end.min(len_bytes);
    if start >= end {
      return Vec::new();
    }

    let root_end = self.tree().root_node().end_byte() as usize;
    if root_end > len_bytes {
      return Vec::new();
    }

    let collected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      self
        .highlighter(source, registry, start as u32..end as u32)
        .collect_highlights()
    }));

    match collected {
      Ok(highlights) => {
        highlights
          .into_iter()
          .map(|(highlight, range)| (highlight, range.start as usize..range.end as usize))
          .collect()
      },
      Err(_) => Vec::new(),
    }
  }

  pub fn query_iter<'a, QueryLoader, LayerState, Range>(
    &'a self,
    source: RopeSlice<'a>,
    loader: QueryLoader,
    range: Range,
  ) -> QueryIter<'a, 'a, QueryLoader, LayerState>
  where
    QueryLoader: FnMut(tree_house::Language) -> Option<&'a Query> + 'a,
    LayerState: Default,
    Range: RangeBounds<u32>,
  {
    QueryIter::new(&self.inner, source, loader, range)
  }

  pub fn tags<'a>(
    &'a self,
    source: RopeSlice<'a>,
    registry: &'a LanguageRegistry,
    range: impl RangeBounds<u32>,
  ) -> QueryIter<'a, 'a, impl FnMut(tree_house::Language) -> Option<&'a Query> + 'a, ()> {
    self.query_iter(
      source,
      |language| {
        registry
          .tag_query(LanguageId::from_backend(language))
          .ok()
          .flatten()
          .map(|query: &TagQuery| &query.query)
      },
      range,
    )
  }
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
    debug_assert!(moved);
    debug_assert!(cursor.node() == node);
  }

  if visible {
    fmt.write_char(')')?;
  }

  Ok(())
}

pub fn child_for_byte_range<'a>(node: &Node<'a>, range: ops::Range<u32>) -> Option<Node<'a>> {
  for child in node.children() {
    let child_range = child.byte_range();
    if range.start >= child_range.start && range.end <= child_range.end {
      return Some(child);
    }
  }
  None
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::syntax::{
    LanguageAssignment,
    LanguageAssignmentSource,
  };

  #[test]
  fn document_syntax_reports_assignment_and_state_when_assigned() {
    let assignment = LanguageAssignment {
      language_id: LanguageId::new(7),
      source:      LanguageAssignmentSource::UserOverride,
    };
    let syntax = DocumentSyntax::Assigned {
      assignment,
      state: DocumentSyntaxState::Unavailable(SyntaxUnavailableReason::MissingGrammar),
    };

    assert_eq!(syntax.assignment(), Some(assignment));
    assert!(matches!(
      syntax.state(),
      Some(DocumentSyntaxState::Unavailable(
        SyntaxUnavailableReason::MissingGrammar
      ))
    ));
  }

  #[test]
  fn document_syntax_reports_absence_when_unassigned() {
    let syntax = DocumentSyntax::Unassigned;

    assert!(syntax.assignment().is_none());
    assert!(syntax.state().is_none());
  }
}
