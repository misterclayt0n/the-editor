//! Filesystem watcher helpers for runtime hosts.
//!
//! This module provides a debounced, batched watcher stream around `notify`.
//! It is intentionally side-effectful and belongs in runtime/app layers.

use std::{
  collections::{
    BTreeMap,
    HashMap,
  },
  fs::OpenOptions,
  io::{
    BufWriter,
    Write,
  },
  path::{
    Path,
    PathBuf,
  },
  sync::{
    Arc,
    Mutex,
    OnceLock,
    atomic::{
      AtomicBool,
      Ordering,
    },
    mpsc,
  },
  thread,
  time::{
    Duration,
    SystemTime,
  },
};

use notify::{
  EventKind,
  RecursiveMode,
  Watcher as _,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PathEventKind {
  Removed,
  Created,
  Changed,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathEvent {
  pub path: PathBuf,
  pub kind: PathEventKind,
}

const FILE_WATCH_TRACE_ENV: &str = "THE_EDITOR_FILE_WATCH_TRACE_LOG";
const FILE_WATCH_TRACE_DEFAULT: &str = "/tmp/the-editor-file-watch.log";
static WATCH_TRACE_WRITER: OnceLock<Option<Mutex<BufWriter<std::fs::File>>>> = OnceLock::new();

/// Resolve the file-watcher trace log path.
///
/// Set `THE_EDITOR_FILE_WATCH_TRACE_LOG=off` (or `none`) to disable.
pub fn resolve_trace_log_path() -> Option<PathBuf> {
  match std::env::var(FILE_WATCH_TRACE_ENV) {
    Ok(path) => {
      let path = path.trim();
      if path.is_empty() || path.eq_ignore_ascii_case("off") || path.eq_ignore_ascii_case("none") {
        None
      } else {
        Some(PathBuf::from(path))
      }
    },
    Err(_) => Some(PathBuf::from(FILE_WATCH_TRACE_DEFAULT)),
  }
}

fn trace_writer() -> Option<&'static Mutex<BufWriter<std::fs::File>>> {
  WATCH_TRACE_WRITER
    .get_or_init(|| {
      let path = resolve_trace_log_path()?;
      if let Some(parent) = path.parent()
        && let Err(err) = std::fs::create_dir_all(parent)
      {
        eprintln!(
          "Warning: failed to create file-watch trace directory '{}': {err}",
          parent.display()
        );
        return None;
      }

      match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => Some(Mutex::new(BufWriter::new(file))),
        Err(err) => {
          eprintln!(
            "Warning: failed to open file-watch trace log '{}': {err}",
            path.display()
          );
          None
        },
      }
    })
    .as_ref()
}

fn file_watch_trace(event: &str, message: impl Into<String>) {
  let message = message.into();
  tracing::trace!(target: "the_runtime::file_watch", event = event, %message);

  let Some(writer) = trace_writer() else {
    return;
  };

  let ts_ms = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .map(|d| d.as_millis() as u64)
    .unwrap_or(0);

  let mut writer = lock(writer);
  let _ = writeln!(writer, "{ts_ms} [{event}] {message}");
  let _ = writer.flush();
}

/// Handle for a logical watcher stream created by [`watch`].
///
/// Dropping the handle unregisters all watched paths and stops background
/// dispatch for this stream.
pub struct WatchHandle {
  registrations:  Mutex<HashMap<PathBuf, Vec<WatcherRegistrationId>>>,
  pending_events: Arc<Mutex<Vec<PathEvent>>>,
  wake_tx:        mpsc::Sender<()>,
  shutdown:       Arc<AtomicBool>,
}

