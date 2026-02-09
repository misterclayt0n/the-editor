use std::{
  collections::HashMap,
  path::{
    Path,
    PathBuf,
  },
  sync::{
    atomic::{
      AtomicU64,
      Ordering,
    },
    mpsc::{
      Receiver,
      Sender,
      TryRecvError,
      channel,
    },
  },
  thread::{
    self,
    JoinHandle,
  },
  time::Duration,
};

use serde_json::Value;
use thiserror::Error;
use tracing::{
  debug,
  warn,
};

use crate::{
  LspCommand,
  LspEvent,
  jsonrpc,
  transport::{
    StdioTransport,
    TransportEvent,
  },
};

#[derive(Debug, Clone)]
pub struct LspServerConfig {
  command: String,
  args:    Vec<String>,
  env:     Vec<(String, String)>,
}

impl LspServerConfig {
  pub fn new(command: impl Into<String>) -> Self {
    Self {
      command: command.into(),
      args:    Vec::new(),
      env:     Vec::new(),
    }
  }

  pub fn with_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
    self.args = args.into_iter().map(Into::into).collect();
    self
  }

  pub fn with_env(
    mut self,
    env: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
  ) -> Self {
    self.env = env
      .into_iter()
      .map(|(key, value)| (key.into(), value.into()))
      .collect();
    self
  }

  pub fn command(&self) -> &str {
    &self.command
  }

  pub fn args(&self) -> &[String] {
    &self.args
  }

  pub fn env(&self) -> &[(String, String)] {
    &self.env
  }
}

#[derive(Debug, Clone)]
pub struct LspRuntimeConfig {
  workspace_root:     PathBuf,
  server:             Option<LspServerConfig>,
  restart_on_failure: bool,
  restart_backoff:    Duration,
}

impl LspRuntimeConfig {
  pub fn new(workspace_root: PathBuf) -> Self {
    Self {
      workspace_root,
      server: None,
      restart_on_failure: true,
      restart_backoff: Duration::from_millis(250),
    }
  }

  pub fn workspace_root(&self) -> &Path {
    &self.workspace_root
  }

  pub fn server(&self) -> Option<&LspServerConfig> {
    self.server.as_ref()
  }

  pub fn with_server(mut self, server: LspServerConfig) -> Self {
    self.server = Some(server);
    self
  }

  pub fn clear_server(mut self) -> Self {
    self.server = None;
    self
  }

  pub fn with_restart_policy(mut self, enabled: bool, backoff: Duration) -> Self {
    self.restart_on_failure = enabled;
    self.restart_backoff = backoff;
    self
  }
}

pub struct LspRuntime {
  config:          LspRuntimeConfig,
  command_tx:      Option<Sender<LspCommand>>,
  event_rx:        Option<Receiver<LspEvent>>,
  worker:          Option<JoinHandle<()>>,
  request_counter: AtomicU64,
}

