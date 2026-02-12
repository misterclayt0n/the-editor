//! Application context (state).

use std::{
  collections::{
    BTreeMap,
    HashMap,
    HashSet,
    VecDeque,
  },
  env,
  fs::OpenOptions,
  io::{
    BufWriter,
    Write,
  },
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
  time::{
    Duration,
    Instant,
    SystemTime,
  },
};

use eyre::Result;
use ropey::Rope;
use serde_json::{
  Value,
  json,
};
use the_default::{
  Command,
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
    GutterConfig,
    RenderGutterDiffKind,
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
  syntax_async::{
    ParseHighlightState,
    ParseLifecycle,
    ParseRequest,
    QueueParseDecision,
  },
  transaction::{
    ChangeSet,
    Transaction,
  },
  view::ViewState,
};
use the_lsp::{
  FileChangeType,
  LspCapability,
  LspCompletionContext,
  LspCompletionItem,
  LspCompletionItemKind,
  LspEvent,
  LspExecuteCommand,
  LspInsertTextFormat,
  LspLocation,
  LspPosition,
  LspProgressKind,
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
  parse_completion_item_response,
  parse_completion_response_with_raw,
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
    did_change_watched_files_params,
    did_close_params,
    did_open_params,
    did_save_params,
    file_uri_for_path,
    path_for_file_uri,
    utf16_position_to_char_idx,
  },
  workspace_symbols_params,
};
use the_runtime::{
  clipboard::ClipboardProvider,
  file_watch::{
    PathEventKind,
    WatchHandle,
    resolve_trace_log_path as resolve_file_watch_trace_log_path,
    trace_event as trace_file_watch_event,
    watch as watch_path,
  },
  file_watch_consumer::{
    WatchPollOutcome,
    WatchedFileEventsState,
    poll_watch_events,
  },
  file_watch_reload::{
    FileWatchReloadDecision,
    FileWatchReloadError,
    FileWatchReloadIoState,
    FileWatchReloadState,
    clear_reload_state,
    evaluate_external_reload_from_disk,
    mark_reload_applied,
  },
};
use the_vcs::{
  DiffHandle,
  DiffProviderRegistry,
  DiffSignKind,
};

use crate::picker_layout::FilePickerLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerDragState {
  ListScrollbar { grab_offset: u16 },
  PreviewScrollbar { grab_offset: u16 },
}

#[derive(Debug)]
pub struct SyntaxParseResult {
  pub request_id:  u64,
  pub doc_version: u64,
  pub syntax:      Option<Syntax>,
}

type SyntaxParseJob = Box<dyn FnOnce() -> Option<Syntax> + Send>;

fn spawn_syntax_parse_request(
  tx: Sender<SyntaxParseResult>,
  request: ParseRequest<SyntaxParseJob>,
) {
  thread::spawn(move || {
    let parsed = (request.payload)();
    let _ = tx.send(SyntaxParseResult {
      request_id:  request.meta.request_id,
      doc_version: request.meta.doc_version,
      syntax:      parsed,
    });
  });
}

#[derive(Debug, Clone)]
pub struct LspDocumentSyncState {
  pub path:        PathBuf,
  pub uri:         String,
  pub language_id: String,
  pub version:     i32,
  pub opened:      bool,
}

struct LspWatchedFileState {
  stream:        WatchedFileEventsState,
  _watch_handle: WatchHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LspStatusPhase {
  Off,
  Starting,
  Initializing,
  Ready,
  Busy,
  Error,
}

#[derive(Debug, Clone)]
struct LspStatuslineState {
  phase:  LspStatusPhase,
  detail: Option<String>,
}

impl LspStatuslineState {
  fn off(detail: Option<String>) -> Self {
    Self {
      phase: LspStatusPhase::Off,
      detail,
    }
  }

  fn is_loading(&self) -> bool {
    matches!(
      self.phase,
      LspStatusPhase::Starting | LspStatusPhase::Initializing | LspStatusPhase::Busy
    )
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionTriggerSource {
  Manual,
  Invoked,
  TriggerCharacter(char),
  Incomplete,
}

impl CompletionTriggerSource {
  fn to_lsp_context(self) -> LspCompletionContext {
    match self {
      Self::Manual | Self::Invoked => LspCompletionContext::invoked(),
      Self::TriggerCharacter(ch) => LspCompletionContext::trigger_character(ch),
      Self::Incomplete => LspCompletionContext::trigger_for_incomplete(),
    }
  }
}

#[derive(Debug, Clone)]
struct PendingAutoCompletion {
  due_at:  Instant,
  trigger: CompletionTriggerSource,
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
    uri:            String,
    generation:     u64,
    cursor:         usize,
    replace_start:  usize,
    announce_empty: bool,
  },
  CompletionResolve {
    uri:   String,
    index: usize,
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
      Self::CompletionResolve { .. } => "completion-resolve",
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
      | Self::CompletionResolve { uri, .. }
      | Self::SignatureHelp { uri }
      | Self::CodeActions { uri }
      | Self::Rename { uri }
      | Self::Format { uri } => Some(uri.as_str()),
      Self::WorkspaceSymbols { .. } => None,
    }
  }

