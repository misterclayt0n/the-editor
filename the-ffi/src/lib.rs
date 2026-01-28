//! FFI bindings for the-editor, exposing core functionality to Swift via
//! swift-bridge.
//!
//! This crate provides a C-compatible interface to the-lib, allowing the
//! SwiftUI client to interact with the Rust editor core.

use std::{
  num::{
    NonZeroU64,
    NonZeroUsize,
  },
  sync::atomic::{
    AtomicUsize,
    Ordering,
  },
};

use ropey::Rope;
use the_lib::{
  Tendril,
  app::App as LibApp,
  document::{
    Document as LibDocument,
    DocumentId,
  },
  editor::EditorId as LibEditorId,
  movement::{
    self,
    Direction,
    Movement,
  },
  position::Position as LibPosition,
  render::{
    NoHighlights,
    RenderStyles,
    build_plan,
    graphics::{
      Color as LibColor,
      CursorKind as LibCursorKind,
      Rect as LibRect,
      Style as LibStyle,
      UnderlineStyle as LibUnderlineStyle,
    },
    text_annotations::TextAnnotations,
    text_format::TextFormat,
  },
  selection::{
    CursorId,
    CursorPick,
  },
  transaction::Transaction,
  view::ViewState,
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
    insert_text(&mut self.inner, text)
  }

  /// Delete one character before each cursor (backspace).
  pub fn delete_backward(&mut self) -> bool {
    delete_backward(&mut self.inner)
  }

  /// Delete one character after each cursor (delete key).
  pub fn delete_forward(&mut self) -> bool {
    delete_forward(&mut self.inner)
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
    let text_fmt = TextFormat::default();
    move_horizontal(&mut self.inner, dir, &text_fmt);
  }

  fn move_vertical(&mut self, dir: Direction) {
    let text_fmt = TextFormat::default();
    move_vertical(&mut self.inner, dir, &text_fmt);
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
    let text_fmt = TextFormat::default();
    add_cursor_vertical(&mut self.inner, dir, CursorPick::First, &text_fmt)
  }

  /// Remove all cursors except the primary.
  pub fn collapse_to_primary(&mut self) {
    let _ = collapse_selection(&mut self.inner, CursorPick::First);
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

#[derive(Debug, Clone)]
pub struct RenderSpan {
  inner: the_lib::render::RenderSpan,
}

impl Default for RenderSpan {
  fn default() -> Self {
    Self {
      inner: the_lib::render::RenderSpan {
        col:        0,
        cols:       0,
        text:       Tendril::new(),
        highlight:  None,
        is_virtual: false,
      },
    }
  }
}

impl RenderSpan {
  fn col(&self) -> u16 {
    self.inner.col
  }

  fn cols(&self) -> u16 {
    self.inner.cols
  }

  fn text(&self) -> String {
    self.inner.text.to_string()
  }

  fn has_highlight(&self) -> bool {
    self.inner.highlight.is_some()
  }

  fn highlight(&self) -> u32 {
    self
      .inner
      .highlight
      .map(|highlight| highlight.get())
      .unwrap_or(0)
  }

  fn is_virtual(&self) -> bool {
    self.inner.is_virtual
  }
}

impl From<the_lib::render::RenderSpan> for RenderSpan {
  fn from(span: the_lib::render::RenderSpan) -> Self {
    Self { inner: span }
  }
}

#[derive(Debug, Clone)]
pub struct RenderLine {
  inner: the_lib::render::RenderLine,
}

impl Default for RenderLine {
  fn default() -> Self {
    Self {
      inner: the_lib::render::RenderLine {
        row:   0,
        spans: Vec::new(),
      },
    }
  }
}

impl RenderLine {
  fn row(&self) -> u16 {
    self.inner.row
  }

  fn span_count(&self) -> usize {
    self.inner.spans.len()
  }

  fn span_at(&self, index: usize) -> RenderSpan {
    self
      .inner
      .spans
      .get(index)
      .cloned()
      .map(RenderSpan::from)
      .unwrap_or_default()
  }
}

impl From<the_lib::render::RenderLine> for RenderLine {
  fn from(line: the_lib::render::RenderLine) -> Self {
    Self { inner: line }
  }
}

#[derive(Debug, Clone)]
pub struct RenderCursor {
  inner: the_lib::render::RenderCursor,
}

impl Default for RenderCursor {
  fn default() -> Self {
    Self {
      inner: the_lib::render::RenderCursor {
        id:    CursorId::new(NonZeroU64::new(1).expect("cursor id must be non-zero")),
        pos:   LibPosition::new(0, 0),
        kind:  LibCursorKind::Hidden,
        style: LibStyle::default(),
      },
    }
  }
}

impl RenderCursor {
  fn id(&self) -> u64 {
    self.inner.id.get()
  }

  fn pos(&self) -> ffi::Position {
    self.inner.pos.into()
  }

  fn kind(&self) -> u8 {
    cursor_kind_to_u8(self.inner.kind)
  }

  fn style(&self) -> ffi::Style {
    self.inner.style.into()
  }
}

impl From<the_lib::render::RenderCursor> for RenderCursor {
  fn from(cursor: the_lib::render::RenderCursor) -> Self {
    Self { inner: cursor }
  }
}

#[derive(Debug, Clone)]
pub struct RenderSelection {
  inner: the_lib::render::RenderSelection,
}

impl Default for RenderSelection {
  fn default() -> Self {
    Self {
      inner: the_lib::render::RenderSelection {
        rect:  LibRect::new(0, 0, 0, 0),
        style: LibStyle::default(),
      },
    }
  }
}

impl RenderSelection {
  fn rect(&self) -> ffi::Rect {
    self.inner.rect.into()
  }

  fn style(&self) -> ffi::Style {
    self.inner.style.into()
  }
}

impl From<the_lib::render::RenderSelection> for RenderSelection {
  fn from(selection: the_lib::render::RenderSelection) -> Self {
    Self { inner: selection }
  }
}

#[derive(Debug, Clone)]
pub struct RenderPlan {
  inner: the_lib::render::RenderPlan,
}

impl RenderPlan {
  fn empty() -> Self {
    Self {
      inner: the_lib::render::RenderPlan::empty(LibRect::new(0, 0, 0, 0), LibPosition::new(0, 0)),
    }
  }

  fn viewport(&self) -> ffi::Rect {
    self.inner.viewport.into()
  }

  fn scroll(&self) -> ffi::Position {
    self.inner.scroll.into()
  }

  fn line_count(&self) -> usize {
    self.inner.lines.len()
  }

  fn line_at(&self, index: usize) -> RenderLine {
    self
      .inner
      .lines
      .get(index)
      .cloned()
      .map(RenderLine::from)
      .unwrap_or_default()
  }

  fn cursor_count(&self) -> usize {
    self.inner.cursors.len()
  }

  fn cursor_at(&self, index: usize) -> RenderCursor {
    self
      .inner
      .cursors
      .get(index)
      .cloned()
      .map(RenderCursor::from)
      .unwrap_or_default()
  }

  fn selection_count(&self) -> usize {
    self.inner.selections.len()
  }

  fn selection_at(&self, index: usize) -> RenderSelection {
    self
      .inner
      .selections
      .get(index)
      .cloned()
      .map(RenderSelection::from)
      .unwrap_or_default()
  }
}

impl From<the_lib::render::RenderPlan> for RenderPlan {
  fn from(plan: the_lib::render::RenderPlan) -> Self {
    Self { inner: plan }
  }
}

impl Default for Document {
  fn default() -> Self {
    Self::new()
  }
}

/// FFI-safe app wrapper with editor management.
pub struct App {
  inner: LibApp,
}

impl App {
  pub fn new() -> Self {
    Self {
      inner: LibApp::default(),
    }
  }

  pub fn create_editor(
    &mut self,
    text: &str,
    viewport: ffi::Rect,
    scroll: ffi::Position,
  ) -> ffi::EditorId {
    let view = ViewState::new(viewport.to_lib(), scroll.to_lib());
    let id = self.inner.create_editor(Rope::from_str(text), view);
    ffi::EditorId::from(id)
  }

  pub fn remove_editor(&mut self, id: ffi::EditorId) -> bool {
    let Some(id) = id.to_lib() else {
      return false;
    };
    self.inner.remove_editor(id).is_some()
  }

  pub fn set_viewport(&mut self, id: ffi::EditorId, viewport: ffi::Rect) -> bool {
    let Some(editor) = self.editor_mut(id) else {
      return false;
    };
    editor.view_mut().viewport = viewport.to_lib();
    true
  }

  pub fn set_scroll(&mut self, id: ffi::EditorId, scroll: ffi::Position) -> bool {
    let Some(editor) = self.editor_mut(id) else {
      return false;
    };
    editor.view_mut().scroll = scroll.to_lib();
    true
  }

  pub fn set_active_cursor(&mut self, id: ffi::EditorId, cursor_id: u64) -> bool {
    let Some(editor) = self.editor_mut(id) else {
      return false;
    };
    let Some(cursor_id) = NonZeroU64::new(cursor_id).map(CursorId::new) else {
      return false;
    };
    let has_cursor = editor
      .document()
      .selection()
      .cursor_ids()
      .iter()
      .any(|id| id.get() == cursor_id.get());
    if !has_cursor {
      return false;
    }
    editor.view_mut().active_cursor = Some(cursor_id);
    true
  }

  pub fn clear_active_cursor(&mut self, id: ffi::EditorId) -> bool {
    let Some(editor) = self.editor_mut(id) else {
      return false;
    };
    editor.view_mut().active_cursor = None;
    true
  }

  pub fn cursor_ids(&self, id: ffi::EditorId) -> Vec<u64> {
    let Some(editor) = self.editor(id) else {
      return Vec::new();
    };
    editor
      .document()
      .selection()
      .cursor_ids()
      .iter()
      .map(|id| id.get())
      .collect()
  }

  pub fn render_plan(&mut self, id: ffi::EditorId) -> RenderPlan {
    self.render_plan_with_styles(id, ffi::RenderStyles::default())
  }

  pub fn render_plan_with_styles(
    &mut self,
    id: ffi::EditorId,
    styles: ffi::RenderStyles,
  ) -> RenderPlan {
    let Some(editor) = self.editor_mut(id) else {
      return RenderPlan::empty();
    };

    let view = editor.view();
    let text_fmt = text_format_for_view(view);
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let styles = styles.to_lib();

    let (doc, cache) = editor.document_and_cache();
    let plan = build_plan(
      doc,
      view,
      &text_fmt,
      &mut annotations,
      &mut highlights,
      cache,
      styles,
    );

    plan.into()
  }

  pub fn text(&self, id: ffi::EditorId) -> String {
    self
      .editor(id)
      .map(|editor| editor.document().text().to_string())
      .unwrap_or_default()
  }

  pub fn insert(&mut self, id: ffi::EditorId, text: &str) -> bool {
    self
      .editor_mut(id)
      .map(|editor| insert_text(editor.document_mut(), text))
      .unwrap_or(false)
  }

  pub fn delete_backward(&mut self, id: ffi::EditorId) -> bool {
    self
      .editor_mut(id)
      .map(|editor| delete_backward(editor.document_mut()))
      .unwrap_or(false)
  }

  pub fn delete_forward(&mut self, id: ffi::EditorId) -> bool {
    self
      .editor_mut(id)
      .map(|editor| delete_forward(editor.document_mut()))
      .unwrap_or(false)
  }

  pub fn move_left(&mut self, id: ffi::EditorId) {
    if let Some(editor) = self.editor_mut(id) {
      let text_fmt = text_format_for_view(editor.view());
      move_horizontal(editor.document_mut(), Direction::Backward, &text_fmt);
    }
  }

  pub fn move_right(&mut self, id: ffi::EditorId) {
    if let Some(editor) = self.editor_mut(id) {
      let text_fmt = text_format_for_view(editor.view());
      move_horizontal(editor.document_mut(), Direction::Forward, &text_fmt);
    }
  }

  pub fn move_up(&mut self, id: ffi::EditorId) {
    if let Some(editor) = self.editor_mut(id) {
      let text_fmt = text_format_for_view(editor.view());
      move_vertical(editor.document_mut(), Direction::Backward, &text_fmt);
    }
  }

  pub fn move_down(&mut self, id: ffi::EditorId) {
    if let Some(editor) = self.editor_mut(id) {
      let text_fmt = text_format_for_view(editor.view());
      move_vertical(editor.document_mut(), Direction::Forward, &text_fmt);
    }
  }

  pub fn add_cursor_above(&mut self, id: ffi::EditorId) -> bool {
    self.add_cursor_vertical(id, Direction::Backward)
  }

  pub fn add_cursor_below(&mut self, id: ffi::EditorId) -> bool {
    self.add_cursor_vertical(id, Direction::Forward)
  }

  pub fn collapse_to_cursor(&mut self, id: ffi::EditorId, cursor_id: u64) -> bool {
    let Some(editor) = self.editor_mut(id) else {
      return false;
    };
    let Some(cursor_id) = NonZeroU64::new(cursor_id).map(CursorId::new) else {
      return false;
    };
    if collapse_selection(editor.document_mut(), CursorPick::Id(cursor_id)) {
      editor.view_mut().active_cursor = Some(cursor_id);
      true
    } else {
      false
    }
  }

  pub fn collapse_to_first(&mut self, id: ffi::EditorId) -> bool {
    let Some(editor) = self.editor_mut(id) else {
      return false;
    };
    let pick = CursorPick::First;
    if collapse_selection(editor.document_mut(), pick) {
      if let Some(id) = editor.document().selection().cursor_ids().first().copied() {
        editor.view_mut().active_cursor = Some(id);
      }
      true
    } else {
      false
    }
  }

  fn add_cursor_vertical(&mut self, id: ffi::EditorId, dir: Direction) -> bool {
    let Some(editor) = self.editor_mut(id) else {
      return false;
    };
    let selection = editor.document().selection();
    let pick = editor
      .view()
      .active_cursor
      .map(CursorPick::Id)
      .filter(|pick| selection.pick(*pick).is_ok())
      .unwrap_or(CursorPick::First);
    let text_fmt = text_format_for_view(editor.view());
    add_cursor_vertical(editor.document_mut(), dir, pick, &text_fmt)
  }

  fn editor(&self, id: ffi::EditorId) -> Option<&the_lib::editor::Editor> {
    let id = id.to_lib()?;
    self.inner.editor(id)
  }

  fn editor_mut(&mut self, id: ffi::EditorId) -> Option<&mut the_lib::editor::Editor> {
    let id = id.to_lib()?;
    self.inner.editor_mut(id)
  }
}

impl Default for App {
  fn default() -> Self {
    Self::new()
  }
}

fn text_format_for_view(view: ViewState) -> TextFormat {
  let mut text_fmt = TextFormat::default();
  text_fmt.viewport_width = view.viewport.width;
  text_fmt
}

fn insert_text(doc: &mut LibDocument, text: &str) -> bool {
  let rope = doc.text();
  let changes: Vec<_> = doc
    .selection()
    .iter()
    .map(|range| {
      let pos = range.cursor(rope.slice(..));
      (pos, pos, Some(text.into()))
    })
    .collect();

  if let Ok(tx) = Transaction::change(rope, changes) {
    doc.apply_transaction(&tx).is_ok()
  } else {
    false
  }
}

fn delete_backward(doc: &mut LibDocument) -> bool {
  let rope = doc.text();
  let changes: Vec<_> = doc
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
    doc.apply_transaction(&tx).is_ok()
  } else {
    false
  }
}

