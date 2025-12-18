use std::{
  borrow::Cow,
  cmp::Ordering,
  collections::HashMap,
  fs::DirEntry,
  path::{
    Path,
    PathBuf,
  },
  sync::{
    Arc,
    Mutex,
  },
};

use anyhow::{
  Result,
  bail,
  ensure,
};
use the_editor_renderer::Key;

use crate::{
  core::{
    animation::{
      AnimationHandle,
      presets,
    },
    graphics::{
      CursorKind,
      Rect,
    },
    position::Position,
  },
  doc,
  editor::{
    Action,
    Editor,
  },
  keymap::KeyBinding,
  ui::{
    GitFileStatus,
    TreeOp,
    TreeView,
    TreeViewItem,
    components::Prompt,
    compositor::{
      Component,
      Context,
      Event,
      EventResult,
      Surface,
    },
  },
};

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
enum FileType {
  File,
  Folder,
  Root,
}

#[derive(Debug, Clone)]
struct FileInfo {
  file_type:        FileType,
  path:             PathBuf,
  git_status_cache: Option<GitStatusCache>,
}

impl PartialEq for FileInfo {
  fn eq(&self, other: &Self) -> bool {
    self.file_type == other.file_type && self.path == other.path
  }
}

impl Eq for FileInfo {}

impl FileInfo {
  fn root(path: PathBuf) -> Self {
    Self {
      file_type: FileType::Root,
      path,
      git_status_cache: None,
    }
  }

  fn with_git_cache(mut self, cache: GitStatusCache) -> Self {
    self.git_status_cache = Some(cache);
    self
  }

  fn get_text(&self) -> Cow<'static, str> {
    let text = match self.file_type {
      FileType::Root => self.path.display().to_string(),
      FileType::File | FileType::Folder => {
        self
          .path
          .file_name()
          .map_or("/".into(), |p| p.to_string_lossy().into_owned())
      },
    };

    #[cfg(test)]
    let text = text.replace(std::path::MAIN_SEPARATOR, "/");

    text.into()
  }
}

impl PartialOrd for FileInfo {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for FileInfo {
  fn cmp(&self, other: &Self) -> Ordering {
    use FileType::*;
    match (self.file_type, other.file_type) {
      (Root, _) => return Ordering::Less,
      (_, Root) => return Ordering::Greater,
      _ => {},
    };

    if let (Some(p1), Some(p2)) = (self.path.parent(), other.path.parent()) {
      if p1 == p2 {
        match (self.file_type, other.file_type) {
          (Folder, File) => return Ordering::Less,
          (File, Folder) => return Ordering::Greater,
          _ => {},
        };
      }
    }
    self.path.cmp(&other.path)
  }
}

impl TreeViewItem for FileInfo {
  type Params = State;

  fn get_children(&self) -> Result<Vec<Self>> {
    match self.file_type {
      FileType::Root | FileType::Folder => {},
      _ => return Ok(vec![]),
    };
    let ret: Vec<_> = std::fs::read_dir(&self.path)?
      .filter_map(|entry| entry.ok())
      .filter_map(|entry| dir_entry_to_file_info(entry, &self.path, self.git_status_cache.clone()))
      .collect();
    Ok(ret)
  }

  fn name(&self) -> String {
    self.get_text().to_string()
  }

  fn is_parent(&self) -> bool {
    matches!(self.file_type, FileType::Folder | FileType::Root)
  }

  fn git_status(&self) -> GitFileStatus {
    let Some(cache) = self.git_status_cache.as_ref() else {
      return GitFileStatus::None;
    };
    let Ok(cache) = cache.lock() else {
      return GitFileStatus::None;
    };

    // For files, just look up directly
    if !self.is_parent() {
      return cache
        .get(&self.path)
        .copied()
        .unwrap_or(GitFileStatus::None);
    }

    // For directories, find the highest priority status among all descendants
    // Priority: Conflict > Deleted > Modified > Renamed > New > Ignored > None
    let mut highest_status = GitFileStatus::None;

    for (path, status) in cache.iter() {
      if path.starts_with(&self.path) && path != &self.path {
        highest_status = higher_priority_status(highest_status, *status);
        // Early exit if we found the highest possible priority
        if highest_status == GitFileStatus::Conflict {
          break;
        }
      }
    }

    highest_status
  }
}

/// Returns the status with higher priority (more important to show)
fn higher_priority_status(a: GitFileStatus, b: GitFileStatus) -> GitFileStatus {
  fn priority(s: GitFileStatus) -> u8 {
    match s {
      GitFileStatus::None => 0,
      GitFileStatus::Ignored => 1,
      GitFileStatus::New => 2,
      GitFileStatus::Renamed => 3,
      GitFileStatus::Modified => 4,
      GitFileStatus::Deleted => 5,
      GitFileStatus::Conflict => 6,
    }
  }
  if priority(a) >= priority(b) { a } else { b }
}

fn dir_entry_to_file_info(
  entry: DirEntry,
  path: &Path,
  git_cache: Option<GitStatusCache>,
) -> Option<FileInfo> {
  entry.metadata().ok().map(|meta| {
    let file_type = match meta.is_dir() {
      true => FileType::Folder,
      false => FileType::File,
    };
    let full_path = path.join(entry.file_name());
    FileInfo {
      file_type,
      path: full_path,
      git_status_cache: git_cache,
    }
  })
}

#[derive(Clone, Debug, Default)]
struct State {
  focus:        bool,
  current_root: PathBuf,
  area_width:   u16,
}

impl State {
  fn new(focus: bool, current_root: PathBuf) -> Self {
    Self {
      focus,
      current_root,
      area_width: 0,
    }
  }
}

struct ExplorerHistory {
  tree:         TreeView<FileInfo>,
  current_root: PathBuf,
}

// Re-export FileTreePosition from editor for backwards compatibility
pub use crate::editor::FileTreePosition as ExplorerPosition;

/// Shared git status cache that can be updated from background threads
type GitStatusCache = Arc<Mutex<HashMap<PathBuf, GitFileStatus>>>;

pub struct Explorer {
  tree:             TreeView<FileInfo>,
  history:          Vec<ExplorerHistory>,
  show_help:        bool,
  state:            State,
  #[allow(clippy::type_complexity)]
  on_next_key:      Option<Box<dyn FnMut(&mut Context, &mut Self, &KeyBinding) -> EventResult>>,
  column_width:     u16,
  /// Width animation (0.0 = closed, 1.0 = fully open)
  /// Uses retarget() to smoothly animate between states
  width_anim:       AnimationHandle<f32>,
  /// Cache of git status for files
  git_status_cache: GitStatusCache,
  /// Path to reveal when the explorer is next opened.
  /// This is set when auto-reveal is triggered while the explorer is closed.
  pending_reveal:   Option<PathBuf>,
}

/// Default column width for the explorer
const DEFAULT_EXPLORER_COLUMN_WIDTH: u16 = 30;

impl Explorer {
  pub fn new(cx: &mut Context) -> Result<Self> {
    let current_root = std::env::current_dir()
      .unwrap_or_else(|_| "./".into())
      .canonicalize()?;
    let git_status_cache: GitStatusCache = Arc::new(Mutex::new(HashMap::new()));
    let (duration, easing) = presets::FAST;

    let mut explorer = Self {
      tree:             Self::new_tree_view(current_root.clone(), Some(git_status_cache.clone()))?,
      history:          vec![],
      show_help:        false,
      state:            State::new(true, current_root.clone()),
      on_next_key:      None,
      column_width:     DEFAULT_EXPLORER_COLUMN_WIDTH,
      // Start fully open (1.0)
      width_anim:       AnimationHandle::new(1.0, 1.0, duration, easing),
      git_status_cache: git_status_cache.clone(),
      pending_reveal:   None,
    };

    // Start initial git status refresh
    explorer.refresh_git_status(cx);

    Ok(explorer)
  }

