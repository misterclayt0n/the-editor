use std::{
  io::{
    BufRead,
    BufReader,
    BufWriter,
    Write,
  },
  path::Path,
  process::{
    Child,
    ChildStderr,
    ChildStdin,
    ChildStdout,
    Command,
    Stdio,
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
};

use thiserror::Error;
use tracing::debug;

use crate::jsonrpc;

#[derive(Debug, Clone)]
pub enum TransportEvent {
  Message(jsonrpc::Message),
  Stderr(String),
  ReadError(String),
  WriteError(String),
  Closed,
}

enum TransportOutbound {
  Message(jsonrpc::Message),
  Shutdown,
}

pub struct StdioTransport {
  child:         Child,
  outbound_tx:   Option<Sender<TransportOutbound>>,
  event_rx:      Receiver<TransportEvent>,
  reader_thread: Option<JoinHandle<()>>,
  writer_thread: Option<JoinHandle<()>>,
  stderr_thread: Option<JoinHandle<()>>,
}

impl StdioTransport {
  pub fn spawn(
    command: &str,
    args: &[String],
    env: &[(String, String)],
    workspace_root: &Path,
  ) -> Result<Self, TransportError> {
    let mut process = Command::new(command);
    process
      .args(args)
      .current_dir(workspace_root)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped());
    for (key, value) in env {
      process.env(key, value);
    }

    let mut child = process.spawn().map_err(TransportError::Spawn)?;
    let stdin = child
      .stdin
      .take()
      .ok_or(TransportError::MissingPipe("stdin"))?;
    let stdout = child
      .stdout
      .take()
      .ok_or(TransportError::MissingPipe("stdout"))?;
    let stderr = child
      .stderr
      .take()
      .ok_or(TransportError::MissingPipe("stderr"))?;

    let (outbound_tx, outbound_rx) = channel();
    let (event_tx, event_rx) = channel();

    let writer_thread = Some(spawn_writer_thread(stdin, outbound_rx, event_tx.clone()));
    let reader_thread = Some(spawn_reader_thread(stdout, event_tx.clone()));
    let stderr_thread = Some(spawn_stderr_thread(stderr, event_tx));

    Ok(Self {
      child,
      outbound_tx: Some(outbound_tx),
      event_rx,
      reader_thread,
      writer_thread,
      stderr_thread,
    })
  }

  pub fn send(&self, message: jsonrpc::Message) -> Result<(), TransportError> {
    let tx = self
      .outbound_tx
      .as_ref()
      .ok_or(TransportError::OutboundChannelClosed)?;
    tx.send(TransportOutbound::Message(message))
      .map_err(|_| TransportError::OutboundChannelClosed)
  }

  pub fn try_recv_event(&self) -> Option<TransportEvent> {
    match self.event_rx.try_recv() {
      Ok(event) => Some(event),
      Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
    }
  }

  pub fn poll_exit_code(&mut self) -> Result<Option<i32>, TransportError> {
    let status = self.child.try_wait().map_err(TransportError::Wait)?;
    Ok(status.and_then(|status| status.code()))
  }

  pub fn shutdown(&mut self) -> Result<Option<i32>, TransportError> {
    if let Some(tx) = self.outbound_tx.take() {
      let _ = tx.send(TransportOutbound::Shutdown);
    }

    let exit_code = match self.child.try_wait().map_err(TransportError::Wait)? {
      Some(status) => status.code(),
      None => {
        if let Err(err) = self.child.kill()
          && err.kind() != std::io::ErrorKind::InvalidInput
        {
          return Err(TransportError::Kill(err));
        }
        self.child.wait().map_err(TransportError::Wait)?.code()
      },
    };

    join_thread(&mut self.reader_thread)?;
    join_thread(&mut self.writer_thread)?;
    join_thread(&mut self.stderr_thread)?;

    Ok(exit_code)
  }
}

fn spawn_reader_thread(stdout: ChildStdout, event_tx: Sender<TransportEvent>) -> JoinHandle<()> {
  thread::Builder::new()
    .name("the-lsp-stdout".into())
    .spawn(move || {
      let mut reader = BufReader::new(stdout);
      let mut header_buffer = String::new();
      let mut body_buffer = Vec::new();

      loop {
        match read_frame(&mut reader, &mut header_buffer, &mut body_buffer) {
          Ok(Some(message)) => {
            let _ = event_tx.send(TransportEvent::Message(message));
          },
          Ok(None) => {
            let _ = event_tx.send(TransportEvent::Closed);
            break;
          },
          Err(err) => {
            let _ = event_tx.send(TransportEvent::ReadError(err.to_string()));
            break;
          },
        }
      }
    })
    .expect("failed to spawn lsp stdout thread")
}

