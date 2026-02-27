use std::{
  collections::{
    HashMap,
    HashSet,
    VecDeque,
  },
  env,
  fs,
  path::{
    Path,
    PathBuf,
  },
  sync::{
    Mutex,
    OnceLock,
    atomic::{
      AtomicU64,
      Ordering,
    },
  },
};

use serde_json::Value;
use the_lib::diagnostics::DocumentDiagnostics;
use the_lsp::{
  LspEvent,
  LspRuntime,
  LspRuntimeConfig,
  ServerCapabilitiesSnapshot,
  jsonrpc,
  text_sync::{
    did_close_params,
    did_open_params,
  },
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionKey {
  workspace_root: String,
  server_name: String,
  command: String,
  args: Vec<String>,
  env: Vec<(String, String)>,
  initialize_options: Option<String>,
}

impl SessionKey {
  pub fn from_runtime_config(config: &LspRuntimeConfig) -> Option<Self> {
    let server = config.server()?;
    Some(Self {
      workspace_root: normalize_workspace_root(config.workspace_root()).to_string_lossy().into(),
      server_name: server.name().to_string(),
      command: server.command().to_string(),
      args: server.args().to_vec(),
      env: server.env().to_vec(),
      initialize_options: server
        .initialize_options()
        .and_then(|value| serde_json::to_string(value).ok()),
    })
  }
}

#[derive(Debug, Clone, Default)]
pub struct BrokerCounters {
  pub sessions_created: u64,
  pub runtime_starts: u64,
}

#[derive(Default)]
struct LspBrokerRegistry {
  sessions: HashMap<SessionKey, LspBrokerSession>,
  counters: BrokerCounters,
}

impl LspBrokerRegistry {
  fn get_or_create_session(
    &mut self,
    key: SessionKey,
    config: LspRuntimeConfig,
  ) -> Result<&mut LspBrokerSession, String> {
    if !self.sessions.contains_key(&key) {
      let mut session = LspBrokerSession::new(config);
      let server_name = session
        .runtime
        .config()
        .server()
        .map(|server| server.name().to_string());
      if let Some(server_name) = server_name {
        session
          .runtime
          .start()
          .map_err(|err| format!("failed to start broker runtime '{server_name}': {err}"))?;
        self.counters.runtime_starts = self.counters.runtime_starts.saturating_add(1);
      }
      self.sessions.insert(key.clone(), session);
      self.counters.sessions_created = self.counters.sessions_created.saturating_add(1);
    }

    // Safe by construction: inserted above when missing.
    Ok(self.sessions.get_mut(&key).expect("session must exist"))
  }

  fn remove_empty_session(&mut self, key: &SessionKey) {
    let should_remove = self
      .sessions
      .get(key)
      .is_some_and(|session| session.clients.is_empty());
    if !should_remove {
      return;
    }

    if let Some(mut session) = self.sessions.remove(key) {
      let _ = session.runtime.shutdown();
    }
  }

  fn counters(&self) -> BrokerCounters {
    self.counters.clone()
  }
}

#[derive(Default)]
struct ClientState {
  inbox: VecDeque<LspEvent>,
  subscribed_uris: HashSet<String>,
}

#[derive(Default)]
struct UriBinding {
  subscribers: HashSet<u64>,
  owner: Option<u64>,
}

struct LspBrokerSession {
  runtime: LspRuntime,
  clients: HashMap<u64, ClientState>,
  runtime_to_client_request: HashMap<u64, (u64, u64)>,
  client_to_runtime_request: HashMap<(u64, u64), u64>,
  uri_bindings: HashMap<String, UriBinding>,
}

impl LspBrokerSession {
  fn new(config: LspRuntimeConfig) -> Self {
    Self {
      runtime: LspRuntime::new(config),
      clients: HashMap::new(),
      runtime_to_client_request: HashMap::new(),
      client_to_runtime_request: HashMap::new(),
      uri_bindings: HashMap::new(),
    }
  }

  fn register_client(&mut self, client_id: u64) {
    trace_log("register_client", format!("client_id={client_id}"));
    self.clients.entry(client_id).or_default();
  }

  fn detach_client(&mut self, client_id: u64) {
    self.poll_runtime_events();

    if let Some(client) = self.clients.remove(&client_id) {
      for uri in client.subscribed_uris {
        self.remove_uri_subscriber(client_id, uri.as_str());
      }
    }

    let pending = self
      .client_to_runtime_request
      .keys()
      .filter_map(|(owner, client_req)| (*owner == client_id).then_some(*client_req))
      .collect::<Vec<_>>();
    for client_request_id in pending {
      let _ = self.cancel_request(client_id, client_request_id);
    }

    let runtime_ids = self
      .runtime_to_client_request
      .iter()
      .filter_map(|(runtime_id, (owner, _))| (*owner == client_id).then_some(*runtime_id))
      .collect::<Vec<_>>();
    for runtime_id in runtime_ids {
      self.runtime_to_client_request.remove(&runtime_id);
    }
  }

  fn poll_runtime_events(&mut self) {
    while let Some(event) = self.runtime.try_recv_event() {
      match event {
        LspEvent::RequestDispatched { id, method } => {
          if let Some((client_id, client_request_id)) =
            self.runtime_to_client_request.get(&id).copied()
          {
            self.enqueue_client(
              client_id,
              LspEvent::RequestDispatched {
                id: client_request_id,
                method,
              },
            );
          } else {
            self.broadcast(LspEvent::RequestDispatched { id, method });
          }
        },
        LspEvent::RequestCompleted { id } => {
          if let Some((client_id, client_request_id)) = self.runtime_to_client_request.remove(&id)
          {
            self
              .client_to_runtime_request
              .remove(&(client_id, client_request_id));
            self.enqueue_client(
              client_id,
              LspEvent::RequestCompleted {
                id: client_request_id,
              },
            );
          } else {
            self.broadcast(LspEvent::RequestCompleted { id });
          }
        },
        LspEvent::RequestTimedOut { id, method } => {
          if let Some((client_id, client_request_id)) = self.runtime_to_client_request.remove(&id)
          {
            self
              .client_to_runtime_request
              .remove(&(client_id, client_request_id));
            self.enqueue_client(
              client_id,
              LspEvent::RequestTimedOut {
                id: client_request_id,
                method,
              },
            );
          } else {
            self.broadcast(LspEvent::RequestTimedOut { id, method });
          }
        },
        LspEvent::RpcMessage { message } => {
          self.route_rpc_message(message);
        },
        LspEvent::DiagnosticsPublished { diagnostics } => {
          self.fanout_diagnostics(diagnostics);
        },
        LspEvent::WorkspaceApplyEdit { label, edit } => {
          if let Some(client_id) = self.resolve_workspace_edit_client(&edit.documents) {
            self.enqueue_client(client_id, LspEvent::WorkspaceApplyEdit { label, edit });
          }
        },
        other => {
          self.broadcast(other);
        },
      }
    }
  }

  fn route_rpc_message(&mut self, message: jsonrpc::Message) {
    let jsonrpc::Message::Response(mut response) = message else {
      self.broadcast(LspEvent::RpcMessage { message });
      return;
    };

    let jsonrpc::Id::Number(runtime_id) = response.id else {
      self.broadcast(LspEvent::RpcMessage {
        message: jsonrpc::Message::Response(response),
      });
      return;
    };

    let Some((client_id, client_request_id)) = self.runtime_to_client_request.remove(&runtime_id)
    else {
      self.broadcast(LspEvent::RpcMessage {
        message: jsonrpc::Message::Response(response),
      });
      return;
    };

    self
      .client_to_runtime_request
      .remove(&(client_id, client_request_id));
    response.id = jsonrpc::Id::Number(client_request_id);
    self.enqueue_client(
      client_id,
      LspEvent::RpcMessage {
        message: jsonrpc::Message::Response(response),
      },
    );
  }

  fn fanout_diagnostics(&mut self, diagnostics: DocumentDiagnostics) {
    let uri = diagnostics.uri.clone();
    let subscribers = self
      .uri_bindings
      .get(uri.as_str())
      .map(|binding| binding.subscribers.iter().copied().collect::<Vec<_>>())
      .unwrap_or_default();

    if subscribers.is_empty() {
      self.broadcast(LspEvent::DiagnosticsPublished { diagnostics });
      return;
    }

    for client_id in subscribers {
      self.enqueue_client(
        client_id,
        LspEvent::DiagnosticsPublished {
          diagnostics: diagnostics.clone(),
        },
      );
    }
  }

  fn resolve_workspace_edit_client(
    &self,
    documents: &[the_lsp::LspDocumentEdit],
  ) -> Option<u64> {
    for document in documents {
      if let Some(binding) = self.uri_bindings.get(document.uri.as_str()) {
        if let Some(owner) = binding.owner {
          return Some(owner);
        }
        if let Some(candidate) = binding.subscribers.iter().copied().next() {
          return Some(candidate);
        }
      }
    }

    self.clients.keys().min().copied()
  }

  fn enqueue_client(&mut self, client_id: u64, event: LspEvent) {
    if let Some(client) = self.clients.get_mut(&client_id) {
      client.inbox.push_back(event);
    }
  }

  fn broadcast(&mut self, event: LspEvent) {
    let recipients = self.clients.keys().copied().collect::<Vec<_>>();
    for client_id in recipients {
      self.enqueue_client(client_id, event.clone());
    }
  }

  fn drain_client_events(&mut self, client_id: u64) -> Vec<LspEvent> {
    self.poll_runtime_events();
    let Some(client) = self.clients.get_mut(&client_id) else {
      return Vec::new();
    };
    client.inbox.drain(..).collect()
  }

  fn send_request(
    &mut self,
    client_id: u64,
    client_request_id: u64,
    method: &str,
    params: Option<Value>,
  ) -> Result<(), String> {
    self.poll_runtime_events();
    let runtime_request_id = self
      .runtime
      .send_request(method.to_string(), params)
      .map_err(|err| format!("failed to dispatch {method}: {err}"))?;
    trace_log(
      "send_request",
      format!(
        "client_id={} client_req_id={} runtime_req_id={} method={}",
        client_id, client_request_id, runtime_request_id, method
      ),
    );
    self.runtime_to_client_request
      .insert(runtime_request_id, (client_id, client_request_id));
    self
      .client_to_runtime_request
      .insert((client_id, client_request_id), runtime_request_id);
    Ok(())
  }

  fn send_notification(&mut self, method: &str, params: Option<Value>) -> Result<(), String> {
    self.poll_runtime_events();
    self
      .runtime
      .send_notification(method.to_string(), params)
      .map_err(|err| format!("failed to send {method}: {err}"))
  }

  fn cancel_request(&mut self, client_id: u64, client_request_id: u64) -> Result<(), String> {
    self.poll_runtime_events();

    let Some(runtime_request_id) = self
      .client_to_runtime_request
      .remove(&(client_id, client_request_id))
    else {
      return Ok(());
    };

    self.runtime_to_client_request.remove(&runtime_request_id);
    self
      .runtime
      .cancel_request(runtime_request_id)
      .map_err(|err| format!("failed to cancel request {client_request_id}: {err}"))
  }

  fn focus_document(
    &mut self,
    client_id: u64,
    uri: &str,
    open_payload: Value,
  ) -> Result<(), String> {
    self.poll_runtime_events();

    let (already_owned_by_client, was_owned_by_other) = {
      let binding = self.uri_bindings.entry(uri.to_string()).or_default();
      binding.subscribers.insert(client_id);
      let already_owned_by_client = binding.owner == Some(client_id);
      let was_owned_by_other = binding.owner.is_some() && !already_owned_by_client;
      (already_owned_by_client, was_owned_by_other)
    };
    trace_log(
      "focus_document",
      format!(
        "client_id={} uri={} already_owned_by_client={} was_owned_by_other={}",
        client_id, uri, already_owned_by_client, was_owned_by_other
      ),
    );

    if let Some(client) = self.clients.get_mut(&client_id) {
      client.subscribed_uris.insert(uri.to_string());
    }

    if already_owned_by_client {
      return Ok(());
    }

    if was_owned_by_other {
      self.send_notification("textDocument/didClose", Some(did_close_params(uri)))?;
    }

    self.send_notification("textDocument/didOpen", Some(open_payload))?;
    if let Some(binding) = self.uri_bindings.get_mut(uri) {
      binding.owner = Some(client_id);
      trace_log(
        "focus_document_owner_set",
        format!(
          "uri={} owner={} subscriber_count={}",
          uri,
          client_id,
          binding.subscribers.len()
        ),
      );
    }
    Ok(())
  }

  fn close_document(&mut self, client_id: u64, uri: &str) -> Result<(), String> {
    self.poll_runtime_events();
    self.remove_uri_subscriber(client_id, uri);
    Ok(())
  }

  fn remove_uri_subscriber(&mut self, client_id: u64, uri: &str) {
    let mut send_close = false;
    let mut remove_binding = false;

    if let Some(binding) = self.uri_bindings.get_mut(uri) {
      binding.subscribers.remove(&client_id);
      if binding.owner == Some(client_id) {
        send_close = true;
        binding.owner = None;
      }
      if binding.subscribers.is_empty() {
        remove_binding = true;
      }
    }

    if let Some(client) = self.clients.get_mut(&client_id) {
      client.subscribed_uris.remove(uri);
    }

    if send_close {
      let _ = self.send_notification("textDocument/didClose", Some(did_close_params(uri)));
    }
    if remove_binding {
      self.uri_bindings.remove(uri);
    }
  }

  fn send_document_sync_notification(
    &mut self,
    client_id: u64,
    uri: &str,
    method: &str,
    params: Value,
  ) -> Result<bool, String> {
    self.poll_runtime_events();
    let is_owner = self
      .uri_bindings
      .get(uri)
      .is_some_and(|binding| binding.owner == Some(client_id));
    trace_log(
      "sync_notification_check",
      format!(
        "client_id={} uri={} method={} is_owner={}",
        client_id, uri, method, is_owner
      ),
    );
    if !is_owner {
      return Ok(false);
    }

    self.send_notification(method, Some(params))?;
    Ok(true)
  }

  fn server_capabilities(&mut self, server_name: &str) -> Option<ServerCapabilitiesSnapshot> {
    self.poll_runtime_events();
    self.runtime.server_capabilities(server_name)
  }

  fn document_owned_by(&mut self, client_id: u64, uri: &str) -> bool {
    self.poll_runtime_events();
    let owned = self
      .uri_bindings
      .get(uri)
      .is_some_and(|binding| binding.owner == Some(client_id));
    trace_log(
      "document_owned_by",
      format!("client_id={} uri={} owned={}", client_id, uri, owned),
    );
    owned
  }
}

fn normalize_workspace_root(root: &Path) -> PathBuf {
  fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
}

fn trace_enabled() -> bool {
  static ENABLED: OnceLock<bool> = OnceLock::new();
  *ENABLED.get_or_init(|| {
    env::var("THE_EDITOR_SWIFT_SHARED_LSP_TRACE")
      .ok()
      .map(|value| {
        let normalized = value.trim().to_ascii_lowercase();
        normalized == "1" || normalized == "true" || normalized == "yes" || normalized == "on"
      })
      .unwrap_or(false)
  })
}

fn trace_log(context: &str, message: impl AsRef<str>) {
  if !trace_enabled() {
    return;
  }
  eprintln!("[the-ffi lsp-broker/core] {context} {}", message.as_ref());
}

fn registry() -> &'static Mutex<LspBrokerRegistry> {
  static REGISTRY: OnceLock<Mutex<LspBrokerRegistry>> = OnceLock::new();
  REGISTRY.get_or_init(|| Mutex::new(LspBrokerRegistry::default()))
}

