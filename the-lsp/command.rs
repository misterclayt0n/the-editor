use serde_json::Value;

#[derive(Debug, Clone)]
pub enum LspCommand {
  Shutdown,
  RestartServer,
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
