//! Minimal editor/surface state for the-lib.
//!
//! This is intentionally small: it owns a set of document buffers plus
//! per-buffer view/render state. IO, UI, and dispatch logic live outside
//! of the-lib.

use std::{
  num::NonZeroUsize,
  path::{
    Path,
    PathBuf,
  },
};

use ropey::Rope;

use crate::{
  document::{
    Document,
    DocumentId,
  },
  render::plan::RenderCache,
  view::ViewState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EditorId(NonZeroUsize);

impl EditorId {
  pub const fn new(id: NonZeroUsize) -> Self {
    Self(id)
  }

  pub const fn get(self) -> NonZeroUsize {
    self.0
  }
}

impl From<NonZeroUsize> for EditorId {
  fn from(value: NonZeroUsize) -> Self {
    Self::new(value)
  }
}

#[derive(Debug)]
pub struct Editor {
  id:               EditorId,
  buffers:          Vec<BufferState>,
  active_buffer:    usize,
  next_document_id: NonZeroUsize,
  access_history:   Vec<usize>,
  modified_history: Vec<usize>,
}

#[derive(Debug)]
struct BufferState {
  document:     Document,
  view:         ViewState,
  render_cache: RenderCache,
  file_path:    Option<PathBuf>,
}

impl BufferState {
  fn new(document: Document, view: ViewState, file_path: Option<PathBuf>) -> Self {
    Self {
      document,
      view,
      render_cache: RenderCache::default(),
      file_path,
    }
  }
}

impl Editor {
  pub fn new(id: EditorId, document: Document, view: ViewState) -> Self {
    let next_doc = document.id().get().get().saturating_add(1);
    let next_document_id = NonZeroUsize::new(next_doc).unwrap_or(document.id().get());

    Self {
      id,
      buffers: vec![BufferState::new(document, view, None)],
      active_buffer: 0,
      next_document_id,
      access_history: Vec::new(),
      modified_history: Vec::new(),
    }
  }

  fn activate_buffer(&mut self, index: usize) -> bool {
    if index >= self.buffers.len() {
      return false;
    }
    if index != self.active_buffer {
      self.access_history.push(self.active_buffer);
      self.active_buffer = index;
    }
    true
  }

  fn touch_modified_history(&mut self, index: usize) {
    if let Some(pos) = self
      .modified_history
      .iter()
      .position(|entry| *entry == index)
    {
      self.modified_history.remove(pos);
    }
    self.modified_history.push(index);
  }

  pub fn id(&self) -> EditorId {
    self.id
  }

  pub fn document(&self) -> &Document {
    &self.buffers[self.active_buffer].document
  }

  pub fn document_mut(&mut self) -> &mut Document {
    &mut self.buffers[self.active_buffer].document
  }

  pub fn view(&self) -> ViewState {
    self.buffers[self.active_buffer].view
  }

  pub fn view_mut(&mut self) -> &mut ViewState {
    &mut self.buffers[self.active_buffer].view
  }

  pub fn render_cache(&self) -> &RenderCache {
    &self.buffers[self.active_buffer].render_cache
  }

  pub fn render_cache_mut(&mut self) -> &mut RenderCache {
    &mut self.buffers[self.active_buffer].render_cache
  }

  pub fn document_and_cache(&mut self) -> (&Document, &mut RenderCache) {
    let buffer = &mut self.buffers[self.active_buffer];
    (&buffer.document, &mut buffer.render_cache)
  }

  pub fn buffer_count(&self) -> usize {
    self.buffers.len()
  }

  pub fn active_buffer_index(&self) -> usize {
    self.active_buffer
  }

  pub fn set_active_buffer(&mut self, index: usize) -> bool {
    self.activate_buffer(index)
  }

  pub fn switch_buffer_forward(&mut self, count: usize) -> bool {
    let len = self.buffers.len();
    if len <= 1 {
      return false;
    }
    let step = count.max(1) % len;
    self.set_active_buffer((self.active_buffer + step) % len)
  }

  pub fn switch_buffer_backward(&mut self, count: usize) -> bool {
    let len = self.buffers.len();
    if len <= 1 {
      return false;
    }
    let step = count.max(1) % len;
    self.set_active_buffer((self.active_buffer + len - step) % len)
  }

  pub fn active_file_path(&self) -> Option<&Path> {
    self.buffers[self.active_buffer].file_path.as_deref()
  }

  pub fn set_active_file_path(&mut self, path: Option<PathBuf>) {
    self.buffers[self.active_buffer].file_path = path;
  }

  pub fn find_buffer_by_path(&self, path: &Path) -> Option<usize> {
    self
      .buffers
      .iter()
      .position(|buffer| buffer.file_path.as_deref() == Some(path))
  }

  pub fn open_buffer(&mut self, text: Rope, view: ViewState, file_path: Option<PathBuf>) -> usize {
    let document_id = DocumentId::new(self.next_document_id);
    let next_doc = self.next_document_id.get().saturating_add(1);
    self.next_document_id = NonZeroUsize::new(next_doc).unwrap_or(self.next_document_id);
    let document = Document::new(document_id, text);

    self
      .buffers
      .push(BufferState::new(document, view, file_path));
    let next_index = self.buffers.len() - 1;
    let _ = self.activate_buffer(next_index);
    next_index
  }

  pub fn goto_last_accessed_buffer(&mut self) -> bool {
    while let Some(index) = self.access_history.pop() {
      if index < self.buffers.len() && index != self.active_buffer {
        return self.set_active_buffer(index);
      }
    }
    false
  }

  pub fn mark_active_buffer_modified(&mut self) {
    self.touch_modified_history(self.active_buffer);
  }

  pub fn goto_last_modified_buffer(&mut self) -> bool {
    let current = self.active_buffer;
    let Some(index) = self
      .modified_history
      .iter()
      .rev()
      .copied()
      .find(|idx| *idx < self.buffers.len() && *idx != current)
    else {
      return false;
    };
    self.set_active_buffer(index)
  }

  pub fn last_modification_position(&self) -> Option<usize> {
    self.document().history().last_edit_pos()
  }
}

#[cfg(test)]
mod tests {
  use std::{
    num::NonZeroUsize,
    path::PathBuf,
  };

  use ropey::Rope;

  use super::*;
  use crate::{
    document::DocumentId,
    position::Position,
    render::graphics::Rect,
    transaction::Transaction,
  };

  #[test]
  fn editor_owns_document_and_view() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("hello"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());

    let editor = Editor::new(editor_id, doc, view);
    assert_eq!(editor.id(), editor_id);
    assert_eq!(editor.document().id(), doc_id);
    assert_eq!(editor.view().viewport, Rect::new(0, 0, 80, 24));
  }

  #[test]
  fn editor_switches_buffers_with_wraparound() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let view2 = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(1, 0));
    let view3 = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(2, 0));
    editor.open_buffer(
      Rope::from("two"),
      view2,
      Some(PathBuf::from("/tmp/two.txt")),
    );
    editor.open_buffer(
      Rope::from("three"),
      view3,
      Some(PathBuf::from("/tmp/three.txt")),
    );

    assert_eq!(editor.buffer_count(), 3);
    assert_eq!(editor.active_buffer_index(), 2);

    assert!(editor.switch_buffer_forward(1));
    assert_eq!(editor.active_buffer_index(), 0);

    assert!(editor.switch_buffer_backward(1));
    assert_eq!(editor.active_buffer_index(), 2);
  }

  #[test]
  fn editor_goto_last_accessed_buffer_toggles_between_recent_buffers() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let view2 = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(1, 0));
    let view3 = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(2, 0));
    editor.open_buffer(
      Rope::from("two"),
      view2,
      Some(PathBuf::from("/tmp/two.txt")),
    );
    editor.open_buffer(
      Rope::from("three"),
      view3,
      Some(PathBuf::from("/tmp/three.txt")),
    );

    assert_eq!(editor.active_buffer_index(), 2);
    assert!(editor.goto_last_accessed_buffer());
    assert_eq!(editor.active_buffer_index(), 1);
    assert!(editor.goto_last_accessed_buffer());
    assert_eq!(editor.active_buffer_index(), 2);
  }

  #[test]
  fn editor_goto_last_modified_buffer_uses_recent_edit_order() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    let view2 = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(1, 0));
    let view3 = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(2, 0));
    editor.open_buffer(
      Rope::from("two"),
      view2,
      Some(PathBuf::from("/tmp/two.txt")),
    );
    editor.open_buffer(
      Rope::from("three"),
      view3,
      Some(PathBuf::from("/tmp/three.txt")),
    );

    assert!(!editor.goto_last_modified_buffer());
    editor.mark_active_buffer_modified();
    let _ = editor.set_active_buffer(0);
    editor.mark_active_buffer_modified();
    let _ = editor.set_active_buffer(1);

    assert!(editor.goto_last_modified_buffer());
    assert_eq!(editor.active_buffer_index(), 0);
    assert!(editor.goto_last_modified_buffer());
    assert_eq!(editor.active_buffer_index(), 2);
  }

  #[test]
  fn editor_last_modification_position_reflects_committed_changes() {
    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, Rope::from("one"));
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    assert_eq!(editor.last_modification_position(), None);

    let tx = Transaction::change(
      editor.document().text(),
      std::iter::once((0, 0, Some("edit ".into()))),
    )
    .expect("insert transaction");
    editor
      .document_mut()
      .apply_transaction(&tx)
      .expect("apply insert");
    editor.document_mut().commit().expect("commit insert");

    assert_eq!(editor.last_modification_position(), Some(5));
  }
}