  fn cancellation_key(&self) -> (&'static str, Option<&str>) {
    match self {
      Self::GotoDefinition { uri } => ("goto-definition", Some(uri)),
      Self::Hover { uri } => ("hover", Some(uri)),
      Self::References { uri } => ("references", Some(uri)),
      Self::DocumentSymbols { uri } => ("document-symbols", Some(uri)),
      Self::WorkspaceSymbols { .. } => ("workspace-symbols", None),
      Self::Completion { uri, .. } => ("completion", Some(uri)),
      Self::CompletionResolve { uri, .. } => ("completion-resolve", Some(uri)),
      Self::SignatureHelp { uri } => ("signature-help", Some(uri)),
      Self::CodeActions { uri } => ("code-actions", Some(uri)),
      Self::Rename { uri } => ("rename", Some(uri)),
      Self::Format { uri } => ("format", Some(uri)),
    }
  }
}

/// Application state passed to all handlers.
pub struct Ctx {
  pub editor:                       Editor,
  pub file_path:                    Option<PathBuf>,
  pub should_quit:                  bool,
  pub needs_render:                 bool,
  pub messages:                     MessageCenter,
  message_log:                      Option<BufWriter<std::fs::File>>,
  message_log_seq:                  u64,
  lsp_trace_log:                    Option<BufWriter<std::fs::File>>,
  pub file_picker_wake_rx:          Receiver<()>,
  pub mode:                         Mode,
  pub keymaps:                      Keymaps,
  pub command_prompt:               CommandPromptState,
  pub command_registry:             CommandRegistry<Ctx>,
  pub command_palette:              CommandPaletteState,
  pub command_palette_style:        CommandPaletteStyle,
  pub completion_menu:              the_default::CompletionMenuState,
  pub file_picker:                  FilePickerState,
  pub lsp_runtime:                  LspRuntime,
  pub lsp_ready:                    bool,
  pub lsp_document:                 Option<LspDocumentSyncState>,
  lsp_statusline:                   LspStatuslineState,
  lsp_spinner_index:                usize,
  lsp_spinner_last_tick:            Instant,
  lsp_active_progress_tokens:       HashSet<String>,
  lsp_watched_file:                 Option<LspWatchedFileState>,
  lsp_pending_requests:             HashMap<u64, PendingLspRequestKind>,
  lsp_completion_items:             Vec<LspCompletionItem>,
  lsp_completion_raw_items:         Vec<Value>,
  lsp_completion_resolved_indices:  HashSet<usize>,
  lsp_completion_visible_indices:   Vec<usize>,
  lsp_completion_fallback_start:    Option<usize>,
  lsp_completion_generation:        u64,
  lsp_pending_auto_completion:      Option<PendingAutoCompletion>,
  pub diagnostics:                  DiagnosticsState,
  pub file_picker_layout:           Option<FilePickerLayout>,
  pub file_picker_drag:             Option<FilePickerDragState>,
  pub search_prompt:                the_default::SearchPromptState,
  pub ui_theme:                     Theme,
  pub ui_state:                     UiState,
  pub pending_input:                Option<the_default::PendingInput>,
  pub dispatch:                     Option<NonNull<DefaultDispatchStatic<Ctx>>>,
  /// Syntax loader for language detection and highlighting.
  pub loader:                       Option<Arc<Loader>>,
  /// Cache for syntax highlights (reused across renders).
  pub highlight_cache:              HighlightCache,
  /// Background parse result channel (async syntax fallback).
  pub syntax_parse_tx:              Sender<SyntaxParseResult>,
  /// Background parse result receiver (async syntax fallback).
  pub syntax_parse_rx:              Receiver<SyntaxParseResult>,
  /// Async parse lifecycle (single in-flight + one queued replacement).
  pub syntax_parse_lifecycle:       ParseLifecycle<SyntaxParseJob>,
  /// Syntax parse/highlight gate state (parsed vs interpolated).
  pub syntax_parse_highlight_state: ParseHighlightState,
  /// Registers for yanking/pasting.
  pub registers:                    Registers,
  /// Active register target (for macros/register operations).
  pub register:                     Option<char>,
  /// Macro recording state.
  pub macro_recording:              Option<(char, Vec<KeyBinding>)>,
  /// Macro replay stack for recursion guard.
  pub macro_replaying:              Vec<char>,
  /// Pending macro key events to replay.
  pub macro_queue:                  VecDeque<KeyEvent>,
  /// Last executed motion for repeat.
  pub last_motion:                  Option<Motion>,
  /// Render formatting used for visual position mapping.
  pub text_format:                  TextFormat,
  /// Gutter layout and line-number rendering config.
  pub gutter_config:                GutterConfig,
  /// VCS-like gutter signs keyed by document line.
  pub gutter_diff_signs:            BTreeMap<usize, RenderGutterDiffKind>,
  /// Active VCS provider registry for diff base resolution.
  pub vcs_provider:                 DiffProviderRegistry,
  /// Cached VCS statusline text for the active file.
  pub vcs_statusline:               Option<String>,
  /// Incremental VCS diff state for the active file.
  pub vcs_diff:                     Option<DiffHandle>,
  /// Inline annotations (virtual text) for rendering.
  pub inline_annotations:           Vec<InlineAnnotation>,
  /// Overlay annotations (virtual text) for rendering.
  pub overlay_annotations:          Vec<Overlay>,
  /// Lines to keep above/below cursor when scrolling.
  pub scrolloff:                    usize,
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

fn resolve_message_log_path() -> Option<PathBuf> {
  match env::var("THE_EDITOR_MESSAGE_LOG") {
    Ok(path) => {
      let path = path.trim();
      if path.is_empty() || path.eq_ignore_ascii_case("off") || path.eq_ignore_ascii_case("none") {
        None
      } else {
        Some(PathBuf::from(path))
      }
    },
    Err(_) => Some(PathBuf::from("/tmp/the-editor-messages.log")),
  }
}

fn open_message_log() -> Option<BufWriter<std::fs::File>> {
  let path = resolve_message_log_path()?;
  if let Some(parent) = path.parent()
    && let Err(err) = std::fs::create_dir_all(parent)
  {
    eprintln!(
      "Warning: failed to create message log directory '{}': {err}",
      parent.display()
    );
    return None;
  }

  match OpenOptions::new().create(true).append(true).open(&path) {
    Ok(file) => Some(BufWriter::new(file)),
    Err(err) => {
      eprintln!(
        "Warning: failed to open message log file '{}': {err}",
        path.display()
      );
      None
    },
  }
}

fn resolve_lsp_trace_log_path() -> Option<PathBuf> {
  match env::var("THE_EDITOR_LSP_TRACE_LOG") {
    Ok(path) => {
      let path = path.trim();
      if path.is_empty() || path.eq_ignore_ascii_case("off") || path.eq_ignore_ascii_case("none") {
        None
      } else {
        Some(PathBuf::from(path))
      }
    },
    Err(_) => Some(PathBuf::from("/tmp/the-editor-lsp-trace.log")),
  }
}

fn open_lsp_trace_log() -> Option<BufWriter<std::fs::File>> {
  let path = resolve_lsp_trace_log_path()?;
  if let Some(parent) = path.parent()
    && let Err(err) = std::fs::create_dir_all(parent)
  {
    eprintln!(
      "Warning: failed to create lsp trace directory '{}': {err}",
      parent.display()
    );
    return None;
  }

  match OpenOptions::new().create(true).append(true).open(&path) {
    Ok(file) => Some(BufWriter::new(file)),
    Err(err) => {
      eprintln!(
        "Warning: failed to open lsp trace log '{}': {err}",
        path.display()
      );
      None
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

fn resolve_lsp_server(loader: Option<&Loader>, path: Option<&Path>) -> Option<LspServerConfig> {
  let server_from_language =
    path.and_then(|path| loader.and_then(|loader| lsp_server_from_language_config(loader, path)));
  server_from_language.or_else(lsp_server_from_env)
}

fn lsp_server_configs_equal(lhs: Option<&LspServerConfig>, rhs: Option<&LspServerConfig>) -> bool {
  match (lhs, rhs) {
    (None, None) => true,
    (Some(lhs), Some(rhs)) => {
      lhs.name() == rhs.name()
        && lhs.command() == rhs.command()
        && lhs.args() == rhs.args()
        && lhs.env() == rhs.env()
        && lhs.initialize_options() == rhs.initialize_options()
        && lhs.initialize_timeout() == rhs.initialize_timeout()
    },
    _ => false,
  }
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
    let message_log = open_message_log();
    let lsp_trace_log = open_lsp_trace_log();

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
    let mut lsp_runtime_config = LspRuntimeConfig::new(workspace_root)
      .with_restart_policy(true, Duration::from_millis(250))
      .with_restart_limits(6, Duration::from_secs(30))
      .with_request_policy(Duration::from_secs(8), 1);
    if let Some(server) = resolve_lsp_server(loader.as_deref(), file_path.map(Path::new)) {
      lsp_runtime_config = lsp_runtime_config.with_server(server);
    }
    let lsp_server_configured = lsp_runtime_config.server().is_some();
    let lsp_runtime = LspRuntime::new(lsp_runtime_config);
    let lsp_document = file_path
      .map(PathBuf::from)
      .as_deref()
      .and_then(|path| build_lsp_document_state(path, loader.as_deref()));

    let mut ctx = Self {
      editor,
      file_path: file_path.map(PathBuf::from),
      should_quit: false,
      needs_render: true,
      messages: MessageCenter::default(),
      message_log,
      message_log_seq: 0,
      lsp_trace_log,
      file_picker_wake_rx,
      mode: Mode::Normal,
      keymaps: Keymaps::default(),
      command_prompt: CommandPromptState::new(),
      command_registry: CommandRegistry::new(),
      command_palette: CommandPaletteState::default(),
      command_palette_style: CommandPaletteStyle::helix_bottom(),
      completion_menu: the_default::CompletionMenuState::default(),
      file_picker,
      lsp_runtime,
      lsp_ready: false,
      lsp_document,
      lsp_statusline: if lsp_server_configured {
        LspStatuslineState {
          phase:  LspStatusPhase::Starting,
          detail: Some("booting".into()),
        }
      } else {
        LspStatuslineState::off(Some("unavailable".into()))
      },
      lsp_spinner_index: 0,
      lsp_spinner_last_tick: Instant::now(),
      lsp_active_progress_tokens: HashSet::new(),
      lsp_watched_file: None,
      lsp_pending_requests: HashMap::new(),
      lsp_completion_items: Vec::new(),
      lsp_completion_raw_items: Vec::new(),
      lsp_completion_resolved_indices: HashSet::new(),
      lsp_completion_visible_indices: Vec::new(),
      lsp_completion_fallback_start: None,
      lsp_completion_generation: 0,
      lsp_pending_auto_completion: None,
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
      syntax_parse_lifecycle: ParseLifecycle::default(),
      syntax_parse_highlight_state: ParseHighlightState::default(),
      registers,
      register: None,
      macro_recording: None,
      macro_replaying: Vec::new(),
      macro_queue: VecDeque::new(),
      last_motion: None,
      text_format,
      gutter_config: GutterConfig::default(),
      gutter_diff_signs: BTreeMap::new(),
      vcs_provider: DiffProviderRegistry::default(),
      vcs_statusline: None,
      vcs_diff: None,
      inline_annotations: Vec::new(),
      overlay_annotations: Vec::new(),
      scrolloff: 5,
    };
    ctx.refresh_vcs_diff_base();
    Ok(ctx)
  }

  pub fn set_dispatch(&mut self, dispatch: &DefaultDispatchStatic<Ctx>) {
    self.dispatch = Some(NonNull::from(dispatch));
  }

  fn clear_vcs_diff(&mut self) {
    self.vcs_diff = None;
    self.gutter_diff_signs.clear();
  }

  fn refresh_vcs_diff_base(&mut self) {
    self.vcs_statusline = self
      .file_path
      .as_deref()
      .and_then(|path| self.vcs_provider.get_statusline_info(path))
      .map(|info| info.statusline_text());

    let Some(path) = self.file_path.clone() else {
      self.clear_vcs_diff();
      return;
    };
    let Some(diff_base) = self.vcs_provider.get_diff_base(&path) else {
      self.clear_vcs_diff();
      return;
    };

    let diff_base = Rope::from_str(String::from_utf8_lossy(&diff_base).as_ref());
    let doc = self.editor.document().text().clone();
    let handle = DiffHandle::new(diff_base, doc);
    self.gutter_diff_signs = vcs_gutter_signs(&handle);
    self.vcs_diff = Some(handle);
  }

  fn refresh_vcs_diff_document(&mut self) {
    let Some(handle) = self.vcs_diff.as_ref() else {
      return;
    };
    let _ = handle.update_document(self.editor.document().text().clone(), true);
    self.gutter_diff_signs = vcs_gutter_signs(handle);
  }

  /// Handle terminal resize.
  pub fn resize(&mut self, width: u16, height: u16) {
    self.editor.view_mut().viewport = Rect::new(0, 0, width, height);
  }

  fn queue_syntax_parse_job(&mut self, doc_version: u64, parse_job: SyntaxParseJob) {
    match self.syntax_parse_lifecycle.queue(doc_version, parse_job) {
      QueueParseDecision::Start(request) => {
        spawn_syntax_parse_request(self.syntax_parse_tx.clone(), request);
      },
      QueueParseDecision::Queued(_) => {},
    }
  }

  pub fn syntax_highlight_refresh_allowed(&self) -> bool {
    self
      .syntax_parse_highlight_state
      .allow_cache_refresh(&self.syntax_parse_lifecycle)
  }

  pub fn poll_syntax_parse_results(&mut self) -> bool {
    let current_doc_version = self.editor.document().version();
    let mut changed = false;

    loop {
      let result = match self.syntax_parse_rx.try_recv() {
        Ok(result) => result,
        Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
      };

      let decision = self.syntax_parse_lifecycle.on_result(
        result.request_id,
        result.doc_version,
        current_doc_version,
      );

      if let Some(next_request) = decision.start_next {
        spawn_syntax_parse_request(self.syntax_parse_tx.clone(), next_request);
      }

      if !decision.apply {
        continue;
      }

      let parsed_state = {
        let doc = self.editor.document_mut();
        match result.syntax {
          Some(syntax) => {
            if let Some(loader) = &self.loader {
              doc.set_syntax_with_loader(syntax, loader.clone());
            } else {
              doc.set_syntax(syntax);
            }
            Some(true)
          },
          None => None,
        }
      };
      if parsed_state == Some(true) {
        self.syntax_parse_highlight_state.mark_parsed();
        self.highlight_cache.clear();
        changed = true;
      } else {
        self.syntax_parse_highlight_state.mark_interpolated();
      }
    }

    changed
  }

  pub fn start_background_services(&mut self) {
    self.lsp_ready = false;
    self.lsp_active_progress_tokens.clear();
    self.lsp_pending_requests.clear();
    self.lsp_sync_watched_file_state();
    let path_preview = env::var("PATH")
      .ok()
      .map(|value| clamp_status_text(&value, 240));
    if let Some(server) = self.lsp_runtime.config().server() {
      self.log_lsp_trace_value(json!({
        "ts_ms": SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|duration| duration.as_millis() as u64).unwrap_or(0),
        "kind": "bootstrap",
        "server": {
          "name": server.name(),
          "command": server.command(),
          "args": server.args(),
        },
        "workspace_root": self.lsp_runtime.config().workspace_root(),
        "env": {
          "THE_EDITOR_LSP_COMMAND": env::var("THE_EDITOR_LSP_COMMAND").ok(),
          "THE_EDITOR_LSP_ARGS": env::var("THE_EDITOR_LSP_ARGS").ok(),
          "PATH": path_preview,
        }
      }));
    } else {
      self.log_lsp_trace_value(json!({
        "ts_ms": SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|duration| duration.as_millis() as u64).unwrap_or(0),
        "kind": "bootstrap",
        "server": null,
        "workspace_root": self.lsp_runtime.config().workspace_root(),
      }));
    }
    if self.lsp_runtime.config().server().is_some() {
      self.set_lsp_status(LspStatusPhase::Starting, Some("starting".into()));
    } else {
      self.set_lsp_status(LspStatusPhase::Off, Some("unavailable".into()));
    }
    if let Err(err) = self.lsp_runtime.start() {
      self.set_lsp_status_error(&err.to_string());
      eprintln!("Warning: failed to start LSP runtime: {err}");
    }
  }

  pub fn shutdown_background_services(&mut self) {
    self.lsp_close_current_document();
    self.lsp_ready = false;
    self.lsp_active_progress_tokens.clear();
    self.lsp_pending_requests.clear();
    self.set_lsp_status(LspStatusPhase::Off, Some("stopped".into()));
    self.log_lsp_trace_value(json!({
      "ts_ms": SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|duration| duration.as_millis() as u64).unwrap_or(0),
      "kind": "shutdown",
    }));
    self.lsp_watched_file = None;
    self.syntax_parse_highlight_state.mark_cleared();
    if let Err(err) = self.lsp_runtime.shutdown() {
      eprintln!("Warning: failed to stop LSP runtime: {err}");
    }
  }

  pub fn flush_message_log(&mut self) {
    let Some(writer) = self.message_log.as_mut() else {
      return;
    };
    let events = self.messages.events_since(self.message_log_seq);
    if events.is_empty() {
      return;
    }

    let mut had_error = None;
    for event in events {
      let seq = event.seq;
      let timestamp_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
      let entry = json!({
        "ts_ms": timestamp_ms,
        "event": event,
      });
      let line = match serde_json::to_string(&entry) {
        Ok(line) => line,
        Err(err) => {
          had_error = Some(format!("failed to serialize message event: {err}"));
          break;
        },
      };
      if let Err(err) = writeln!(writer, "{line}") {
        had_error = Some(format!("failed to write message event log: {err}"));
        break;
      }
      self.message_log_seq = seq;
    }

    if had_error.is_none()
      && let Err(err) = writer.flush()
    {
      had_error = Some(format!("failed to flush message event log: {err}"));
    }

    if let Some(err) = had_error {
      eprintln!("Warning: {err}");
      self.message_log = None;
    }
  }

  fn log_lsp_trace_value(&mut self, entry: Value) {
    let Some(writer) = self.lsp_trace_log.as_mut() else {
      return;
    };
    let line = match serde_json::to_string(&entry) {
      Ok(line) => line,
      Err(err) => {
        eprintln!("Warning: failed to serialize lsp trace entry: {err}");
        self.lsp_trace_log = None;
        return;
      },
    };
    if let Err(err) = writeln!(writer, "{line}") {
      eprintln!("Warning: failed to write lsp trace entry: {err}");
      self.lsp_trace_log = None;
      return;
    }
    if let Err(err) = writer.flush() {
      eprintln!("Warning: failed to flush lsp trace log: {err}");
      self.lsp_trace_log = None;
    }
  }

  fn log_lsp_trace_event(&mut self, event: &LspEvent) {
    let timestamp_ms = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .map(|duration| duration.as_millis() as u64)
      .unwrap_or(0);
    self.log_lsp_trace_value(json!({
      "ts_ms": timestamp_ms,
      "kind": "event",
      "event": summarize_lsp_event(event),
    }));
  }

  fn set_lsp_status(&mut self, phase: LspStatusPhase, detail: Option<String>) {
    self.lsp_statusline = LspStatuslineState {
      phase,
      detail: detail.map(|value| clamp_status_text(&value, 28)),
    };
    if !self.lsp_statusline.is_loading() {
      self.lsp_spinner_index = 0;
    }
  }

  fn set_lsp_status_error(&mut self, message: &str) {
    self.lsp_active_progress_tokens.clear();
    let summary = summarize_lsp_error(message);
    self.set_lsp_status(LspStatusPhase::Error, Some(summary));
  }

  pub fn tick_lsp_statusline(&mut self) -> bool {
    if matches!(self.lsp_statusline.phase, LspStatusPhase::Busy)
      && self.lsp_active_progress_tokens.is_empty()
      && self.lsp_ready
    {
      self.set_lsp_status(LspStatusPhase::Ready, None);
      return true;
    }
    if !self.lsp_statusline.is_loading() {
      return false;
    }
    let now = Instant::now();
    if now.duration_since(self.lsp_spinner_last_tick) < Duration::from_millis(80) {
      return false;
    }
    self.lsp_spinner_last_tick = now;
    self.lsp_spinner_index = (self.lsp_spinner_index + 1) % 8;
    true
  }

  fn lsp_statusline_text_value(&self) -> Option<String> {
    let has_server = self.lsp_runtime.config().server().is_some();
    if !has_server && matches!(self.lsp_statusline.phase, LspStatusPhase::Off) {
      return Some("lsp: unavailable".to_string());
    }

    let detail = self.lsp_statusline.detail.clone().unwrap_or_default();
    let text = match self.lsp_statusline.phase {
      LspStatusPhase::Off => {
        if detail.is_empty() {
          "lsp: off".to_string()
        } else {
          format!("lsp: {detail}")
        }
      },
      LspStatusPhase::Starting => {
        format!(
          "lsp: {} {}",
          spinner_frame(self.lsp_spinner_index),
          detail_if_empty(detail, "starting")
        )
      },
      LspStatusPhase::Initializing => {
        format!(
          "lsp: {} {}",
          spinner_frame(self.lsp_spinner_index),
          detail_if_empty(detail, "initializing")
        )
      },
      LspStatusPhase::Ready => {
        if detail.is_empty() {
          "lsp: ready".to_string()
        } else {
          format!("lsp: ready ({detail})")
        }
      },
      LspStatusPhase::Busy => {
        format!(
          "lsp: {} {}",
          spinner_frame(self.lsp_spinner_index),
          detail_if_empty(detail, "working")
        )
      },
      LspStatusPhase::Error => {
        if detail.is_empty() {
          "lsp: error".to_string()
        } else {
          format!("lsp: error ({detail})")
        }
      },
    };

    Some(clamp_status_text(&text, 36))
  }