fn delete_forward(doc: &mut LibDocument) -> bool {
  let rope = doc.text();
  let len = rope.len_chars();
  let changes: Vec<_> = doc
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
    doc.apply_transaction(&tx).is_ok()
  } else {
    false
  }
}

fn move_horizontal(doc: &mut LibDocument, dir: Direction, text_fmt: &TextFormat) {
  let selection = {
    let slice = doc.text().slice(..);
    let mut annotations = TextAnnotations::default();
    doc.selection().clone().transform(|range| {
      movement::move_horizontally(
        slice,
        range,
        dir,
        1,
        Movement::Move,
        text_fmt,
        &mut annotations,
      )
    })
  };

  let _ = doc.set_selection(selection);
}

fn move_vertical(doc: &mut LibDocument, dir: Direction, text_fmt: &TextFormat) {
  let selection = {
    let slice = doc.text().slice(..);
    let mut annotations = TextAnnotations::default();
    doc.selection().clone().transform(|range| {
      movement::move_vertically(
        slice,
        range,
        dir,
        1,
        Movement::Move,
        text_fmt,
        &mut annotations,
      )
    })
  };

  let _ = doc.set_selection(selection);
}

fn add_cursor_vertical(
  doc: &mut LibDocument,
  dir: Direction,
  pick: CursorPick,
  text_fmt: &TextFormat,
) -> bool {
  let (base_cursor, new_cursor, new_range) = {
    let slice = doc.text().slice(..);
    let mut annotations = TextAnnotations::default();

    let Ok((_, base_range)) = doc.selection().pick(pick) else {
      return false;
    };

    let new_range = movement::move_vertically(
      slice,
      base_range,
      dir,
      1,
      Movement::Move,
      text_fmt,
      &mut annotations,
    );

    let base_cursor = base_range.cursor(slice);
    let new_cursor = new_range.cursor(slice);
    (base_cursor, new_cursor, new_range)
  };

  if new_cursor == base_cursor {
    return false;
  }

  let selection = doc.selection().clone().push(new_range);
  doc.set_selection(selection).is_ok()
}