impl WatchHandle {
  /// Add a path to this watcher stream.
  ///
  /// If the exact path cannot be watched (for example if it does not exist
  /// yet), this falls back to watching the parent directory and filtering
  /// events to the requested path prefix.
  pub fn add(&self, path: &Path) -> notify::Result<()> {
    let logical_path = normalize_path(path);
    file_watch_trace("watch_add_begin", format!("path={}", logical_path.display()));
    {
      let registrations = lock(&self.registrations);
      if registrations.contains_key(&logical_path) {
        file_watch_trace(
          "watch_add_skip",
          format!("already_registered path={}", logical_path.display()),
        );
        return Ok(());
      }
    }

    let mut registration_ids = Vec::new();

    match self.register(logical_path.clone(), logical_path.clone()) {
      Ok(id) => {
        file_watch_trace(
          "watch_add_direct",
          format!(
            "path={} registration_id={}",
            logical_path.display(),
            id.0
          ),
        );
        registration_ids.push(id);
      },
      Err(err) => {
        let Some(parent) = logical_path.parent() else {
          file_watch_trace(
            "watch_add_error",
            format!(
              "path={} reason=no_parent err={err}",
              logical_path.display()
            ),
          );
          return Err(err);
        };
        let parent = normalize_path(parent);
        file_watch_trace(
          "watch_add_fallback_parent",
          format!(
            "path={} parent={} err={err}",
            logical_path.display(),
            parent.display()
          ),
        );
        let id = self.register(parent, logical_path.clone())?;
        registration_ids.push(id);
      },
    }

    if let Some(target) = resolve_symlink_target(&logical_path) {
      file_watch_trace(
        "watch_add_symlink_target",
        format!(
          "path={} target={}",
          logical_path.display(),
          target.display()
        ),
      );
      if let Ok(id) = self.register(target.clone(), target.clone()) {
        file_watch_trace(
          "watch_add_symlink_target_registered",
          format!(
            "path={} target={} registration_id={}",
            logical_path.display(),
            target.display(),
            id.0
          ),
        );
        registration_ids.push(id);
      }
      if let Some(parent) = target.parent() {
        let parent = normalize_path(parent);
        if let Ok(id) = self.register(parent, target) {
          file_watch_trace(
            "watch_add_symlink_parent_registered",
            format!(
              "path={} registration_id={}",
              logical_path.display(),
              id.0
            ),
          );
          registration_ids.push(id);
        }
      }
    }

    let registered_count = registration_ids.len();
    lock(&self.registrations).insert(logical_path, registration_ids);
    file_watch_trace(
      "watch_add_done",
      format!("registrations={registered_count}"),
    );
    Ok(())
  }

  /// Remove a previously added logical path.
  pub fn remove(&self, path: &Path) -> notify::Result<()> {
    let logical_path = normalize_path(path);
    file_watch_trace("watch_remove_begin", format!("path={}", logical_path.display()));
    let Some(registrations) = lock(&self.registrations).remove(&logical_path) else {
      file_watch_trace(
        "watch_remove_skip",
        format!("path={} reason=not_registered", logical_path.display()),
      );
      return Ok(());
    };

    for registration in registrations {
      global(|watcher| watcher.remove(registration))?;
    }

    file_watch_trace("watch_remove_done", format!("path={}", logical_path.display()));
    Ok(())
  }

  fn register(
    &self,
    watch_path: PathBuf,
    filter_path: PathBuf,
  ) -> notify::Result<WatcherRegistrationId> {
    let pending_events = Arc::clone(&self.pending_events);
    let wake_tx = self.wake_tx.clone();
    let shutdown = Arc::clone(&self.shutdown);
    let filter_path = normalize_path(&filter_path);
    let watch_path_display = watch_path.display().to_string();
    let filter_path_display = filter_path.display().to_string();
    file_watch_trace(
      "register_begin",
      format!("watch_path={watch_path_display} filter_path={filter_path_display}"),
    );

    let callback_watch_path = watch_path_display.clone();
    let callback_filter_path = filter_path_display.clone();
    let registration = global(|watcher| {
      watcher.add(watch_path, watch_mode(), move |event: &notify::Event| {
        if shutdown.load(Ordering::Relaxed) {
          file_watch_trace(
            "register_callback_skip",
            format!("watch_path={callback_watch_path} reason=shutdown"),
          );
          return;
        }

        let Some(kind) = path_event_kind(&event.kind) else {
          return;
        };

        let mut mapped = Vec::new();
        for path in &event.paths {
          let event_path = normalize_path(path);
          if path_matches_filter(&event_path, &filter_path) {
            mapped.push(PathEvent {
              path: event_path,
              kind,
            });
          }
        }

        if mapped.is_empty() {
          file_watch_trace(
            "register_callback_filtered_out",
            format!(
              "watch_path={callback_watch_path} filter_path={callback_filter_path} event_paths={}",
              event.paths.len()
            ),
          );
          return;
        }

        let mapped_count = mapped.len();
        let mut pending = lock(&pending_events);
        let should_wake = pending.is_empty();
        pending.extend(mapped);
        drop(pending);

        if should_wake {
          if wake_tx.send(()).is_err() {
            file_watch_trace(
              "register_callback_wake_failed",
              format!("watch_path={callback_watch_path}"),
            );
          }
        }
        file_watch_trace(
          "register_callback_mapped",
          format!(
            "watch_path={callback_watch_path} filter_path={callback_filter_path} kind={kind:?} mapped={mapped_count} wake={should_wake}"
          ),
        );
      })
    })??;

    file_watch_trace(
      "register_done",
      format!(
        "watch_path={watch_path_display} filter_path={filter_path_display} registration_id={}",
        registration.0
      ),
    );
    Ok(registration)
  }
}