  #[cfg(test)]
  fn from_path(root: PathBuf, column_width: u16) -> Result<Self> {
    let git_status_cache: GitStatusCache = Arc::new(Mutex::new(HashMap::new()));
    let (duration, easing) = presets::FAST;
    Ok(Self {
      tree: Self::new_tree_view(root.clone(), Some(git_status_cache.clone()))?,
      history: vec![],
      show_help: false,
      state: State::new(true, root),
      on_next_key: None,
      column_width,
      width_anim: AnimationHandle::new(1.0, 1.0, duration, easing),
      git_status_cache: Arc::new(Mutex::new(HashMap::new())),
      pending_reveal: None,
    })
  }

  fn new_tree_view(root: PathBuf, git_cache: Option<GitStatusCache>) -> Result<TreeView<FileInfo>> {
    let mut root = FileInfo::root(root);
    if let Some(cache) = git_cache {
      root = root.with_git_cache(cache);
    }
    Ok(TreeView::build_tree(root)?.with_enter_fn(Self::toggle_current))
  }

  fn push_history(&mut self, tree_view: TreeView<FileInfo>, current_root: PathBuf) {
    self.history.push(ExplorerHistory {
      tree: tree_view,
      current_root,
    });
    const MAX_HISTORY_SIZE: usize = 20;
    Vec::truncate(&mut self.history, MAX_HISTORY_SIZE)
  }

  fn change_root(&mut self, root: PathBuf) -> Result<()> {
    if self.state.current_root.eq(&root) {
      return Ok(());
    }
    let tree = Self::new_tree_view(root.clone(), Some(self.git_status_cache.clone()))?;
    let old_tree = std::mem::replace(&mut self.tree, tree);
    self.push_history(old_tree, self.state.current_root.clone());
    self.state.current_root = root;
    Ok(())
  }

  pub fn reveal_file(&mut self, path: PathBuf) -> Result<()> {
    let current_root = &self.state.current_root.canonicalize()?;
    let current_path = &path.canonicalize()?;
    let segments = {
      let stripped = match current_path.strip_prefix(current_root) {
        Ok(stripped) => Ok(stripped),
        Err(_) => {
          let parent = path.parent().ok_or_else(|| {
            anyhow::anyhow!("Failed get parent of '{}'", current_path.to_string_lossy())
          })?;
          self.change_root(parent.into())?;
          current_path
            .strip_prefix(parent.canonicalize()?)
            .map_err(|_| {
              anyhow::anyhow!(
                "Failed to strip prefix (parent) '{}' from '{}'",
                parent.to_string_lossy(),
                current_path.to_string_lossy()
              )
            })
        },
      }?;

      stripped
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
    };
    self.tree.reveal_item(segments)?;
    Ok(())
  }

  pub fn reveal_current_file(&mut self, cx: &mut Context) -> Result<()> {
    self.focus();
    let current_document_path = doc!(cx.editor).path().cloned();
    match current_document_path {
      None => Ok(()),
      Some(current_path) => self.reveal_file(current_path),
    }
  }

  /// Reveal a file in the tree without focusing the explorer.
  /// This is used for auto-reveal functionality where we want to
  /// show the file location without stealing focus from the editor.
  pub fn reveal_file_quiet(&mut self, path: PathBuf) -> Result<()> {
    // If the explorer is closed, store the path to reveal when it opens
    if !self.is_opened() {
      log::debug!(
        "Explorer is not open, storing pending reveal for {:?}",
        path
      );
      self.pending_reveal = Some(path);
      return Ok(());
    }
    log::debug!("Explorer is open, calling reveal_file");
    self.reveal_file(path)
  }

  /// Refresh the tree and reveal a specific path.
  /// Used after file operations (create, rename, delete) to update the tree.
  pub fn refresh_and_reveal(&mut self, path: PathBuf) -> Result<()> {
    self.tree.refresh()?;
    self.reveal_file(path)
  }

  /// Refresh the tree without revealing any specific file.
  pub fn refresh(&mut self) -> Result<()> {
    self.tree.refresh()
  }

  pub fn focus(&mut self) {
    self.state.focus = true;
    // Animate to fully open
    self.width_anim.retarget(1.0);

    // Execute any pending reveal from when the explorer was closed
    if let Some(path) = self.pending_reveal.take() {
      log::debug!("Executing pending reveal for {:?}", path);
      if let Err(e) = self.reveal_file(path) {
        log::warn!("Failed to execute pending reveal: {}", e);
      }
    }
  }

  pub fn unfocus(&mut self) {
    self.state.focus = false;
  }

  /// Close the explorer with animation
  pub fn close(&mut self) {
    self.state.focus = false;
    // Animate to fully closed
    self.width_anim.retarget(0.0);
  }

  /// Check if the explorer is currently animating
  pub fn is_animating(&self) -> bool {
    !self.width_anim.is_complete() || self.tree.is_animating()
  }

  /// Update the animation. Returns true if explorer just finished closing.
  pub fn update_closing(&mut self, dt: f32) -> bool {
    // Update tree animations even when explorer is not visible to keep them in sync
    self.tree.tick_animations(dt);

    let was_closing = *self.width_anim.target() == 0.0 && !self.width_anim.is_complete();
    self.width_anim.update(dt);

    // Return true if we just finished closing
    was_closing && self.width_anim.is_complete()
  }

