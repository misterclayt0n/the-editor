//! ACP connection handle management.
//!
//! This module provides `AcpHandle` which manages the lifecycle of the connection
//! to an ACP agent subprocess.
//!
//! ## Architecture
//!
//! ACP uses `!Send` futures internally, which means we can't use them directly
//! on tokio's multi-threaded runtime. To solve this, we spawn a dedicated thread
//! that runs its own single-threaded runtime with a LocalSet. Communication
//! between the editor and this thread happens via channels.
//!
//! All operations are non-blocking from the editor's perspective:
//! - Commands are sent via `command_tx` (fire-and-forget)
//! - Results/errors come back via `event_rx` (polled in main loop)

use std::{
  path::PathBuf,
  process::Stdio,
  sync::Arc,
  thread::JoinHandle,
};

use agent_client_protocol::{
  self as acp,
  Agent,
};
use anyhow::{
  Context as _,
  Result,
  bail,
};
use parking_lot::Mutex;
use tokio::{
  process::{
    Child,
    Command,
  },
  sync::{
    mpsc,
    oneshot,
  },
};
use tokio_util::compat::{
  TokioAsyncReadCompatExt,
  TokioAsyncWriteCompatExt,
};

use super::{
  AcpConfig,
  EditorClient,
  PendingPermission,
  StreamEvent,
};

/// Commands sent to the ACP thread.
enum AcpCommand {
  /// Send a prompt to the agent (fire-and-forget, results via event_tx)
  Prompt {
    session_id: acp::SessionId,
    content:    Vec<acp::ContentBlock>,
  },
  /// Set the session model
  SetModel {
    session_id: acp::SessionId,
    model_id:   acp::ModelId,
  },
  /// Shutdown the ACP thread
  Shutdown,
}

/// Handle to an active ACP connection.
///
/// This struct manages the agent subprocess and provides methods to interact
/// with it. It is designed to be held in the `Editor` struct.
pub struct AcpHandle {
  /// The current session ID
  session_id: Arc<Mutex<Option<acp::SessionId>>>,
  /// Current model state (available models and current selection)
  model_state: Arc<Mutex<Option<acp::SessionModelState>>>,
  /// Receiver for streaming events from the agent
  pub event_rx: mpsc::UnboundedReceiver<StreamEvent>,
  /// Receiver for permission requests from the agent
  pub permission_rx: mpsc::UnboundedReceiver<PendingPermission>,
  /// Channel to send commands to the ACP thread
  command_tx: mpsc::UnboundedSender<AcpCommand>,
  /// Handle to the ACP thread (for cleanup)
  _thread: JoinHandle<()>,
}

impl AcpHandle {
  /// Start an ACP agent and establish a connection.
  ///
  /// This spawns a dedicated thread for the ACP runtime since ACP uses !Send futures.
  pub fn start(config: &AcpConfig, cwd: PathBuf) -> Result<Self> {
    if config.command.is_empty() {
      bail!("ACP command is empty");
    }

    let program = config.command[0].clone();
    let args: Vec<String> = config.command[1..].to_vec();

    // Create channels for communication
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (permission_tx, permission_rx) = mpsc::unbounded_channel();
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (init_tx, init_rx) =
      oneshot::channel::<Result<(acp::SessionId, Option<acp::SessionModelState>)>>();

    let cwd_clone = cwd.clone();

    // Clone event_tx for the event loop to report errors
    let event_tx_for_loop = event_tx.clone();

    // Spawn dedicated thread for ACP runtime
    let thread = std::thread::spawn(move || {
      let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create ACP runtime");

      let local = tokio::task::LocalSet::new();

      local.block_on(&rt, async move {
        match Self::init_connection(&program, &args, cwd_clone, event_tx, permission_tx).await {
          Ok((conn, session_id, model_state, child)) => {
            let _ = init_tx.send(Ok((session_id.clone(), model_state)));
            Self::run_event_loop(conn, command_rx, event_tx_for_loop, child).await;
          },
          Err(e) => {
            let _ = init_tx.send(Err(e));
          },
        }
      });
    });

    // Wait for initialization to complete (this is the only blocking part)
    let (session_id, model_state) = init_rx
      .blocking_recv()
      .context("ACP thread died during initialization")??;

    log::info!("ACP connection established, session: {}", session_id);

    Ok(Self {
      session_id: Arc::new(Mutex::new(Some(session_id))),
      model_state: Arc::new(Mutex::new(model_state)),
      event_rx,
      permission_rx,
      command_tx,
      _thread: thread,
    })
  }