fn with_registry<T>(f: impl FnOnce(&mut LspBrokerRegistry) -> T) -> T {
  let mut guard = registry()
    .lock()
    .unwrap_or_else(|poisoned| poisoned.into_inner());
  f(&mut guard)
}

fn with_session<T>(key: &SessionKey, f: impl FnOnce(&mut LspBrokerSession) -> T) -> Option<T> {
  with_registry(|registry| registry.sessions.get_mut(key).map(f))
}

pub fn allocate_client_id() -> u64 {
  static NEXT_CLIENT_ID: AtomicU64 = AtomicU64::new(1);
  NEXT_CLIENT_ID.fetch_add(1, Ordering::Relaxed).max(1)
}

pub fn counters() -> BrokerCounters {
  with_registry(|registry| registry.counters())
}

pub fn register_client(
  client_id: u64,
  key: SessionKey,
  config: LspRuntimeConfig,
) -> Result<(), String> {
  with_registry(|registry| {
    let session = registry.get_or_create_session(key, config)?;
    session.register_client(client_id);
    Ok(())
  })
}

pub fn unregister_client(client_id: u64, key: &SessionKey) {
  with_registry(|registry| {
    if let Some(session) = registry.sessions.get_mut(key) {
      session.detach_client(client_id);
    }
    registry.remove_empty_session(key);
  });
}

