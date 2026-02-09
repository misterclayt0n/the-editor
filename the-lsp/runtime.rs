use std::{
  collections::HashMap,
  path::{
    Path,
    PathBuf,
  },
  sync::{
    Arc,
    Mutex,
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
  time::{
    Duration,
    Instant,
  },
};

use serde_json::{
  Value,
  json,
};
use thiserror::Error;
use tracing::{
  debug,
  warn,
};

use crate::{
  LspCommand,
  LspEvent,
  capabilities::{
    CapabilityRegistry,
    ServerCapabilitiesSnapshot,
  },
  diagnostics::parse_publish_diagnostics,
  jsonrpc,
  text_sync::file_uri_for_path,
  transport::{
    StdioTransport,
    TransportEvent,
  },
};

const INTERNAL_INITIALIZE_REQUEST_ID: u64 = 1;
const INTERNAL_SHUTDOWN_REQUEST_ID: u64 = 2;
const EXTERNAL_REQUEST_ID_START: u64 = 10_000;

#[derive(Debug, Clone)]
pub struct LspServerConfig {
  name:               String,
  command:            String,
  args:               Vec<String>,
  env:                Vec<(String, String)>,
  initialize_options: Option<Value>,
  initialize_timeout: Duration,
}

impl LspServerConfig {
  pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
    Self {
      name:               name.into(),
      command:            command.into(),
      args:               Vec::new(),
      env:                Vec::new(),
      initialize_options: None,
      initialize_timeout: Duration::from_secs(20),
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

  pub fn with_initialize_options(mut self, initialize_options: Option<Value>) -> Self {
    self.initialize_options = initialize_options;
    self
  }

  pub fn with_initialize_timeout(mut self, timeout: Duration) -> Self {
    self.initialize_timeout = timeout;
    self
  }

  pub fn name(&self) -> &str {
    &self.name
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

  pub fn initialize_options(&self) -> Option<&Value> {
    self.initialize_options.as_ref()
  }

  pub fn initialize_timeout(&self) -> Duration {
    self.initialize_timeout
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
  capabilities:    Arc<Mutex<CapabilityRegistry>>,
}

impl LspRuntime {
  pub fn new(config: LspRuntimeConfig) -> Self {
    Self {
      config,
      command_tx: None,
      event_rx: None,
      worker: None,
      request_counter: AtomicU64::new(EXTERNAL_REQUEST_ID_START),
      capabilities: Arc::new(Mutex::new(CapabilityRegistry::default())),
    }
  }

  pub fn config(&self) -> &LspRuntimeConfig {
    &self.config
  }

  pub fn is_running(&self) -> bool {
    self.worker.is_some()
  }

  pub fn capabilities(&self) -> CapabilityRegistry {
    self
      .capabilities
      .lock()
      .unwrap_or_else(|err| err.into_inner())
      .clone()
  }

  pub fn server_capabilities(&self, server_name: &str) -> Option<ServerCapabilitiesSnapshot> {
    self
      .capabilities
      .lock()
      .unwrap_or_else(|err| err.into_inner())
      .get(server_name)
      .cloned()
  }

  pub fn start(&mut self) -> Result<(), LspRuntimeError> {
    if self.is_running() {
      return Err(LspRuntimeError::AlreadyRunning);
    }

    self
      .capabilities
      .lock()
      .unwrap_or_else(|err| err.into_inner())
      .clear();

    let (command_tx, command_rx) = channel();
    let (event_tx, event_rx) = channel();
    let config = self.config.clone();
    let capabilities = Arc::clone(&self.capabilities);

    let worker = thread::Builder::new()
      .name("the-lsp-runtime".into())
      .spawn(move || run_worker(config, capabilities, command_rx, event_tx))
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

#[derive(Debug)]
struct PendingRequest {
  method:     String,
  kind:       PendingRequestKind,
  timeout_at: Option<Instant>,
}

impl PendingRequest {
  fn initialize(server_name: String, timeout_at: Instant) -> Self {
    Self {
      method:     "initialize".into(),
      kind:       PendingRequestKind::Initialize { server_name },
      timeout_at: Some(timeout_at),
    }
  }

  fn other(method: String) -> Self {
    Self {
      method,
      kind: PendingRequestKind::Other,
      timeout_at: None,
    }
  }
}

#[derive(Debug)]
enum PendingRequestKind {
  Initialize { server_name: String },
  Other,
}

fn run_worker(
  config: LspRuntimeConfig,
  capabilities: Arc<Mutex<CapabilityRegistry>>,
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

  let mut pending_requests = HashMap::<u64, PendingRequest>::new();
  let mut transport = spawn_transport(&config, &event_tx);
  if let Some(current_transport) = transport.as_ref() {
    initialize_server(&config, current_transport, &event_tx, &mut pending_requests);
  }

  let mut should_exit = false;
  while !should_exit {
    if check_request_timeouts(&mut pending_requests, &event_tx) && config.restart_on_failure {
      transport = restart_transport(
        &config,
        &capabilities,
        transport,
        &event_tx,
        &mut pending_requests,
      );
    }

    if let Some(current_transport) = transport.as_mut() {
      let mut should_restart = false;
      while let Some(event) = current_transport.try_recv_event() {
        match event {
          TransportEvent::Message(message) => {
            handle_rpc_message(
              &message,
              current_transport,
              &event_tx,
              &capabilities,
              &mut pending_requests,
            );
            handle_notification_message(&message, &event_tx);
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
        transport = restart_transport(
          &config,
          &capabilities,
          transport,
          &event_tx,
          &mut pending_requests,
        );
      }
    }

    match command_rx.recv_timeout(Duration::from_millis(16)) {
      Ok(LspCommand::Shutdown) => {
        should_exit = true;
      },
      Ok(LspCommand::RestartServer) => {
        transport = restart_transport(
          &config,
          &capabilities,
          transport,
          &event_tx,
          &mut pending_requests,
        );
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
          pending_requests.insert(id, PendingRequest::other(method.clone()));
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
  transport: &StdioTransport,
  event_tx: &Sender<LspEvent>,
  capabilities: &Arc<Mutex<CapabilityRegistry>>,
  pending_requests: &mut HashMap<u64, PendingRequest>,
) {
  let jsonrpc::Message::Response(response) = message else {
    return;
  };
  let jsonrpc::Id::Number(id) = &response.id else {
    return;
  };

  let Some(pending) = pending_requests.remove(id) else {
    return;
  };
  let _ = event_tx.send(LspEvent::RequestCompleted { id: *id });

  match pending.kind {
    PendingRequestKind::Initialize { server_name } => {
      if let Some(error) = &response.error {
        let _ = event_tx.send(LspEvent::Error(format!(
          "lsp initialize failed: {} ({})",
          error.message, error.code
        )));
        return;
      }

      if let Some(raw_capabilities) = extract_server_capabilities(response) {
        capabilities
          .lock()
          .unwrap_or_else(|err| err.into_inner())
          .register(server_name.clone(), raw_capabilities);
        let _ = event_tx.send(LspEvent::CapabilitiesRegistered { server_name });
      } else {
        let _ = event_tx.send(LspEvent::Error(
          "lsp initialize response missing capabilities".into(),
        ));
      }

      if let Err(err) = transport.send(jsonrpc::Message::notification(
        "initialized",
        Some(json!({})),
      )) {
        let _ = event_tx.send(LspEvent::Error(format!(
          "failed to send initialized notification: {err}"
        )));
      }
    },
    PendingRequestKind::Other => {},
  }
}

fn handle_notification_message(message: &jsonrpc::Message, event_tx: &Sender<LspEvent>) {
  let jsonrpc::Message::Notification(notification) = message else {
    return;
  };

  if notification.method != "textDocument/publishDiagnostics" {
    return;
  }

  match parse_publish_diagnostics(notification.params.as_ref()) {
    Ok(diagnostics) => {
      let _ = event_tx.send(LspEvent::DiagnosticsPublished { diagnostics });
    },
    Err(err) => {
      let _ = event_tx.send(LspEvent::Error(format!(
        "failed to parse publishDiagnostics: {err}"
      )));
    },
  }
}

fn check_request_timeouts(
  pending_requests: &mut HashMap<u64, PendingRequest>,
  event_tx: &Sender<LspEvent>,
) -> bool {
  let now = Instant::now();
  let mut initialize_timed_out = false;

  pending_requests.retain(|_id, pending| {
    let Some(deadline) = pending.timeout_at else {
      return true;
    };
    if now < deadline {
      return true;
    }

    if matches!(pending.kind, PendingRequestKind::Initialize { .. }) {
      initialize_timed_out = true;
    }
    let _ = event_tx.send(LspEvent::Error(format!(
      "lsp request timeout: {}",
      pending.method
    )));
    false
  });

  initialize_timed_out
}

fn initialize_server(
  config: &LspRuntimeConfig,
  transport: &StdioTransport,
  event_tx: &Sender<LspEvent>,
  pending_requests: &mut HashMap<u64, PendingRequest>,
) {
  let Some(server) = config.server() else {
    return;
  };

  let params = build_initialize_params(config.workspace_root(), server.initialize_options());
  let message =
    jsonrpc::Message::request(INTERNAL_INITIALIZE_REQUEST_ID, "initialize", Some(params));
  if let Err(err) = transport.send(message) {
    let _ = event_tx.send(LspEvent::Error(format!(
      "failed to send initialize request: {err}"
    )));
    return;
  }

  pending_requests.insert(
    INTERNAL_INITIALIZE_REQUEST_ID,
    PendingRequest::initialize(
      server.name().to_string(),
      Instant::now() + server.initialize_timeout(),
    ),
  );
  let _ = event_tx.send(LspEvent::RequestDispatched {
    id:     INTERNAL_INITIALIZE_REQUEST_ID,
    method: "initialize".into(),
  });
}

fn build_initialize_params(workspace_root: &Path, initialize_options: Option<&Value>) -> Value {
  let root_uri = file_uri_for_path(workspace_root);
  let workspace_name = workspace_root
    .file_name()
    .map(|name| name.to_string_lossy().to_string())
    .unwrap_or_else(|| workspace_root.display().to_string());

  let workspace_folders = root_uri
    .as_ref()
    .map(|uri| {
      json!([{
        "uri": uri,
        "name": workspace_name,
      }])
    })
    .unwrap_or(Value::Null);

  let mut params = json!({
    "processId": std::process::id(),
    "clientInfo": {
      "name": "the-editor",
      "version": env!("CARGO_PKG_VERSION"),
    },
    "rootPath": workspace_root.display().to_string(),
    "rootUri": root_uri,
    "workspaceFolders": workspace_folders,
    "capabilities": default_client_capabilities(),
    "trace": "off",
  });

  if let (Some(opts), Some(object)) = (initialize_options, params.as_object_mut()) {
    object.insert("initializationOptions".into(), opts.clone());
  }

  params
}

fn default_client_capabilities() -> Value {
  json!({
    "workspace": {
      "workspaceFolders": true,
      "configuration": true,
      "didChangeConfiguration": {
        "dynamicRegistration": true
      }
    },
    "textDocument": {
      "synchronization": {
        "dynamicRegistration": true,
        "didSave": true,
        "willSave": false,
        "willSaveWaitUntil": false
      }
    }
  })
}

fn extract_server_capabilities(response: &jsonrpc::Response) -> Option<Value> {
  response.result.as_ref()?.get("capabilities").cloned()
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
        server_name: server.name().to_string(),
        command:     server.command().to_string(),
        args:        server.args().to_vec(),
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
  let _ = transport.send(jsonrpc::Message::request(
    INTERNAL_SHUTDOWN_REQUEST_ID,
    "shutdown",
    None,
  ));
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
  capabilities: &Arc<Mutex<CapabilityRegistry>>,
  mut current: Option<StdioTransport>,
  event_tx: &Sender<LspEvent>,
  pending_requests: &mut HashMap<u64, PendingRequest>,
) -> Option<StdioTransport> {
  pending_requests.clear();
  let _ = shutdown_transport(&mut current, event_tx);

  if let Some(server) = config.server() {
    capabilities
      .lock()
      .unwrap_or_else(|err| err.into_inner())
      .remove(server.name());
  }

  if config.server().is_none() {
    return None;
  }

  thread::sleep(config.restart_backoff);
  let restarted = spawn_transport(config, event_tx);
  if let Some(transport) = restarted.as_ref() {
    initialize_server(config, transport, event_tx, pending_requests);
    warn!("lsp transport restarted");
  }
  restarted
}