  /// Get the animation progress (1.0 = fully open, 0.0 = fully closed)
  pub fn closing_progress(&self) -> f32 {
    *self.width_anim.current()
  }

  /// Check if explorer is open (target is open, regardless of animation)
  pub fn is_opened(&self) -> bool {
    *self.width_anim.target() > 0.0
  }

  pub fn is_focus(&self) -> bool {
    self.state.focus
  }

  /// Refresh git status for all files in the current root.
  /// This spawns a background task that updates the git status cache.
  pub fn refresh_git_status(&mut self, cx: &mut Context) {
    self.refresh_git_status_with_providers(&cx.editor.diff_providers);
  }

  /// Refresh git status using the provided diff providers.
  /// This is useful when called from contexts without access to a full Context.
  pub fn refresh_git_status_with_providers(
    &mut self,
    diff_providers: &the_editor_vcs::DiffProviderRegistry,
  ) {
    use std::collections::HashMap;

    use the_editor_vcs::FileChange;

    let cwd = self.state.current_root.clone();
    let cache = self.git_status_cache.clone();
    let diff_providers = diff_providers.clone();

    // Build new cache in background, then swap atomically to avoid flashing.
    // We collect all changes first, then replace the cache contents all at once.
    let new_cache: Arc<Mutex<HashMap<PathBuf, GitFileStatus>>> =
      Arc::new(Mutex::new(HashMap::new()));
    let new_cache_for_iteration = new_cache.clone();
    let new_cache_for_completion = new_cache.clone();

    // Spawn background task to fetch git status
    diff_providers.for_each_changed_file_with_completion(
      cwd,
      move |result| {
        match result {
          Ok(change) => {
            let (path, status) = match change {
              FileChange::Modified { path } => (path, GitFileStatus::Modified),
              FileChange::Untracked { path } => (path, GitFileStatus::New),
              FileChange::Deleted { path } => (path, GitFileStatus::Deleted),
              FileChange::Conflict { path } => (path, GitFileStatus::Conflict),
              FileChange::Renamed { to_path, .. } => (to_path, GitFileStatus::Renamed),
            };
            if let Ok(mut new_cache) = new_cache_for_iteration.lock() {
              new_cache.insert(path, status);
            }
            true // Continue iteration
          },
          Err(_) => false, // Stop on error
        }
      },
      move || {
        // On completion, swap the new cache into the main cache atomically
        if let (Ok(mut main_cache), Ok(new_cache)) = (cache.lock(), new_cache_for_completion.lock())
        {
          main_cache.clear();
          main_cache.extend(new_cache.iter().map(|(k, v)| (k.clone(), *v)));
        }
      },
    );
  }

  /// Look up the git status for a path from the cache
  pub fn get_git_status(&self, path: &Path) -> GitFileStatus {
    self
      .git_status_cache
      .lock()
      .ok()
      .and_then(|cache| cache.get(path).copied())
      .unwrap_or(GitFileStatus::None)
  }

  fn new_create_file_or_folder_prompt(&mut self, cx: &mut Context) -> Result<EventResult> {
    use std::sync::Arc;

    use crate::ui::components::prompt::{
      Completion,
      PromptEvent,
    };

    let folder_path = self.nearest_folder()?;
    // Prefill with just the folder path (with trailing separator)
    let prefill = format!(
      "{}{}",
      folder_path.to_string_lossy(),
      std::path::MAIN_SEPARATOR
    );

    // Set custom mode string for the statusline
    cx.editor.set_custom_mode_str("ADD FILE".to_string());

    // Set mode to Command so prompt keybindings work
    cx.editor.set_mode(crate::keymap::Mode::Command);

    // Create file path completion function
    let file_completion: Arc<
      dyn Fn(&crate::editor::Editor, &str) -> Vec<Completion> + Send + Sync,
    > = Arc::new(|_editor, input| {
      if input.is_empty() {
        return Vec::new();
      }

      let path = PathBuf::from(input);

      // Determine the directory to list and the prefix to filter by
      let (dir_to_list, prefix) = if input.ends_with(std::path::MAIN_SEPARATOR) {
        // Input ends with separator, list that directory
        (path.clone(), String::new())
      } else if path.is_dir() {
        // Input is an existing directory without trailing separator
        (path.clone(), String::new())
      } else {
        // Input is a partial path, get parent dir and filename prefix
        let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let file_prefix = path
          .file_name()
          .and_then(|n| n.to_str())
          .unwrap_or("")
          .to_string();
        (parent.to_path_buf(), file_prefix)
      };

      // Read directory entries
      let Ok(entries) = std::fs::read_dir(&dir_to_list) else {
        return Vec::new();
      };

      let mut items = Vec::new();
      let prefix_lower = prefix.to_lowercase();

      for entry in entries.flatten() {
        let entry_path = entry.path();
        let Some(file_name) = entry_path.file_name().and_then(|n| n.to_str()) else {
          continue;
        };

        // Filter by prefix (case-insensitive)
        if !prefix.is_empty() && !file_name.to_lowercase().starts_with(&prefix_lower) {
          continue;
        }

        // Skip hidden files unless prefix starts with '.'
        if file_name.starts_with('.') && !prefix.starts_with('.') {
          continue;
        }

        let is_dir = entry_path.is_dir();

        // Build the full completion path
        let completion_path = dir_to_list.join(file_name);
        let completion_text = if is_dir {
          format!("{}{}", completion_path.display(), std::path::MAIN_SEPARATOR)
        } else {
          completion_path.display().to_string()
        };

        // Calculate the range to replace (from the start of input)
        let range = 0..;

        items.push(Completion {
          range,
          text: completion_text,
          doc: Some(if is_dir {
            "directory".to_string()
          } else {
            "file".to_string()
          }),
        });
      }

      // Sort: directories first, then files, both alphabetically
      items.sort_by(|a, b| {
        let a_is_dir = a.text.ends_with(std::path::MAIN_SEPARATOR);
        let b_is_dir = b.text.ends_with(std::path::MAIN_SEPARATOR);
        match (a_is_dir, b_is_dir) {
          (true, false) => std::cmp::Ordering::Less,
          (false, true) => std::cmp::Ordering::Greater,
          _ => a.text.cmp(&b.text),
        }
      });

      items
    });

    let prompt = Prompt::new(String::new())
      .with_prefill(prefill)
      .with_completion(file_completion)
      .with_callback(move |cx, input, event| {
        match event {
          PromptEvent::Validate => {
            // Clear custom mode string
            cx.editor.clear_custom_mode_str();

            if input.is_empty() {
              return;
            }

            let path = the_editor_stdx::path::normalize(PathBuf::from(input));
            let is_folder = input.ends_with(std::path::MAIN_SEPARATOR);

            let result = if is_folder {
              std::fs::create_dir_all(&path).map_err(anyhow::Error::from)
            } else {
              // Create parent directories if needed
              if let Some(parent) = path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                  cx.editor
                    .set_error(format!("Failed to create parent directory: {}", e));
                  return;
                }
              }
              std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .map(|_| ())
                .map_err(anyhow::Error::from)
            };

            match &result {
              Ok(()) => {
                cx.editor.set_status(format!(
                  "Created {}",
                  if is_folder { "folder" } else { "file" }
                ));
              },
              Err(e) => {
                cx.editor.set_error(format!("Failed to create: {}", e));
              },
            }

            // Refresh the explorer tree after successful file operation
            if result.is_ok() {
              let path_to_reveal = path.clone();
              crate::ui::job::dispatch_blocking(move |_editor, compositor| {
                if let Some(editor_view) = compositor.find::<crate::ui::EditorView>() {
                  if let Some(explorer) = editor_view.explorer_mut() {
                    let _ = explorer.refresh_and_reveal(path_to_reveal);
                  }
                }
              });
            }
          },
          PromptEvent::Abort => {
            // Clear custom mode string on abort
            cx.editor.clear_custom_mode_str();
          },
          PromptEvent::Update => {},
        }
      });

