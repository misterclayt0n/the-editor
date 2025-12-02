//! Auto-reveal handler for the Explorer component.
//!
//! This handler listens for document focus events and reveals the
//! current file in the explorer when `file_tree.auto-reveal` is enabled.

use std::path::PathBuf;

use the_editor_event::register_hook;

use crate::{
  event::{
    DocumentDidOpen,
    DocumentFocusGained,
  },
  ui::{
    EditorView,
    job,
  },
};

fn reveal_file_in_explorer(path: PathBuf) {
  job::dispatch_blocking(move |editor, compositor| {
    if let Some(editor_view) = compositor.find::<EditorView>()
      && let Some(explorer) = editor_view.explorer_mut()
    {
      if let Err(e) = explorer.reveal_file_quiet(path) {
        editor.set_error(format!("Auto-reveal failed: {}", e));
      }
    }
  });
}

/// Register hooks for auto-revealing files in the explorer.
///
/// When `file_tree.auto-reveal` is enabled in the config, this handler
/// will automatically expand folders and select the currently focused
/// file in the explorer sidebar, similar to VS Code's "Reveal in Side Bar"
/// or Zed's file tree sync behavior.
pub fn register_hooks() {
  // Handle document opening
  register_hook!(move |event: &mut DocumentDidOpen<'_>| {
    if !event.editor.config().file_tree.auto_reveal {
      return Ok(());
    }

    if let Some(path) = event
      .editor
      .documents
      .get(&event.doc)
      .and_then(|doc| doc.path().cloned())
    {
      reveal_file_in_explorer(path);
    }

    Ok(())
  });

  // Handle focus changes between documents
  register_hook!(move |event: &mut DocumentFocusGained<'_>| {
    if !event.editor.config().file_tree.auto_reveal {
      return Ok(());
    }

    if let Some(path) = event
      .editor
      .documents
      .get(&event.doc)
      .and_then(|doc| doc.path().cloned())
    {
      reveal_file_in_explorer(path);
    }

    Ok(())
  });
}
