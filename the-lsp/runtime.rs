use std::{
  path::{
    Path,
    PathBuf,
  },
  sync::mpsc::{
    Receiver,
    Sender,
    TryRecvError,
    channel,
  },
  thread::{
    self,
    JoinHandle,
  },
};

use thiserror::Error;
use tracing::debug;

use crate::{
  LspCommand,
  LspEvent,
};

#[derive(Debug, Clone)]
pub struct LspRuntimeConfig {
  workspace_root: PathBuf,
}

impl LspRuntimeConfig {
  pub fn new(workspace_root: PathBuf) -> Self {
    Self { workspace_root }
  }

  pub fn workspace_root(&self) -> &Path {
    &self.workspace_root
  }
}

pub struct LspRuntime {
  config:     LspRuntimeConfig,
  command_tx: Option<Sender<LspCommand>>,
  event_rx:   Option<Receiver<LspEvent>>,
  worker:     Option<JoinHandle<()>>,
}

impl LspRuntime {
  pub fn new(config: LspRuntimeConfig) -> Self {
    Self {
      config,
      command_tx: None,
      event_rx: None,
      worker: None,
    }
  }

  pub fn config(&self) -> &LspRuntimeConfig {
    &self.config
  }

  pub fn is_running(&self) -> bool {
    self.worker.is_some()
  }

  pub fn start(&mut self) -> Result<(), LspRuntimeError> {
    if self.is_running() {
      return Err(LspRuntimeError::AlreadyRunning);
    }

    let (command_tx, command_rx) = channel();
    let (event_tx, event_rx) = channel();
    let workspace_root = self.config.workspace_root.clone();

    let worker = thread::Builder::new()
      .name("the-lsp-runtime".into())
      .spawn(move || run_worker(workspace_root, command_rx, event_tx))
      .map_err(|_| LspRuntimeError::FailedToSpawnWorker)?;

    self.command_tx = Some(command_tx);
    self.event_rx = Some(event_rx);
    self.worker = Some(worker);

    Ok(())
  }

  pub fn send(&self, command: LspCommand) -> Result<(), LspRuntimeError> {
    let Some(tx) = &self.command_tx else {
      return Err(LspRuntimeError::NotRunning);
    };

    tx.send(command)
      .map_err(|_| LspRuntimeError::CommandChannelClosed)
  }

  pub fn try_recv_event(&self) -> Option<LspEvent> {
    let rx = self.event_rx.as_ref()?;
    match rx.try_recv() {
      Ok(event) => Some(event),
      Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
    }
  }

  pub fn shutdown(&mut self) -> Result<(), LspRuntimeError> {
    if !self.is_running() {
      return Ok(());
    }

    if let Some(tx) = self.command_tx.take() {
      let _ = tx.send(LspCommand::Shutdown);
    }

    if let Some(worker) = self.worker.take() {
      worker.join().map_err(|_| LspRuntimeError::WorkerPanicked)?;
    }

    self.event_rx = None;
    Ok(())
  }
}

impl Drop for LspRuntime {
  fn drop(&mut self) {
    let _ = self.shutdown();
  }
}

#[derive(Debug, Error)]
pub enum LspRuntimeError {
  #[error("lsp runtime is already running")]
  AlreadyRunning,
  #[error("lsp runtime is not running")]
  NotRunning,
  #[error("failed to spawn lsp runtime worker thread")]
  FailedToSpawnWorker,
  #[error("lsp runtime command channel is closed")]
  CommandChannelClosed,
  #[error("lsp runtime worker thread panicked")]
  WorkerPanicked,
}

fn run_worker(
  workspace_root: PathBuf,
  command_rx: Receiver<LspCommand>,
  event_tx: Sender<LspEvent>,
) {
  debug!(
    workspace = %workspace_root.display(),
    "lsp runtime worker started"
  );
  let _ = event_tx.send(LspEvent::Started {
    workspace_root: workspace_root.clone(),
  });

  while let Ok(command) = command_rx.recv() {
    match command {
      LspCommand::Shutdown => break,
    }
  }

  debug!(
    workspace = %workspace_root.display(),
    "lsp runtime worker stopped"
  );
  let _ = event_tx.send(LspEvent::Stopped);
}
