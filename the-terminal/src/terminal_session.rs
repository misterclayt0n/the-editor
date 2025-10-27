//! Complete terminal session combining VT emulation and PTY management
//!
//! This module provides `TerminalSession` which wraps both the Terminal (VT
//! emulation) and PtySession (process management) into a single cohesive
//! interface.

use std::{
  ffi::c_void,
  sync::Arc,
};

use anyhow::Result;
use crossbeam::queue::ArrayQueue;

use crate::{
  Terminal,
  pty::PtySession,
};

/// A complete terminal session: VT100 emulation + PTY process
///
/// This combines:
/// - `Terminal`: Parses VT100 escape sequences and maintains terminal state
/// - `PtySession`: Manages the shell process and I/O
///
/// Usage pattern:
/// ```no_run
/// # use the_terminal::TerminalSession;
/// # async fn example() -> anyhow::Result<()> {
/// let mut session = TerminalSession::new(24, 80, None)?;
///
/// // In your render loop:
/// loop {
///   // Update terminal with any pending PTY output
///   session.update();
///
///   // Get terminal state for rendering
///   let grid = session.terminal().grid();
///   // ... render grid to UI
///
///   // Handle user input
///   session.send_input(b"echo hello\n".to_vec())?;
///
///   if !session.is_alive() {
///     break; // Shell exited
///   }
/// }
/// # Ok(())
/// # }
/// ```
pub struct TerminalSession {
  /// VT100 terminal emulator
  terminal: Terminal,

  /// PTY session with shell process
  pty: PtySession,

  /// Lock-free queue for terminal responses (e.g., cursor position reports)
  /// Bounded to prevent unbounded memory growth. Callbacks push here,
  /// update() drains and sends to PTY.
  response_queue: Arc<ArrayQueue<Vec<u8>>>,

  /// Raw pointer to callback context (Arc<ArrayQueue>)
  /// Must be cleaned up in Drop by reconstructing the Arc
  callback_ctx_ptr: *mut c_void,
}

impl TerminalSession {
  /// Create a new terminal session with a shell process
  ///
  /// # Arguments
  /// * `rows` - Number of terminal rows
  /// * `cols` - Number of terminal columns
  /// * `shell` - Optional shell command. If None, uses $SHELL or /bin/bash
  pub fn new(rows: u16, cols: u16, shell: Option<&str>) -> Result<Self> {
    let mut terminal = Terminal::new(cols, rows)?;
    let pty = PtySession::new(rows, cols, shell)?;

    // Create bounded queue for terminal responses (capacity 64, matching ghostty)
    // This is lock-free and can be safely called from any thread
    let response_queue = Arc::new(ArrayQueue::new(64));

    // CRITICAL: Clone the Arc BEFORE converting to raw pointer.
    // Arc::into_raw consumes the Arc without incrementing refcount.
    // We need TWO strong references:
    //   1. response_queue field (keeps data alive)
    //   2. callback_ctx_ptr (for FFI callback access)
    let queue_for_callback = Arc::clone(&response_queue);
    let queue_ptr = Arc::into_raw(queue_for_callback) as *mut c_void;

    // SAFETY: The queue pointer will remain valid because:
    // - response_queue field holds one strong Arc reference
    // - callback_ctx_ptr holds another strong reference (as raw pointer)
    // - In Drop, we reconstruct the Arc from raw to properly decrement refcount
    unsafe {
      terminal.set_response_callback_raw(Self::response_callback, queue_ptr);
    }

    Ok(Self {
      terminal,
      pty,
      response_queue,
      callback_ctx_ptr: queue_ptr,
    })
  }

  /// Callback invoked by the terminal when it needs to send responses to the
  /// PTY
  ///
  /// # Safety
  /// This is called from Zig FFI and must be thread-safe.
  /// The ctx pointer is an Arc<ArrayQueue> from Arc::into_raw.
  /// ArrayQueue is lock-free so this is safe to call from any thread.
  extern "C" fn response_callback(ctx: *mut c_void, data: *const u8, len: usize) {
    // SAFETY: ctx is a pointer to ArrayQueue<Vec<u8>> obtained via Arc::into_raw in
    // new(). We reborrow the queue without adjusting the strong count;
    // ownership stays with the Arc in TerminalSession.
    let queue = unsafe { &*(ctx as *const ArrayQueue<Vec<u8>>) };

    // Convert the slice to a Vec
    let response = unsafe { std::slice::from_raw_parts(data, len).to_vec() };

    // Debug: log what we're sending
    log::debug!(
      "Terminal response ({} bytes): {:?}",
      len,
      String::from_utf8_lossy(&response)
    );

    // Push to queue, drop if full (backpressure)
    // This is lock-free and safe from any thread
    if queue.push(response).is_err() {
      log::warn!("Terminal response queue full, dropping message");
    }
  }

