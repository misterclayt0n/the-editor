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
