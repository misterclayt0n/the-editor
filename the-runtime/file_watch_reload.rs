//! Shared watched-file reload state machine for runtime hosts.
//!
//! This keeps external-change conflict/reload behavior consistent between
//! editor clients while remaining independent from UI concerns.

use std::{
  path::Path,
  time::{
    Duration,
    Instant,
    SystemTime,
  },
};

use ropey::Rope;
use the_lib::diff::compare_ropes;

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

/// Cached disk fingerprint used for fast-path no-op evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileWatchDiskFingerprint {
  pub len:      u64,
  pub modified: Option<SystemTime>,
}

/// Mutable IO state for watched-file reload evaluation.
#[derive(Debug, Clone, Copy, Default)]
pub struct FileWatchReloadIoState {
  pub disk_fingerprint: Option<FileWatchDiskFingerprint>,
  pub read_failures:    u8,
  pub retry_after:      Option<Instant>,
}

/// Reload evaluation error.
#[derive(Debug)]
pub enum FileWatchReloadError {
  BackoffActive {
    retry_after: Instant,
  },
  ReadFailed {
    error:       std::io::Error,
    failures:    u8,
    retry_after: Instant,
  },
}

impl FileWatchReloadError {
  pub fn retry_after(&self) -> Instant {
    match self {
      Self::BackoffActive { retry_after } => *retry_after,
      Self::ReadFailed { retry_after, .. } => *retry_after,
    }
  }

