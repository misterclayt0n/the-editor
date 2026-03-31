use serde_json::Value;

#[derive(Debug, Clone)]
pub enum LspCommand {
  Shutdown,
  RestartServer,
  AddWorkspaceFolder {
    uri:  String,
    name: String,
  },
  RemoveWorkspaceFolder {
    uri:  String,
    name: String,
  },
  CancelRequest {
    id: u64,
  },
  SendRequest {
    id:     u64,
    method: String,
    params: Option<Value>,
  },
  SendNotification {
    method: String,
    params: Option<Value>,
  },
}
