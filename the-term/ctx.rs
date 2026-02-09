//! Application context (state).

use std::{
  collections::{
    HashMap,
    VecDeque,
  },
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
use serde_json::{
  Value,
  json,
};
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
  diagnostics::{
    Diagnostic,
    DiagnosticCounts,
    DiagnosticSeverity,
    DiagnosticsState,
  },
  document::{
    Document,
    DocumentId,
  },
  editor::{
    Editor,
    EditorId,
  },
  indent::IndentStyle,
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
  transaction::{
    ChangeSet,
    Transaction,
  },
  view::ViewState,
};
use the_lsp::{
  LspCapability,
  LspCompletionItem,
  LspEvent,
  LspExecuteCommand,
  LspLocation,
  LspPosition,
  LspRuntime,
  LspRuntimeConfig,
  LspServerConfig,
  LspSymbol,
  LspTextEdit,
  LspWorkspaceEdit,
  TextDocumentSyncKind,
  code_action_params,
  completion_params,
  document_symbols_params,
  execute_command_params,
  formatting_params,
  goto_definition_params,
  hover_params,
  jsonrpc,
  parse_code_actions_response,
  parse_completion_response,
  parse_document_symbols_response,
  parse_formatting_response,
  parse_hover_response,
  parse_locations_response,
  parse_signature_help_response,
  parse_workspace_edit_response,
  parse_workspace_symbols_response,
  references_params,
  rename_params,
  signature_help_params,
  text_sync::{
    char_idx_to_utf16_position,
    did_change_params,
    did_close_params,
    did_open_params,
    did_save_params,
    file_uri_for_path,
    path_for_file_uri,
    utf16_position_to_char_idx,
  },
  workspace_symbols_params,
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

#[derive(Debug, Clone)]
pub struct LspDocumentSyncState {
  pub path:        PathBuf,
  pub uri:         String,
  pub language_id: String,
  pub version:     i32,
  pub opened:      bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingLspRequestKind {
  GotoDefinition {
    uri: String,
  },
  Hover {
    uri: String,
  },
  References {
    uri: String,
  },
  DocumentSymbols {
    uri: String,
  },
  WorkspaceSymbols {
    query: String,
  },
  Completion {
    uri:           String,
    fallback_char: usize,
  },
  SignatureHelp {
    uri: String,
  },
  CodeActions {
    uri: String,
  },
  Rename {
    uri: String,
  },
  Format {
    uri: String,
  },
}

impl PendingLspRequestKind {
  fn label(&self) -> &'static str {
    match self {
      Self::GotoDefinition { .. } => "goto-definition",
      Self::Hover { .. } => "hover",
      Self::References { .. } => "references",
      Self::DocumentSymbols { .. } => "document-symbols",
      Self::WorkspaceSymbols { .. } => "workspace-symbols",
      Self::Completion { .. } => "completion",
      Self::SignatureHelp { .. } => "signature-help",
      Self::CodeActions { .. } => "code-actions",
      Self::Rename { .. } => "rename",
      Self::Format { .. } => "format",
    }
  }

  fn uri(&self) -> Option<&str> {
    match self {
      Self::GotoDefinition { uri }
      | Self::Hover { uri }
      | Self::References { uri }
      | Self::DocumentSymbols { uri }
      | Self::Completion { uri, .. }
      | Self::SignatureHelp { uri }
      | Self::CodeActions { uri }
      | Self::Rename { uri }
      | Self::Format { uri } => Some(uri.as_str()),
      Self::WorkspaceSymbols { .. } => None,
    }
  }
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
  pub lsp_ready:             bool,
  pub lsp_document:          Option<LspDocumentSyncState>,
  lsp_pending_requests:      HashMap<u64, PendingLspRequestKind>,
  pub diagnostics:           DiagnosticsState,
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

  let mut server = LspServerConfig::new(command.clone(), command);
  if let Ok(args) = env::var("THE_EDITOR_LSP_ARGS") {
    let args: Vec<String> = args.split_whitespace().map(ToOwned::to_owned).collect();
    if !args.is_empty() {
      server = server.with_args(args);
    }
  }

  Some(server)
}

fn lsp_server_from_language_config(loader: &Loader, path: &Path) -> Option<LspServerConfig> {
  let language = loader.language_for_filename(path)?;
  let language_config = loader.language(language).config();
  let server_features = language_config.services.language_servers.first()?;
  let server_name = server_features.name.clone();
  let server_config = loader.language_server_configs().get(&server_name)?;

  Some(
    LspServerConfig::new(server_name, server_config.command.clone())
      .with_args(server_config.args.clone())
      .with_env(
        server_config
          .environment
          .iter()
          .map(|(key, value)| (key.clone(), value.clone())),
      )
      .with_initialize_options(server_config.config.clone())
      .with_initialize_timeout(Duration::from_secs(server_config.timeout)),
  )
}

fn lsp_language_id_for_path(loader: Option<&Loader>, path: &Path) -> Option<String> {
  let loader = loader?;
  let language = loader.language_for_filename(path)?;
  let language_config = loader.language(language).config();
  Some(
    language_config
      .services
      .language_server_language_id
      .clone()
      .unwrap_or_else(|| language_config.syntax.language_id.clone()),
  )
}

fn build_lsp_document_state(path: &Path, loader: Option<&Loader>) -> Option<LspDocumentSyncState> {
  let uri = file_uri_for_path(path)?;
  let language_id = lsp_language_id_for_path(loader, path).unwrap_or_else(|| "plaintext".into());
  Some(LspDocumentSyncState {
    path: path.to_path_buf(),
    uri,
    language_id,
    version: 1,
    opened: false,
  })
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
    let server_from_language = file_path.map(Path::new).and_then(|path| {
      loader
        .as_deref()
        .and_then(|loader| lsp_server_from_language_config(loader, path))
    });
    if let Some(server) = server_from_language.or_else(lsp_server_from_env) {
      lsp_runtime_config = lsp_runtime_config.with_server(server);
    }
    let lsp_runtime = LspRuntime::new(lsp_runtime_config);
    let lsp_document = file_path
      .map(PathBuf::from)
      .as_deref()
      .and_then(|path| build_lsp_document_state(path, loader.as_deref()));

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
      lsp_ready: false,
      lsp_document,
      lsp_pending_requests: HashMap::new(),
      diagnostics: DiagnosticsState::default(),
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
    self.lsp_ready = false;
    self.lsp_pending_requests.clear();
    if let Err(err) = self.lsp_runtime.start() {
      eprintln!("Warning: failed to start LSP runtime: {err}");
    }
  }

  pub fn shutdown_background_services(&mut self) {
    self.lsp_close_current_document();
    self.lsp_ready = false;
    self.lsp_pending_requests.clear();
    if let Err(err) = self.lsp_runtime.shutdown() {
      eprintln!("Warning: failed to stop LSP runtime: {err}");
    }
  }

  pub fn poll_lsp_events(&mut self) -> bool {
    let mut needs_render = false;
    while let Some(event) = self.lsp_runtime.try_recv_event() {
      match event {
        LspEvent::CapabilitiesRegistered { server_name } => {
          let matches_configured_server = self
            .lsp_runtime
            .config()
            .server()
            .is_some_and(|server| server.name() == server_name);
          if matches_configured_server {
            self.lsp_ready = true;
            self.lsp_open_current_document();
            needs_render = true;
          }
        },
        LspEvent::ServerStarted { .. } => {
          self.lsp_ready = false;
          self.lsp_pending_requests.clear();
          if let Some(state) = self.lsp_document.as_mut() {
            state.opened = false;
          }
        },
        LspEvent::ServerStopped { .. } | LspEvent::Stopped => {
          self.lsp_ready = false;
          self.lsp_pending_requests.clear();
          if let Some(state) = self.lsp_document.as_mut() {
            state.opened = false;
          }
        },
        LspEvent::RpcMessage { message } => {
          needs_render |= self.handle_lsp_rpc_message(message);
        },
        LspEvent::Error(message) => {
          self
            .messages
            .publish(MessageLevel::Error, Some("lsp".into()), message);
          needs_render = true;
        },
        LspEvent::DiagnosticsPublished { diagnostics } => {
          let diagnostic_uri = diagnostics.uri.clone();
          let active_uri = self.lsp_document.as_ref().map(|state| state.uri.as_str());
          let previous_counts = self
            .diagnostics
            .document(&diagnostic_uri)
            .map(|document| document.counts())
            .unwrap_or_default();
          let next_counts = self.diagnostics.apply_document(diagnostics);
          if active_uri.is_some_and(|uri| uri == diagnostic_uri) && previous_counts != next_counts {
            self.publish_lsp_diagnostic_message(next_counts);
            needs_render = true;
          }
        },
        _ => {},
      }
    }
    needs_render
  }

  fn handle_lsp_rpc_message(&mut self, message: jsonrpc::Message) -> bool {
    let jsonrpc::Message::Response(response) = message else {
      return false;
    };
    self.handle_lsp_response(response)
  }

  fn handle_lsp_response(&mut self, response: jsonrpc::Response) -> bool {
    let jsonrpc::Id::Number(id) = response.id else {
      return false;
    };
    let Some(kind) = self.lsp_pending_requests.remove(&id) else {
      return false;
    };

    if let Some(uri) = kind.uri() {
      let current_uri = self.lsp_document.as_ref().map(|state| state.uri.as_str());
      if current_uri != Some(uri) {
        return false;
      }
    }

    if let Some(error) = response.error {
      self.messages.publish(
        MessageLevel::Error,
        Some("lsp".into()),
        format!("lsp {} failed: {}", kind.label(), error.message),
      );
      return true;
    }

    match kind {
      PendingLspRequestKind::GotoDefinition { .. } => {
        let locations = match parse_locations_response(response.result.as_ref()) {
          Ok(locations) => locations,
          Err(err) => {
            self.messages.publish(
              MessageLevel::Error,
              Some("lsp".into()),
              format!("failed to parse goto-definition response: {err}"),
            );
            return true;
          },
        };
        self.apply_locations_result("definition", locations)
      },
      PendingLspRequestKind::Hover { .. } => {
        let hover = match parse_hover_response(response.result.as_ref()) {
          Ok(hover) => hover,
          Err(err) => {
            self.messages.publish(
              MessageLevel::Error,
              Some("lsp".into()),
              format!("failed to parse hover response: {err}"),
            );
            return true;
          },
        };
        match hover {
          Some(text) => {
            self
              .messages
              .publish(MessageLevel::Info, Some("lsp".into()), text);
          },
          None => {
            self.messages.publish(
              MessageLevel::Info,
              Some("lsp".into()),
              "no hover information",
            );
          },
        }
        true
      },
      PendingLspRequestKind::References { .. } => {
        let locations = match parse_locations_response(response.result.as_ref()) {
          Ok(locations) => locations,
          Err(err) => {
            self.messages.publish(
              MessageLevel::Error,
              Some("lsp".into()),
              format!("failed to parse references response: {err}"),
            );
            return true;
          },
        };
        self.apply_locations_result("references", locations)
      },
      PendingLspRequestKind::DocumentSymbols { uri } => {
        let symbols = match parse_document_symbols_response(&uri, response.result.as_ref()) {
          Ok(symbols) => symbols,
          Err(err) => {
            self.messages.publish(
              MessageLevel::Error,
              Some("lsp".into()),
              format!("failed to parse document-symbols response: {err}"),
            );
            return true;
          },
        };
        self.apply_symbols_result("document symbols", symbols)
      },
      PendingLspRequestKind::WorkspaceSymbols { query: _query } => {
        let symbols = match parse_workspace_symbols_response(response.result.as_ref()) {
          Ok(symbols) => symbols,
          Err(err) => {
            self.messages.publish(
              MessageLevel::Error,
              Some("lsp".into()),
              format!("failed to parse workspace-symbols response: {err}"),
            );
            return true;
          },
        };
        self.apply_symbols_result("workspace symbols", symbols)
      },
      PendingLspRequestKind::Completion { fallback_char, .. } => {
        self.handle_completion_response(response.result.as_ref(), fallback_char)
      },
      PendingLspRequestKind::SignatureHelp { .. } => {
        self.handle_signature_help_response(response.result.as_ref())
      },
      PendingLspRequestKind::CodeActions { .. } => {
        self.handle_code_actions_response(response.result.as_ref())
      },
      PendingLspRequestKind::Rename { .. } => self.handle_rename_response(response.result.as_ref()),
      PendingLspRequestKind::Format { .. } => self.handle_format_response(response.result.as_ref()),
    }
  }

  fn apply_locations_result(&mut self, label: &str, locations: Vec<LspLocation>) -> bool {
    if locations.is_empty() {
      self.messages.publish(
        MessageLevel::Info,
        Some("lsp".into()),
        format!("no {label} found"),
      );
      return true;
    }

    let jumped = self.jump_to_location(&locations[0]);
    if jumped {
      let total = locations.len();
      let text = if total == 1 {
        format!("{label}: 1 result")
      } else {
        format!("{label}: {total} results (jumped to first)")
      };
      self
        .messages
        .publish(MessageLevel::Info, Some("lsp".into()), text);
    }
    jumped
  }

  fn apply_symbols_result(&mut self, label: &str, symbols: Vec<LspSymbol>) -> bool {
    if symbols.is_empty() {
      self.messages.publish(
        MessageLevel::Info,
        Some("lsp".into()),
        format!("no {label} found"),
      );
      return true;
    }

    if let Some(location) = symbols.iter().find_map(|symbol| symbol.location.as_ref()) {
      let jumped = self.jump_to_location(location);
      if jumped {
        self.messages.publish(
          MessageLevel::Info,
          Some("lsp".into()),
          format!("{label}: {} results (jumped to first)", symbols.len()),
        );
      }
      return jumped;
    }

    self.messages.publish(
      MessageLevel::Info,
      Some("lsp".into()),
      format!("{label}: {} results", symbols.len()),
    );
    true
  }

  fn handle_completion_response(&mut self, result: Option<&Value>, fallback_char: usize) -> bool {
    let items = match parse_completion_response(result) {
      Ok(items) => items,
      Err(err) => {
        self.messages.publish(
          MessageLevel::Error,
          Some("lsp".into()),
          format!("failed to parse completion response: {err}"),
        );
        return true;
      },
    };

    let Some(item) = items.into_iter().next() else {
      self.messages.publish(
        MessageLevel::Info,
        Some("lsp".into()),
        "no completion candidates",
      );
      return true;
    };

    self.apply_completion_item(item, fallback_char)
  }

  fn apply_completion_item(&mut self, item: LspCompletionItem, fallback_char: usize) -> bool {
    let has_text_edits = item.primary_edit.is_some() || !item.additional_edits.is_empty();
    if has_text_edits {
      let Some(uri) = self.current_lsp_uri() else {
        self.messages.publish(
          MessageLevel::Warning,
          Some("lsp".into()),
          "completion unavailable: no active LSP document",
        );
        return true;
      };

      let mut edits = Vec::with_capacity(1 + item.additional_edits.len());
      if let Some(primary) = item.primary_edit {
        edits.push(primary);
      }
      edits.extend(item.additional_edits);
      let workspace_edit = LspWorkspaceEdit {
        documents: vec![the_lsp::LspDocumentEdit {
          uri,
          version: None,
          edits,
        }],
      };
      return self.apply_workspace_edit(&workspace_edit, "completion");
    }

    let insert_text = item.insert_text.unwrap_or(item.label);
    if insert_text.is_empty() {
      return true;
    }

    let cursor = fallback_char.min(self.editor.document().text().len_chars());
    let tx = match Transaction::change(self.editor.document().text(), vec![(
      cursor,
      cursor,
      Some(insert_text.into()),
    )]) {
      Ok(tx) => tx,
      Err(err) => {
        self.messages.publish(
          MessageLevel::Error,
          Some("lsp".into()),
          format!("failed to build completion transaction: {err}"),
        );
        return true;
      },
    };

    if <Self as the_default::DefaultContext>::apply_transaction(self, &tx) {
      <Self as the_default::DefaultContext>::request_render(self);
      self
        .messages
        .publish(MessageLevel::Info, Some("lsp".into()), "completion applied");
    } else {
      self.messages.publish(
        MessageLevel::Error,
        Some("lsp".into()),
        "failed to apply completion",
      );
    }
    true
  }

  fn handle_signature_help_response(&mut self, result: Option<&Value>) -> bool {
    let signature = match parse_signature_help_response(result) {
      Ok(signature) => signature,
      Err(err) => {
        self.messages.publish(
          MessageLevel::Error,
          Some("lsp".into()),
          format!("failed to parse signature help response: {err}"),
        );
        return true;
      },
    };

    let Some(signature) = signature else {
      self.messages.publish(
        MessageLevel::Info,
        Some("lsp".into()),
        "no signature help available",
      );
      return true;
    };

    let mut text = signature.label;
    if text.len() > 240 {
      text.truncate(240);
      text.push('…');
    }
    if let Some(active_parameter) = signature.active_parameter {
      text.push_str(&format!("  (param {})", active_parameter + 1));
    }
    self
      .messages
      .publish(MessageLevel::Info, Some("lsp".into()), text);
    true
  }

  fn handle_code_actions_response(&mut self, result: Option<&Value>) -> bool {
    let mut actions = match parse_code_actions_response(result) {
      Ok(actions) => actions,
      Err(err) => {
        self.messages.publish(
          MessageLevel::Error,
          Some("lsp".into()),
          format!("failed to parse code actions response: {err}"),
        );
        return true;
      },
    };

    if actions.is_empty() {
      self.messages.publish(
        MessageLevel::Info,
        Some("lsp".into()),
        "no code actions available",
      );
      return true;
    }

    actions.sort_by_key(|action| !action.is_preferred);
    let action = actions.remove(0);

    if let Some(edit) = action.edit.as_ref() {
      let _ = self.apply_workspace_edit(edit, "code action");
      self.messages.publish(
        MessageLevel::Info,
        Some("lsp".into()),
        format!("code action: {}", action.title),
      );
      return true;
    }

    if let Some(command) = action.command {
      return self.execute_lsp_command_action(command, action.title);
    }

    self.messages.publish(
      MessageLevel::Info,
      Some("lsp".into()),
      format!("code action '{}' had no edits", action.title),
    );
    true
  }

  fn handle_rename_response(&mut self, result: Option<&Value>) -> bool {
    let workspace_edit = match parse_workspace_edit_response(result) {
      Ok(edit) => edit,
      Err(err) => {
        self.messages.publish(
          MessageLevel::Error,
          Some("lsp".into()),
          format!("failed to parse rename response: {err}"),
        );
        return true;
      },
    };

    let Some(workspace_edit) = workspace_edit else {
      self.messages.publish(
        MessageLevel::Info,
        Some("lsp".into()),
        "rename produced no edits",
      );
      return true;
    };

    self.apply_workspace_edit(&workspace_edit, "rename")
  }

  fn handle_format_response(&mut self, result: Option<&Value>) -> bool {
    let edits = match parse_formatting_response(result) {
      Ok(edits) => edits,
      Err(err) => {
        self.messages.publish(
          MessageLevel::Error,
          Some("lsp".into()),
          format!("failed to parse formatting response: {err}"),
        );
        return true;
      },
    };

    if edits.is_empty() {
      self
        .messages
        .publish(MessageLevel::Info, Some("lsp".into()), "already formatted");
      return true;
    }

    let Some(uri) = self.current_lsp_uri() else {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "format unavailable: no active LSP document",
      );
      return true;
    };

    let workspace_edit = LspWorkspaceEdit {
      documents: vec![the_lsp::LspDocumentEdit {
        uri,
        version: None,
        edits,
      }],
    };
    self.apply_workspace_edit(&workspace_edit, "format")
  }

  fn execute_lsp_command_action(&mut self, command: LspExecuteCommand, title: String) -> bool {
    let params = execute_command_params(&command.command, command.arguments);
    match self
      .lsp_runtime
      .send_request("workspace/executeCommand", Some(params))
    {
      Ok(_) => {
        self.messages.publish(
          MessageLevel::Info,
          Some("lsp".into()),
          format!("executed code action: {title}"),
        );
      },
      Err(err) => {
        self.messages.publish(
          MessageLevel::Error,
          Some("lsp".into()),
          format!("failed to execute code action '{title}': {err}"),
        );
      },
    }
    true
  }

  fn apply_workspace_edit(&mut self, workspace_edit: &LspWorkspaceEdit, source: &str) -> bool {
    if workspace_edit.documents.is_empty() {
      self.messages.publish(
        MessageLevel::Info,
        Some("lsp".into()),
        format!("{source}: no edits"),
      );
      return true;
    }

    let current_uri = self.current_lsp_uri();
    let mut applied_documents = 0usize;
    let mut applied_edits = 0usize;

    for document in &workspace_edit.documents {
      if document.edits.is_empty() {
        continue;
      }
      let applied = if current_uri.as_ref() == Some(&document.uri) {
        self.apply_text_edits_to_current_document(&document.edits)
      } else {
        self.apply_text_edits_to_file_uri(&document.uri, &document.edits)
      };
      if applied {
        applied_documents = applied_documents.saturating_add(1);
        applied_edits = applied_edits.saturating_add(document.edits.len());
      }
    }

    if applied_documents > 0 {
      self.messages.publish(
        MessageLevel::Info,
        Some("lsp".into()),
        format!("{source}: applied {applied_edits} edit(s) across {applied_documents} file(s)"),
      );
    } else {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        format!("{source}: no edits were applied"),
      );
    }
    true
  }

  fn apply_text_edits_to_current_document(&mut self, edits: &[LspTextEdit]) -> bool {
    let tx = match build_transaction_from_lsp_text_edits(self.editor.document().text(), edits) {
      Ok(tx) => tx,
      Err(err) => {
        self.messages.publish(
          MessageLevel::Error,
          Some("lsp".into()),
          format!("failed to build edit transaction: {err}"),
        );
        return false;
      },
    };

    if <Self as the_default::DefaultContext>::apply_transaction(self, &tx) {
      <Self as the_default::DefaultContext>::request_render(self);
      true
    } else {
      self.messages.publish(
        MessageLevel::Error,
        Some("lsp".into()),
        "failed to apply edit transaction",
      );
      false
    }
  }

  fn apply_text_edits_to_file_uri(&mut self, uri: &str, edits: &[LspTextEdit]) -> bool {
    let Some(path) = path_for_file_uri(uri) else {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        format!("unsupported file URI in workspace edit: {uri}"),
      );
      return false;
    };

    let content = match std::fs::read_to_string(&path) {
      Ok(content) => content,
      Err(err) => {
        self.messages.publish(
          MessageLevel::Error,
          Some("lsp".into()),
          format!("failed to read '{}': {err}", path.display()),
        );
        return false;
      },
    };
    let mut rope = Rope::from(content);

    let tx = match build_transaction_from_lsp_text_edits(&rope, edits) {
      Ok(tx) => tx,
      Err(err) => {
        self.messages.publish(
          MessageLevel::Error,
          Some("lsp".into()),
          format!("failed to build workspace edit transaction: {err}"),
        );
        return false;
      },
    };

    if let Err(err) = tx.apply(&mut rope) {
      self.messages.publish(
        MessageLevel::Error,
        Some("lsp".into()),
        format!("failed to apply edits to '{}': {err}", path.display()),
      );
      return false;
    }

    if let Err(err) = std::fs::write(&path, rope.to_string()) {
      self.messages.publish(
        MessageLevel::Error,
        Some("lsp".into()),
        format!("failed to write '{}': {err}", path.display()),
      );
      return false;
    }
    true
  }

  fn jump_to_location(&mut self, location: &LspLocation) -> bool {
    let Some(path) = path_for_file_uri(&location.uri) else {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        format!("unsupported location URI: {}", location.uri),
      );
      return true;
    };

    if self
      .file_path
      .as_ref()
      .is_none_or(|current| current != &path)
      && let Err(err) = <Self as the_default::DefaultContext>::open_file(self, &path)
    {
      self.messages.publish(
        MessageLevel::Error,
        Some("lsp".into()),
        format!("failed to open location '{}': {err}", path.display()),
      );
      return true;
    }

    let cursor = {
      let doc = self.editor.document();
      utf16_position_to_char_idx(
        doc.text(),
        location.range.start.line,
        location.range.start.character,
      )
    };

    let _ = self
      .editor
      .document_mut()
      .set_selection(Selection::point(cursor));
    self.editor.view_mut().scroll = Position::new(
      (location.range.start.line as usize).saturating_sub(self.scrolloff),
      0,
    );
    <Self as the_default::DefaultContext>::request_render(self);
    true
  }

  fn lsp_supports(&self, capability: LspCapability) -> bool {
    let Some(server) = self.lsp_runtime.config().server() else {
      return false;
    };
    self
      .lsp_runtime
      .server_capabilities(server.name())
      .is_some_and(|capabilities| capabilities.supports(capability))
  }

  fn current_lsp_position(&self) -> Option<(String, LspPosition)> {
    if !self.lsp_ready {
      return None;
    }
    let state = self.lsp_document.as_ref()?.clone();
    if !state.opened {
      return None;
    }

    let doc = self.editor.document();
    let range = doc.selection().ranges().first().copied()?;
    let cursor = range.cursor(doc.text().slice(..));
    let (line, character) = char_idx_to_utf16_position(doc.text(), cursor);

    Some((state.uri, LspPosition { line, character }))
  }

  fn current_lsp_range(&self) -> Option<(String, the_lsp::LspRange)> {
    if !self.lsp_ready {
      return None;
    }
    let state = self.lsp_document.as_ref()?.clone();
    if !state.opened {
      return None;
    }

    let doc = self.editor.document();
    let range = doc.selection().ranges().first().copied()?;
    let start = range.anchor.min(range.head);
    let end = range.anchor.max(range.head);
    let (start_line, start_character) = char_idx_to_utf16_position(doc.text(), start);
    let (end_line, end_character) = char_idx_to_utf16_position(doc.text(), end);

    Some((state.uri, the_lsp::LspRange {
      start: LspPosition {
        line:      start_line,
        character: start_character,
      },
      end:   LspPosition {
        line:      end_line,
        character: end_character,
      },
    }))
  }

  fn current_lsp_uri(&self) -> Option<String> {
    if !self.lsp_ready {
      return None;
    }
    self
      .lsp_document
      .as_ref()
      .filter(|state| state.opened)
      .map(|state| state.uri.clone())
  }

  fn current_lsp_diagnostics_payload(&self, uri: &str) -> Value {
    let Some(document_diagnostics) = self.diagnostics.document(uri) else {
      return json!([]);
    };

    Value::Array(
      document_diagnostics
        .diagnostics
        .iter()
        .map(diagnostic_to_lsp_json)
        .collect(),
    )
  }

  fn dispatch_lsp_request(
    &mut self,
    method: &'static str,
    params: Value,
    pending: PendingLspRequestKind,
  ) {
    match self.lsp_runtime.send_request(method, Some(params)) {
      Ok(request_id) => {
        self.lsp_pending_requests.insert(request_id, pending);
      },
      Err(err) => {
        self.messages.publish(
          MessageLevel::Error,
          Some("lsp".into()),
          format!("failed to dispatch {method}: {err}"),
        );
      },
    }
  }

  fn workspace_symbol_query_from_cursor(&self) -> String {
    let doc = self.editor.document();
    let text = doc.text();
    let Some(range) = doc.selection().ranges().first().copied() else {
      return String::new();
    };
    let cursor = range.cursor(text.slice(..));
    let line_idx = text.char_to_line(cursor);
    let line_start = text.line_to_char(line_idx);
    let line_end = if line_idx + 1 < text.len_lines() {
      text.line_to_char(line_idx + 1)
    } else {
      text.len_chars()
    };

    let line: Vec<char> = text.slice(line_start..line_end).chars().collect();
    let local_cursor = cursor.saturating_sub(line_start);
    let mut start = local_cursor.min(line.len());
    while start > 0 && is_symbol_word_char(line[start - 1]) {
      start -= 1;
    }
    let mut end = local_cursor.min(line.len());
    while end < line.len() && is_symbol_word_char(line[end]) {
      end += 1;
    }

    line[start..end].iter().collect()
  }

  fn lsp_sync_kind(&self) -> Option<TextDocumentSyncKind> {
    let server = self.lsp_runtime.config().server()?;
    self
      .lsp_runtime
      .server_capabilities(server.name())
      .map(|capabilities| capabilities.text_document_sync().kind)
  }

  fn lsp_save_include_text(&self) -> bool {
    let Some(server) = self.lsp_runtime.config().server() else {
      return false;
    };
    self
      .lsp_runtime
      .server_capabilities(server.name())
      .is_some_and(|capabilities| capabilities.text_document_sync().save_include_text)
  }

  fn lsp_open_current_document(&mut self) {
    if !self.lsp_ready {
      return;
    }

    let Some(state) = self.lsp_document.as_ref() else {
      return;
    };
    if state.opened {
      return;
    }

    let uri = state.uri.clone();
    let language_id = state.language_id.clone();
    let version = state.version;
    let text = self.editor.document().text().clone();
    let params = did_open_params(&uri, &language_id, version, &text);

    if self
      .lsp_runtime
      .send_notification("textDocument/didOpen", Some(params))
      .is_ok()
      && let Some(state) = self.lsp_document.as_mut()
    {
      state.opened = true;
    }
  }

  fn lsp_close_current_document(&mut self) {
    let Some(uri) = self
      .lsp_document
      .as_ref()
      .filter(|state| state.opened)
      .map(|state| state.uri.clone())
    else {
      return;
    };

    let params = did_close_params(&uri);
    let _ = self
      .lsp_runtime
      .send_notification("textDocument/didClose", Some(params));
    if let Some(state) = self.lsp_document.as_mut() {
      state.opened = false;
    }
  }

  fn lsp_send_did_change(&mut self, old_text: &Rope, changes: &ChangeSet) {
    if !self.lsp_ready {
      return;
    }

    let Some(sync_kind) = self.lsp_sync_kind() else {
      return;
    };

    let Some((uri, current_version)) = self
      .lsp_document
      .as_ref()
      .filter(|state| state.opened)
      .map(|state| (state.uri.clone(), state.version))
    else {
      return;
    };

    let next_version = current_version.saturating_add(1);
    let new_text = self.editor.document().text().clone();
    let Some(params) =
      did_change_params(&uri, next_version, old_text, &new_text, changes, sync_kind)
    else {
      return;
    };

    if self
      .lsp_runtime
      .send_notification("textDocument/didChange", Some(params))
      .is_ok()
      && let Some(state) = self.lsp_document.as_mut()
    {
      state.version = next_version;
    }
  }

  fn lsp_send_did_save(&mut self, text: Option<&str>) {
    if !self.lsp_ready {
      return;
    }

    let Some(uri) = self
      .lsp_document
      .as_ref()
      .filter(|state| state.opened)
      .map(|state| state.uri.clone())
    else {
      return;
    };

    let payload_text = if self.lsp_save_include_text() {
      text
    } else {
      None
    };
    let params = did_save_params(&uri, payload_text);
    let _ = self
      .lsp_runtime
      .send_notification("textDocument/didSave", Some(params));
  }

  fn lsp_refresh_document_state(&mut self, path: Option<&Path>) {
    self.lsp_document =
      path.and_then(|path| build_lsp_document_state(path, self.loader.as_deref()));
  }

  fn publish_lsp_diagnostic_message(&mut self, counts: DiagnosticCounts) {
    let text = if counts.total == 0 {
      "diagnostics cleared".to_string()
    } else {
      format!(
        "diagnostics: {} error(s), {} warning(s), {} info, {} hint(s)",
        counts.errors, counts.warnings, counts.information, counts.hints
      )
    };
    let level = if counts.errors > 0 {
      MessageLevel::Error
    } else if counts.warnings > 0 {
      MessageLevel::Warning
    } else {
      MessageLevel::Info
    };
    self.messages.publish(level, Some("lsp".into()), text);
  }
}

