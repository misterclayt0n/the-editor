use std::{
  path::PathBuf,
  sync::Arc,
};

use anyhow::{
  Context,
  Result,
};
use arc_swap::{
  ArcSwap,
  access::DynAccess,
};
use ropey::Rope;

use crate::{
  core::{
    document::{
      Document,
      FileManagerEntryState,
      SpecialBufferMetadata,
    },
    syntax,
  },
  editor::EditorConfig,
  file_manager::format,
};

/// Create a new file manager buffer for the given directory
pub fn create_file_manager_buffer(
  path: PathBuf,
  show_hidden: bool,
  config: Arc<dyn DynAccess<EditorConfig>>,
  syn_loader: Arc<ArcSwap<syntax::Loader>>,
) -> Result<Document> {
  // Canonicalize the path
  let path = path.canonicalize().context("Failed to canonicalize path")?;

  if !path.is_dir() {
    anyhow::bail!("Path is not a directory: {}", path.display());
  }

  // Generate buffer content
  let content = format::generate_buffer_content(&path, show_hidden)?;
  let original_state = format::parse_buffer_content(&content);

  // Create document from content
  let rope = Rope::from_str(&content);
  let mut doc = Document::from(rope, None, config, syn_loader);

  // Set special buffer metadata
  let mut metadata = SpecialBufferMetadata::new_with_file_manager(path, show_hidden);
  if let Some(ref mut fm) = metadata.file_manager {
    fm.original_state = original_state;
  }
  doc.set_special_buffer_kind(Some(metadata.kind()));
  doc.set_special_buffer_metadata(metadata);

  // Buffer is editable for oil.nvim style editing
  // readonly field defaults to false, so no need to set it

  Ok(doc)
}

/// Refresh the buffer content for the current directory
pub fn refresh_buffer(doc: &mut Document, view_id: crate::core::ViewId) -> Result<()> {
  let metadata = doc
    .special_buffer_metadata()
    .and_then(|m| m.file_manager.as_ref())
    .context("Not a file manager buffer")?;

  let path = metadata.current_path.clone();
  let show_hidden = metadata.show_hidden;

  // Generate new content
  let content = format::generate_buffer_content(&path, show_hidden)?;
  let original_state = format::parse_buffer_content(&content);

  // Update document text
  let new_rope = Rope::from_str(&content);
  doc.replace_all_text(new_rope, view_id);

  // Update metadata
  if let Some(meta) = doc.special_buffer_metadata_mut() {
    if let Some(ref mut fm) = meta.file_manager {
      fm.original_state = original_state;
    }
  }

  Ok(())
}

/// Navigate to a different directory
pub fn navigate_to(doc: &mut Document, path: PathBuf, view_id: crate::core::ViewId) -> Result<()> {
  let metadata = doc
    .special_buffer_metadata_mut()
    .and_then(|m| m.file_manager.as_mut())
    .context("Not a file manager buffer")?;

  // Canonicalize the new path
  let path = path.canonicalize().context("Failed to canonicalize path")?;

  if !path.is_dir() {
    anyhow::bail!("Path is not a directory: {}", path.display());
  }

  // Update current path
  metadata.current_path = path;

  // Refresh buffer with new path
  refresh_buffer(doc, view_id)
}

/// Toggle hidden files visibility
pub fn toggle_hidden_files(doc: &mut Document, view_id: crate::core::ViewId) -> Result<()> {
  let metadata = doc
    .special_buffer_metadata_mut()
    .and_then(|m| m.file_manager.as_mut())
    .context("Not a file manager buffer")?;

  // Toggle show_hidden
  metadata.show_hidden = !metadata.show_hidden;

  // Refresh buffer
  refresh_buffer(doc, view_id)
}

/// Get the current directory path from a file manager buffer
pub fn current_path(doc: &Document) -> Option<PathBuf> {
  doc
    .special_buffer_metadata()
    .and_then(|m| m.file_manager.as_ref())
    .map(|fm| fm.current_path.clone())
}

/// Get the original state (list of filenames) from when the buffer was last refreshed
pub fn original_state(doc: &Document) -> Option<Vec<FileManagerEntryState>> {
  doc
    .special_buffer_metadata()
    .and_then(|m| m.file_manager.as_ref())
    .map(|fm| fm.original_state.clone())
}

/// Initialize an existing document as a file manager buffer and navigate to a path
pub fn refresh_to_path(
  doc: &mut Document,
  path: PathBuf,
  show_hidden: bool,
  view_id: crate::core::ViewId,
) -> Result<()> {
  // Canonicalize the path
  let path = path.canonicalize().context("Failed to canonicalize path")?;

  if !path.is_dir() {
    anyhow::bail!("Path is not a directory: {}", path.display());
  }

  // Generate buffer content
  let content = format::generate_buffer_content(&path, show_hidden)?;
  let original_state = format::parse_buffer_content(&content);

  // Create metadata
  let mut metadata = SpecialBufferMetadata::new_with_file_manager(path, show_hidden);
  if let Some(ref mut fm) = metadata.file_manager {
    fm.original_state = original_state;
  }

  // Set metadata
  doc.set_special_buffer_metadata(metadata);

  // Update document text
  let new_rope = Rope::from_str(&content);
  doc.replace_all_text(new_rope, view_id);

  Ok(())
}
