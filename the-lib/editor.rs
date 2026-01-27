//! Minimal editor/surface state for the-lib.
//!
//! This is intentionally small: it owns a single document plus view/render
//! state. IO, UI, and dispatch logic live outside of the-lib.

use std::num::NonZeroUsize;

use crate::{
  document::Document,
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
  id:           EditorId,
  document:     Document,
  view:         ViewState,
  render_cache: RenderCache,
}

impl Editor {
  pub fn new(id: EditorId, document: Document, view: ViewState) -> Self {
    Self {
      id,
      document,
      view,
      render_cache: RenderCache::default(),
    }
  }

  pub fn id(&self) -> EditorId {
    self.id
  }

  pub fn document(&self) -> &Document {
    &self.document
  }

  pub fn document_mut(&mut self) -> &mut Document {
    &mut self.document
  }

  pub fn view(&self) -> ViewState {
    self.view
  }

  pub fn view_mut(&mut self) -> &mut ViewState {
    &mut self.view
  }

  pub fn render_cache(&self) -> &RenderCache {
    &self.render_cache
  }

  pub fn render_cache_mut(&mut self) -> &mut RenderCache {
    &mut self.render_cache
  }

  pub fn document_and_cache(&mut self) -> (&Document, &mut RenderCache) {
    (&self.document, &mut self.render_cache)
  }
}

#[cfg(test)]
mod tests {
  use std::num::NonZeroUsize;

  use ropey::Rope;

  use super::*;
  use crate::document::DocumentId;
  use crate::position::Position;
  use crate::render::graphics::Rect;

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
}
