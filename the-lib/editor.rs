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
    }
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
    if index >= self.buffers.len() {
      return false;
    }
    self.active_buffer = index;
    true
  }

  pub fn switch_buffer_forward(&mut self, count: usize) -> bool {
    let len = self.buffers.len();
    if len <= 1 {
      return false;
    }
    let step = count.max(1) % len;
    self.active_buffer = (self.active_buffer + step) % len;
    true
  }

  pub fn switch_buffer_backward(&mut self, count: usize) -> bool {
    let len = self.buffers.len();
    if len <= 1 {
      return false;
    }
    let step = count.max(1) % len;
    self.active_buffer = (self.active_buffer + len - step) % len;
    true
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

    self.buffers.push(BufferState::new(document, view, file_path));
    self.active_buffer = self.buffers.len() - 1;
    self.active_buffer
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
    editor.open_buffer(Rope::from("two"), view2, Some(PathBuf::from("/tmp/two.txt")));
    editor.open_buffer(Rope::from("three"), view3, Some(PathBuf::from("/tmp/three.txt")));

    assert_eq!(editor.buffer_count(), 3);
    assert_eq!(editor.active_buffer_index(), 2);

    assert!(editor.switch_buffer_forward(1));
    assert_eq!(editor.active_buffer_index(), 0);

    assert!(editor.switch_buffer_backward(1));
    assert_eq!(editor.active_buffer_index(), 2);
  }
}
