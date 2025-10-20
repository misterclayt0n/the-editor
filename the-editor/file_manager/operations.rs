use std::{
  collections::{
    HashMap,
    HashSet,
  },
  fs,
  path::PathBuf,
};

use anyhow::{
  Context,
  Result,
};

use crate::{
  core::document::{
    Document,
    FileManagerEntryState,
  },
  file_manager::{
    buffer,
    format,
  },
};

/// Represents a file system operation to be performed
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOperation {
  Create { name: String, is_dir: bool },
  Delete { name: String, is_dir: bool },
  Rename { from: String, to: String, is_dir: bool },
}

impl FileOperation {
  fn display_name(name: &str, is_dir: bool) -> String {
    if is_dir {
      format!("{name}/")
    } else {
      name.to_string()
    }
  }

  pub fn description(&self) -> String {
    match self {
      FileOperation::Create { name, is_dir } => {
        format!("Create: {}", Self::display_name(name, *is_dir))
      },
      FileOperation::Delete { name, is_dir } => {
        format!("Delete: {}", Self::display_name(name, *is_dir))
      },
      FileOperation::Rename { from, to, is_dir } => format!(
        "Rename: {} -> {}",
        Self::display_name(from, *is_dir),
        Self::display_name(to, *is_dir)
      ),
    }
  }
}

/// Compute the list of operations needed to transform the original state to the current buffer
/// state
pub fn compute_operations(doc: &Document) -> Result<Vec<FileOperation>> {
  let buffer_content = doc.text().to_string();
  let current_state = format::parse_buffer_content(&buffer_content);

  let original_state = buffer::original_state(doc)
    .context("Failed to get original state from file manager buffer")?;

  Ok(diff_states(&original_state, &current_state))
}

/// Execute the file operations in the given directory
pub fn execute_operations(
  directory: &PathBuf,
  operations: &[FileOperation],
) -> Result<Vec<(FileOperation, Result<()>)>> {
  let mut results = Vec::new();

  for op in operations {
    let result = execute_single_operation(directory, op);
    results.push((op.clone(), result));
  }

  Ok(results)
}

fn execute_single_operation(directory: &PathBuf, op: &FileOperation) -> Result<()> {
  match op {
    FileOperation::Create { name, is_dir } => {
      let path = directory.join(name);

      if *is_dir {
        fs::create_dir(&path)
          .with_context(|| format!("Failed to create directory: {}", path.display()))?;
      } else {
        // Create empty file
        fs::write(&path, "")
          .with_context(|| format!("Failed to create file: {}", path.display()))?;
      }
    },
    FileOperation::Delete { name, is_dir } => {
      let path = directory.join(name);

      if *is_dir || path.is_dir() {
        fs::remove_dir_all(&path)
          .with_context(|| format!("Failed to delete directory: {}", path.display()))?;
      } else {
        fs::remove_file(&path)
          .with_context(|| format!("Failed to delete file: {}", path.display()))?;
      }
    },
    FileOperation::Rename { from, to, .. } => {
      let from_path = directory.join(from);
      let to_path = directory.join(to);

      fs::rename(&from_path, &to_path).with_context(|| {
        format!(
          "Failed to rename {} to {}",
          from_path.display(),
          to_path.display()
        )
      })?;
    },
  }

  Ok(())
}

/// Format operations summary for the confirmation dialog
pub fn format_operations_summary(operations: &[FileOperation]) -> String {
  if operations.is_empty() {
    return "No changes to apply".to_string();
  }

  let creates = operations
    .iter()
    .filter(|op| matches!(op, FileOperation::Create { .. }))
    .count();
  let deletes = operations
    .iter()
    .filter(|op| matches!(op, FileOperation::Delete { .. }))
    .count();
  let renames = operations
    .iter()
    .filter(|op| matches!(op, FileOperation::Rename { .. }))
    .count();

  let mut summary = format!(
    "{} operation(s): {} create, {} delete, {} rename\n\n",
    operations.len(),
    creates,
    deletes,
    renames
  );

  for op in operations {
    summary.push_str(&format!("  {}\n", op.description()));
  }

  summary
}