    Ok(EventResult::Consumed(Some(Box::new(
      move |compositor, _cx| {
        // Find the statusline and trigger slide animation
        for layer in compositor.layers.iter_mut() {
          if let Some(statusline) = layer
            .as_any_mut()
            .downcast_mut::<crate::ui::components::statusline::StatusLine>()
          {
            statusline.slide_for_prompt(true);
            break;
          }
        }

        compositor.push(Box::new(prompt));
      },
    ))))
  }

  fn nearest_folder(&self) -> Result<PathBuf> {
    let current = self.tree.current()?.item();
    if current.is_parent() {
      Ok(current.path.to_path_buf())
    } else {
      let parent_path = current.path.parent().ok_or_else(|| {
        anyhow::anyhow!(format!(
          "Unable to get parent path of '{}'",
          current.path.to_string_lossy()
        ))
      })?;
      Ok(parent_path.to_path_buf())
    }
  }

  fn new_remove_prompt(&mut self, cx: &mut Context) -> Result<EventResult> {
    let item = self.tree.current()?.item();
    match item.file_type {
      FileType::Folder => self.new_remove_folder_prompt(cx),
      FileType::File => self.new_remove_file_prompt(cx),
      FileType::Root => bail!("Root is not removable"),
    }
  }

  fn new_rename_prompt(&mut self, cx: &mut Context) -> Result<EventResult> {
    use std::sync::Arc;

    use crate::ui::components::prompt::{
      Completion,
      PromptEvent,
    };

    let path = self.tree.current_item()?.path.clone();
    let prefill = path.to_string_lossy().to_string();
    let original_path = path.clone();

    // Set custom mode string for the statusline
    cx.editor.set_custom_mode_str("RENAME".to_string());

    // Set mode to Command so prompt keybindings work
    cx.editor.set_mode(crate::keymap::Mode::Command);

    // Create file path completion function
    let file_completion: Arc<
      dyn Fn(&crate::editor::Editor, &str) -> Vec<Completion> + Send + Sync,
    > = Arc::new(|_editor, input| {
      if input.is_empty() {
        return Vec::new();
      }

      let path = PathBuf::from(input);

      // Determine the directory to list and the prefix to filter by
      let (dir_to_list, prefix) = if input.ends_with(std::path::MAIN_SEPARATOR) {
        (path.clone(), String::new())
      } else if path.is_dir() {
        (path.clone(), String::new())
      } else {
        let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let file_prefix = path
          .file_name()
          .and_then(|n| n.to_str())
          .unwrap_or("")
          .to_string();
        (parent.to_path_buf(), file_prefix)
      };

      let Ok(entries) = std::fs::read_dir(&dir_to_list) else {
        return Vec::new();
      };

      let mut items = Vec::new();
      let prefix_lower = prefix.to_lowercase();

      for entry in entries.flatten() {
        let entry_path = entry.path();
        let Some(file_name) = entry_path.file_name().and_then(|n| n.to_str()) else {
          continue;
        };

        if !prefix.is_empty() && !file_name.to_lowercase().starts_with(&prefix_lower) {
          continue;
        }

        if file_name.starts_with('.') && !prefix.starts_with('.') {
          continue;
        }

        let is_dir = entry_path.is_dir();
        let completion_path = dir_to_list.join(file_name);
        let completion_text = if is_dir {
          format!("{}{}", completion_path.display(), std::path::MAIN_SEPARATOR)
        } else {
          completion_path.display().to_string()
        };

        items.push(Completion {
          range: 0..,
          text:  completion_text,
          doc:   Some(if is_dir { "directory" } else { "file" }.to_string()),
        });
      }

      items.sort_by(|a, b| {
        let a_is_dir = a.text.ends_with(std::path::MAIN_SEPARATOR);
        let b_is_dir = b.text.ends_with(std::path::MAIN_SEPARATOR);
        match (a_is_dir, b_is_dir) {
          (true, false) => std::cmp::Ordering::Less,
          (false, true) => std::cmp::Ordering::Greater,
          _ => a.text.cmp(&b.text),
        }
      });

      items
    });

    let prompt = Prompt::new(String::new())
      .with_prefill(prefill)
      .with_completion(file_completion)
      .with_callback(move |cx, input, event| {
        match event {
          PromptEvent::Validate => {
            // Clear custom mode string
            cx.editor.clear_custom_mode_str();

            if input.is_empty() {
              return;
            }

            let new_path = PathBuf::from(input);

            // Create parent directories if needed
            if let Some(parent) = new_path.parent() {
              if let Err(e) = std::fs::create_dir_all(parent) {
                cx.editor
                  .set_error(format!("Failed to create parent directory: {}", e));
                return;
              }
            }

            let rename_result = std::fs::rename(&original_path, &new_path);

            match &rename_result {
              Ok(()) => {
                cx.editor.set_status(format!(
                  "Renamed '{}' to '{}'",
                  original_path.display(),
                  new_path.display()
                ));
              },
              Err(e) => {
                cx.editor.set_error(format!("Failed to rename: {}", e));
              },
            }

            // Refresh the explorer tree after successful rename
            if rename_result.is_ok() {
              let path_to_reveal = new_path.clone();
              crate::ui::job::dispatch_blocking(move |_editor, compositor| {
                if let Some(editor_view) = compositor.find::<crate::ui::EditorView>() {
                  if let Some(explorer) = editor_view.explorer_mut() {
                    let _ = explorer.refresh_and_reveal(path_to_reveal);
                  }
                }
              });
            }
          },
          PromptEvent::Abort => {
            // Clear custom mode string on abort
            cx.editor.clear_custom_mode_str();
          },
          PromptEvent::Update => {},
        }
      });

    Ok(EventResult::Consumed(Some(Box::new(
      move |compositor, _cx| {
        // Find the statusline and trigger slide animation
        for layer in compositor.layers.iter_mut() {
          if let Some(statusline) = layer
            .as_any_mut()
            .downcast_mut::<crate::ui::components::statusline::StatusLine>()
          {
            statusline.slide_for_prompt(true);
            break;
          }
        }

        compositor.push(Box::new(prompt));
      },
    ))))
  }

  fn new_remove_file_prompt(&mut self, _cx: &mut Context) -> Result<EventResult> {
    use crate::ui::components::{
      ConfirmationButton,
      ConfirmationConfig,
      ConfirmationPopup,
      ConfirmationResult,
    };

    let item = self.tree.current_item()?;
    ensure!(
      item.path.is_file(),
      "The path '{}' is not a file",
      item.path.to_string_lossy()
    );

    let path = item.path.clone();
    let display_path = path.display().to_string();

    let config = ConfirmationConfig::new("delete-file", format!("Delete '{}'?", display_path))
      .with_buttons([
        ConfirmationButton::new("Delete", "y"),
        ConfirmationButton::new("Cancel", "n"),
      ]);

    let popup = ConfirmationPopup::new(config, move |cx, result| {
      match result {
        ConfirmationResult::Confirmed => {
          let delete_result = std::fs::remove_file(&path);
          match &delete_result {
            Ok(()) => {
              cx.editor
                .set_status(format!("Deleted file '{}'", path.display()));
            },
            Err(e) => {
              cx.editor.set_error(format!("Failed to delete file: {}", e));
            },
          }

          // Refresh the explorer tree after successful delete
          if delete_result.is_ok() {
            crate::ui::job::dispatch_blocking(move |_editor, compositor| {
              if let Some(editor_view) = compositor.find::<crate::ui::EditorView>() {
                if let Some(explorer) = editor_view.explorer_mut() {
                  let _ = explorer.refresh();
                }
              }
            });
          }
        },
        ConfirmationResult::Denied => {
          cx.editor.set_status("Delete cancelled");
        },
      }
    });

    Ok(EventResult::Consumed(Some(Box::new(
      move |compositor, _cx| {
        compositor.push(Box::new(popup));
      },
    ))))
  }

  fn new_remove_folder_prompt(&mut self, _cx: &mut Context) -> Result<EventResult> {
    use crate::ui::components::{
      ConfirmationButton,
      ConfirmationConfig,
      ConfirmationPopup,
      ConfirmationResult,
    };

    let item = self.tree.current_item()?;
    ensure!(
      item.path.is_dir(),
      "The path '{}' is not a folder",
      item.path.to_string_lossy()
    );

    let path = item.path.clone();
    let display_path = path.display().to_string();

    let config = ConfirmationConfig::new("delete-folder", format!("Delete '{}'?", display_path))
      .with_buttons([
        ConfirmationButton::new("Delete", "y"),
        ConfirmationButton::new("Cancel", "n"),
      ]);

    let popup = ConfirmationPopup::new(config, move |cx, result| {
      match result {
        ConfirmationResult::Confirmed => {
          let delete_result = std::fs::remove_dir_all(&path);
          match &delete_result {
            Ok(()) => {
              cx.editor
                .set_status(format!("Deleted folder '{}'", path.display()));
            },
            Err(e) => {
              cx.editor
                .set_error(format!("Failed to delete folder: {}", e));
            },
          }

          // Refresh the explorer tree after successful delete
          if delete_result.is_ok() {
            crate::ui::job::dispatch_blocking(move |_editor, compositor| {
              if let Some(editor_view) = compositor.find::<crate::ui::EditorView>() {
                if let Some(explorer) = editor_view.explorer_mut() {
                  let _ = explorer.refresh();
                }
              }
            });
          }
        },
        ConfirmationResult::Denied => {
          cx.editor.set_status("Delete cancelled");
        },
      }
    });

    Ok(EventResult::Consumed(Some(Box::new(
      move |compositor, _cx| {
        compositor.push(Box::new(popup));
      },
    ))))
  }

  fn toggle_current(item: &mut FileInfo, cx: &mut Context, state: &mut State) -> TreeOp {
    (|| -> Result<TreeOp> {
      if item.path == Path::new("") {
        return Ok(TreeOp::Noop);
      }
      let meta = std::fs::metadata(&item.path)?;
      if meta.is_file() {
        cx.editor.open(&item.path, Action::Replace)?;
        state.focus = false;
        return Ok(TreeOp::Noop);
      }

      if item.path.is_dir() {
        return Ok(TreeOp::GetChildsAndInsert);
      }

      Err(anyhow::anyhow!("Unknown file type: {:?}", meta.file_type()))
    })()
    .unwrap_or_else(|err| {
      cx.editor.set_error(format!("{err}"));
      TreeOp::Noop
    })
  }

  fn render_tree(
    &mut self,
    area: Rect,
    y_offset_px: f32,
    prompt_area: Rect,
    surface: &mut Surface,
    cx: &mut Context,
  ) {
    self.tree.render(area, y_offset_px, prompt_area, surface, cx);
  }

  /// Render the explorer as a sidebar
  ///
  /// # Arguments
  /// * `px_x` - X position in pixels
  /// * `px_y` - Y position in pixels
  /// * `px_width` - Width in pixels
  /// * `px_height` - Height in pixels (full viewport height)
  pub fn render(
    &mut self,
    px_x: f32,
    px_y: f32,
    px_width: f32,
    px_height: f32,
    surface: &mut Surface,
    cx: &mut Context,
  ) {
    use the_editor_renderer::{
      Color,
      TextSection,
    };

    use crate::ui::UI_FONT_SIZE;

    // Don't render if fully closed
    let close_alpha = self.closing_progress();
    if close_alpha <= 0.0 {
      return;
    }

    // Configure font to UI font size (independent of editor font size)
    let ui_font_family = surface.current_font_family().to_owned();
    surface.configure_font(&ui_font_family, UI_FONT_SIZE);

    let cell_width = surface.cell_width();

    // Get theme colors
    let theme = &cx.editor.theme;
    let bg_style = theme.get("ui.background");
    let mut bg_color = bg_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.1, 0.1, 0.15, 1.0));

    let text_style = theme.get("ui.text");
    let mut text_color = text_style
      .fg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::WHITE);

    let statusline_style = if self.is_focus() {
      theme.get("ui.statusline")
    } else {
      theme.get("ui.statusline.inactive")
    };
    let mut statusline_bg = statusline_style
      .bg
      .map(crate::ui::theme_color_to_renderer_color)
      .unwrap_or(Color::new(0.15, 0.15, 0.2, 1.0));

    // Apply closing animation alpha to colors
    bg_color.a *= close_alpha;
    text_color.a *= close_alpha;
    statusline_bg.a *= close_alpha;

    // Store width in cells for column_width tracking
    self.state.area_width = (px_width / cell_width).floor() as u16;

    // Draw background (full height)
    surface.draw_rect(px_x, px_y, px_width, px_height, bg_color);

    // Draw header/title bar
    let header_height = UI_FONT_SIZE + 8.0;
    surface.draw_rect(px_x, px_y, px_width, header_height, statusline_bg);

    // Draw title text
    let title = "EXPLORER";
    let title_color = if self.is_focus() {
      text_color
    } else {
      Color::new(
        text_color.r * 0.6,
        text_color.g * 0.6,
        text_color.b * 0.6,
        text_color.a,
      )
    };
    surface.draw_text(TextSection::simple(
      px_x + 8.0,
      px_y + 2.0,
      title,
      UI_FONT_SIZE,
      title_color,
    ));

    // Draw separator line below header
    let sep_y = px_y + header_height;
    let sep_color = Color::new(text_color.r, text_color.g, text_color.b, 0.2 * close_alpha);
    surface.draw_rect(px_x, sep_y, px_width, 1.0, sep_color);

    // Draw border separator on the edge facing the editor content
    let border_color = Color::new(text_color.r, text_color.g, text_color.b, 0.15 * close_alpha);
    let position = cx.editor.config().file_tree.position;
    let border_x = match position {
      crate::editor::FileTreePosition::Left => px_x + px_width - 1.0, // Right edge
      crate::editor::FileTreePosition::Right => px_x,                 // Left edge
    };
    surface.draw_rect(border_x, px_y, 1.0, px_height, border_color);

    // Calculate tree area
    // Tree starts below header separator
    let tree_start_y = sep_y + 1.0;
    let tree_height = px_height - header_height - 1.0;

    // The tree renders items with: item_height = UI_FONT_SIZE + 8.0, item_gap = 2.0
    // So each item takes UI_FONT_SIZE + 10.0 pixels total
    let tree_item_stride = UI_FONT_SIZE + 10.0;
    let tree_area = Rect::new(
      (px_x / cell_width).floor() as u16,
      0, // Y is now passed directly as pixels
      (px_width / cell_width).floor() as u16,
      // Calculate height as number of items that fit
      (tree_height / tree_item_stride).floor() as u16,
    );
    let prompt_area = Rect::new(
      tree_area.x,
      tree_area.y + tree_area.height,
      tree_area.width,
      1,
    );

    // Set tree global alpha for closing animation
    self.tree.set_global_alpha(close_alpha);
    self.render_tree(tree_area, tree_start_y, prompt_area, surface, cx);

    // Render search prompt if active
    if let Some(prompt) = self.tree.prompt() {
      let prompt_y = tree_start_y + tree_height - UI_FONT_SIZE - 8.0;
      let prompt_height = UI_FONT_SIZE + 8.0;

      // Draw prompt background
      let prompt_bg = Color::new(
        statusline_bg.r,
        statusline_bg.g,
        statusline_bg.b,
        statusline_bg.a.max(0.9),
      );
      surface.draw_rect(px_x, prompt_y, px_width, prompt_height, prompt_bg);

      // Draw separator above prompt
      surface.draw_rect(px_x, prompt_y, px_width, 1.0, sep_color);

      // Draw prompt text (prefix + input)
      let prompt_text = format!("{}{}", prompt.prefix(), prompt.input());
      surface.draw_text(TextSection::simple(
        px_x + 8.0,
        prompt_y + 4.0,
        &prompt_text,
        UI_FONT_SIZE,
        text_color,
      ));
    }
  }

  fn render_help(&mut self, _area: Rect, _surface: &mut Surface, _cx: &mut Context) {
    // TODO: Implement help rendering using the GPU renderer API
    // For now, help is shown in the status line when ? is pressed
    //
    // Help items:
    // "?", "Toggle help"
    // "a", "Add file/folder"
    // "r", "Rename file/folder"
    // "d", "Delete file"
    // "B", "Change root to parent folder"
    // "]", "Change root to current folder"
    // "[", "Go to previous root"
    // "+, =", "Increase size"
    // "-, _", "Decrease size"
    // "q", "Close"
    // Plus tree_view_help() items
  }

  /// Handle a mouse click on the explorer
  ///
  /// # Arguments
  /// * `visual_row` - The visual row index (0-based from top of tree area)
  /// * `double_click` - Whether this is a double-click
  pub fn handle_mouse_click(&mut self, visual_row: usize, double_click: bool, cx: &mut Context) {
    // Get the tree index for this visual row
    if let Some(tree_index) = self.tree.tree_index_at_row(visual_row) {
      // Select the item
      self.tree.select_by_tree_index(tree_index);

      // On double-click, activate the item (open file or toggle folder)
      if double_click {
        if let Err(err) = self.tree.on_enter(cx, &mut self.state, tree_index) {
          cx.editor.set_error(format!("{err}"));
        }
      }
    }
  }

  /// Get the number of visible items (for hover detection)
  pub fn visible_item_count(&self) -> usize {
    self.tree.visible_item_count()
  }

  /// Set the hovered visual row (for hover effects)
  pub fn set_hovered_row(&mut self, row: Option<usize>) {
    self.tree.set_hovered_row(row);
  }

  /// Set the mouse position for glow effects
  pub fn set_mouse_pos(&mut self, pos: Option<(f32, f32)>) {
    self.tree.set_mouse_pos(pos);
  }

  /// Scroll the tree view by delta lines (positive = down, negative = up)
  pub fn scroll(&mut self, delta: i32) {
    self.tree.scroll(delta);
  }

  fn toggle_help(&mut self) {
    self.show_help = !self.show_help
  }

  fn go_to_previous_root(&mut self) {
    if let Some(history) = self.history.pop() {
      self.tree = history.tree;
      self.state.current_root = history.current_root
    }
  }

  fn change_root_to_current_folder(&mut self) -> Result<()> {
    self.change_root(self.tree.current_item()?.path.clone())
  }

  fn change_root_parent_folder(&mut self) -> Result<()> {
    if let Some(parent) = self.state.current_root.parent() {
      let path = parent.to_path_buf();
      self.change_root(path)
    } else {
      Ok(())
    }
  }

  pub fn column_width(&self) -> u16 {
    self.column_width
  }

  fn increase_size(&mut self) {
    const EDITOR_MIN_WIDTH: u16 = 10;
    // If area_width hasn't been set yet (no render), allow unconstrained growth
    let max_width = if self.state.area_width == 0 {
      u16::MAX
    } else {
      self.state.area_width.saturating_sub(EDITOR_MIN_WIDTH)
    };
    self.column_width = std::cmp::min(max_width, self.column_width.saturating_add(1))
  }

  fn decrease_size(&mut self) {
    self.column_width = self.column_width.saturating_sub(1)
  }
}