  pub fn io_error(&self) -> Option<&std::io::Error> {
    match self {
      Self::ReadFailed { error, .. } => Some(error),
      Self::BackoffActive { .. } => None,
    }
  }
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

const READ_BACKOFF_BASE_MS: u64 = 80;
const READ_BACKOFF_MAX_MS: u64 = 2_000;

fn backoff_delay_for_failures(failures: u8) -> Duration {
  let shift = failures.saturating_sub(1).min(5) as u32;
  let factor = 1u64 << shift;
  Duration::from_millis((READ_BACKOFF_BASE_MS.saturating_mul(factor)).min(READ_BACKOFF_MAX_MS))
}

fn mark_read_failure(
  io_state: &mut FileWatchReloadIoState,
  now: Instant,
  error: std::io::Error,
) -> FileWatchReloadError {
  io_state.read_failures = io_state.read_failures.saturating_add(1);
  let retry_after = now + backoff_delay_for_failures(io_state.read_failures);
  io_state.retry_after = Some(retry_after);
  FileWatchReloadError::ReadFailed {
    error,
    failures: io_state.read_failures,
    retry_after,
  }
}

/// Evaluate reload/conflict decision by comparing current buffer text with
/// disk.
pub fn evaluate_external_reload_from_disk(
  state: &mut FileWatchReloadState,
  io_state: &mut FileWatchReloadIoState,
  watched_path: &Path,
  current_text: &Rope,
  buffer_modified: bool,
) -> Result<FileWatchReloadDecision, FileWatchReloadError> {
  let now = Instant::now();
  if let Some(retry_after) = io_state.retry_after
    && now < retry_after
  {
    return Err(FileWatchReloadError::BackoffActive { retry_after });
  }

  let metadata = match std::fs::metadata(watched_path) {
    Ok(metadata) => metadata,
    Err(err) => return Err(mark_read_failure(io_state, now, err)),
  };

  let fingerprint = FileWatchDiskFingerprint {
    len:      metadata.len(),
    modified: metadata.modified().ok(),
  };
  if io_state.disk_fingerprint == Some(fingerprint) {
    return Ok(FileWatchReloadDecision::Noop);
  }

  let disk_text = match std::fs::read_to_string(watched_path) {
    Ok(text) => text,
    Err(err) => return Err(mark_read_failure(io_state, now, err)),
  };
  io_state.disk_fingerprint = Some(fingerprint);
  io_state.read_failures = 0;
  io_state.retry_after = None;

  let disk_rope = Rope::from_str(&disk_text);
  let has_disk_changes = !compare_ropes(current_text, &disk_rope).changes().is_empty();
  Ok(decide_external_reload(
    state,
    buffer_modified,
    has_disk_changes,
  ))
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
  use std::{
    fs,
    path::{
      Path,
      PathBuf,
    },
    time::{
      Duration,
      SystemTime,
    },
  };

  use ropey::Rope;

  use super::{
    FileWatchReloadDecision,
    FileWatchReloadError,
    FileWatchReloadIoState,
    FileWatchReloadState,
    READ_BACKOFF_BASE_MS,
    backoff_delay_for_failures,
    clear_reload_state,
    decide_external_reload,
    evaluate_external_reload_from_disk,
    mark_reload_applied,
  };

  fn temp_path(prefix: &str) -> PathBuf {
    let nonce = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .map(|d| d.as_nanos())
      .unwrap_or(0);
    std::env::temp_dir().join(format!(
      "the-editor-file-watch-reload-{prefix}-{}-{nonce}.txt",
      std::process::id()
    ))
  }

  fn write_file(path: &Path, text: &str) {
    fs::write(path, text).expect("write temp file");
  }

  fn remove_file(path: &Path) {
    let _ = fs::remove_file(path);
  }

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

  #[test]
  fn evaluate_external_reload_handles_clean_and_conflict_flows() {
    let path = temp_path("evaluate");
    write_file(&path, "alpha\n");
    let mut state = FileWatchReloadState::Clean;
    let mut io_state = FileWatchReloadIoState::default();

    let same = evaluate_external_reload_from_disk(
      &mut state,
      &mut io_state,
      &path,
      &Rope::from_str("alpha\n"),
      false,
    )
    .expect("evaluate same");
    assert_eq!(same, FileWatchReloadDecision::Noop);
    assert_eq!(state, FileWatchReloadState::Clean);

    write_file(&path, "beta\n");
    let reload = evaluate_external_reload_from_disk(
      &mut state,
      &mut io_state,
      &path,
      &Rope::from_str("alpha\n"),
      false,
    )
    .expect("evaluate reload");
    assert_eq!(reload, FileWatchReloadDecision::ReloadNeeded);
    assert_eq!(state, FileWatchReloadState::ReloadNeeded);

    write_file(&path, "gamma\n");
    let conflict = evaluate_external_reload_from_disk(
      &mut state,
      &mut io_state,
      &path,
      &Rope::from_str("alpha\n"),
      true,
    )
    .expect("evaluate conflict");
    assert_eq!(conflict, FileWatchReloadDecision::ConflictEntered);
    assert_eq!(state, FileWatchReloadState::Conflict);

    remove_file(&path);
  }

  #[test]
  fn evaluate_external_reload_reports_read_error() {
    let path = temp_path("missing");
    remove_file(&path);

    let mut state = FileWatchReloadState::Clean;
    let mut io_state = FileWatchReloadIoState::default();
    let err = evaluate_external_reload_from_disk(
      &mut state,
      &mut io_state,
      &path,
      &Rope::from_str("alpha\n"),
      false,
    )
    .expect_err("missing file should error");
    match err {
      FileWatchReloadError::ReadFailed { error, .. } => {
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
      },
      other => panic!("expected read failure, got {other:?}"),
    }
  }

  #[test]
  fn evaluate_external_reload_uses_metadata_fast_path() {
    let path = temp_path("metadata-fast-path");
    write_file(&path, "alpha\n");
    let mut state = FileWatchReloadState::Clean;
    let mut io_state = FileWatchReloadIoState::default();

    let first = evaluate_external_reload_from_disk(
      &mut state,
      &mut io_state,
      &path,
      &Rope::from_str("alpha\n"),
      false,
    )
    .expect("first evaluate");
    assert_eq!(first, FileWatchReloadDecision::Noop);
    assert!(io_state.disk_fingerprint.is_some());

    let second = evaluate_external_reload_from_disk(
      &mut state,
      &mut io_state,
      &path,
      &Rope::from_str("alpha\n"),
      false,
    )
    .expect("second evaluate");
    assert_eq!(second, FileWatchReloadDecision::Noop);

    remove_file(&path);
  }

  #[test]
  fn evaluate_external_reload_applies_read_backoff_window() {
    let path = temp_path("read-backoff");
    remove_file(&path);
    let mut state = FileWatchReloadState::Clean;
    let mut io_state = FileWatchReloadIoState::default();

    let first = evaluate_external_reload_from_disk(
      &mut state,
      &mut io_state,
      &path,
      &Rope::from_str("alpha\n"),
      false,
    )
    .expect_err("first read should fail");
    let retry_after = first.retry_after();

    let second = evaluate_external_reload_from_disk(
      &mut state,
      &mut io_state,
      &path,
      &Rope::from_str("alpha\n"),
      false,
    )
    .expect_err("second call during window should back off");
    match second {
      FileWatchReloadError::BackoffActive {
        retry_after: second_retry,
      } => {
        assert_eq!(retry_after, second_retry);
      },
      other => panic!("expected backoff active, got {other:?}"),
    }

    let delay = backoff_delay_for_failures(1);
    assert!(delay >= Duration::from_millis(READ_BACKOFF_BASE_MS));
  }
}
