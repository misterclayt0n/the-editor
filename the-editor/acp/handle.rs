//! ACP connection handle management.
//!
//! This module provides `AcpHandle` which manages the lifecycle of the connection
//! to an ACP agent subprocess.

use std::{
  path::PathBuf,
  process::Stdio,
  sync::Arc,
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
  sync::mpsc,
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

/// Handle to an active ACP connection.
///
/// This struct manages the agent subprocess and provides methods to interact
/// with it. It is designed to be held in the `Editor` struct.
pub struct AcpHandle {
  /// The connection to the agent
  conn: Arc<acp::ClientSideConnection>,
  /// The current session ID
  session_id: Arc<Mutex<Option<acp::SessionId>>>,
  /// Receiver for streaming events from the agent
  pub event_rx: mpsc::UnboundedReceiver<StreamEvent>,
  /// Receiver for permission requests from the agent
  pub permission_rx: mpsc::UnboundedReceiver<PendingPermission>,
  /// The agent subprocess (kept alive)
  _child: Child,
}

impl AcpHandle {
  /// Start an ACP agent and establish a connection.
  ///
  /// This spawns the agent subprocess and initializes the ACP protocol.
  pub async fn start(config: &AcpConfig, cwd: PathBuf) -> Result<Self> {
    if config.command.is_empty() {
      bail!("ACP command is empty");
    }

    let program = &config.command[0];
    let args = &config.command[1..];

    log::info!("Starting ACP agent: {} {:?}", program, args);

    // Spawn the agent process
    let mut child = Command::new(program)
      .args(args)
      .current_dir(&cwd)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::inherit()) // Let agent stderr go to editor's stderr for debugging
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

    // Create channels for communication with the EditorClient
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (permission_tx, permission_rx) = mpsc::unbounded_channel();

    // Create the client
    let client = EditorClient::new(event_tx, permission_tx);

    // Create the connection
    // Note: ACP futures are !Send, so we use spawn_local
    let (conn, handle_io) = acp::ClientSideConnection::new(client, stdin, stdout, |fut| {
      tokio::task::spawn_local(fut);
    });

    let conn = Arc::new(conn);

    // Spawn the I/O handler
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

    Ok(Self {
      conn,
      session_id: Arc::new(Mutex::new(Some(session_response.session_id.into()))),
      event_rx,
      permission_rx,
      _child: child,
    })
  }

  /// Send a prompt to the agent.
  ///
  /// The response will be streamed via `event_rx`.
  pub async fn prompt(&self, content: Vec<acp::ContentBlock>) -> Result<()> {
    let session_id = self
      .session_id
      .lock()
      .clone()
      .context("No active ACP session")?;

    self
      .conn
      .prompt(acp::PromptRequest {
        session_id: session_id.clone(),
        prompt: content,
        meta: None,
      })
      .await
      .context("Failed to send prompt to agent")?;

    Ok(())
  }

  /// Send a text prompt to the agent.
  ///
  /// This is a convenience wrapper around `prompt` for simple text prompts.
  pub async fn prompt_text(&self, text: String) -> Result<()> {
    self.prompt(vec![text.into()]).await
  }

  /// Check if the connection is still active.
  pub fn is_connected(&self) -> bool {
    self.session_id.lock().is_some()
  }

  /// Get the current session ID.
  pub fn session_id(&self) -> Option<acp::SessionId> {
    self.session_id.lock().clone()
  }

  /// Get a reference to the connection.
  pub fn conn(&self) -> &Arc<acp::ClientSideConnection> {
    &self.conn
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

impl std::fmt::Debug for AcpHandle {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("AcpHandle")
      .field("session_id", &self.session_id.lock())
      .field("connected", &self.is_connected())
      .finish()
  }
}