pub fn unregister_client_everywhere(client_id: u64) {
  with_registry(|registry| {
    let keys = registry.sessions.keys().cloned().collect::<Vec<_>>();
    for key in keys {
      if let Some(session) = registry.sessions.get_mut(&key) {
        session.detach_client(client_id);
      }
      registry.remove_empty_session(&key);
    }
  });
}

pub fn poll_client_events(client_id: u64, key: &SessionKey) -> Vec<LspEvent> {
  with_session(key, |session| session.drain_client_events(client_id)).unwrap_or_default()
}

pub fn send_request(
  client_id: u64,
  key: &SessionKey,
  client_request_id: u64,
  method: &str,
  params: Option<Value>,
) -> Result<(), String> {
  with_session(key, |session| {
    session.send_request(client_id, client_request_id, method, params)
  })
  .unwrap_or_else(|| {
    trace_log(
      "send_request_missing_session",
      format!(
        "client_id={} client_req_id={} method={} key={:?}",
        client_id, client_request_id, method, key
      ),
    );
    Err("missing broker session".to_string())
  })
}

pub fn cancel_request(
  client_id: u64,
  key: &SessionKey,
  client_request_id: u64,
) -> Result<(), String> {
  with_session(key, |session| session.cancel_request(client_id, client_request_id))
    .unwrap_or_else(|| {
      trace_log(
        "cancel_request_missing_session",
        format!(
          "client_id={} client_req_id={} key={:?}",
          client_id, client_request_id, key
        ),
      );
      Err("missing broker session".to_string())
    })
}

