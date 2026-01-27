//! FFI bindings for the-editor, exposing core functionality to Swift via
//! swift-bridge.
//!
//! This crate provides a C-compatible interface to the-lib, allowing the
//! SwiftUI client to interact with the Rust editor core.

use std::{
  num::NonZeroUsize,
  sync::atomic::{
    AtomicUsize,
    Ordering,
  },
};

use ropey::Rope;
use the_lib::{
  document::{
    Document as LibDocument,
    DocumentId,
  },
  movement::{
    self,
    Direction,
    Movement,
  },
  render::{
    text_annotations::TextAnnotations,
    text_format::TextFormat,
  },
  selection::Selection,
  transaction::Transaction,
};

/// Global document ID counter for FFI layer.
static NEXT_DOC_ID: AtomicUsize = AtomicUsize::new(1);

fn next_doc_id() -> DocumentId {
  let id = NEXT_DOC_ID.fetch_add(1, Ordering::Relaxed).max(1);
  DocumentId::new(NonZeroUsize::new(id).expect("document id overflow"))
}

/// FFI-safe document wrapper.
///
/// This wraps the core `Document` type and provides simplified methods
/// suitable for FFI export.
pub struct Document {
  inner: LibDocument,
}

impl Document {
  /// Create a new empty document.
  pub fn new() -> Self {
    Self {
      inner: LibDocument::new(next_doc_id(), Rope::new()),
    }
  }

  /// Create a document from text content.
  pub fn from_text(text: &str) -> Self {
    Self {
      inner: LibDocument::new(next_doc_id(), Rope::from_str(text)),
    }
  }

  /// Get the full text content as a string.
  pub fn text(&self) -> String {
    self.inner.text().to_string()
  }

  /// Get the number of characters in the document.
  pub fn len_chars(&self) -> usize {
    self.inner.text().len_chars()
  }

  /// Get the number of lines in the document.
  pub fn len_lines(&self) -> usize {
    self.inner.text().len_lines()
  }

  /// Check if the document is empty.
  pub fn is_empty(&self) -> bool {
    self.inner.text().len_chars() == 0
  }

  /// Get the document version (increments on each change).
  pub fn version(&self) -> u64 {
    self.inner.version()
  }

  /// Check if the document has been modified since last commit.
  pub fn is_modified(&self) -> bool {
    self.inner.flags().modified
  }

  // --- Selection queries ---

  /// Get the primary cursor position (character index).
  pub fn primary_cursor(&self) -> usize {
    let slice = self.inner.text().slice(..);
    self.inner.selection().ranges()[0].cursor(slice)
  }

  /// Get the number of cursors (ranges) in the selection.
  pub fn cursor_count(&self) -> usize {
    self.inner.selection().len()
  }

  /// Get cursor position at the given index.
  /// Returns None if index is out of bounds.
  pub fn cursor_at(&self, index: usize) -> Option<usize> {
    let ranges = self.inner.selection().ranges();
    if index < ranges.len() {
      let slice = self.inner.text().slice(..);
      Some(ranges[index].cursor(slice))
    } else {
      None
    }
  }

  /// Get all cursor positions as a vector.
  pub fn all_cursors(&self) -> Vec<usize> {
    let slice = self.inner.text().slice(..);
    self
      .inner
      .selection()
      .ranges()
      .iter()
      .map(|r| r.cursor(slice))
      .collect()
  }

  // --- Text editing ---

  /// Insert text at all cursor positions.
  pub fn insert(&mut self, text: &str) -> bool {
    let rope = self.inner.text();
    let changes: Vec<_> = self
      .inner
      .selection()
      .iter()
      .map(|range| {
        let pos = range.cursor(rope.slice(..));
        (pos, pos, Some(text.into()))
      })
      .collect();

    if let Ok(tx) = Transaction::change(rope, changes) {
      self.inner.apply_transaction(&tx).is_ok()
    } else {
      false
    }
  }

  /// Delete one character before each cursor (backspace).
  pub fn delete_backward(&mut self) -> bool {
    let rope = self.inner.text();
    let changes: Vec<_> = self
      .inner
      .selection()
      .iter()
      .filter_map(|range| {
        let pos = range.cursor(rope.slice(..));
        if pos > 0 {
          Some((pos - 1, pos, None))
        } else {
          None
        }
      })
      .collect();

    if changes.is_empty() {
      return false;
    }

    if let Ok(tx) = Transaction::change(rope, changes) {
      self.inner.apply_transaction(&tx).is_ok()
    } else {
      false
    }
  }

  /// Delete one character after each cursor (delete key).
  pub fn delete_forward(&mut self) -> bool {
    let rope = self.inner.text();
    let len = rope.len_chars();
    let changes: Vec<_> = self
      .inner
      .selection()
      .iter()
      .filter_map(|range| {
        let pos = range.cursor(rope.slice(..));
        if pos < len {
          Some((pos, pos + 1, None))
        } else {
          None
        }
      })
      .collect();

    if changes.is_empty() {
      return false;
    }

    if let Ok(tx) = Transaction::change(rope, changes) {
      self.inner.apply_transaction(&tx).is_ok()
    } else {
      false
    }
  }

  // --- Cursor movement ---

