//! Filesystem watcher helpers for runtime hosts.
//!
//! This module provides a debounced, batched watcher stream around `notify`.
//! It is intentionally side-effectful and belongs in runtime/app layers.

use std::{
  collections::{
    BTreeMap,
    HashMap,
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
  time::Duration,
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
    {
      let registrations = lock(&self.registrations);
      if registrations.contains_key(&logical_path) {
        return Ok(());
      }
    }

    let mut registration_ids = Vec::new();

    match self.register(logical_path.clone(), logical_path.clone()) {
      Ok(id) => registration_ids.push(id),
      Err(err) => {
        let Some(parent) = logical_path.parent() else {
          return Err(err);
        };
        let parent = normalize_path(parent);
        let id = self.register(parent, logical_path.clone())?;
        registration_ids.push(id);
      },
    }

    if let Some(target) = resolve_symlink_target(&logical_path) {
      if let Ok(id) = self.register(target.clone(), target.clone()) {
        registration_ids.push(id);
      }
      if let Some(parent) = target.parent() {
        let parent = normalize_path(parent);
        if let Ok(id) = self.register(parent, target) {
          registration_ids.push(id);
        }
      }
    }

    lock(&self.registrations).insert(logical_path, registration_ids);
    Ok(())
  }

  /// Remove a previously added logical path.
  pub fn remove(&self, path: &Path) -> notify::Result<()> {
    let logical_path = normalize_path(path);
    let Some(registrations) = lock(&self.registrations).remove(&logical_path) else {
      return Ok(());
    };

    for registration in registrations {
      global(|watcher| watcher.remove(registration))?;
    }

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

    let registration = global(|watcher| {
      watcher.add(watch_path, watch_mode(), move |event: &notify::Event| {
        if shutdown.load(Ordering::Relaxed) {
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
          return;
        }

        let mut pending = lock(&pending_events);
        let should_wake = pending.is_empty();
        pending.extend(mapped);
        drop(pending);

        if should_wake {
          let _ = wake_tx.send(());
        }
      })
    })?;

    registration
  }
}

impl Drop for WatchHandle {
  fn drop(&mut self) {
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
  }
}

/// Create a debounced watcher stream for `path`.
///
/// The returned receiver yields batches of coalesced path events.
pub fn watch(path: &Path, latency: Duration) -> (mpsc::Receiver<Vec<PathEvent>>, WatchHandle) {
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

  let _ = handle.add(path);
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
    while wake_rx.recv().is_ok() {
      if shutdown.load(Ordering::Relaxed) {
        break;
      }

      if !latency.is_zero() {
        thread::sleep(latency);
      }

      while wake_rx.try_recv().is_ok() {}

      let pending = {
        let mut pending = lock(&pending_events);
        if pending.is_empty() {
          continue;
        }
        std::mem::take(&mut *pending)
      };
      let batch = coalesce_events(pending);
      if batch.is_empty() {
        continue;
      }

      if events_tx.send(batch).is_err() {
        shutdown.store(true, Ordering::Relaxed);
        break;
      }
    }
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
    }

    let id = state.next_registration_id;
    state.next_registration_id = WatcherRegistrationId(id.0 + 1);
    state.watchers.insert(id, WatcherRegistrationState {
      callback: Arc::new(cb),
      path:     path.clone(),
    });
    *state.path_registrations.entry(path).or_insert(0) += 1;
    Ok(id)
  }

  fn remove(&self, id: WatcherRegistrationId) {
    let mut state = lock(&self.state);
    let Some(registration_state) = state.watchers.remove(&id) else {
      return;
    };

    let Some(count) = state.path_registrations.get_mut(&registration_state.path) else {
      return;
    };
    *count -= 1;
    if *count == 0 {
      state.path_registrations.remove(&registration_state.path);

      drop(state);
      let _ = lock(&self.watcher).unwatch(&registration_state.path);
    }
  }
}

static GLOBAL_WATCHER: OnceLock<GlobalWatcher> = OnceLock::new();

fn global<T>(f: impl FnOnce(&GlobalWatcher) -> T) -> notify::Result<T> {
  if let Some(global) = GLOBAL_WATCHER.get() {
    return Ok(f(global));
  }

  let watcher = notify::recommended_watcher(handle_event)?;
  let global_watcher = GlobalWatcher {
    state:   Mutex::new(WatcherState::default()),
    watcher: Mutex::new(watcher),
  };
  let _ = GLOBAL_WATCHER.set(global_watcher);

  let global = GLOBAL_WATCHER
    .get()
    .expect("global watcher should be initialized");
  Ok(f(global))
}

fn handle_event(event: notify::Result<notify::Event>) {
  let Ok(event) = event else {
    return;
  };
  if matches!(event.kind, EventKind::Access(_)) {
    return;
  }

  let callbacks = match global(|watcher| {
    let state = lock(&watcher.state);
    state
      .watchers
      .values()
      .map(|registration| Arc::clone(&registration.callback))
      .collect::<Vec<_>>()
  }) {
    Ok(callbacks) => callbacks,
    Err(_) => return,
  };

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