  pub fn poll_lsp_events(&mut self) -> bool {
    let mut needs_render = false;
    while let Some(event) = self.lsp_runtime.try_recv_event() {
      self.log_lsp_trace_event(&event);
      match event {
        LspEvent::Started { .. } => {
          if self.lsp_runtime.config().server().is_none() {
            self.set_lsp_status(LspStatusPhase::Off, Some("unavailable".into()));
          } else {
            self.set_lsp_status(LspStatusPhase::Starting, Some("starting".into()));
          }
          needs_render = true;
        },
        LspEvent::CapabilitiesRegistered { server_name } => {
          let matches_configured_server = self
            .lsp_runtime
            .config()
            .server()
            .is_some_and(|server| server.name() == server_name);
          if matches_configured_server {
            self.lsp_ready = true;
            self.lsp_active_progress_tokens.clear();
            self.lsp_open_current_document();
            self.set_lsp_status(LspStatusPhase::Ready, Some(server_name));
            needs_render = true;
          }
        },
        LspEvent::ServerStarted { server_name, .. } => {
          self.lsp_ready = false;
          self.lsp_active_progress_tokens.clear();
          self.lsp_pending_requests.clear();
          if let Some(state) = self.lsp_document.as_mut() {
            state.opened = false;
          }
          self.set_lsp_status(LspStatusPhase::Starting, Some(server_name));
          needs_render = true;
        },
        LspEvent::RequestDispatched { method, .. } => {
          if method == "initialize" {
            self.set_lsp_status(LspStatusPhase::Initializing, Some("initializing".into()));
            needs_render = true;
          }
        },
        LspEvent::ServerStopped { .. } | LspEvent::Stopped => {
          self.lsp_ready = false;
          self.lsp_active_progress_tokens.clear();
          self.lsp_pending_requests.clear();
          if let Some(state) = self.lsp_document.as_mut() {
            state.opened = false;
          }
          if self.lsp_runtime.config().server().is_some() {
            self.set_lsp_status(LspStatusPhase::Starting, Some("restarting".into()));
          } else {
            self.set_lsp_status(LspStatusPhase::Off, Some("stopped".into()));
          }
          needs_render = true;
        },
        LspEvent::RpcMessage { message } => {
          needs_render |= self.handle_lsp_rpc_message(message);
        },
        LspEvent::RequestTimedOut { id, method } => {
          if let Some(pending) = self.lsp_pending_requests.remove(&id) {
            self.messages.publish(
              MessageLevel::Warning,
              Some("lsp".into()),
              format!("lsp {} timed out", pending.label()),
            );
          } else {
            self.messages.publish(
              MessageLevel::Warning,
              Some("lsp".into()),
              format!("lsp {method} timed out"),
            );
          }
          self.set_lsp_status(LspStatusPhase::Error, Some("request timeout".into()));
          needs_render = true;
        },
        LspEvent::Progress { progress } => {
          match progress.kind {
            LspProgressKind::Begin => {
              let text =
                format_lsp_progress_text(progress.title.as_deref(), progress.message.as_deref());
              self.lsp_active_progress_tokens.insert(progress.token);
              self.set_lsp_status(LspStatusPhase::Busy, Some(text.clone()));
              self
                .messages
                .publish(MessageLevel::Info, Some("lsp".into()), text);
              needs_render = true;
            },
            LspProgressKind::End => {
              self.lsp_active_progress_tokens.remove(&progress.token);
              if self.lsp_ready && self.lsp_active_progress_tokens.is_empty() {
                self.set_lsp_status(LspStatusPhase::Ready, None);
                needs_render = true;
              }
              if let Some(message) = progress.message.and_then(non_empty_trimmed) {
                self
                  .messages
                  .publish(MessageLevel::Info, Some("lsp".into()), message);
                needs_render = true;
              }
            },
            LspProgressKind::Report => {
              if self.lsp_active_progress_tokens.contains(&progress.token) {
                let text =
                  format_lsp_progress_text(progress.title.as_deref(), progress.message.as_deref());
                self.set_lsp_status(LspStatusPhase::Busy, Some(text));
                needs_render = true;
              }
            },
          }
        },
        LspEvent::Error(message) => {
          self.set_lsp_status_error(&message);
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

  pub fn poll_lsp_completion_auto_trigger(&mut self) -> bool {
    let Some(pending) = self.lsp_pending_auto_completion.clone() else {
      return false;
    };
    if self.mode != Mode::Insert {
      self.lsp_pending_auto_completion = None;
      return false;
    }
    if Instant::now() < pending.due_at {
      return false;
    }

    self.lsp_pending_auto_completion = None;
    let _ = self.dispatch_completion_request(pending.trigger, false);
    false
  }

  pub fn poll_lsp_file_watch(&mut self) -> bool {
    let lsp_ready = self.lsp_ready;
    let (watched_uri, watched_path, pending_changes) = match poll_watch_events(
      self
        .lsp_watched_file
        .as_mut()
        .map(|watch| &mut watch.stream),
      Instant::now(),
      "term",
      |event, message| trace_file_watch_event(event, message),
    ) {
      WatchPollOutcome::NoChanges => return false,
      WatchPollOutcome::Disconnected { .. } => {
        self.lsp_sync_watched_file_state();
        return false;
      },
      WatchPollOutcome::Changes { path, uri, kinds } => {
        let pending_changes = kinds
          .into_iter()
          .map(file_change_type_for_path_event)
          .collect::<Vec<_>>();
        (uri, path, pending_changes)
      },
    };

    trace_file_watch_event(
      "consumer_changes_collected",
      format!(
        "client=term path={} changes={}",
        watched_path.display(),
        pending_changes.len()
      ),
    );

    if lsp_ready {
      let params = did_change_watched_files_params(
        pending_changes
          .iter()
          .copied()
          .map(|change_type| (watched_uri.clone(), change_type)),
      );
      let _ = self
        .lsp_runtime
        .send_notification("workspace/didChangeWatchedFiles", Some(params));
      trace_file_watch_event(
        "consumer_lsp_notify_sent",
        format!(
          "client=term path={} changes={}",
          watched_path.display(),
          pending_changes.len()
        ),
      );
    } else {
      trace_file_watch_event(
        "consumer_lsp_notify_skipped",
        format!(
          "client=term path={} reason=lsp_not_ready changes={}",
          watched_path.display(),
          pending_changes.len()
        ),
      );
    }

    if let Some(change_type) = pending_changes.last().copied() {
      return self.handle_external_file_watch_change(&watched_path, change_type);
    }

    false
  }

  fn handle_external_file_watch_change(
    &mut self,
    watched_path: &Path,
    change_type: FileChangeType,
  ) -> bool {
    let label = watched_path
      .file_name()
      .map(|name| name.to_string_lossy().to_string())
      .unwrap_or_else(|| watched_path.display().to_string());

    match change_type {
      FileChangeType::Deleted => {
        if let Some(watch) = self.lsp_watched_file.as_mut() {
          clear_reload_state(&mut watch.stream.reload_state);
        }
        trace_file_watch_event(
          "consumer_external_deleted",
          format!("client=term path={}", watched_path.display()),
        );
        self.messages.publish(
          MessageLevel::Warning,
          Some("watch".into()),
          format!("file deleted on disk: {label}"),
        );
        true
      },
      FileChangeType::Created | FileChangeType::Changed => {
        let current = self.editor.document().text().clone();
        let buffer_modified = self.editor.document().flags().modified;
        let decision = match self.lsp_watched_file.as_mut() {
          Some(watch) => {
            match evaluate_external_reload_from_disk(
              &mut watch.stream.reload_state,
              &mut watch.stream.reload_io,
              watched_path,
              &current,
              buffer_modified,
            ) {
              Ok(decision) => decision,
              Err(err) => {
                match err {
                  FileWatchReloadError::BackoffActive { retry_after } => {
                    let retry_in_ms = retry_after
                      .saturating_duration_since(Instant::now())
                      .as_millis();
                    trace_file_watch_event(
                      "consumer_external_read_backoff",
                      format!(
                        "client=term path={} retry_in_ms={retry_in_ms}",
                        watched_path.display()
                      ),
                    );
                    return false;
                  },
                  FileWatchReloadError::ReadFailed {
                    error, retry_after, ..
                  } => {
                    let retry_in_ms = retry_after
                      .saturating_duration_since(Instant::now())
                      .as_millis();
                    trace_file_watch_event(
                      "consumer_external_read_err",
                      format!(
                        "client=term path={} err={} retry_in_ms={retry_in_ms}",
                        watched_path.display(),
                        error
                      ),
                    );
                    self.messages.publish(
                      MessageLevel::Warning,
                      Some("watch".into()),
                      format!(
                        "failed to read '{label}' from disk: {error} (retrying in {retry_in_ms}ms)"
                      ),
                    );
                    return true;
                  },
                }
              },
            }
          },
          None => return false,
        };

        match decision {
          FileWatchReloadDecision::Noop => {
            trace_file_watch_event(
              "consumer_external_noop",
              format!("client=term path={}", watched_path.display()),
            );
            false
          },
          FileWatchReloadDecision::ConflictEntered => {
            trace_file_watch_event(
              "consumer_external_changed_dirty",
              format!("client=term path={}", watched_path.display()),
            );
            self.messages.publish(
              MessageLevel::Warning,
              Some("watch".into()),
              format!(
                "file changed on disk: {label} (buffer has unsaved changes; run :rl to reload \
                 disk or :w! to overwrite disk)"
              ),
            );
            true
          },
          FileWatchReloadDecision::ConflictOngoing => {
            trace_file_watch_event(
              "consumer_external_conflict_ongoing",
              format!("client=term path={}", watched_path.display()),
            );
            false
          },
          FileWatchReloadDecision::ReloadNeeded => {
            match <Self as the_default::DefaultContext>::reload_file_preserving_view(
              self,
              watched_path,
            ) {
              Ok(()) => {
                if let Some(watch) = self.lsp_watched_file.as_mut() {
                  mark_reload_applied(&mut watch.stream.reload_state);
                }
                trace_file_watch_event(
                  "consumer_external_reload_ok",
                  format!("client=term path={}", watched_path.display()),
                );
                self.messages.publish(
                  MessageLevel::Info,
                  Some("watch".into()),
                  format!("reloaded from disk: {label}"),
                );
                true
              },
              Err(err) => {
                trace_file_watch_event(
                  "consumer_external_reload_err",
                  format!("client=term path={} err={err}", watched_path.display()),
                );
                self.messages.publish(
                  MessageLevel::Error,
                  Some("watch".into()),
                  format!("failed to reload '{label}': {err}"),
                );
                true
              },
            }
          },
        }
      },
    }
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
      PendingLspRequestKind::Completion {
        generation,
        cursor,
        replace_start,
        announce_empty,
        ..
      } => {
        self.handle_completion_response(
          response.result.as_ref(),
          generation,
          cursor,
          replace_start,
          announce_empty,
        )
      },
      PendingLspRequestKind::CompletionResolve { index, .. } => {
        self.handle_completion_resolve_response(index, response.result.as_ref())
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

  fn handle_completion_response(
    &mut self,
    result: Option<&Value>,
    generation: u64,
    request_cursor: usize,
    replace_start: usize,
    announce_empty: bool,
  ) -> bool {
    if generation != self.lsp_completion_generation {
      return false;
    }
    if self.mode != Mode::Insert {
      return false;
    }
    let Some(current_cursor) = self.primary_cursor_char_idx() else {
      return false;
    };
    if current_cursor != request_cursor {
      return false;
    }

    let completion = match parse_completion_response_with_raw(result) {
      Ok(completion) => completion,
      Err(err) => {
        self.messages.publish(
          MessageLevel::Error,
          Some("lsp".into()),
          format!("failed to parse completion response: {err}"),
        );
        return true;
      },
    };

    if completion.items.is_empty() {
      self.lsp_completion_items.clear();
      self.lsp_completion_raw_items.clear();
      self.lsp_completion_resolved_indices.clear();
      self.lsp_completion_visible_indices.clear();
      self.lsp_completion_fallback_start = None;
      self.completion_menu.clear();
      if announce_empty {
        self.messages.publish(
          MessageLevel::Info,
          Some("lsp".into()),
          "no completion candidates",
        );
      }
      return true;
    }

    self.lsp_completion_items = completion.items;
    self.lsp_completion_raw_items = completion.raw_items;
    self.lsp_completion_resolved_indices.clear();
    self.lsp_completion_fallback_start = Some(replace_start.min(request_cursor));
    self.rebuild_completion_menu();
    true
  }

  fn handle_completion_resolve_response(&mut self, index: usize, result: Option<&Value>) -> bool {
    let resolved = match parse_completion_item_response(result) {
      Ok(item) => item,
      Err(err) => {
        self.messages.publish(
          MessageLevel::Warning,
          Some("lsp".into()),
          format!("failed to parse completion resolve response: {err}"),
        );
        return true;
      },
    };

    self.lsp_completion_resolved_indices.insert(index);

    let Some(resolved) = resolved else {
      return true;
    };
    let Some(item) = self.lsp_completion_items.get_mut(index) else {
      return true;
    };
    merge_resolved_completion_item(item, resolved);

    if let Some(visible_index) = self
      .lsp_completion_visible_indices
      .iter()
      .position(|candidate| *candidate == index)
      && let Some(ui_item) = self.completion_menu.items.get_mut(visible_index)
    {
      *ui_item = completion_menu_item_for_lsp_item(item);
      self.needs_render = true;
    }
    true
  }

  fn apply_completion_item(
    &mut self,
    item: LspCompletionItem,
    fallback_range: std::ops::Range<usize>,
  ) -> bool {
    let item = normalize_completion_item_for_apply(item);
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
      let applied = self.apply_workspace_edit(&workspace_edit, "completion");
      if applied {
        let _ = self.editor.document_mut().commit();
      }
      return applied;
    }

    let insert_text = item.insert_text.unwrap_or(item.label);
    if insert_text.is_empty() {
      return true;
    }

    let text_len = self.editor.document().text().len_chars();
    let from = fallback_range.start.min(text_len);
    let to = fallback_range.end.min(text_len).max(from);
    let tx = match Transaction::change(self.editor.document().text(), vec![(
      from,
      to,
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
      let _ = self.editor.document_mut().commit();
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

  fn primary_cursor_char_idx(&self) -> Option<usize> {
    let doc = self.editor.document();
    let range = doc.selection().ranges().first().copied()?;
    Some(range.cursor(doc.text().slice(..)))
  }

  fn cursor_prev_char_is_word(&self) -> bool {
    let Some(cursor) = self.primary_cursor_char_idx() else {
      return false;
    };
    if cursor == 0 {
      return false;
    }
    self
      .editor
      .document()
      .text()
      .get_char(cursor.saturating_sub(1))
      .is_some_and(is_symbol_word_char)
  }

  fn completion_replace_start_at_cursor(&self, cursor: usize) -> usize {
    let text = self.editor.document().text();
    let mut start = cursor.min(text.len_chars());
    while start > 0
      && text
        .get_char(start - 1)
        .is_some_and(is_completion_replace_char)
    {
      start -= 1;
    }
    start
  }

  fn lsp_completion_supports_trigger_char(&self, ch: char) -> bool {
    let Some(server) = self.lsp_runtime.config().server() else {
      return false;
    };
    let Some(capabilities) = self.lsp_runtime.server_capabilities(server.name()) else {
      return false;
    };
    let Some(values) = capabilities
      .raw()
      .get("completionProvider")
      .and_then(|provider| provider.get("triggerCharacters"))
      .and_then(Value::as_array)
    else {
      return false;
    };

    values.iter().filter_map(Value::as_str).any(|value| {
      let mut chars = value.chars();
      matches!(chars.next(), Some(first) if first == ch && chars.next().is_none())
    })
  }

  fn completion_source_index_for_visible_index(&self, index: usize) -> Option<usize> {
    self.lsp_completion_visible_indices.get(index).copied()
  }

  fn completion_filter_fragment(&self) -> Option<String> {
    let cursor = self.primary_cursor_char_idx()?;
    let start = self.lsp_completion_fallback_start.unwrap_or(cursor).min(cursor);
    let doc = self.editor.document();
    let text = doc.text();
    let fragment = text.slice(start..cursor).to_string();
    Some(fragment)
  }

  fn rebuild_completion_menu(&mut self) {
    if self.lsp_completion_items.is_empty() {
      self.lsp_completion_visible_indices.clear();
      self.completion_menu.clear();
      return;
    }

    let fragment = self.completion_filter_fragment().unwrap_or_default();
    let mut visible: Vec<(usize, u32)> = self
      .lsp_completion_items
      .iter()
      .enumerate()
      .filter_map(|(index, item)| {
        let candidate = completion_item_filter_text(item);
        completion_match_score(&fragment, candidate).map(|score| (index, score))
      })
      .collect();

    visible.sort_by(|(left_index, left_score), (right_index, right_score)| {
      right_score
        .cmp(left_score)
        .then_with(|| {
          self.lsp_completion_items[*right_index]
            .preselect
            .cmp(&self.lsp_completion_items[*left_index].preselect)
        })
        .then_with(|| {
          let left_key = completion_item_sort_key(&self.lsp_completion_items[*left_index]);
          let right_key = completion_item_sort_key(&self.lsp_completion_items[*right_index]);
          left_key.cmp(&right_key)
        })
        .then_with(|| {
          self.lsp_completion_items[*left_index]
            .label
            .cmp(&self.lsp_completion_items[*right_index].label)
        })
        .then_with(|| left_index.cmp(right_index))
    });

    self.lsp_completion_visible_indices = visible.into_iter().map(|(index, _)| index).collect();
    if self.lsp_completion_visible_indices.is_empty() {
      self.completion_menu.clear();
      return;
    }

    let menu_items = self
      .lsp_completion_visible_indices
      .iter()
      .filter_map(|index| self.lsp_completion_items.get(*index))
      .map(completion_menu_item_for_lsp_item)
      .collect();
    the_default::show_completion_menu(self, menu_items);
  }

  fn clear_completion_state(&mut self) {
    self.lsp_completion_items.clear();
    self.lsp_completion_raw_items.clear();
    self.lsp_completion_resolved_indices.clear();
    self.lsp_completion_visible_indices.clear();
    self.lsp_completion_fallback_start = None;
    self.completion_menu.clear();
  }

  fn dispatch_completion_request(
    &mut self,
    trigger: CompletionTriggerSource,
    announce_empty: bool,
  ) -> bool {
    if !self.lsp_supports(LspCapability::Completion) {
      if matches!(trigger, CompletionTriggerSource::Manual) {
        self.messages.publish(
          MessageLevel::Warning,
          Some("lsp".into()),
          "completion is not supported by the active server",
        );
      }
      return false;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      if matches!(trigger, CompletionTriggerSource::Manual) {
        self.messages.publish(
          MessageLevel::Warning,
          Some("lsp".into()),
          "completion unavailable: no active LSP document",
        );
      }
      return false;
    };
    let Some(cursor) = self.primary_cursor_char_idx() else {
      return false;
    };
    let replace_start = self.completion_replace_start_at_cursor(cursor);

    self.lsp_completion_generation = self.lsp_completion_generation.wrapping_add(1);
    let generation = self.lsp_completion_generation;
    let context = trigger.to_lsp_context();
    self.dispatch_lsp_request(
      "textDocument/completion",
      completion_params(&uri, position, &context),
      PendingLspRequestKind::Completion {
        uri,
        generation,
        cursor,
        replace_start,
        announce_empty,
      },
    );
    true
  }

  fn schedule_auto_completion(
    &mut self,
    trigger: CompletionTriggerSource,
    delay: Duration,
  ) -> bool {
    if self.mode != Mode::Insert || !self.lsp_supports(LspCapability::Completion) {
      self.lsp_pending_auto_completion = None;
      return false;
    }

    self.lsp_pending_auto_completion = Some(PendingAutoCompletion {
      due_at: Instant::now() + delay,
      trigger,
    });
    true
  }

  fn cancel_auto_completion(&mut self) {
    self.lsp_pending_auto_completion = None;
  }

  fn handle_completion_action(&mut self, command: Command) -> bool {
    if self.mode != Mode::Insert {
      self.cancel_auto_completion();
      return false;
    }

    match command {
      Command::InsertChar(ch) => {
        if self.completion_menu.active {
          self.rebuild_completion_menu();
        }
        if self.lsp_completion_supports_trigger_char(ch) {
          return self.schedule_auto_completion(
            CompletionTriggerSource::TriggerCharacter(ch),
            lsp_completion_trigger_char_latency(),
          );
        }
        if is_symbol_word_char(ch) {
          return self.schedule_auto_completion(
            CompletionTriggerSource::Invoked,
            lsp_completion_auto_trigger_latency(),
          );
        }
        self.cancel_auto_completion();
        false
      },
      Command::DeleteChar
      | Command::DeleteCharForward { .. }
      | Command::DeleteWordBackward { .. }
      | Command::DeleteWordForward { .. }
      | Command::KillToLineStart
      | Command::KillToLineEnd => {
        if self.completion_menu.active {
          self.rebuild_completion_menu();
        }
        if self.completion_menu.active || self.cursor_prev_char_is_word() {
          return self.schedule_auto_completion(
            CompletionTriggerSource::Incomplete,
            lsp_completion_auto_trigger_latency(),
          );
        }
        self.cancel_auto_completion();
        false
      },
      Command::LspCompletion
      | Command::CompletionNext
      | Command::CompletionPrev
      | Command::CompletionAccept
      | Command::CompletionCancel => true,
      _ => {
        self.cancel_auto_completion();
        false
      },
    }
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

  fn cancel_pending_lsp_requests_for(&mut self, next: &PendingLspRequestKind) {
    let target = next.cancellation_key();
    let ids_to_cancel = self
      .lsp_pending_requests
      .iter()
      .filter_map(|(id, pending)| {
        if pending.cancellation_key() == target {
          Some(*id)
        } else {
          None
        }
      })
      .collect::<Vec<_>>();

    for id in ids_to_cancel {
      let _ = self.lsp_pending_requests.remove(&id);
      if let Err(err) = self.lsp_runtime.cancel_request(id) {
        self.messages.publish(
          MessageLevel::Warning,
          Some("lsp".into()),
          format!("failed to cancel stale request {id}: {err}"),
        );
      }
    }
  }

  fn dispatch_lsp_request(
    &mut self,
    method: &'static str,
    params: Value,
    pending: PendingLspRequestKind,
  ) {
    self.cancel_pending_lsp_requests_for(&pending);
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

  fn resolve_completion_item_if_needed(&mut self, index: usize) {
    if !self.completion_menu.active {
      return;
    }
    if self.lsp_completion_resolved_indices.contains(&index) {
      return;
    }
    if index >= self.lsp_completion_items.len() || index >= self.lsp_completion_raw_items.len() {
      return;
    }
    let pending = self.lsp_pending_requests.values().any(|request| {
      matches!(
        request,
        PendingLspRequestKind::CompletionResolve {
          index: pending_index,
          ..
        } if *pending_index == index
      )
    });
    if pending {
      return;
    }

    let Some(uri) = self.current_lsp_uri() else {
      return;
    };
    let params = self.lsp_completion_raw_items[index].clone();
    match self
      .lsp_runtime
      .send_request("completionItem/resolve", Some(params))
    {
      Ok(request_id) => {
        self
          .lsp_pending_requests
          .insert(request_id, PendingLspRequestKind::CompletionResolve {
            uri,
            index,
          });
      },
      Err(err) => {
        self.messages.publish(
          MessageLevel::Warning,
          Some("lsp".into()),
          format!("failed to dispatch completionItem/resolve: {err}"),
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

  fn lsp_sync_watched_file_state(&mut self) {
    self.lsp_watched_file = self.lsp_document.as_ref().map(|state| {
      let (events_rx, watch_handle) = watch_path(&state.path, lsp_file_watch_latency());
      LspWatchedFileState {
        stream:        WatchedFileEventsState {
          path: state.path.clone(),
          uri: state.uri.clone(),
          events_rx,
          suppress_until: None,
          reload_state: FileWatchReloadState::Clean,
          reload_io: FileWatchReloadIoState::default(),
        },
        _watch_handle: watch_handle,
      }
    });
  }

  fn lsp_reconfigure_runtime_for_path(&mut self, path: Option<&Path>) {
    let next_server = resolve_lsp_server(self.loader.as_deref(), path);
    if lsp_server_configs_equal(self.lsp_runtime.config().server(), next_server.as_ref()) {
      return;
    }

    let was_running = self.lsp_runtime.is_running();
    self.lsp_close_current_document();
    self.lsp_ready = false;
    self.lsp_active_progress_tokens.clear();
    self.lsp_pending_requests.clear();
    self.lsp_watched_file = None;
    self.clear_completion_state();
    self.cancel_auto_completion();

    if was_running && let Err(err) = self.lsp_runtime.shutdown() {
      eprintln!("Warning: failed to stop LSP runtime while reconfiguring: {err}");
    }

    let mut runtime_config = self.lsp_runtime.config().clone();
    runtime_config = match next_server {
      Some(server) => runtime_config.with_server(server),
      None => runtime_config.clear_server(),
    };
    self.lsp_runtime = LspRuntime::new(runtime_config);

    if was_running {
      self.start_background_services();
    } else if self.lsp_runtime.config().server().is_some() {
      self.set_lsp_status(LspStatusPhase::Starting, Some("starting".into()));
    } else {
      self.set_lsp_status(LspStatusPhase::Off, Some("unavailable".into()));
    }
  }

  fn lsp_refresh_document_state(&mut self, path: Option<&Path>) {
    self.lsp_document =
      path.and_then(|path| build_lsp_document_state(path, self.loader.as_deref()));
    self.lsp_reconfigure_runtime_for_path(path);
    self.lsp_sync_watched_file_state();
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

fn is_completion_replace_char(ch: char) -> bool {
  is_symbol_word_char(ch)
}

fn lsp_file_watch_latency() -> Duration {
  Duration::from_millis(120)
}

fn lsp_completion_auto_trigger_latency() -> Duration {
  Duration::from_millis(80)
}

fn lsp_completion_trigger_char_latency() -> Duration {
  Duration::from_millis(20)
}

fn lsp_self_save_suppress_window() -> Duration {
  Duration::from_millis(500)
}

fn watch_statusline_text_for_state(state: FileWatchReloadState) -> Option<String> {
  match state {
    FileWatchReloadState::Conflict => Some("watch: conflict".to_string()),
    FileWatchReloadState::ReloadNeeded => Some("watch: reload pending".to_string()),
    FileWatchReloadState::Clean => None,
  }
}

fn file_change_type_for_path_event(kind: PathEventKind) -> FileChangeType {
  match kind {
    PathEventKind::Created => FileChangeType::Created,
    PathEventKind::Changed => FileChangeType::Changed,
    PathEventKind::Removed => FileChangeType::Deleted,
  }
}

fn vcs_gutter_signs(handle: &DiffHandle) -> BTreeMap<usize, RenderGutterDiffKind> {
  handle
    .load()
    .line_signs()
    .into_iter()
    .map(|(line, kind)| {
      let marker = match kind {
        DiffSignKind::Added => RenderGutterDiffKind::Added,
        DiffSignKind::Modified => RenderGutterDiffKind::Modified,
        DiffSignKind::Removed => RenderGutterDiffKind::Removed,
      };
      (line, marker)
    })
    .collect()
}

fn summarize_lsp_event(event: &LspEvent) -> Value {
  match event {
    LspEvent::Started { workspace_root } => {
      json!({
        "name": "started",
        "workspace_root": workspace_root,
      })
    },
    LspEvent::ServerStarted {
      server_name,
      command,
      args,
    } => {
      json!({
        "name": "server_started",
        "server": server_name,
        "command": command,
        "args": args,
      })
    },
    LspEvent::ServerStopped { exit_code } => {
      json!({
        "name": "server_stopped",
        "exit_code": exit_code,
      })
    },
    LspEvent::CapabilitiesRegistered { server_name } => {
      json!({
        "name": "capabilities_registered",
        "server": server_name,
      })
    },
    LspEvent::RequestDispatched { id, method } => {
      json!({
        "name": "request_dispatched",
        "id": id,
        "method": method,
      })
    },
    LspEvent::RequestCompleted { id } => {
      json!({
        "name": "request_completed",
        "id": id,
      })
    },
    LspEvent::RequestTimedOut { id, method } => {
      json!({
        "name": "request_timed_out",
        "id": id,
        "method": method,
      })
    },
    LspEvent::DiagnosticsPublished { diagnostics } => {
      json!({
        "name": "diagnostics_published",
        "uri": diagnostics.uri,
        "count": diagnostics.diagnostics.len(),
      })
    },
    LspEvent::Progress { progress } => {
      json!({
        "name": "progress",
        "token": progress.token,
        "phase": format!("{:?}", progress.kind).to_lowercase(),
        "title": progress.title,
        "message": progress.message,
        "percentage": progress.percentage,
      })
    },
    LspEvent::RpcMessage { message } => {
      json!({
        "name": "rpc_message",
        "summary": summarize_rpc_message(message),
      })
    },
    LspEvent::ServerStderr { line } => {
      json!({
        "name": "server_stderr",
        "line": line,
      })
    },
    LspEvent::Stopped => {
      json!({
        "name": "stopped",
      })
    },
    LspEvent::Error(message) => {
      json!({
        "name": "error",
        "message": message,
      })
    },
  }
}

fn summarize_rpc_message(message: &jsonrpc::Message) -> Value {
  match message {
    jsonrpc::Message::Request(request) => {
      json!({
        "type": "request",
        "id": summarize_jsonrpc_id(&request.id),
        "method": request.method,
      })
    },
    jsonrpc::Message::Notification(notification) => {
      json!({
        "type": "notification",
        "method": notification.method,
      })
    },
    jsonrpc::Message::Response(response) => {
      json!({
        "type": "response",
        "id": summarize_jsonrpc_id(&response.id),
        "is_error": response.error.is_some(),
      })
    },
  }
}

fn summarize_jsonrpc_id(id: &jsonrpc::Id) -> Value {
  match id {
    jsonrpc::Id::Null => Value::Null,
    jsonrpc::Id::Number(number) => json!(number),
    jsonrpc::Id::String(value) => json!(value),
  }
}

fn spinner_frame(index: usize) -> char {
  const FRAMES: [char; 8] = ['⣾', '⣽', '⣻', '⢿', '⡿', '⣟', '⣯', '⣷'];
  FRAMES[index % FRAMES.len()]
}

fn detail_if_empty(detail: String, fallback: &str) -> String {
  if detail.is_empty() {
    fallback.to_string()
  } else {
    detail
  }
}

fn clamp_status_text(text: &str, max_chars: usize) -> String {
  if max_chars == 0 {
    return String::new();
  }
  if text.chars().count() <= max_chars {
    return text.to_string();
  }
  if max_chars == 1 {
    return "…".to_string();
  }
  let mut out = String::new();
  for ch in text.chars().take(max_chars - 1) {
    out.push(ch);
  }
  out.push('…');
  out
}

fn non_empty_trimmed(value: String) -> Option<String> {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    None
  } else {
    Some(trimmed.to_string())
  }
}

fn completion_menu_item_for_lsp_item(item: &LspCompletionItem) -> the_default::CompletionMenuItem {
  let mut menu_item = the_default::CompletionMenuItem::new(item.label.clone());
  menu_item.detail = completion_menu_detail_text(item);
  menu_item.documentation = completion_menu_documentation_text(item);
  if let Some(kind) = item.kind {
    menu_item.kind_icon = Some(completion_kind_icon(kind).to_string());
    menu_item.kind_color = Some(completion_kind_color(kind));
  }
  menu_item
}

fn completion_kind_icon(kind: LspCompletionItemKind) -> &'static str {
  use LspCompletionItemKind::*;
  match kind {
    Text          => "w",
    Method        => "f",
    Function      => "f",
    Constructor   => "f",
    Field         => "m",
    Variable      => "v",
    Class         => "c",
    Interface     => "i",
    Module        => "M",
    Property      => "m",
    Unit          => "u",
    Value         => "v",
    Enum          => "e",
    Keyword       => "k",
    Snippet       => "S",
    Color         => "v",
    File          => "F",
    Reference     => "r",
    Folder        => "D",
    EnumMember    => "e",
    Constant      => "C",
    Struct        => "s",
    Event         => "E",
    Operator      => "o",
    TypeParameter => "t",
  }
}

fn completion_kind_color(kind: LspCompletionItemKind) -> the_lib::render::graphics::Color {
  use LspCompletionItemKind::*;
  use the_lib::render::graphics::Color;
  match kind {
    Method | Function | Constructor | Operator => Color::Rgb(0xdb, 0xbf, 0xef), // lilac
    Field | Variable | Property | Value | Reference => Color::Rgb(0xa4, 0xa0, 0xe8), // lavender
    Class | Interface | Enum | Struct | TypeParameter => Color::Rgb(0xef, 0xba, 0x5d), // honey
    Module | Folder | EnumMember | Constant => Color::Rgb(0xe8, 0xdc, 0xa0),    // chamois
    Keyword                                 => Color::Rgb(0xec, 0xcd, 0xba),    // almond
    Snippet                                 => Color::Rgb(0x9f, 0xf2, 0x8f),    // mint
    Event                                   => Color::Rgb(0xf4, 0x78, 0x68),    // apricot
    Text | Unit | Color | File              => Color::Rgb(0xcc, 0xcc, 0xcc),    // silver
  }
}

fn completion_menu_detail_text(item: &LspCompletionItem) -> Option<String> {
  item
    .detail
    .as_ref()
    .map(|value| value.trim())
    .filter(|value| !value.is_empty())
    .map(ToOwned::to_owned)
}

fn completion_menu_documentation_text(item: &LspCompletionItem) -> Option<String> {
  item
    .documentation
    .as_deref()
    .and_then(|value| format_completion_documentation(value, 8, 88))
}

fn format_completion_documentation(
  value: &str,
  max_lines: usize,
  max_chars_per_line: usize,
) -> Option<String> {
  if max_lines == 0 || max_chars_per_line == 0 {
    return None;
  }
  let mut lines = Vec::new();
  for line in value.lines() {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("```") {
      continue;
    }
    lines.push(clamp_status_text(trimmed, max_chars_per_line));
    if lines.len() >= max_lines {
      break;
    }
  }
  if lines.is_empty() {
    None
  } else {
    Some(lines.join("\n"))
  }
}

fn merge_resolved_completion_item(current: &mut LspCompletionItem, resolved: LspCompletionItem) {
  if current.filter_text.is_none() {
    current.filter_text = resolved.filter_text;
  }
  if current.sort_text.is_none() {
    current.sort_text = resolved.sort_text;
  }
  if !current.preselect {
    current.preselect = resolved.preselect;
  }
  if current.detail.is_none() {
    current.detail = resolved.detail;
  }
  if current.documentation.is_none() {
    current.documentation = resolved.documentation;
  }
  if current.kind.is_none() {
    current.kind = resolved.kind;
  }
  if current.primary_edit.is_none() {
    current.primary_edit = resolved.primary_edit;
  }
  if current.additional_edits.is_empty() {
    current.additional_edits = resolved.additional_edits;
  }
  if current.insert_text.is_none() {
    current.insert_text = resolved.insert_text;
  }
  if current.insert_text_format.is_none() {
    current.insert_text_format = resolved.insert_text_format;
  }
  if current.commit_characters.is_empty() {
    current.commit_characters = resolved.commit_characters;
  }
}

fn normalize_completion_item_for_apply(mut item: LspCompletionItem) -> LspCompletionItem {
  if item.insert_text_format == Some(LspInsertTextFormat::Snippet) {
    if let Some(insert_text) = item.insert_text.as_mut() {
      let rendered = render_lsp_snippet_fallback(insert_text);
      *insert_text = rendered;
    }
    if let Some(primary_edit) = item.primary_edit.as_mut() {
      let rendered = render_lsp_snippet_fallback(&primary_edit.new_text);
      primary_edit.new_text = rendered;
    }
    for additional in &mut item.additional_edits {
      let rendered = render_lsp_snippet_fallback(&additional.new_text);
      additional.new_text = rendered;
    }
  }
  item
}

fn completion_item_accepts_commit_char(item: &LspCompletionItem, ch: char) -> bool {
  item.commit_characters.iter().any(|candidate| {
    let mut chars = candidate.chars();
    matches!(chars.next(), Some(first) if first == ch) && chars.next().is_none()
  })
}

fn completion_item_filter_text(item: &LspCompletionItem) -> &str {
  item.filter_text.as_deref().unwrap_or(&item.label)
}

fn completion_item_sort_key(item: &LspCompletionItem) -> String {
  item
    .sort_text
    .as_deref()
    .unwrap_or(completion_item_filter_text(item))
    .to_ascii_lowercase()
}

fn completion_match_score(filter: &str, candidate: &str) -> Option<u32> {
  if filter.is_empty() {
    return Some(0);
  }

  let filter = filter.to_ascii_lowercase();
  let candidate = candidate.to_ascii_lowercase();
  if candidate.is_empty() {
    return None;
  }

  if candidate.starts_with(&filter) {
    let extra = candidate.len().saturating_sub(filter.len()) as u32;
    return Some(10_000u32.saturating_sub(extra.min(2_000)));
  }

  if let Some(pos) = candidate.find(&filter) {
    return Some(6_000u32.saturating_sub((pos as u32).saturating_mul(16)));
  }

  let mut candidate_chars = candidate.chars().enumerate();
  let mut last = 0usize;
  let mut gaps = 0usize;
  let mut matched = false;
  for needle in filter.chars() {
    let mut found = None;
    for (idx, hay) in candidate_chars.by_ref() {
      if hay == needle {
        found = Some(idx);
        break;
      }
    }
    let idx = found?;
    if matched {
      gaps += idx.saturating_sub(last + 1);
    }
    last = idx;
    matched = true;
  }

  Some(2_000u32.saturating_sub((gaps as u32).saturating_mul(8)))
}

fn render_lsp_snippet_fallback(source: &str) -> String {
  let chars: Vec<char> = source.chars().collect();
  let (rendered, _) = render_snippet_fragment(&chars, 0, None);
  rendered
}

fn render_snippet_fragment(
  chars: &[char],
  mut index: usize,
  terminator: Option<char>,
) -> (String, usize) {
  let mut out = String::new();
  while index < chars.len() {
    let ch = chars[index];
    if terminator == Some(ch) {
      return (out, index + 1);
    }
    if ch == '\\' {
      if let Some(next) = chars.get(index + 1).copied() {
        out.push(next);
        index += 2;
      } else {
        index += 1;
      }
      continue;
    }
    if ch == '$'
      && let Some((rendered, next_index)) = parse_snippet_dollar(chars, index)
    {
      out.push_str(&rendered);
      index = next_index;
      continue;
    }
    out.push(ch);
    index += 1;
  }
  (out, index)
}

fn parse_snippet_dollar(chars: &[char], index: usize) -> Option<(String, usize)> {
  let next = *chars.get(index + 1)?;
  if next.is_ascii_digit() {
    let mut cursor = index + 1;
    while chars
      .get(cursor)
      .copied()
      .is_some_and(|value| value.is_ascii_digit())
    {
      cursor += 1;
    }
    return Some((String::new(), cursor));
  }
  if next == '{' {
    return Some(parse_snippet_braced(chars, index + 2));
  }
  if is_snippet_identifier_char(next) {
    let mut cursor = index + 1;
    while chars
      .get(cursor)
      .copied()
      .is_some_and(is_snippet_identifier_char)
    {
      cursor += 1;
    }
    return Some((String::new(), cursor));
  }
  None
}

fn parse_snippet_braced(chars: &[char], mut index: usize) -> (String, usize) {
  let start = index;
  while chars
    .get(index)
    .copied()
    .is_some_and(is_snippet_identifier_char)
  {
    index += 1;
  }
  if index == start {
    return (String::new(), index);
  }
  match chars.get(index).copied() {
    Some('}') => (String::new(), index + 1),
    Some(':') => render_snippet_fragment(chars, index + 1, Some('}')),
    Some('|') => parse_snippet_choice(chars, index + 1),
    Some(_) => {
      let mut cursor = index;
      while chars.get(cursor).copied() != Some('}') {
        if cursor >= chars.len() {
          return (String::new(), cursor);
        }
        cursor += 1;
      }
      (String::new(), cursor + 1)
    },
    None => (String::new(), index),
  }
}

fn parse_snippet_choice(chars: &[char], mut index: usize) -> (String, usize) {
  let mut first_choice: Option<String> = None;
  let mut current = String::new();
  let mut escaped = false;
  while index < chars.len() {
    let ch = chars[index];
    if escaped {
      current.push(ch);
      escaped = false;
      index += 1;
      continue;
    }
    if ch == '\\' {
      escaped = true;
      index += 1;
      continue;
    }
    if ch == ',' {
      if first_choice.is_none() {
        first_choice = Some(current.clone());
      }
      current.clear();
      index += 1;
      continue;
    }
    if ch == '|' && chars.get(index + 1).copied() == Some('}') {
      if first_choice.is_none() {
        first_choice = Some(current);
      }
      return (first_choice.unwrap_or_default(), index + 2);
    }
    current.push(ch);
    index += 1;
  }
  (first_choice.unwrap_or(current), index)
}

fn is_snippet_identifier_char(ch: char) -> bool {
  ch.is_ascii_alphanumeric() || ch == '_'
}

fn format_lsp_progress_text(title: Option<&str>, message: Option<&str>) -> String {
  let title = title.map(str::trim).filter(|title| !title.is_empty());
  let message = message.map(str::trim).filter(|message| !message.is_empty());
  match (title, message) {
    (Some(title), Some(message)) => format!("{title}: {message}"),
    (Some(title), None) => title.to_string(),
    (None, Some(message)) => message.to_string(),
    (None, None) => "work".to_string(),
  }
}

fn summarize_lsp_error(message: &str) -> String {
  if message.contains("No such file or directory") {
    return "command not found".to_string();
  }
  if message.contains("server closed stdio") || message.contains("closed the stream") {
    return "server exited".to_string();
  }
  if message.contains("initialize request timed out") {
    return "initialize timeout".to_string();
  }
  clamp_status_text(message, 24)
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

  fn lsp_statusline_text(&self) -> Option<String> {
    self.lsp_statusline_text_value()
  }

  fn vcs_statusline_text(&self) -> Option<String> {
    self.vcs_statusline.clone()
  }

  fn watch_statusline_text(&self) -> Option<String> {
    self
      .lsp_watched_file
      .as_ref()
      .and_then(|watch| watch_statusline_text_for_state(watch.stream.reload_state))
  }

  fn watch_conflict_active(&self) -> bool {
    self
      .lsp_watched_file
      .as_ref()
      .is_some_and(|watch| watch.stream.reload_state == FileWatchReloadState::Conflict)
  }

  fn clear_watch_conflict(&mut self) {
    if let Some(watch) = self.lsp_watched_file.as_mut() {
      clear_reload_state(&mut watch.stream.reload_state);
    }
  }

  fn apply_transaction(&mut self, transaction: &Transaction) -> bool {
    enum SyntaxParseHighlightUpdate {
      Parsed,
      Interpolated,
    }

    let old_text_for_lsp = self.editor.document().text().clone();
    let loader = self.loader.clone();
    let mut async_parse_job: Option<SyntaxParseJob> = None;
    let mut async_parse_doc_version = None;
    let mut syntax_highlight_update: Option<SyntaxParseHighlightUpdate> = None;
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

        if let Some(syntax) = doc.syntax_mut() {
          match syntax.try_update_with_short_timeout(
            new_text.slice(..),
            &edits,
            loader.as_ref(),
            Duration::from_millis(3),
          ) {
            Ok(true) => {
              bump_syntax_version = true;
              syntax_highlight_update = Some(SyntaxParseHighlightUpdate::Parsed);
            },
            Ok(false) => {
              syntax.interpolate_with_edits(&edits);
              bump_syntax_version = true;
              syntax_highlight_update = Some(SyntaxParseHighlightUpdate::Interpolated);
              let mut parse_syntax = syntax.clone();
              let parse_source = new_text.clone();
              let parse_loader = loader.clone();
              let parse_edits = edits.clone();
              async_parse_doc_version = Some(doc.version());
              async_parse_job = Some(Box::new(move || {
                parse_syntax
                  .update_with_edits(parse_source.slice(..), &parse_edits, parse_loader.as_ref())
                  .ok()
                  .map(|()| parse_syntax)
              }));
            },
            Err(_) => {
              syntax.interpolate_with_edits(&edits);
              bump_syntax_version = true;
              syntax_highlight_update = Some(SyntaxParseHighlightUpdate::Interpolated);
              let mut parse_syntax = syntax.clone();
              let parse_source = new_text.clone();
              let parse_loader = loader.clone();
              let parse_edits = edits.clone();
              async_parse_doc_version = Some(doc.version());
              async_parse_job = Some(Box::new(move || {
                parse_syntax
                  .update_with_edits(parse_source.slice(..), &parse_edits, parse_loader.as_ref())
                  .ok()
                  .map(|()| parse_syntax)
              }));
            },
          }
        }

        if bump_syntax_version {
          doc.bump_syntax_version();
        }
      }
    }

    if let (Some(parse_job), Some(doc_version)) = (async_parse_job, async_parse_doc_version) {
      self.queue_syntax_parse_job(doc_version, parse_job);
    }

    if let Some(update) = syntax_highlight_update {
      match update {
        SyntaxParseHighlightUpdate::Parsed => self.syntax_parse_highlight_state.mark_parsed(),
        SyntaxParseHighlightUpdate::Interpolated => {
          self.syntax_parse_highlight_state.mark_interpolated();
        },
      }
    }

    self.lsp_send_did_change(&old_text_for_lsp, transaction.changes());
    self.refresh_vcs_diff_document();

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

  fn completion_menu(&self) -> &the_default::CompletionMenuState {
    &self.completion_menu
  }

  fn completion_menu_mut(&mut self) -> &mut the_default::CompletionMenuState {
    &mut self.completion_menu
  }

  fn completion_selection_changed(&mut self, index: usize) {
    let source_index = self
      .completion_source_index_for_visible_index(index)
      .unwrap_or(index);
    self.resolve_completion_item_if_needed(source_index);
  }

  fn completion_accept_on_commit_char(&mut self, ch: char) -> bool {
    let Some(selected) = self.completion_menu.selected else {
      return false;
    };
    let source_index = self
      .completion_source_index_for_visible_index(selected)
      .unwrap_or(selected);
    let should_accept = self
      .lsp_completion_items
      .get(source_index)
      .is_some_and(|item| completion_item_accepts_commit_char(item, ch));
    if should_accept {
      the_default::completion_accept(self);
      return true;
    }
    false
  }

  fn completion_on_action(&mut self, command: Command) -> bool {
    self.handle_completion_action(command)
  }

  fn completion_accept_selected(&mut self, index: usize) -> bool {
    let source_index = self
      .completion_source_index_for_visible_index(index)
      .unwrap_or(index);
    let Some(item) = self.lsp_completion_items.get(source_index).cloned() else {
      return false;
    };

    let fallback_end = self
      .editor
      .document()
      .selection()
      .ranges()
      .first()
      .map(|range| range.cursor(self.editor.document().text().slice(..)))
      .unwrap_or(0);
    let fallback_start = self
      .lsp_completion_fallback_start
      .unwrap_or(fallback_end)
      .min(fallback_end);
    let applied = self.apply_completion_item(item, fallback_start..fallback_end);
    if applied {
      self.clear_completion_state();
      self.cancel_auto_completion();
    }
    applied
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

  fn soft_wrap_enabled(&self) -> bool {
    self.text_format.soft_wrap
  }

  fn set_soft_wrap_enabled(&mut self, enabled: bool) {
    self.text_format.soft_wrap = enabled;
    if enabled {
      self.editor.view_mut().scroll.col = 0;
    }
  }

  fn gutter_config(&self) -> &GutterConfig {
    &self.gutter_config
  }

  fn gutter_config_mut(&mut self) -> &mut GutterConfig {
    &mut self.gutter_config
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
    self.refresh_vcs_diff_base();
  }

  fn log_target_names(&self) -> &'static [&'static str] {
    &["messages", "lsp", "watch"]
  }

  fn log_path_for_target(&self, target: &str) -> Option<PathBuf> {
    match target {
      "messages" => resolve_message_log_path(),
      "lsp" => resolve_lsp_trace_log_path(),
      "watch" => resolve_file_watch_trace_log_path(),
      _ => None,
    }
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
    self.cancel_auto_completion();
    let _ = self.dispatch_completion_request(CompletionTriggerSource::Manual, true);
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
    if let Some(watch) = self.lsp_watched_file.as_mut() {
      watch.stream.suppress_until = Some(Instant::now() + lsp_self_save_suppress_window());
      clear_reload_state(&mut watch.stream.reload_state);
    }
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

    self.syntax_parse_lifecycle.cancel_pending();
    self.highlight_cache.clear();
    if self.editor.document().syntax().is_some() {
      self.syntax_parse_highlight_state.mark_parsed();
    } else {
      self.syntax_parse_highlight_state.mark_cleared();
    }

    <Self as the_default::DefaultContext>::set_file_path(self, Some(path.to_path_buf()));
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

#[cfg(test)]
mod tests {
  use std::{
    fs,
    path::{
      Path,
      PathBuf,
    },
    sync::mpsc::{
      Sender,
      channel,
    },
    thread,
    time::{
      Duration,
      SystemTime,
    },
  };

  use the_default::{
    CommandEvent,
    DefaultContext,
    Key,
    KeyEvent,
    Mode,
    Modifiers,
    handle_key,
  };
  use the_lib::{
    messages::MessageEventKind,
    position::{
      Position,
      char_idx_at_coords,
      coords_at_pos,
    },
    selection::Selection,
    transaction::Transaction,
  };
  use the_lsp::{
    LspCompletionItem,
    LspInsertTextFormat,
  };
  use the_runtime::file_watch::{
    PathEvent,
    PathEventKind,
  };

  use super::{
    Ctx,
    WatchedFileEventsState,
    completion_item_accepts_commit_char,
    completion_match_score,
    completion_menu_detail_text,
    completion_menu_documentation_text,
    merge_resolved_completion_item,
    render_lsp_snippet_fallback,
  };
  use crate::{
    dispatch::build_dispatch,
    render::{
      build_render_plan,
      ensure_cursor_visible,
    },
  };

  struct TempTestFile {
    path: PathBuf,
  }

  fn empty_completion_item() -> LspCompletionItem {
    LspCompletionItem {
      label:              "item".to_string(),
      filter_text:        None,
      sort_text:          None,
      preselect:          false,
      detail:             None,
      documentation:      None,
      kind:               None,
      primary_edit:       None,
      additional_edits:   Vec::new(),
      insert_text:        None,
      insert_text_format: Some(LspInsertTextFormat::PlainText),
      commit_characters:  Vec::new(),
    }
  }

  #[test]
  fn snippet_fallback_renders_placeholders_and_choices() {
    assert_eq!(
      render_lsp_snippet_fallback("foo($1, ${2:bar}, ${3|x,y|})$0"),
      "foo(, bar, x)"
    );
    assert_eq!(
      render_lsp_snippet_fallback("${TM_FILENAME:main}.rs"),
      "main.rs"
    );
    assert_eq!(render_lsp_snippet_fallback("a\\$b\\}"), "a$b}");
  }

  #[test]
  fn completion_commit_characters_match_single_character_entries() {
    let mut item = empty_completion_item();
    item.commit_characters = vec![";".into(), "::".into()];
    assert!(completion_item_accepts_commit_char(&item, ';'));
    assert!(!completion_item_accepts_commit_char(&item, ':'));
  }

  #[test]
  fn completion_menu_detail_uses_only_item_detail() {
    let mut item = empty_completion_item();
    item.detail = Some("fn(item)".to_string());
    item.documentation = Some("line one\nline two".to_string());
    assert_eq!(
      completion_menu_detail_text(&item).as_deref(),
      Some("fn(item)")
    );
  }

  #[test]
  fn completion_menu_documentation_compacts_markdown_blocks() {
    let mut item = empty_completion_item();
    item.documentation = Some("```rust\nfn test() {}\n```\n\nMore details".to_string());
    assert_eq!(
      completion_menu_documentation_text(&item).as_deref(),
      Some("fn test() {}\nMore details")
    );
  }

  #[test]
  fn completion_match_score_prefers_prefix_and_rejects_unrelated_candidates() {
    let prefix = completion_match_score("std", "std::").expect("prefix score");
    let fuzzy = completion_match_score("std", "serde_std_types").expect("fuzzy score");
    assert!(prefix > fuzzy);
    assert!(completion_match_score("std", "command_palette").is_none());
  }

  #[test]
  fn merge_resolved_completion_item_keeps_existing_and_fills_missing_fields() {
    let mut current = empty_completion_item();
    current.detail = Some("existing".to_string());

    let mut resolved = empty_completion_item();
    resolved.documentation = Some("docs".to_string());
    resolved.commit_characters = vec![";".to_string()];
    resolved.insert_text = Some("insert".to_string());

    merge_resolved_completion_item(&mut current, resolved);

    assert_eq!(current.detail.as_deref(), Some("existing"));
    assert_eq!(current.documentation.as_deref(), Some("docs"));
    assert_eq!(current.commit_characters, vec![";".to_string()]);
    assert_eq!(current.insert_text.as_deref(), Some("insert"));
  }

  #[test]
  fn rebuild_completion_menu_filters_to_matching_items() {
    let mut ctx = Ctx::new(None).expect("ctx");
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("use std".into()))),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));
    let cursor = ctx.editor.document().text().len_chars();
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::point(cursor));

    let mut low = empty_completion_item();
    low.label = "command".to_string();
    low.filter_text = Some("command".to_string());
    low.sort_text = Some("zzz".to_string());

    let mut top = empty_completion_item();
    top.label = "std".to_string();
    top.filter_text = Some("std".to_string());
    top.sort_text = Some("aaa".to_string());

    ctx.lsp_completion_items = vec![low, top];
    ctx.lsp_completion_fallback_start = Some("use ".chars().count());
    ctx.rebuild_completion_menu();

    assert!(ctx.completion_menu.active);
    assert_eq!(ctx.completion_menu.items.len(), 1);
    assert_eq!(ctx.completion_menu.items[0].label, "std");
  }

  #[test]
  fn completion_accept_selected_replaces_request_prefix_range() {
    let mut ctx = Ctx::new(None).expect("ctx");
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("say he".into()))),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    let cursor = ctx.editor.document().text().len_chars();
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::point(cursor));

    let mut item = empty_completion_item();
    item.insert_text = Some("hello".to_string());
    ctx.lsp_completion_items = vec![item];
    ctx.lsp_completion_fallback_start = Some("say ".chars().count());

    assert!(<Ctx as DefaultContext>::completion_accept_selected(
      &mut ctx, 0
    ));
    assert_eq!(ctx.editor.document().text().to_string(), "say hello");
  }

  #[test]
  fn completion_accept_undo_redo_keeps_syntax_and_render_stable() {
    let fixture =
      TempTestFile::with_extension("completion-undo-redo", "rs", "fn main() {\n  le\n}\n");
    let dispatch = build_dispatch::<Ctx>();
    let mut ctx = Ctx::new(Some(
      fixture
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");
    ctx.set_dispatch(&dispatch);
    assert!(ctx.editor.document().syntax().is_some());

    let (cursor, replace_start) = {
      let text = ctx.editor.document().text().slice(..);
      (
        char_idx_at_coords(text, Position::new(1, 4)),
        char_idx_at_coords(text, Position::new(1, 2)),
      )
    };
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::point(cursor));
    let before_text = ctx.editor.document().text().to_string();
    let syntax_version_before = ctx.editor.document().syntax_version();

    let mut item = empty_completion_item();
    item.insert_text = Some("let".to_string());
    ctx.lsp_completion_items = vec![item];
    ctx.lsp_completion_fallback_start = Some(replace_start);

    assert!(<Ctx as DefaultContext>::completion_accept_selected(
      &mut ctx, 0
    ));
    let after_accept_text = ctx.editor.document().text().to_string();
    assert_eq!(after_accept_text, "fn main() {\n  let\n}\n");
    assert!(ctx.editor.document().syntax().is_some());
    assert!(ctx.editor.document().syntax_version() > syntax_version_before);
    let accept_plan = build_render_plan(&mut ctx);
    assert!(!accept_plan.lines.is_empty());

    let dispatch_ref = ctx.dispatch();
    dispatch_ref.undo(&mut ctx, 1);
    assert_eq!(ctx.editor.document().text().to_string(), before_text);
    assert!(ctx.editor.document().syntax().is_some());
    let undo_plan = build_render_plan(&mut ctx);
    assert!(!undo_plan.lines.is_empty());

    let dispatch_ref = ctx.dispatch();
    dispatch_ref.redo(&mut ctx, 1);
    assert_eq!(ctx.editor.document().text().to_string(), after_accept_text);
    assert!(ctx.editor.document().syntax().is_some());
    let redo_plan = build_render_plan(&mut ctx);
    assert!(!redo_plan.lines.is_empty());
  }

  impl TempTestFile {
    fn new(prefix: &str, content: &str) -> Self {
      Self::with_extension(prefix, "txt", content)
    }

    fn with_extension(prefix: &str, extension: &str, content: &str) -> Self {
      let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
      let extension = extension.trim_start_matches('.');
      let path = std::env::temp_dir().join(format!(
        "the-editor-{prefix}-{}-{nonce}.{extension}",
        std::process::id(),
      ));
      fs::write(&path, content).expect("write temp test file");
      Self { path }
    }

    fn as_path(&self) -> &Path {
      &self.path
    }
  }

  impl Drop for TempTestFile {
    fn drop(&mut self) {
      let _ = fs::remove_file(&self.path);
    }
  }

  fn install_test_watch_state(ctx: &mut Ctx, path: &Path) -> Sender<Vec<PathEvent>> {
    let (events_tx, events_rx) = channel();
    let (_unused_rx, watch_handle) = super::watch_path(path, Duration::from_millis(0));
    let uri = the_lsp::text_sync::file_uri_for_path(path).expect("file uri");
    ctx.lsp_watched_file = Some(super::LspWatchedFileState {
      stream:        WatchedFileEventsState {
        path: path.to_path_buf(),
        uri,
        events_rx,
        suppress_until: None,
        reload_state: super::FileWatchReloadState::Clean,
        reload_io: super::FileWatchReloadIoState::default(),
      },
      _watch_handle: watch_handle,
    });
    events_tx
  }

  #[derive(Debug, Clone, Copy)]
  struct SimRng {
    state: u64,
  }

  impl SimRng {
    fn new(seed: u64) -> Self {
      Self { state: seed.max(1) }
    }

    fn next_u64(&mut self) -> u64 {
      let mut x = self.state;
      x ^= x << 13;
      x ^= x >> 7;
      x ^= x << 17;
      self.state = x;
      x
    }

    fn next_usize(&mut self, upper: usize) -> usize {
      if upper == 0 {
        0
      } else {
        (self.next_u64() as usize) % upper
      }
    }
  }

  fn fixture_matrix() -> [(&'static str, String); 5] {
    [
      (
        "fixture.rs",
        r#"fn main() {
    let greeting = "hello🙂";
    let mut total = 0;
    for value in [1, 2, 3, 4] {
        total += value;
    }
    println!("{greeting} {total}");
}
"#
        .repeat(18),
      ),
      (
        "fixture.md",
        r#"# heading

- alpha
- beta
- gamma

```rust
fn fenced() {}
```
"#
        .repeat(20),
      ),
      (
        "fixture.toml",
        r#"[package]
name = "fixture"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = "1"
"#
        .repeat(16),
      ),
      (
        "fixture.nix",
        r#"{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  buildInputs = with pkgs; [ rustc cargo ];
}
"#
        .repeat(18),
      ),
      (
        "fixture.txt",
        "unicode: 🙂🚀 café e\u{301} こんにちは Привет عربى हिन्दी\n".repeat(28),
      ),
    ]
  }

  fn next_edit(rng: &mut SimRng, len_chars: usize) -> (usize, usize, Option<&'static str>) {
    const TOKENS: &[&str] = &[
      "a", "_", " ", "\n", "{}", "let ", "fn ", "🙂", "é", "λ", "->", "\"",
    ];

    let op = if len_chars == 0 { 0 } else { rng.next_usize(3) };
    match op {
      0 => {
        let at = rng.next_usize(len_chars.saturating_add(1));
        let insert = TOKENS[rng.next_usize(TOKENS.len())];
        (at, at, Some(insert))
      },
      1 => {
        let from = rng.next_usize(len_chars);
        let span = 1 + rng.next_usize((len_chars - from).min(6));
        (from, from + span, None)
      },
      _ => {
        let from = rng.next_usize(len_chars);
        let span = 1 + rng.next_usize((len_chars - from).min(6));
        let insert = TOKENS[rng.next_usize(TOKENS.len())];
        (from, from + span, Some(insert))
      },
    }
  }

  #[test]
  fn headless_client_stress_fixture_matrix() {
    let dispatch = build_dispatch::<Ctx>();

    for (fixture_index, (fixture_name, fixture_text)) in fixture_matrix().into_iter().enumerate() {
      let mut ctx = Ctx::new(None).expect("ctx");
      ctx.set_dispatch(&dispatch);

      let initial = Transaction::change(
        ctx.editor.document().text(),
        std::iter::once((0, 0, Some(fixture_text.into()))),
      )
      .expect("initial transaction");
      assert!(DefaultContext::apply_transaction(&mut ctx, &initial));

      if let Some(loader) = ctx.loader.clone() {
        let _ = super::setup_syntax(ctx.editor.document_mut(), Path::new(fixture_name), &loader);
      }

      let mut rng = SimRng::new(0xFACE_B00C ^ fixture_index as u64);
      for step in 0..96usize {
        let current = ctx.editor.document().text().clone();
        let (from, to, insert) = next_edit(&mut rng, current.len_chars());
        let tx = Transaction::change(
          &current,
          std::iter::once((from, to, insert.map(|text| text.into()))),
        )
        .expect("edit transaction");
        assert!(
          DefaultContext::apply_transaction(&mut ctx, &tx),
          "failed apply for fixture={fixture_name} step={step}"
        );

        if step % 4 == 0 {
          for _ in 0..3 {
            let _ = ctx.poll_syntax_parse_results();
            thread::sleep(Duration::from_millis(1));
          }
        }

        let plan = build_render_plan(&mut ctx);
        assert!(
          !plan.lines.is_empty(),
          "empty render plan for fixture={fixture_name} step={step}"
        );
      }

      for _ in 0..12 {
        let _ = ctx.poll_syntax_parse_results();
        let plan = build_render_plan(&mut ctx);
        assert!(
          !plan.lines.is_empty(),
          "empty render plan during settle for fixture={fixture_name}"
        );
        thread::sleep(Duration::from_millis(1));
      }
    }
  }

  #[test]
  fn wrap_command_toggles_soft_wrap_and_changes_render_lines() {
    let dispatch = build_dispatch::<Ctx>();
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.set_dispatch(&dispatch);
    ctx.resize(24, 12);

    let long_line = "wrap-me-".repeat(40);
    let initial_tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some(long_line.into()))),
    )
    .expect("initial transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &initial_tx));

    assert!(!ctx.soft_wrap_enabled());
    let no_wrap_plan = build_render_plan(&mut ctx);
    assert_eq!(no_wrap_plan.lines.len(), 1);

    let registry = ctx.command_registry_ref() as *const the_default::CommandRegistry<Ctx>;
    unsafe { (&*registry).execute(&mut ctx, "wrap", "on", CommandEvent::Validate) }
      .expect("wrap on");
    assert!(ctx.soft_wrap_enabled());

    let wrapped_plan = build_render_plan(&mut ctx);
    assert!(wrapped_plan.lines.len() > no_wrap_plan.lines.len());

    unsafe { (&*registry).execute(&mut ctx, "wrap", "status", CommandEvent::Validate) }
      .expect("wrap status");
    assert!(ctx.soft_wrap_enabled());

    unsafe { (&*registry).execute(&mut ctx, "wrap", "toggle", CommandEvent::Validate) }
      .expect("wrap toggle");
    assert!(!ctx.soft_wrap_enabled());

    let toggled_plan = build_render_plan(&mut ctx);
    assert_eq!(toggled_plan.lines.len(), no_wrap_plan.lines.len());
  }

  #[test]
  fn gutter_and_line_number_commands_update_config() {
    let dispatch = build_dispatch::<Ctx>();
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.set_dispatch(&dispatch);

    let registry = ctx.command_registry_ref() as *const the_default::CommandRegistry<Ctx>;

    assert!(!ctx.gutter_config.layout.is_empty());

    unsafe { (&*registry).execute(&mut ctx, "gutter", "off", CommandEvent::Validate) }
      .expect("gutter off");
    assert!(ctx.gutter_config.layout.is_empty());

    unsafe { (&*registry).execute(&mut ctx, "line-number", "relative", CommandEvent::Validate) }
      .expect("line-number relative");
    assert!(
      ctx
        .gutter_config
        .layout
        .contains(&the_lib::render::GutterType::LineNumbers)
    );
    assert_eq!(
      ctx.gutter_config.line_numbers.mode,
      the_lib::render::LineNumberMode::Relative
    );

    unsafe { (&*registry).execute(&mut ctx, "line-number", "off", CommandEvent::Validate) }
      .expect("line-number off");
    assert!(
      !ctx
        .gutter_config
        .layout
        .contains(&the_lib::render::GutterType::LineNumbers)
    );
  }

  #[test]
  fn ensure_cursor_visible_keeps_horizontal_scroll_zero_with_soft_wrap() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.resize(24, 12);
    DefaultContext::set_soft_wrap_enabled(&mut ctx, true);

    let long_line = "horizontal-scroll-".repeat(20);
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some(long_line.into()))),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    ctx.editor.view_mut().scroll.col = 40;
    ensure_cursor_visible(&mut ctx);
    assert_eq!(ctx.editor.view().scroll.col, 0);
  }

  #[test]
  fn set_file_path_reconfigures_lsp_server_for_rust_files() {
    let txt_fixture = TempTestFile::with_extension("lsp-config", "txt", "plain text\n");
    let rust_fixture = TempTestFile::with_extension("lsp-config", "rs", "fn main() {}\n");
    let mut ctx = Ctx::new(Some(
      txt_fixture
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    <Ctx as DefaultContext>::set_file_path(&mut ctx, Some(rust_fixture.as_path().to_path_buf()));

    let server_name = ctx
      .lsp_runtime
      .config()
      .server()
      .map(|server| server.name().to_string());
    assert_eq!(server_name.as_deref(), Some("rust-analyzer"));
  }

  #[test]
  fn set_file_path_reconfigures_running_lsp_runtime() {
    let rust_fixture = TempTestFile::with_extension("lsp-running-reconfig", "rs", "fn main() {}\n");
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.start_background_services();
    assert!(ctx.lsp_runtime.is_running());

    <Ctx as DefaultContext>::set_file_path(&mut ctx, Some(rust_fixture.as_path().to_path_buf()));

    let server_name = ctx
      .lsp_runtime
      .config()
      .server()
      .map(|server| server.name().to_string());
    assert_eq!(server_name.as_deref(), Some("rust-analyzer"));
    assert!(ctx.lsp_runtime.is_running());
    ctx.shutdown_background_services();
  }

  #[test]
  fn reload_preserves_cursor_and_scroll_semantically_after_external_edit() {
    let fixture = TempTestFile::new("semantic-reload", "zero\none\ntwo\nthree\n");
    let mut ctx = Ctx::new(Some(
      fixture
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    let cursor = {
      let text = ctx.editor.document().text().slice(..);
      char_idx_at_coords(text, Position::new(2, 1))
    };
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::point(cursor));
    ctx.editor.view_mut().scroll = Position::new(2, 7);

    let before_cursor_coords = {
      let text = ctx.editor.document().text().slice(..);
      let head = ctx.editor.document().selection().ranges()[0].head;
      coords_at_pos(text, head)
    };
    let before_scroll = ctx.editor.view().scroll;

    fs::write(fixture.as_path(), "inserted\nzero\none\ntwo\nthree\n").expect("update fixture");
    <Ctx as DefaultContext>::reload_file_preserving_view(&mut ctx, fixture.as_path())
      .expect("reload preserving view");

    let after_cursor_coords = {
      let text = ctx.editor.document().text().slice(..);
      let head = ctx.editor.document().selection().ranges()[0].head;
      coords_at_pos(text, head)
    };

    assert_eq!(
      ctx.editor.document().text().to_string(),
      "inserted\nzero\none\ntwo\nthree\n"
    );
    assert_eq!(after_cursor_coords, before_cursor_coords);
    assert_eq!(ctx.editor.view().scroll, before_scroll);
  }

  #[test]
  fn dirty_buffer_external_change_keeps_buffer_and_warns() {
    let fixture = TempTestFile::new("dirty-watch", "alpha\nbeta\n");
    let mut ctx = Ctx::new(Some(
      fixture
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    let local_edit = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("local-".into()))),
    )
    .expect("local edit");
    assert!(DefaultContext::apply_transaction(&mut ctx, &local_edit));
    assert!(ctx.editor.document().flags().modified);
    let dirty_snapshot = ctx.editor.document().text().to_string();

    let watch_tx = install_test_watch_state(&mut ctx, fixture.as_path());
    fs::write(fixture.as_path(), "disk-alpha\ndisk-beta\n").expect("update fixture");
    watch_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send watch event");

    let before_seq = ctx.messages.latest_seq();
    assert!(ctx.poll_lsp_file_watch());
    assert_eq!(ctx.editor.document().text().to_string(), dirty_snapshot);

    let events = ctx.messages.events_since(before_seq);
    let warning = events
      .iter()
      .find_map(|event| {
        match &event.kind {
          MessageEventKind::Published { message } => {
            (message.level == the_lib::messages::MessageLevel::Warning
              && message.source.as_deref() == Some("watch"))
            .then_some(message.text.as_str())
          },
          _ => None,
        }
      })
      .expect("watch warning message");
    assert!(warning.contains("buffer has unsaved changes"));
    assert_eq!(
      <Ctx as DefaultContext>::watch_statusline_text(&ctx).as_deref(),
      Some("watch: conflict")
    );
  }

  #[test]
  fn watch_conflict_discard_command_reloads_and_clears_conflict_state() {
    let fixture = TempTestFile::new("conflict-discard", "alpha\nbeta\n");
    let mut ctx = Ctx::new(Some(
      fixture
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    let local_edit = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("local-".into()))),
    )
    .expect("local edit");
    assert!(DefaultContext::apply_transaction(&mut ctx, &local_edit));

    let watch_tx = install_test_watch_state(&mut ctx, fixture.as_path());
    fs::write(fixture.as_path(), "disk-alpha\ndisk-beta\n").expect("update fixture");
    watch_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send watch event");
    assert!(ctx.poll_lsp_file_watch());
    assert_eq!(
      <Ctx as DefaultContext>::watch_statusline_text(&ctx).as_deref(),
      Some("watch: conflict")
    );

    let registry = ctx.command_registry_ref() as *const the_default::CommandRegistry<Ctx>;
    unsafe {
      (&*registry).execute(
        &mut ctx,
        "watch-conflict",
        "discard",
        CommandEvent::Validate,
      )
    }
    .expect("discard conflict");

    assert_eq!(
      ctx.editor.document().text().to_string(),
      "disk-alpha\ndisk-beta\n"
    );
    assert!(!<Ctx as DefaultContext>::watch_conflict_active(&ctx));
  }

  #[test]
  fn watch_conflict_write_requires_force_and_w_bang_overwrites_disk() {
    let fixture = TempTestFile::new("conflict-write-force", "alpha\nbeta\n");
    let mut ctx = Ctx::new(Some(
      fixture
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    let local_edit = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("local-".into()))),
    )
    .expect("local edit");
    assert!(DefaultContext::apply_transaction(&mut ctx, &local_edit));
    let local_snapshot = ctx.editor.document().text().to_string();

    let watch_tx = install_test_watch_state(&mut ctx, fixture.as_path());
    fs::write(fixture.as_path(), "disk-alpha\ndisk-beta\n").expect("update fixture");
    watch_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send watch event");
    assert!(ctx.poll_lsp_file_watch());
    assert!(<Ctx as DefaultContext>::watch_conflict_active(&ctx));

    let registry = ctx.command_registry_ref() as *const the_default::CommandRegistry<Ctx>;
    let write_err = unsafe { (&*registry).execute(&mut ctx, "write", "", CommandEvent::Validate) }
      .expect_err("write should fail with conflict");
    assert!(write_err.to_string().contains(":w!"));

    unsafe { (&*registry).execute(&mut ctx, "w!", "", CommandEvent::Validate) }
      .expect("force write");
    assert_eq!(
      fs::read_to_string(fixture.as_path()).expect("read disk"),
      local_snapshot
    );
    assert!(!<Ctx as DefaultContext>::watch_conflict_active(&ctx));
  }

  #[test]
  fn watch_conflict_rl_and_rla_aliases_reload_and_clear_conflict() {
    let fixture = TempTestFile::new("conflict-reload-alias", "alpha\nbeta\n");
    let mut ctx = Ctx::new(Some(
      fixture
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    let local_edit = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("local-".into()))),
    )
    .expect("local edit");
    assert!(DefaultContext::apply_transaction(&mut ctx, &local_edit));

    let watch_tx = install_test_watch_state(&mut ctx, fixture.as_path());
    fs::write(fixture.as_path(), "disk-alpha\ndisk-beta\n").expect("update fixture");
    watch_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send watch event");
    assert!(ctx.poll_lsp_file_watch());
    assert!(<Ctx as DefaultContext>::watch_conflict_active(&ctx));

    let registry = ctx.command_registry_ref() as *const the_default::CommandRegistry<Ctx>;
    unsafe { (&*registry).execute(&mut ctx, "rl", "", CommandEvent::Validate) }
      .expect("reload alias");
    assert_eq!(
      ctx.editor.document().text().to_string(),
      "disk-alpha\ndisk-beta\n"
    );
    assert!(!<Ctx as DefaultContext>::watch_conflict_active(&ctx));

    let local_edit_again = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("local-".into()))),
    )
    .expect("local edit");
    assert!(DefaultContext::apply_transaction(
      &mut ctx,
      &local_edit_again
    ));
    fs::write(fixture.as_path(), "disk-gamma\ndisk-delta\n").expect("update fixture");
    watch_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send watch event");
    assert!(ctx.poll_lsp_file_watch());
    assert!(<Ctx as DefaultContext>::watch_conflict_active(&ctx));