fn collapse_selection(doc: &mut LibDocument, pick: CursorPick) -> bool {
  match doc.selection().clone().collapse(pick) {
    Ok(selection) => doc.set_selection(selection).is_ok(),
    Err(_) => false,
  }
}

// Swift bridge module
#[swift_bridge::bridge]
mod ffi {
  extern "Rust" {
    type Document;
    type App;

    // App lifecycle
    #[swift_bridge(init)]
    fn new() -> App;
    fn create_editor(self: &mut App, text: &str, viewport: Rect, scroll: Position) -> EditorId;
    fn remove_editor(self: &mut App, id: EditorId) -> bool;
    fn set_viewport(self: &mut App, id: EditorId, viewport: Rect) -> bool;
    fn set_scroll(self: &mut App, id: EditorId, scroll: Position) -> bool;
    fn set_active_cursor(self: &mut App, id: EditorId, cursor_id: u64) -> bool;
    fn clear_active_cursor(self: &mut App, id: EditorId) -> bool;
    fn cursor_ids(self: &App, id: EditorId) -> Vec<u64>;
    fn render_plan(self: &mut App, id: EditorId) -> RenderPlan;
    fn render_plan_with_styles(self: &mut App, id: EditorId, styles: RenderStyles) -> RenderPlan;
    fn text(self: &App, id: EditorId) -> String;

    // Editor editing
    fn insert(self: &mut App, id: EditorId, text: &str) -> bool;
    fn delete_backward(self: &mut App, id: EditorId) -> bool;
    fn delete_forward(self: &mut App, id: EditorId) -> bool;
    fn move_left(self: &mut App, id: EditorId);
    fn move_right(self: &mut App, id: EditorId);
    fn move_up(self: &mut App, id: EditorId);
    fn move_down(self: &mut App, id: EditorId);
    fn add_cursor_above(self: &mut App, id: EditorId) -> bool;
    fn add_cursor_below(self: &mut App, id: EditorId) -> bool;
    fn collapse_to_cursor(self: &mut App, id: EditorId, cursor_id: u64) -> bool;
    fn collapse_to_first(self: &mut App, id: EditorId) -> bool;

    // Constructors
    #[swift_bridge(init)]
    fn new() -> Document;

    #[swift_bridge(associated_to = Document)]
    fn from_text(text: &str) -> Document;

    // Content access
    fn text(self: &Document) -> String;
    fn len_chars(self: &Document) -> usize;
    fn len_lines(self: &Document) -> usize;
    fn is_empty(self: &Document) -> bool;
    fn version(self: &Document) -> u64;
    fn is_modified(self: &Document) -> bool;

    // Selection queries
    fn primary_cursor(self: &Document) -> usize;
    fn cursor_count(self: &Document) -> usize;
    fn all_cursors(self: &Document) -> Vec<usize>;

    // Text editing
    fn insert(self: &mut Document, text: &str) -> bool;
    fn delete_backward(self: &mut Document) -> bool;
    fn delete_forward(self: &mut Document) -> bool;

    // Cursor movement
    fn move_left(self: &mut Document);
    fn move_right(self: &mut Document);
    fn move_up(self: &mut Document);
    fn move_down(self: &mut Document);

    // Multi-cursor
    fn add_cursor_above(self: &mut Document) -> bool;
    fn add_cursor_below(self: &mut Document) -> bool;
    fn collapse_to_primary(self: &mut Document);

    // History
    fn commit(self: &mut Document) -> bool;
    fn undo(self: &mut Document) -> bool;
    fn redo(self: &mut Document) -> bool;

    // Line access
    fn char_to_line(self: &Document, char_idx: usize) -> usize;
    fn line_to_char(self: &Document, line_idx: usize) -> usize;
  }

