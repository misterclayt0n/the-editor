use std::{
  sync::mpsc::Sender,
  thread,
  time::Duration,
};

#[derive(Clone)]
pub struct RenderWaker {
  tx: Sender<()>,
}

impl RenderWaker {
  pub fn new(tx: Sender<()>) -> Self {
    Self { tx }
  }

  pub fn wake(&self) {
    let _ = self.tx.send(());
  }

  pub fn wake_after(&self, delay: Duration) {
    let waker = self.clone();
    thread::spawn(move || {
      thread::sleep(delay);
      waker.wake();
    });
  }

  pub fn sender(&self) -> Sender<()> {
    self.tx.clone()
  }
}

impl std::fmt::Debug for RenderWaker {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("RenderWaker(..)")
  }
}