impl Component for Explorer {
  /// Process input events, return true if handled.
  fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
    if self.tree.prompting() {
      return self.tree.handle_event(event, cx, &mut self.state);
    }
    let key_event = match event {
      Event::Key(event) => event,
      Event::Resize(..) => return EventResult::Consumed(None),
      _ => return EventResult::Ignored(None),
    };
    if !self.is_focus() {
      return EventResult::Ignored(None);
    }
    if let Some(mut on_next_key) = self.on_next_key.take() {
      return on_next_key(cx, self, key_event);
    }

    // Check for shifted keys
    if key_event.shift && !key_event.ctrl && !key_event.alt {
      match key_event.code {
        Key::Char('B') => {
          if let Err(err) = self.change_root_parent_folder() {
            cx.editor.set_error(format!("{err}"));
          }
          return EventResult::Consumed(None);
        },
        _ => {},
      }
    }

    // Ctrl+W - window prefix key (for consistency with editor)
    if key_event.ctrl && !key_event.alt && !key_event.shift {
      if let Key::Char('w') = key_event.code {
        // Enter pending state for window commands
        self.on_next_key = Some(Box::new(|_cx, explorer, key| {
          // Handle h/l for focus switching
          match key.code {
            Key::Char('h') | Key::Left => {
              // Move focus left - if explorer is on left, this is a no-op
              // If explorer is on right, unfocus to editor
              explorer.unfocus();
              EventResult::Consumed(None)
            },
            Key::Char('l') | Key::Right => {
              // Move focus right - if explorer is on left, unfocus to editor
              // If explorer is on right, this is a no-op
              explorer.unfocus();
              EventResult::Consumed(None)
            },
            Key::Char('j') | Key::Down | Key::Char('k') | Key::Up => {
              // Vertical movements - just unfocus to editor
              explorer.unfocus();
              EventResult::Consumed(None)
            },
            Key::Char('q') => {
              // Ctrl+W q - close explorer
              explorer.close();
              EventResult::Consumed(None)
            },
            _ => EventResult::Consumed(None),
          }
        }));
        return EventResult::Consumed(None);
      }
    }