pub fn send_notification(
  key: &SessionKey,
  method: &str,
  params: Option<Value>,
) -> Result<(), String> {
  with_session(key, |session| session.send_notification(method, params))
    .unwrap_or_else(|| {
      trace_log(
        "send_notification_missing_session",
        format!("method={} key={:?}", method, key),
      );
      Err("missing broker session".to_string())
    })
}

pub fn focus_document(
  client_id: u64,
  key: &SessionKey,
  uri: &str,
  language_id: &str,
  version: i32,
  text: &str,
) -> Result<(), String> {
  let open_payload = did_open_params(uri, language_id, version, &ropey::Rope::from_str(text));
  with_session(key, |session| {
    session.focus_document(client_id, uri, open_payload)
  })
  .unwrap_or_else(|| {
    trace_log(
      "focus_document_missing_session",
      format!("client_id={} uri={} key={:?}", client_id, uri, key),
    );
    Err("missing broker session".to_string())
  })
}

pub fn close_document(client_id: u64, key: &SessionKey, uri: &str) -> Result<(), String> {
  with_session(key, |session| session.close_document(client_id, uri))
    .unwrap_or_else(|| {
      trace_log(
        "close_document_missing_session",
        format!("client_id={} uri={} key={:?}", client_id, uri, key),
      );
      Err("missing broker session".to_string())
    })
}

