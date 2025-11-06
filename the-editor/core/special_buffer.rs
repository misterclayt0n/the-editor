use std::{
  collections::{
    HashMap,
    HashSet,
  },
  fmt,
};

use crate::core::DocumentId;

/// Kinds of editor-managed special buffers such as compilation outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialBufferKind {
  Compilation,
}

impl SpecialBufferKind {
  pub const fn display_name(self) -> &'static str {
    match self {
      SpecialBufferKind::Compilation => COMPILATION_BUFFER_NAME,
    }
  }
}

pub const COMPILATION_BUFFER_NAME: &str = "*compilation*";

impl fmt::Display for SpecialBufferKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.display_name())
  }
}

#[derive(Debug, Default, Clone)]
pub struct SpecialBuffers {
  doc_to_kind:   HashMap<DocumentId, SpecialBufferKind>,
  last_for_kind: HashMap<SpecialBufferKind, DocumentId>,
  running:       HashSet<DocumentId>,
}

impl SpecialBuffers {
  pub fn register(&mut self, doc_id: DocumentId, kind: SpecialBufferKind) {
    self.unregister(doc_id);
    self.doc_to_kind.insert(doc_id, kind);
    self.last_for_kind.insert(kind, doc_id);
    self.running.remove(&doc_id);
  }

  pub fn unregister(&mut self, doc_id: DocumentId) {
    if let Some(kind) = self.doc_to_kind.remove(&doc_id) {
      if self.last_for_kind.get(&kind) == Some(&doc_id) {
        let replacement = self
          .doc_to_kind
          .iter()
          .find_map(|(&id, &other_kind)| (other_kind == kind).then_some(id));
        if let Some(id) = replacement {
          self.last_for_kind.insert(kind, id);
        } else {
          self.last_for_kind.remove(&kind);
        }
      }
    }
    self.running.remove(&doc_id);
  }

  pub fn mark_active(&mut self, doc_id: DocumentId) {
    if let Some(kind) = self.doc_to_kind.get(&doc_id).copied() {
      self.last_for_kind.insert(kind, doc_id);
    }
  }

  pub fn kind_of(&self, doc_id: DocumentId) -> Option<SpecialBufferKind> {
    self.doc_to_kind.get(&doc_id).copied()
  }

  pub fn last_for(&self, kind: SpecialBufferKind) -> Option<DocumentId> {
    self.last_for_kind.get(&kind).copied()
  }

  pub fn iter(&self) -> impl Iterator<Item = (DocumentId, SpecialBufferKind)> + '_ {
    self.doc_to_kind.iter().map(|(&doc, &kind)| (doc, kind))
  }

  pub fn set_running(&mut self, doc_id: DocumentId, running: bool) {
    if running {
      self.running.insert(doc_id);
    } else {
      self.running.remove(&doc_id);
    }
  }

  pub fn is_running(&self, doc_id: DocumentId) -> bool {
    self.running.contains(&doc_id)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn doc_id(value: usize) -> DocumentId {
    DocumentId(std::num::NonZeroUsize::new(value).unwrap())
  }

  #[test]
  fn registers_and_tracks_last_active() {
    let mut buffers = SpecialBuffers::default();
    let compile = doc_id(1);

    buffers.register(compile, SpecialBufferKind::Compilation);
    assert_eq!(
      buffers.last_for(SpecialBufferKind::Compilation),
      Some(compile)
    );
  }

  #[test]
  fn tracks_running_state() {
    let mut buffers = SpecialBuffers::default();
    let compile = doc_id(1);

    buffers.register(compile, SpecialBufferKind::Compilation);
    assert!(!buffers.is_running(compile));

    buffers.set_running(compile, true);
    assert!(buffers.is_running(compile));

    buffers.unregister(compile);
    assert!(!buffers.is_running(compile));
  }
}
