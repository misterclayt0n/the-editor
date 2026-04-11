#[cfg(unix)] use std::io::Write;
use std::{
  collections::hash_map::DefaultHasher,
  fs,
  hash::{
    Hash,
    Hasher,
  },
  path::{
    Path,
    PathBuf,
  },
  sync::mpsc::{
    Receiver,
    Sender,
    TryRecvError,
    channel,
  },
  thread::{
    self,
    JoinHandle,
  },
  time::Duration,
};

use serde::{
  Deserialize,
  Serialize,
};
use serde_json::Value;

pub const PI_BRIDGE_PROTOCOL_VERSION: u32 = 1;
pub const PI_BRIDGE_MANIFEST_FILE: &str = "pi-bridge.json";
const PI_BRIDGE_GIT_DIR: &str = "the-editor";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PiBridgeManifest {
  pub version:        u32,
  pub transport:      String,
  pub workspace_root: String,
  pub socket_path:    String,
  pub editor_pid:     u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionPayload {
  pub absolute_path:           String,
  pub workspace_relative_path: String,
  pub language:                Option<String>,
  pub selected_text:           String,
  pub start_char:              usize,
  pub end_char:                usize,
  pub start_line:              usize,
  pub end_line:                usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PiBridgeEnvelope {
  Request {
    id:     String,
    method: String,
    params: Value,
  },
  Response {
    id:     String,
    ok:     bool,
    result: Option<Value>,
    error:  Option<String>,
  },
  Notification {
    method: String,
    params: Value,
  },
}

impl PiBridgeEnvelope {
  pub fn notification<T: Serialize>(method: impl Into<String>, params: T) -> Result<Self, String> {
    let params = serde_json::to_value(params)
      .map_err(|err| format!("failed to encode bridge notification: {err}"))?;
    Ok(Self::Notification {
      method: method.into(),
      params,
    })
  }

  pub fn ok<T: Serialize>(id: String, result: T) -> Result<Self, String> {
    let result = serde_json::to_value(result)
      .map_err(|err| format!("failed to encode bridge response: {err}"))?;
    Ok(Self::Response {
      id,
      ok: true,
      result: Some(result),
      error: None,
    })
  }

  pub fn err(id: String, error: impl Into<String>) -> Self {
    Self::Response {
      id,
      ok: false,
      result: None,
      error: Some(error.into()),
    }
  }
}

#[derive(Debug, Clone)]
pub enum PiBridgeEvent {
  Attached,
  Detached,
  Request {
    id:     String,
    method: String,
    params: Value,
  },
  Notification {
    method: String,
    params: Value,
  },
  InvalidMessage(String),
}

#[derive(Debug)]
enum PiBridgeWorkerCommand {
  Send(PiBridgeEnvelope),
  Stop,
}

#[cfg(unix)]
mod imp {
  use std::{
    io::{
      ErrorKind,
      Read,
    },
    net::Shutdown,
    os::unix::net::{
      UnixListener,
      UnixStream,
    },
  };

  use super::*;

  struct WorkerConnection {
    stream:      UnixStream,
    read_buffer: Vec<u8>,
  }

  impl WorkerConnection {
    fn new(stream: UnixStream) -> Result<Self, String> {
      stream
        .set_nonblocking(true)
        .map_err(|err| format!("failed to configure bridge stream: {err}"))?;
      Ok(Self {
        stream,
        read_buffer: Vec::new(),
      })
    }
  }

  pub struct PiBridgeHandle {
    manifest_path: PathBuf,
    socket_path:   PathBuf,
    tx:                   Sender<PiBridgeWorkerCommand>,
    rx:                   Receiver<PiBridgeEvent>,
    join_handle:          Option<JoinHandle<()>>,
    attached:             bool,
  }

  impl PiBridgeHandle {
    pub fn start(workspace_root: &Path) -> Result<Self, String> {
      let manifest_path = manifest_path_for_workspace(workspace_root)?;
      let bridge_dir = manifest_path
        .parent()
        .ok_or_else(|| "bridge manifest path has no parent directory".to_string())?
        .to_path_buf();
      fs::create_dir_all(&bridge_dir).map_err(|err| {
        format!(
          "failed to create bridge directory '{}': {err}",
          bridge_dir.display()
        )
      })?;
      let socket_path = socket_path_for_workspace(workspace_root);
      if socket_path.exists() {
        let _ = fs::remove_file(&socket_path);
      }
      if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
          format!(
            "failed to create bridge socket directory '{}': {err}",
            parent.display()
          )
        })?;
      }

      let listener = UnixListener::bind(&socket_path).map_err(|err| {
        format!(
          "failed to bind bridge socket '{}': {err}",
          socket_path.display()
        )
      })?;
      listener
        .set_nonblocking(true)
        .map_err(|err| format!("failed to configure bridge listener: {err}"))?;

      let manifest = PiBridgeManifest {
        version:        PI_BRIDGE_PROTOCOL_VERSION,
        transport:      "unix-jsonl".to_string(),
        workspace_root: workspace_root.display().to_string(),
        socket_path:    socket_path.display().to_string(),
        editor_pid:     std::process::id(),
      };
      let manifest_text = serde_json::to_string_pretty(&manifest)
        .map_err(|err| format!("failed to encode bridge manifest: {err}"))?;
      fs::write(&manifest_path, manifest_text).map_err(|err| {
        format!(
          "failed to write bridge manifest '{}': {err}",
          manifest_path.display()
        )
      })?;

      let (tx, worker_rx) = channel();
      let (worker_tx, rx) = channel();
      let socket_path_for_thread = socket_path.clone();
      let join_handle = thread::spawn(move || {
        worker_main(listener, worker_rx, worker_tx, socket_path_for_thread);
      });

      Ok(Self {
        manifest_path,
        socket_path,
        tx,
        rx,
        join_handle: Some(join_handle),
        attached: false,
      })
    }

    pub fn is_attached(&self) -> bool {
      self.attached
    }

    pub fn send(&self, envelope: PiBridgeEnvelope) -> Result<(), String> {
      self
        .tx
        .send(PiBridgeWorkerCommand::Send(envelope))
        .map_err(|_| "pi bridge worker is not running".to_string())
    }

    pub fn drain_events(&mut self) -> Vec<PiBridgeEvent> {
      let mut events = Vec::new();
      loop {
        match self.rx.try_recv() {
          Ok(event) => {
            match &event {
              PiBridgeEvent::Attached => self.attached = true,
              PiBridgeEvent::Detached => self.attached = false,
              _ => {},
            }
            events.push(event);
          },
          Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
        }
      }
      events
    }

    pub fn shutdown(&mut self) {
      let _ = self.tx.send(PiBridgeWorkerCommand::Stop);
      if let Some(join_handle) = self.join_handle.take() {
        let _ = join_handle.join();
      }
      self.attached = false;
      let _ = fs::remove_file(&self.socket_path);
      let _ = fs::remove_file(&self.manifest_path);
    }
  }

  impl Drop for PiBridgeHandle {
    fn drop(&mut self) {
      self.shutdown();
    }
  }

  fn worker_main(
    listener: UnixListener,
    rx: Receiver<PiBridgeWorkerCommand>,
    tx: Sender<PiBridgeEvent>,
    socket_path: PathBuf,
  ) {
    let mut active = None::<WorkerConnection>;
    let mut was_attached = false;
    let mut scratch = [0_u8; 4096];

    loop {
      let mut stop = false;
      loop {
        match rx.try_recv() {
          Ok(PiBridgeWorkerCommand::Send(message)) => {
            if let Some(connection) = active.as_mut()
              && let Err(err) = write_envelope(connection, &message)
            {
              let _ = tx.send(PiBridgeEvent::InvalidMessage(err));
              let _ = tx.send(PiBridgeEvent::Detached);
              let _ = connection.stream.shutdown(Shutdown::Both);
              active = None;
              was_attached = false;
            }
          },
          Ok(PiBridgeWorkerCommand::Stop) => {
            stop = true;
            break;
          },
          Err(TryRecvError::Empty) => break,
          Err(TryRecvError::Disconnected) => {
            stop = true;
            break;
          },
        }
      }
      if stop {
        break;
      }

      match listener.accept() {
        Ok((stream, _)) => {
          if active.is_some() {
            reject_connection(stream, &tx);
            continue;
          }
          match WorkerConnection::new(stream) {
            Ok(connection) => {
              active = Some(connection);
              if !was_attached {
                let _ = tx.send(PiBridgeEvent::Attached);
                was_attached = true;
              }
            },
            Err(err) => {
              let _ = tx.send(PiBridgeEvent::InvalidMessage(err));
            },
          }
        },
        Err(err) if err.kind() == ErrorKind::WouldBlock => {},
        Err(err) => {
          let _ = tx.send(PiBridgeEvent::InvalidMessage(format!(
            "bridge accept failed: {err}"
          )));
        },
      }

      let mut disconnected = false;
      if let Some(connection) = active.as_mut() {
        loop {
          match connection.stream.read(&mut scratch) {
            Ok(0) => {
              disconnected = true;
              break;
            },
            Ok(read) => {
              connection.read_buffer.extend_from_slice(&scratch[..read]);
              process_lines(connection, &tx);
            },
            Err(err) if err.kind() == ErrorKind::WouldBlock => break,
            Err(err) => {
              let _ = tx.send(PiBridgeEvent::InvalidMessage(format!(
                "bridge read failed: {err}"
              )));
              disconnected = true;
              break;
            },
          }
        }
      }
      if disconnected {
        if let Some(connection) = active.as_mut() {
          let _ = connection.stream.shutdown(Shutdown::Both);
        }
        active = None;
        if was_attached {
          let _ = tx.send(PiBridgeEvent::Detached);
          was_attached = false;
        }
      }

      thread::sleep(Duration::from_millis(16));
    }

    if let Some(connection) = active.as_mut() {
      let _ = connection.stream.shutdown(Shutdown::Both);
    }
    let _ = fs::remove_file(socket_path);
  }

  fn reject_connection(mut stream: UnixStream, tx: &Sender<PiBridgeEvent>) {
    match PiBridgeEnvelope::notification("attach_rejected", serde_json::json!({ "reason": "busy" }))
    {
      Ok(envelope) => {
        if let Err(err) = write_envelope_to_stream(&mut stream, &envelope) {
          let _ = tx.send(PiBridgeEvent::InvalidMessage(err));
        }
      },
      Err(err) => {
        let _ = tx.send(PiBridgeEvent::InvalidMessage(err));
      },
    }
    let _ = stream.shutdown(Shutdown::Both);
  }

  fn process_lines(connection: &mut WorkerConnection, tx: &Sender<PiBridgeEvent>) {
    while let Some(pos) = connection
      .read_buffer
      .iter()
      .position(|byte| *byte == b'\n')
    {
      let mut line = connection.read_buffer.drain(..=pos).collect::<Vec<_>>();
      if line.last() == Some(&b'\n') {
        line.pop();
      }
      if line.last() == Some(&b'\r') {
        line.pop();
      }
      if line.is_empty() {
        continue;
      }
      match serde_json::from_slice::<PiBridgeEnvelope>(&line) {
        Ok(PiBridgeEnvelope::Request { id, method, params }) => {
          let _ = tx.send(PiBridgeEvent::Request { id, method, params });
        },
        Ok(PiBridgeEnvelope::Notification { method, params }) => {
          let _ = tx.send(PiBridgeEvent::Notification { method, params });
        },
        Ok(PiBridgeEnvelope::Response { .. }) => {},
        Err(err) => {
          let _ = tx.send(PiBridgeEvent::InvalidMessage(format!(
            "failed to decode bridge message: {err}"
          )));
        },
      }
    }
  }

  fn write_envelope(
    connection: &mut WorkerConnection,
    envelope: &PiBridgeEnvelope,
  ) -> Result<(), String> {
    write_envelope_to_stream(&mut connection.stream, envelope)
  }
}