pub fn send_document_change(
  client_id: u64,
  key: &SessionKey,
  uri: &str,
  params: Value,
) -> Result<bool, String> {
  with_session(key, |session| {
    session.send_document_sync_notification(client_id, uri, "textDocument/didChange", params)
  })
  .unwrap_or_else(|| {
    trace_log(
      "did_change_missing_session",
      format!("client_id={} uri={} key={:?}", client_id, uri, key),
    );
    Err("missing broker session".to_string())
  })
}

pub fn send_document_save(
  client_id: u64,
  key: &SessionKey,
  uri: &str,
  params: Value,
) -> Result<bool, String> {
  with_session(key, |session| {
    session.send_document_sync_notification(client_id, uri, "textDocument/didSave", params)
  })
  .unwrap_or_else(|| {
    trace_log(
      "did_save_missing_session",
      format!("client_id={} uri={} key={:?}", client_id, uri, key),
    );
    Err("missing broker session".to_string())
  })
}

pub fn server_capabilities(
  key: &SessionKey,
  server_name: &str,
) -> Option<ServerCapabilitiesSnapshot> {
  with_session(key, |session| session.server_capabilities(server_name))
    .or_else(|| {
      trace_log(
        "server_capabilities_missing_session",
        format!("server_name={} key={:?}", server_name, key),
      );
      None
    })
    .flatten()
}

