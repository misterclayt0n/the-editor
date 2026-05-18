use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::{Path, PathBuf},
  time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorkspaceRootId(u64);

impl WorkspaceRootId {
  pub fn new(id: u64) -> Option<Self> {
    (id != 0).then_some(Self(id))
  }

  pub fn get(self) -> u64 {
    self.0
  }

  fn next(counter: &mut u64) -> Self {
    *counter = counter.wrapping_add(1).max(1);
    Self(*counter)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorkspaceEntryId(u64);

impl WorkspaceEntryId {
  pub fn get(self) -> u64 {
    self.0
  }

  fn next(counter: &mut u64) -> Self {
    *counter = counter.wrapping_add(1).max(1);
    Self(*counter)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceEntryKind {
  File,
  Directory,
  Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntry {
  pub id:    u64,
  pub root_id: u64,
  pub path:  String,
  pub kind:  WorkspaceEntryKind,
  pub mtime: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceEntryChangeKind {
  Added,
  Removed,
  Modified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntryChange {
  pub kind:  WorkspaceEntryChangeKind,
  pub entry: WorkspaceEntry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
  pub root_id:   u64,
  pub root_path: String,
  pub revision:  u64,
  pub entries:   Vec<WorkspaceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntriesChanged {
  pub root_id:   u64,
  pub root_path: String,
  pub revision:  u64,
  pub changes:   Vec<WorkspaceEntryChange>,
}

#[derive(Debug, Error)]
pub enum WorkspaceError {
  #[error("unknown workspace root {0:?}")]
  UnknownRoot(WorkspaceRootId),
  #[error("failed to canonicalize workspace root {path:?}: {source}")]
  Canonicalize { path: PathBuf, source: std::io::Error },
  #[error("failed to scan workspace root {path:?}: {source}")]
  Scan { path: PathBuf, source: std::io::Error },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryRecord {
  id:    WorkspaceEntryId,
  path:  String,
  kind:  WorkspaceEntryKind,
  mtime: Option<u64>,
}

#[derive(Debug)]
struct WorkspaceRoot {
  id:             WorkspaceRootId,
  path:           PathBuf,
  revision:       u64,
  next_entry_id:  u64,
  entries_by_path: BTreeMap<String, EntryRecord>,
}

#[derive(Debug, Default)]
pub struct WorkspaceManager {
  next_root_id: u64,
  roots:        BTreeMap<WorkspaceRootId, WorkspaceRoot>,
  roots_by_path: BTreeMap<PathBuf, WorkspaceRootId>,
}

impl WorkspaceManager {
  pub fn add_root(&mut self, path: &Path) -> Result<WorkspaceSnapshot, WorkspaceError> {
    let root_path = canonicalize_dir(path)?;
    if let Some(root_id) = self.roots_by_path.get(&root_path).copied() {
      return self.snapshot(root_id);
    }

    let id = WorkspaceRootId::next(&mut self.next_root_id);
    let mut root = WorkspaceRoot {
      id,
      path: root_path.clone(),
      revision: 0,
      next_entry_id: 0,
      entries_by_path: BTreeMap::new(),
    };
    let scanned = scan_root(id, &root_path, &mut root.next_entry_id)?;
    root.entries_by_path = scanned;
    root.revision = 1;
    self.roots_by_path.insert(root_path, id);
    self.roots.insert(id, root);
    self.snapshot(id)
  }

  pub fn snapshot(&self, root_id: WorkspaceRootId) -> Result<WorkspaceSnapshot, WorkspaceError> {
    let root = self.roots.get(&root_id).ok_or(WorkspaceError::UnknownRoot(root_id))?;
    Ok(WorkspaceSnapshot {
      root_id: root.id.get(),
      root_path: root.path.to_string_lossy().into_owned(),
      revision: root.revision,
      entries: root.entries_by_path.values().map(|entry| public_entry(root.id, entry)).collect(),
    })
  }

  pub fn refresh_root(&mut self, root_id: WorkspaceRootId) -> Result<WorkspaceEntriesChanged, WorkspaceError> {
    let root = self.roots.get_mut(&root_id).ok_or(WorkspaceError::UnknownRoot(root_id))?;
    let next_entries = scan_root(root.id, &root.path, &mut root.next_entry_id)?;
    let changes = diff_entries(root.id, &root.entries_by_path, &next_entries);
    if !changes.is_empty() {
      root.revision = root.revision.saturating_add(1);
      root.entries_by_path = next_entries;
    }
    Ok(WorkspaceEntriesChanged {
      root_id: root.id.get(),
      root_path: root.path.to_string_lossy().into_owned(),
      revision: root.revision,
      changes,
    })
  }

  pub fn refresh_all(&mut self) -> Result<Vec<WorkspaceEntriesChanged>, WorkspaceError> {
    let ids = self.roots.keys().copied().collect::<Vec<_>>();
    ids.into_iter().map(|id| self.refresh_root(id)).collect()
  }
}

fn canonicalize_dir(path: &Path) -> Result<PathBuf, WorkspaceError> {
  dunce::canonicalize(path).map_err(|source| WorkspaceError::Canonicalize { path: path.to_path_buf(), source })
}

fn scan_root(
  root_id: WorkspaceRootId,
  root_path: &Path,
  next_entry_id: &mut u64,
) -> Result<BTreeMap<String, EntryRecord>, WorkspaceError> {
  let mut entries = BTreeMap::new();
  scan_dir(root_id, root_path, root_path, next_entry_id, &mut entries)?;
  Ok(entries)
}

fn scan_dir(
  root_id: WorkspaceRootId,
  root_path: &Path,
  dir: &Path,
  next_entry_id: &mut u64,
  entries: &mut BTreeMap<String, EntryRecord>,
) -> Result<(), WorkspaceError> {
  let read_dir = fs::read_dir(dir).map_err(|source| WorkspaceError::Scan { path: dir.to_path_buf(), source })?;
  for entry in read_dir {
    let entry = entry.map_err(|source| WorkspaceError::Scan { path: dir.to_path_buf(), source })?;
    let name = entry.file_name();
    if should_ignore_name(&name.to_string_lossy()) {
      continue;
    }
    let path = entry.path();
    let metadata = fs::symlink_metadata(&path).map_err(|source| WorkspaceError::Scan { path: path.clone(), source })?;
    let file_type = metadata.file_type();
    let kind = if file_type.is_symlink() {
      WorkspaceEntryKind::Symlink
    } else if metadata.is_dir() {
      WorkspaceEntryKind::Directory
    } else if metadata.is_file() {
      WorkspaceEntryKind::File
    } else {
      continue;
    };
    let rel = normalize_relative_path(root_path, &path);
    entries.insert(rel.clone(), EntryRecord {
      id: WorkspaceEntryId::next(next_entry_id),
      path: rel,
      kind,
      mtime: metadata_mtime(&metadata),
    });
    if kind == WorkspaceEntryKind::Directory {
      scan_dir(root_id, root_path, &path, next_entry_id, entries)?;
    }
  }
  let _ = root_id;
  Ok(())
}

fn diff_entries(
  root_id: WorkspaceRootId,
  old: &BTreeMap<String, EntryRecord>,
  new: &BTreeMap<String, EntryRecord>,
) -> Vec<WorkspaceEntryChange> {
  let paths = old.keys().chain(new.keys()).cloned().collect::<BTreeSet<_>>();
  let mut changes = Vec::new();
  for path in paths {
    match (old.get(&path), new.get(&path)) {
      (None, Some(entry)) => changes.push(WorkspaceEntryChange { kind: WorkspaceEntryChangeKind::Added, entry: public_entry(root_id, entry) }),
      (Some(entry), None) => changes.push(WorkspaceEntryChange { kind: WorkspaceEntryChangeKind::Removed, entry: public_entry(root_id, entry) }),
      (Some(old), Some(new)) if old.kind != new.kind || old.mtime != new.mtime => {
        changes.push(WorkspaceEntryChange { kind: WorkspaceEntryChangeKind::Modified, entry: public_entry(root_id, new) });
      },
      _ => {},
    }
  }
  changes
}

fn public_entry(root_id: WorkspaceRootId, entry: &EntryRecord) -> WorkspaceEntry {
  WorkspaceEntry {
    id: entry.id.get(),
    root_id: root_id.get(),
    path: entry.path.clone(),
    kind: entry.kind,
    mtime: entry.mtime,
  }
}

fn should_ignore_name(name: &str) -> bool {
  if name.starts_with('.') && name != ".env" && name != ".gitignore" {
    return true;
  }
  matches!(name, ".git" | ".jj" | "node_modules" | "target" | "dist" | ".next")
}

fn normalize_relative_path(root: &Path, path: &Path) -> String {
  path.strip_prefix(root)
    .unwrap_or(path)
    .to_string_lossy()
    .replace('\\', "/")
}

pub fn metadata_mtime(metadata: &fs::Metadata) -> Option<u64> {
  metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok().map(|duration| duration.as_nanos().min(u128::from(u64::MAX)) as u64)
}
