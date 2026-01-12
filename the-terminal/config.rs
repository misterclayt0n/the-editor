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

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use super::*;

  #[test]
  fn test_terminal_config_default() {
    let config = TerminalConfig::default();

    assert!(config.shell.is_none());
    assert!(config.env.is_empty());
    assert!(config.working_directory.is_none());
    assert!(config.mouse_reporting);
    assert_eq!(config.scrollback_lines, 10000);
  }

  #[test]
  fn test_terminal_config_custom_shell() {
    let config = TerminalConfig {
      shell: Some("/bin/zsh".to_string()),
      ..Default::default()
    };

    assert_eq!(config.shell, Some("/bin/zsh".to_string()));
  }

  #[test]
  fn test_terminal_config_environment() {
    let config = TerminalConfig {
      env: vec![
        ("TERM".to_string(), "xterm-256color".to_string()),
        ("EDITOR".to_string(), "vim".to_string()),
      ],
      ..Default::default()
    };

    assert_eq!(config.env.len(), 2);
    assert_eq!(
      config.env[0],
      ("TERM".to_string(), "xterm-256color".to_string())
    );
    assert_eq!(config.env[1], ("EDITOR".to_string(), "vim".to_string()));
  }

  #[test]
  fn test_terminal_config_working_directory() {
    let config = TerminalConfig {
      working_directory: Some(PathBuf::from("/home/user")),
      ..Default::default()
    };

    assert_eq!(config.working_directory, Some(PathBuf::from("/home/user")));
  }

  #[test]
  fn test_terminal_config_mouse_reporting_disabled() {
    let config = TerminalConfig {
      mouse_reporting: false,
      ..Default::default()
    };

    assert!(!config.mouse_reporting);
  }

  #[test]
  fn test_terminal_config_scrollback_custom() {
    let config = TerminalConfig {
      scrollback_lines: 5000,
      ..Default::default()
    };

    assert_eq!(config.scrollback_lines, 5000);
  }

  #[test]
  fn test_terminal_config_all_custom() {
    let config = TerminalConfig {
      shell: Some("/bin/fish".to_string()),
      env: vec![("FOO".to_string(), "bar".to_string())],
      working_directory: Some(PathBuf::from("/tmp")),
      mouse_reporting: false,
      scrollback_lines: 20000,
    };

    assert_eq!(config.shell, Some("/bin/fish".to_string()));
    assert_eq!(config.env.len(), 1);
    assert_eq!(config.working_directory, Some(PathBuf::from("/tmp")));
    assert!(!config.mouse_reporting);
    assert_eq!(config.scrollback_lines, 20000);
  }
}
