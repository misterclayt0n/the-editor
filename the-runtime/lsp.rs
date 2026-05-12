use std::{
  collections::{
    BTreeMap,
    HashMap,
  },
  io::{
    BufRead,
    BufReader,
    Read,
    Write,
  },
  process::{
    Child,
    ChildStdin,
    Command,
    Stdio,
  },
  sync::{
    Arc,
    Mutex,
    mpsc,
  },
  thread,
  time::{
    Duration,
    Instant,
  },
};

use ropey::{
  Rope,
  RopeSlice,
};
use serde::{
  Deserialize,
  Serialize,
};
use serde_json::{
  Value,
  json,
};
use the_lib::transaction::{
  ChangeSet,
  Operation,
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextDocumentSyncKind {
  None,
  Full,
  Incremental,
}

impl Default for TextDocumentSyncKind {
  fn default() -> Self {
    Self::Full
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LspServerHandle(u64);

impl LspServerHandle {
  pub fn get(self) -> u64 {
    self.0
  }
  pub fn from_raw(id: u64) -> Self {
    Self(id)
  }
  fn next(counter: &mut u64) -> Self {
    *counter = counter.wrapping_add(1).max(1);
    Self(*counter)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
  pub id:                     String,
  pub command:                String,
  #[serde(default)]
  pub args:                   Vec<String>,
  #[serde(default)]
  pub env:                    BTreeMap<String, String>,
  #[serde(default)]
  pub languages:              Vec<String>,
  #[serde(default)]
  pub file_types:             Vec<String>,
  #[serde(default)]
  pub initialization_options: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerStatus {
  pub handle:                        u64,
  pub id:                            String,
  pub state:                         String,
  pub message:                       Option<String>,
  #[serde(default, rename = "completionTriggerCharacters")]
  pub completion_trigger_characters: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspLogEntry {
  pub server:  u64,
  pub stream:  String,
  pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspDiagnosticEntry {
  pub server:      u64,
  pub uri:         String,
  pub diagnostics: Vec<Value>,
  pub version:     Option<i64>,
}

#[derive(Debug, Error)]
pub enum LspError {
  #[error("unknown LSP server {0}")]
  UnknownServer(u64),
  #[error("failed to spawn LSP server: {0}")]
  Spawn(String),
  #[error("LSP transport failed: {0}")]
  Transport(String),
  #[error("LSP request timed out")]
  Timeout,
  #[error("failed to encode/decode LSP payload: {0}")]
  Json(#[from] serde_json::Error),
}

struct LspServerProcess {
  config:                        LspServerConfig,
  child:                         Child,
  stdin:                         Arc<Mutex<ChildStdin>>,
  rx:                            mpsc::Receiver<Value>,
  logs:                          Arc<Mutex<Vec<LspLogEntry>>>,
  next_request_id:               u64,
  initialized:                   bool,
  state:                         String,
  message:                       Option<String>,
  open_documents:                HashMap<String, i64>,
  diagnostics:                   HashMap<String, (Option<i64>, Vec<Value>)>,
  text_sync:                     TextDocumentSyncKind,
  completion_trigger_characters: Vec<String>,
}

#[derive(Default)]
pub struct LspStore {
  next_server_handle: u64,
  configured:         BTreeMap<String, LspServerConfig>,
  servers:            BTreeMap<LspServerHandle, LspServerProcess>,
}

impl std::fmt::Debug for LspStore {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LspStore")
      .field("configured", &self.configured.keys().collect::<Vec<_>>())
      .field(
        "servers",
        &self.servers.keys().map(|h| h.get()).collect::<Vec<_>>(),
      )
      .finish()
  }
}

impl LspStore {
  pub fn register_server(&mut self, config: LspServerConfig) {
    self.configured.insert(config.id.clone(), config);
  }

  pub fn configured_servers(&self) -> Vec<LspServerConfig> {
    self.configured.values().cloned().collect()
  }

  pub fn start_server(
    &mut self,
    id: &str,
    root_uri: Option<String>,
  ) -> Result<LspServerHandle, LspError> {
    if let Some((handle, _)) = self
      .servers
      .iter()
      .find(|(_, server)| server.config.id == id)
    {
      return Ok(*handle);
    }
    let config = self
      .configured
      .get(id)
      .cloned()
      .ok_or_else(|| LspError::Spawn(format!("server config '{id}' is not registered")))?;
    let handle = LspServerHandle::next(&mut self.next_server_handle);
    let mut command = Command::new(&config.command);
    command
      .args(&config.args)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped());
    for (key, value) in &config.env {
      command.env(key, value);
    }
    let mut child = command
      .spawn()
      .map_err(|err| LspError::Spawn(err.to_string()))?;
    let stdin = Arc::new(Mutex::new(
      child
        .stdin
        .take()
        .ok_or_else(|| LspError::Spawn("missing stdin".into()))?,
    ));
    let stdout = child
      .stdout
      .take()
      .ok_or_else(|| LspError::Spawn("missing stdout".into()))?;
    let stderr = child.stderr.take();
    let (tx, rx) = mpsc::channel();
    let logs = Arc::new(Mutex::new(Vec::new()));
    spawn_stdout_reader(handle, stdout, tx, logs.clone());
    if let Some(stderr) = stderr {
      spawn_stderr_reader(handle, stderr, logs.clone());
    }
    let mut server = LspServerProcess {
      config,
      child,
      stdin,
      rx,
      logs,
      next_request_id: 0,
      initialized: false,
      state: "starting".into(),
      message: None,
      open_documents: HashMap::new(),
      diagnostics: HashMap::new(),
      text_sync: TextDocumentSyncKind::Full,
      completion_trigger_characters: Vec::new(),
    };
    let init = json!({
      "processId": std::process::id(),
      "rootUri": root_uri,
      "capabilities": {
        "general": {
          "positionEncodings": ["utf-16"]
        },
        "textDocument": {
          "completion": {
            "completionItem": {
              "snippetSupport": true,
              "resolveSupport": {
                "properties": ["additionalTextEdits", "command", "detail", "documentation"]
              },
              "deprecatedSupport": true,
              "tagSupport": { "valueSet": [1] },
              "insertReplaceSupport": true,
              "labelDetailsSupport": true,
              "insertTextModeSupport": { "valueSet": [1, 2] },
              "documentationFormat": ["markdown", "plaintext"]
            },
            "insertTextMode": 2,
            "completionList": {
              "itemDefaults": ["commitCharacters", "editRange", "insertTextMode", "insertTextFormat", "data"]
            },
            "contextSupport": true
          },
          "hover": {
            "contentFormat": ["markdown", "plaintext"]
          },
          "signatureHelp": {
            "signatureInformation": {
              "documentationFormat": ["markdown", "plaintext"],
              "parameterInformation": { "labelOffsetSupport": true },
              "activeParameterSupport": true
            }
          },
          "codeAction": {
            "dynamicRegistration": false,
            "isPreferredSupport": true,
            "disabledSupport": true,
            "dataSupport": true,
            "resolveSupport": { "properties": ["edit", "command"] },
            "codeActionLiteralSupport": {
              "codeActionKind": {
                "valueSet": ["", "quickfix", "refactor", "refactor.extract", "refactor.inline", "refactor.rewrite", "source", "source.organizeImports", "source.fixAll"]
              }
            }
          }
        },
        "window": {
          "workDoneProgress": true
        }
      },
      "initializationOptions": server.config.initialization_options.clone().unwrap_or(Value::Null),
    });
    match server.request("initialize", init, Duration::from_secs(20)) {
      Ok(result) => {
        server.text_sync = parse_text_document_sync_kind(&result);
        server.completion_trigger_characters = parse_completion_trigger_characters(&result);
        server.notify("initialized", json!({}))?;
        server.initialized = true;
        server.state = "running".into();
      },
      Err(err) => {
        server.state = "error".into();
        server.message = Some(err.to_string());
      },
    }
    self.servers.insert(handle, server);
    Ok(handle)
  }

  pub fn shutdown_server(&mut self, handle: LspServerHandle) -> Result<(), LspError> {
    let mut server = self
      .servers
      .remove(&handle)
      .ok_or(LspError::UnknownServer(handle.get()))?;
    let _ = server.request("shutdown", Value::Null, Duration::from_secs(5));
    let _ = server.notify("exit", Value::Null);
    let _ = server.child.kill();
    let _ = server.child.wait();
    Ok(())
  }

  pub fn did_open_document(
    &mut self,
    uri: &str,
    language_id: &str,
    text: &str,
    version: i64,
  ) -> Result<(), LspError> {
    for server in self.servers.values_mut() {
      if !server.matches_language(language_id, uri) || server.open_documents.contains_key(uri) {
        continue;
      }
      server.open_documents.insert(uri.to_string(), version);
      server.notify("textDocument/didOpen", json!({
        "textDocument": { "uri": uri, "languageId": language_id, "version": version, "text": text }
      }))?;
    }
    Ok(())
  }

  pub fn did_change_document(
    &mut self,
    uri: &str,
    text: &str,
    version: i64,
  ) -> Result<(), LspError> {
    for server in self.servers.values_mut() {
      if !server.open_documents.contains_key(uri) {
        continue;
      }
      match server.text_sync {
        TextDocumentSyncKind::None => {},
        TextDocumentSyncKind::Full | TextDocumentSyncKind::Incremental => {
          server.open_documents.insert(uri.to_string(), version);
          server.notify(
            "textDocument/didChange",
            json!({
              "textDocument": { "uri": uri, "version": version },
              "contentChanges": [{ "text": text }]
            }),
          )?;
        },
      }
    }
    Ok(())
  }

  pub fn did_change_document_incremental(
    &mut self,
    uri: &str,
    old_text: &Rope,
    new_text: &Rope,
    changeset: &ChangeSet,
    version: i64,
  ) -> Result<(), LspError> {
    for server in self.servers.values_mut() {
      if !server.open_documents.contains_key(uri) {
        continue;
      }
      let content_changes = match server.text_sync {
        TextDocumentSyncKind::None => continue,
        TextDocumentSyncKind::Full => vec![json!({ "text": new_text.to_string() })],
        TextDocumentSyncKind::Incremental => {
          changeset_to_content_changes(old_text, new_text, changeset)
        },
      };
      if content_changes.is_empty() {
        continue;
      }
      server.open_documents.insert(uri.to_string(), version);
      server.notify(
        "textDocument/didChange",
        json!({
          "textDocument": { "uri": uri, "version": version },
          "contentChanges": content_changes
        }),
      )?;
    }
    Ok(())
  }

  pub fn did_close_document(&mut self, uri: &str) -> Result<(), LspError> {
    for server in self.servers.values_mut() {
      if server.open_documents.remove(uri).is_none() {
        continue;
      }
      server.notify(
        "textDocument/didClose",
        json!({ "textDocument": { "uri": uri } }),
      )?;
    }
    Ok(())
  }

  pub fn poll(&mut self) {
    for (handle, server) in self.servers.iter_mut() {
      server.poll_notifications(*handle);
    }
  }

  pub fn diagnostics(&mut self) -> Vec<LspDiagnosticEntry> {
    self.poll();
    self
      .servers
      .iter()
      .flat_map(|(handle, server)| {
        server
          .diagnostics
          .iter()
          .map(|(uri, (version, diagnostics))| {
            LspDiagnosticEntry {
              server:      handle.get(),
              uri:         uri.clone(),
              diagnostics: diagnostics.clone(),
              version:     *version,
            }
          })
          .collect::<Vec<_>>()
      })
      .collect()
  }

  pub fn status(&mut self) -> Vec<LspServerStatus> {
    self.poll();
    self
      .servers
      .iter()
      .map(|(handle, server)| {
        LspServerStatus {
          handle:                        handle.get(),
          id:                            server.config.id.clone(),
          state:                         server.state.clone(),
          message:                       server.message.clone(),
          completion_trigger_characters: server.completion_trigger_characters.clone(),
        }
      })
      .collect()
  }

  pub fn logs(&mut self, handle: LspServerHandle) -> Result<Vec<LspLogEntry>, LspError> {
    self.poll();
    let server = self
      .servers
      .get(&handle)
      .ok_or(LspError::UnknownServer(handle.get()))?;
    Ok(
      server
        .logs
        .lock()
        .map(|logs| logs.clone())
        .unwrap_or_default(),
    )
  }

  pub fn request(
    &mut self,
    handle: LspServerHandle,
    method: &str,
    params: Value,
    timeout: Duration,
  ) -> Result<Value, LspError> {
    self
      .servers
      .get_mut(&handle)
      .ok_or(LspError::UnknownServer(handle.get()))?
      .request(method, params, timeout)
  }

  pub fn notify(
    &mut self,
    handle: LspServerHandle,
    method: &str,
    params: Value,
  ) -> Result<(), LspError> {
    self
      .servers
      .get_mut(&handle)
      .ok_or(LspError::UnknownServer(handle.get()))?
      .notify(method, params)
  }
}

impl LspServerProcess {
  fn request(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value, LspError> {
    self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
    let id = self.next_request_id;
    write_message(
      &self.stdin,
      &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }),
    )?;
    let deadline = Instant::now() + timeout;
    loop {
      let remaining = deadline.saturating_duration_since(Instant::now());
      if remaining.is_zero() {
        return Err(LspError::Timeout);
      }
      let message = self
        .rx
        .recv_timeout(remaining)
        .map_err(|_| LspError::Timeout)?;
      if message.get("method").is_some() {
        self.handle_server_message(message)?;
        continue;
      }
      if message.get("id").and_then(Value::as_u64) == Some(id) {
        if let Some(error) = message.get("error") {
          return Err(LspError::Transport(error.to_string()));
        }
        return Ok(message.get("result").cloned().unwrap_or(Value::Null));
      }
    }
  }

  fn notify(&mut self, method: &str, params: Value) -> Result<(), LspError> {
    write_message(
      &self.stdin,
      &json!({ "jsonrpc": "2.0", "method": method, "params": params }),
    )
  }

  fn matches_language(&self, language_id: &str, uri: &str) -> bool {
    self
      .config
      .languages
      .iter()
      .any(|language| language == language_id)
      || self
        .config
        .file_types
        .iter()
        .any(|extension| uri.ends_with(&format!(".{extension}")))
  }

  fn poll_notifications(&mut self, handle: LspServerHandle) {
    while let Ok(message) = self.rx.try_recv() {
      if message.get("method").is_some() {
        if let Err(error) = self.handle_server_message(message) {
          self.message = Some(error.to_string());
          self.state = "error".into();
          push_log(&self.logs, handle, "protocol", error.to_string());
        }
      } else if let Some(error) = message.get("error") {
        self.message = Some(error.to_string());
        self.state = "error".into();
        push_log(&self.logs, handle, "protocol", error.to_string());
      }
    }
  }

  fn handle_server_message(&mut self, message: Value) -> Result<(), LspError> {
    if message.get("id").is_some() {
      self.handle_server_request(message)
    } else {
      self.handle_notification(message);
      Ok(())
    }
  }

  fn handle_server_request(&mut self, message: Value) -> Result<(), LspError> {
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    let method = message
      .get("method")
      .and_then(Value::as_str)
      .unwrap_or_default();
    let result = match method {
      "workspace/configuration" => Value::Array(Vec::new()),
      "window/workDoneProgress/create"
      | "client/registerCapability"
      | "client/unregisterCapability" => Value::Null,
      _ => Value::Null,
    };
    write_message(
      &self.stdin,
      &json!({ "jsonrpc": "2.0", "id": id, "result": result }),
    )
  }

  fn handle_notification(&mut self, message: Value) {
    let method = message
      .get("method")
      .and_then(Value::as_str)
      .unwrap_or_default();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    match method {
      "textDocument/publishDiagnostics" => {
        if let Some(uri) = params.get("uri").and_then(Value::as_str) {
          let diagnostics = params
            .get("diagnostics")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
          let version = params.get("version").and_then(Value::as_i64);
          self
            .diagnostics
            .insert(uri.to_string(), (version, diagnostics));
        }
      },
      "window/logMessage" | "window/showMessage" => {
        push_log(&self.logs, LspServerHandle(0), method, params.to_string());
      },
      _ => {},
    }
  }
}

fn parse_text_document_sync_kind(initialize_result: &Value) -> TextDocumentSyncKind {
  let Some(sync) = initialize_result
    .get("capabilities")
    .and_then(|capabilities| capabilities.get("textDocumentSync"))
  else {
    return TextDocumentSyncKind::Full;
  };
  let kind = if let Some(kind) = sync.as_u64() {
    kind
  } else {
    sync.get("change").and_then(Value::as_u64).unwrap_or(1)
  };
  match kind {
    0 => TextDocumentSyncKind::None,
    2 => TextDocumentSyncKind::Incremental,
    _ => TextDocumentSyncKind::Full,
  }
}

fn parse_completion_trigger_characters(initialize_result: &Value) -> Vec<String> {
  initialize_result
    .get("capabilities")
    .and_then(|capabilities| capabilities.get("completionProvider"))
    .and_then(|provider| provider.get("triggerCharacters"))
    .and_then(Value::as_array)
    .map(|characters| {
      characters
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
    })
    .unwrap_or_default()
}

fn changeset_to_content_changes(
  old_text: &Rope,
  new_text: &Rope,
  changeset: &ChangeSet,
) -> Vec<Value> {
  let mut iter = changeset.changes().iter().peekable();
  let mut old_pos = 0usize;
  let mut new_pos = 0usize;
  let old_slice = old_text.slice(..);
  let mut changes = Vec::new();

  while let Some(operation) = iter.next() {
    let len = match operation {
      Operation::Delete(count) | Operation::Retain(count) => *count,
      Operation::Insert(_) => 0,
    };
    let mut old_end = old_pos + len;

    match operation {
      Operation::Retain(count) => {
        new_pos += count;
      },
      Operation::Delete(_) => {
        // LSP contentChanges are applied sequentially, so the range start must
        // be expressed in the document state after earlier changes in this
        // batch. `new_pos` tracks that state; deleted extent still comes from
        // the old document.
        let start = pos_to_utf16_position(new_text, new_pos);
        let end = traverse_utf16(start, old_slice.slice(old_pos..old_end));
        changes.push(json!({
          "range": { "start": start.as_json(), "end": end.as_json() },
          "text": "",
        }));
      },
      Operation::Insert(text) => {
        let start = pos_to_utf16_position(new_text, new_pos);
        new_pos += text.chars().count();
        let end = if let Some(Operation::Delete(delete_count)) = iter.peek() {
          old_end = old_pos + *delete_count;
          iter.next();
          traverse_utf16(start, old_slice.slice(old_pos..old_end))
        } else {
          start
        };
        changes.push(json!({
          "range": { "start": start.as_json(), "end": end.as_json() },
          "text": text.to_string(),
        }));
      },
    }

    old_pos = old_end;
  }

  changes
}

#[derive(Debug, Clone, Copy)]
struct Utf16Position {
  line:      u32,
  character: u32,
}

impl Utf16Position {
  fn as_json(self) -> Value {
    json!({ "line": self.line, "character": self.character })
  }
}

fn pos_to_utf16_position(text: &Rope, pos: usize) -> Utf16Position {
  let line = text.char_to_line(pos.min(text.len_chars()));
  let line_start = text.line_to_char(line);
  let utf16_col = text
    .slice(line_start..pos.min(text.len_chars()))
    .chars()
    .map(|ch| ch.len_utf16() as u32)
    .sum::<u32>();
  Utf16Position {
    line:      line as u32,
    character: utf16_col,
  }
}

fn traverse_utf16(pos: Utf16Position, text: RopeSlice<'_>) -> Utf16Position {
  let Utf16Position {
    mut line,
    mut character,
  } = pos;
  let mut chars = text.chars().peekable();
  while let Some(ch) = chars.next() {
    if ch == '\n' || ch == '\r' {
      if ch == '\r' && chars.peek() == Some(&'\n') {
        chars.next();
      }
      line += 1;
      character = 0;
    } else {
      character += ch.len_utf16() as u32;
    }
  }
  Utf16Position { line, character }
}

fn write_message(stdin: &Arc<Mutex<ChildStdin>>, value: &Value) -> Result<(), LspError> {
  let body = serde_json::to_vec(value)?;
  let mut stdin = stdin
    .lock()
    .map_err(|_| LspError::Transport("stdin lock poisoned".into()))?;
  write!(stdin, "Content-Length: {}\r\n\r\n", body.len())
    .map_err(|err| LspError::Transport(err.to_string()))?;
  stdin
    .write_all(&body)
    .map_err(|err| LspError::Transport(err.to_string()))?;
  stdin
    .flush()
    .map_err(|err| LspError::Transport(err.to_string()))
}

fn spawn_stdout_reader(
  handle: LspServerHandle,
  stdout: impl Read + Send + 'static,
  tx: mpsc::Sender<Value>,
  logs: Arc<Mutex<Vec<LspLogEntry>>>,
) {
  thread::Builder::new()
    .name("the-runtime-lsp-stdout".into())
    .spawn(move || {
      let mut reader = BufReader::new(stdout);
      loop {
        match read_message(&mut reader) {
          Ok(Some(value)) => {
            let _ = tx.send(value);
          },
          Ok(None) => break,
          Err(err) => {
            push_log(&logs, handle, "stdout", err);
            break;
          },
        }
      }
    })
    .expect("failed to spawn lsp stdout reader");
}

fn spawn_stderr_reader(
  handle: LspServerHandle,
  stderr: impl Read + Send + 'static,
  logs: Arc<Mutex<Vec<LspLogEntry>>>,
) {
  thread::Builder::new()
    .name("the-runtime-lsp-stderr".into())
    .spawn(move || {
      let reader = BufReader::new(stderr);
      for line in reader.lines().map_while(Result::ok) {
        push_log(&logs, handle, "stderr", line);
      }
    })
    .expect("failed to spawn lsp stderr reader");
}

fn push_log(
  logs: &Arc<Mutex<Vec<LspLogEntry>>>,
  handle: LspServerHandle,
  stream: &str,
  message: impl Into<String>,
) {
  if let Ok(mut logs) = logs.lock() {
    logs.push(LspLogEntry {
      server:  handle.get(),
      stream:  stream.into(),
      message: message.into(),
    });
    if logs.len() > 1000 {
      logs.drain(0..100);
    }
  }
}

fn read_message(reader: &mut BufReader<impl Read>) -> Result<Option<Value>, String> {
  let mut content_length = None;
  loop {
    let mut line = String::new();
    let bytes = reader.read_line(&mut line).map_err(|err| err.to_string())?;
    if bytes == 0 {
      return Ok(None);
    }
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if trimmed.is_empty() {
      break;
    }
    if let Some(value) = trimmed.strip_prefix("Content-Length:") {
      content_length = Some(
        value
          .trim()
          .parse::<usize>()
          .map_err(|err| err.to_string())?,
      );
    }
  }
  let len = content_length.ok_or_else(|| "missing Content-Length".to_string())?;
  let mut body = vec![0; len];
  reader
    .read_exact(&mut body)
    .map_err(|err| err.to_string())?;
  serde_json::from_slice(&body)
    .map(Some)
    .map_err(|err| err.to_string())
}
