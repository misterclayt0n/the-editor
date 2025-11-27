//! ACP (Agent Client Protocol) integration for the-editor.
//!
//! This module provides integration with ACP-compatible AI coding agents,
//! allowing the editor to communicate with agents like OpenCode for
//! AI-assisted code editing.
//!
//! ## Architecture
//!
//! - `client.rs` - Implements `acp::Client` trait to handle agent callbacks
//! - `handle.rs` - Manages the connection lifecycle with the agent
//! - `context.rs` - Builds and visualizes context sent to agents
//! - `permissions.rs` - Handles permission requests from agents
//!
//! ## Usage Flow
//!
//! 1. User starts agent with `:acp-start` command
//! 2. User selects text and triggers `acp_prompt` (e.g., `<space>a`)
//! 3. Selection is sent to agent as a prompt
//! 4. Agent streams response back, which is inserted after selection
//! 5. If agent requests file operations, user is prompted for permission

mod client;
mod context;
mod handle;
mod permissions;

use std::path::PathBuf;

pub use client::EditorClient;
pub use context::{
  ContextVisualizer,
  PromptContext,
};
pub use handle::AcpHandle;
pub use permissions::{
  PendingPermission,
  PermissionManager,
};
use serde::{
  Deserialize,
  Serialize,
};

/// Configuration for ACP integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct AcpConfig {
  /// Command to spawn ACP agent (e.g., ["opencode", "acp"])
  pub command:       Vec<String>,
  /// Working directory for agent (defaults to workspace root)
  pub cwd:           Option<PathBuf>,
  /// Number of context lines to include before/after selection
  pub context_lines: usize,
  /// Auto-start agent on editor launch
  pub auto_start:    bool,
  /// Request timeout in seconds
  pub timeout_secs:  u64,
}

impl Default for AcpConfig {
  fn default() -> Self {
    Self {
      command:       vec!["opencode".into(), "acp".into()],
      cwd:           None,
      context_lines: 50,
      auto_start:    false,
      timeout_secs:  120,
    }
  }
}

/// Events streamed from the ACP agent.
#[derive(Debug, Clone)]
pub enum StreamEvent {
  /// A chunk of text from the agent's response
  TextChunk(String),
  /// Agent is making a tool call
  ToolCall {
    /// Human-readable title of the tool call
    title:     String,
    /// Tool kind (read, edit, execute, etc.)
    kind:      Option<String>,
    /// Raw input parameters as JSON
    raw_input: Option<serde_json::Value>,
    /// Current status of the tool call
    status:    ToolCallStatus,
  },
  /// Agent's execution plan (TODOs) has been updated
  PlanUpdate(agent_client_protocol::Plan),
  /// Agent has finished responding
  Done,
  /// An error occurred
  Error(String),
  /// Model was successfully changed
  ModelChanged(agent_client_protocol::ModelId),
}

/// Status of a tool call.
#[derive(Debug, Clone)]
pub enum ToolCallStatus {
  Started,
  InProgress(String),
  Completed,
  Failed(String),
}
