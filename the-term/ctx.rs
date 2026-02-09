//! Application context (state).

use std::{
  collections::VecDeque,
  env,
  num::NonZeroUsize,
  path::{
    Path,
    PathBuf,
  },
  ptr::NonNull,
  sync::{
    Arc,
    mpsc::{
      Receiver,
      Sender,
      TryRecvError,
      channel,
    },
  },
  thread,
  time::Duration,
};

use eyre::Result;
use ropey::Rope;
use the_default::{
  CommandPaletteState,
  CommandPaletteStyle,
  CommandPromptState,
  CommandRegistry,
  DefaultDispatchStatic,
  DispatchRef,
  FilePickerState,
  KeyBinding,
  KeyEvent,
  Keymaps,
  MessagePresentation,
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
  messages::{
    MessageCenter,
    MessageLevel,
  },
  position::Position,
  registers::Registers,
  render::{
    RenderPlan,
    RenderStyles,
    UiState,
    graphics::Rect,
    text_annotations::{
      InlineAnnotation,
      Overlay,
      TextAnnotations,
    },
    text_format::TextFormat,
    theme::{
      Theme,
      base16_default_theme,
      default_theme,
    },
  },
  selection::Selection,
  syntax::{
    HighlightCache,
    Loader,
    Syntax,
    generate_edits,
  },
  transaction::Transaction,
  view::ViewState,
};
use the_lsp::{
  LspEvent,
  LspRuntime,
  LspRuntimeConfig,
  LspServerConfig,
};
use the_runtime::clipboard::ClipboardProvider;

use crate::picker_layout::FilePickerLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerDragState {
  ListScrollbar { grab_offset: u16 },
  PreviewScrollbar { grab_offset: u16 },
}

#[derive(Debug)]
pub struct SyntaxParseResult {
  pub request_id: u64,
  pub syntax:     Option<Syntax>,
}

/// Application state passed to all handlers.
pub struct Ctx {
  pub editor:                Editor,
  pub file_path:             Option<PathBuf>,
  pub should_quit:           bool,
  pub needs_render:          bool,
  pub messages:              MessageCenter,
  pub file_picker_wake_rx:   Receiver<()>,
  pub mode:                  Mode,
  pub keymaps:               Keymaps,
  pub command_prompt:        CommandPromptState,
  pub command_registry:      CommandRegistry<Ctx>,
  pub command_palette:       CommandPaletteState,
  pub command_palette_style: CommandPaletteStyle,
  pub file_picker:           FilePickerState,
  pub lsp_runtime:           LspRuntime,
  pub file_picker_layout:    Option<FilePickerLayout>,
  pub file_picker_drag:      Option<FilePickerDragState>,
  pub search_prompt:         the_default::SearchPromptState,
  pub ui_theme:              Theme,
  pub ui_state:              UiState,
  pub pending_input:         Option<the_default::PendingInput>,
  pub dispatch:              Option<NonNull<DefaultDispatchStatic<Ctx>>>,
  /// Syntax loader for language detection and highlighting.
  pub loader:                Option<Arc<Loader>>,
  /// Cache for syntax highlights (reused across renders).
  pub highlight_cache:       HighlightCache,
  /// Background parse result channel (async syntax fallback).
  pub syntax_parse_tx:       Sender<SyntaxParseResult>,
  /// Background parse result receiver (async syntax fallback).
  pub syntax_parse_rx:       Receiver<SyntaxParseResult>,
  /// Latest parse request id; stale parse results are discarded.
  pub syntax_parse_latest:   u64,
  /// Registers for yanking/pasting.
  pub registers:             Registers,
  /// Active register target (for macros/register operations).
  pub register:              Option<char>,
  /// Macro recording state.
  pub macro_recording:       Option<(char, Vec<KeyBinding>)>,
  /// Macro replay stack for recursion guard.
  pub macro_replaying:       Vec<char>,
  /// Pending macro key events to replay.
  pub macro_queue:           VecDeque<KeyEvent>,
  /// Last executed motion for repeat.
  pub last_motion:           Option<Motion>,
  /// Render formatting used for visual position mapping.
  pub text_format:           TextFormat,
  /// Inline annotations (virtual text) for rendering.
  pub inline_annotations:    Vec<InlineAnnotation>,
  /// Overlay annotations (virtual text) for rendering.
  pub overlay_annotations:   Vec<Overlay>,
  /// Lines to keep above/below cursor when scrolling.
  pub scrolloff:             usize,
}