    unsafe { (&*registry).execute(&mut ctx, "rla", "", CommandEvent::Validate) }
      .expect("reload-all alias");
    assert_eq!(
      ctx.editor.document().text().to_string(),
      "disk-gamma\ndisk-delta\n"
    );
    assert!(!<Ctx as DefaultContext>::watch_conflict_active(&ctx));
  }

  #[test]
  fn watch_scope_command_reports_active_document_policy() {
    let mut ctx = Ctx::new(None).expect("ctx");
    let before_seq = ctx.messages.latest_seq();
    let registry = ctx.command_registry_ref() as *const the_default::CommandRegistry<Ctx>;
    unsafe { (&*registry).execute(&mut ctx, "watch-scope", "", CommandEvent::Validate) }
      .expect("watch-scope command");

    let events = ctx.messages.events_since(before_seq);
    let info = events
      .iter()
      .find_map(|event| {
        match &event.kind {
          MessageEventKind::Published { message } => {
            (message.level == the_lib::messages::MessageLevel::Info
              && message.source.as_deref() == Some("watch"))
            .then_some(message.text.as_str())
          },
          _ => None,
        }
      })
      .expect("watch-scope info");
    assert!(info.contains("active-document"));
  }

  #[test]
  fn rapid_external_changes_reload_to_latest_on_disk_content() {
    let fixture = TempTestFile::new("rapid-watch", "first\n");
    let mut ctx = Ctx::new(Some(
      fixture
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    let watch_tx = install_test_watch_state(&mut ctx, fixture.as_path());
    fs::write(fixture.as_path(), "second\n").expect("write second");
    watch_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send first event");

    fs::write(fixture.as_path(), "third\n").expect("write third");
    watch_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send second event");

    assert!(ctx.poll_lsp_file_watch());
    assert_eq!(ctx.editor.document().text().to_string(), "third\n");
  }

  #[test]
  fn self_save_suppression_window_ignores_all_events_until_expiry() {
    let fixture = TempTestFile::new("suppression-watch", "one\n");
    let mut ctx = Ctx::new(Some(
      fixture
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    let watch_tx = install_test_watch_state(&mut ctx, fixture.as_path());
    let before = ctx.editor.document().text().to_string();
    if let Some(watch) = ctx.lsp_watched_file.as_mut() {
      watch.stream.suppress_until = Some(std::time::Instant::now() + Duration::from_secs(2));
    } else {
      panic!("expected watch state");
    }

    watch_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send first suppressed event");
    watch_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send second suppressed event");

    assert!(!ctx.poll_lsp_file_watch());
    assert_eq!(ctx.editor.document().text().to_string(), before);
  }

  #[test]
  fn watcher_disconnect_rebinds_and_keeps_processing_changes() {
    let fixture = TempTestFile::new("disconnect-watch", "one\n");
    let mut ctx = Ctx::new(Some(
      fixture
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    let watch_tx = install_test_watch_state(&mut ctx, fixture.as_path());
    drop(watch_tx);

    assert!(!ctx.poll_lsp_file_watch());
    let rebound_watch_path = ctx
      .lsp_watched_file
      .as_ref()
      .map(|watch| watch.stream.path.clone())
      .expect("watch should be rebound");
    assert_eq!(rebound_watch_path, fixture.as_path());

    let rebound_tx = install_test_watch_state(&mut ctx, fixture.as_path());
    fs::write(fixture.as_path(), "two\n").expect("update fixture");
    rebound_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send rebound event");

    assert!(ctx.poll_lsp_file_watch());
    assert_eq!(ctx.editor.document().text().to_string(), "two\n");
  }

  #[test]
  fn normal_x_then_c_performs_linewise_change() {
    let dispatch = build_dispatch::<Ctx>();
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.set_dispatch(&dispatch);

    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("one\nline-two\nthree\n".into()))),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    let line_two_start = ctx.editor.document().text().line_to_char(1);
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::single(line_two_start, line_two_start));

    handle_key(&dispatch, &mut ctx, KeyEvent {
      key:       Key::Char('x'),
      modifiers: Modifiers::empty(),
    });

    let selected = ctx.editor.document().selection().ranges()[0];
    assert_eq!(selected.from(), line_two_start);
    assert_eq!(selected.to(), ctx.editor.document().text().line_to_char(2));

    handle_key(&dispatch, &mut ctx, KeyEvent {
      key:       Key::Char('c'),
      modifiers: Modifiers::empty(),
    });

    assert_eq!(ctx.editor.document().text().to_string(), "one\n\nthree\n");
    assert_eq!(ctx.mode(), Mode::Insert);
  }
}