fn spawn_writer_thread(
  stdin: ChildStdin,
  outbound_rx: Receiver<TransportOutbound>,
  event_tx: Sender<TransportEvent>,
) -> JoinHandle<()> {
  thread::Builder::new()
    .name("the-lsp-stdin".into())
    .spawn(move || {
      let mut writer = BufWriter::new(stdin);
      while let Ok(outbound) = outbound_rx.recv() {
        match outbound {
          TransportOutbound::Message(message) => {
            if let Err(err) = write_frame(&mut writer, &message) {
              let _ = event_tx.send(TransportEvent::WriteError(err.to_string()));
              break;
            }
          },
          TransportOutbound::Shutdown => break,
        }
      }
    })
    .expect("failed to spawn lsp stdin thread")
}

fn spawn_stderr_thread(stderr: ChildStderr, event_tx: Sender<TransportEvent>) -> JoinHandle<()> {
  thread::Builder::new()
    .name("the-lsp-stderr".into())
    .spawn(move || {
      let mut reader = BufReader::new(stderr);
      let mut line = String::new();
      loop {
        line.clear();
        match reader.read_line(&mut line) {
          Ok(0) => break,
          Ok(_) => {
            let line = line.trim_end_matches(['\r', '\n']).to_string();
            if !line.is_empty() {
              let _ = event_tx.send(TransportEvent::Stderr(line));
            }
          },
          Err(err) => {
            debug!(error = %err, "lsp stderr stream closed with error");
            break;
          },
        }
      }
    })
    .expect("failed to spawn lsp stderr thread")
}

fn read_frame<R: BufRead>(
  reader: &mut R,
  header_buffer: &mut String,
  body_buffer: &mut Vec<u8>,
) -> Result<Option<jsonrpc::Message>, TransportError> {
  let mut content_length: Option<usize> = None;
  loop {
    header_buffer.clear();
    let read = reader
      .read_line(header_buffer)
      .map_err(TransportError::Read)?;
    if read == 0 {
      return Ok(None);
    }

    if header_buffer == "\r\n" {
      if content_length.is_some() {
        break;
      }
      continue;
    }

    let header = header_buffer.trim_end_matches(['\r', '\n']);
    if let Some(rest) = header.strip_prefix("Content-Length:") {
      let value = rest.trim();
      let parsed = value
        .parse::<usize>()
        .map_err(|_| TransportError::InvalidContentLength(value.to_string()))?;
      content_length = Some(parsed);
    }
  }

  let content_length = content_length.ok_or(TransportError::MissingContentLength)?;
  body_buffer.resize(content_length, 0);
  reader
    .read_exact(body_buffer)
    .map_err(TransportError::ReadBody)?;
  let message = serde_json::from_slice(body_buffer).map_err(TransportError::ParseJson)?;
  body_buffer.clear();
  Ok(Some(message))
}

fn write_frame<W: Write>(writer: &mut W, message: &jsonrpc::Message) -> Result<(), TransportError> {
  let body = serde_json::to_vec(message).map_err(TransportError::SerializeJson)?;
  write!(writer, "Content-Length: {}\r\n\r\n", body.len()).map_err(TransportError::WriteHeader)?;
  writer.write_all(&body).map_err(TransportError::WriteBody)?;
  writer.flush().map_err(TransportError::Flush)?;
  Ok(())
}

fn join_thread(handle: &mut Option<JoinHandle<()>>) -> Result<(), TransportError> {
  if let Some(handle) = handle.take() {
    handle.join().map_err(|_| TransportError::ThreadPanicked)?;
  }
  Ok(())
}

#[derive(Debug, Error)]
pub enum TransportError {
  #[error("failed to spawn lsp process: {0}")]
  Spawn(std::io::Error),
  #[error("missing child {0} pipe")]
  MissingPipe(&'static str),
  #[error("transport outbound channel is closed")]
  OutboundChannelClosed,
  #[error("failed to read frame header: {0}")]
  Read(std::io::Error),
  #[error("invalid content-length header value: {0}")]
  InvalidContentLength(String),
  #[error("missing content-length header")]
  MissingContentLength,
  #[error("failed to read frame body: {0}")]
  ReadBody(std::io::Error),
  #[error("failed to parse json-rpc message: {0}")]
  ParseJson(serde_json::Error),
  #[error("failed to serialize json-rpc message: {0}")]
  SerializeJson(serde_json::Error),
  #[error("failed to write frame header: {0}")]
  WriteHeader(std::io::Error),
  #[error("failed to write frame body: {0}")]
  WriteBody(std::io::Error),
  #[error("failed to flush frame body: {0}")]
  Flush(std::io::Error),
  #[error("failed to kill lsp process: {0}")]
  Kill(std::io::Error),
  #[error("failed to wait for lsp process: {0}")]
  Wait(std::io::Error),
  #[error("transport thread panicked")]
  ThreadPanicked,
}