pub fn document_owned_by(client_id: u64, key: &SessionKey, uri: &str) -> bool {
  with_session(key, |session| session.document_owned_by(client_id, uri)).unwrap_or_else(|| {
    trace_log(
      "document_owned_by_missing_session",
      format!("client_id={} uri={} key={:?}", client_id, uri, key),
    );
    false
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn session_key_canonicalizes_workspace_root() {
    let workspace = std::env::current_dir().expect("cwd");
    let config = LspRuntimeConfig::new(workspace.clone()).with_server(
      the_lsp::LspServerConfig::new("test", "cat")
        .with_args(["--stdio"])
        .with_env([("RUST_LOG", "error")]),
    );

    let key = SessionKey::from_runtime_config(&config).expect("session key");
    assert_eq!(key.server_name, "test");
    assert!(key.workspace_root.contains(workspace.file_name().unwrap().to_string_lossy().as_ref()));
  }

  #[test]
  fn register_and_unregister_client_is_ref_counted() {
    let workspace = std::env::current_dir().expect("cwd");
    let config = LspRuntimeConfig::new(workspace.clone());
    let key = SessionKey {
      workspace_root: workspace.to_string_lossy().into_owned(),
      server_name: "none".into(),
      command: "none".into(),
      args: Vec::new(),
      env: Vec::new(),
      initialize_options: None,
    };

    assert!(register_client(101, key.clone(), config.clone()).is_ok());
    assert!(register_client(202, key.clone(), config).is_ok());
    unregister_client(101, &key);
    let events = poll_client_events(202, &key);
    assert!(events.is_empty());
    unregister_client(202, &key);
  }
}
