//! Complete terminal session combining VT emulation and PTY management
//!
//! This module provides `TerminalSession` which wraps both the Terminal (VT emulation)
//! and PtySession (process management) into a single cohesive interface.

use crate::{Terminal, pty::PtySession};
use anyhow::Result;

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
///     // Update terminal with any pending PTY output
///     session.update();
///
///     // Get terminal state for rendering
///     let grid = session.terminal().grid();
///     // ... render grid to UI
///
///     // Handle user input
///     session.send_input(b"echo hello\n".to_vec())?;
///
///     if !session.is_alive() {
///         break; // Shell exited
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub struct TerminalSession {
  /// VT100 terminal emulator
  terminal: Terminal,

  /// PTY session with shell process
  pty:      PtySession,
}

impl TerminalSession {
  /// Create a new terminal session with a shell process
  ///
  /// # Arguments
  /// * `rows` - Number of terminal rows
  /// * `cols` - Number of terminal columns
  /// * `shell` - Optional shell command. If None, uses $SHELL or /bin/bash
  pub fn new(rows: u16, cols: u16, shell: Option<&str>) -> Result<Self> {
    let terminal = Terminal::new(cols, rows)?;
    let pty = PtySession::new(rows, cols, shell)?;

    Ok(Self { terminal, pty })
  }

  /// Update terminal with pending PTY output
  ///
  /// Reads any available data from the PTY and feeds it to the Terminal
  /// emulator to parse VT100 sequences and update grid state.
  ///
  /// This should be called frequently, typically once per frame.
  pub fn update(&mut self) {
    while let Some(data) = self.pty.try_recv_output() {
      // Convert bytes to string, handling UTF-8 errors gracefully
      match String::from_utf8_lossy(&data) {
        std::borrow::Cow::Borrowed(s) => {
          let _ = self.terminal.print_string(s);
        }
        std::borrow::Cow::Owned(s) => {
          let _ = self.terminal.print_string(&s);
        }
      }
    }
  }

  /// Send input to the PTY (keyboard input from user)
  ///
  /// # Arguments
  /// * `data` - Bytes to send to shell (e.g., keyboard input or VT100 sequences)
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

// Tests are in ../tests/pty_integration.rs to handle shell discovery