    // Check for regular keys (no modifiers)
    if !key_event.ctrl && !key_event.alt && !key_event.shift {
      match key_event.code {
        Key::Escape => {
          self.unfocus();
          return EventResult::Consumed(None);
        },
        Key::Char('q') => {
          self.close();
          return EventResult::Consumed(None);
        },
        Key::Char('?') => {
          self.toggle_help();
          return EventResult::Consumed(None);
        },
        Key::Char('a') => {
          match self.new_create_file_or_folder_prompt(cx) {
            Ok(result) => return result,
            Err(err) => cx.editor.set_error(format!("{err}")),
          }
          return EventResult::Consumed(None);
        },
        Key::Char(']') => {
          if let Err(err) = self.change_root_to_current_folder() {
            cx.editor.set_error(format!("{err}"));
          }
          return EventResult::Consumed(None);
        },
        Key::Char('[') => {
          self.go_to_previous_root();
          return EventResult::Consumed(None);
        },
        Key::Char('d') => {
          match self.new_remove_prompt(cx) {
            Ok(result) => return result,
            Err(err) => cx.editor.set_error(format!("{err}")),
          }
          return EventResult::Consumed(None);
        },
        Key::Char('r') => {
          match self.new_rename_prompt(cx) {
            Ok(result) => return result,
            Err(err) => cx.editor.set_error(format!("{err}")),
          }
          return EventResult::Consumed(None);
        },
        Key::Char('-') | Key::Char('_') => {
          self.decrease_size();
          return EventResult::Consumed(None);
        },
        Key::Char('+') | Key::Char('=') => {
          self.increase_size();
          return EventResult::Consumed(None);
        },
        _ => {},
      }
    }

