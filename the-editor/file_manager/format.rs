use std::{
  fs,
  path::{
    Path,
  },
  time::SystemTime,
};

use anyhow::Result;

use crate::core::document::FileManagerEntryState;

/// Represents a single file entry as displayed in the buffer
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
  pub name:        String,
  pub is_dir:      bool,
  pub permissions: String,
  pub size:        u64,
  pub modified:    SystemTime,
  pub user:        String,
  pub group:       String,
}

impl FileEntry {
  /// Format this entry as an ls -l style line
  pub fn format(&self) -> String {
    let name_display = if self.is_dir {
      format!("{}/", self.name)
    } else {
      self.name.clone()
    };

    let modified = format_time(self.modified);
    let size = format_size(self.size);

    format!(
      "{} {:>2} {:<8} {:<8} {:>6} {} {}",
      self.permissions, 1, // link count (simplified to 1)
      self.user, self.group, size, modified, name_display
    )
  }
}

/// Parse directory contents into file entries
pub fn read_directory(path: &Path, show_hidden: bool) -> Result<Vec<FileEntry>> {
  let mut entries = Vec::new();

  for entry in fs::read_dir(path)? {
    let entry = entry?;
    let file_name = entry.file_name();
    let name = file_name.to_string_lossy().to_string();

    // Skip hidden files if not showing them
    if !show_hidden && name.starts_with('.') {
      continue;
    }

    let metadata = entry.metadata()?;
    let is_dir = metadata.is_dir();

    entries.push(FileEntry {
      name,
      is_dir,
      permissions: format_permissions(&metadata),
      size: metadata.len(),
      modified: metadata.modified()?,
      user: get_user(),
      group: get_group(),
    });
  }

  // Sort: directories first, then files, both alphabetically
  entries.sort_by(|a, b| {
    match (a.is_dir, b.is_dir) {
      (true, false) => std::cmp::Ordering::Less,
      (false, true) => std::cmp::Ordering::Greater,
      _ => a.name.cmp(&b.name),
    }
  });

  Ok(entries)
}

/// Generate the complete buffer content for a directory
pub fn generate_buffer_content(path: &Path, show_hidden: bool) -> Result<String> {
  let entries = read_directory(path, show_hidden)?;
  let mut lines = vec!["../".to_string()];

  for entry in entries {
    lines.push(entry.format());
  }

  Ok(lines.join("\n"))
}

/// Parse a single line from the buffer back into a filename
/// Returns None for the parent directory marker "../"
pub fn parse_line(line: &str) -> Option<FileManagerEntryState> {
  let line = line.trim();

  // Parent directory marker
  if line == "../" {
    return None;
  }

  // Empty line
  if line.is_empty() {
    return None;
  }

  // The formatted line always ends with a single space followed by the name
  let (name_part, is_dir) = if let Some((_, name)) = line.rsplit_once(' ') {
    let is_dir = name.ends_with('/');
    let name = name.strip_suffix('/').unwrap_or(name).to_string();
    (name, is_dir)
  } else {
    let is_dir = line.ends_with('/');
    let name = line.strip_suffix('/').unwrap_or(line).to_string();
    (name, is_dir)
  };

  Some(FileManagerEntryState::new(name_part, is_dir))
}

/// Parse the entire buffer content into a list of filenames
pub fn parse_buffer_content(content: &str) -> Vec<FileManagerEntryState> {
  content
    .lines()
    .filter_map(parse_line)
    .collect()
}

// Platform-specific helpers

#[cfg(unix)]
fn format_permissions(metadata: &fs::Metadata) -> String {
  use std::os::unix::fs::PermissionsExt;

  let mode = metadata.permissions().mode();
  let file_type = if metadata.is_dir() {
    'd'
  } else if metadata.is_symlink() {
    'l'
  } else {
    '-'
  };

  let user = format!(
    "{}{}{}",
    if mode & 0o400 != 0 { 'r' } else { '-' },
    if mode & 0o200 != 0 { 'w' } else { '-' },
    if mode & 0o100 != 0 { 'x' } else { '-' }
  );

  let group = format!(
    "{}{}{}",
    if mode & 0o040 != 0 { 'r' } else { '-' },
    if mode & 0o020 != 0 { 'w' } else { '-' },
    if mode & 0o010 != 0 { 'x' } else { '-' }
  );

  let other = format!(
    "{}{}{}",
    if mode & 0o004 != 0 { 'r' } else { '-' },
    if mode & 0o002 != 0 { 'w' } else { '-' },
    if mode & 0o001 != 0 { 'x' } else { '-' }
  );

  format!("{}{}{}{}", file_type, user, group, other)
}

