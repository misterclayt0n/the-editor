//! Utilities for declaring an async (usually debounced) hook

use std::time::Duration;

use futures_executor::block_on;
use tokio::{
  sync::mpsc::{
    self,
    Sender,
    error::TrySendError,
  },
  time::Instant,
};

/// Maximum time to block when sending to a full channel.
/// Keep this very short to avoid UI freezes - better to drop a message
/// than to freeze the editor.
const SEND_TIMEOUT_MS: u64 = 2;

/// Async hooks provide a convenient framework for implementing (debounced)
/// async event handlers. Most synchronous event hooks will likely need to
/// debounce their events, coordinate multiple different hooks and potentially
/// track some state. `AsyncHooks` facilitate these use cases by running as
/// a background tokio task that waits for events (usually an enum) to be
/// sent through a channel.
pub trait AsyncHook: Sync + Send + 'static + Sized {
  type Event: Sync + Send + 'static;
  /// Called immediately whenever an event is received, this function can
  /// consume the event immediately or debounce it. In case of debouncing,
  /// it can either define a new debounce timeout or continue the current one
  fn handle_event(&mut self, event: Self::Event, timeout: Option<Instant>) -> Option<Instant>;

  /// Called whenever the debounce timeline is reached
  fn finish_debounce(&mut self);

  fn spawn(self) -> mpsc::Sender<Self::Event> {
    // Use a larger capacity to reduce the chance of blocking.
    // The channel should rarely fill up since we immediately drain events,
    // but during rapid typing we want extra headroom.
    let (tx, rx) = mpsc::channel(256);
    // only spawn worker if we are inside runtime to avoid having to spawn a runtime
    // for unrelated unit tests
    if tokio::runtime::Handle::try_current().is_ok() {
      tokio::spawn(run(self, rx));
    }
    tx
  }
}

async fn run<Hook: AsyncHook>(mut hook: Hook, mut rx: mpsc::Receiver<Hook::Event>) {
  let mut deadline = None;
  loop {
    let event = match deadline {
      Some(deadline_) => {
        let res = tokio::time::timeout_at(deadline_, rx.recv()).await;
        match res {
          Ok(event) => event,
          Err(_) => {
            hook.finish_debounce();
            deadline = None;
            continue;
          },
        }
      },
      None => rx.recv().await,
    };
    let Some(event) = event else {
      break;
    };
    deadline = hook.handle_event(event, deadline);
  }
}

/// Send an event to a channel, blocking only briefly if the channel is full.
///
/// This function is designed to be called from synchronous code that needs to
/// communicate with async tasks. It prioritizes responsiveness over reliability:
/// - First attempts a non-blocking send (fast path)
/// - If the channel is full, blocks for at most `SEND_TIMEOUT_MS` milliseconds
/// - If still full after timeout, the message is dropped
///
/// This trade-off prevents UI freezes when the async system is overwhelmed.
pub fn send_blocking<T>(tx: &Sender<T>, data: T) {
  // Fast path: try non-blocking send first
  match tx.try_send(data) {
    Ok(()) => {},
    Err(TrySendError::Full(data)) => {
      // Channel is full - block briefly but don't freeze the UI
      // Use a very short timeout to minimize UI impact
      let _ = block_on(tx.send_timeout(data, Duration::from_millis(SEND_TIMEOUT_MS)));
    },
    Err(TrySendError::Closed(_)) => {
      // Channel is closed, nothing we can do
      log::warn!("Attempted to send to closed channel");
    },
  }
}

/// Try to send an event without blocking at all.
/// Returns true if the event was sent, false if the channel was full or closed.
pub fn try_send<T>(tx: &Sender<T>, data: T) -> bool {
  tx.try_send(data).is_ok()
}