    // Pass to tree view
    self
      .tree
      .handle_event(&Event::Key(*key_event), cx, &mut self.state);
    EventResult::Consumed(None)
  }

  fn render(&mut self, _area: Rect, _surface: &mut Surface, _cx: &mut Context) {
    // Explorer is rendered directly by EditorView using the pixel-based render
    // method. This Component::render is kept for trait compliance but is
    // not used when the explorer is embedded as a sidebar.
  }

  fn cursor(&self, area: Rect, editor: &Editor) -> (Option<Position>, CursorKind) {
    // Check if the tree has a search prompt active
    if let Some(prompt) = self.tree.prompt() {
      let (x, y) = (area.x, area.y + area.height.saturating_sub(1));
      prompt.cursor(Rect::new(x, y, area.width, 1), editor)
    } else {
      (None, CursorKind::Hidden)
    }
  }
}

/// Simple border indicators for render_block
#[derive(Clone, Copy)]
enum Borders {
  LEFT,
  RIGHT,
}

fn render_block(area: Rect, _surface: &mut Surface, borders: Borders) -> Rect {
  // Compute inner area based on border side
  // We don't actually render a block widget, just calculate the usable area
  match borders {
    Borders::LEFT => {
      Rect {
        x: area.x.saturating_add(1),
        width: area.width.saturating_sub(1),
        ..area
      }
    },
    Borders::RIGHT => {
      Rect {
        width: area.width.saturating_sub(1),
        ..area
      }
    },
  }
}

#[cfg(test)]
mod test_explorer {
  use std::{
    fs,
    path::PathBuf,
  };

  use super::Explorer;
  use crate::core::graphics::Rect;

  /// This code should create the following file tree:
  ///
  ///   <temp_path>
  ///   ├── index.html
  ///   ├── .gitignore
  ///   ├── scripts
  ///   │   └── main.js
  ///   └── styles
  ///      ├── style.css
  ///      └── public
  ///          └── file
  fn dummy_file_tree() -> PathBuf {
    let path = tempfile::tempdir().unwrap().path().to_path_buf();
    if path.exists() {
      fs::remove_dir_all(path.clone()).unwrap();
    }
    fs::create_dir_all(path.clone()).unwrap();
    fs::write(path.join("index.html"), "").unwrap();
    fs::write(path.join(".gitignore"), "").unwrap();

    fs::create_dir_all(path.join("scripts")).unwrap();
    fs::write(path.join("scripts").join("main.js"), "").unwrap();

    fs::create_dir_all(path.join("styles")).unwrap();
    fs::write(path.join("styles").join("style.css"), "").unwrap();

    fs::create_dir_all(path.join("styles").join("public")).unwrap();
    fs::write(path.join("styles").join("public").join("file"), "").unwrap();

    path
  }

