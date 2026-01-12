//! Path completion handler
//!
//! Provides file and directory completion when users type paths (e.g., `/`,
//! `./`, `~/`). Similar to Helix's path completion feature.

use std::{fs, path::PathBuf};

use ropey::Rope;
use the_editor_stdx::path::get_path_suffix;

use crate::{
  core::transaction::Transaction,
  handlers::completion::{CompletionItem, CompletionProvider, OtherCompletionItem},
};

/// Perform path completion at the given cursor position
///
/// Returns a vector of completion items for files and directories matching
/// the path prefix at the cursor position.
pub fn path_completion(
  text: ropey::RopeSlice<'_>,
  cursor: usize,
  doc_path: Option<&std::path::Path>,
) -> Vec<CompletionItem> {
  // Extract path suffix from text before cursor
  let text_before_cursor = text.slice(..cursor);
  let Some(path_suffix) = get_path_suffix(text_before_cursor, false) else {
    return Vec::new();
  };

  // Convert path suffix to string
  let path_str = path_suffix.to_string();

  // Expand tilde and environment variables
  let expanded_path = the_editor_stdx::path::expand(&path_str);

  // Determine the base directory and file prefix
  let (base_dir, file_prefix) = if expanded_path.is_dir() {
    (expanded_path.as_ref(), "")
  } else {
    let parent = expanded_path
      .parent()
      .unwrap_or_else(|| std::path::Path::new("."));
    let file_name = expanded_path
      .file_name()
      .and_then(|n| n.to_str())
      .unwrap_or("");
    (parent, file_name)
  };

  // Resolve base directory relative to document path or current working directory
  let base_dir = if base_dir.is_absolute() {
    base_dir.to_path_buf()
  } else if let Some(doc_path) = doc_path {
    if let Some(parent) = doc_path.parent() {
      parent.join(base_dir)
    } else {
      base_dir.to_path_buf()
    }
  } else {
    std::env::current_dir()
      .unwrap_or_else(|_| PathBuf::from("."))
      .join(base_dir)
  };

  // Read directory entries
  let Ok(entries) = fs::read_dir(&base_dir) else {
    return Vec::new();
  };

  let mut items = Vec::new();

  for entry in entries {
    let Ok(entry) = entry else {
      continue;
    };

    let entry_path = entry.path();
    let Some(file_name) = entry_path.file_name().and_then(|n| n.to_str()) else {
      continue;
    };

    // Filter by prefix if we have one
    if !file_prefix.is_empty() && !file_name.starts_with(file_prefix) {
      continue;
    }

    // Determine if this is a directory or file
    let metadata = match entry.metadata() {
      Ok(m) => m,
      Err(_) => continue,
    };

    let is_dir = metadata.is_dir();
    let kind = if is_dir {
      Some("folder".to_string())
    } else {
      Some("file".to_string())
    };

    // Create completion label (add trailing slash for directories)
    let label = if is_dir {
      format!("{}/", file_name)
    } else {
      file_name.to_string()
    };

    // Create documentation with full path
    let full_path = base_dir.join(file_name);
    let documentation = Some(format!("{}", full_path.display()));

    // Create a dummy transaction (not actually used, but kept for consistency)
    // The completion UI will create its own transaction when applying
    let rope = Rope::from_str("");
    let transaction = Transaction::new(&rope);

    items.push(CompletionItem::Other(OtherCompletionItem {
      transaction,
      label,
      kind,
      documentation,
      provider: CompletionProvider::Path,
    }));
  }

  // Sort items: directories first, then files, both alphabetically
  items.sort_by(|a, b| {
    let a_kind = match a {
      CompletionItem::Other(item) => item.kind.as_deref().unwrap_or(""),
      _ => "",
    };
    let b_kind = match b {
      CompletionItem::Other(item) => item.kind.as_deref().unwrap_or(""),
      _ => "",
    };

    match (a_kind == "folder", b_kind == "folder") {
      (true, false) => std::cmp::Ordering::Less,
      (false, true) => std::cmp::Ordering::Greater,
      _ => {
        let a_label = match a {
          CompletionItem::Other(item) => &item.label,
          _ => "",
        };
        let b_label = match b {
          CompletionItem::Other(item) => &item.label,
          _ => "",
        };
        a_label.cmp(b_label)
      },
    }
  });

  items
}
