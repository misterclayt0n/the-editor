//! Tool display formatting for ACP tool calls.
//!
//! This module provides common formatting logic for displaying tool calls
//! in both the ACP overlay and the ACP buffer, ensuring consistent
//! presentation across different UI components.

use serde_json::Value;

/// Information needed to display a tool call.
#[derive(Debug, Clone)]
pub struct ToolDisplayInfo {
  /// Human-readable title (e.g., "read", "write", "bash")
  pub title:     String,
  /// Tool kind for icon selection
  pub kind:      ToolKind,
  /// Optional path associated with the tool call
  pub path:      Option<String>,
  /// Optional command for bash/execute tools
  pub command:   Option<String>,
  /// Current status of the tool call
  pub status:    ToolStatus,
  /// Optional error message if failed
  pub error_msg: Option<String>,
}

/// Tool kind categories for display purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolKind {
  Read,
  Write,
  Edit,
  Delete,
  Search,
  Execute,
  Think,
  Fetch,
  #[default]
  Other,
}

impl ToolKind {
  /// Parse a tool kind from an ACP ToolKind string.
  pub fn from_acp_kind(kind: &str) -> Self {
    match kind {
      "read" => Self::Read,
      "edit" => Self::Write, // Map edit to write for display purposes
      "delete" => Self::Delete,
      "move" => Self::Edit,
      "search" => Self::Search,
      "execute" => Self::Execute,
      "think" => Self::Think,
      "fetch" => Self::Fetch,
      _ => Self::Other,
    }
  }

  /// Infer tool kind from the tool title.
  pub fn from_title(title: &str) -> Self {
    let title_lower = title.to_lowercase();
    if title_lower.starts_with("read") || title_lower.contains("reading") {
      Self::Read
    } else if title_lower.starts_with("write") || title_lower.contains("writing") {
      Self::Write
    } else if title_lower.starts_with("edit") || title_lower.contains("editing") {
      Self::Edit
    } else if title_lower.starts_with("delete") || title_lower.contains("deleting") {
      Self::Delete
    } else if title_lower.starts_with("search")
      || title_lower.starts_with("grep")
      || title_lower.starts_with("glob")
      || title_lower.starts_with("find")
    {
      Self::Search
    } else if title_lower.starts_with("bash")
      || title_lower.starts_with("exec")
      || title_lower.starts_with("run")
    {
      Self::Execute
    } else if title_lower.starts_with("think") || title_lower.contains("thinking") {
      Self::Think
    } else if title_lower.starts_with("fetch") || title_lower.starts_with("webfetch") {
      Self::Fetch
    } else {
      Self::Other
    }
  }
}

/// Tool call status for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolStatus {
  #[default]
  Started,
  InProgress,
  Completed,
  Failed,
}

impl ToolDisplayInfo {
  /// Create a new ToolDisplayInfo from available data.
  pub fn new(
    title: String,
    kind: Option<ToolKind>,
    raw_input: Option<&Value>,
    status: ToolStatus,
    error_msg: Option<String>,
  ) -> Self {
    let inferred_kind = kind.unwrap_or_else(|| ToolKind::from_title(&title));

    // Extract path from raw_input if available
    let path = raw_input.and_then(|input| {
      // Try common path field names
      input
        .get("filePath")
        .or_else(|| input.get("path"))
        .or_else(|| input.get("file"))
        .and_then(|v| v.as_str())
        .map(String::from)
    });

    // Extract command from raw_input if available (for bash/execute tools)
    let command = raw_input.and_then(|input| {
      input
        .get("command")
        .or_else(|| input.get("cmd"))
        .and_then(|v| v.as_str())
        .map(String::from)
    });

    Self {
      title,
      kind: inferred_kind,
      path,
      command,
      status,
      error_msg,
    }
  }

  /// Format the tool display as a string for the ACP buffer.
  ///
  /// Returns format like:
  /// - `-> read /path/to/file`
  /// - `-> write /path/to/file`
  /// - `-> bash echo hello`
  /// - `<- read /path/to/file` (completed)
  /// - `x  read /path/to/file\n   error message` (failed - error on new line)
  pub fn format(&self) -> String {
    let icon = self.status_icon();
    let detail = self.format_detail();

    if let Some(ref err) = self.error_msg {
      if !err.is_empty() {
        // Put error message on a new line, indented to align with tool name
        format!("{} {}\n   {}", icon, detail, err)
      } else {
        format!("{} {}", icon, detail)
      }
    } else {
      format!("{} {}", icon, detail)
    }
  }