  /// Update terminal with pending PTY output
  ///
  /// Reads any available data from the PTY and feeds it to the Terminal
  /// emulator to parse VT100 sequences and update grid state.
  ///
  /// This should be called frequently, typically once per frame.
  pub fn update(&mut self) {
    // First, drain response queue and send to PTY
    // Rate limit to prevent infinite loops (max 16 responses per frame)
    const MAX_RESPONSES_PER_FRAME: usize = 16;

    for _ in 0..MAX_RESPONSES_PER_FRAME {
      match self.response_queue.pop() {
        Some(response) => {
          // Send response to PTY input (shell's stdin)
          if let Err(e) = self.pty.send_input(response) {
            log::warn!("Failed to send terminal response to PTY: {}", e);
            break; // Stop if PTY is closed
          }
        },
        None => break, // Queue empty
      }
    }

    // Then process PTY output as before
    while let Some(data) = self.pty.try_recv_output() {
      // Write raw bytes directly to the terminal
      // The terminal's Stream parser will handle VT100/ANSI escape sequences
      let _ = self.terminal.write(&data);
    }
  }

  /// Send input to the PTY (keyboard input from user)
  ///
  /// # Arguments
  /// * `data` - Bytes to send to shell (e.g., keyboard input or VT100
  ///   sequences)
  ///
  /// # Example
  /// ```no_run
  /// # use the_terminal::TerminalSession;
  /// # async fn example() -> anyhow::Result<()> {
  /// # let mut session = TerminalSession::new(24, 80, None)?;
  /// // Send "echo hello"
  /// session.send_input(b"echo hello\n".to_vec())?;
  /// # Ok(())
  /// # }
  /// ```
  pub fn send_input(&self, data: Vec<u8>) -> Result<()> {
    self.pty.send_input(data)
  }

  /// Get the terminal emulator
  ///
  /// Use this to access the terminal grid for rendering.
  pub fn terminal(&self) -> &Terminal {
    &self.terminal
  }

  /// Get mutable reference to terminal
  ///
  /// Usually not needed - use `terminal()` for read-only access.
  pub fn terminal_mut(&mut self) -> &mut Terminal {
    &mut self.terminal
  }

  /// Resize the terminal and PTY
  ///
  /// This updates both the Terminal emulation size and the PTY size,
  /// sending SIGWINCH to the shell process.
  pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
    // Resize Terminal emulation
    self.terminal.resize(cols, rows)?;

    // Resize PTY (sends SIGWINCH to shell)
    self.pty.resize(rows, cols)?;

    Ok(())
  }

  /// Get current terminal dimensions
  pub fn size(&self) -> (u16, u16) {
    self.pty.size()
  }

  /// Check if the shell process is still alive
  ///
  /// Returns false when the shell has exited.
  pub fn is_alive(&mut self) -> bool {
    self.pty.is_alive()
  }

  /// Kill the shell process
  pub async fn kill(&mut self) -> Result<()> {
    self.pty.kill().await
  }
}

impl Drop for TerminalSession {
  fn drop(&mut self) {
    // Clean up the callback context pointer by reconstructing the Arc
    // This properly decrements the refcount and frees memory if needed
    if !self.callback_ctx_ptr.is_null() {
      log::debug!("Cleaning up terminal callback context");

      // SAFETY: This pointer was created with Arc::into_raw in new()
      // Reconstructing the Arc will properly decrement the refcount
      unsafe {
        let _arc = Arc::from_raw(self.callback_ctx_ptr as *const ArrayQueue<Vec<u8>>);
        // Arc is dropped here, decrementing refcount
      }

      // Prevent double-free
      self.callback_ctx_ptr = std::ptr::null_mut();
    }
  }
}

// Tests are in ../tests/pty_integration.rs to handle shell discovery
