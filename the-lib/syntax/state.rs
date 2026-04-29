use std::{
  borrow::Cow,
  fmt,
  ops,
  sync::{
    Arc,
    atomic::{
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
use smallvec::SmallVec;
use thiserror::Error;

use super::{
  HighlightKindId,
  LanguageId,
  LanguageRegistry,
  QueryKind,
  TREE_SITTER_MATCH_LIMIT,
  edits::generate_edits,
  tree_sitter::{
    self,
    InactiveQueryCursor,
    InputEdit,
    Node,
    Parser,
    RopeInput,
    Tree,
  },
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

#[derive(Debug, Error)]
pub enum SyntaxBackendError {
  #[error("parser timed out")]
  Timeout,

  #[error(transparent)]
  IncompatibleGrammar(#[from] tree_sitter::IncompatibleGrammarError),

  #[error("parser did not produce a syntax tree")]
  ParseFailed,
}

#[derive(Debug, Clone)]
pub struct Syntax {
  language_id:        LanguageId,
  tree:               Tree,
  injections_enabled: bool,
}

pub struct SyntaxState {
  language_id: LanguageId,
  inner:       Arc<SyntaxStateInner>,
}

struct SyntaxStateInner {
  syntax:                 Mutex<Syntax>,
  edit_generation:        AtomicU64,
  tree_generation:        AtomicU64,
  baseline_generation:    AtomicU64,
  enrichment_generation:  AtomicU64,
  invalidation_and_queue: Mutex<SyntaxInvalidationState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxGeneration {
  pub edit:       u64,
  pub tree:       u64,
  pub baseline:   u64,
  pub enrichment: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxHighlightRun {
  pub byte_range: ops::Range<usize>,
  pub kind:       HighlightKindId,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyntaxInvalidation {
  pub tree:       InvalidatedByteRanges,
  pub baseline:   InvalidatedByteRanges,
  pub enrichment: InvalidatedByteRanges,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InvalidatedByteRanges {
  ranges: SmallVec<[ops::Range<usize>; 2]>,
}

#[derive(Debug, Default)]
struct SyntaxInvalidationState {
  invalidation:  SyntaxInvalidation,
  parse_pending: bool,
}

pub struct SyntaxSnapshot {
  pub language_id:     LanguageId,
  pub syntax:          Syntax,
  pub source:          Rope,
  pub edit_generation: u64,
}

pub const PARSE_TIMEOUT: Duration = Duration::from_millis(500);

impl fmt::Debug for SyntaxStateInner {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("SyntaxStateInner")
      .field(
        "edit_generation",
        &self.edit_generation.load(Ordering::Relaxed),
      )
      .field(
        "tree_generation",
        &self.tree_generation.load(Ordering::Relaxed),
      )
      .field(
        "baseline_generation",
        &self.baseline_generation.load(Ordering::Relaxed),
      )
      .field(
        "enrichment_generation",
        &self.enrichment_generation.load(Ordering::Relaxed),
      )
      .field(
        "invalidation",
        &self.invalidation_and_queue.lock().invalidation,
      )
      .field(
        "parse_pending",
        &self.invalidation_and_queue.lock().parse_pending,
      )
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

impl InvalidatedByteRanges {
  pub fn is_empty(&self) -> bool {
    self.ranges.is_empty()
  }

  pub fn len(&self) -> usize {
    self.ranges.len()
  }

  pub fn iter(&self) -> impl ExactSizeIterator<Item = &ops::Range<usize>> {
    self.ranges.iter()
  }

  pub fn intersects(&self, range: &ops::Range<usize>) -> bool {
    self
      .ranges
      .iter()
      .any(|candidate| candidate.start < range.end && range.start < candidate.end)
  }

  pub fn clear(&mut self) {
    self.ranges.clear();
  }

  fn insert(&mut self, mut range: ops::Range<usize>) {
    if range.start >= range.end {
      return;
    }

    let mut idx = 0;
    while idx < self.ranges.len() {
      let current = &self.ranges[idx];
      if range.end < current.start {
        break;
      }
      if current.end < range.start {
        idx += 1;
        continue;
      }

      range.start = range.start.min(current.start);
      range.end = range.end.max(current.end);
      self.ranges.remove(idx);
    }

    self.ranges.insert(idx, range);
  }
}

impl SyntaxState {
  pub fn new(language_id: LanguageId, syntax: Syntax) -> Self {
    Self {
      language_id,
      inner: Arc::new(SyntaxStateInner {
        syntax:                 Mutex::new(syntax),
        edit_generation:        AtomicU64::new(0),
        tree_generation:        AtomicU64::new(0),
        baseline_generation:    AtomicU64::new(0),
        enrichment_generation:  AtomicU64::new(0),
        invalidation_and_queue: Mutex::new(SyntaxInvalidationState::default()),
      }),
    }
  }

  pub fn language_id(&self) -> LanguageId {
    self.language_id
  }

  pub fn interpolate(&self, edits: &[InputEdit]) {
    let mut syntax = self.inner.syntax.lock();
    syntax.interpolate_with_edits(edits);
    drop(syntax);

    if edits.is_empty() {
      return;
    }

    self.inner.edit_generation.fetch_add(1, Ordering::Release);

    let mut state = self.inner.invalidation_and_queue.lock();
    for range in invalidated_ranges_for_edits(edits) {
      state.invalidation.tree.insert(range.clone());
      state.invalidation.baseline.insert(range.clone());
      state.invalidation.enrichment.insert(range);
    }
    state.parse_pending = true;
  }

  pub fn snapshot(&self, source: Rope) -> SyntaxSnapshot {
    let syntax = self.inner.syntax.lock();
    SyntaxSnapshot {
      language_id: self.language_id,
      syntax: syntax.clone(),
      source,
      edit_generation: self.edit_generation(),
    }
  }

  pub fn apply_parsed(&self, snapshot: SyntaxSnapshot) -> bool {
    let current_generation = self.edit_generation();
    if snapshot.language_id != self.language_id || snapshot.edit_generation != current_generation {
      return false;
    }

    let mut syntax = self.inner.syntax.lock();
    *syntax = snapshot.syntax;
    drop(syntax);

    self
      .inner
      .tree_generation
      .store(snapshot.edit_generation, Ordering::Release);
    self
      .inner
      .baseline_generation
      .store(snapshot.edit_generation, Ordering::Release);
    self
      .inner
      .enrichment_generation
      .store(snapshot.edit_generation, Ordering::Release);

    let mut state = self.inner.invalidation_and_queue.lock();
    state.invalidation = SyntaxInvalidation::default();
    state.parse_pending = false;
    true
  }

  pub fn generation(&self) -> SyntaxGeneration {
    SyntaxGeneration {
      edit:       self.edit_generation(),
      tree:       self.tree_generation(),
      baseline:   self.baseline_generation(),
      enrichment: self.enrichment_generation(),
    }
  }

  pub fn invalidation(&self) -> SyntaxInvalidation {
    self
      .inner
      .invalidation_and_queue
      .lock()
      .invalidation
      .clone()
  }

  pub fn needs_reparse(&self) -> bool {
    !self.invalidation().tree.is_empty()
  }

  pub fn edit_generation(&self) -> u64 {
    self.inner.edit_generation.load(Ordering::Acquire)
  }

  pub fn tree_generation(&self) -> u64 {
    self.inner.tree_generation.load(Ordering::Acquire)
  }

  pub fn baseline_generation(&self) -> u64 {
    self.inner.baseline_generation.load(Ordering::Acquire)
  }

  pub fn enrichment_generation(&self) -> u64 {
    self.inner.enrichment_generation.load(Ordering::Acquire)
  }

  pub fn interpolated_version(&self) -> u64 {
    self.edit_generation()
  }

  pub fn parsed_version(&self) -> u64 {
    self.tree_generation()
  }

  pub fn is_parse_pending(&self) -> bool {
    self.inner.invalidation_and_queue.lock().parse_pending
  }

  pub fn set_parse_pending(&self, pending: bool) {
    self.inner.invalidation_and_queue.lock().parse_pending = pending;
  }

  pub fn is_accurate(&self) -> bool {
    self.tree_generation() >= self.edit_generation() && !self.needs_reparse()
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
  ) -> Result<Self, SyntaxBackendError> {
    let grammar = registry
      .grammar(language_id)
      .expect("invalid LanguageId must not be passed to Syntax::new")
      .expect("Syntax::new requires an available grammar");
    let tree = parse_tree(source, grammar, None, PARSE_TIMEOUT)?;
    Ok(Self {
      language_id,
      tree,
      injections_enabled: true,
    })
  }

  pub fn update(
    &mut self,
    old_source: RopeSlice,
    source: RopeSlice,
    changeset: &ChangeSet,
    registry: &LanguageRegistry,
  ) -> Result<(), SyntaxBackendError> {
    let edits = generate_edits(old_source, changeset);
    self.update_with_edits(source, &edits, registry)
  }

  pub fn update_with_edits(
    &mut self,
    source: RopeSlice,
    edits: &[InputEdit],
    registry: &LanguageRegistry,
  ) -> Result<(), SyntaxBackendError> {
    if edits.is_empty() {
      return Ok(());
    }

    self.interpolate_with_edits(edits);
    let grammar = registry
      .grammar(self.language_id)
      .expect("invalid LanguageId on active syntax")
      .expect("active syntax requires an available grammar");
    self.tree = parse_tree(source, grammar, Some(&self.tree), PARSE_TIMEOUT)?;
    Ok(())
  }

  pub fn reparse(
    &mut self,
    source: RopeSlice,
    registry: &LanguageRegistry,
  ) -> Result<(), SyntaxBackendError> {
    let grammar = registry
      .grammar(self.language_id)
      .expect("invalid LanguageId on active syntax")
      .expect("active syntax requires an available grammar");
    self.tree = parse_tree(source, grammar, Some(&self.tree), PARSE_TIMEOUT)?;
    Ok(())
  }

  pub fn try_reparse_with_timeout(
    &mut self,
    source: RopeSlice,
    registry: &LanguageRegistry,
    timeout: Duration,
  ) -> Result<bool, SyntaxBackendError> {
    let grammar = registry
      .grammar(self.language_id)
      .expect("invalid LanguageId on active syntax")
      .expect("active syntax requires an available grammar");
    match try_parse_tree(source, grammar, Some(&self.tree), timeout)? {
      Some(tree) => {
        self.tree = tree;
        Ok(true)
      },
      None => Ok(false),
    }
  }

  pub fn try_update_with_short_timeout(
    &mut self,
    source: RopeSlice,
    edits: &[InputEdit],
    registry: &LanguageRegistry,
    timeout: Duration,
  ) -> Result<bool, SyntaxBackendError> {
    if edits.is_empty() {
      return Ok(true);
    }

    self.interpolate_with_edits(edits);
    let grammar = registry
      .grammar(self.language_id)
      .expect("invalid LanguageId on active syntax")
      .expect("active syntax requires an available grammar");
    match try_parse_tree(source, grammar, Some(&self.tree), timeout)? {
      Some(tree) => {
        self.tree = tree;
        Ok(true)
      },
      None => Ok(false),
    }
  }

  pub fn interpolate(&mut self, old_source: RopeSlice, changeset: &ChangeSet) {
    let edits = generate_edits(old_source, changeset);
    self.interpolate_with_edits(&edits);
  }

  pub fn interpolate_with_edits(&mut self, edits: &[InputEdit]) {
    for edit in edits {
      self.tree.edit(edit);
    }
  }

  pub fn root_language(&self) -> LanguageId {
    self.language_id
  }

  pub fn tree(&self) -> &Tree {
    &self.tree
  }

  pub fn tree_for_byte_range(&self, _start: u32, _end: u32) -> &Tree {
    &self.tree
  }

  pub fn named_descendant_for_byte_range(&self, start: u32, end: u32) -> Option<Node<'_>> {
    self
      .tree
      .root_node()
      .named_descendant_for_byte_range(start, end)
  }

  pub fn descendant_for_byte_range(&self, start: u32, end: u32) -> Option<Node<'_>> {
    self.tree.root_node().descendant_for_byte_range(start, end)
  }

  pub fn walk(&self) -> tree_sitter::TreeCursor<'_> {
    self.tree.walk()
  }

  pub fn scopes_at<'a>(&'a self, text: RopeSlice<'a>, pos: usize) -> Vec<&'a str> {
    let mut scopes = Vec::new();
    let byte_pos = text.char_to_byte(pos) as u32;
    let Some(mut node) = self
      .tree
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
    self.injections_enabled
  }

  pub fn set_injections_enabled(&mut self, enabled: bool) {
    self.injections_enabled = enabled;
  }

  pub fn highlight_runs_in_byte_range(
    &self,
    source: RopeSlice,
    registry: &LanguageRegistry,
    byte_range: ops::Range<usize>,
  ) -> Vec<SyntaxHighlightRun> {
    let len_bytes = source.len_bytes();
    if len_bytes == 0 {
      return Vec::new();
    }

    let start = byte_range.start.min(len_bytes);
    let end = byte_range.end.min(len_bytes);
    if start >= end {
      return Vec::new();
    }

    let Some(query) = registry.highlight_query(self.language_id).ok().flatten() else {
      return Vec::new();
    };

    let root = self.tree.root_node();
    let mut cursor = InactiveQueryCursor::new(start as u32..end as u32, TREE_SITTER_MATCH_LIMIT)
      .execute_query(&query.query, &root, RopeInput::new(source));
    let mut matched_ranges = Vec::new();

    while let Some((mat, matched_node_idx)) = cursor.next_matched_node() {
      let matched = mat.matched_node(matched_node_idx);
      let Some(kind) = query
        .capture_kinds
        .get(matched.capture.idx())
        .copied()
        .flatten()
      else {
        continue;
      };

      let node_start = matched.node.start_byte() as usize;
      let node_end = matched.node.end_byte() as usize;
      if node_start >= node_end {
        continue;
      }

      let clipped_start = node_start.max(start);
      let clipped_end = node_end.min(end);
      if clipped_start >= clipped_end {
        continue;
      }

      matched_ranges.push((clipped_start, clipped_end, kind));
    }

    highlight_runs_from_captures(matched_ranges)
  }
}

fn parse_tree(
  source: RopeSlice,
  grammar: tree_sitter::Grammar,
  old_tree: Option<&Tree>,
  timeout: Duration,
) -> Result<Tree, SyntaxBackendError> {
  try_parse_tree(source, grammar, old_tree, timeout)?.ok_or(SyntaxBackendError::Timeout)
}

fn try_parse_tree(
  source: RopeSlice,
  grammar: tree_sitter::Grammar,
  old_tree: Option<&Tree>,
  timeout: Duration,
) -> Result<Option<Tree>, SyntaxBackendError> {
  let mut parser = Parser::new();
  parser.set_grammar(grammar)?;
  parser.set_timeout(timeout);
  Ok(parser.parse(RopeInput::new(source), old_tree))
}

fn invalidated_ranges_for_edits(edits: &[InputEdit]) -> Vec<ops::Range<usize>> {
  edits
    .iter()
    .map(|edit| {
      let start = edit.start_byte as usize;
      let old_end = edit.old_end_byte as usize;
      let new_end = edit.new_end_byte as usize;
      start..old_end.max(new_end)
    })
    .collect()
}

fn highlight_runs_from_captures(
  mut captures: Vec<(usize, usize, HighlightKindId)>,
) -> Vec<SyntaxHighlightRun> {
  if captures.is_empty() {
    return Vec::new();
  }

  captures.sort_by_key(|(run_start, run_end, _)| (*run_start, std::cmp::Reverse(*run_end)));

  let mut boundaries = captures
    .iter()
    .flat_map(|(run_start, run_end, _)| [*run_start, *run_end])
    .collect::<Vec<_>>();
  boundaries.sort_unstable();
  boundaries.dedup();

  let mut spans: Vec<SyntaxHighlightRun> = Vec::new();
  for window in boundaries.windows(2) {
    let segment_start = window[0];
    let segment_end = window[1];
    if segment_start >= segment_end {
      continue;
    }

    let Some((_, _, kind)) = captures
      .iter()
      .filter(|(run_start, run_end, _)| *run_start <= segment_start && segment_end <= *run_end)
      .max_by_key(|(run_start, run_end, _)| (*run_start, std::cmp::Reverse(*run_end)))
    else {
      continue;
    };

    if let Some(last) = spans.last_mut()
      && last.kind == *kind
      && last.byte_range.end == segment_start
    {
      last.byte_range.end = segment_end;
    } else {
      spans.push(SyntaxHighlightRun {
        byte_range: segment_start..segment_end,
        kind:       *kind,
      });
    }
  }

  spans
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

  #[test]
  fn invalidated_byte_ranges_merge_overlaps() {
    let mut ranges = InvalidatedByteRanges::default();
    ranges.insert(4..8);
    ranges.insert(1..3);
    ranges.insert(2..6);

    assert_eq!(ranges.iter().cloned().collect::<Vec<_>>(), vec![1..8]);
  }
}
