use std::{
  collections::HashSet,
  path::PathBuf,
  sync::Arc,
};

use agent_client_protocol::SessionId;

use super::Agent;
use crate::core::DocumentId;

/// Tracks permissions granted for a session
#[derive(Debug, Clone, Default)]
pub struct PermissionTracker {
  /// Files that can be read
  pub readable_files:  HashSet<PathBuf>,
  /// Files that can be written
  pub writable_files:  HashSet<PathBuf>,
  /// Whether all files in workspace can be read
  pub read_all:        bool,
  /// Whether all files in workspace can be written
  pub write_all:       bool,
  /// Whether terminal access is granted
  pub terminal_access: bool,
}

impl PermissionTracker {
  pub fn new() -> Self {
    Self::default()
  }

  /// Check if reading a file is permitted
  pub fn can_read(&self, path: &PathBuf) -> bool {
    self.read_all || self.readable_files.contains(path)
  }

  /// Check if writing a file is permitted
  pub fn can_write(&self, path: &PathBuf) -> bool {
    self.write_all || self.writable_files.contains(path)
  }

  /// Grant read access to a file
  pub fn grant_read(&mut self, path: PathBuf) {
    self.readable_files.insert(path);
  }

  /// Grant write access to a file
  pub fn grant_write(&mut self, path: PathBuf) {
    self.writable_files.insert(path);
  }

  /// Grant read access to all files
  pub fn grant_read_all(&mut self) {
    self.read_all = true;
  }

  /// Grant write access to all files
  pub fn grant_write_all(&mut self) {
    self.write_all = true;
  }
}

/// Message in the conversation history
#[derive(Debug, Clone)]
pub enum Message {
  User { content: String },
  Agent { content: String },
  System { content: String },
  Thought { content: String },
  ToolCall { name: String, args: String },
  ToolResult { content: String },
}

impl Message {
  /// Get the text content of the message for rendering
  pub fn content(&self) -> &str {
    match self {
      Message::User { content }
      | Message::Agent { content }
      | Message::System { content }
      | Message::Thought { content }
      | Message::ToolResult { content } => content,
      Message::ToolCall { name, args: _ } => {
        // This is a placeholder; we might want to format this better
        name
      },
    }
  }

  /// Get the role prefix for rendering
  pub fn role_prefix(&self) -> &str {
    match self {
      Message::User { .. } => "You",
      Message::Agent { .. } => "Agent",
      Message::System { .. } => "System",
      Message::Thought { .. } => "Thinking",
      Message::ToolCall { .. } => "Tool Call",
      Message::ToolResult { .. } => "Tool Result",
    }
  }
}

/// Represents an active ACP session
pub struct Session {
  pub session_id:  SessionId,
  pub agent:       Arc<Agent>,
  pub doc_id:      DocumentId,
  pub permissions: PermissionTracker,
  pub history:     Vec<Message>,
  pub is_active:   bool,
}

impl Session {
  pub fn new(session_id: SessionId, agent: Arc<Agent>, doc_id: DocumentId) -> Self {
    let mut permissions = PermissionTracker::new();
    // Grant read access to all workspace files by default (as per requirement)
    permissions.grant_read_all();

    Self {
      session_id,
      agent,
      doc_id,
      permissions,
      history: Vec::new(),
      is_active: true,
    }
  }

  /// Add a message to the history
  pub fn add_message(&mut self, message: Message) {
    self.history.push(message);
  }

  /// Get the conversation history
  pub fn history(&self) -> &[Message] {
    &self.history
  }
}