  #[swift_bridge(swift_repr = "struct")]
  struct EditorId {
    value: u64,
  }

  #[swift_bridge(swift_repr = "struct")]
  struct Rect {
    x:      u16,
    y:      u16,
    width:  u16,
    height: u16,
  }

  #[swift_bridge(swift_repr = "struct")]
  struct Position {
    row: u64,
    col: u64,
  }

  #[swift_bridge(swift_repr = "struct")]
  struct Color {
    kind:  u8,
    value: u32,
  }

  #[swift_bridge(swift_repr = "struct")]
  struct Style {
    has_fg:              bool,
    fg:                  Color,
    has_bg:              bool,
    bg:                  Color,
    has_underline_color: bool,
    underline_color:     Color,
    underline_style:     u8,
    add_modifier:        u16,
    sub_modifier:        u16,
  }

  #[swift_bridge(swift_repr = "struct")]
  struct RenderStyles {
    selection:     Style,
    cursor:        Style,
    active_cursor: Style,
  }

  extern "Rust" {
    type RenderSpan;
    fn col(self: &RenderSpan) -> u16;
    fn cols(self: &RenderSpan) -> u16;
    fn text(self: &RenderSpan) -> String;
    fn has_highlight(self: &RenderSpan) -> bool;
    fn highlight(self: &RenderSpan) -> u32;
    fn is_virtual(self: &RenderSpan) -> bool;
  }