  /// Move all cursors left by one character.
  pub fn move_left(&mut self) {
    self.move_horizontal(Direction::Backward);
  }

  /// Move all cursors right by one character.
  pub fn move_right(&mut self) {
    self.move_horizontal(Direction::Forward);
  }

  /// Move all cursors up by one line.
  pub fn move_up(&mut self) {
    self.move_vertical(Direction::Backward);
  }

  /// Move all cursors down by one line.
  pub fn move_down(&mut self) {
    self.move_vertical(Direction::Forward);
  }

  fn move_horizontal(&mut self, dir: Direction) {
    let slice = self.inner.text().slice(..);
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();

    let selection = self.inner.selection().clone().transform(|range| {
      movement::move_horizontally(
        slice,
        range,
        dir,
        1,
        Movement::Move,
        &text_fmt,
        &mut annotations,
      )
    });

    drop(annotations);
    let _ = self.inner.set_selection(selection);
  }

  fn move_vertical(&mut self, dir: Direction) {
    let slice = self.inner.text().slice(..);
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();

    let selection = self.inner.selection().clone().transform(|range| {
      movement::move_vertically(
        slice,
        range,
        dir,
        1,
        Movement::Move,
        &text_fmt,
        &mut annotations,
      )
    });

    drop(annotations);
    let _ = self.inner.set_selection(selection);
  }

  // --- Multi-cursor ---

  /// Add a cursor on the line above the primary cursor.
  pub fn add_cursor_above(&mut self) -> bool {
    self.add_cursor_vertical(Direction::Backward)
  }

  /// Add a cursor on the line below the primary cursor.
  pub fn add_cursor_below(&mut self) -> bool {
    self.add_cursor_vertical(Direction::Forward)
  }

  fn add_cursor_vertical(&mut self, dir: Direction) -> bool {
    let slice = self.inner.text().slice(..);
    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();

    let mut ranges: Vec<_> = self.inner.selection().iter().cloned().collect();
    let primary = ranges[0];

    let new_range = movement::move_vertically(
      slice,
      primary,
      dir,
      1,
      Movement::Move,
      &text_fmt,
      &mut annotations,
    );

    drop(annotations);

    // Only add if position is different
    if new_range.cursor(slice) != primary.cursor(slice) {
      ranges.push(new_range);
      if let Ok(selection) = Selection::new(ranges.into()) {
        let _ = self.inner.set_selection(selection);
        return true;
      }
    }

    false
  }

  /// Remove all cursors except the primary.
  pub fn collapse_to_primary(&mut self) {
    let primary = self.inner.selection().ranges()[0];
    let _ = self
      .inner
      .set_selection(Selection::single(primary.anchor, primary.head));
  }

  // --- History ---

  /// Commit current changes to history.
  pub fn commit(&mut self) -> bool {
    self.inner.commit().is_ok()
  }

  /// Undo the last committed change.
  pub fn undo(&mut self) -> bool {
    self.inner.undo().unwrap_or(false)
  }

  /// Redo the last undone change.
  pub fn redo(&mut self) -> bool {
    self.inner.redo().unwrap_or(false)
  }

  // --- Line access (for rendering) ---

  /// Get a specific line's content.
  /// Returns None if line index is out of bounds.
  pub fn line(&self, line_idx: usize) -> Option<String> {
    let rope = self.inner.text();
    if line_idx < rope.len_lines() {
      Some(rope.line(line_idx).to_string())
    } else {
      None
    }
  }

  /// Get the line number for a character position.
  pub fn char_to_line(&self, char_idx: usize) -> usize {
    self.inner.text().char_to_line(char_idx)
  }

  /// Get the character position at the start of a line.
  pub fn line_to_char(&self, line_idx: usize) -> usize {
    self.inner.text().line_to_char(line_idx)
  }
}

impl Default for Document {
  fn default() -> Self {
    Self::new()
  }
}

// Swift bridge module
#[swift_bridge::bridge]
mod ffi {
  extern "Rust" {
    type Document;

    // Constructors
    #[swift_bridge(init)]
    fn new() -> Document;

    #[swift_bridge(associated_to = Document)]
    fn from_text(text: &str) -> Document;

    // Content access
    fn text(&self) -> String;
    fn len_chars(&self) -> usize;
    fn len_lines(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn version(&self) -> u64;
    fn is_modified(&self) -> bool;

    // Selection queries
    fn primary_cursor(&self) -> usize;
    fn cursor_count(&self) -> usize;
    fn all_cursors(&self) -> Vec<usize>;

    // Text editing
    fn insert(&mut self, text: &str) -> bool;
    fn delete_backward(&mut self) -> bool;
    fn delete_forward(&mut self) -> bool;

    // Cursor movement
    fn move_left(&mut self);
    fn move_right(&mut self);
    fn move_up(&mut self);
    fn move_down(&mut self);

    // Multi-cursor
    fn add_cursor_above(&mut self) -> bool;
    fn add_cursor_below(&mut self) -> bool;
    fn collapse_to_primary(&mut self);

    // History
    fn commit(&mut self) -> bool;
    fn undo(&mut self) -> bool;
    fn redo(&mut self) -> bool;

    // Line access
    fn char_to_line(&self, char_idx: usize) -> usize;
    fn line_to_char(&self, line_idx: usize) -> usize;
  }
}
