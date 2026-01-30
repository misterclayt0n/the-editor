//! Application context (state).

use std::{
  num::NonZeroUsize,
  path::{
    Path,
    PathBuf,
  },
  ptr::NonNull,
  sync::Arc,
};

use eyre::Result;
use ropey::Rope;
use the_default::{
  CommandPromptState,
  CommandRegistry,
  DefaultDispatchStatic,
  DispatchRef,
  Keymaps,
  Mode,
  Motion,
};
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
  registers::Registers,
  render::graphics::Rect,
  render::text_annotations::{
    InlineAnnotation,
    Overlay,
    TextAnnotations,
  },
  render::text_format::TextFormat,
  syntax::{
    HighlightCache,
    Loader,
    Syntax,
  },
  view::ViewState,
};
use the_runtime::clipboard::ClipboardProvider;

/// Application state passed to all handlers.
pub struct Ctx {
  pub editor:           Editor,
  pub file_path:        Option<PathBuf>,
  pub should_quit:      bool,
  pub needs_render:     bool,
  pub mode:             Mode,
  pub keymaps:          Keymaps,
  pub command_prompt:   CommandPromptState,
  pub command_registry: CommandRegistry<Ctx>,
  pub pending_input:    Option<the_default::PendingInput>,
  pub dispatch:         Option<NonNull<DefaultDispatchStatic<Ctx>>>,
  /// Syntax loader for language detection and highlighting.
  pub loader:           Option<Arc<Loader>>,
  /// Cache for syntax highlights (reused across renders).
  pub highlight_cache:  HighlightCache,
  /// Registers for yanking/pasting.
  pub registers:        Registers,
  /// Last executed motion for repeat.
  pub last_motion:      Option<Motion>,
  /// Render formatting used for visual position mapping.
  pub text_format:      TextFormat,
  /// Inline annotations (virtual text) for rendering.
  pub inline_annotations: Vec<InlineAnnotation>,
  /// Overlay annotations (virtual text) for rendering.
  pub overlay_annotations: Vec<Overlay>,
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

    // Initialize clipboard provider and registers
    let clipboard = Arc::new(ClipboardProvider::detect());
    let registers = Registers::with_clipboard(clipboard);

    let mut text_format = TextFormat::default();
    text_format.viewport_width = viewport.width;

    Ok(Self {
      editor,
      file_path: file_path.map(PathBuf::from),
      should_quit: false,
      needs_render: true,
      mode: Mode::Normal,
      keymaps: Keymaps::default(),
      command_prompt: CommandPromptState::new(),
      command_registry: CommandRegistry::new(),
      pending_input: None,
      dispatch: None,
      loader,
      highlight_cache: HighlightCache::default(),
      registers,
      last_motion: None,
      text_format,
      inline_annotations: Vec::new(),
      overlay_annotations: Vec::new(),
    })
  }

  pub fn set_dispatch(&mut self, dispatch: &DefaultDispatchStatic<Ctx>) {
    self.dispatch = Some(NonNull::from(dispatch));
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

  fn editor_ref(&self) -> &Editor {
    &self.editor
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

  fn mode(&self) -> Mode {
    self.mode
  }

  fn set_mode(&mut self, mode: Mode) {
    self.mode = mode;
  }

  fn keymaps(&mut self) -> &mut Keymaps {
    &mut self.keymaps
  }

  fn command_prompt_mut(&mut self) -> &mut CommandPromptState {
    &mut self.command_prompt
  }

  fn command_prompt_ref(&self) -> &CommandPromptState {
    &self.command_prompt
  }

  fn command_registry_mut(&mut self) -> &mut CommandRegistry<Self> {
    &mut self.command_registry
  }

  fn command_registry_ref(&self) -> &CommandRegistry<Self> {
    &self.command_registry
  }

  fn dispatch(&self) -> DispatchRef<Self> {
    let Some(ptr) = self.dispatch else {
      panic!("dispatch is not set");
    };
    DispatchRef::from_ptr(ptr.as_ptr())
  }

  fn pending_input(&self) -> Option<&the_default::PendingInput> {
    self.pending_input.as_ref()
  }

  fn set_pending_input(&mut self, pending: Option<the_default::PendingInput>) {
    self.pending_input = pending;
  }

  fn registers(&self) -> &Registers {
    &self.registers
  }

  fn registers_mut(&mut self) -> &mut Registers {
    &mut self.registers
  }

  fn last_motion(&self) -> Option<Motion> {
    self.last_motion
  }

  fn set_last_motion(&mut self, motion: Option<Motion>) {
    self.last_motion = motion;
  }

  fn text_format(&self) -> TextFormat {
    self.text_format.clone()
  }

  fn text_annotations(&self) -> TextAnnotations<'_> {
    let mut annotations = TextAnnotations::default();
    if !self.inline_annotations.is_empty() {
      let _ = annotations.add_inline_annotations(&self.inline_annotations, None);
    }
    if !self.overlay_annotations.is_empty() {
      let _ = annotations.add_overlay(&self.overlay_annotations, None);
    }
    annotations
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