  extern "Rust" {
    type RenderLine;
    fn row(self: &RenderLine) -> u16;
    fn span_count(self: &RenderLine) -> usize;
    fn span_at(self: &RenderLine, index: usize) -> RenderSpan;
  }

  extern "Rust" {
    type RenderCursor;
    fn id(self: &RenderCursor) -> u64;
    fn pos(self: &RenderCursor) -> Position;
    fn kind(self: &RenderCursor) -> u8;
    fn style(self: &RenderCursor) -> Style;
  }

  extern "Rust" {
    type RenderSelection;
    fn rect(self: &RenderSelection) -> Rect;
    fn style(self: &RenderSelection) -> Style;
  }

  extern "Rust" {
    type RenderPlan;
    fn viewport(self: &RenderPlan) -> Rect;
    fn scroll(self: &RenderPlan) -> Position;
    fn line_count(self: &RenderPlan) -> usize;
    fn line_at(self: &RenderPlan, index: usize) -> RenderLine;
    fn cursor_count(self: &RenderPlan) -> usize;
    fn cursor_at(self: &RenderPlan, index: usize) -> RenderCursor;
    fn selection_count(self: &RenderPlan) -> usize;
    fn selection_at(self: &RenderPlan, index: usize) -> RenderSelection;
  }
}

