//! Global application state for the-lib.
//!
//! The `App` owns configuration, command registry, and the set of active
//! editors (surfaces). It does not perform IO; platform hosts call into
//! this API through FFI or other adapters.

use std::{
  collections::BTreeMap,
  num::NonZeroUsize,
};

use ropey::Rope;

use crate::{
  document::{
    Document,
    DocumentId,
  },
  editor::{
    Editor,
    EditorId,
  },
  view::ViewState,
};

#[derive(Debug)]
pub struct App<Config = (), Commands = (), Global = ()> {
  config:           Config,
  commands:         Commands,
  global:           Global,
  next_editor_id:   NonZeroUsize,
  next_document_id: NonZeroUsize,
  editors:          BTreeMap<EditorId, Editor>,
}

impl<Config, Commands, Global> App<Config, Commands, Global> {
  pub fn new(config: Config, commands: Commands, global: Global) -> Self {
    Self {
      config,
      commands,
      global,
      next_editor_id: NonZeroUsize::new(1).unwrap(),
      next_document_id: NonZeroUsize::new(1).unwrap(),
      editors: BTreeMap::new(),
    }
  }

  pub fn config(&self) -> &Config {
    &self.config
  }

  pub fn config_mut(&mut self) -> &mut Config {
    &mut self.config
  }

  pub fn commands(&self) -> &Commands {
    &self.commands
  }

  pub fn commands_mut(&mut self) -> &mut Commands {
    &mut self.commands
  }

  pub fn global(&self) -> &Global {
    &self.global
  }

  pub fn global_mut(&mut self) -> &mut Global {
    &mut self.global
  }

  pub fn editor(&self, id: EditorId) -> Option<&Editor> {
    self.editors.get(&id)
  }

  pub fn editor_mut(&mut self, id: EditorId) -> Option<&mut Editor> {
    self.editors.get_mut(&id)
  }

  pub fn editors(&self) -> impl Iterator<Item = &Editor> {
    self.editors.values()
  }

  pub fn editors_mut(&mut self) -> impl Iterator<Item = &mut Editor> {
    self.editors.values_mut()
  }

  pub fn create_editor(&mut self, text: Rope, view: ViewState) -> EditorId {
    let doc_id = DocumentId::new(self.next_document_id);
    let next_doc = self.next_document_id.get().saturating_add(1);
    self.next_document_id = NonZeroUsize::new(next_doc).unwrap_or(self.next_document_id);

    let editor_id = EditorId::new(self.next_editor_id);
    let next_editor = self.next_editor_id.get().saturating_add(1);
    self.next_editor_id = NonZeroUsize::new(next_editor).unwrap_or(self.next_editor_id);

    let doc = Document::new(doc_id, text);
    let editor = Editor::new(editor_id, doc, view);
    self.editors.insert(editor_id, editor);
    editor_id
  }

  pub fn remove_editor(&mut self, id: EditorId) -> Option<Editor> {
    self.editors.remove(&id)
  }
}

impl Default for App {
  fn default() -> Self {
    Self::new((), (), ())
  }
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::*;
  use crate::{
    position::Position,
    render::graphics::Rect,
  };

  #[test]
  fn create_and_remove_editor() {
    let mut app = App::default();
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let id = app.create_editor(Rope::from("hello"), view);
    assert!(app.editor(id).is_some());
    let removed = app.remove_editor(id);
    assert!(removed.is_some());
    assert!(app.editor(id).is_none());
  }
}