#[cfg(not(unix))]
mod imp {
  use super::*;

  pub struct PiBridgeHandle {
    attached: bool,
  }

  impl PiBridgeHandle {
    pub fn start(_workspace_root: &Path) -> Result<Self, String> {
      Err("pi bridge is only available on unix-like systems".to_string())
    }

    pub fn is_attached(&self) -> bool {
      self.attached
    }

    pub fn send(&self, _envelope: PiBridgeEnvelope) -> Result<(), String> {
      Err("pi bridge is only available on unix-like systems".to_string())
    }

    pub fn drain_events(&mut self) -> Vec<PiBridgeEvent> {
      Vec::new()
    }

    pub fn shutdown(&mut self) {}
  }
}

pub use imp::PiBridgeHandle;

#[cfg(unix)]
fn write_envelope_to_stream(
  stream: &mut std::os::unix::net::UnixStream,
  envelope: &PiBridgeEnvelope,
) -> Result<(), String> {
  let line = serde_json::to_vec(envelope)
    .map_err(|err| format!("failed to encode bridge message: {err}"))?;
  stream
    .write_all(&line)
    .map_err(|err| format!("failed to write bridge message: {err}"))?;
  stream
    .write_all(b"\n")
    .map_err(|err| format!("failed to terminate bridge message: {err}"))?;
  Ok(())
}

