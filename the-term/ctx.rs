//! Application context (state).

use std::path::PathBuf;

use eyre::Result;
use ropey::Rope;
use the_lib::{
  document::DocumentId,
  editor::Editor,
  position::Position,
  render::graphics::Rect,
  view::ViewState,
};

/// Application state passed to all handlers.
pub struct Ctx {
  pub editor:       Editor,
  pub view:         ViewState,
  pub active_doc:   DocumentId,
  pub file_path:    Option<PathBuf>,
  pub should_quit:  bool,
  pub needs_render: bool,
}

impl Ctx {
  pub fn new(file_path: Option<&str>) -> Result<Self> {
    // Load text from file or create empty document
    let text = if let Some(path) = file_path {
      Rope::from(std::fs::read_to_string(path).unwrap_or_default())
    } else {
      Rope::new()
    };

    // Create editor and document
    let mut editor = Editor::new();
    let doc_id = editor.create_document(text);

    // Get terminal size for viewport
    let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
    let viewport = Rect::new(0, 0, width, height);
    let scroll = Position::new(0, 0);
    let view = ViewState::new(viewport, scroll);

    Ok(Self {
      editor,
      view,
      active_doc: doc_id,
      file_path: file_path.map(PathBuf::from),
      should_quit: false,
      needs_render: true,
    })
  }

  /// Handle terminal resize.
  pub fn resize(&mut self, width: u16, height: u16) {
    self.view.viewport = Rect::new(0, 0, width, height);
  }
}
