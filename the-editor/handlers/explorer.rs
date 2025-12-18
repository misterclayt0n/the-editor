//! File system change handler for the Explorer component.
//!
//! This handler listens for `FileSystemDidChange` events and triggers a refresh
//! of the explorer's tree structure and git status cache when relevant file
//! changes occur. This enables automatic updates when files are created or
//! deleted by external processes.

use notify::event::EventKind;
use the_editor_event::register_hook;

use crate::{
  core::file_watcher::FileSystemDidChange,
  ui::{
    EditorView,
    job,
  },
};

/// Register hooks for explorer file watching.
///
/// This sets up a listener for file system changes that will refresh the
/// explorer's git status when files are modified, created, or deleted.
pub fn register_hooks() {
  register_hook!(move |event: &mut FileSystemDidChange| {
    // Check if any of the events are relevant for git status updates
    // We care about modifications, creates, removes, and renames
    let has_relevant_changes = event.fs_events.iter().any(|evt| {
      matches!(
        evt.kind,
        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) | EventKind::Other
      )
    });

    if !has_relevant_changes {
      return Ok(());
    }

    // Dispatch a job to refresh the explorer's tree and git status
    // We use dispatch_blocking since this is called from the file watcher callback
    job::dispatch_blocking(move |editor, compositor| {
      // Find the EditorView component and refresh its explorer
      if let Some(editor_view) = compositor.find::<EditorView>()
        && let Some(explorer) = editor_view.explorer_mut()
      {
        // Refresh tree structure first (for new/deleted files)
        if let Err(e) = explorer.refresh() {
          log::warn!("Failed to refresh explorer tree: {}", e);
        }
        // Then refresh git status (spawns a background task)
        explorer.refresh_git_status_with_providers(&editor.diff_providers);
      }
    });

    Ok(())
  });
}