impl ffi::EditorId {
  fn to_lib(self) -> Option<LibEditorId> {
    let value = usize::try_from(self.value).ok()?;
    NonZeroUsize::new(value).map(LibEditorId::new)
  }
}

impl Copy for ffi::EditorId {}

impl Clone for ffi::EditorId {
  fn clone(&self) -> Self {
    *self
  }
}

impl From<LibEditorId> for ffi::EditorId {
  fn from(id: LibEditorId) -> Self {
    Self {
      value: id.get().get() as u64,
    }
  }
}

impl ffi::Rect {
  fn to_lib(self) -> LibRect {
    LibRect::new(self.x, self.y, self.width, self.height)
  }
}

impl From<LibRect> for ffi::Rect {
  fn from(rect: LibRect) -> Self {
    Self {
      x:      rect.x,
      y:      rect.y,
      width:  rect.width,
      height: rect.height,
    }
  }
}

impl ffi::Position {
  fn to_lib(self) -> LibPosition {
    LibPosition::new(u64_to_usize(self.row), u64_to_usize(self.col))
  }
}

impl From<LibPosition> for ffi::Position {
  fn from(pos: LibPosition) -> Self {
    Self {
      row: pos.row as u64,
      col: pos.col as u64,
    }
  }
}

impl ffi::RenderStyles {
  fn to_lib(self) -> RenderStyles {
    RenderStyles {
      selection:     self.selection.to_lib(),
      cursor:        self.cursor.to_lib(),
      active_cursor: self.active_cursor.to_lib(),
    }
  }
}