impl LspRuntime {
  pub fn new(config: LspRuntimeConfig) -> Self {
    Self {
      config,
      command_tx: None,
      event_rx: None,
      worker: None,
      request_counter: AtomicU64::new(1),
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
    let config = self.config.clone();

    let worker = thread::Builder::new()
      .name("the-lsp-runtime".into())
      .spawn(move || run_worker(config, command_rx, event_tx))
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

  pub fn send_request(
    &self,
    method: impl Into<String>,
    params: Option<Value>,
  ) -> Result<u64, LspRuntimeError> {
    let request_id = self.request_counter.fetch_add(1, Ordering::Relaxed);
    self.send(LspCommand::SendRequest {
      id: request_id,
      method: method.into(),
      params,
    })?;
    Ok(request_id)
  }

  pub fn send_notification(
    &self,
    method: impl Into<String>,
    params: Option<Value>,
  ) -> Result<(), LspRuntimeError> {
    self.send(LspCommand::SendNotification {
      method: method.into(),
      params,
    })
  }

  pub fn restart_server(&self) -> Result<(), LspRuntimeError> {
    self.send(LspCommand::RestartServer)
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
  config: LspRuntimeConfig,
  command_rx: Receiver<LspCommand>,
  event_tx: Sender<LspEvent>,
) {
  debug!(
    workspace = %config.workspace_root.display(),
    "lsp runtime worker started"
  );
  let _ = event_tx.send(LspEvent::Started {
    workspace_root: config.workspace_root.clone(),
  });

  let mut pending_requests = HashMap::<u64, String>::new();
  let mut transport = spawn_transport(&config, &event_tx);
  let mut should_exit = false;

  while !should_exit {
    if let Some(current_transport) = transport.as_mut() {
      let mut should_restart = false;
      while let Some(event) = current_transport.try_recv_event() {
        match event {
          TransportEvent::Message(message) => {
            handle_rpc_message(&message, &event_tx, &mut pending_requests);
            let _ = event_tx.send(LspEvent::RpcMessage { message });
          },
          TransportEvent::Stderr(line) => {
            let _ = event_tx.send(LspEvent::ServerStderr { line });
          },
          TransportEvent::ReadError(err) => {
            let _ = event_tx.send(LspEvent::Error(format!("lsp read error: {err}")));
            should_restart = config.restart_on_failure;
            break;
          },
          TransportEvent::WriteError(err) => {
            let _ = event_tx.send(LspEvent::Error(format!("lsp write error: {err}")));
            should_restart = config.restart_on_failure;
            break;
          },
          TransportEvent::Closed => {
            let _ = event_tx.send(LspEvent::Error("lsp server closed stdio".into()));
            should_restart = config.restart_on_failure;
            break;
          },
        }
      }

      match current_transport.poll_exit_code() {
        Ok(Some(exit_code)) => {
          let _ = event_tx.send(LspEvent::ServerStopped {
            exit_code: Some(exit_code),
          });
          should_restart = config.restart_on_failure;
        },
        Ok(None) => {},
        Err(err) => {
          let _ = event_tx.send(LspEvent::Error(format!(
            "failed to poll lsp process: {err}"
          )));
          should_restart = config.restart_on_failure;
        },
      }

      if should_restart {
        transport = restart_transport(&config, transport, &event_tx);
      }
    }

    match command_rx.recv_timeout(Duration::from_millis(16)) {
      Ok(LspCommand::Shutdown) => {
        should_exit = true;
      },
      Ok(LspCommand::RestartServer) => {
        transport = restart_transport(&config, transport, &event_tx);
      },
      Ok(LspCommand::SendNotification { method, params }) => {
        if let Some(current_transport) = transport.as_ref() {
          if let Err(err) = current_transport.send(jsonrpc::Message::notification(method, params)) {
            let _ = event_tx.send(LspEvent::Error(format!(
              "failed to send notification: {err}"
            )));
          }
        } else {
          let _ = event_tx.send(LspEvent::Error(
            "no active lsp transport to send notification".into(),
          ));
        }
      },
      Ok(LspCommand::SendRequest { id, method, params }) => {
        if let Some(current_transport) = transport.as_ref() {
          let message = jsonrpc::Message::request(id, method.clone(), params);
          if let Err(err) = current_transport.send(message) {
            let _ = event_tx.send(LspEvent::Error(format!("failed to send request: {err}")));
            continue;
          }
          pending_requests.insert(id, method.clone());
          let _ = event_tx.send(LspEvent::RequestDispatched { id, method });
        } else {
          let _ = event_tx.send(LspEvent::Error(
            "no active lsp transport to send request".into(),
          ));
        }
      },
      Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {},
      Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
        should_exit = true;
      },
    }
  }

  let _ = shutdown_transport(&mut transport, &event_tx);

  debug!(
    workspace = %config.workspace_root.display(),
    "lsp runtime worker stopped"
  );
  let _ = event_tx.send(LspEvent::Stopped);
}

fn handle_rpc_message(
  message: &jsonrpc::Message,
  event_tx: &Sender<LspEvent>,
  pending_requests: &mut HashMap<u64, String>,
) {
  let jsonrpc::Message::Response(response) = message else {
    return;
  };
  let jsonrpc::Id::Number(id) = &response.id else {
    return;
  };
  if pending_requests.remove(id).is_some() {
    let _ = event_tx.send(LspEvent::RequestCompleted { id: *id });
  }
}

fn spawn_transport(
  config: &LspRuntimeConfig,
  event_tx: &Sender<LspEvent>,
) -> Option<StdioTransport> {
  let server = config.server()?;
  match StdioTransport::spawn(
    server.command(),
    server.args(),
    server.env(),
    config.workspace_root(),
  ) {
    Ok(transport) => {
      let _ = event_tx.send(LspEvent::ServerStarted {
        command: server.command().to_string(),
        args:    server.args().to_vec(),
      });
      Some(transport)
    },
    Err(err) => {
      let _ = event_tx.send(LspEvent::Error(format!(
        "failed to start lsp server: {err}"
      )));
      None
    },
  }
}

fn shutdown_transport(
  transport: &mut Option<StdioTransport>,
  event_tx: &Sender<LspEvent>,
) -> Option<i32> {
  let mut transport = transport.take()?;
  let _ = transport.send(jsonrpc::Message::request(0, "shutdown", None));
  let _ = transport.send(jsonrpc::Message::notification("exit", None));
  match transport.shutdown() {
    Ok(exit_code) => {
      let _ = event_tx.send(LspEvent::ServerStopped { exit_code });
      exit_code
    },
    Err(err) => {
      let _ = event_tx.send(LspEvent::Error(format!(
        "failed to shutdown lsp transport: {err}"
      )));
      None
    },
  }
}

fn restart_transport(
  config: &LspRuntimeConfig,
  mut current: Option<StdioTransport>,
  event_tx: &Sender<LspEvent>,
) -> Option<StdioTransport> {
  let _ = shutdown_transport(&mut current, event_tx);
  if config.server().is_none() {
    return None;
  }
  thread::sleep(config.restart_backoff);
  let restarted = spawn_transport(config, event_tx);
  if restarted.is_some() {
    warn!("lsp transport restarted");
  }
  restarted
}
