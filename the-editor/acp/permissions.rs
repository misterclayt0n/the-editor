//! Permission handling for ACP file operations.
//!
//! When an ACP agent wants to read or write files, we require user approval.
//! This module provides the types and logic for managing permission requests.

use std::path::PathBuf;

use tokio::sync::oneshot;

/// A pending permission request from the ACP agent.
#[derive(Debug)]
pub struct PendingPermission {
  /// Unique identifier for this permission request
  pub id: String,
  /// The kind of permission being requested
  pub kind: PermissionKind,
  /// Human-readable description of what the agent wants to do
  pub description: String,
  /// Channel to send the user's response
  pub response_tx: oneshot::Sender<bool>,
}

impl PendingPermission {
  /// Approve this permission request.
  pub fn approve(self) {
    let _ = self.response_tx.send(true);
  }

  /// Deny this permission request.
  pub fn deny(self) {
    let _ = self.response_tx.send(false);
  }

  /// Get a short summary for display in the statusline.
  pub fn short_summary(&self) -> String {
    match &self.kind {
      PermissionKind::ReadFile(path) => {
        format!("Read: {}", path.file_name().unwrap_or_default().to_string_lossy())
      },
      PermissionKind::WriteFile(path) => {
        format!("Write: {}", path.file_name().unwrap_or_default().to_string_lossy())
      },
      PermissionKind::CreateTerminal => "Create terminal".to_string(),
      PermissionKind::Other(desc) => desc.clone(),
    }
  }
}

/// The kind of permission being requested.
#[derive(Debug, Clone)]
pub enum PermissionKind {
  /// Read a file
  ReadFile(PathBuf),
  /// Write to a file
  WriteFile(PathBuf),
  /// Create a terminal
  CreateTerminal,
  /// Other permission type
  Other(String),
}

impl PermissionKind {
  /// Get the path associated with this permission, if any.
  pub fn path(&self) -> Option<&PathBuf> {
    match self {
      PermissionKind::ReadFile(p) | PermissionKind::WriteFile(p) => Some(p),
      _ => None,
    }
  }

  /// Check if this is a read operation.
  pub fn is_read(&self) -> bool {
    matches!(self, PermissionKind::ReadFile(_))
  }

  /// Check if this is a write operation.
  pub fn is_write(&self) -> bool {
    matches!(self, PermissionKind::WriteFile(_))
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
