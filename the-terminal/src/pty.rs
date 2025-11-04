//! PTY (pseudo-terminal) session management using pty-process
//!
//! This module handles spawning a shell process in a pseudo-terminal,
//! managing I/O between the shell and the terminal emulator.
//!
//! Architecture: Follows Ghostty's model with dedicated blocking read thread
//! for continuous PTY output processing independent of UI frame timing.

use std::{
  os::unix::io::RawFd,
  sync::{
    Arc,
    Mutex,
  },
  thread,
  time::Duration,
};

use anyhow::Result;
use pty_process::{
  Command as PtyCommand,
  OwnedWritePty,
  Pty,
};
use tokio::sync::mpsc;

/// Callback type for PTY output
///
/// Called from the dedicated read thread when data arrives from the PTY.
/// Must be thread-safe (Send + Sync).
pub type OutputCallback = Arc<dyn Fn(&[u8]) + Send + Sync>;

/// Represents a PTY session with a running shell process
///
/// Manages bidirectional communication between the application and the shell:
/// - Shell output is processed via callback in dedicated read thread
/// - Shell input is sent via channel to writer task
///
/// Architecture:
/// - Dedicated blocking read thread: Continuously reads PTY, calls callback
/// - Async writer task: Processes input_tx channel, writes to PTY (uses raw fd)
/// - Main thread: Sends input via send_input(), gets updates via callback
pub struct PtySession {
  /// Write half of PTY (for sending input and resize)
  pty_write: Arc<Mutex<OwnedWritePty>>,

  /// Child process handle
  child: Arc<Mutex<Option<tokio::process::Child>>>,

  /// Sender for PTY input (bytes to shell)
  input_tx: mpsc::UnboundedSender<Vec<u8>>,

  /// Handle to the read thread
  read_thread: Option<thread::JoinHandle<()>>,

  /// Raw PTY file descriptor (kept for read thread and writer task)
  _pty_fd: RawFd,

  /// Current dimensions
  rows: u16,
  cols: u16,
}

impl PtySession {
  /// Create a new PTY session and spawn a shell
  ///
  /// # Arguments
  /// * `rows` - Number of rows in the terminal
  /// * `cols` - Number of columns in the terminal
  /// * `shell` - Shell command with args (e.g., vec!["nu"]) or vec!["bash", "-l"]).
  ///   If None, uses $SHELL or /bin/bash
  /// * `on_output` - Callback invoked when PTY output arrives (called from read
  ///   thread)
  ///
  /// # Spawns background threads/tasks
  /// - Dedicated blocking read thread: Continuously reads PTY, calls callback
  /// - Async writer task: Processes input queue, writes to PTY
  pub fn new(rows: u16, cols: u16, shell: Option<Vec<String>>, on_output: OutputCallback) -> Result<Self> {
    // Create PTY
    let pty = Pty::new()?;
    pty.resize(pty_process::Size::new(rows, cols))?;

    // Determine shell to use
    let shell_cmd = if let Some(s) = shell {
      s
    } else if let Ok(s) = std::env::var("SHELL") {
      vec![s]
    } else {
      vec!["/bin/bash".to_string()]
    };

    log::debug!("Spawning shell: {:?}", shell_cmd);

    // Spawn shell process attached to PTY
    let mut cmd = PtyCommand::new(&shell_cmd[0]);
    if shell_cmd.len() > 1 {
      cmd.args(&shell_cmd[1..]);
    }

    // Set terminal environment variables so the shell knows it's in a proper
    // terminal TERM tells programs what terminal capabilities are available
    cmd.env("TERM", "xterm-256color");
    // COLORTERM indicates true color (24-bit) support
    cmd.env("COLORTERM", "truecolor");

    let child = cmd.spawn(&pty.pts()?)?;
    let child = Arc::new(Mutex::new(Some(child)));

    // Create channel for input (user → shell)
    let (input_tx, input_rx): (mpsc::UnboundedSender<Vec<u8>>, _) = mpsc::unbounded_channel();

    // Get raw file descriptor before splitting
    // We need this for poll() and fcntl() to set non-blocking mode
    use std::os::unix::io::AsRawFd;
    let pty_fd = pty.as_raw_fd();

    // Split PTY into owned read and write halves
    let (_pty_read, pty_write) = pty.into_split();
    let pty_write = Arc::new(Mutex::new(pty_write));
    // Note: We keep _pty_read to prevent the fd from being closed

    // Set PTY to non-blocking mode for tight read loop
    // SAFETY: We own the file descriptor and it's valid
    unsafe {
      let flags = libc::fcntl(pty_fd, libc::F_GETFL, 0);
      if flags == -1 {
        return Err(anyhow::anyhow!("fcntl F_GETFL failed"));
      }
      if libc::fcntl(pty_fd, libc::F_SETFL, flags | libc::O_NONBLOCK) == -1 {
        return Err(anyhow::anyhow!("fcntl F_SETFL failed"));
      }
    }

    // Spawn dedicated blocking read thread (Ghostty-style)
    // This thread continuously reads from PTY and calls the callback immediately
    let read_thread = thread::Builder::new()
      .name("pty-reader".to_string())
      .spawn(move || {
        let mut buf = [0u8; 4096];

        'main: loop {
          // Tight read loop: Read as much as possible before polling
          loop {
            // SAFETY: We set the fd to non-blocking above
            let n = unsafe { libc::read(pty_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };

            if n > 0 {
              // Got data - call callback immediately
              let data = &buf[..n as usize];
              on_output(data);
            } else if n == 0 {
              // EOF - shell process exited
              log::debug!("PTY EOF reached");
              break 'main;
            } else {
              // n < 0: Check error
              let errno = unsafe { *libc::__errno_location() };

              if errno == libc::EAGAIN || errno == libc::EWOULDBLOCK {
                // No more data available, break to poll
                break;
              } else if errno == libc::EIO || errno == libc::EBADF {
                // PTY closed
                log::debug!("PTY closed (errno: {})", errno);
                break 'main;
              } else {
                // Other error
                log::error!("PTY read error (errno: {})", errno);
                break 'main;
              }
            }
          }

          // Poll for more data (blocks until data available)
          let mut pollfd = libc::pollfd {
            fd:      pty_fd,
            events:  libc::POLLIN,
            revents: 0,
          };

          // SAFETY: pollfd is valid and we own the fd
          let poll_result = unsafe { libc::poll(&mut pollfd, 1, -1) };

          if poll_result < 0 {
            let errno = unsafe { *libc::__errno_location() };
            if errno != libc::EINTR {
              log::error!("poll error (errno: {})", errno);
              break 'main;
            }
            // EINTR: Interrupted by signal, retry
            continue;
          }

          if poll_result == 0 {
            // Timeout (shouldn't happen with -1 timeout)
            continue;
          }

          // Check if POLLIN is set (data available)
          if pollfd.revents & libc::POLLIN != 0 {
            // Data available, go back to reading
            continue;
          }

          // Check for errors or hangup
          if pollfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            log::debug!("PTY poll hangup/error (revents: {})", pollfd.revents);
            break 'main;
          }
        }

        log::debug!("PTY read thread exiting");
      })?;

