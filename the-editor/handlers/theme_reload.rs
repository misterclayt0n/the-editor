//! Theme hot reload handler.
//!
//! This handler listens for `FileSystemDidChange` events and automatically
//! reloads the current theme when its file is modified.

use notify::event::EventKind;
use the_editor_event::register_hook;

use crate::{
  core::file_watcher::FileSystemDidChange,
  ui::job,
};

pub(crate) fn register_hooks() {
  register_hook!(move |event: &mut FileSystemDidChange| {
    on_file_did_change(event);
    Ok(())
  });
}

fn on_file_did_change(event: &mut FileSystemDidChange) {
  // Only process modify events
  let fs_events = event.fs_events.clone();
  if !fs_events
    .iter()
    .any(|evt| matches!(evt.kind, EventKind::Modify(_)))
  {
    return;
  }

  job::dispatch_blocking(move |editor, _| {
    // Get the current theme path
    let Some(theme_path) = editor.theme_path.clone() else {
      return;
    };

    // Check if any of the changed paths match the current theme file
    let theme_changed = fs_events.iter().any(|fs_event| {
      matches!(fs_event.kind, EventKind::Modify(_))
        && fs_event.paths.iter().any(|path| path == &theme_path)
    });

    if !theme_changed {
      return;
    }

    // Extract theme name from the file path (e.g., "naysayer.toml" -> "naysayer")
    let Some(theme_name) = theme_path.file_stem().and_then(|s| s.to_str()) else {
      return;
    };

    // Reload the theme
    match editor.theme_loader.load(theme_name) {
      Ok(new_theme) => {
        editor.set_theme(new_theme);
        editor.set_status(format!("Theme '{}' reloaded", theme_name));
      },
      Err(err) => {
        editor.set_error(format!("Failed to reload theme '{}': {}", theme_name, err));
      },
    }
  });
}
