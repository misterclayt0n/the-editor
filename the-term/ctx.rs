//! Application context (state).

use std::{
  num::NonZeroUsize,
  path::{
    Path,
    PathBuf,
  },
  sync::Arc,
};

use eyre::Result;
use ropey::Rope;
use the_lib::{
  document::{
    Document,
    DocumentId,
  },
  editor::{
    Editor,
    EditorId,
  },
  position::Position,
  render::graphics::Rect,
  syntax::{
    HighlightCache,
    Loader,
    Syntax,
  },
  view::ViewState,
};

/// Application state passed to all handlers.
pub struct Ctx {
  pub editor:          Editor,
  pub file_path:       Option<PathBuf>,
  pub should_quit:     bool,
  pub needs_render:    bool,
  /// Syntax loader for language detection and highlighting.
  pub loader:          Option<Arc<Loader>>,
  /// Cache for syntax highlights (reused across renders).
  pub highlight_cache: HighlightCache,
}

impl Ctx {
  pub fn new(file_path: Option<&str>) -> Result<Self> {
    // Load text from file or create empty document
    let text = if let Some(path) = file_path {
      Rope::from(std::fs::read_to_string(path).unwrap_or_default())
    } else {
      Rope::new()
    };

    let doc_id = DocumentId::new(NonZeroUsize::new(1).unwrap());
    let doc = Document::new(doc_id, text);

    // Get terminal size for viewport
    let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
    let viewport = Rect::new(0, 0, width, height);
    let scroll = Position::new(0, 0);
    let view = ViewState::new(viewport, scroll);

    let editor_id = EditorId::new(NonZeroUsize::new(1).unwrap());
    let mut editor = Editor::new(editor_id, doc, view);

    // Initialize syntax loader
    let loader = match init_loader() {
      Ok(loader) => Some(Arc::new(loader)),
      Err(e) => {
        eprintln!("Warning: syntax highlighting unavailable: {e}");
        None
      },
    };

    // Set up syntax on document if we have a loader and file path
    if let (Some(loader), Some(path)) = (&loader, file_path) {
      let doc = editor.document_mut();
      if let Err(e) = setup_syntax(doc, Path::new(path), loader) {
        eprintln!("Warning: could not enable syntax for file: {e}");
      }
    }

    Ok(Self {
      editor,
      file_path: file_path.map(PathBuf::from),
      should_quit: false,
      needs_render: true,
      loader,
      highlight_cache: HighlightCache::default(),
    })
  }

  /// Handle terminal resize.
  pub fn resize(&mut self, width: u16, height: u16) {
    self.editor.view_mut().viewport = Rect::new(0, 0, width, height);
  }
}

impl the_default::DefaultContext for Ctx {
  fn editor(&mut self) -> &mut Editor {
    &mut self.editor
  }

  fn file_path(&self) -> Option<&Path> {
    self.file_path.as_deref()
  }

  fn request_render(&mut self) {
    self.needs_render = true;
  }

  fn request_quit(&mut self) {
    self.should_quit = true;
  }
}

/// Initialize the syntax loader with languages.toml config.
fn init_loader() -> Result<Loader> {
  use the_lib::syntax::{
    config::Configuration,
    runtime_loader::RuntimeLoader,
  };
  use the_loader::config::user_lang_config;

  // Parse languages.toml (built-in + user overrides)
  let config_value = user_lang_config()?;
  let config: Configuration = config_value.try_into()?;

  // Create loader with runtime resources (grammars from runtime/grammars/)
  let loader = Loader::new(config, RuntimeLoader::new())?;

  // Set up highlight scopes so Highlight indices map to our theme
  loader.set_scopes(crate::theme::SCOPES.iter().map(|s| s.to_string()).collect());

  Ok(loader)
}

/// Set up syntax highlighting for a document based on filename.
fn setup_syntax(doc: &mut Document, path: &Path, loader: &Loader) -> Result<()> {
  // Detect language from filename
  let lang = loader
    .language_for_filename(path)
    .ok_or_else(|| eyre::eyre!("unknown language for {}", path.display()))?;

  // Create syntax tree
  let syntax = Syntax::new(doc.text().slice(..), lang, loader).map_err(|e| eyre::eyre!("{e}"))?;
  doc.set_syntax(syntax);

  Ok(())
}