#[cfg(not(unix))]
fn format_permissions(metadata: &fs::Metadata) -> String {
  let file_type = if metadata.is_dir() { 'd' } else { '-' };
  let perms = if metadata.permissions().readonly() {
    "r--r--r--"
  } else {
    "rw-rw-rw-"
  };
  format!("{}{}", file_type, perms)
}

#[cfg(unix)]
fn get_user() -> String {
  std::env::var("USER").unwrap_or_else(|_| "user".to_string())
}

#[cfg(not(unix))]
fn get_user() -> String {
  std::env::var("USERNAME").unwrap_or_else(|_| "user".to_string())
}

#[cfg(unix)]
fn get_group() -> String {
  // Simplified: just return a generic group name
  // A full implementation would use libc to get the actual group
  "users".to_string()
}

#[cfg(not(unix))]
fn get_group() -> String {
  "users".to_string()
}

fn format_time(time: SystemTime) -> String {
  use std::time::UNIX_EPOCH;

  let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
  let secs = duration.as_secs();

  // Convert to local time approximation (simplified)
  let tm = chrono::DateTime::from_timestamp(secs as i64, 0)
    .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());

  tm.format("%b %d %H:%M").to_string()
}

fn format_size(size: u64) -> String {
  if size < 1024 {
    format!("{}", size)
  } else if size < 1024 * 1024 {
    format!("{}K", size / 1024)
  } else if size < 1024 * 1024 * 1024 {
    format!("{}M", size / (1024 * 1024))
  } else {
    format!("{}G", size / (1024 * 1024 * 1024))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::document::FileManagerEntryState;

  #[test]
  fn parse_line_extracts_filename() {
    let line = "drwxr-xr-x  2 user users   4096 Sep 26 13:59 assets/";
    assert_eq!(
      parse_line(line),
      Some(FileManagerEntryState::new("assets", true))
    );

    let line = "-rw-r--r--  1 user users   8117 Oct 10 15:20 CLAUDE.md";
    assert_eq!(
      parse_line(line),
      Some(FileManagerEntryState::new("CLAUDE.md", false))
    );
  }

  #[test]
  fn parse_line_handles_spaces_in_names() {
    let line = "-rw-r--r--  1 user users   1234 Oct 10 15:20 my file.txt";
    assert_eq!(
      parse_line(line),
      Some(FileManagerEntryState::new("my file.txt", false))
    );
  }

  #[test]
  fn parse_line_skips_parent_marker() {
    assert_eq!(parse_line("../"), None);
  }

  #[test]
  fn parse_buffer_content_extracts_all_names() {
    let content = r#"../
drwxr-xr-x  2 user users   4096 Sep 26 13:59 assets/
-rw-r--r--  1 user users   8117 Oct 10 15:20 CLAUDE.md
-rw-r--r--  1 user users 129412 Oct 16 19:33 Cargo.lock"#;

    let names = parse_buffer_content(content);
    assert_eq!(
      names,
      vec![
        FileManagerEntryState::new("assets", true),
        FileManagerEntryState::new("CLAUDE.md", false),
        FileManagerEntryState::new("Cargo.lock", false),
      ]
    );
  }

  #[test]
  fn format_size_uses_human_readable_units() {
    assert_eq!(format_size(512), "512");
    assert_eq!(format_size(2048), "2K");
    assert_eq!(format_size(2 * 1024 * 1024), "2M");
    assert_eq!(format_size(3 * 1024 * 1024 * 1024), "3G");
  }
}
