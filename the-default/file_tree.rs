use std::{
  any::Any,
  cmp::Ordering,
  collections::{
    HashMap,
    HashSet,
  },
  fs,
  path::{
    Path,
    PathBuf,
  },
  sync::Arc,
  time::SystemTime,
};

use ignore::gitignore::{
  Gitignore,
  GitignoreBuilder,
};
use the_stdx::env::current_working_dir;
use the_lib::{
  diagnostics::DiagnosticSeverity,
  editor::OpenTarget,
};

use crate::DefaultContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileTreeMode {
  #[default]
  WorkingDirectory,
  CurrentBufferDirectory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTreeNodeKind {
  File,
  Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeFilter {
  pub show_hidden:   bool,
  pub show_ignored:  bool,
  pub include_globs: Vec<String>,
  pub exclude_globs: Vec<String>,
}

impl Default for FileTreeFilter {
  fn default() -> Self {
    Self {
      show_hidden:   false,
      show_ignored:  false,
      include_globs: Vec::new(),
      exclude_globs: Vec::new(),
    }
  }
}

impl FileTreeFilter {
  #[must_use]
  pub fn show_hidden(mut self, show_hidden: bool) -> Self {
    self.show_hidden = show_hidden;
    self
  }

  #[must_use]
  pub fn show_ignored(mut self, show_ignored: bool) -> Self {
    self.show_ignored = show_ignored;
    self
  }

  #[must_use]
  pub fn include_glob(mut self, glob: impl Into<String>) -> Self {
    self.include_globs.push(glob.into());
    self
  }

  #[must_use]
  pub fn exclude_glob(mut self, glob: impl Into<String>) -> Self {
    self.exclude_globs.push(glob.into());
    self
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeNodeSnapshot {
  pub id:                    String,
  pub path:                  PathBuf,
  pub name:                  String,
  pub depth:                 usize,
  pub kind:                  FileTreeNodeKind,
  pub hidden:                bool,
  pub ignored:               bool,
  pub expanded:              bool,
  pub selected:              bool,
  pub active:                bool,
  pub has_unloaded_children: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeSnapshot {
  pub visible:            bool,
  pub root:               PathBuf,
  pub mode:               FileTreeMode,
  pub filter:             FileTreeFilter,
  pub selected_path:      Option<PathBuf>,
  pub active_path:        Option<PathBuf>,
  pub refresh_generation: u64,
  pub nodes:              Vec<FileTreeNodeSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeNodeBadge {
  pub label: String,
  pub role:  Option<String>,
}

impl FileTreeNodeBadge {
  #[must_use]
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      label: label.into(),
      role:  None,
    }
  }

  #[must_use]
  pub fn role(mut self, role: impl Into<String>) -> Self {
    self.role = Some(role.into());
    self
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileTreeVcsStatusKind {
  Conflict,
  Deleted,
  Modified,
  Renamed,
  Untracked,
}

impl FileTreeVcsStatusKind {
  #[must_use]
  pub const fn short_label(self) -> &'static str {
    match self {
      Self::Conflict => "!",
      Self::Deleted => "D",
      Self::Modified => "M",
      Self::Renamed => "R",
      Self::Untracked => "?",
    }
  }

  #[must_use]
  pub const fn badge_role(self) -> &'static str {
    match self {
      Self::Conflict => "error",
      Self::Deleted => "warning",
      Self::Modified => "info",
      Self::Renamed => "info",
      Self::Untracked => "hint",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileTreeVcsSummary {
  pub kind:  FileTreeVcsStatusKind,
  pub count: usize,
}

impl FileTreeVcsSummary {
  #[must_use]
  pub fn new(kind: FileTreeVcsStatusKind, count: usize) -> Self {
    Self {
      kind,
      count: count.max(1),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileTreeDiagnosticSummary {
  pub severity: DiagnosticSeverity,
  pub count:    usize,
}

impl FileTreeDiagnosticSummary {
  #[must_use]
  pub fn new(severity: DiagnosticSeverity, count: usize) -> Self {
    Self {
      severity,
      count: count.max(1),
    }
  }
}

#[derive(Clone)]
pub struct FileTreeNodePayload {
  type_name: &'static str,
  value:     Arc<dyn Any + Send + Sync>,
}

impl FileTreeNodePayload {
  pub fn new<T>(value: T) -> Self
  where
    T: Any + Send + Sync,
  {
    Self {
      type_name: std::any::type_name::<T>(),
      value:     Arc::new(value),
    }
  }

  pub fn get<T>(&self) -> Option<&T>
  where
    T: Any + Send + Sync,
  {
    self.value.as_ref().downcast_ref::<T>()
  }
}

impl std::fmt::Debug for FileTreeNodePayload {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("FileTreeNodePayload")
      .field("type_name", &self.type_name)
      .finish()
  }
}

#[derive(Debug, Clone, Default)]
pub struct FileTreeNodeDecoration {
  pub icon:           Option<String>,
  pub badges:         Vec<FileTreeNodeBadge>,
  pub severity:       Option<DiagnosticSeverity>,
  pub secondary_text: Option<String>,
  pub status:         Option<String>,
  pub payload:        Option<FileTreeNodePayload>,
}

impl FileTreeNodeDecoration {
  #[must_use]
  pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
    self.icon = Some(icon.into());
    self
  }

  #[must_use]
  pub fn with_badge(mut self, badge: FileTreeNodeBadge) -> Self {
    self.badges.push(badge);
    self
  }

  #[must_use]
  pub fn with_secondary_text(mut self, text: impl Into<String>) -> Self {
    self.secondary_text = Some(text.into());
    self
  }

  #[must_use]
  pub fn with_status(mut self, status: impl Into<String>) -> Self {
    self.status = Some(status.into());
    self
  }

  #[must_use]
  pub fn with_severity(mut self, severity: DiagnosticSeverity) -> Self {
    self.severity = Some(severity);
    self
  }

  #[must_use]
  pub fn with_payload<T>(mut self, payload: T) -> Self
  where
    T: Any + Send + Sync,
  {
    self.payload = Some(FileTreeNodePayload::new(payload));
    self
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileTreeNodeRequest<'a> {
  pub tree: &'a FileTreeSnapshot,
  pub node: &'a FileTreeNodeSnapshot,
}

#[derive(Debug, Clone)]
pub struct FileTreeNodePresentation {
  pub node:       FileTreeNodeSnapshot,
  pub decoration: FileTreeNodeDecoration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTreeDisclosure {
  None,
  Collapsed,
  Expanded,
  Loading,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTreeRowAccent {
  Default,
  Selected,
  Active,
  Hint,
  Info,
  Warning,
  Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTreeGuideColumn {
  Empty,
  Continue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileTreeGuideConnector {
  #[default]
  None,
  Branch,
  LastChild,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileTreeRowGuides {
  pub ancestor_columns: Vec<FileTreeGuideColumn>,
  pub connector:        FileTreeGuideConnector,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeRowLayout {
  pub id:               String,
  pub path:             PathBuf,
  pub depth:            usize,
  pub indent_text:      String,
  pub guides:           FileTreeRowGuides,
  pub disclosure:       FileTreeDisclosure,
  pub disclosure_glyph: &'static str,
  pub icon:             String,
  pub primary_text:     String,
  pub secondary_text:   Option<String>,
  pub badges:           Vec<FileTreeNodeBadge>,
  pub status:           Option<String>,
  pub accent:           FileTreeRowAccent,
  pub selected:         bool,
  pub active:           bool,
  pub kind:             FileTreeNodeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileTreeOp {
  Open {
    path:   PathBuf,
    target: OpenTarget,
  },
  CreateFile {
    path: PathBuf,
  },
  CreateDirectory {
    path: PathBuf,
  },
  Rename {
    from: PathBuf,
    to:   PathBuf,
  },
  Delete {
    path:      PathBuf,
    recursive: bool,
  },
  Move {
    from: PathBuf,
    to:   PathBuf,
  },
  Refresh {
    path: Option<PathBuf>,
  },
}

impl FileTreeOp {
  #[must_use]
  pub fn open(path: impl Into<PathBuf>, target: OpenTarget) -> Self {
    Self::Open {
      path:   path.into(),
      target,
    }
  }

  #[must_use]
  pub fn create_file(path: impl Into<PathBuf>) -> Self {
    Self::CreateFile { path: path.into() }
  }

  #[must_use]
  pub fn create_directory(path: impl Into<PathBuf>) -> Self {
    Self::CreateDirectory { path: path.into() }
  }

  #[must_use]
  pub fn rename(from: impl Into<PathBuf>, to: impl Into<PathBuf>) -> Self {
    Self::Rename {
      from: from.into(),
      to:   to.into(),
    }
  }

  #[must_use]
  pub fn delete(path: impl Into<PathBuf>) -> Self {
    Self::Delete {
      path:      path.into(),
      recursive: true,
    }
  }

  #[must_use]
  pub fn move_to(from: impl Into<PathBuf>, to: impl Into<PathBuf>) -> Self {
    Self::Move {
      from: from.into(),
      to:   to.into(),
    }
  }

  #[must_use]
  pub fn refresh(path: Option<impl Into<PathBuf>>) -> Self {
    Self::Refresh {
      path: path.map(Into::into),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileTreeOpOutcome {
  Opened {
    path: PathBuf,
  },
  Changed {
    path: PathBuf,
  },
  Refreshed {
    path: Option<PathBuf>,
  },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeEditEntry {
  pub path:  PathBuf,
  pub depth: usize,
  pub kind:  FileTreeNodeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeEditSession {
  pub root:    PathBuf,
  entries:     Vec<FileTreeEditEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileTreeEditPatch {
  pub operations: Vec<FileTreeOp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeEditError {
  pub line:    Option<usize>,
  pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileTreeEntry {
  path:   PathBuf,
  name:   String,
  is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirectoryCacheFingerprint {
  modified: SystemTime,
  len:      u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirectoryCacheEntry {
  fingerprint: Option<DirectoryCacheFingerprint>,
  entries:     Option<Vec<FileTreeEntry>>,
}

#[derive(Debug)]
struct FileTreeFilterMatcher {
  include: Option<Gitignore>,
  exclude: Option<Gitignore>,
  filter:  FileTreeFilter,
}

impl FileTreeFilterMatcher {
  fn new(root: &Path, filter: &FileTreeFilter) -> Self {
    Self {
      include: compile_filter_globs(root, &filter.include_globs),
      exclude: compile_filter_globs(root, &filter.exclude_globs),
      filter:  filter.clone(),
    }
  }

  fn is_visible_path(&self, path: &Path, name: &str, is_dir: bool) -> bool {
    if !self.filter.show_hidden && is_hidden_entry_name(name) {
      return false;
    }
    if !self.filter.show_ignored && is_ignored_entry_name(name, is_dir) {
      return false;
    }
    if self
      .exclude
      .as_ref()
      .is_some_and(|exclude| exclude.matched_path_or_any_parents(path, is_dir).is_ignore())
    {
      return false;
    }
    if self.filter.include_globs.is_empty() {
      return true;
    }
    if is_dir {
      return true;
    }
    self
      .include
      .as_ref()
      .is_some_and(|include| include.matched_path_or_any_parents(path, false).is_ignore())
  }
}

#[derive(Debug, Clone, Default)]
pub struct FileTreeState {
  pub visible:            bool,
  pub mode:               FileTreeMode,
  pub filter:             FileTreeFilter,
  pub selected_path:      Option<PathBuf>,
  pub active_path:        Option<PathBuf>,
  pub refresh_generation: u64,
  root:                   Option<PathBuf>,
  expanded_dirs:          HashSet<PathBuf>,
  nodes_cache:            HashMap<PathBuf, DirectoryCacheEntry>,
}

impl FileTreeState {
  #[must_use]
  pub fn with_working_directory(root: PathBuf) -> Self {
    let mut state = Self::default();
    state.set_root_internal(FileTreeMode::WorkingDirectory, root);
    state
  }

  #[must_use]
  pub fn with_workspace_root(root: PathBuf) -> Self {
    Self::with_working_directory(root)
  }

  #[must_use]
  pub fn root(&self) -> Option<&Path> {
    self.root.as_deref()
  }

  #[must_use]
  pub fn selected_path(&self) -> Option<&Path> {
    self.selected_path.as_deref()
  }

  #[must_use]
  pub fn active_path(&self) -> Option<&Path> {
    self.active_path.as_deref()
  }

  #[must_use]
  pub fn filter(&self) -> &FileTreeFilter {
    &self.filter
  }

  pub fn set_filter(&mut self, filter: FileTreeFilter) -> bool {
    if self.filter == filter {
      return false;
    }
    self.filter = filter;
    self.bump_generation();
    true
  }

  pub fn set_show_hidden(&mut self, show_hidden: bool) -> bool {
    let mut next = self.filter.clone();
    next.show_hidden = show_hidden;
    self.set_filter(next)
  }

  pub fn toggle_show_hidden(&mut self) -> bool {
    let next = !self.filter.show_hidden;
    self.set_show_hidden(next)
  }

  pub fn set_show_ignored(&mut self, show_ignored: bool) -> bool {
    let mut next = self.filter.clone();
    next.show_ignored = show_ignored;
    self.set_filter(next)
  }

  pub fn toggle_show_ignored(&mut self) -> bool {
    let next = !self.filter.show_ignored;
    self.set_show_ignored(next)
  }

  pub fn set_include_globs<I, S>(&mut self, globs: I) -> bool
  where
    I: IntoIterator<Item = S>,
    S: Into<String>,
  {
    let mut next = self.filter.clone();
    next.include_globs = globs.into_iter().map(Into::into).collect();
    self.set_filter(next)
  }

  pub fn set_exclude_globs<I, S>(&mut self, globs: I) -> bool
  where
    I: IntoIterator<Item = S>,
    S: Into<String>,
  {
    let mut next = self.filter.clone();
    next.exclude_globs = globs.into_iter().map(Into::into).collect();
    self.set_filter(next)
  }

  pub fn set_visible(&mut self, visible: bool) {
    if self.visible != visible {
      self.visible = visible;
      self.bump_generation();
    }
  }

  pub fn toggle_visible(&mut self) {
    self.set_visible(!self.visible);
  }

  pub fn toggle_working_directory(&mut self, working_directory: &Path) {
    if self.visible && self.mode == FileTreeMode::WorkingDirectory {
      self.set_visible(false);
      return;
    }
    self.set_root_internal(
      FileTreeMode::WorkingDirectory,
      working_directory.to_path_buf(),
    );
    self.set_visible(true);
  }

  pub fn open_working_directory(&mut self, working_directory: &Path) {
    self.set_root_internal(
      FileTreeMode::WorkingDirectory,
      working_directory.to_path_buf(),
    );
    self.set_visible(true);
  }

  pub fn open_current_buffer_directory(
    &mut self,
    current_file: Option<&Path>,
    working_directory: &Path,
  ) {
    let root = current_file
      .and_then(Path::parent)
      .unwrap_or(working_directory)
      .to_path_buf();
    self.set_root_internal(FileTreeMode::CurrentBufferDirectory, root);
    if let Some(path) = current_file {
      let _ = self.set_active_path(Some(path));
      let _ = self.select_path(path);
    }
    self.set_visible(true);
  }

  pub fn sync_for_working_directory(&mut self, working_directory: &Path) {
    if self.mode == FileTreeMode::WorkingDirectory {
      self.set_root_internal(
        FileTreeMode::WorkingDirectory,
        working_directory.to_path_buf(),
      );
    }
  }

  pub fn sync_for_active_file(&mut self, working_directory: &Path, active_file: Option<&Path>) {
    match self.mode {
      FileTreeMode::WorkingDirectory => {
        if self.root.is_none() {
          self.set_root_internal(
            FileTreeMode::WorkingDirectory,
            working_directory.to_path_buf(),
          );
        }
      },
      FileTreeMode::CurrentBufferDirectory => {
        let root = active_file
          .and_then(Path::parent)
          .unwrap_or(working_directory)
          .to_path_buf();
        self.set_root_internal(FileTreeMode::CurrentBufferDirectory, root);
      },
    }

    if let Some(path) = active_file {
      let _ = self.set_active_path(Some(path));
      let _ = self.select_path(path);
    } else {
      let _ = self.set_active_path(None::<&Path>);
    }
  }

  pub fn set_active_path<P>(&mut self, path: Option<P>) -> bool
  where
    P: AsRef<Path>,
  {
    let next = path.map(|value| normalize_path(value.as_ref()));
    if self.active_path == next {
      return false;
    }
    self.active_path = next;
    self.bump_generation();
    true
  }

  pub fn invalidate_visible_subtree(&mut self) -> bool {
    let Some(root) = self.root.clone() else {
      return false;
    };

    let mut invalidated = false;
    let visible_dirs = self
      .expanded_dirs
      .iter()
      .filter(|path| path.starts_with(&root))
      .cloned()
      .collect::<Vec<_>>();

    for path in visible_dirs {
      invalidated |= self.nodes_cache.remove(&path).is_some();
    }
    invalidated |= self.nodes_cache.remove(&root).is_some();

    if invalidated {
      self.bump_generation();
    }
    invalidated
  }

  pub fn set_expanded(&mut self, path: &Path, expanded: bool) -> bool {
    let normalized = normalize_path(path);
    let Some(root) = self.root.as_ref() else {
      return false;
    };
    if !normalized.starts_with(root) {
      return false;
    }

    if expanded {
      let changed = self.expanded_dirs.insert(normalized.clone());
      let _ = self.load_children(&normalized);
      if changed {
        self.bump_generation();
      }
      return true;
    }

    let mut changed = false;
    if self.expanded_dirs.remove(&normalized) {
      changed = true;
    }
    let descendants = self
      .expanded_dirs
      .iter()
      .filter(|candidate| candidate.starts_with(&normalized))
      .cloned()
      .collect::<Vec<_>>();
    for descendant in descendants {
      if self.expanded_dirs.remove(&descendant) {
        changed = true;
      }
    }
    if changed {
      self.bump_generation();
    }
    true
  }

  #[must_use]
  pub fn is_expanded(&self, path: &Path) -> bool {
    self.expanded_dirs.contains(&normalize_path(path))
  }

  pub fn expand_to(&mut self, path: &Path) -> bool {
    let normalized = normalize_path(path);
    let Some(root) = self.root.clone() else {
      return false;
    };
    if !normalized.starts_with(&root) {
      return false;
    }
    self.expand_visible_ancestors(&normalized, &root)
  }

  #[must_use]
  pub fn is_visible(&self, path: &Path) -> bool {
    let normalized = normalize_path(path);
    let Some(root) = self.root.as_ref() else {
      return false;
    };
    if !normalized.starts_with(root) {
      return false;
    }

    let Ok(metadata) = fs::metadata(&normalized) else {
      return false;
    };
    let is_dir = metadata.is_dir();
    let Some(name) = normalized.file_name().and_then(|name| name.to_str()) else {
      return normalized == *root;
    };
    let matcher = FileTreeFilterMatcher::new(root, &self.filter);
    if normalized != *root && !matcher.is_visible_path(&normalized, name, is_dir) {
      return false;
    }

    let mut current = normalized.parent();
    while let Some(ancestor) = current {
      if ancestor == root {
        break;
      }
      if !self.expanded_dirs.contains(ancestor) {
        return false;
      }
      let Ok(ancestor_metadata) = fs::metadata(ancestor) else {
        return false;
      };
      let Some(ancestor_name) = ancestor.file_name().and_then(|name| name.to_str()) else {
        return false;
      };
      if !matcher.is_visible_path(ancestor, ancestor_name, ancestor_metadata.is_dir()) {
        return false;
      }
      current = ancestor.parent();
    }
    true
  }

  pub fn ensure_visible(&mut self, path: &Path) -> bool {
    self.expand_to(path) && self.is_visible(path)
  }

  pub fn next_visible<F>(&mut self, path: &Path, mut predicate: F) -> Option<PathBuf>
  where
    F: FnMut(&FileTreeNodeSnapshot) -> bool,
  {
    let normalized = normalize_path(path);
    let snapshot = self.snapshot(usize::MAX);
    let index = snapshot
      .nodes
      .iter()
      .position(|node| node.path == normalized)?;
    snapshot
      .nodes
      .iter()
      .skip(index.saturating_add(1))
      .find(|node| predicate(node))
      .map(|node| node.path.clone())
  }

  pub fn prev_visible<F>(&mut self, path: &Path, mut predicate: F) -> Option<PathBuf>
  where
    F: FnMut(&FileTreeNodeSnapshot) -> bool,
  {
    let normalized = normalize_path(path);
    let snapshot = self.snapshot(usize::MAX);
    let index = snapshot
      .nodes
      .iter()
      .position(|node| node.path == normalized)?;
    snapshot
      .nodes
      .iter()
      .take(index)
      .rev()
      .find(|node| predicate(node))
      .map(|node| node.path.clone())
  }

  pub fn close_all(&mut self, root: Option<&Path>) -> bool {
    let Some(current_root) = self.root.clone() else {
      return false;
    };
    let base = root.map(normalize_path).unwrap_or_else(|| current_root.clone());
    if !base.starts_with(&current_root) {
      return false;
    }

    let descendants = self
      .expanded_dirs
      .iter()
      .filter(|candidate| **candidate != base && candidate.starts_with(&base))
      .cloned()
      .collect::<Vec<_>>();
    if descendants.is_empty() {
      return false;
    }
    for descendant in descendants {
      self.expanded_dirs.remove(&descendant);
    }
    self.bump_generation();
    true
  }

  pub fn refresh_path(&mut self, path: Option<&Path>) -> bool {
    let target = path
      .map(normalize_path)
      .or_else(|| self.root.clone());
    let Some(target) = target else {
      return false;
    };

    let mut invalidated = self.nodes_cache.remove(&target).is_some();
    if let Some(parent) = target.parent() {
      invalidated |= self.nodes_cache.remove(parent).is_some();
    }
    if invalidated {
      self.bump_generation();
    }
    invalidated
  }

  pub fn select_path(&mut self, path: &Path) -> bool {
    let normalized = normalize_path(path);
    let Some(root) = self.root.clone() else {
      return false;
    };
    if !normalized.starts_with(&root) {
      return false;
    }

    let mut changed = self.selected_path.as_ref() != Some(&normalized);
    self.selected_path = Some(normalized.clone());
    changed |= self.expand_visible_ancestors(&normalized, &root);

    if changed {
      self.bump_generation();
    }
    true
  }

  #[must_use]
  pub fn open_selected(&mut self) -> Option<PathBuf> {
    let selected = self.selected_path.as_ref()?.clone();
    if selected.is_dir() {
      let _ = self.set_expanded(&selected, true);
      return None;
    }
    if selected.is_file() {
      Some(selected)
    } else {
      None
    }
  }

  #[must_use]
  pub fn snapshot(&mut self, max_nodes: usize) -> FileTreeSnapshot {
    let root = self
      .root
      .as_ref()
      .cloned()
      .unwrap_or_else(|| normalize_path(Path::new(".")));
    if !self.expanded_dirs.contains(&root) {
      self.expanded_dirs.insert(root.clone());
    }

    let mut nodes = Vec::new();
    let limit = max_nodes.max(1);
    let matcher = FileTreeFilterMatcher::new(&root, &self.filter);
    self.append_node(&mut nodes, &root, 0, limit, &matcher);

    FileTreeSnapshot {
      visible: self.visible,
      root,
      mode: self.mode,
      filter: self.filter.clone(),
      selected_path: self.selected_path.clone(),
      active_path: self.active_path.clone(),
      refresh_generation: self.refresh_generation,
      nodes,
    }
  }

  pub fn selected_container_path(&self) -> Option<PathBuf> {
    let Some(selected) = self.selected_path.as_ref() else {
      return self.root.clone();
    };
    if selected.is_dir() {
      return Some(selected.clone());
    }
    selected.parent().map(Path::to_path_buf)
  }

  pub fn open_selected_op(&self, target: OpenTarget) -> Option<FileTreeOp> {
    self
      .selected_path
      .as_ref()
      .cloned()
      .map(|path| FileTreeOp::open(path, target))
  }

  pub fn create_child_op(&self, name: &str, directory: bool) -> Option<FileTreeOp> {
    let container = self.selected_container_path()?;
    let name = name.trim();
    if name.is_empty() {
      return None;
    }
    let path = container.join(name);
    Some(if directory {
      FileTreeOp::create_directory(path)
    } else {
      FileTreeOp::create_file(path)
    })
  }

  pub fn rename_selected_op(&self, new_name: &str) -> Option<FileTreeOp> {
    let selected = self.selected_path.as_ref()?;
    let parent = selected.parent()?;
    let new_name = new_name.trim();
    if new_name.is_empty() {
      return None;
    }
    let target = parent.join(new_name);
    if target == *selected {
      return None;
    }
    Some(path_change_op(selected, &target))
  }

  pub fn delete_selected_op(&self) -> Option<FileTreeOp> {
    self.selected_path.as_ref().cloned().map(FileTreeOp::delete)
  }

  pub fn refresh_op(&self) -> FileTreeOp {
    FileTreeOp::refresh(self.selected_path.clone())
  }

  fn set_root_internal(&mut self, mode: FileTreeMode, root: PathBuf) {
    let normalized_root = normalize_path(&root);
    let changed = self.root.as_ref() != Some(&normalized_root) || self.mode != mode;
    if !changed {
      return;
    }

    self.mode = mode;
    self.root = Some(normalized_root.clone());
    self.expanded_dirs.clear();
    self.expanded_dirs.insert(normalized_root.clone());
    self.nodes_cache.clear();

    if let Some(selected) = self.selected_path.as_ref()
      && !selected.starts_with(&normalized_root)
    {
      self.selected_path = None;
    }
    if let Some(active) = self.active_path.as_ref()
      && !active.starts_with(&normalized_root)
    {
      self.active_path = None;
    }

    let _ = self.load_children(&normalized_root);
    self.bump_generation();
  }

  fn append_node(
    &mut self,
    out: &mut Vec<FileTreeNodeSnapshot>,
    path: &Path,
    depth: usize,
    limit: usize,
    matcher: &FileTreeFilterMatcher,
  ) {
    if out.len() >= limit {
      return;
    }

    let path_buf = path.to_path_buf();
    let is_dir = path_buf.is_dir();
    let name = node_name_for_path(&path_buf);
    let hidden = is_hidden_entry_name(&name);
    let ignored = is_ignored_entry_name(&name, is_dir);
    let is_root = self.root.as_ref() == Some(&path_buf);
    if !is_root && !matcher.is_visible_path(&path_buf, &name, is_dir) {
      return;
    }
    let expanded = is_dir && self.expanded_dirs.contains(&path_buf);
    let has_unloaded_children = is_dir && !expanded && !self.nodes_cache.contains_key(&path_buf);
    let selected = self.selected_path.as_ref() == Some(&path_buf);
    let active = self.active_path.as_ref() == Some(&path_buf);
    out.push(FileTreeNodeSnapshot {
      id: stable_id_for_path(&path_buf),
      path: path_buf.clone(),
      name,
      depth,
      kind: if is_dir {
        FileTreeNodeKind::Directory
      } else {
        FileTreeNodeKind::File
      },
      hidden,
      ignored,
      expanded,
      selected,
      active,
      has_unloaded_children,
    });

    if !expanded || out.len() >= limit {
      return;
    }

    let Some(children) = self.load_children(&path_buf) else {
      return;
    };
    for entry in children {
      if out.len() >= limit {
        break;
      }
      self.append_node(out, entry.path.as_path(), depth + 1, limit, matcher);
    }
  }

  fn load_children(&mut self, dir: &Path) -> Option<Vec<FileTreeEntry>> {
    let key = normalize_path(dir);
    let next_fingerprint = directory_cache_fingerprint(&key);
    let cached = self.nodes_cache.get(&key).cloned();
    let should_reload = match cached.as_ref() {
      Some(entry) => {
        match (entry.fingerprint.as_ref(), next_fingerprint.as_ref()) {
          (Some(previous), Some(next)) => previous != next,
          _ => true,
        }
      },
      None => true,
    };

    if should_reload {
      let next_entries = read_directory_entries(&key).ok();
      let replacement = DirectoryCacheEntry {
        fingerprint: next_fingerprint,
        entries:     next_entries,
      };
      let entries_changed = cached
        .as_ref()
        .is_some_and(|previous| previous.entries != replacement.entries);
      self.nodes_cache.insert(key.clone(), replacement);
      if entries_changed {
        self.bump_generation();
      }
    }

    self
      .nodes_cache
      .get(&key)
      .and_then(|entry| entry.entries.clone())
  }

  fn bump_generation(&mut self) {
    self.refresh_generation = self.refresh_generation.saturating_add(1);
  }

  fn expand_visible_ancestors(&mut self, normalized: &Path, root: &Path) -> bool {
    let Some(parent) = normalized.parent() else {
      return false;
    };

    let ancestors = parent
      .ancestors()
      .take_while(|ancestor| ancestor.starts_with(root))
      .map(Path::to_path_buf)
      .collect::<Vec<_>>();

    let mut changed = false;
    for ancestor in ancestors {
      if self.expanded_dirs.insert(ancestor.clone()) {
        changed = true;
      }
      if ancestor.is_dir() {
        let _ = self.load_children(&ancestor);
      }
    }
    changed
  }

  fn remap_path_prefix(&mut self, from: &Path, to: &Path) {
    let from = normalize_path(from);
    let to = normalize_path(to);

    if let Some(root) = self.root.as_ref().cloned()
      && let Some(remapped) = remap_path_if_matches(root, &from, &to)
    {
      self.root = Some(remapped);
    }

    if let Some(selected) = self.selected_path.as_ref().cloned() {
      self.selected_path = remap_path_if_matches(selected, &from, &to);
    }
    if let Some(active) = self.active_path.as_ref().cloned() {
      self.active_path = remap_path_if_matches(active, &from, &to);
    }

    self.expanded_dirs = self
      .expanded_dirs
      .iter()
      .cloned()
      .filter_map(|path| remap_path_if_matches(path, &from, &to))
      .collect();
    self.nodes_cache.clear();
    self.bump_generation();
  }

  pub fn clear_removed_path(&mut self, path: &Path) {
    let path = normalize_path(path);

    if self.selected_path.as_ref().is_some_and(|selected| selected.starts_with(&path)) {
      self.selected_path = path.parent().map(Path::to_path_buf);
    }
    if self.active_path.as_ref().is_some_and(|active| active.starts_with(&path)) {
      self.active_path = None;
    }
    if self.root.as_ref().is_some_and(|root| root.starts_with(&path)) {
      self.root = path.parent().map(Path::to_path_buf);
    }

    self.expanded_dirs.retain(|candidate| !candidate.starts_with(&path));
    self.nodes_cache.clear();
    self.bump_generation();
  }
}

fn stable_id_for_path(path: &Path) -> String {
  path.to_string_lossy().into_owned()
}

fn node_name_for_path(path: &Path) -> String {
  if let Some(name) = path.file_name().and_then(|name| name.to_str())
    && !name.is_empty()
  {
    return name.to_string();
  }
  path.to_string_lossy().into_owned()
}

fn remap_path_if_matches(path: PathBuf, from: &Path, to: &Path) -> Option<PathBuf> {
  if !path.starts_with(from) {
    return Some(path);
  }
  let suffix = path.strip_prefix(from).ok()?;
  if suffix.as_os_str().is_empty() {
    return Some(to.to_path_buf());
  }
  Some(to.join(suffix))
}

fn path_change_op(from: &Path, to: &Path) -> FileTreeOp {
  let from_parent = from.parent();
  let to_parent = to.parent();
  if from_parent == to_parent {
    FileTreeOp::rename(from.to_path_buf(), to.to_path_buf())
  } else {
    FileTreeOp::move_to(from.to_path_buf(), to.to_path_buf())
  }
}

#[must_use]
pub fn file_tree_disclosure(node: &FileTreeNodeSnapshot) -> FileTreeDisclosure {
  match node.kind {
    FileTreeNodeKind::File => FileTreeDisclosure::None,
    FileTreeNodeKind::Directory if node.expanded => FileTreeDisclosure::Expanded,
    FileTreeNodeKind::Directory if node.has_unloaded_children => FileTreeDisclosure::Loading,
    FileTreeNodeKind::Directory => FileTreeDisclosure::Collapsed,
  }
}

#[must_use]
pub const fn file_tree_disclosure_glyph(disclosure: FileTreeDisclosure) -> &'static str {
  match disclosure {
    FileTreeDisclosure::None => " ",
    FileTreeDisclosure::Collapsed => "▸",
    FileTreeDisclosure::Expanded => "▾",
    FileTreeDisclosure::Loading => "◌",
  }
}

#[must_use]
pub const fn file_tree_default_icon_name(kind: FileTreeNodeKind, expanded: bool) -> &'static str {
  match (kind, expanded) {
    (FileTreeNodeKind::Directory, true) => "folder_open",
    (FileTreeNodeKind::Directory, false) => "folder",
    (FileTreeNodeKind::File, _) => "file",
  }
}

#[must_use]
pub fn file_tree_indentation(depth: usize) -> String {
  "  ".repeat(depth)
}

#[must_use]
pub fn build_file_tree_row_guides(snapshot: &FileTreeSnapshot) -> Vec<FileTreeRowGuides> {
  let mut path_stack: Vec<usize> = Vec::new();
  let mut ancestor_indices: Vec<Vec<usize>> = Vec::with_capacity(snapshot.nodes.len());

  for (index, node) in snapshot.nodes.iter().enumerate() {
    while path_stack.len() > node.depth {
      path_stack.pop();
    }
    ancestor_indices.push(path_stack.clone());
    path_stack.push(index);
  }

  let is_last_sibling = snapshot
    .nodes
    .iter()
    .enumerate()
    .map(|(index, node)| {
      let mut is_last = true;
      for next in snapshot.nodes.iter().skip(index.saturating_add(1)) {
        if next.depth < node.depth {
          break;
        }
        if next.depth == node.depth {
          is_last = false;
          break;
        }
      }
      is_last
    })
    .collect::<Vec<_>>();

  snapshot
    .nodes
    .iter()
    .enumerate()
    .map(|(index, node)| {
      let ancestor_columns = ancestor_indices[index]
        .iter()
        .skip(1)
        .map(|ancestor_index| {
          if is_last_sibling[*ancestor_index] {
            FileTreeGuideColumn::Empty
          } else {
            FileTreeGuideColumn::Continue
          }
        })
        .collect::<Vec<_>>();
      let connector = if node.depth == 0 {
        FileTreeGuideConnector::None
      } else if is_last_sibling[index] {
        FileTreeGuideConnector::LastChild
      } else {
        FileTreeGuideConnector::Branch
      };
      FileTreeRowGuides {
        ancestor_columns,
        connector,
      }
    })
    .collect()
}

#[must_use]
pub fn file_tree_row_layout(node: &FileTreeNodePresentation) -> FileTreeRowLayout {
  file_tree_row_layout_with_guides(node, FileTreeRowGuides::default())
}

#[must_use]
pub fn file_tree_row_layout_with_guides(
  node: &FileTreeNodePresentation,
  guides: FileTreeRowGuides,
) -> FileTreeRowLayout {
  let disclosure = file_tree_disclosure(&node.node);
  let icon = node
    .decoration
    .icon
    .clone()
    .unwrap_or_else(|| file_tree_default_icon_name(node.node.kind, node.node.expanded).to_string());
  let accent = match node.decoration.severity {
    Some(DiagnosticSeverity::Error) => FileTreeRowAccent::Error,
    Some(DiagnosticSeverity::Warning) => FileTreeRowAccent::Warning,
    Some(DiagnosticSeverity::Information) => FileTreeRowAccent::Info,
    Some(DiagnosticSeverity::Hint) => FileTreeRowAccent::Hint,
    None if node.node.selected => FileTreeRowAccent::Selected,
    None if node.node.active => FileTreeRowAccent::Active,
    None => FileTreeRowAccent::Default,
  };

  FileTreeRowLayout {
    id:               node.node.id.clone(),
    path:             node.node.path.clone(),
    depth:            node.node.depth,
    indent_text:      file_tree_indentation(node.node.depth.saturating_sub(1)),
    guides,
    disclosure,
    disclosure_glyph: file_tree_disclosure_glyph(disclosure),
    icon,
    primary_text:     node.node.name.clone(),
    secondary_text:   node.decoration.secondary_text.clone(),
    badges:           node.decoration.badges.clone(),
    status:           node.decoration.status.clone(),
    accent,
    selected:         node.node.selected,
    active:           node.node.active,
    kind:             node.node.kind,
  }
}

#[must_use]
pub fn build_file_tree_presentations(snapshot: &FileTreeSnapshot) -> Vec<FileTreeNodePresentation> {
  snapshot
    .nodes
    .iter()
    .cloned()
    .map(|node| {
      FileTreeNodePresentation {
        node,
        decoration: FileTreeNodeDecoration::default(),
      }
    })
    .collect()
}

pub fn build_file_tree_presentations_with_providers<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  snapshot: &FileTreeSnapshot,
) -> Vec<FileTreeNodePresentation> {
  let mut presentations = Vec::with_capacity(snapshot.nodes.len());
  for node in &snapshot.nodes {
    let request = FileTreeNodeRequest {
      tree: snapshot,
      node,
    };
    let mut decoration = FileTreeNodeDecoration::default();
    ctx.decorate_file_tree_node(&request, &mut decoration);
    presentations.push(FileTreeNodePresentation {
      node: node.clone(),
      decoration,
    });
  }
  presentations
}

#[must_use]
pub fn build_file_tree_row_layouts(snapshot: &FileTreeSnapshot) -> Vec<FileTreeRowLayout> {
  let presentations = build_file_tree_presentations(snapshot);
  let guides = build_file_tree_row_guides(snapshot);
  presentations
    .iter()
    .zip(guides)
    .map(|(node, guide)| file_tree_row_layout_with_guides(node, guide))
    .collect()
}

pub fn build_file_tree_row_layouts_with_providers<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  snapshot: &FileTreeSnapshot,
) -> Vec<FileTreeRowLayout> {
  let presentations = build_file_tree_presentations_with_providers(ctx, snapshot);
  let guides = build_file_tree_row_guides(snapshot);
  presentations
    .iter()
    .zip(guides)
    .map(|(node, guide)| file_tree_row_layout_with_guides(node, guide))
    .collect()
}

pub fn execute_file_tree_op<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  op: &FileTreeOp,
) -> Result<FileTreeOpOutcome, String> {
  match op {
    FileTreeOp::Open { path, target } => {
      if path.is_dir() {
        let _ = ctx.file_tree_mut().set_expanded(path, true);
        let _ = ctx.file_tree_mut().select_path(path);
        ctx.request_render();
        return Ok(FileTreeOpOutcome::Changed { path: path.clone() });
      }
      ctx
        .open_file_in_target(path, *target)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
      let _ = ctx.file_tree_mut().set_active_path(Some(path.as_path()));
      let _ = ctx.file_tree_mut().select_path(path);
      ctx.request_render();
      Ok(FileTreeOpOutcome::Opened { path: path.clone() })
    },
    FileTreeOp::CreateFile { path } => {
      let Some(parent) = path.parent() else {
        return Err(format!("cannot create file without parent: {}", path.display()));
      };
      fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create parent {}: {error}", parent.display()))?;
      fs::write(path, "")
        .map_err(|error| format!("failed to create file {}: {error}", path.display()))?;
      {
        let tree = ctx.file_tree_mut();
        let _ = tree.refresh_path(Some(parent));
        let _ = tree.select_path(path);
      }
      ctx.request_render();
      Ok(FileTreeOpOutcome::Changed { path: path.clone() })
    },
    FileTreeOp::CreateDirectory { path } => {
      fs::create_dir_all(path)
        .map_err(|error| format!("failed to create directory {}: {error}", path.display()))?;
      {
        let tree = ctx.file_tree_mut();
        let _ = tree.refresh_path(path.parent());
        let _ = tree.select_path(path);
      }
      ctx.request_render();
      Ok(FileTreeOpOutcome::Changed { path: path.clone() })
    },
    FileTreeOp::Rename { from, to } | FileTreeOp::Move { from, to } => {
      if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)
          .map_err(|error| format!("failed to create parent {}: {error}", parent.display()))?;
      }
      fs::rename(from, to)
        .map_err(|error| format!("failed to move {} to {}: {error}", from.display(), to.display()))?;
      if ctx.file_path() == Some(from.as_path()) {
        ctx.set_file_path(Some(to.clone()));
      }
      let _ = ctx.editor().rename_file_path(from, to.clone());
      ctx.file_tree_mut().remap_path_prefix(from, to);
      ctx.request_render();
      Ok(FileTreeOpOutcome::Changed { path: to.clone() })
    },
    FileTreeOp::Delete { path, recursive } => {
      let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
      if metadata.is_dir() {
        if *recursive {
          fs::remove_dir_all(path)
            .map_err(|error| format!("failed to remove directory {}: {error}", path.display()))?;
        } else {
          fs::remove_dir(path)
            .map_err(|error| format!("failed to remove directory {}: {error}", path.display()))?;
        }
      } else {
        fs::remove_file(path)
          .map_err(|error| format!("failed to remove file {}: {error}", path.display()))?;
      }
      if ctx.file_path() == Some(path.as_path()) {
        ctx.set_file_path(None);
      }
      ctx.file_tree_mut().clear_removed_path(path);
      ctx.request_render();
      Ok(FileTreeOpOutcome::Changed { path: path.clone() })
    },
    FileTreeOp::Refresh { path } => {
      let _ = ctx.file_tree_mut().refresh_path(path.as_deref());
      ctx.request_render();
      Ok(FileTreeOpOutcome::Refreshed { path: path.clone() })
    },
  }
}

pub fn execute_file_tree_edit_patch<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  patch: &FileTreeEditPatch,
) -> Result<Vec<FileTreeOpOutcome>, String> {
  let mut outcomes = Vec::with_capacity(patch.operations.len());
  for op in &patch.operations {
    outcomes.push(execute_file_tree_op(ctx, op)?);
  }
  Ok(outcomes)
}

impl FileTreeEditSession {
  #[must_use]
  pub fn from_snapshot(snapshot: &FileTreeSnapshot) -> Self {
    let entries = snapshot
      .nodes
      .iter()
      .filter(|node| node.path != snapshot.root)
      .map(|node| {
        FileTreeEditEntry {
          path:  node.path.clone(),
          depth: node.depth.saturating_sub(1),
          kind:  node.kind,
        }
      })
      .collect();
    Self {
      root: snapshot.root.clone(),
      entries,
    }
  }

  #[must_use]
  pub fn entries(&self) -> &[FileTreeEditEntry] {
    &self.entries
  }

  #[must_use]
  pub fn to_text(&self) -> String {
    self
      .entries
      .iter()
      .map(|entry| {
        let name = node_name_for_path(&entry.path);
        let suffix = matches!(entry.kind, FileTreeNodeKind::Directory)
          .then_some("/")
          .unwrap_or("");
        format!("{}{}{}", file_tree_indentation(entry.depth), name, suffix)
      })
      .collect::<Vec<_>>()
      .join("\n")
  }

  pub fn parse(&self, text: &str) -> Result<FileTreeEditPatch, FileTreeEditError> {
    let parsed = parse_edit_lines(&self.root, text)?;
    Ok(FileTreeEditPatch {
      operations: diff_edit_entries(&self.entries, &parsed),
    })
  }
}

fn parse_edit_lines(root: &Path, text: &str) -> Result<Vec<FileTreeEditEntry>, FileTreeEditError> {
  let mut entries = Vec::new();
  let mut stack = vec![root.to_path_buf()];

  for (line_index, raw_line) in text.lines().enumerate() {
    if raw_line.trim().is_empty() {
      continue;
    }
    let indent_width = raw_line.chars().take_while(|ch| *ch == ' ').count();
    if indent_width % 2 != 0 {
      return Err(FileTreeEditError {
        line:    Some(line_index + 1),
        message: "indentation must use 2-space steps".to_string(),
      });
    }
    let depth = indent_width / 2;
    let trimmed = raw_line[indent_width..].trim();
    if trimmed.is_empty() {
      return Err(FileTreeEditError {
        line:    Some(line_index + 1),
        message: "tree row name cannot be empty".to_string(),
      });
    }
    let (kind, name) = if let Some(name) = trimmed.strip_suffix('/') {
      (FileTreeNodeKind::Directory, name.trim())
    } else {
      (FileTreeNodeKind::File, trimmed)
    };
    if name.is_empty() {
      return Err(FileTreeEditError {
        line:    Some(line_index + 1),
        message: "tree row name cannot be empty".to_string(),
      });
    }
    if depth + 1 > stack.len() {
      return Err(FileTreeEditError {
        line:    Some(line_index + 1),
        message: "cannot indent more than one level deeper than the previous line".to_string(),
      });
    }
    stack.truncate(depth + 1);
    let Some(parent) = stack.last().cloned() else {
      return Err(FileTreeEditError {
        line:    Some(line_index + 1),
        message: "missing parent directory".to_string(),
      });
    };
    let path = parent.join(name);
    if matches!(kind, FileTreeNodeKind::Directory) {
      stack.push(path.clone());
    }
    entries.push(FileTreeEditEntry { path, depth, kind });
  }

  Ok(entries)
}

fn diff_edit_entries(original: &[FileTreeEditEntry], parsed: &[FileTreeEditEntry]) -> Vec<FileTreeOp> {
  let shared = original.len().min(parsed.len());
  let mut ops = Vec::new();
  let mut renamed_dirs = Vec::<(PathBuf, PathBuf)>::new();

  for index in 0..shared {
    let before = &original[index];
    let after = &parsed[index];
    if before.kind != after.kind {
      continue;
    }
    if before.kind == FileTreeNodeKind::Directory && before.path != after.path {
      renamed_dirs.push((before.path.clone(), after.path.clone()));
      ops.push(path_change_op(&before.path, &after.path));
    }
  }

  for index in 0..shared {
    let before = &original[index];
    let after = &parsed[index];
    if before.kind != after.kind {
      continue;
    }
    if before.kind == FileTreeNodeKind::Directory {
      continue;
    }

    let expected = renamed_dirs.iter().fold(before.path.clone(), |current, (from, to)| {
      remap_path_if_matches(current, from, to).unwrap_or_else(|| to.clone())
    });
    if expected != after.path {
      ops.push(path_change_op(&before.path, &after.path));
    }
  }

  let deleted_ancestors = original
    .iter()
    .skip(shared)
    .filter(|entry| entry.kind == FileTreeNodeKind::Directory)
    .map(|entry| entry.path.clone())
    .collect::<Vec<_>>();
  for entry in original.iter().skip(shared) {
    if deleted_ancestors
      .iter()
      .any(|ancestor| ancestor != &entry.path && entry.path.starts_with(ancestor))
    {
      continue;
    }
    ops.push(FileTreeOp::delete(entry.path.clone()));
  }

  for entry in parsed.iter().skip(shared) {
    ops.push(match entry.kind {
      FileTreeNodeKind::File => FileTreeOp::create_file(entry.path.clone()),
      FileTreeNodeKind::Directory => FileTreeOp::create_directory(entry.path.clone()),
    });
  }

  ops
}

fn normalize_path(path: &Path) -> PathBuf {
  let absolute = if path.is_absolute() {
    path.to_path_buf()
  } else {
    current_working_dir()
      .ok()
      .map(|cwd| cwd.join(path))
      .unwrap_or_else(|| path.to_path_buf())
  };
  fs::canonicalize(&absolute).unwrap_or(absolute)
}

fn directory_cache_fingerprint(path: &Path) -> Option<DirectoryCacheFingerprint> {
  let metadata = fs::metadata(path).ok()?;
  let modified = metadata.modified().ok()?;
  Some(DirectoryCacheFingerprint {
    modified,
    len: metadata.len(),
  })
}

fn compile_filter_globs(root: &Path, patterns: &[String]) -> Option<Gitignore> {
  if patterns.is_empty() {
    return None;
  }

  let mut builder = GitignoreBuilder::new(root);
  for pattern in patterns {
    let _ = builder.add_line(None, pattern);
  }
  builder.build().ok()
}

fn read_directory_entries(path: &Path) -> std::io::Result<Vec<FileTreeEntry>> {
  let mut entries = fs::read_dir(path)?
    .filter_map(Result::ok)
    .filter_map(|entry| {
      let path = entry.path();
      let name = entry.file_name().to_string_lossy().to_string();
      if name.is_empty() {
        return None;
      }
      let is_dir = entry
        .file_type()
        .map(|ft| ft.is_dir())
        .unwrap_or_else(|_| path.is_dir());
      Some(FileTreeEntry { path, name, is_dir })
    })
    .collect::<Vec<_>>();

  entries.sort_by(|left, right| {
    match (left.is_dir, right.is_dir) {
      (true, false) => Ordering::Less,
      (false, true) => Ordering::Greater,
      _ => {
        let left_lower = left.name.to_lowercase();
        let right_lower = right.name.to_lowercase();
        left_lower
          .cmp(&right_lower)
          .then_with(|| left.name.cmp(&right.name))
      },
    }
  });

  Ok(entries)
}

fn is_hidden_entry_name(name: &str) -> bool {
  name.starts_with('.')
}

fn is_ignored_entry_name(name: &str, is_dir: bool) -> bool {
  if is_dir {
    matches!(
      name,
      ".git" | ".jj" | ".hg" | ".svn" | "node_modules" | "target" | ".direnv"
    )
  } else {
    name == ".DS_Store"
  }
}

#[cfg(test)]
mod tests {
  use std::{
    fs,
    path::PathBuf,
    thread,
    time::Duration,
  };

  use tempfile::tempdir;

  use super::{
    FileTreeDisclosure,
    FileTreeEditSession,
    FileTreeFilter,
    FileTreeGuideColumn,
    FileTreeGuideConnector,
    FileTreeMode,
    FileTreeNodeBadge,
    FileTreeNodeDecoration,
    FileTreeNodeKind,
    FileTreeNodePresentation,
    FileTreeNodeSnapshot,
    FileTreeRowAccent,
    FileTreeState,
    FileTreeSnapshot,
    build_file_tree_row_guides,
    directory_cache_fingerprint,
    file_tree_disclosure,
    file_tree_row_layout,
    normalize_path,
  };

  fn wait_for_directory_fingerprint_change(path: &std::path::Path) {
    let before = directory_cache_fingerprint(path);
    for _ in 0..150 {
      thread::sleep(Duration::from_millis(20));
      if directory_cache_fingerprint(path) != before {
        return;
      }
    }
  }

  #[test]
  fn working_directory_mode_re_roots_when_directory_changes() {
    let temp = tempdir().expect("tempdir");
    let alpha = temp.path().join("alpha");
    let beta = temp.path().join("beta");
    fs::create_dir_all(&alpha).expect("create alpha");
    fs::create_dir_all(&beta).expect("create beta");

    let mut tree = FileTreeState::with_working_directory(alpha.clone());
    assert_eq!(tree.mode, FileTreeMode::WorkingDirectory);
    let alpha_root = normalize_path(&alpha);
    assert_eq!(tree.root(), Some(alpha_root.as_path()));

    tree.sync_for_working_directory(&beta);

    let beta_root = normalize_path(&beta);
    assert_eq!(tree.root(), Some(beta_root.as_path()));
  }

  #[test]
  fn snapshot_refreshes_cached_directory_entries_when_contents_change() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path().to_path_buf();
    fs::write(root.join("alpha.txt"), "alpha\n").expect("seed alpha");

    let mut tree = FileTreeState::with_working_directory(root.clone());
    let first = tree.snapshot(32);
    assert!(first.nodes.iter().any(|node| node.name == "alpha.txt"));
    let first_generation = tree.refresh_generation;

    fs::write(root.join("beta.txt"), "beta\n").expect("seed beta");
    wait_for_directory_fingerprint_change(&root);

    let second = tree.snapshot(32);
    assert!(second.nodes.iter().any(|node| node.name == "beta.txt"));
    assert!(tree.refresh_generation > first_generation);
  }

  #[test]
  fn snapshot_marks_active_node_separately_from_selection() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path().to_path_buf();
    let file = root.join("demo.rs");
    fs::write(&file, "fn main() {}\n").expect("seed file");
    let normalized_file = normalize_path(&file);

    let mut tree = FileTreeState::with_working_directory(root.clone());
    let _ = tree.set_active_path(Some(file.as_path()));
    let _ = tree.select_path(&file);

    let snapshot = tree.snapshot(32);
    let node = snapshot
      .nodes
      .iter()
      .find(|node| node.path == normalized_file)
      .expect("file node");
    assert!(node.active);
    assert!(node.selected);
  }

  #[test]
  fn row_layout_uses_decoration_and_selection_accent() {
    let presentation = FileTreeNodePresentation {
      node: FileTreeNodeSnapshot {
        id: "demo".to_string(),
        path: PathBuf::from("/tmp/demo"),
        name: "demo".to_string(),
        depth: 1,
        kind: FileTreeNodeKind::Directory,
        hidden: false,
        ignored: false,
        expanded: true,
        selected: true,
        active: false,
        has_unloaded_children: false,
      },
      decoration: FileTreeNodeDecoration::default()
        .with_icon("folder_git")
        .with_badge(FileTreeNodeBadge::new("git"))
        .with_secondary_text("tracked")
        .with_status("dirty"),
    };

    let layout = file_tree_row_layout(&presentation);

    assert_eq!(file_tree_disclosure(&presentation.node), FileTreeDisclosure::Expanded);
    assert_eq!(layout.icon, "folder_git");
    assert_eq!(layout.secondary_text.as_deref(), Some("tracked"));
    assert_eq!(layout.status.as_deref(), Some("dirty"));
    assert_eq!(layout.badges.len(), 1);
    assert_eq!(layout.accent, FileTreeRowAccent::Selected);
  }

  #[test]
  fn snapshot_respects_hidden_and_ignored_filter_toggles() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path().to_path_buf();
    fs::create_dir_all(root.join(".git")).expect("create git dir");
    fs::create_dir_all(root.join(".hidden_dir")).expect("create hidden dir");
    fs::create_dir_all(root.join("target")).expect("create target dir");
    fs::write(root.join("visible.rs"), "fn main() {}\n").expect("visible file");
    fs::write(root.join(".env"), "KEY=value\n").expect("hidden file");

    let mut tree = FileTreeState::with_working_directory(root.clone());
    let snapshot = tree.snapshot(64);
    assert!(snapshot.nodes.iter().any(|node| node.name == "visible.rs"));
    assert!(!snapshot.nodes.iter().any(|node| node.name == ".env"));
    assert!(!snapshot.nodes.iter().any(|node| node.name == ".hidden_dir"));
    assert!(!snapshot.nodes.iter().any(|node| node.name == ".git"));
    assert!(!snapshot.nodes.iter().any(|node| node.name == "target"));

    assert!(tree.toggle_show_hidden());
    let hidden_snapshot = tree.snapshot(64);
    assert!(hidden_snapshot.nodes.iter().any(|node| node.name == ".env"));
    assert!(hidden_snapshot.nodes.iter().any(|node| node.name == ".hidden_dir"));
    assert!(!hidden_snapshot.nodes.iter().any(|node| node.name == ".git"));

    assert!(tree.toggle_show_ignored());
    let ignored_snapshot = tree.snapshot(64);
    assert!(ignored_snapshot.nodes.iter().any(|node| node.name == ".git"));
    assert!(ignored_snapshot.nodes.iter().any(|node| node.name == "target"));
  }

  #[test]
  fn include_and_exclude_globs_filter_file_entries() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path().to_path_buf();
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(root.join("src").join("main.rs"), "fn main() {}\n").expect("main");
    fs::write(root.join("src").join("main.ts"), "console.log('x')\n").expect("ts");
    fs::write(root.join("README.md"), "# docs\n").expect("readme");

    let mut tree = FileTreeState::with_working_directory(root.clone());
    assert!(tree.set_include_globs(["**/*.rs"]));
    assert!(tree.set_exclude_globs(["README.md"]));
    let _ = tree.set_expanded(&root.join("src"), true);

    let snapshot = tree.snapshot(64);
    assert!(snapshot.nodes.iter().any(|node| node.name == "src"));
    assert!(snapshot.nodes.iter().any(|node| node.name == "main.rs"));
    assert!(!snapshot.nodes.iter().any(|node| node.name == "main.ts"));
    assert!(!snapshot.nodes.iter().any(|node| node.name == "README.md"));
  }

  #[test]
  fn visible_helpers_expand_reveal_and_navigate() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path().to_path_buf();
    fs::create_dir_all(root.join("src").join("nested")).expect("create nested");
    fs::write(root.join("src").join("lib.rs"), "pub fn lib() {}\n").expect("lib");
    fs::write(
      root.join("src").join("nested").join("mod.rs"),
      "pub fn nested() {}\n",
    )
    .expect("nested mod");

    let mut tree = FileTreeState::with_working_directory(root.clone());
    let nested_file = root.join("src").join("nested").join("mod.rs");

    assert!(!tree.is_visible(&nested_file));
    assert!(tree.ensure_visible(&nested_file));
    assert!(tree.is_visible(&nested_file));

    let next = tree
      .next_visible(&root.join("src"), |node| node.kind == FileTreeNodeKind::File)
      .expect("next visible file");
    assert_eq!(next, normalize_path(&nested_file));

    let next_after_nested = tree
      .next_visible(&nested_file, |node| node.kind == FileTreeNodeKind::File)
      .expect("next file after nested");
    assert_eq!(next_after_nested, normalize_path(&root.join("src").join("lib.rs")));

    let prev = tree
      .prev_visible(&nested_file, |node| node.kind == FileTreeNodeKind::Directory)
      .expect("previous visible directory");
    assert_eq!(prev, normalize_path(&root.join("src").join("nested")));

    assert!(tree.close_all(None));
    assert!(!tree.is_visible(&nested_file));
  }

  #[test]
  fn row_guides_capture_branch_and_continuation_shape() {
    let snapshot = FileTreeSnapshot {
      visible: true,
      root: PathBuf::from("/workspace"),
      mode: FileTreeMode::WorkingDirectory,
      filter: FileTreeFilter::default(),
      selected_path: None,
      active_path: None,
      refresh_generation: 1,
      nodes: vec![
        FileTreeNodeSnapshot {
          id: "/workspace".to_string(),
          path: PathBuf::from("/workspace"),
          name: "workspace".to_string(),
          depth: 0,
          kind: FileTreeNodeKind::Directory,
          hidden: false,
          ignored: false,
          expanded: true,
          selected: false,
          active: false,
          has_unloaded_children: false,
        },
        FileTreeNodeSnapshot {
          id: "/workspace/src".to_string(),
          path: PathBuf::from("/workspace/src"),
          name: "src".to_string(),
          depth: 1,
          kind: FileTreeNodeKind::Directory,
          hidden: false,
          ignored: false,
          expanded: true,
          selected: false,
          active: false,
          has_unloaded_children: false,
        },
        FileTreeNodeSnapshot {
          id: "/workspace/src/lib.rs".to_string(),
          path: PathBuf::from("/workspace/src/lib.rs"),
          name: "lib.rs".to_string(),
          depth: 2,
          kind: FileTreeNodeKind::File,
          hidden: false,
          ignored: false,
          expanded: false,
          selected: false,
          active: false,
          has_unloaded_children: false,
        },
        FileTreeNodeSnapshot {
          id: "/workspace/tests".to_string(),
          path: PathBuf::from("/workspace/tests"),
          name: "tests".to_string(),
          depth: 1,
          kind: FileTreeNodeKind::Directory,
          hidden: false,
          ignored: false,
          expanded: false,
          selected: false,
          active: false,
          has_unloaded_children: false,
        },
      ],
    };

    let guides = build_file_tree_row_guides(&snapshot);
    assert_eq!(guides[0].connector, FileTreeGuideConnector::None);
    assert_eq!(guides[1].connector, FileTreeGuideConnector::Branch);
    assert_eq!(guides[2].ancestor_columns, vec![FileTreeGuideColumn::Continue]);
    assert_eq!(guides[2].connector, FileTreeGuideConnector::LastChild);
    assert_eq!(guides[3].connector, FileTreeGuideConnector::LastChild);
  }

  #[test]
  fn edit_session_parses_rename_create_and_delete_operations() {
    let snapshot = FileTreeSnapshot {
      visible: true,
      root: PathBuf::from("/workspace"),
      mode: FileTreeMode::WorkingDirectory,
      filter: FileTreeFilter::default(),
      selected_path: None,
      active_path: None,
      refresh_generation: 1,
      nodes: vec![
        FileTreeNodeSnapshot {
          id: "/workspace".to_string(),
          path: PathBuf::from("/workspace"),
          name: "workspace".to_string(),
          depth: 0,
          kind: FileTreeNodeKind::Directory,
          hidden: false,
          ignored: false,
          expanded: true,
          selected: false,
          active: false,
          has_unloaded_children: false,
        },
        FileTreeNodeSnapshot {
          id: "/workspace/src".to_string(),
          path: PathBuf::from("/workspace/src"),
          name: "src".to_string(),
          depth: 1,
          kind: FileTreeNodeKind::Directory,
          hidden: false,
          ignored: false,
          expanded: true,
          selected: false,
          active: false,
          has_unloaded_children: false,
        },
        FileTreeNodeSnapshot {
          id: "/workspace/src/main.rs".to_string(),
          path: PathBuf::from("/workspace/src/main.rs"),
          name: "main.rs".to_string(),
          depth: 2,
          kind: FileTreeNodeKind::File,
          hidden: false,
          ignored: false,
          expanded: false,
          selected: false,
          active: false,
          has_unloaded_children: false,
        },
      ],
    };

    let session = FileTreeEditSession::from_snapshot(&snapshot);
    let patch = session
      .parse("lib/\n  main.rs\nREADME.md")
      .expect("parse patch");

    assert_eq!(patch.operations.len(), 2);
    assert!(patch.operations.iter().any(|op| {
      matches!(op, super::FileTreeOp::Rename { from, to }
        if from == &PathBuf::from("/workspace/src") && to == &PathBuf::from("/workspace/lib"))
    }));
    assert!(patch.operations.iter().any(|op| {
      matches!(op, super::FileTreeOp::CreateFile { path }
        if path == &PathBuf::from("/workspace/README.md"))
    }));
  }
}