  /// Initialize the ACP connection (runs on the ACP thread).
  async fn init_connection(
    program: &str,
    args: &[String],
    cwd: PathBuf,
    event_tx: mpsc::UnboundedSender<StreamEvent>,
    permission_tx: mpsc::UnboundedSender<PendingPermission>,
  ) -> Result<(
    Arc<acp::ClientSideConnection>,
    acp::SessionId,
    Option<acp::SessionModelState>,
    Child,
  )> {
    log::info!("Starting ACP agent: {} {:?}", program, args);

    // Spawn the agent process
    let mut child = Command::new(program)
      .args(args)
      .current_dir(&cwd)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::inherit())
      .kill_on_drop(true)
      .spawn()
      .with_context(|| format!("Failed to spawn ACP agent: {}", program))?;

    let stdin = child
      .stdin
      .take()
      .context("Failed to get agent stdin")?
      .compat_write();
    let stdout = child
      .stdout
      .take()
      .context("Failed to get agent stdout")?
      .compat();

    // Create the client
    let client = EditorClient::new(event_tx, permission_tx);

    // Create the connection
    let (conn, handle_io) = acp::ClientSideConnection::new(client, stdin, stdout, |fut| {
      tokio::task::spawn_local(fut);
    });

    let conn = Arc::new(conn);

    // Spawn the I/O handler on the local set
    tokio::task::spawn_local(handle_io);

    // Initialize the connection
    log::info!("Initializing ACP connection...");
    conn
      .initialize(acp::InitializeRequest {
        protocol_version:    acp::V1,
        client_capabilities: acp::ClientCapabilities::default(),
        client_info:         Some(acp::Implementation {
          name:    "the-editor".to_string(),
          title:   Some("The Editor".to_string()),
          version: env!("CARGO_PKG_VERSION").to_string(),
        }),
        meta:                None,
      })
      .await
      .context("Failed to initialize ACP connection")?;

    log::info!("ACP connection initialized");

    // Create a new session
    log::info!("Creating ACP session...");
    let session_response = conn
      .new_session(acp::NewSessionRequest {
        mcp_servers: Vec::new(),
        cwd,
        meta: None,
      })
      .await
      .context("Failed to create ACP session")?;

    log::info!("ACP session created: {}", session_response.session_id);

    // Log available models if present
    if let Some(ref models) = session_response.models {
      log::info!(
        "ACP models available: {} (current: {})",
        models.available_models.len(),
        models.current_model_id
      );
    }