fn select_ui_theme() -> Theme {
  match env::var("THE_EDITOR_THEME").ok().as_deref() {
    Some("base16") | Some("base16_default") => base16_default_theme().clone(),
    Some("default") | None => default_theme().clone(),
    Some(other) => {
      eprintln!("Unknown theme '{other}', falling back to default theme.");
      default_theme().clone()
    },
  }
}

fn lsp_server_from_env() -> Option<LspServerConfig> {
  let command = env::var("THE_EDITOR_LSP_COMMAND").ok()?.trim().to_string();
  if command.is_empty() {
    return None;
  }

  let mut server = LspServerConfig::new(command);
  if let Ok(args) = env::var("THE_EDITOR_LSP_ARGS") {
    let args: Vec<String> = args.split_whitespace().map(ToOwned::to_owned).collect();
    if !args.is_empty() {
      server = server.with_args(args);
    }
  }

  Some(server)
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
    let ui_theme = select_ui_theme();

    let loader = match init_loader(&ui_theme) {
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

    let (file_picker_wake_tx, file_picker_wake_rx) = std::sync::mpsc::channel();
    let mut file_picker = FilePickerState::default();
    the_default::set_file_picker_config(
      &mut file_picker,
      the_config::defaults::build_file_picker_config(),
    );
    the_default::set_file_picker_wake_sender(&mut file_picker, Some(file_picker_wake_tx));
    the_default::set_file_picker_syntax_loader(&mut file_picker, loader.clone());
    let (syntax_parse_tx, syntax_parse_rx) = channel();
    let workspace_root = file_path
      .map(PathBuf::from)
      .and_then(|path| {
        let path = if path.is_absolute() {
          path
        } else {
          env::current_dir().ok()?.join(path)
        };
        path.parent().map(|parent| parent.to_path_buf())
      })
      .map(|path| the_loader::find_workspace_in(path).0)
      .unwrap_or_else(|| the_loader::find_workspace().0);
    let mut lsp_runtime_config = LspRuntimeConfig::new(workspace_root);
    if let Some(server) = lsp_server_from_env() {
      lsp_runtime_config = lsp_runtime_config.with_server(server);
    }
    let lsp_runtime = LspRuntime::new(lsp_runtime_config);

    Ok(Self {
      editor,
      file_path: file_path.map(PathBuf::from),
      should_quit: false,
      needs_render: true,
      messages: MessageCenter::default(),
      file_picker_wake_rx,
      mode: Mode::Normal,
      keymaps: Keymaps::default(),
      command_prompt: CommandPromptState::new(),
      command_registry: CommandRegistry::new(),
      command_palette: CommandPaletteState::default(),
      command_palette_style: CommandPaletteStyle::helix_bottom(),
      file_picker,
      lsp_runtime,
      file_picker_layout: None,
      file_picker_drag: None,
      search_prompt: the_default::SearchPromptState::new(),
      ui_theme,
      ui_state: UiState::default(),
      pending_input: None,
      dispatch: None,
      loader,
      highlight_cache: HighlightCache::default(),
      syntax_parse_tx,
      syntax_parse_rx,
      syntax_parse_latest: 0,
      registers,
      register: None,
      macro_recording: None,
      macro_replaying: Vec::new(),
      macro_queue: VecDeque::new(),
      last_motion: None,
      text_format,
      inline_annotations: Vec::new(),
      overlay_annotations: Vec::new(),
      scrolloff: 5,
    })
  }

  pub fn set_dispatch(&mut self, dispatch: &DefaultDispatchStatic<Ctx>) {
    self.dispatch = Some(NonNull::from(dispatch));
  }

  /// Handle terminal resize.
  pub fn resize(&mut self, width: u16, height: u16) {
    self.editor.view_mut().viewport = Rect::new(0, 0, width, height);
  }

  pub fn poll_syntax_parse_results(&mut self) -> bool {
    let mut newest: Option<SyntaxParseResult> = None;
    loop {
      match self.syntax_parse_rx.try_recv() {
        Ok(result) => newest = Some(result),
        Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
      }
    }

    let Some(result) = newest else {
      return false;
    };

    if result.request_id != self.syntax_parse_latest {
      return false;
    }

    let doc = self.editor.document_mut();
    match result.syntax {
      Some(syntax) => {
        if let Some(loader) = &self.loader {
          doc.set_syntax_with_loader(syntax, loader.clone());
        } else {
          doc.set_syntax(syntax);
        }
      },
      None => doc.clear_syntax(),
    }
    self.highlight_cache.clear();
    true
  }

  pub fn start_background_services(&mut self) {
    if let Err(err) = self.lsp_runtime.start() {
      eprintln!("Warning: failed to start LSP runtime: {err}");
    }
  }

  pub fn shutdown_background_services(&mut self) {
    if let Err(err) = self.lsp_runtime.shutdown() {
      eprintln!("Warning: failed to stop LSP runtime: {err}");
    }
  }

  pub fn poll_lsp_events(&mut self) -> bool {
    let mut needs_render = false;
    while let Some(event) = self.lsp_runtime.try_recv_event() {
      if let LspEvent::Error(message) = event {
        self
          .messages
          .publish(MessageLevel::Error, Some("lsp".into()), message);
        needs_render = true;
      }
    }
    needs_render
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

  fn messages(&self) -> &MessageCenter {
    &self.messages
  }

  fn messages_mut(&mut self) -> &mut MessageCenter {
    &mut self.messages
  }

  fn message_presentation(&self) -> MessagePresentation {
    MessagePresentation::InlineStatusline
  }

  fn apply_transaction(&mut self, transaction: &Transaction) -> bool {
    let loader = self.loader.clone();
    let changes = transaction.changes().clone();

    let mut async_payload: Option<(Syntax, Rope, Rope, Arc<Loader>)> = None;
    {
      let doc = self.editor.document_mut();
      let old_text = doc.text().clone();
      if doc
        .apply_transaction_with_syntax(transaction, None)
        .is_err()
      {
        return false;
      }

      if transaction.changes().is_empty() {
        return true;
      }

      if let Some(loader) = loader.as_ref() {
        let new_text = doc.text().clone();
        let edits = generate_edits(old_text.slice(..), transaction.changes());
        let mut bump_syntax_version = false;
        let mut clear_syntax = false;

        if let Some(syntax) = doc.syntax_mut() {
          match syntax.try_update_with_short_timeout(
            new_text.slice(..),
            &edits,
            loader.as_ref(),
            Duration::from_millis(3),
          ) {
            Ok(true) => {
              bump_syntax_version = true;
            },
            Ok(false) => {
              syntax.interpolate(old_text.slice(..), transaction.changes());
              bump_syntax_version = true;
              async_payload = Some((syntax.clone(), old_text.clone(), new_text, loader.clone()));
            },
            Err(_) => {
              clear_syntax = true;
            },
          }
        }

        if clear_syntax {
          doc.clear_syntax();
          self.highlight_cache.clear();
        } else if bump_syntax_version {
          doc.bump_syntax_version();
        }
      }
    }

    if let Some((mut syntax, old_text, new_text, loader)) = async_payload {
      self.syntax_parse_latest = self.syntax_parse_latest.saturating_add(1);
      let request_id = self.syntax_parse_latest;
      let tx = self.syntax_parse_tx.clone();
      thread::spawn(move || {
        let parsed = syntax
          .update(
            old_text.slice(..),
            new_text.slice(..),
            &changes,
            loader.as_ref(),
          )
          .ok()
          .map(|_| syntax);
        let _ = tx.send(SyntaxParseResult {
          request_id,
          syntax: parsed,
        });
      });
    }

    true
  }

  fn build_render_plan(&mut self) -> RenderPlan {
    crate::render::build_render_plan(self)
  }

  fn build_render_plan_with_styles(&mut self, styles: RenderStyles) -> RenderPlan {
    crate::render::build_render_plan_with_styles(self, styles)
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

  fn command_palette(&self) -> &CommandPaletteState {
    &self.command_palette
  }

  fn command_palette_mut(&mut self) -> &mut CommandPaletteState {
    &mut self.command_palette
  }

  fn command_palette_style(&self) -> &CommandPaletteStyle {
    &self.command_palette_style
  }

  fn command_palette_style_mut(&mut self) -> &mut CommandPaletteStyle {
    &mut self.command_palette_style
  }

  fn file_picker(&self) -> &FilePickerState {
    &self.file_picker
  }

  fn file_picker_mut(&mut self) -> &mut FilePickerState {
    &mut self.file_picker
  }

  fn search_prompt_ref(&self) -> &the_default::SearchPromptState {
    &self.search_prompt
  }

  fn search_prompt_mut(&mut self) -> &mut the_default::SearchPromptState {
    &mut self.search_prompt
  }

  fn ui_state(&self) -> &UiState {
    &self.ui_state
  }

  fn ui_state_mut(&mut self) -> &mut UiState {
    &mut self.ui_state
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

  fn register(&self) -> Option<char> {
    self.register
  }

  fn set_register(&mut self, register: Option<char>) {
    self.register = register;
  }

  fn macro_recording(&self) -> &Option<(char, Vec<KeyBinding>)> {
    &self.macro_recording
  }

  fn set_macro_recording(&mut self, recording: Option<(char, Vec<KeyBinding>)>) {
    self.macro_recording = recording;
  }

  fn macro_replaying(&self) -> &Vec<char> {
    &self.macro_replaying
  }

  fn macro_replaying_mut(&mut self) -> &mut Vec<char> {
    &mut self.macro_replaying
  }

  fn macro_queue(&self) -> &VecDeque<KeyEvent> {
    &self.macro_queue
  }

  fn macro_queue_mut(&mut self) -> &mut VecDeque<KeyEvent> {
    &mut self.macro_queue
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

  fn syntax_loader(&self) -> Option<&Loader> {
    self.loader.as_deref()
  }

  fn ui_theme(&self) -> &Theme {
    &self.ui_theme
  }

  fn set_file_path(&mut self, path: Option<PathBuf>) {
    self.file_path = path;
  }

  fn open_file(&mut self, path: &Path) -> std::io::Result<()> {
    let content = std::fs::read_to_string(path)?;

    {
      let doc = self.editor.document_mut();
      let len = doc.text().len_chars();
      let tx = Transaction::change(doc.text(), vec![(0, len, Some(content.as_str().into()))])
        .map_err(|err| std::io::Error::other(err.to_string()))?;
      doc
        .apply_transaction(&tx)
        .map_err(|err| std::io::Error::other(err.to_string()))?;
      let _ = doc.set_selection(Selection::point(0));
      doc.clear_syntax();
      if let Some(loader) = &self.loader {
        let _ = setup_syntax(doc, path, loader);
      }
      doc.set_display_name(
        path
          .file_name()
          .map(|name| name.to_string_lossy().to_string())
          .unwrap_or_else(|| path.display().to_string()),
      );
      let _ = doc.mark_saved();
    }

    self.syntax_parse_latest = self.syntax_parse_latest.saturating_add(1);
    self.highlight_cache.clear();

    self.file_path = Some(path.to_path_buf());
    self.editor.view_mut().scroll = Position::new(0, 0);
    self.needs_render = true;
    Ok(())
  }

  fn scrolloff(&self) -> usize {
    self.scrolloff
  }
}

/// Initialize the syntax loader with languages.toml config.
fn init_loader(theme: &Theme) -> Result<Loader> {
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
  loader.set_scopes(theme.scopes().iter().cloned().collect());

  Ok(loader)
}

/// Set up syntax highlighting for a document based on filename.
fn setup_syntax(doc: &mut Document, path: &Path, loader: &Arc<Loader>) -> Result<()> {
  // Detect language from filename
  let lang = loader
    .language_for_filename(path)
    .ok_or_else(|| eyre::eyre!("unknown language for {}", path.display()))?;

  // Create syntax tree
  let syntax =
    Syntax::new(doc.text().slice(..), lang, loader.as_ref()).map_err(|e| eyre::eyre!("{e}"))?;
  doc.set_syntax_with_loader(syntax, loader.clone());

  Ok(())
}