  /// Get the status icon.
  pub fn status_icon(&self) -> &'static str {
    match self.status {
      ToolStatus::Started | ToolStatus::InProgress => "->",
      ToolStatus::Completed => "<-",
      ToolStatus::Failed => "x ",
    }
  }

  /// Format the detail part (tool name + path/command).
  fn format_detail(&self) -> String {
    // Normalize the title to a clean tool name
    let tool_name = self.normalize_tool_name();

    match self.kind {
      ToolKind::Read | ToolKind::Write | ToolKind::Edit | ToolKind::Delete => {
        if let Some(ref path) = self.path {
          format!("{} {}", tool_name, path)
        } else {
          // Fall back to title if it contains a path
          self.extract_path_from_title().unwrap_or(tool_name)
        }
      },
      ToolKind::Execute => {
        if let Some(ref cmd) = self.command {
          // Truncate long commands
          let display_cmd = truncate_command(cmd, 60);
          format!("{} {}", tool_name, display_cmd)
        } else {
          // Fall back to title if it contains a command
          self.extract_command_from_title().unwrap_or(tool_name)
        }
      },
      _ => {
        // For other tools, use path if available, otherwise just the name
        if let Some(ref path) = self.path {
          format!("{} {}", tool_name, path)
        } else if let Some(ref cmd) = self.command {
          let display_cmd = truncate_command(cmd, 60);
          format!("{} {}", tool_name, display_cmd)
        } else {
          tool_name
        }
      },
    }
  }

  /// Normalize the tool title to a clean name.
  fn normalize_tool_name(&self) -> String {
    let title = &self.title;

    // If title is a simple tool name, return as-is
    if !title.contains(' ') && !title.contains('/') {
      return title.to_lowercase();
    }

    // Extract just the tool name from titles like "read /path/to/file"
    if let Some(first_word) = title.split_whitespace().next() {
      // Check if it's a known tool name
      let lower = first_word.to_lowercase();
      if matches!(
        lower.as_str(),
        "read" | "write" | "edit" | "bash" | "grep" | "glob" | "task" | "webfetch"
      ) {
        return lower;
      }
    }

    // Return the title as-is if we can't normalize it
    title.to_string()
  }

  /// Try to extract a path from the title string.
  fn extract_path_from_title(&self) -> Option<String> {
    // Look for something that looks like a path (starts with / or contains /)
    for word in self.title.split_whitespace().skip(1) {
      if word.starts_with('/') || word.contains('/') {
        return Some(format!("{} {}", self.normalize_tool_name(), word));
      }
    }
    None
  }

  /// Try to extract a command from the title string.
  fn extract_command_from_title(&self) -> Option<String> {
    // Skip the first word (tool name) and use the rest as command
    let parts: Vec<&str> = self.title.splitn(2, ' ').collect();
    if parts.len() > 1 {
      let cmd = truncate_command(parts[1], 60);
      return Some(format!("{} {}", self.normalize_tool_name(), cmd));
    }
    None
  }
}

/// Truncate a command string for display.
fn truncate_command(cmd: &str, max_len: usize) -> String {
  // Take only the first line
  let first_line = cmd.lines().next().unwrap_or(cmd);

  if first_line.len() <= max_len {
    first_line.to_string()
  } else {
    format!("{}...", &first_line[..max_len.saturating_sub(3)])
  }
}

/// Parse a tool marker line into display info.
///
/// Handles formats like:
/// - `[TOOL:start:read]`
/// - `[TOOL:done:write]`
/// - `[TOOL:error:bash:command failed]`
/// - `[TOOL:progress:read:loading]`
pub fn parse_tool_marker(line: &str) -> Option<ToolDisplayInfo> {
  let trimmed = line.trim();
  if !trimmed.starts_with("[TOOL:") || !trimmed.ends_with(']') {
    return None;
  }

  let content = trimmed.strip_prefix("[TOOL:")?.strip_suffix(']')?;

  let parts: Vec<&str> = content.splitn(3, ':').collect();
  if parts.len() < 2 {
    return None;
  }

  let status_str = parts[0];
  let name = parts[1];
  let details = parts.get(2).unwrap_or(&"");

  let (status, error_msg) = match status_str {
    "start" => (ToolStatus::Started, None),
    "done" => (ToolStatus::Completed, None),
    "error" => (ToolStatus::Failed, Some(details.to_string())),
    "progress" => (ToolStatus::InProgress, None),
    _ => (ToolStatus::Started, None),
  };

  Some(ToolDisplayInfo::new(
    name.to_string(),
    None,
    None,
    status,
    error_msg,
  ))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_format_read_with_path() {
    let info = ToolDisplayInfo::new(
      "read".to_string(),
      Some(ToolKind::Read),
      Some(&serde_json::json!({"filePath": "/home/user/file.rs"})),
      ToolStatus::Started,
      None,
    );
    assert_eq!(info.format(), "-> read /home/user/file.rs");
  }

  #[test]
  fn test_format_bash_with_command() {
    let info = ToolDisplayInfo::new(
      "bash".to_string(),
      Some(ToolKind::Execute),
      Some(&serde_json::json!({"command": "cargo build"})),
      ToolStatus::Started,
      None,
    );
    assert_eq!(info.format(), "-> bash cargo build");
  }

  #[test]
  fn test_format_completed() {
    let info = ToolDisplayInfo::new(
      "write".to_string(),
      Some(ToolKind::Write),
      Some(&serde_json::json!({"filePath": "/tmp/test.txt"})),
      ToolStatus::Completed,
      None,
    );
    assert_eq!(info.format(), "<- write /tmp/test.txt");
  }

  #[test]
  fn test_format_failed() {
    let info = ToolDisplayInfo::new(
      "read".to_string(),
      Some(ToolKind::Read),
      None,
      ToolStatus::Failed,
      Some("file not found".to_string()),
    );
    // Error message is on a new line, indented
    assert_eq!(info.format(), "x  read\n   file not found");
  }

  #[test]
  fn test_parse_tool_marker() {
    let info = parse_tool_marker("[TOOL:start:read]").unwrap();
    assert_eq!(info.title, "read");
    assert_eq!(info.status, ToolStatus::Started);
    assert_eq!(info.format(), "-> read");
  }

  #[test]
  fn test_parse_tool_marker_with_path_in_title() {
    let info = parse_tool_marker("[TOOL:start:read /home/user/file.rs]").unwrap();
    assert_eq!(info.format(), "-> read /home/user/file.rs");
  }

  #[test]
  fn test_infer_kind_from_title() {
    assert_eq!(ToolKind::from_title("read"), ToolKind::Read);
    assert_eq!(ToolKind::from_title("write"), ToolKind::Write);
    assert_eq!(ToolKind::from_title("bash"), ToolKind::Execute);
    assert_eq!(ToolKind::from_title("grep"), ToolKind::Search);
  }
}