fn diff_states(
  original_state: &[FileManagerEntryState],
  current_state: &[FileManagerEntryState],
) -> Vec<FileOperation> {
  let mut operations = Vec::new();

  let original_set: HashSet<&str> = original_state.iter().map(|s| s.name.as_str()).collect();
  let current_set: HashSet<&str> = current_state.iter().map(|s| s.name.as_str()).collect();

  let original_by_pos: HashMap<usize, &FileManagerEntryState> =
    original_state.iter().enumerate().map(|(i, s)| (i, s)).collect();
  let current_by_pos: HashMap<usize, &FileManagerEntryState> =
    current_state.iter().enumerate().map(|(i, s)| (i, s)).collect();

  let mut processed = HashSet::new();

  for (pos, current_entry) in &current_by_pos {
    if let Some(original_entry) = original_by_pos.get(pos) {
      let original_entry = *original_entry;
      if current_entry.name != original_entry.name
        && current_entry.is_dir == original_entry.is_dir
        && !processed.contains(&original_entry.name)
      {
        if !current_set.contains(original_entry.name.as_str())
          && !original_set.contains(current_entry.name.as_str())
        {
          operations.push(FileOperation::Rename {
            from:   original_entry.name.clone(),
            to:     current_entry.name.clone(),
            is_dir: original_entry.is_dir,
          });
          processed.insert(original_entry.name.clone());
          processed.insert(current_entry.name.clone());
        }
      }
    }
  }

  for entry in original_state {
    if !current_set.contains(entry.name.as_str()) && !processed.contains(&entry.name) {
      operations.push(FileOperation::Delete {
        name:   entry.name.clone(),
        is_dir: entry.is_dir,
      });
      processed.insert(entry.name.clone());
    }
  }

  for entry in current_state {
    if !original_set.contains(entry.name.as_str()) && !processed.contains(&entry.name) {
      operations.push(FileOperation::Create {
        name:   entry.name.clone(),
        is_dir: entry.is_dir,
      });
      processed.insert(entry.name.clone());
    }
  }

  operations
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn detects_creation() {
    let original = vec![
      FileManagerEntryState::new("file1.txt", false),
      FileManagerEntryState::new("file2.txt", false),
    ];
    let current = vec![
      FileManagerEntryState::new("file1.txt", false),
      FileManagerEntryState::new("file2.txt", false),
      FileManagerEntryState::new("file3.txt", false),
    ];

    let ops = diff_states(&original, &current);

    assert_eq!(ops.len(), 1);
    assert!(matches!(
      ops[0],
      FileOperation::Create { ref name, is_dir: false } if name == "file3.txt"
    ));
  }

  #[test]
  fn detects_deletion() {
    let original = vec![
      FileManagerEntryState::new("file1.txt", false),
      FileManagerEntryState::new("file2.txt", false),
    ];
    let current = vec![FileManagerEntryState::new("file1.txt", false)];

    let ops = diff_states(&original, &current);

    assert_eq!(ops.len(), 1);
    assert!(matches!(
      ops[0],
      FileOperation::Delete { ref name, is_dir: false } if name == "file2.txt"
    ));
  }

  #[test]
  fn detects_rename() {
    let original = vec![
      FileManagerEntryState::new("oldname.txt", false),
      FileManagerEntryState::new("file2.txt", false),
    ];
    let current = vec![
      FileManagerEntryState::new("newname.txt", false),
      FileManagerEntryState::new("file2.txt", false),
    ];

    let ops = diff_states(&original, &current);

    assert_eq!(ops.len(), 1);
    assert!(matches!(
      ops[0],
      FileOperation::Rename { ref from, ref to, is_dir: false }
        if from == "oldname.txt" && to == "newname.txt"
    ));
  }
}
