//! Shared watched-file reload state machine for runtime hosts.
//!
//! This keeps external-change conflict/reload behavior consistent between
//! editor clients while remaining independent from UI concerns.

/// State for the active watched file's external reload lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileWatchReloadState {
  /// Buffer and disk are in sync (or no pending external action exists).
  #[default]
  Clean,
  /// Disk differs and the buffer should reload from disk.
  ReloadNeeded,
  /// Disk differs while the buffer has unsaved local edits.
  Conflict,
}

/// Decision produced by [`decide_external_reload`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileWatchReloadDecision {
  /// No external action is currently required.
  Noop,
  /// Reload from disk should be attempted.
  ReloadNeeded,
  /// Conflict state has just been entered.
  ConflictEntered,
  /// Conflict is still active from a previous external change.
  ConflictOngoing,
}

/// Advance watched-file reload state after observing an external disk change.
///
/// - `buffer_modified`: whether the in-memory buffer currently has unsaved
///   local edits.
/// - `has_disk_changes`: whether disk content differs from the current buffer
///   snapshot (typically computed via a diff transaction).
pub fn decide_external_reload(
  state: &mut FileWatchReloadState,
  buffer_modified: bool,
  has_disk_changes: bool,
) -> FileWatchReloadDecision {
  if !has_disk_changes {
    *state = FileWatchReloadState::Clean;
    return FileWatchReloadDecision::Noop;
  }

  if buffer_modified {
    let decision = if matches!(state, FileWatchReloadState::Conflict) {
      FileWatchReloadDecision::ConflictOngoing
    } else {
      FileWatchReloadDecision::ConflictEntered
    };
    *state = FileWatchReloadState::Conflict;
    return decision;
  }

  *state = FileWatchReloadState::ReloadNeeded;
  FileWatchReloadDecision::ReloadNeeded
}

/// Mark a successful reload from disk.
pub fn mark_reload_applied(state: &mut FileWatchReloadState) {
  *state = FileWatchReloadState::Clean;
}

/// Clear state when a local save resolves prior conflict/reload state.
pub fn clear_reload_state(state: &mut FileWatchReloadState) {
  *state = FileWatchReloadState::Clean;
}

#[cfg(test)]
mod tests {
  use super::{
    FileWatchReloadDecision,
    FileWatchReloadState,
    clear_reload_state,
    decide_external_reload,
    mark_reload_applied,
  };

  #[test]
  fn no_disk_changes_resets_to_clean() {
    let mut state = FileWatchReloadState::Conflict;
    let decision = decide_external_reload(&mut state, true, false);
    assert_eq!(decision, FileWatchReloadDecision::Noop);
    assert_eq!(state, FileWatchReloadState::Clean);
  }

  #[test]
  fn clean_buffer_with_disk_changes_requests_reload() {
    let mut state = FileWatchReloadState::Clean;
    let decision = decide_external_reload(&mut state, false, true);
    assert_eq!(decision, FileWatchReloadDecision::ReloadNeeded);
    assert_eq!(state, FileWatchReloadState::ReloadNeeded);
  }

  #[test]
  fn dirty_buffer_enters_and_stays_in_conflict() {
    let mut state = FileWatchReloadState::Clean;
    let first = decide_external_reload(&mut state, true, true);
    let second = decide_external_reload(&mut state, true, true);

    assert_eq!(first, FileWatchReloadDecision::ConflictEntered);
    assert_eq!(second, FileWatchReloadDecision::ConflictOngoing);
    assert_eq!(state, FileWatchReloadState::Conflict);
  }

  #[test]
  fn reload_and_save_clear_state() {
    let mut state = FileWatchReloadState::ReloadNeeded;
    mark_reload_applied(&mut state);
    assert_eq!(state, FileWatchReloadState::Clean);

    state = FileWatchReloadState::Conflict;
    clear_reload_state(&mut state);
    assert_eq!(state, FileWatchReloadState::Clean);
  }
}
