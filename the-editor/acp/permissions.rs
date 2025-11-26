//! Permission handling for ACP tool calls.
//!
//! When an ACP agent wants to perform an operation that requires user approval,
//! it sends a permission request with a list of options. The user selects one
//! option, and we respond with the selected option's ID.

use std::sync::Arc;

use agent_client_protocol::{
  PermissionOption,
  PermissionOptionId,
  ToolCallUpdate,
};
use tokio::sync::oneshot;

/// A pending permission request from the ACP agent.
pub struct PendingPermission {
  /// The tool call requiring permission
  pub tool_call:   ToolCallUpdate,
  /// Available options for the user to choose from
  pub options:     Vec<PermissionOption>,
  /// Channel to send the user's selected option
  pub response_tx: oneshot::Sender<PermissionOptionId>,
}

impl std::fmt::Debug for PendingPermission {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("PendingPermission")
      .field("tool_call", &self.title())
      .field("options", &self.options.len())
      .finish()
  }
}

impl PendingPermission {
  /// Get the title of the tool call
  pub fn title(&self) -> &str {
    self
      .tool_call
      .fields
      .title
      .as_deref()
      .unwrap_or("Permission Request")
  }

  /// Get a short summary for display
  pub fn short_summary(&self) -> String {
    self.title().to_string()
  }

  /// Respond with the selected option
  pub fn respond(self, option_id: PermissionOptionId) {
    log::info!("[ACP] Permission response: {} -> {}", self.title(), option_id);
    let _ = self.response_tx.send(option_id);
  }

  /// Find an "allow" option (AllowOnce or AllowAlways)
  pub fn find_allow_option(&self) -> Option<&PermissionOption> {
    use agent_client_protocol::PermissionOptionKind;
    self
      .options
      .iter()
      .find(|o| matches!(o.kind, PermissionOptionKind::AllowOnce | PermissionOptionKind::AllowAlways))
  }

  /// Find a "reject" option (RejectOnce or RejectAlways)
  pub fn find_reject_option(&self) -> Option<&PermissionOption> {
    use agent_client_protocol::PermissionOptionKind;
    self
      .options
      .iter()
      .find(|o| matches!(o.kind, PermissionOptionKind::RejectOnce | PermissionOptionKind::RejectAlways))
  }

  /// Approve using the first available allow option
  pub fn approve(self) {
    if let Some(opt) = self.find_allow_option() {
      let id = opt.id.clone();
      self.respond(id);
    } else if let Some(first) = self.options.first() {
      // Fallback to first option
      let id = first.id.clone();
      self.respond(id);
    } else {
      // No options - create a synthetic "allow" response
      self.respond(PermissionOptionId(Arc::from("allow")));
    }
  }

  /// Deny using the first available reject option
  pub fn deny(self) {
    if let Some(opt) = self.find_reject_option() {
      let id = opt.id.clone();
      self.respond(id);
    } else if let Some(last) = self.options.last() {
      // Fallback to last option (often reject)
      let id = last.id.clone();
      self.respond(id);
    } else {
      // No options - create a synthetic "reject" response
      self.respond(PermissionOptionId(Arc::from("reject")));
    }
  }
}

/// Manager for pending permission requests.
///
/// This is stored in the Editor and tracks all pending permissions from the
/// ACP agent.
#[derive(Debug, Default)]
pub struct PermissionManager {
  /// Queue of pending permission requests
  pending: Vec<PendingPermission>,
}

impl PermissionManager {
  /// Create a new permission manager.
  pub fn new() -> Self {
    Self {
      pending: Vec::new(),
    }
  }

  /// Add a pending permission request.
  pub fn push(&mut self, permission: PendingPermission) {
    log::info!(
      "[ACP] Permission request added: {} ({} options)",
      permission.title(),
      permission.options.len()
    );
    self.pending.push(permission);
  }

  /// Get the next pending permission (if any).
  pub fn peek(&self) -> Option<&PendingPermission> {
    self.pending.first()
  }

  /// Take and return the next pending permission.
  pub fn pop(&mut self) -> Option<PendingPermission> {
    if self.pending.is_empty() {
      None
    } else {
      Some(self.pending.remove(0))
    }
  }

  /// Check if there are any pending permissions.
  pub fn has_pending(&self) -> bool {
    !self.pending.is_empty()
  }

  /// Get the count of pending permissions.
  pub fn pending_count(&self) -> usize {
    self.pending.len()
  }

  /// Approve the next pending permission.
  pub fn approve_next(&mut self) -> bool {
    if let Some(permission) = self.pop() {
      permission.approve();
      true
    } else {
      false
    }
  }

  /// Deny the next pending permission.
  pub fn deny_next(&mut self) -> bool {
    if let Some(permission) = self.pop() {
      permission.deny();
      true
    } else {
      false
    }
  }

  /// Deny all pending permissions.
  pub fn deny_all(&mut self) {
    while let Some(permission) = self.pop() {
      permission.deny();
    }
  }

  /// Approve all pending permissions.
  pub fn approve_all(&mut self) {
    while let Some(permission) = self.pop() {
      permission.approve();
    }
  }

  /// Get a reference to all pending permissions.
  pub fn pending_ref(&self) -> &[PendingPermission] {
    &self.pending
  }

  /// Approve a permission at a specific index.
  pub fn approve_at(&mut self, index: usize) -> bool {
    if index < self.pending.len() {
      let permission = self.pending.remove(index);
      permission.approve();
      true
    } else {
      false
    }
  }

  /// Deny a permission at a specific index.
  pub fn deny_at(&mut self, index: usize) -> bool {
    if index < self.pending.len() {
      let permission = self.pending.remove(index);
      permission.deny();
      true
    } else {
      false
    }
  }

  /// Format a status message showing pending permissions.
  pub fn status_message(&self) -> Option<String> {
    if self.pending.is_empty() {
      return None;
    }

    let first = &self.pending[0];
    let count = self.pending.len();

    if count == 1 {
      Some(format!("[ACP] {}", first.short_summary()))
    } else {
      Some(format!(
        "[ACP] {} (+{} more)",
        first.short_summary(),
        count - 1
      ))
    }
  }
}