    // Spawn async writer task: channel → PTY input
    // Uses raw libc::write() for simplicity
    {
      let pty_fd_for_write = pty_fd;
      let mut input_channel = input_rx;
      tokio::spawn(async move {
        while let Some(data) = input_channel.recv().await {
          // Write data to PTY using raw libc::write()
          // This is simple and works with the file descriptor directly
          let mut written = 0;
          while written < data.len() {
            let n = unsafe {
              libc::write(
                pty_fd_for_write,
                data[written..].as_ptr() as *const libc::c_void,
                data.len() - written,
              )
            };

            if n < 0 {
              let errno = unsafe { *libc::__errno_location() };
              if errno == libc::EINTR {
                // Interrupted, retry
                continue;
              } else {
                log::error!("PTY write error (errno: {})", errno);
                return;
              }
            }

            written += n as usize;
          }
        }
      });
    }

    Ok(Self {
      pty_write,
      child,
      input_tx,
      read_thread: Some(read_thread),
      _pty_fd: pty_fd,
      rows,
      cols,
    })
  }

  /// Send input to the PTY (keyboard input to shell)
  ///
  /// # Arguments
  /// * `data` - Bytes to send (typically VT100 escape sequences or text)
  pub fn send_input(&self, data: Vec<u8>) -> Result<()> {
    self
      .input_tx
      .send(data)
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

    // We need to resize from a sync context
    // Try to lock and resize immediately
    match pty_write.try_lock() {
      Ok(writer) => writer.resize(size)?,
      Err(_) => {
        // If lock fails, just log a warning
        // Resize will be applied on next successful lock
        log::warn!("Failed to resize PTY immediately (lock busy)");
      },
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
  pub fn is_alive(&self) -> bool {
    if let Ok(mut child_guard) = self.child.lock() {
      if let Some(child) = child_guard.as_mut() {
        return child.try_wait().ok().flatten().is_none();
      }
    }
    false
  }

  /// Kill the child process
  pub async fn kill(&mut self) -> Result<()> {
    if let Ok(mut child_guard) = self.child.lock() {
      if let Some(child) = child_guard.as_mut() {
        child.kill().await?;
      }
    }
    Ok(())
  }
}

impl Drop for PtySession {
  fn drop(&mut self) {
    log::debug!("Dropping PtySession, waiting for read thread to exit");

    // The read thread will exit automatically when the PTY fd is closed
    // (which happens when OwnedWritePty is dropped)
    // We just need to join the thread to ensure clean shutdown
    if let Some(handle) = self.read_thread.take() {
      // Give the thread a moment to notice PTY closure
      thread::sleep(Duration::from_millis(50));

      // Try to join with timeout
      match handle.join() {
        Ok(_) => log::debug!("PTY read thread exited cleanly"),
        Err(e) => log::error!("PTY read thread panicked: {:?}", e),
      }
    }
  }
}

// Unit tests are in ../tests/pty_integration.rs to handle shell discovery