impl Drop for WatchHandle {
  fn drop(&mut self) {
    file_watch_trace("watch_drop_begin", "dropping watch handle");
    self.shutdown.store(true, Ordering::Relaxed);
    let _ = self.wake_tx.send(());

    let registrations = {
      let mut registrations = lock(&self.registrations);
      std::mem::take(&mut *registrations)
    };
    for ids in registrations.into_values() {
      for id in ids {
        let _ = global(|watcher| watcher.remove(id));
      }
    }
    file_watch_trace("watch_drop_done", "watch handle dropped");
  }
}

/// Create a debounced watcher stream for `path`.
///
/// The returned receiver yields batches of coalesced path events.
pub fn watch(path: &Path, latency: Duration) -> (mpsc::Receiver<Vec<PathEvent>>, WatchHandle) {
  file_watch_trace(
    "watch_stream_create",
    format!("path={} latency_ms={}", path.display(), latency.as_millis()),
  );
  let (events_tx, events_rx) = mpsc::channel();
  let (wake_tx, wake_rx) = mpsc::channel();
  let pending_events: Arc<Mutex<Vec<PathEvent>>> = Arc::new(Mutex::new(Vec::new()));
  let shutdown = Arc::new(AtomicBool::new(false));

  spawn_dispatch_thread(
    latency,
    Arc::clone(&pending_events),
    wake_rx,
    events_tx,
    Arc::clone(&shutdown),
  );

  let handle = WatchHandle {
    registrations: Mutex::new(HashMap::new()),
    pending_events,
    wake_tx,
    shutdown,
  };

  if let Err(err) = handle.add(path) {
    file_watch_trace(
      "watch_stream_add_error",
      format!("path={} err={err}", path.display()),
    );
  }
  (events_rx, handle)
}

fn spawn_dispatch_thread(
  latency: Duration,
  pending_events: Arc<Mutex<Vec<PathEvent>>>,
  wake_rx: mpsc::Receiver<()>,
  events_tx: mpsc::Sender<Vec<PathEvent>>,
  shutdown: Arc<AtomicBool>,
) {
  thread::spawn(move || {
    file_watch_trace(
      "dispatch_thread_start",
      format!("latency_ms={}", latency.as_millis()),
    );
    while wake_rx.recv().is_ok() {
      if shutdown.load(Ordering::Relaxed) {
        file_watch_trace("dispatch_thread_shutdown", "shutdown requested");
        break;
      }

      if !latency.is_zero() {
        thread::sleep(latency);
      }

      while wake_rx.try_recv().is_ok() {}

      let pending = {
        let mut pending = lock(&pending_events);
        if pending.is_empty() {
          file_watch_trace("dispatch_thread_empty", "woken_with_no_pending_events");
          continue;
        }
        std::mem::take(&mut *pending)
      };
      let batch = coalesce_events(pending);
      if batch.is_empty() {
        file_watch_trace("dispatch_thread_coalesce_empty", "coalesced_batch_empty");
        continue;
      }
      file_watch_trace(
        "dispatch_thread_batch",
        format!("batch_size={}", batch.len()),
      );

      if events_tx.send(batch).is_err() {
        shutdown.store(true, Ordering::Relaxed);
        file_watch_trace("dispatch_thread_send_failed", "receiver disconnected");
        break;
      }
    }
    file_watch_trace("dispatch_thread_exit", "dispatch loop exited");
  });
}

