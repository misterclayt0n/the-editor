//! ACP Client implementation for the-editor.
//!
//! This module implements the `acp::Client` trait, which defines how the editor
//! responds to requests and notifications from the ACP agent.

use std::sync::atomic::{
  AtomicU64,
  Ordering,
};

use agent_client_protocol as acp;
use tokio::sync::{
  mpsc,
  oneshot,
};

use super::{
  PendingPermission,
  PermissionKind,
  StreamEvent,
  ToolCallStatus,
};

/// Counter for generating unique permission request IDs.
static PERMISSION_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique permission request ID.
fn next_permission_id() -> String {
  let id = PERMISSION_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
  format!("perm-{}", id)
}

/// The editor's ACP client implementation.
///
/// This struct is passed to `ClientSideConnection` and handles all callbacks
/// from the ACP agent. It communicates with the main editor thread via channels.
pub struct EditorClient {
  /// Channel to send streaming events back to the editor
  event_tx: mpsc::UnboundedSender<StreamEvent>,
  /// Channel to send permission requests to the editor
  permission_tx: mpsc::UnboundedSender<PendingPermission>,
}

impl EditorClient {
  /// Create a new EditorClient with the given channels.
  pub fn new(
    event_tx: mpsc::UnboundedSender<StreamEvent>,
    permission_tx: mpsc::UnboundedSender<PendingPermission>,
  ) -> Self {
    Self {
      event_tx,
      permission_tx,
    }
  }

  /// Request permission from the user for a file operation.
  async fn request_permission(&self, kind: PermissionKind, description: String) -> bool {
    let (response_tx, response_rx) = oneshot::channel();

    let permission = PendingPermission {
      id: next_permission_id(),
      kind,
      description,
      response_tx,
    };

    // Send permission request to editor
    if self.permission_tx.send(permission).is_err() {
      log::error!("Failed to send permission request - channel closed");
      return false;
    }

    // Wait for user response
    match response_rx.await {
      Ok(approved) => approved,
      Err(_) => {
        log::error!("Permission response channel closed");
        false
      },
    }
  }
}

#[async_trait::async_trait(?Send)]
impl acp::Client for EditorClient {
  /// Handle permission requests from the agent.
  async fn request_permission(
    &self,
    _args: acp::RequestPermissionRequest,
  ) -> acp::Result<acp::RequestPermissionResponse> {
    // For now, we handle permissions at the operation level (read/write file)
    // rather than through the generic permission request
    Err(acp::Error::method_not_found())
  }

  /// Handle file write requests from the agent.
  async fn write_text_file(
    &self,
    args: acp::WriteTextFileRequest,
  ) -> acp::Result<acp::WriteTextFileResponse> {
    let path = args.path.clone();
    let description = format!("Agent wants to write to: {}", path.display());

    let approved = self
      .request_permission(PermissionKind::WriteFile(path.clone()), description)
      .await;

    if !approved {
      return Err(acp::Error {
        code:    -32001,
        message: "Permission denied by user".into(),
        data:    None,
      });
    }

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
      if let Err(e) = tokio::fs::create_dir_all(parent).await {
        return Err(acp::Error {
          code:    -32603,
          message: format!("Failed to create directories: {}", e),
          data:    None,
        });
      }
    }

    // Write the file
    if let Err(e) = tokio::fs::write(&path, &args.content).await {
      return Err(acp::Error {
        code:    -32603,
        message: format!("Failed to write file: {}", e),
        data:    None,
      });
    }

