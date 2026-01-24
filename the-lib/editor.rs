//! Minimal editor/session state for the-lib.
//!
//! This is intentionally small: it owns documents and ids and provides
//! helpers to create and access documents. IO, UI, and dispatch logic live
//! outside of the-lib.

use std::{collections::BTreeMap, num::NonZeroUsize};

use ropey::Rope;

use crate::document::{Document, DocumentId};

#[derive(Debug)]
pub struct Editor {
  next_document_id: NonZeroUsize,
  documents: BTreeMap<DocumentId, Document>,
}

impl Editor {
  pub fn new() -> Self {
    Self {
      next_document_id: NonZeroUsize::new(1).unwrap(),
      documents: BTreeMap::new(),
    }
  }

  pub fn document(&self, id: DocumentId) -> Option<&Document> {
    self.documents.get(&id)
  }

  pub fn document_mut(&mut self, id: DocumentId) -> Option<&mut Document> {
    self.documents.get_mut(&id)
  }

  pub fn documents(&self) -> impl Iterator<Item = &Document> {
    self.documents.values()
  }

  pub fn documents_mut(&mut self) -> impl Iterator<Item = &mut Document> {
    self.documents.values_mut()
  }

  pub fn create_document(&mut self, text: Rope) -> DocumentId {
    let id = DocumentId::new(self.next_document_id);
    let next = self.next_document_id.get().saturating_add(1);
    self.next_document_id = NonZeroUsize::new(next).unwrap_or(self.next_document_id);
    self.documents.insert(id, Document::new(id, text));
    id
  }

  pub fn remove_document(&mut self, id: DocumentId) -> Option<Document> {
    self.documents.remove(&id)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn create_and_remove_document() {
    let mut editor = Editor::new();
    let id = editor.create_document(Rope::from("hello"));
    assert!(editor.document(id).is_some());
    let removed = editor.remove_document(id);
    assert!(removed.is_some());
    assert!(editor.document(id).is_none());
  }
}