fn socket_path_for_workspace(workspace_root: &Path) -> PathBuf {
  let mut hasher = DefaultHasher::new();
  workspace_root.hash(&mut hasher);
  let hash = hasher.finish();
  std::env::temp_dir().join(format!("the-editor-pi-{hash:016x}.sock"))
}

fn manifest_path_for_workspace(workspace_root: &Path) -> Result<PathBuf, String> {
  let git_dir = resolve_git_dir(workspace_root)
    .ok_or_else(|| format!("workspace '{}' is not backed by a git directory", workspace_root.display()))?;
  Ok(git_dir
    .join(PI_BRIDGE_GIT_DIR)
    .join(PI_BRIDGE_MANIFEST_FILE))
}

fn resolve_git_dir(workspace_root: &Path) -> Option<PathBuf> {
  let dot_git = workspace_root.join(".git");
  if dot_git.is_dir() {
    return Some(dot_git);
  }
  if !dot_git.is_file() {
    return None;
  }
  let gitdir = fs::read_to_string(&dot_git).ok()?;
  let raw_path = gitdir.strip_prefix("gitdir:")?.trim();
  let path = Path::new(raw_path);
  Some(if path.is_absolute() {
    path.to_path_buf()
  } else {
    workspace_root.join(path)
  })
}

