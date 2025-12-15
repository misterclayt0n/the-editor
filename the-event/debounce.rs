//! Utilities for declaring an async (usually debounced) hook

use tokio::{
  sync::mpsc::{
    self,
    Sender,
    error::TrySendError,
  },
  time::Instant,
};

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

/// Send an event to a channel without blocking.
///
/// This function is designed to be called from synchronous code that needs to
/// communicate with async tasks. It prioritizes responsiveness over reliability:
/// - Attempts a non-blocking send
/// - If the channel is full, the event is dropped (no blocking)
///
/// This is acceptable because completion/signature events are advisory -
/// the next keystroke will trigger a new request anyway.
pub fn send_blocking<T>(tx: &Sender<T>, data: T) {
  match tx.try_send(data) {
    Ok(()) => {},
    Err(TrySendError::Full(_)) => {
      // Channel is full - drop event rather than block UI thread
      // Completion events are advisory; next keystroke triggers new request
    },
    Err(TrySendError::Closed(_)) => {
      // Channel is closed, nothing we can do
    },
  }
}

/// Try to send an event without blocking at all.
/// Returns true if the event was sent, false if the channel was full or closed.
pub fn try_send<T>(tx: &Sender<T>, data: T) -> bool {
  tx.try_send(data).is_ok()
}