fn normalize_path(path: &Path) -> PathBuf {
  if path.is_absolute() {
    return path.to_path_buf();
  }
  match std::env::current_dir() {
    Ok(cwd) => cwd.join(path),
    Err(_) => path.to_path_buf(),
  }
}

fn resolve_symlink_target(path: &Path) -> Option<PathBuf> {
  let mut target = std::fs::read_link(path).ok()?;
  if target.is_relative()
    && let Some(parent) = path.parent()
  {
    target = parent.join(target);
  }
  if let Ok(canonical_target) = std::fs::canonicalize(&target) {
    target = canonical_target;
  }
  Some(normalize_path(&target))
}

fn path_matches_filter(event_path: &Path, filter_path: &Path) -> bool {
  event_path == filter_path || event_path.starts_with(filter_path)
}

fn path_event_kind(kind: &EventKind) -> Option<PathEventKind> {
  match kind {
    EventKind::Create(_) => Some(PathEventKind::Created),
    EventKind::Modify(_) => Some(PathEventKind::Changed),
    EventKind::Remove(_) => Some(PathEventKind::Removed),
    _ => None,
  }
}

fn coalesce_events(events: Vec<PathEvent>) -> Vec<PathEvent> {
  let mut coalesced = BTreeMap::new();
  for event in events {
    coalesced.insert(event.path, event.kind);
  }
  coalesced
    .into_iter()
    .map(|(path, kind)| PathEvent { path, kind })
    .collect()
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn watch_mode() -> RecursiveMode {
  RecursiveMode::Recursive
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn watch_mode() -> RecursiveMode {
  RecursiveMode::NonRecursive
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
struct WatcherRegistrationId(u64);

struct WatcherRegistrationState {
  callback: Arc<dyn Fn(&notify::Event) + Send + Sync>,
  path:     PathBuf,
}

#[derive(Default)]
struct WatcherState {
  watchers:             HashMap<WatcherRegistrationId, WatcherRegistrationState>,
  path_registrations:   HashMap<PathBuf, u32>,
  next_registration_id: WatcherRegistrationId,
}

struct GlobalWatcher {
  state:   Mutex<WatcherState>,
  watcher: Mutex<notify::RecommendedWatcher>,
}

impl GlobalWatcher {
  fn add(
    &self,
    path: PathBuf,
    mode: RecursiveMode,
    cb: impl Fn(&notify::Event) + Send + Sync + 'static,
  ) -> notify::Result<WatcherRegistrationId> {
    file_watch_trace(
      "global_add_begin",
      format!("path={} mode={mode:?}", path.display()),
    );
    let mut state = lock(&self.state);

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let path_already_covered = state
      .path_registrations
      .keys()
      .any(|existing| path.starts_with(existing) && path != *existing);

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let path_already_covered = false;

    if !path_already_covered && !state.path_registrations.contains_key(&path) {
      drop(state);
      lock(&self.watcher).watch(&path, mode)?;
      state = lock(&self.state);
      file_watch_trace(
        "global_add_os_watch",
        format!("path={} mode={mode:?}", path.display()),
      );
    }

    let id = state.next_registration_id;
    state.next_registration_id = WatcherRegistrationId(id.0 + 1);
    state.watchers.insert(id, WatcherRegistrationState {
      callback: Arc::new(cb),
      path:     path.clone(),
    });
    *state.path_registrations.entry(path).or_insert(0) += 1;
    file_watch_trace("global_add_done", format!("registration_id={}", id.0));
    Ok(id)
  }

  fn remove(&self, id: WatcherRegistrationId) {
    file_watch_trace("global_remove_begin", format!("registration_id={}", id.0));
    let mut state = lock(&self.state);
    let Some(registration_state) = state.watchers.remove(&id) else {
      file_watch_trace(
        "global_remove_skip",
        format!("registration_id={} reason=not_found", id.0),
      );
      return;
    };

    let Some(count) = state.path_registrations.get_mut(&registration_state.path) else {
      file_watch_trace(
        "global_remove_skip",
        format!(
          "registration_id={} reason=missing_path path={}",
          id.0,
          registration_state.path.display()
        ),
      );
      return;
    };
    *count -= 1;
    if *count == 0 {
      state.path_registrations.remove(&registration_state.path);

      drop(state);
      let _ = lock(&self.watcher).unwatch(&registration_state.path);
      file_watch_trace(
        "global_remove_os_unwatch",
        format!("path={}", registration_state.path.display()),
      );
      return;
    }
    file_watch_trace(
      "global_remove_done",
      format!(
        "registration_id={} remaining_path_refs={}",
        id.0,
        count
      ),
    );
  }
}

static GLOBAL_WATCHER: OnceLock<GlobalWatcher> = OnceLock::new();

fn global<T>(f: impl FnOnce(&GlobalWatcher) -> T) -> notify::Result<T> {
  if let Some(global) = GLOBAL_WATCHER.get() {
    return Ok(f(global));
  }

  file_watch_trace("global_init_begin", "creating global notify watcher");
  let watcher = notify::recommended_watcher(handle_event)?;
  let global_watcher = GlobalWatcher {
    state:   Mutex::new(WatcherState::default()),
    watcher: Mutex::new(watcher),
  };
  let _ = GLOBAL_WATCHER.set(global_watcher);
  file_watch_trace("global_init_done", "global notify watcher ready");

  let global = GLOBAL_WATCHER
    .get()
    .expect("global watcher should be initialized");
  Ok(f(global))
}

fn handle_event(event: notify::Result<notify::Event>) {
  let Ok(event) = event else {
    file_watch_trace("notify_event_error", "notify returned error event");
    return;
  };
  if matches!(event.kind, EventKind::Access(_)) {
    return;
  }
  file_watch_trace(
    "notify_event",
    format!("kind={:?} paths={}", event.kind, event.paths.len()),
  );

  let callbacks = match global(|watcher| {
    let state = lock(&watcher.state);
    state
      .watchers
      .values()
      .map(|registration| Arc::clone(&registration.callback))
      .collect::<Vec<_>>()
  }) {
    Ok(callbacks) => callbacks,
    Err(err) => {
      file_watch_trace(
        "notify_event_callbacks_error",
        format!("failed_to_get_callbacks err={err}"),
      );
      return;
    },
  };
  file_watch_trace(
    "notify_event_callbacks",
    format!("count={}", callbacks.len()),
  );

  for callback in callbacks {
    callback(&event);
  }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
  mutex
    .lock()
    .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod test {
  use std::path::PathBuf;

  use super::{
    PathEvent,
    PathEventKind,
    coalesce_events,
    path_matches_filter,
  };

  #[test]
  fn coalesce_events_keeps_last_kind_and_sorts_by_path() {
    let events = vec![
      PathEvent {
        path: PathBuf::from("b.rs"),
        kind: PathEventKind::Changed,
      },
      PathEvent {
        path: PathBuf::from("a.rs"),
        kind: PathEventKind::Created,
      },
      PathEvent {
        path: PathBuf::from("b.rs"),
        kind: PathEventKind::Removed,
      },
    ];

    let coalesced = coalesce_events(events);
    assert_eq!(coalesced, vec![
      PathEvent {
        path: PathBuf::from("a.rs"),
        kind: PathEventKind::Created,
      },
      PathEvent {
        path: PathBuf::from("b.rs"),
        kind: PathEventKind::Removed,
      },
    ]);
  }

  #[test]
  fn path_filter_matches_descendants() {
    let root = PathBuf::from("/tmp/work");
    assert!(path_matches_filter(&root, &root));
    assert!(path_matches_filter(&root.join("src/main.rs"), &root));
    assert!(!path_matches_filter(
      &PathBuf::from("/tmp/elsewhere"),
      &root
    ));
  }
}