#[cfg(all(test, unix))]
mod tests {
  use std::{
    io::{
      BufRead,
      BufReader,
    },
    os::unix::net::UnixStream,
    thread,
    time::Duration,
  };

  use tempfile::tempdir;

  use super::*;

  #[test]
  fn second_client_is_rejected_without_replacing_owner() {
    let workspace = tempdir().unwrap();
    fs::create_dir(workspace.path().join(".git")).unwrap();
    let mut handle = PiBridgeHandle::start(workspace.path()).unwrap();
    let socket_path = socket_path_for_workspace(workspace.path());

    let mut first = UnixStream::connect(&socket_path).unwrap();
    first
      .set_write_timeout(Some(Duration::from_millis(250)))
      .unwrap();
    assert!(wait_for_event(&mut handle, |event| {
      matches!(event, PiBridgeEvent::Attached)
    }));

    send_request(&mut first, "1", "ping");
    assert!(wait_for_event(&mut handle, |event| {
      matches!(
        event,
        PiBridgeEvent::Request { id, method, .. } if id == "1" && method == "ping"
      )
    }));

    let second = UnixStream::connect(&socket_path).unwrap();
    let rejection = read_envelope(second).unwrap();
    assert!(matches!(
      rejection,
      PiBridgeEnvelope::Notification { method, params }
        if method == "attach_rejected"
          && params.get("reason").and_then(Value::as_str) == Some("busy")
    ));

    send_request(&mut first, "2", "ping");
    assert!(wait_for_event(&mut handle, |event| {
      matches!(
        event,
        PiBridgeEvent::Request { id, method, .. } if id == "2" && method == "ping"
      )
    }));
  }

  #[test]
  fn new_client_can_attach_after_owner_disconnects() {
    let workspace = tempdir().unwrap();
    fs::create_dir(workspace.path().join(".git")).unwrap();
    let mut handle = PiBridgeHandle::start(workspace.path()).unwrap();
    let socket_path = socket_path_for_workspace(workspace.path());

    let first = UnixStream::connect(&socket_path).unwrap();
    assert!(wait_for_event(&mut handle, |event| {
      matches!(event, PiBridgeEvent::Attached)
    }));
    drop(first);
    assert!(wait_for_event(&mut handle, |event| {
      matches!(event, PiBridgeEvent::Detached)
    }));

    let _second = UnixStream::connect(&socket_path).unwrap();
    assert!(wait_for_event(&mut handle, |event| {
      matches!(event, PiBridgeEvent::Attached)
    }));
  }

  fn wait_for_event(
    handle: &mut PiBridgeHandle,
    predicate: impl Fn(&PiBridgeEvent) -> bool,
  ) -> bool {
    for _ in 0..50 {
      if handle.drain_events().iter().any(&predicate) {
        return true;
      }
      thread::sleep(Duration::from_millis(20));
    }
    false
  }

  fn send_request(stream: &mut UnixStream, id: &str, method: &str) {
    let envelope = PiBridgeEnvelope::Request {
      id:     id.to_string(),
      method: method.to_string(),
      params: Value::Object(Default::default()),
    };
    write_envelope_to_stream(stream, &envelope).unwrap();
  }

  fn read_envelope(stream: UnixStream) -> Option<PiBridgeEnvelope> {
    let mut reader = BufReader::new(stream);
    reader
      .get_mut()
      .set_read_timeout(Some(Duration::from_millis(250)))
      .unwrap();
    let mut line = String::new();
    let bytes = reader.read_line(&mut line).ok()?;
    if bytes == 0 {
      return None;
    }
    serde_json::from_str(line.trim()).ok()
  }
}