    Ok((
      conn,
      session_response.session_id.into(),
      session_response.models,
      child,
    ))
  }

  /// Run the event loop on the ACP thread.
  ///
  /// Processes commands from the editor and sends results/errors via event_tx.
  async fn run_event_loop(
    conn: Arc<acp::ClientSideConnection>,
    mut command_rx: mpsc::UnboundedReceiver<AcpCommand>,
    event_tx: mpsc::UnboundedSender<StreamEvent>,
    _child: Child,
  ) {
    log::info!("ACP event loop started");

    while let Some(cmd) = command_rx.recv().await {
      match cmd {
        AcpCommand::Prompt { session_id, content } => {
          log::debug!("Processing prompt command");

          let result = conn
            .prompt(acp::PromptRequest {
              session_id,
              prompt: content,
              meta: None,
            })
            .await;

          match result {
            Ok(_response) => {
              // Response streaming is handled by EditorClient callbacks
              // The Done event will be sent by the client when streaming completes
              log::debug!("Prompt request completed");
            },
            Err(e) => {
              // Send error via event channel so it appears in the editor
              let _ = event_tx.send(StreamEvent::Error(format!("Prompt failed: {}", e)));
            },
          }
        },
        AcpCommand::SetModel { session_id, model_id } => {
          log::debug!("Processing set model command: {}", model_id);

          let result = conn
            .set_session_model(acp::SetSessionModelRequest {
              session_id,
              model_id: model_id.clone(),
              meta: None,
            })
            .await;

          match result {
            Ok(_response) => {
              log::info!("Model changed to: {}", model_id);
              let _ = event_tx.send(StreamEvent::ModelChanged(model_id));
            },
            Err(e) => {
              let _ = event_tx.send(StreamEvent::Error(format!("Failed to set model: {}", e)));
            },
          }
        },
        AcpCommand::Shutdown => {
          log::info!("ACP shutdown requested");
          break;
        },
      }
    }

    log::info!("ACP event loop ended");
  }

  /// Send a prompt to the agent (non-blocking).
  ///
  /// Returns immediately after queuing the request. Results and errors
  /// will arrive via `event_rx`, which should be polled in the main loop.
  pub fn prompt(&self, content: Vec<acp::ContentBlock>) -> Result<()> {
    let session_id = self
      .session_id
      .lock()
      .clone()
      .context("No active ACP session")?;

    self
      .command_tx
      .send(AcpCommand::Prompt { session_id, content })
      .map_err(|_| anyhow::anyhow!("ACP thread has shut down"))
  }

  /// Send a text prompt to the agent (non-blocking).
  pub fn prompt_text(&self, text: String) -> Result<()> {
    self.prompt(vec![text.into()])
  }

  /// Check if the connection is still active.
  pub fn is_connected(&self) -> bool {
    self.session_id.lock().is_some() && !self.command_tx.is_closed()
  }

  /// Get the current session ID.
  pub fn session_id(&self) -> Option<acp::SessionId> {
    self.session_id.lock().clone()
  }

  /// Get the current model state.
  pub fn model_state(&self) -> Option<acp::SessionModelState> {
    self.model_state.lock().clone()
  }

  /// Set the session model (non-blocking).
  ///
  /// The result will arrive via `event_rx` as either `ModelChanged` or `Error`.
  pub fn set_session_model(&self, model_id: acp::ModelId) -> Result<()> {
    let session_id = self
      .session_id
      .lock()
      .clone()
      .context("No active ACP session")?;

    self
      .command_tx
      .send(AcpCommand::SetModel { session_id, model_id })
      .map_err(|_| anyhow::anyhow!("ACP thread has shut down"))
  }

  /// Update the stored model state after a model change.
  ///
  /// Called from the main event loop when `ModelChanged` is received.
  pub fn update_current_model(&self, model_id: &acp::ModelId) {
    if let Some(ref mut state) = *self.model_state.lock() {
      state.current_model_id = model_id.clone();
    }
  }

  /// Try to receive the next streaming event without blocking.
  pub fn try_recv_event(&mut self) -> Option<StreamEvent> {
    self.event_rx.try_recv().ok()
  }

  /// Try to receive the next permission request without blocking.
  pub fn try_recv_permission(&mut self) -> Option<PendingPermission> {
    self.permission_rx.try_recv().ok()
  }
}

impl Drop for AcpHandle {
  fn drop(&mut self) {
    // Request shutdown of the ACP thread
    let _ = self.command_tx.send(AcpCommand::Shutdown);
  }
}

impl std::fmt::Debug for AcpHandle {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("AcpHandle")
      .field("session_id", &self.session_id.lock())
      .field("connected", &self.is_connected())
      .finish()
  }
}
