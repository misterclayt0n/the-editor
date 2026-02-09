use std::path::PathBuf;

use the_lib::diagnostics::DocumentDiagnostics;

use crate::jsonrpc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspProgress {
  pub token:      String,
  pub kind:       LspProgressKind,
  pub title:      Option<String>,
  pub message:    Option<String>,
  pub percentage: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspProgressKind {
  Begin,
  Report,
  End,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LspEvent {
  Started {
    workspace_root: PathBuf,
  },
  ServerStarted {
    server_name: String,
    command:     String,
    args:        Vec<String>,
  },
  ServerStopped {
    exit_code: Option<i32>,
  },
  CapabilitiesRegistered {
    server_name: String,
  },
  RequestDispatched {
    id:     u64,
    method: String,
  },
  RequestCompleted {
    id: u64,
  },
  RequestTimedOut {
    id:     u64,
    method: String,
  },
  DiagnosticsPublished {
    diagnostics: DocumentDiagnostics,
  },
  Progress {
    progress: LspProgress,
  },
  RpcMessage {
    message: jsonrpc::Message,
  },
  ServerStderr {
    line: String,
  },
  Stopped,
  Error(String),
}
