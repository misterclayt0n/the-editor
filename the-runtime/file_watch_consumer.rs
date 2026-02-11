//! Shared watch-consumer polling logic for editor clients.
//!
//! This keeps `the-term` and `the-ffi` behavior in sync while still allowing
//! each client to decide how to react to collected change kinds.

use std::{
  path::PathBuf,
  sync::mpsc::{
    Receiver,
    TryRecvError,
  },
  time::Instant,
};

use crate::{
  file_watch::{
    PathEvent,
    PathEventKind,
  },
  file_watch_reload::{
    FileWatchReloadIoState,
    FileWatchReloadState,
  },
};

/// Mutable state required to poll a watched file event stream.
pub struct WatchedFileEventsState {
  pub path:           PathBuf,
  pub uri:            String,
  pub events_rx:      Receiver<Vec<PathEvent>>,
  pub suppress_until: Option<Instant>,
  pub reload_state:   FileWatchReloadState,
  pub reload_io:      FileWatchReloadIoState,
}

/// Poll result for [`poll_watch_events`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchPollOutcome {
  NoChanges,
  Disconnected {
    path: PathBuf,
  },
  Changes {
    path:  PathBuf,
    uri:   String,
    kinds: Vec<PathEventKind>,
  },
}

/// Poll a watched-file event stream and collect debounced change kinds.
///
/// - Drops batches while `suppress_until` is active.
/// - Returns only the final kind per received batch (matching existing client
///   behavior).
/// - Returns [`WatchPollOutcome::Disconnected`] when the stream disconnects.
pub fn poll_watch_events(
  state: Option<&mut WatchedFileEventsState>,
  now: Instant,
  client: &str,
  mut trace: impl FnMut(&str, String),
) -> WatchPollOutcome {
  let Some(state) = state else {
    trace(
      "consumer_poll_skip",
      format!("client={client} reason=no_watch_state"),
    );
    return WatchPollOutcome::NoChanges;
  };

  let watched_path = state.path.clone();
  let watched_uri = state.uri.clone();
  let mut disconnected = false;
  let mut kinds = Vec::new();

  loop {
    match state.events_rx.try_recv() {
      Ok(batch) => {
        if batch.is_empty() {
          continue;
        }

        if let Some(until) = state.suppress_until {
          if now <= until {
            trace(
              "consumer_suppress_drop",
              format!(
                "client={client} path={} reason=self_save_window",
                state.path.display()
              ),
            );
            continue;
          }
          state.suppress_until = None;
        }

        let mut batch_kind = None;
        for event in batch {
          batch_kind = Some(event.kind);
        }
        if let Some(kind) = batch_kind {
          kinds.push(kind);
        }
      },
      Err(TryRecvError::Empty) => break,
      Err(TryRecvError::Disconnected) => {
        disconnected = true;
        break;
      },
    }
  }

  if disconnected {
    trace(
      "consumer_watcher_disconnected",
      format!("client={client} path={}", watched_path.display()),
    );
    return WatchPollOutcome::Disconnected { path: watched_path };
  }

  if kinds.is_empty() {
    return WatchPollOutcome::NoChanges;
  }

  WatchPollOutcome::Changes {
    path: watched_path,
    uri: watched_uri,
    kinds,
  }
}

#[cfg(test)]
mod tests {
  use std::{
    sync::mpsc::channel,
    time::{
      Duration,
      Instant,
    },
  };

  use super::{
    WatchPollOutcome,
    WatchedFileEventsState,
    poll_watch_events,
  };
  use crate::{
    file_watch::{
      PathEvent,
      PathEventKind,
    },
    file_watch_reload::{
      FileWatchReloadIoState,
      FileWatchReloadState,
    },
  };

  fn new_state() -> (
    WatchedFileEventsState,
    std::sync::mpsc::Sender<Vec<PathEvent>>,
  ) {
    let (tx, rx) = channel();
    (
      WatchedFileEventsState {
        path:           "/tmp/watched.txt".into(),
        uri:            "file:///tmp/watched.txt".into(),
        events_rx:      rx,
        suppress_until: None,
        reload_state:   FileWatchReloadState::Clean,
        reload_io:      FileWatchReloadIoState::default(),
      },
      tx,
    )
  }

  #[test]
  fn poll_without_state_returns_no_changes() {
    let mut traced = Vec::new();
    let outcome = poll_watch_events(None, Instant::now(), "test", |event, message| {
      traced.push((event.to_string(), message));
    });

    assert_eq!(outcome, WatchPollOutcome::NoChanges);
    assert_eq!(traced.len(), 1);
    assert_eq!(traced[0].0, "consumer_poll_skip");
  }

  #[test]
  fn poll_collects_last_kind_per_batch() {
    let (mut state, tx) = new_state();
    tx.send(vec![
      PathEvent {
        path: state.path.clone(),
        kind: PathEventKind::Created,
      },
      PathEvent {
        path: state.path.clone(),
        kind: PathEventKind::Changed,
      },
    ])
    .expect("send first batch");
    tx.send(vec![PathEvent {
      path: state.path.clone(),
      kind: PathEventKind::Removed,
    }])
    .expect("send second batch");

    let outcome = poll_watch_events(Some(&mut state), Instant::now(), "test", |_, _| {});
    assert_eq!(outcome, WatchPollOutcome::Changes {
      path:  state.path.clone(),
      uri:   state.uri.clone(),
      kinds: vec![PathEventKind::Changed, PathEventKind::Removed],
    });
  }

  #[test]
  fn poll_honors_suppress_window_until_expired() {
    let (mut state, tx) = new_state();
    let now = Instant::now();
    state.suppress_until = Some(now + Duration::from_secs(2));

    tx.send(vec![PathEvent {
      path: state.path.clone(),
      kind: PathEventKind::Changed,
    }])
    .expect("send suppressed batch");

    let first = poll_watch_events(Some(&mut state), now, "test", |_, _| {});
    assert_eq!(first, WatchPollOutcome::NoChanges);
    assert!(state.suppress_until.is_some());

    tx.send(vec![PathEvent {
      path: state.path.clone(),
      kind: PathEventKind::Changed,
    }])
    .expect("send post-window batch");

    let second = poll_watch_events(
      Some(&mut state),
      now + Duration::from_secs(3),
      "test",
      |_, _| {},
    );
    assert_eq!(second, WatchPollOutcome::Changes {
      path:  state.path.clone(),
      uri:   state.uri.clone(),
      kinds: vec![PathEventKind::Changed],
    });
    assert!(state.suppress_until.is_none());
  }

  #[test]
  fn poll_reports_disconnect() {
    let (mut state, tx) = new_state();
    drop(tx);

    let outcome = poll_watch_events(Some(&mut state), Instant::now(), "test", |_, _| {});
    assert_eq!(outcome, WatchPollOutcome::Disconnected {
      path: state.path.clone(),
    });
  }
}