impl Default for ffi::RenderStyles {
  fn default() -> Self {
    Self {
      selection:     ffi::Style::default(),
      cursor:        ffi::Style::default(),
      active_cursor: ffi::Style::default(),
    }
  }
}

impl From<LibStyle> for ffi::Style {
  fn from(style: LibStyle) -> Self {
    Self {
      has_fg:              style.fg.is_some(),
      fg:                  style.fg.map(ffi::Color::from).unwrap_or_default(),
      has_bg:              style.bg.is_some(),
      bg:                  style.bg.map(ffi::Color::from).unwrap_or_default(),
      has_underline_color: style.underline_color.is_some(),
      underline_color:     style
        .underline_color
        .map(ffi::Color::from)
        .unwrap_or_default(),
      underline_style:     style
        .underline_style
        .map(underline_style_to_u8)
        .unwrap_or(0),
      add_modifier:        style.add_modifier.bits(),
      sub_modifier:        style.sub_modifier.bits(),
    }
  }
}

impl ffi::Style {
  fn to_lib(self) -> LibStyle {
    let mut style = LibStyle::new();
    if self.has_fg {
      style.fg = Some(self.fg.to_lib());
    }
    if self.has_bg {
      style.bg = Some(self.bg.to_lib());
    }
    if self.has_underline_color {
      style.underline_color = Some(self.underline_color.to_lib());
    }
    style.underline_style = underline_style_from_u8(self.underline_style);
    style.add_modifier = the_lib::render::graphics::Modifier::from_bits_truncate(self.add_modifier);
    style.sub_modifier = the_lib::render::graphics::Modifier::from_bits_truncate(self.sub_modifier);
    style
  }
}

impl Default for ffi::Style {
  fn default() -> Self {
    Self {
      has_fg:              false,
      fg:                  ffi::Color::default(),
      has_bg:              false,
      bg:                  ffi::Color::default(),
      has_underline_color: false,
      underline_color:     ffi::Color::default(),
      underline_style:     0,
      add_modifier:        0,
      sub_modifier:        0,
    }
  }
}

impl From<LibColor> for ffi::Color {
  fn from(color: LibColor) -> Self {
    match color {
      LibColor::Reset => Self { kind: 0, value: 0 },
      LibColor::Black => Self { kind: 1, value: 0 },
      LibColor::Red => Self { kind: 1, value: 1 },
      LibColor::Green => Self { kind: 1, value: 2 },
      LibColor::Yellow => Self { kind: 1, value: 3 },
      LibColor::Blue => Self { kind: 1, value: 4 },
      LibColor::Magenta => Self { kind: 1, value: 5 },
      LibColor::Cyan => Self { kind: 1, value: 6 },
      LibColor::Gray => Self { kind: 1, value: 7 },
      LibColor::LightRed => Self { kind: 1, value: 8 },
      LibColor::LightGreen => Self { kind: 1, value: 9 },
      LibColor::LightYellow => {
        Self {
          kind:  1,
          value: 10,
        }
      },
      LibColor::LightBlue => {
        Self {
          kind:  1,
          value: 11,
        }
      },
      LibColor::LightMagenta => {
        Self {
          kind:  1,
          value: 12,
        }
      },
      LibColor::LightCyan => {
        Self {
          kind:  1,
          value: 13,
        }
      },
      LibColor::LightGray => {
        Self {
          kind:  1,
          value: 14,
        }
      },
      LibColor::White => {
        Self {
          kind:  1,
          value: 15,
        }
      },
      LibColor::Rgb(r, g, b) => {
        Self {
          kind:  2,
          value: ((r as u32) << 16) | ((g as u32) << 8) | b as u32,
        }
      },
      LibColor::Indexed(idx) => {
        Self {
          kind:  3,
          value: idx as u32,
        }
      },
    }
  }
}