  fn render(explorer: &mut Explorer) -> String {
    explorer.tree.render_to_string(Rect::new(0, 0, 100, 10))
  }

  fn new_explorer() -> (PathBuf, Explorer) {
    let path = dummy_file_tree();
    (path.clone(), Explorer::from_path(path, 100).unwrap())
  }

  #[test]
  fn test_reveal_file() {
    let (path, mut explorer) = new_explorer();

    let path_str = path.display().to_string();

    // 0a. Expect the "scripts" folder is not opened
    assert_eq!(
      render(&mut explorer),
      format!(
        "
({path_str})
⏵ scripts
⏵ styles
  .gitignore
  index.html
"
      )
      .trim()
    );

    // 1. Reveal "scripts/main.js"
    explorer.reveal_file(path.join("scripts/main.js")).unwrap();

    // 1a. Expect the "scripts" folder is opened, and "main.js" is focused
    assert_eq!(
      render(&mut explorer),
      format!(
        "
[{path_str}]
⏷ [scripts]
    (main.js)
⏵ styles
  .gitignore
  index.html
"
      )
      .trim()
    );

    // 2. Change root to "scripts"
    explorer.tree.move_up(1);
    explorer.change_root_to_current_folder().unwrap();

    // 2a. Expect the current root is "scripts"
    assert_eq!(
      render(&mut explorer),
      format!(
        "
({path_str}/scripts)
  main.js
"
      )
      .trim()
    );

    // 3. Reveal "styles/public/file", which is outside of the current root
    explorer
      .reveal_file(path.join("styles/public/file"))
      .unwrap();

    // 3a. Expect the current root is "public", and "file" is focused
    assert_eq!(
      render(&mut explorer),
      format!(
        "
[{path_str}/styles/public]
  (file)
"
      )
      .trim()
    );
  }

  // NOTE: The following tests require `handle_events()` method which simulates
  // keyboard input. This was part of the original Helix implementation but
  // hasn't been ported yet. These tests are commented out until that
  // functionality is implemented.
  //
  // Tests that need `handle_events`:
  // - test_rename
  // - test_new_folder
  // - test_new_file
  // - test_remove_file
  // - test_remove_folder

  #[test]
  fn test_change_root() {
    let (path, mut explorer) = new_explorer();
    let path_str = path.display().to_string();

    // 1. Move cursor to "styles"
    explorer.reveal_file(path.join("styles")).unwrap();

    // 2. Change root to current folder, and move cursor down
    explorer.change_root_to_current_folder().unwrap();
    explorer.tree.move_down(1);

    // 2a. Expect the current root to be "styles", and the cursor is at "public"
    assert_eq!(
      render(&mut explorer),
      format!(
        "
[{path_str}/styles]
⏵ (public)
  style.css
"
      )
      .trim()
    );

    let current_root = explorer.state.current_root.clone();

    // 3. Change root to the parent of current folder
    explorer.change_root_parent_folder().unwrap();

    // 3a. Expect the current root to be "change_root"
    assert_eq!(
      render(&mut explorer),
      format!(
        "
({path_str})
⏵ scripts
⏵ styles
  .gitignore
  index.html
"
      )
      .trim()
    );

    // 4. Go back to previous root
    explorer.go_to_previous_root();

    // 4a. Expect the root te become "styles", and the cursor position is not
    // forgotten
    assert_eq!(
      render(&mut explorer),
      format!(
        "
[{path_str}/styles]
⏵ (public)
  style.css
"
      )
      .trim()
    );
    assert_eq!(explorer.state.current_root, current_root);

    // 5. Go back to previous root again
    explorer.go_to_previous_root();

    // 5a. Expect the current root to be "change_root" again,
    //     but this time the "styles" folder is opened,
    //     because it was opened before any change of root
    assert_eq!(
      render(&mut explorer),
      format!(
        "
[{path_str}]
⏵ scripts
⏷ (styles)
  ⏵ public
    style.css
  .gitignore
  index.html
"
      )
      .trim()
    );
  }

  #[test]
  fn test_focus_state() {
    let (_path, mut explorer) = new_explorer();

    // Initially, explorer should be focused (from new())
    assert!(
      explorer.is_focus(),
      "Explorer should be focused after creation"
    );
    assert!(
      explorer.is_opened(),
      "Explorer should be open after creation"
    );

    // Unfocus the explorer
    explorer.unfocus();
    assert!(
      !explorer.is_focus(),
      "Explorer should not be focused after unfocus()"
    );
    assert!(
      explorer.is_opened(),
      "Explorer should still be open after unfocus()"
    );

    // Focus the explorer again
    explorer.focus();
    assert!(
      explorer.is_focus(),
      "Explorer should be focused after focus()"
    );
    assert!(
      explorer.is_opened(),
      "Explorer should be open after focus()"
    );

    // Close the explorer (starts closing animation)
    explorer.close();
    assert!(
      !explorer.is_focus(),
      "Explorer should not be focused after close()"
    );
    assert!(
      !explorer.is_opened(),
      "Explorer should not be open (target is 0) after close()"
    );
    assert!(
      explorer.is_animating(),
      "Explorer should be animating after close()"
    );

    // Simulate animation completion
    explorer.update_closing(1.0); // Large dt to complete animation
    assert!(
      !explorer.is_opened(),
      "Explorer should not be open after animation completes"
    );
    assert!(
      explorer.closing_progress() == 0.0,
      "Explorer width should be 0 after close animation completes"
    );
  }

  #[test]
  fn test_column_width() {
    let (_path, mut explorer) = new_explorer();

    // Default column width should be reasonable
    let initial_width = explorer.column_width();
    assert!(initial_width > 0, "Column width should be positive");

    // Increase size
    let old_width = explorer.column_width();
    explorer.increase_size();
    assert!(
      explorer.column_width() > old_width || explorer.column_width() == old_width,
      "Column width should increase or stay same (if at max)"
    );

    // Decrease size
    let current_width = explorer.column_width();
    explorer.decrease_size();
    assert!(
      explorer.column_width() < current_width || explorer.column_width() == 0,
      "Column width should decrease"
    );
  }
}
