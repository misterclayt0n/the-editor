//! Integration tests for PTY and TerminalSession functionality

use std::time::Duration;

use the_terminal::TerminalSession;

/// Find a shell that exists on this system
fn get_shell() -> &'static str {
  // Try common shell locations
  for shell in &["/bin/bash", "/usr/bin/bash", "/bin/sh", "/usr/bin/sh"] {
    if std::path::Path::new(shell).exists() {
      return shell;
    }
  }
  // If nothing found, try $SHELL env var, but have a final fallback
  // On Nix, bash might be at /nix/store/.../bin/bash
  "/bin/bash"
}

#[tokio::test]
async fn test_terminal_session_spawn() {
  let shell = get_shell();
  let session =
    TerminalSession::new(24, 80, Some(vec![shell.to_string()])).expect("Failed to create session");

  assert!(session.is_alive(), "Shell should be alive");
  assert_eq!(session.size(), (24, 80), "Size should match");
}

#[tokio::test]
async fn test_terminal_session_send_input() {
  let shell = get_shell();
  let session =
    TerminalSession::new(24, 80, Some(vec![shell.to_string()])).expect("Failed to create session");

  // Should not panic
  session
    .send_input(b"ls\n".to_vec())
    .expect("Send input should succeed");
}

#[tokio::test]
async fn test_terminal_session_echo_output() {
  let shell = get_shell();
  let session =
    TerminalSession::new(24, 80, Some(vec![shell.to_string()])).expect("Failed to create session");

  // Send echo command
  session
    .send_input(b"echo hello\n".to_vec())
    .expect("Send input should succeed");

  // Give the shell time to execute
  tokio::time::sleep(Duration::from_millis(200)).await;

  // Check if we got any output in the terminal grid
  let terminal = session.lock_terminal();
  let grid = terminal.grid();
  let mut found_char = false;

  for row in 0..grid.rows() {
    for col in 0..grid.cols() {
      let cell = grid.get(row, col);
      if let Some(ch) = cell.character() {
        // Check for any visible character (not just spaces)
        if !ch.is_whitespace() && (ch as u32) > 31 {
          found_char = true;
          break;
        }
      }
    }
    if found_char {
      break;
    }
  }

  assert!(found_char, "Expected to find output in terminal grid");
}

#[tokio::test]
async fn test_terminal_session_resize() {
  let shell = get_shell();
  let mut session =
    TerminalSession::new(24, 80, Some(vec![shell.to_string()])).expect("Failed to create session");

  session.resize(40, 100).expect("Resize should succeed");

  assert_eq!(session.size(), (40, 100), "Size should be updated");
  assert!(
    session.is_alive(),
    "Shell should still be alive after resize"
  );
}

#[tokio::test]
async fn test_terminal_session_multiple_commands() {
  let shell = get_shell();
  let session =
    TerminalSession::new(24, 80, Some(vec![shell.to_string()])).expect("Failed to create session");

  // Send first command
  session
    .send_input(b"echo first\n".to_vec())
    .expect("Send input should succeed");

  tokio::time::sleep(Duration::from_millis(100)).await;

  // Send second command
  session
    .send_input(b"echo second\n".to_vec())
    .expect("Send input should succeed");

  tokio::time::sleep(Duration::from_millis(100)).await;

  // Should still be alive
  assert!(session.is_alive(), "Shell should still be alive");
}
