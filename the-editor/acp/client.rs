//! ACP Client implementation for the-editor.
//!
//! This module implements the `acp::Client` trait, which defines how the editor
//! responds to requests and notifications from the ACP agent.

use agent_client_protocol as acp;
use tokio::sync::{mpsc, oneshot};

use super::{PendingPermission, StreamEvent, ToolCallStatus};

/// The editor's ACP client implementation.
///
/// This struct is passed to `ClientSideConnection` and handles all callbacks
/// from the ACP agent. It communicates with the main editor thread via
/// channels.
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
}

#[async_trait::async_trait(?Send)]
impl acp::Client for EditorClient {
  /// Handle permission requests from the agent.
  ///
  /// This is the main permission flow - when the agent wants to perform an
  /// operation that requires user approval, it sends a request with options.
  async fn request_permission(
    &self,
    args: acp::RequestPermissionRequest,
  ) -> acp::Result<acp::RequestPermissionResponse> {
    let title = args
      .tool_call
      .fields
      .title
      .as_deref()
      .unwrap_or("Permission Request");
    log::info!(
      "[ACP] Permission request: {} ({} options)",
      title,
      args.options.len()
    );

    let (response_tx, response_rx) = oneshot::channel();

    let permission = PendingPermission {
      tool_call: args.tool_call,
      options: args.options,
      response_tx,
    };

    // Send permission request to editor
    if self.permission_tx.send(permission).is_err() {
      log::error!("[ACP] Failed to send permission request - channel closed");
      return Err(acp::Error {
        code: -32603,
        message: "Internal error: permission channel closed".into(),
        data: None,
      });
    }

    // Wait for user response
    match response_rx.await {
      Ok(option_id) => {
        log::info!("[ACP] Permission response received: {}", option_id);
        Ok(acp::RequestPermissionResponse {
          outcome: acp::RequestPermissionOutcome::Selected { option_id },
          meta: None,
        })
      },
      Err(_) => {
        log::error!("[ACP] Permission response channel closed");
        Ok(acp::RequestPermissionResponse {
          outcome: acp::RequestPermissionOutcome::Cancelled,
          meta: None,
        })
      },
    }
  }

  /// Handle file write requests from the agent.
  async fn write_text_file(
    &self,
    args: acp::WriteTextFileRequest,
  ) -> acp::Result<acp::WriteTextFileResponse> {
    log::info!("[ACP] Write file request: {:?}", args.path);

    // Create parent directories if needed
    if let Some(parent) = args.path.parent() {
      if let Err(e) = tokio::fs::create_dir_all(parent).await {
        return Err(acp::Error {
          code: -32603,
          message: format!("Failed to create directories: {}", e),
          data: None,
        });
      }
    }

    // Write the file
    if let Err(e) = tokio::fs::write(&args.path, &args.content).await {
      return Err(acp::Error {
        code: -32603,
        message: format!("Failed to write file: {}", e),
        data: None,
      });
    }

    log::info!("[ACP] Wrote file {:?}", args.path);
    Ok(acp::WriteTextFileResponse { meta: None })
  }

  /// Handle file read requests from the agent.
  async fn read_text_file(
    &self,
    args: acp::ReadTextFileRequest,
  ) -> acp::Result<acp::ReadTextFileResponse> {
    log::info!("[ACP] Read file request: {:?}", args.path);

    // Read the file
    let content = match tokio::fs::read_to_string(&args.path).await {
      Ok(content) => content,
      Err(e) => {
        return Err(acp::Error {
          code: -32603,
          message: format!("Failed to read file: {}", e),
          data: None,
        });
      },
    };

    log::info!("[ACP] Read file {:?}", args.path);
    Ok(acp::ReadTextFileResponse {
      content,
      meta: None,
    })
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

  /// Handle session notifications - this is where streaming responses come
  /// through.
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
        // Convert ACP ToolKind to string for display
        let kind = match tool_call.kind {
          acp::ToolKind::Read => Some("read".to_string()),
          acp::ToolKind::Edit => Some("edit".to_string()),
          acp::ToolKind::Delete => Some("delete".to_string()),
          acp::ToolKind::Move => Some("move".to_string()),
          acp::ToolKind::Search => Some("search".to_string()),
          acp::ToolKind::Execute => Some("execute".to_string()),
          acp::ToolKind::Think => Some("think".to_string()),
          acp::ToolKind::Fetch => Some("fetch".to_string()),
          acp::ToolKind::SwitchMode => Some("switch_mode".to_string()),
          acp::ToolKind::Other => None,
        };

        let _ = self.event_tx.send(StreamEvent::ToolCall {
          title: tool_call.title,
          kind,
          raw_input: tool_call.raw_input,
          status: ToolCallStatus::Started,
        });
      },
      acp::SessionUpdate::ToolCallUpdate(update) => {
        log::debug!("[ACP] Tool call update: {:?}", update);

        // Extract error message from content if present (for failed tool calls)
        let error_msg = update.fields.content.as_ref().and_then(|contents| {
          for content in contents {
            if let acp::ToolCallContent::Content { content } = content {
              if let acp::ContentBlock::Text(text_content) = content {
                if !text_content.text.is_empty() {
                  return Some(text_content.text.clone());
                }
              }
            }
          }
          None
        });

        // Send tool call updates with new status
        let status = match update.fields.status {
          Some(acp::ToolCallStatus::Completed) => Some(ToolCallStatus::Completed),
          Some(acp::ToolCallStatus::Failed) => {
            Some(ToolCallStatus::Failed(error_msg.unwrap_or_default()))
          },
          Some(acp::ToolCallStatus::InProgress) => Some(ToolCallStatus::InProgress(String::new())),
          Some(acp::ToolCallStatus::Pending) | None => None,
        };

        if let Some(status) = status {
          // Get the title from the update or use a placeholder
          let title = update.fields.title.unwrap_or_default();
          let kind = update.fields.kind.map(|k| match k {
            acp::ToolKind::Read => "read".to_string(),
            acp::ToolKind::Edit => "edit".to_string(),
            acp::ToolKind::Delete => "delete".to_string(),
            acp::ToolKind::Move => "move".to_string(),
            acp::ToolKind::Search => "search".to_string(),
            acp::ToolKind::Execute => "execute".to_string(),
            acp::ToolKind::Think => "think".to_string(),
            acp::ToolKind::Fetch => "fetch".to_string(),
            acp::ToolKind::SwitchMode => "switch_mode".to_string(),
            acp::ToolKind::Other => "other".to_string(),
          });

          let _ = self.event_tx.send(StreamEvent::ToolCall {
            title,
            kind,
            raw_input: update.fields.raw_input,
            status,
          });
        }
      },
      acp::SessionUpdate::AgentThoughtChunk { .. } => {
        // Agent thinking - could display in status or ignore
      },
      acp::SessionUpdate::UserMessageChunk { .. } => {
        // Echo of user message - ignore
      },
      acp::SessionUpdate::Plan(plan) => {
        // Forward plan updates to the editor for display
        let _ = self.event_tx.send(StreamEvent::PlanUpdate(plan));
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
