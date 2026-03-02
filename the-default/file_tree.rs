use std::{
  cmp::Ordering,
  collections::{
    HashMap,
    HashSet,
  },
  env,
  fs,
  path::{
    Path,
    PathBuf,
  },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileTreeMode {
  #[default]
  WorkspaceRoot,
  CurrentBufferDirectory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTreeNodeKind {
  File,
  Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeNodeSnapshot {
  pub id:                    String,
  pub path:                  PathBuf,
  pub name:                  String,
  pub depth:                 usize,
  pub kind:                  FileTreeNodeKind,
  pub expanded:              bool,
  pub selected:              bool,
  pub has_unloaded_children: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeSnapshot {
  pub visible:            bool,
  pub root:               PathBuf,
  pub mode:               FileTreeMode,
  pub selected_path:      Option<PathBuf>,
  pub refresh_generation: u64,
  pub nodes:              Vec<FileTreeNodeSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileTreeEntry {
  path:   PathBuf,
  name:   String,
  is_dir: bool,
}

#[derive(Debug, Clone, Default)]
pub struct FileTreeState {
  pub visible:            bool,
  pub mode:               FileTreeMode,
  pub selected_path:      Option<PathBuf>,
  pub refresh_generation: u64,
  root:                   Option<PathBuf>,
  expanded_dirs:          HashSet<PathBuf>,
  nodes_cache:            HashMap<PathBuf, Option<Vec<FileTreeEntry>>>,
}

impl FileTreeState {
  #[must_use]
  pub fn with_workspace_root(root: PathBuf) -> Self {
    let mut state = Self::default();
    state.set_root_internal(FileTreeMode::WorkspaceRoot, root);
    state
  }

  #[must_use]
  pub fn root(&self) -> Option<&Path> {
    self.root.as_deref()
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

  pub fn toggle_workspace_root(&mut self, workspace_root: &Path) {
    if self.visible && self.mode == FileTreeMode::WorkspaceRoot {
      self.set_visible(false);
      return;
    }
    self.set_root_internal(FileTreeMode::WorkspaceRoot, workspace_root.to_path_buf());
    self.set_visible(true);
  }

  pub fn open_workspace_root(&mut self, workspace_root: &Path) {
    self.set_root_internal(FileTreeMode::WorkspaceRoot, workspace_root.to_path_buf());
    self.set_visible(true);
  }

  pub fn open_current_buffer_directory(
    &mut self,
    current_file: Option<&Path>,
    workspace_root: &Path,
  ) {
    let root = current_file
      .and_then(Path::parent)
      .unwrap_or(workspace_root)
      .to_path_buf();
    self.set_root_internal(FileTreeMode::CurrentBufferDirectory, root);
    if let Some(path) = current_file {
      let _ = self.select_path(path);
    }
    self.set_visible(true);
  }

  pub fn sync_for_active_file(&mut self, workspace_root: &Path, active_file: Option<&Path>) {
    match self.mode {
      FileTreeMode::WorkspaceRoot => {
        if self.root.is_none() {
          self.set_root_internal(FileTreeMode::WorkspaceRoot, workspace_root.to_path_buf());
        }
      },
      FileTreeMode::CurrentBufferDirectory => {
        let root = active_file
          .and_then(Path::parent)
          .unwrap_or(workspace_root)
          .to_path_buf();
        self.set_root_internal(FileTreeMode::CurrentBufferDirectory, root);
      },
    }

    if let Some(path) = active_file {
      let _ = self.select_path(path);
    }
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

  pub fn select_path(&mut self, path: &Path) -> bool {
    let normalized = normalize_path(path);
    let Some(root) = self.root.as_ref() else {
      return false;
    };
    if !normalized.starts_with(root) {
      return false;
    }

    let mut changed = self.selected_path.as_ref() != Some(&normalized);
    self.selected_path = Some(normalized.clone());

    // Keep selection independent from expansion: selecting a directory should
    // not force it open. We only ensure parents are expanded so the selected
    // entry remains visible.
    if let Some(parent) = normalized.parent() {
      let ancestors = parent
        .ancestors()
        .take_while(|ancestor| ancestor.starts_with(root))
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
      for ancestor in ancestors {
        if self.expanded_dirs.insert(ancestor.clone()) {
          changed = true;
        }
        if ancestor.is_dir() {
          let _ = self.load_children(&ancestor);
        }
      }
    }

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
    self.append_node(&mut nodes, &root, 0, limit);

    FileTreeSnapshot {
      visible: self.visible,
      root,
      mode: self.mode,
      selected_path: self.selected_path.clone(),
      refresh_generation: self.refresh_generation,
      nodes,
    }
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

    let _ = self.load_children(&normalized_root);
    self.bump_generation();
  }

  fn append_node(
    &mut self,
    out: &mut Vec<FileTreeNodeSnapshot>,
    path: &Path,
    depth: usize,
    limit: usize,
  ) {
    if out.len() >= limit {
      return;
    }

    let path_buf = path.to_path_buf();
    let is_dir = path_buf.is_dir();
    let expanded = is_dir && self.expanded_dirs.contains(&path_buf);
    let has_unloaded_children = is_dir && !expanded && !self.nodes_cache.contains_key(&path_buf);
    let selected = self.selected_path.as_ref() == Some(&path_buf);
    out.push(FileTreeNodeSnapshot {
      id: stable_id_for_path(&path_buf),
      path: path_buf.clone(),
      name: node_name_for_path(&path_buf),
      depth,
      kind: if is_dir {
        FileTreeNodeKind::Directory
      } else {
        FileTreeNodeKind::File
      },
      expanded,
      selected,
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
      self.append_node(out, entry.path.as_path(), depth + 1, limit);
    }
  }

  fn load_children(&mut self, dir: &Path) -> Option<Vec<FileTreeEntry>> {
    let key = normalize_path(dir);
    if !self.nodes_cache.contains_key(&key) {
      let loaded = read_directory_entries(&key).ok();
      self.nodes_cache.insert(key.clone(), loaded);
    }
    self.nodes_cache.get(&key).cloned().flatten()
  }

  fn bump_generation(&mut self) {
    self.refresh_generation = self.refresh_generation.saturating_add(1);
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

fn normalize_path(path: &Path) -> PathBuf {
  let absolute = if path.is_absolute() {
    path.to_path_buf()
  } else {
    env::current_dir()
      .ok()
      .map(|cwd| cwd.join(path))
      .unwrap_or_else(|| path.to_path_buf())
  };
  fs::canonicalize(&absolute).unwrap_or(absolute)
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
      if should_hide_entry(name.as_str(), is_dir) {
        return None;
      }
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

fn should_hide_entry(name: &str, is_dir: bool) -> bool {
  if is_dir && matches!(name, ".git" | ".jj") {
    return true;
  }
  name == ".DS_Store"
}