impl ffi::Color {
  fn to_lib(self) -> LibColor {
    match self.kind {
      0 => LibColor::Reset,
      1 => named_color_from_index(self.value),
      2 => {
        let r = ((self.value >> 16) & 0xFF) as u8;
        let g = ((self.value >> 8) & 0xFF) as u8;
        let b = (self.value & 0xFF) as u8;
        LibColor::Rgb(r, g, b)
      },
      3 => LibColor::Indexed(self.value as u8),
      _ => LibColor::Reset,
    }
  }
}

impl Default for ffi::Color {
  fn default() -> Self {
    Self { kind: 0, value: 0 }
  }
}

fn u64_to_usize(value: u64) -> usize {
  usize::try_from(value).unwrap_or(usize::MAX)
}

fn named_color_from_index(value: u32) -> LibColor {
  match value {
    0 => LibColor::Black,
    1 => LibColor::Red,
    2 => LibColor::Green,
    3 => LibColor::Yellow,
    4 => LibColor::Blue,
    5 => LibColor::Magenta,
    6 => LibColor::Cyan,
    7 => LibColor::Gray,
    8 => LibColor::LightRed,
    9 => LibColor::LightGreen,
    10 => LibColor::LightYellow,
    11 => LibColor::LightBlue,
    12 => LibColor::LightMagenta,
    13 => LibColor::LightCyan,
    14 => LibColor::LightGray,
    15 => LibColor::White,
    _ => LibColor::Reset,
  }
}

fn underline_style_to_u8(style: LibUnderlineStyle) -> u8 {
  match style {
    LibUnderlineStyle::Reset => 1,
    LibUnderlineStyle::Line => 2,
    LibUnderlineStyle::Curl => 3,
    LibUnderlineStyle::Dotted => 4,
    LibUnderlineStyle::Dashed => 5,
    LibUnderlineStyle::DoubleLine => 6,
  }
}

fn underline_style_from_u8(value: u8) -> Option<LibUnderlineStyle> {
  match value {
    1 => Some(LibUnderlineStyle::Reset),
    2 => Some(LibUnderlineStyle::Line),
    3 => Some(LibUnderlineStyle::Curl),
    4 => Some(LibUnderlineStyle::Dotted),
    5 => Some(LibUnderlineStyle::Dashed),
    6 => Some(LibUnderlineStyle::DoubleLine),
    _ => None,
  }
}

fn cursor_kind_to_u8(kind: LibCursorKind) -> u8 {
  match kind {
    LibCursorKind::Block => 0,
    LibCursorKind::Bar => 1,
    LibCursorKind::Underline => 2,
    LibCursorKind::Hollow => 3,
    LibCursorKind::Hidden => 4,
  }
}

#[cfg(test)]
mod tests {
  use super::{
    App,
    ffi,
  };

  #[test]
  fn app_render_plan_basic() {
    let mut app = App::new();
    let viewport = ffi::Rect {
      x:      0,
      y:      0,
      width:  80,
      height: 24,
    };
    let scroll = ffi::Position { row: 0, col: 0 };
    let id = app.create_editor("hello", viewport, scroll);

    let plan = app.render_plan(id);
    assert_eq!(plan.line_count(), 1);
    let line = plan.line_at(0);
    assert_eq!(line.span_count(), 1);
    assert_eq!(line.span_at(0).text(), "hello");
  }

  #[test]
  fn app_insert_updates_text_and_plan() {
    let mut app = App::new();
    let viewport = ffi::Rect {
      x:      0,
      y:      0,
      width:  80,
      height: 24,
    };
    let scroll = ffi::Position { row: 0, col: 0 };
    let id = app.create_editor("hello", viewport, scroll);

    assert!(app.insert(id, "yo "));
    assert_eq!(app.text(id), "yo hello");

    let plan = app.render_plan(id);
    let line = plan.line_at(0);
    assert_eq!(line.span_at(0).text(), "yo hello");
  }
}