    log::info!("ACP: Wrote file {:?}", args.path);
    Ok(acp::WriteTextFileResponse { meta: None })
  }

  /// Handle file read requests from the agent.
  async fn read_text_file(
    &self,
    args: acp::ReadTextFileRequest,
  ) -> acp::Result<acp::ReadTextFileResponse> {
    let path = args.path.clone();
    let description = format!("Agent wants to read: {}", path.display());

    let approved = self
      .request_permission(PermissionKind::ReadFile(path.clone()), description)
      .await;

    if !approved {
      return Err(acp::Error {
        code:    -32001,
        message: "Permission denied by user".into(),
        data:    None,
      });
    }

    // Read the file
    let content = match tokio::fs::read_to_string(&path).await {
      Ok(content) => content,
      Err(e) => {
        return Err(acp::Error {
          code:    -32603,
          message: format!("Failed to read file: {}", e),
          data:    None,
        });
      },
    };

    log::info!("ACP: Read file {:?}", args.path);
    Ok(acp::ReadTextFileResponse { content, meta: None })
  }

  /// Handle terminal creation requests.
  async fn create_terminal(
    &self,
    _args: acp::CreateTerminalRequest,
  ) -> acp::Result<acp::CreateTerminalResponse> {
    // Terminal support is not implemented yet
    Err(acp::Error::method_not_found())
  }

  /// Handle terminal output requests.
  async fn terminal_output(
    &self,
    _args: acp::TerminalOutputRequest,
  ) -> acp::Result<acp::TerminalOutputResponse> {
    Err(acp::Error::method_not_found())
  }

  /// Handle terminal release requests.
  async fn release_terminal(
    &self,
    _args: acp::ReleaseTerminalRequest,
  ) -> acp::Result<acp::ReleaseTerminalResponse> {
    Err(acp::Error::method_not_found())
  }

  /// Handle terminal exit wait requests.
  async fn wait_for_terminal_exit(
    &self,
    _args: acp::WaitForTerminalExitRequest,
  ) -> acp::Result<acp::WaitForTerminalExitResponse> {
    Err(acp::Error::method_not_found())
  }

  /// Handle terminal kill requests.
  async fn kill_terminal_command(
    &self,
    _args: acp::KillTerminalCommandRequest,
  ) -> acp::Result<acp::KillTerminalCommandResponse> {
    Err(acp::Error::method_not_found())
  }

  /// Handle session notifications - this is where streaming responses come through.
  async fn session_notification(&self, args: acp::SessionNotification) -> acp::Result<()> {
    match args.update {
      acp::SessionUpdate::AgentMessageChunk(chunk) => {
        // Extract text from content block
        let text = match chunk.content {
          acp::ContentBlock::Text(text_content) => text_content.text,
          acp::ContentBlock::Image(_) => "[image]".into(),
          acp::ContentBlock::Audio(_) => "[audio]".into(),
          acp::ContentBlock::ResourceLink(link) => format!("[link: {}]", link.uri),
          acp::ContentBlock::Resource(_) => "[resource]".into(),
        };

        if !text.is_empty() {
          let _ = self.event_tx.send(StreamEvent::TextChunk(text));
        }
      },
      acp::SessionUpdate::ToolCall(tool_call) => {
        let _ = self.event_tx.send(StreamEvent::ToolCall {
          name:   tool_call.title,
          status: ToolCallStatus::Started,
        });
      },
      acp::SessionUpdate::ToolCallUpdate(update) => {
        // Log tool call progress
        log::debug!("ACP tool call update: {:?}", update);
      },
      acp::SessionUpdate::AgentThoughtChunk { .. } => {
        // Agent thinking - could display in status or ignore
      },
      acp::SessionUpdate::UserMessageChunk { .. } => {
        // Echo of user message - ignore
      },
      acp::SessionUpdate::Plan(_) => {
        // Agent planning - could display or ignore
      },
      acp::SessionUpdate::CurrentModeUpdate { .. } => {
        // Mode changes
      },
      acp::SessionUpdate::AvailableCommandsUpdate { .. } => {
        // Available commands changed
      },
    }

    Ok(())
  }

  /// Handle extension methods.
  async fn ext_method(&self, _args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
    Err(acp::Error::method_not_found())
  }

  /// Handle extension notifications.
  async fn ext_notification(&self, _args: acp::ExtNotification) -> acp::Result<()> {
    Ok(())
  }
}
