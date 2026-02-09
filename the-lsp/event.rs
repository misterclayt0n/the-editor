use std::path::PathBuf;

use crate::jsonrpc;

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
  RpcMessage {
    message: jsonrpc::Message,
  },
  ServerStderr {
    line: String,
  },
  Stopped,
  Error(String),
}
