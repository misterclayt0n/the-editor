mod client;
pub mod commands;
pub mod session;

use std::{
  cell::RefCell,
  collections::HashMap,
  path::PathBuf,
  rc::Rc,
  sync::Arc,
};

pub use agent_client_protocol as acp;
use acp::{
  Agent as _,
  ClientSideConnection,
  SessionId,
  SessionUpdate,
};
use thiserror::Error;
use tokio::sync::Mutex;

pub use self::{
  client::EditorClient,
  session::Session,
};

/// Notification from an ACP session to update the editor
#[derive(Debug, Clone)]
pub struct SessionNotification {
  pub session_id: SessionId,
  pub update:     SessionUpdate,
}

#[derive(Error, Debug)]
pub enum Error {
  #[error("ACP protocol error: {0}")]
  Acp(#[from] acp::Error),
  #[error("Agent not found: {0}")]
  AgentNotFound(String),
  #[error("Session not found: {0}")]
  SessionNotFound(String),
  #[error("IO Error: {0}")]
  IO(#[from] std::io::Error),
  #[error("Failed to spawn agent: {0}")]
  SpawnError(String),
  #[error(transparent)]
  Other(#[from] anyhow::Error),
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Configuration for an ACP agent
#[derive(Debug, Clone)]
pub struct AgentConfig {
  pub name:       String,
  pub command:    String,
  pub args:       Vec<String>,
  pub auto_start: bool,
}

impl Default for AgentConfig {
  fn default() -> Self {
    Self {
      name:       "claude-code".to_string(),
      command:    "claude-code-acp".to_string(),
      args:       vec![],
      auto_start: false,
    }
  }
}

/// Represents a running ACP agent connection
pub struct Agent {
  pub name:       String,
  pub config:     AgentConfig,
  pub connection: Arc<ClientSideConnection>,
  // Store the agent process handle to keep it alive
  #[allow(dead_code)]
  process:        tokio::process::Child,
}

/// Internal mutable state for Registry
struct RegistryState {
  /// Active agents by name
  agents:         HashMap<String, Arc<Agent>>,
  /// Active sessions by session ID
  sessions:       HashMap<SessionId, Session>,
  /// Reverse mapping: document ID -> session ID
  doc_to_session: HashMap<crate::core::DocumentId, SessionId>,
}

/// Registry for managing ACP agents and sessions
pub struct Registry {
  /// Mutable state protected by mutex for async access
  state:        Arc<Mutex<RegistryState>>,
  /// Agent configurations
  configs:      Vec<AgentConfig>,
  /// Notification queue for session updates (thread-local, !Send)
  notifications: Rc<RefCell<Vec<SessionNotification>>>,
}

impl Registry {
  pub fn new(configs: Vec<AgentConfig>) -> Self {
    Self {
      state: Arc::new(Mutex::new(RegistryState {
        agents:         HashMap::new(),
        sessions:       HashMap::new(),
        doc_to_session: HashMap::new(),
      })),
      configs,
      notifications: Rc::new(RefCell::new(Vec::new())),
    }
  }

  /// Get a handle to the registry for async operations
  pub fn handle(&self) -> RegistryHandle {
    RegistryHandle {
      state:        self.state.clone(),
      configs:      self.configs.clone(),
      notifications: self.notifications.clone(),
    }
  }

  /// Get the notification queue (for polling in the application loop)
  pub fn notifications(&self) -> &Rc<RefCell<Vec<SessionNotification>>> {
    &self.notifications
  }
}

/// Handle for async operations on the Registry
#[derive(Clone)]
pub struct RegistryHandle {
  state:        Arc<Mutex<RegistryState>>,
  configs:      Vec<AgentConfig>,
  notifications: Rc<RefCell<Vec<SessionNotification>>>,
}

impl RegistryHandle {
  /// Get or spawn an agent by name
  pub async fn get_or_spawn_agent(&self, name: &str) -> Result<Arc<Agent>> {
    log::info!("ACP Registry: get_or_spawn_agent({})", name);

    // Check if agent already exists
    {
      let state = self.state.lock().await;
      if let Some(agent) = state.agents.get(name) {
        log::info!("ACP Registry: Agent '{}' already exists, reusing", name);
        return Ok(agent.clone());
      }
    }

    log::info!("ACP Registry: Agent '{}' not found, spawning new one", name);

    // Find config for this agent
    let config = self
      .configs
      .iter()
      .find(|c| c.name == name)
      .ok_or_else(|| {
        log::error!("ACP Registry: No config found for agent '{}'", name);
        Error::AgentNotFound(name.to_string())
      })?
      .clone();

    log::info!("ACP Registry: Found config, command: {}", config.command);

    // Spawn the agent with notifications queue
    let agent = Arc::new(Self::spawn_agent(config, self.notifications.clone()).await?);

    log::info!("ACP Registry: Agent spawned successfully");

    // Insert into state
    {
      let mut state = self.state.lock().await;
      state.agents.insert(name.to_string(), agent.clone());
    }

    Ok(agent)
  }

  /// Spawn a new agent process
  async fn spawn_agent(
    config: AgentConfig,
    notifications: Rc<RefCell<Vec<SessionNotification>>>,
  ) -> Result<Agent> {
    log::info!("ACP Registry: spawn_agent - command: {}, args: {:?}", config.command, config.args);

    // Spawn the agent process
    let mut child = tokio::process::Command::new(&config.command)
      .args(&config.args)
      .stdin(std::process::Stdio::piped())
      .stdout(std::process::Stdio::piped())
      .kill_on_drop(true)
      .spawn()
      .map_err(|e| {
        log::error!("ACP Registry: Failed to spawn process '{}': {}", config.command, e);
        Error::SpawnError(format!("Failed to spawn {}: {}", config.command, e))
      })?;

    log::info!("ACP Registry: Process spawned, PID: {:?}", child.id());

    // Get stdin/stdout for communication
    let stdin = child
      .stdin
      .take()
      .ok_or_else(|| Error::SpawnError("Failed to get stdin".to_string()))?;
    let stdout = child
      .stdout
      .take()
      .ok_or_else(|| Error::SpawnError("Failed to get stdout".to_string()))?;

    // Convert to futures_io::AsyncRead/AsyncWrite using tokio_util compat
    use tokio_util::compat::{
      TokioAsyncReadCompatExt,
      TokioAsyncWriteCompatExt,
    };
    let outgoing = stdin.compat_write();
    let incoming = stdout.compat();

    // Create the client implementation with notifications queue
    let client = EditorClient::new(notifications);
    log::info!("ACP Registry: Created EditorClient");

    // Create the connection
    // Note: spawn_local will be used within a LocalSet context
    log::info!("ACP Registry: Creating ClientSideConnection");
    let (connection, handle_io) =
      ClientSideConnection::new(client, outgoing, incoming, |fut| {
        tokio::task::spawn_local(fut);
      });

    let connection = Arc::new(connection);
    log::info!("ACP Registry: ClientSideConnection created");

    // Spawn a task to handle IO
    tokio::task::spawn_local(handle_io);
    log::info!("ACP Registry: IO handler spawned");

    // Initialize the agent
    log::info!("ACP Registry: Sending initialize request");
    let init_response = connection
      .as_ref()
      .initialize(acp::InitializeRequest {
        protocol_version:    acp::V1,
        client_capabilities: acp::ClientCapabilities::default(),
        meta:                None,
      })
      .await?;

    log::info!(
      "ACP Registry: Initialized agent '{}': protocol v{:?}",
      config.name,
      init_response.protocol_version
    );

    Ok(Agent {
      name: config.name.clone(),
      config,
      connection,
      process: child,
    })
  }

  /// Create a new session with an agent
  pub async fn new_session(
    &self,
    agent_name: &str,
    doc_id: crate::core::DocumentId,
  ) -> Result<SessionId> {
    log::info!("ACP Registry: new_session - agent: {}, doc_id: {:?}", agent_name, doc_id);

    let agent = self.get_or_spawn_agent(agent_name).await?;

    // Request a new session from the agent
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    log::info!("ACP Registry: Requesting new session from agent, cwd: {:?}", cwd);

    let response = agent
      .connection
      .as_ref()
      .new_session(acp::NewSessionRequest {
        mcp_servers: Vec::new(),
        cwd,
        meta: None,
      })
      .await?;

    let session_id = response.session_id.clone();
    log::info!("ACP Registry: Agent returned session_id: {:?}", session_id);

    // Create session tracking
    let session = Session::new(session_id.clone(), agent.clone(), doc_id);

    {
      let mut state = self.state.lock().await;
      state.sessions.insert(session_id.clone(), session);
      state.doc_to_session.insert(doc_id, session_id.clone());
      log::info!("ACP Registry: Session tracked, {} active sessions", state.sessions.len());
    }

    Ok(session_id)
  }

  /// Send a prompt to a session
  pub async fn send_prompt(
    &self,
    session_id: &SessionId,
    prompt: Vec<acp::ContentBlock>,
  ) -> Result<acp::PromptResponse> {
    log::info!("ACP Registry: send_prompt - session_id: {:?}, {} content blocks", session_id, prompt.len());

    let agent = {
      let state = self.state.lock().await;
      let session = state
        .sessions
        .get(session_id)
        .ok_or_else(|| {
          log::error!("ACP Registry: Session {:?} not found", session_id);
          Error::SessionNotFound(session_id.0.to_string())
        })?;
      session.agent.clone()
    };

    log::info!("ACP Registry: Sending prompt request to agent");
    let response = agent
      .connection
      .as_ref()
      .prompt(acp::PromptRequest {
        session_id: session_id.clone(),
        prompt,
        meta: None,
      })
      .await?;

    log::info!("ACP Registry: Received prompt response");
    Ok(response)
  }

  /// Get session ID by document ID
  pub async fn get_session_id_by_doc(
    &self,
    doc_id: crate::core::DocumentId,
  ) -> Option<SessionId> {
    let state = self.state.lock().await;
    state.doc_to_session.get(&doc_id).cloned()
  }

  /// Close a session
  pub async fn close_session(&self, session_id: &SessionId) -> Result<()> {
    let mut state = self.state.lock().await;
    let session = state
      .sessions
      .remove(session_id)
      .ok_or_else(|| Error::SessionNotFound(session_id.0.to_string()))?;

    // Remove the doc->session mapping
    state.doc_to_session.remove(&session.doc_id);

    Ok(())
  }

  /// Shutdown an agent and all its sessions
  pub async fn shutdown_agent(&self, agent_name: &str) {
    let mut state = self.state.lock().await;
    // Remove all sessions for this agent
    state.sessions.retain(|_, session| session.agent.name != agent_name);

    // Remove the agent
    state.agents.remove(agent_name);
  }

  /// Shutdown all agents and sessions
  pub async fn shutdown_all(&self) {
    let mut state = self.state.lock().await;
    state.sessions.clear();
    state.agents.clear();
  }

  /// Add a message to a session's history
  pub async fn add_message_to_session(
    &self,
    session_id: &SessionId,
    message: crate::acp::session::Message,
  ) -> Result<()> {
    let mut state = self.state.lock().await;
    let session = state
      .sessions
      .get_mut(session_id)
      .ok_or_else(|| Error::SessionNotFound(session_id.0.to_string()))?;
    session.add_message(message);
    Ok(())
  }

  /// Get the document ID for a session
  pub async fn get_doc_id_by_session(&self, session_id: &SessionId) -> Option<crate::core::DocumentId> {
    let state = self.state.lock().await;
    state.sessions.get(session_id).map(|s| s.doc_id)
  }

  /// Update session state with a callback
  pub async fn update_session<F>(&self, session_id: &SessionId, f: F) -> Result<()>
  where
    F: FnOnce(&mut Session),
  {
    let mut state = self.state.lock().await;
    if let Some(session) = state.sessions.get_mut(session_id) {
      f(session);
      Ok(())
    } else {
      Err(Error::SessionNotFound(session_id.0.to_string()))
    }
  }
}
