//! PTY (pseudo-terminal) session management using pty-process
//!
//! This module handles spawning a shell process in a pseudo-terminal,
//! managing I/O between the shell and the terminal emulator.

use anyhow::Result;
use pty_process::{Command as PtyCommand, Pty, OwnedWritePty};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Mutex};

/// Represents a PTY session with a running shell process
///
/// Manages bidirectional communication between the application and the shell:
/// - Shell output is sent via `output_rx` channel
/// - Shell input is received via `input_tx` channel
pub struct PtySession {
  /// Write half of PTY (for sending input)
  pty_write: Arc<Mutex<OwnedWritePty>>,

  /// Child process handle
  child:    tokio::process::Child,

  /// Receiver for PTY output (bytes from shell)
  output_rx: mpsc::UnboundedReceiver<Vec<u8>>,

  /// Sender for PTY input (bytes to shell)
  input_tx:  mpsc::UnboundedSender<Vec<u8>>,

  /// Current dimensions
  rows:     u16,
  cols:     u16,
}

impl PtySession {
  /// Create a new PTY session and spawn a shell
  ///
  /// # Arguments
  /// * `rows` - Number of rows in the terminal
  /// * `cols` - Number of columns in the terminal
  /// * `shell` - Shell command to run (e.g., "/bin/bash"). If None, uses $SHELL or /bin/bash
  ///
  /// # Spawns background tasks
  /// - Reader task: PTY output → output_rx channel
  /// - Writer task: input_tx channel → PTY input
  pub fn new(rows: u16, cols: u16, shell: Option<&str>) -> Result<Self> {
    // Create PTY
    let pty = Pty::new()?;
    pty.resize(pty_process::Size::new(rows, cols))?;

    // Determine shell to use
    let shell_path = if let Some(s) = shell {
      s.to_string()
    } else if let Ok(s) = std::env::var("SHELL") {
      s
    } else {
      "/bin/bash".to_string()
    };

    log::debug!("Spawning shell: {}", shell_path);

    // Spawn shell process attached to PTY
    let mut cmd = PtyCommand::new(&shell_path);
    let child = cmd.spawn(&pty.pts()?)?;

    // Create channels for I/O
    let (output_tx, output_rx): (mpsc::UnboundedSender<Vec<u8>>, _) = mpsc::unbounded_channel();
    let (input_tx, input_rx): (mpsc::UnboundedSender<Vec<u8>>, _) = mpsc::unbounded_channel();

    // Split PTY into owned read and write halves
    let (pty_read, pty_write) = pty.into_split();
    let pty_write = Arc::new(Mutex::new(pty_write));

    // Spawn reader task: PTY output → channel
    {
      tokio::spawn(async move {
        let mut pty_read = pty_read;
        let mut buf = vec![0u8; 4096];
        loop {
          match pty_read.read(&mut buf).await {
            Ok(0) => {
              // EOF - shell process exited
              log::debug!("PTY EOF reached");
              break;
            }
            Ok(n) => {
              let data = buf[..n].to_vec();
              if output_tx.send(data).is_err() {
                // Receiver dropped, stop reading
                log::debug!("Output channel closed");
                break;
              }
            }
            Err(e) => {
              log::error!("PTY read error: {}", e);
              break;
            }
          }
        }
      });
    }

    // Spawn writer task: channel → PTY input
    {
      let pty_write_clone = pty_write.clone();
      let mut input_channel = input_rx;
      tokio::spawn(async move {
        while let Some(data) = input_channel.recv().await {
          let mut writer = pty_write_clone.lock().await;
          if let Err(e) = writer.write_all(&data).await {
            log::error!("PTY write error: {}", e);
            break;
          }
          if let Err(e) = writer.flush().await {
            log::error!("PTY flush error: {}", e);
            break;
          }
        }
      });
    }

    Ok(Self {
      pty_write,
      child,
      output_rx,
      input_tx,
      rows,
      cols,
    })
  }

  /// Try to receive PTY output (non-blocking)
  ///
  /// Returns Some(bytes) if data is available, None if no data pending.
  /// This should be called frequently (e.g., every frame) to keep up with shell output.
  pub fn try_recv_output(&mut self) -> Option<Vec<u8>> {
    self.output_rx.try_recv().ok()
  }

  /// Send input to the PTY (keyboard input to shell)
  ///
  /// # Arguments
  /// * `data` - Bytes to send (typically VT100 escape sequences or text)
  pub fn send_input(&self, data: Vec<u8>) -> Result<()> {
    self.input_tx.send(data)
      .map_err(|_| anyhow::anyhow!("Failed to send input to PTY"))
  }

  /// Resize the PTY
  ///
  /// This sends SIGWINCH to the shell process and updates the PTY size.
  pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
    // Update stored dimensions
    self.rows = rows;
    self.cols = cols;

    // Resize the PTY (sends SIGWINCH to shell)
    // OwnedWritePty::resize() is available and updates the terminal size
    let pty_write = self.pty_write.clone();
    let size = pty_process::Size::new(rows, cols);

    // We need to resize from an async context, but this is a sync method
    // Use blocking_lock since we're not in an async context
    match pty_write.try_lock() {
      Ok(writer) => writer.resize(size)?,
      Err(_) => {
        // If lock fails, queue resize for next write operation
        // This is safe because resize will happen before next write
        let pty_write_clone = pty_write.clone();
        tokio::spawn(async move {
          let writer = pty_write_clone.lock().await;
          if let Err(e) = writer.resize(size) {
            log::error!("PTY resize error: {}", e);
          }
        });
      }
    }

    Ok(())
  }

  /// Get current PTY dimensions
  pub fn size(&self) -> (u16, u16) {
    (self.rows, self.cols)
  }

  /// Check if child process is still alive
  ///
  /// Returns false if the process has exited.
  pub fn is_alive(&mut self) -> bool {
    self.child.try_wait().ok().flatten().is_none()
  }

  /// Kill the child process
  pub async fn kill(&mut self) -> Result<()> {
    self.child.kill().await?;
    Ok(())
  }
}

// Unit tests are in ../tests/pty_integration.rs to handle shell discovery
