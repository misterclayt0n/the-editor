//! Terminal configuration.

/// Configuration for terminal behavior.
#[derive(Debug, Clone)]
pub struct TerminalConfig {
  /// Default shell to use. If None, uses $SHELL or /bin/sh.
  pub shell: Option<String>,

  /// Environment variables to set for the shell.
  pub env: Vec<(String, String)>,

  /// Working directory for the shell. If None, uses current directory.
  pub working_directory: Option<std::path::PathBuf>,

  /// Enable mouse reporting to the terminal.
  pub mouse_reporting: bool,

  /// Scrollback buffer size in lines.
  pub scrollback_lines: usize,
}

impl Default for TerminalConfig {
  fn default() -> Self {
    Self {
      shell: None,
      env: Vec::new(),
      working_directory: None,
      mouse_reporting: true,
      scrollback_lines: 10000,
    }
  }
}