fn is_symbol_word_char(ch: char) -> bool {
  ch == '_' || ch.is_alphanumeric()
}

fn diagnostic_severity_to_lsp_code(severity: DiagnosticSeverity) -> u8 {
  match severity {
    DiagnosticSeverity::Error => 1,
    DiagnosticSeverity::Warning => 2,
    DiagnosticSeverity::Information => 3,
    DiagnosticSeverity::Hint => 4,
  }
}

fn diagnostic_to_lsp_json(diagnostic: &Diagnostic) -> Value {
  let mut value = json!({
    "range": {
      "start": {
        "line": diagnostic.range.start.line,
        "character": diagnostic.range.start.character,
      },
      "end": {
        "line": diagnostic.range.end.line,
        "character": diagnostic.range.end.character,
      },
    },
    "message": diagnostic.message,
  });

  if let Some(object) = value.as_object_mut() {
    if let Some(severity) = diagnostic.severity {
      object.insert(
        "severity".into(),
        json!(diagnostic_severity_to_lsp_code(severity)),
      );
    }
    if let Some(code) = &diagnostic.code {
      object.insert("code".into(), json!(code));
    }
    if let Some(source) = &diagnostic.source {
      object.insert("source".into(), json!(source));
    }
  }

  value
}

fn build_transaction_from_lsp_text_edits(
  text: &Rope,
  edits: &[LspTextEdit],
) -> std::result::Result<Transaction, String> {
  let mut changes = Vec::with_capacity(edits.len());
  for edit in edits {
    let from = utf16_position_to_char_idx(text, edit.range.start.line, edit.range.start.character);
    let to = utf16_position_to_char_idx(text, edit.range.end.line, edit.range.end.character);
    changes.push((from, to, Some(edit.new_text.clone().into())));
  }
  changes.sort_by_key(|(from, to, _)| (*from, *to));
  Transaction::change(text, changes).map_err(|err| err.to_string())
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
    let old_text_for_lsp = self.editor.document().text().clone();
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

    self.lsp_send_did_change(&old_text_for_lsp, transaction.changes());

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
    self.lsp_refresh_document_state(path.as_deref());
    self.file_path = path;
  }

  fn lsp_goto_definition(&mut self) {
    if !self.lsp_supports(LspCapability::GotoDefinition) {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "goto-definition is not supported by the active server",
      );
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "goto-definition unavailable: no active LSP document",
      );
      return;
    };

    self.dispatch_lsp_request(
      "textDocument/definition",
      goto_definition_params(&uri, position),
      PendingLspRequestKind::GotoDefinition { uri },
    );
  }

  fn lsp_hover(&mut self) {
    if !self.lsp_supports(LspCapability::Hover) {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "hover is not supported by the active server",
      );
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "hover unavailable: no active LSP document",
      );
      return;
    };

    self.dispatch_lsp_request(
      "textDocument/hover",
      hover_params(&uri, position),
      PendingLspRequestKind::Hover { uri },
    );
  }

  fn lsp_references(&mut self) {
    if !self.lsp_supports(LspCapability::GotoReference) {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "references are not supported by the active server",
      );
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "references unavailable: no active LSP document",
      );
      return;
    };

    self.dispatch_lsp_request(
      "textDocument/references",
      references_params(&uri, position, false),
      PendingLspRequestKind::References { uri },
    );
  }

  fn lsp_document_symbols(&mut self) {
    if !self.lsp_supports(LspCapability::DocumentSymbols) {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "document symbols are not supported by the active server",
      );
      return;
    }

    let Some(uri) = self
      .lsp_document
      .as_ref()
      .filter(|state| state.opened && self.lsp_ready)
      .map(|state| state.uri.clone())
    else {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "document symbols unavailable: no active LSP document",
      );
      return;
    };

    self.dispatch_lsp_request(
      "textDocument/documentSymbol",
      document_symbols_params(&uri),
      PendingLspRequestKind::DocumentSymbols { uri },
    );
  }

  fn lsp_workspace_symbols(&mut self) {
    if !self.lsp_supports(LspCapability::WorkspaceSymbols) {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "workspace symbols are not supported by the active server",
      );
      return;
    }

    let query = self.workspace_symbol_query_from_cursor();
    self.dispatch_lsp_request(
      "workspace/symbol",
      workspace_symbols_params(&query),
      PendingLspRequestKind::WorkspaceSymbols { query },
    );
  }

  fn lsp_completion(&mut self) {
    if !self.lsp_supports(LspCapability::Completion) {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "completion is not supported by the active server",
      );
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "completion unavailable: no active LSP document",
      );
      return;
    };

    let fallback_char = self
      .editor
      .document()
      .selection()
      .ranges()
      .first()
      .map(|range| range.cursor(self.editor.document().text().slice(..)))
      .unwrap_or(0);

    self.dispatch_lsp_request(
      "textDocument/completion",
      completion_params(&uri, position),
      PendingLspRequestKind::Completion { uri, fallback_char },
    );
  }

  fn lsp_signature_help(&mut self) {
    if !self.lsp_supports(LspCapability::SignatureHelp) {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "signature help is not supported by the active server",
      );
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "signature help unavailable: no active LSP document",
      );
      return;
    };

    self.dispatch_lsp_request(
      "textDocument/signatureHelp",
      signature_help_params(&uri, position),
      PendingLspRequestKind::SignatureHelp { uri },
    );
  }

  fn lsp_code_actions(&mut self) {
    if !self.lsp_supports(LspCapability::CodeAction) {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "code actions are not supported by the active server",
      );
      return;
    }

    let Some((uri, range)) = self.current_lsp_range() else {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "code actions unavailable: no active LSP document",
      );
      return;
    };

    let diagnostics = self.current_lsp_diagnostics_payload(&uri);
    self.dispatch_lsp_request(
      "textDocument/codeAction",
      code_action_params(&uri, range, diagnostics, None),
      PendingLspRequestKind::CodeActions { uri },
    );
  }

  fn lsp_rename(&mut self, new_name: &str) {
    if !self.lsp_supports(LspCapability::RenameSymbol) {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "rename is not supported by the active server",
      );
      return;
    }

    let new_name = new_name.trim();
    if new_name.is_empty() {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "rename requires a non-empty name",
      );
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "rename unavailable: no active LSP document",
      );
      return;
    };

    self.dispatch_lsp_request(
      "textDocument/rename",
      rename_params(&uri, position, new_name),
      PendingLspRequestKind::Rename { uri },
    );
  }

  fn lsp_format(&mut self) {
    if !self.lsp_supports(LspCapability::Format) {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "format is not supported by the active server",
      );
      return;
    }

    let Some(uri) = self.current_lsp_uri() else {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "format unavailable: no active LSP document",
      );
      return;
    };

    let (tab_size, insert_spaces) = match self.editor.document().indent_style() {
      IndentStyle::Tabs => (4, false),
      IndentStyle::Spaces(width) => (width as u32, true),
    };

    self.dispatch_lsp_request(
      "textDocument/formatting",
      formatting_params(&uri, tab_size, insert_spaces),
      PendingLspRequestKind::Format { uri },
    );
  }

  fn on_file_saved(&mut self, _path: &Path, text: &str) {
    self.lsp_send_did_save(Some(text));
  }

  fn on_before_quit(&mut self) {
    self.lsp_close_current_document();
  }

  fn open_file(&mut self, path: &Path) -> std::io::Result<()> {
    self.lsp_close_current_document();
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
    self.lsp_refresh_document_state(Some(path));
    self.lsp_open_current_document();
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
