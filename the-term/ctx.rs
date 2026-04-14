//! Application context (state).

#[cfg(test)]
use std::fs;

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
    atomic::{
      AtomicBool,
      Ordering,
    },
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
use git2::{
  ObjectType,
  Repository,
};
use ropey::Rope;
use serde_json::{
  Value,
  json,
};
use smallvec::SmallVec;
use the_default::{
  BufferTabsSnapshot,
  Command,
  CommandPaletteState,
  CommandPaletteStyle,
  CommandPromptState,
  CommandRegistry,
  CursorShapes,
  DefaultApi,
  DefaultContext,
  Defaults,
  DispatchRef,
  FilePickerChangedFileItem,
  FilePickerChangedKind,
  FilePickerDiagnosticItem,
  FilePickerItem,
  FilePickerItemAction,
  FilePickerPreview,
  FilePickerState,
  FilePickerVcsDiffEntry,
  FilePickerVcsDiffHunk,
  FilePickerVcsDiffPayload,
  FilePickerVcsDiffPreview,
  FilePickerVcsDiffPreviewRow,
  FilePickerVcsDiffPreviewRowKind,
  FileTreeState,
  FileTreeVcsKind,
  GlobalSearchOptions,
  GlobalSearchState,
  KeyBinding,
  KeyEvent,
  Keymaps,
  Mode,
  Motion,
  PointerButton,
  PointerEvent,
  PointerEventOutcome,
  PointerKind,
  ThemeCatalog,
  WorkingDirectoryState,
  buffer_tabs_snapshot,
  builtin_completion_menu_keymaps,
  builtin_keymaps,
  clear_file_tree_decorations,
  collapse_file_tree_vcs_statuses,
  default_defaults,
  file_picker_items_from_specs,
  file_picker_source_preview_from_text,
  file_picker_vcs_diff_placeholder_entry,
  file_picker_vcs_diff_specs,
  finalize_vcs_diff_preview,
  install_default_wiring,
  open_dynamic_picker,
  rebuild_file_tree_diagnostic_statuses,
  replace_file_picker_items,
  replace_file_picker_items_preserving_selection,
  replace_file_picker_items_preserving_selection_and_viewport,
  set_file_tree_diagnostic_statuses,
  set_file_tree_vcs_statuses,
};
use the_lib::{
  self,
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
    BufferId,
    Editor,
    EditorId,
    PaneContentKind,
    PaneSnapshot,
  },
  indent::IndentStyle,
  messages::{
    MessageCenter,
    MessageDisposition,
    MessageLevel,
  },
  position::Position,
  registers::Registers,
  render::{
    FrameRenderPlan,
    GutterConfig,
    InlineDiagnosticRenderLine,
    RenderGutterDiffKind,
    RenderPlan,
    RenderStyles,
    char_at_visual_pos,
    graphics::Rect,
    gutter_width_for_document,
    text_annotations::{
      InlineAnnotation,
      Overlay,
      TextAnnotations,
    },
    text_format::TextFormat,
    theme::{
      Theme,
      default_theme,
    },
    visual_pos_at_char,
  },
  selection::{
    Range,
    Selection,
  },
  split_tree::{
    PaneId,
    SplitNodeId,
  },
  syntax::{
    HighlightCache,
    Loader,
    Syntax,
  },
  syntax_async::{
    ParseHighlightState,
    ParseLifecycle,
    ParseRequest,
  },
  transaction::{
    Assoc,
    ChangeSet,
    Transaction,
  },
  view::ViewState,
};
use the_lsp::{
  FileChangeType,
  LspCapability,
  LspCodeAction,
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
  LspSignatureHelpContext,
  LspSymbol,
  LspTextEdit,
  LspWorkspaceEdit,
  ServerCapabilitiesSnapshot,
  TextDocumentSyncKind,
  code_action_params,
  completion_params,
  document_highlight_params,
  document_symbols_params,
  execute_command_params,
  formatting_params,
  goto_declaration_params,
  goto_definition_params,
  goto_implementation_params,
  goto_type_definition_params,
  hover_params,
  jsonrpc,
  parse_code_action_response,
  parse_code_actions_response,
  parse_completion_item_response,
  parse_completion_response_with_raw,
  parse_document_highlights_response,
  parse_document_symbols_response,
  parse_formatting_response,
  parse_hover_response,
  parse_locations_response,
  parse_signature_help_response,
  parse_workspace_edit_response,
  parse_workspace_symbols_response,
  references_params,
  rename_params,
  render_lsp_snippet,
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
  FileChange,
  VcsWorkspaceScan,
};

use crate::picker_layout::{
  CompletionDocsLayout,
  FilePickerLayout,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerDragState {
  ListScrollbar { grab_offset: u16 },
  PreviewScrollbar { grab_offset: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionDocsDragState {
  Scrollbar { grab_offset: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneResizeDragState {
  Split { split_id: SplitNodeId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileTreeLayout {
  pub pane_id:       PaneId,
  pub pane:          ratatui::layout::Rect,
  pub header:        ratatui::layout::Rect,
  pub list:          ratatui::layout::Rect,
  pub visible_rows:  usize,
  pub scroll_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointerSelectionDragMode {
  Char,
  Word,
  Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PointerSelectionDragState {
  mode:         PointerSelectionDragMode,
  anchor:       usize,
  initial_from: usize,
  initial_to:   usize,
}

#[derive(Debug, Clone, Copy)]
struct PointerClickTracker {
  at:    Instant,
  x:     u16,
  y:     u16,
  count: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BufferTabPointerDragState {
  pub buffer_id:   BufferId,
  pub pointer_x:   u16,
  pub press_x:     u16,
  pub grab_offset: u16,
  pub moved:       bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BufferTabHoverState {
  pub buffer_id:  BufferId,
  pub over_close: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BufferTabLayoutSlot {
  pub tab_index: usize,
  pub buffer_id: BufferId,
  pub x:         u16,
  pub width:     u16,
  pub close_x:   Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticUnderlineRenderSpan {
  pub row:       u16,
  pub start_col: u16,
  pub end_col:   u16,
  pub severity:  DiagnosticSeverity,
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

type LspRuntimeId = u64;

#[derive(Debug, Clone)]
struct BufferLspState {
  document:             Option<LspDocumentSyncState>,
  attached_runtime_ids: Vec<LspRuntimeId>,
  opened_runtime_ids:   HashSet<LspRuntimeId>,
}

struct ManagedLspRuntime {
  id:                     LspRuntimeId,
  runtime:                LspRuntime,
  ready:                  bool,
  statusline:             LspStatuslineState,
  active_progress_tokens: HashSet<String>,
  pending_requests:       HashMap<u64, PendingLspRequestKind>,
  workspace_folders:      BTreeMap<String, String>,
}

impl ManagedLspRuntime {
  fn configured_server_name(&self) -> Option<&str> {
    self.runtime.config().server().map(|server| server.name())
  }
}

struct LspWatchedFileState {
  stream:        WatchedFileEventsState,
  _watch_handle: WatchHandle,
}

struct VcsWatchState {
  stream:        WatchedFileEventsState,
  _watch_handle: WatchHandle,
}

#[derive(Debug)]
enum VcsDiffPickerFileResult {
  Entry {
    generation: u64,
    index:      usize,
    entry:      FilePickerVcsDiffEntry,
  },
  Complete {
    generation: u64,
  },
}

#[derive(Debug, Clone)]
struct OpenBufferVcsSnapshot {
  text:     String,
  modified: bool,
}

#[derive(Debug, Default)]
struct VcsDiffPickerState {
  generation:           u64,
  scan_generation:      u64,
  live_refresh_pending: bool,
  root:                 PathBuf,
  entries:              Vec<FilePickerVcsDiffEntry>,
  cancel:               Option<Arc<AtomicBool>>,
  result_rx:            Option<Receiver<VcsDiffPickerFileResult>>,
}

enum PickerDiffBaseLoader {
  GitRevision {
    repo:      Repository,
    repo_root: PathBuf,
    revision:  String,
  },
  Provider(DiffProviderRegistry),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileTreeVcsRefreshReason {
  TreeOpen,
  TreeRootChange,
  VcsWatch,
  FileTreeCreatedRemoved,
  FileTreeChangedDebounce,
  WatchRebind,
}

impl FileTreeVcsRefreshReason {
  const fn log_label(self) -> &'static str {
    match self {
      Self::TreeOpen => "tree_open",
      Self::TreeRootChange => "tree_root_change",
      Self::VcsWatch => "vcs_watch",
      Self::FileTreeCreatedRemoved => "file_tree_created_removed",
      Self::FileTreeChangedDebounce => "file_tree_changed_debounce",
      Self::WatchRebind => "watch_rebind",
    }
  }

  const fn priority(self) -> u8 {
    match self {
      Self::TreeOpen => 6,
      Self::TreeRootChange => 5,
      Self::WatchRebind => 4,
      Self::VcsWatch => 3,
      Self::FileTreeCreatedRemoved => 2,
      Self::FileTreeChangedDebounce => 1,
    }
  }

  const fn combine(self, other: Self) -> Self {
    if self.priority() >= other.priority() {
      self
    } else {
      other
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveFileVcsRefreshReason {
  Startup,
  PathChange,
  VcsWatch,
}

impl ActiveFileVcsRefreshReason {
  const fn log_label(self) -> &'static str {
    match self {
      Self::Startup => "startup",
      Self::PathChange => "path_change",
      Self::VcsWatch => "vcs_watch",
    }
  }

  const fn priority(self) -> u8 {
    match self {
      Self::PathChange => 3,
      Self::Startup => 2,
      Self::VcsWatch => 1,
    }
  }

  const fn combine(self, other: Self) -> Self {
    if self.priority() >= other.priority() {
      self
    } else {
      other
    }
  }
}

#[derive(Debug)]
struct ActiveFileVcsRefreshResult {
  generation: u64,
  path:       PathBuf,
  reason:     ActiveFileVcsRefreshReason,
  statusline: Option<String>,
  diff_base:  Option<Vec<u8>>,
  scan:       Option<VcsWorkspaceScan>,
  collect_ms: f64,
}

#[derive(Debug)]
struct FileTreeVcsRefreshResult {
  generation:     u64,
  root:           PathBuf,
  reason:         FileTreeVcsRefreshReason,
  statuses:       BTreeMap<PathBuf, FileTreeVcsKind>,
  change_count:   usize,
  status_entries: usize,
  collect_ms:     f64,
  collapse_ms:    f64,
  scan:           Option<VcsWorkspaceScan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct VcsBaseCacheKey {
  repo_root:     PathBuf,
  head_revision: Option<String>,
  path:          PathBuf,
}

#[derive(Debug, Default, Clone)]
struct SharedVcsState {
  scan:       Option<Arc<VcsWorkspaceScan>>,
  generation: u64,
  base_cache: HashMap<VcsBaseCacheKey, Option<Vec<u8>>>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignatureHelpTriggerSource {
  Manual,
  TriggerCharacter {
    ch:           char,
    is_retrigger: bool,
  },
  ContentChangeRetrigger,
}

impl SignatureHelpTriggerSource {
  fn to_lsp_context(self) -> LspSignatureHelpContext {
    match self {
      Self::Manual => LspSignatureHelpContext::invoked(),
      Self::TriggerCharacter { ch, is_retrigger } => {
        if is_retrigger {
          LspSignatureHelpContext::trigger_character_retrigger(ch)
        } else {
          LspSignatureHelpContext::trigger_character(ch)
        }
      },
      Self::ContentChangeRetrigger => LspSignatureHelpContext::content_change_retrigger(),
    }
  }
}

#[derive(Debug, Clone)]
struct PendingAutoCompletion {
  due_at:  Instant,
  trigger: CompletionTriggerSource,
}

#[derive(Debug, Clone)]
struct PendingAutoSignatureHelp {
  due_at:  Instant,
  trigger: SignatureHelpTriggerSource,
}

#[derive(Debug, Clone, PartialEq)]
enum PendingLspRequestKind {
  GotoDeclaration {
    uri: String,
  },
  GotoDefinition {
    uri: String,
  },
  GotoTypeDefinition {
    uri: String,
  },
  GotoImplementation {
    uri: String,
  },
  Hover {
    uri: String,
  },
  DocumentHighlightSelect {
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
  CodeActionResolve {
    uri:    String,
    action: LspCodeAction,
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
      Self::GotoDeclaration { .. } => "goto-declaration",
      Self::GotoDefinition { .. } => "goto-definition",
      Self::GotoTypeDefinition { .. } => "goto-type-definition",
      Self::GotoImplementation { .. } => "goto-implementation",
      Self::Hover { .. } => "hover",
      Self::DocumentHighlightSelect { .. } => "document-highlight-select",
      Self::References { .. } => "references",
      Self::DocumentSymbols { .. } => "document-symbols",
      Self::WorkspaceSymbols { .. } => "workspace-symbols",
      Self::Completion { .. } => "completion",
      Self::CompletionResolve { .. } => "completion-resolve",
      Self::SignatureHelp { .. } => "signature-help",
      Self::CodeActions { .. } => "code-actions",
      Self::CodeActionResolve { .. } => "code-action-resolve",
      Self::Rename { .. } => "rename",
      Self::Format { .. } => "format",
    }
  }

  fn uri(&self) -> Option<&str> {
    match self {
      Self::GotoDeclaration { uri }
      | Self::GotoTypeDefinition { uri }
      | Self::GotoImplementation { uri }
      | Self::GotoDefinition { uri }
      | Self::Hover { uri }
      | Self::DocumentHighlightSelect { uri }
      | Self::References { uri }
      | Self::DocumentSymbols { uri }
      | Self::Completion { uri, .. }
      | Self::CompletionResolve { uri, .. }
      | Self::SignatureHelp { uri }
      | Self::CodeActions { uri }
      | Self::CodeActionResolve { uri, .. }
      | Self::Rename { uri }
      | Self::Format { uri } => Some(uri.as_str()),
      Self::WorkspaceSymbols { .. } => None,
    }
  }

  fn cancellation_key(&self) -> (&'static str, Option<&str>) {
    match self {
      Self::GotoDeclaration { uri } => ("goto-declaration", Some(uri)),
      Self::GotoDefinition { uri } => ("goto-definition", Some(uri)),
      Self::GotoTypeDefinition { uri } => ("goto-type-definition", Some(uri)),
      Self::GotoImplementation { uri } => ("goto-implementation", Some(uri)),
      Self::Hover { uri } => ("hover", Some(uri)),
      Self::DocumentHighlightSelect { uri } => ("document-highlight-select", Some(uri)),
      Self::References { uri } => ("references", Some(uri)),
      Self::DocumentSymbols { uri } => ("document-symbols", Some(uri)),
      Self::WorkspaceSymbols { .. } => ("workspace-symbols", None),
      Self::Completion { uri, .. } => ("completion", Some(uri)),
      Self::CompletionResolve { uri, .. } => ("completion-resolve", Some(uri)),
      Self::SignatureHelp { uri } => ("signature-help", Some(uri)),
      Self::CodeActions { uri } => ("code-actions", Some(uri)),
      Self::CodeActionResolve { uri, .. } => ("code-action-resolve", Some(uri)),
      Self::Rename { uri } => ("rename", Some(uri)),
      Self::Format { uri } => ("format", Some(uri)),
    }
  }
}

/// Application state passed to all handlers.
pub struct Ctx {
  pub editor:                         Editor,
  pub file_path:                      Option<PathBuf>,
  pub working_directory:              WorkingDirectoryState,
  pub should_quit:                    bool,
  pub needs_render:                   bool,
  cursor_blink_generation:            u64,
  pub messages:                       MessageCenter,
  message_log:                        Option<BufWriter<std::fs::File>>,
  message_log_seq:                    u64,
  lsp_trace_log:                      Option<BufWriter<std::fs::File>>,
  render_wake_tx:                     Sender<()>,
  pub render_wake_rx:                 Receiver<()>,
  pub mode:                           Mode,
  pub defaults:                       Defaults,
  pub dispatch:                       Box<dyn DefaultApi<Ctx>>,
  pub keymaps:                        Keymaps,
  pub completion_menu_keymaps:        Keymaps,
  pub command_registry:               CommandRegistry<Ctx>,
  pub command_prompt:                 CommandPromptState,
  pub command_palette:                CommandPaletteState,
  pub command_palette_style:          CommandPaletteStyle,
  pub completion_menu:                the_default::CompletionMenuState,
  pub inline_completion:              the_default::InlineCompletionState,
  pub signature_help:                 the_default::SignatureHelpState,
  pub hover_docs:                     Option<String>,
  pub hover_docs_scroll:              usize,
  pub file_tree:                      FileTreeState,
  pub file_picker:                    FilePickerState,
  pub picker_runtime_store:           the_default::PickerRuntimeStore<Ctx>,
  lsp_services_started:               bool,
  next_lsp_runtime_id:                LspRuntimeId,
  lsp_runtimes:                       BTreeMap<LspRuntimeId, ManagedLspRuntime>,
  buffer_lsp_states:                  BTreeMap<BufferId, BufferLspState>,
  active_lsp_runtime_id:              Option<LspRuntimeId>,
  pub lsp_ready:                      bool,
  pub lsp_document:                   Option<LspDocumentSyncState>,
  lsp_statusline:                     LspStatuslineState,
  lsp_spinner_index:                  usize,
  lsp_spinner_last_tick:              Instant,
  lsp_active_progress_tokens:         HashSet<String>,
  lsp_watched_file:                   Option<LspWatchedFileState>,
  lsp_pending_requests:               HashMap<u64, PendingLspRequestKind>,
  lsp_completion_items:               Vec<LspCompletionItem>,
  lsp_completion_raw_items:           Vec<Value>,
  lsp_completion_resolved_indices:    HashSet<usize>,
  lsp_completion_resolve_supported:   bool,
  lsp_completion_inline_item_active:  bool,
  lsp_completion_visible_indices:     Vec<usize>,
  lsp_completion_fallback_start:      Option<usize>,
  lsp_code_action_items:              Vec<LspCodeAction>,
  lsp_code_action_menu_active:        bool,
  lsp_completion_generation:          u64,
  lsp_pending_auto_completion:        Option<PendingAutoCompletion>,
  lsp_pending_auto_signature_help:    Option<PendingAutoSignatureHelp>,
  lsp_signature_help_presentation:    Option<the_default::SignatureHelpPresentation>,
  pub diagnostics:                    DiagnosticsState,
  vcs_watch:                          Option<VcsWatchState>,
  shared_vcs:                         SharedVcsState,
  active_file_vcs_refresh_due_at:     Option<Instant>,
  active_file_vcs_refresh_reason:     Option<ActiveFileVcsRefreshReason>,
  active_file_vcs_refresh_in_flight:  bool,
  active_file_vcs_refresh_generation: u64,
  active_file_vcs_refresh_rerun:      bool,
  active_file_vcs_refresh_tx:         Sender<ActiveFileVcsRefreshResult>,
  active_file_vcs_refresh_rx:         Receiver<ActiveFileVcsRefreshResult>,
  file_tree_vcs_refresh_due_at:       Option<Instant>,
  file_tree_vcs_refresh_reason:       Option<FileTreeVcsRefreshReason>,
  file_tree_vcs_refresh_in_flight:    bool,
  file_tree_vcs_refresh_generation:   u64,
  file_tree_vcs_refresh_rerun:        bool,
  file_tree_decoration_root:          Option<PathBuf>,
  file_tree_diagnostics_seq:          u64,
  file_tree_vcs_refresh_tx:           Sender<FileTreeVcsRefreshResult>,
  file_tree_vcs_refresh_rx:           Receiver<FileTreeVcsRefreshResult>,
  pub file_tree_layout:               Option<FileTreeLayout>,
  pub file_picker_layout:             Option<FilePickerLayout>,
  pub file_picker_drag:               Option<FilePickerDragState>,
  pub completion_docs_layout:         Option<CompletionDocsLayout>,
  pub completion_docs_drag:           Option<CompletionDocsDragState>,
  pub pane_resize_drag:               Option<PaneResizeDragState>,
  pub buffer_tab_drag:                Option<BufferTabPointerDragState>,
  pub buffer_tab_hover:               Option<BufferTabHoverState>,
  pub mouse_selection_drag_active:    bool,
  pub mouse_viewport_detached:        bool,
  pointer_drag_selection:             Option<PointerSelectionDragState>,
  mouse_last_primary_click:           Option<PointerClickTracker>,
  pub search_prompt:                  the_default::SearchPromptState,
  global_search:                      GlobalSearchState,
  vcs_diff_picker:                    VcsDiffPickerState,
  pub ui_theme_catalog:               ThemeCatalog,
  pub ui_theme_name:                  String,
  pub ui_theme_base:                  Theme,
  pub ui_theme_preview_name:          Option<String>,
  pub ui_theme:                       Theme,
  pub pending_input:                  Option<the_default::PendingInput>,
  pub dispatch_override:              Option<NonNull<dyn DefaultApi<Ctx>>>,
  /// Syntax loader for language detection and highlighting.
  pub loader:                         Option<Arc<Loader>>,
  /// Cache for syntax highlights (reused across renders).
  pub highlight_cache:                HighlightCache,
  /// Per-buffer caches for inactive split panes.
  pub inactive_highlight_caches:      BTreeMap<BufferId, HighlightCache>,
  /// Background parse result channel (async syntax fallback).
  pub syntax_parse_tx:                Sender<SyntaxParseResult>,
  /// Background parse result receiver (async syntax fallback).
  pub syntax_parse_rx:                Receiver<SyntaxParseResult>,
  /// Async parse lifecycle (single in-flight + one queued replacement).
  pub syntax_parse_lifecycle:         ParseLifecycle<SyntaxParseJob>,
  /// Syntax parse/highlight gate state (parsed vs interpolated).
  pub syntax_parse_highlight_state:   ParseHighlightState,
  /// Registers for yanking/pasting.
  pub registers:                      Registers,
  /// Active register target (for macros/register operations).
  pub register:                       Option<char>,
  /// Macro recording state.
  pub macro_recording:                Option<(char, Vec<KeyBinding>)>,
  /// Macro replay stack for recursion guard.
  pub macro_replaying:                Vec<char>,
  /// Pending macro key events to replay.
  pub macro_queue:                    VecDeque<KeyEvent>,
  /// Last executed motion for repeat.
  pub last_motion:                    Option<Motion>,
  /// Render formatting used for visual position mapping.
  pub text_format:                    TextFormat,
  pub cursor_shapes:                  CursorShapes,
  /// Gutter layout and line-number rendering config.
  pub gutter_config:                  GutterConfig,
  /// VCS-like gutter signs keyed by document line.
  pub gutter_diff_signs:              BTreeMap<usize, RenderGutterDiffKind>,
  /// Active VCS provider registry for diff base resolution.
  pub vcs_provider:                   DiffProviderRegistry,
  /// Cached VCS statusline text for the active file.
  pub vcs_statusline:                 Option<String>,
  /// Incremental VCS diff state for the active file.
  pub vcs_diff:                       Option<DiffHandle>,
  /// Inline annotations (virtual text) for rendering.
  pub inline_annotations:             Vec<InlineAnnotation>,
  /// Overlay annotations (virtual text) for rendering.
  pub overlay_annotations:            Vec<Overlay>,
  /// Built-in inline completion ghost text annotations.
  pub inline_completion_annotations:  the_default::OwnedTextAnnotations,
  /// Transient inline jump labels for word-jump navigation.
  pub word_jump_inline_annotations:   Vec<InlineAnnotation>,
  /// Transient overlay jump labels for word-jump navigation.
  pub word_jump_overlay_annotations:  Vec<Overlay>,
  /// Inline diagnostic ghost lines collected during render-plan construction.
  pub inline_diagnostic_lines:        Vec<InlineDiagnosticRenderLine>,
  /// Underline spans for diagnostic ranges in the current viewport.
  pub diagnostic_underlines:          Vec<DiagnosticUnderlineRenderSpan>,
  /// Per-pane inline diagnostic state for frame rendering.
  pub frame_inline_diagnostic_lines:  BTreeMap<PaneId, Vec<InlineDiagnosticRenderLine>>,
  /// Per-pane diagnostic underlines for frame rendering.
  pub frame_diagnostic_underlines:    BTreeMap<PaneId, Vec<DiagnosticUnderlineRenderSpan>>,
  /// Per-pane raw diagnostics used by split-pane end-of-line rendering.
  pub frame_diagnostics:              BTreeMap<PaneId, Vec<Diagnostic>>,
  /// Last emitted render generations and row hashes per pane.
  pub frame_generation_state:         the_lib::render::FrameGenerationState,
  /// Theme generation token for render metadata consumers.
  pub render_theme_generation:        u64,
  /// Lines to keep above/below cursor when scrolling.
  pub scrolloff:                      usize,
  pub term_cursor_mode:               TermCursorMode,
}

#[derive(Debug, Clone, Copy)]
pub struct TermHardwareCursor {
  pub x:    u16,
  pub y:    u16,
  pub kind: the_lib::render::graphics::CursorKind,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum TermCursorMode {
  #[default]
  Hidden,
  Hardware(TermHardwareCursor),
}

fn select_ui_theme(catalog: &ThemeCatalog, configured_theme: Option<&str>) -> (String, Theme) {
  let requested_theme = env::var("THE_EDITOR_THEME")
    .ok()
    .map(|theme| theme.trim().to_string())
    .filter(|theme| !theme.is_empty())
    .or_else(|| {
      configured_theme
        .map(str::trim)
        .map(str::to_string)
        .filter(|theme| !theme.is_empty())
    });

  if let Some(theme_name) = requested_theme {
    if let Some(theme) = catalog.load_theme(&theme_name) {
      return (theme_name, theme);
    }
    eprintln!("Unknown theme '{theme_name}', falling back to default theme.");
  }

  (
    default_theme().name().to_string(),
    catalog
      .load_theme(default_theme().name())
      .unwrap_or_else(|| default_theme().clone()),
  )
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

fn lsp_servers_from_language_config(loader: &Loader, path: &Path) -> Vec<LspServerConfig> {
  let Some(language) = loader.language_for_filename(path) else {
    return Vec::new();
  };
  let language_config = loader.language(language).config();
  let mut servers = Vec::new();
  for server_features in &language_config.services.language_servers {
    let server_name = server_features.name.clone();
    let Some(server_config) = loader.language_server_configs().get(&server_name) else {
      continue;
    };
    servers.push(
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
    );
  }
  servers
}

fn resolve_lsp_servers(loader: Option<&Loader>, path: Option<&Path>) -> Vec<LspServerConfig> {
  let mut servers = path
    .and_then(|path| loader.map(|loader| lsp_servers_from_language_config(loader, path)))
    .unwrap_or_default();
  if servers.is_empty()
    && let Some(server) = lsp_server_from_env()
  {
    servers.push(server);
  }
  servers
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

fn workspace_root_for_path(path: &Path) -> PathBuf {
  let path = if path.is_absolute() {
    path.to_path_buf()
  } else {
    env::current_dir()
      .unwrap_or_else(|_| PathBuf::from("."))
      .join(path)
  };
  path
    .parent()
    .map(|parent| the_loader::find_workspace_in(parent.to_path_buf()).0)
    .unwrap_or_else(|| the_loader::find_workspace().0)
}

fn vcs_repo_absolute_path(path: &Path, repo_root: &Path) -> PathBuf {
  if path.is_absolute() {
    path.to_path_buf()
  } else {
    repo_root.join(path)
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
    Self::new_with_defaults(file_path, &default_defaults())
  }

  pub fn new_with_defaults(file_path: Option<&str>, defaults: &Defaults) -> Result<Self> {
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
    if let Some(path) = file_path {
      editor.set_active_file_path(Some(PathBuf::from(path)));
    }

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

    // Initialize syntax loader
    let ui_theme_catalog = ThemeCatalog::load(Some(workspace_root.as_path()));
    let (ui_theme_name, ui_theme) = select_ui_theme(&ui_theme_catalog, defaults.theme.as_deref());

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

    let (render_wake_tx, render_wake_rx) = std::sync::mpsc::channel();
    let (active_file_vcs_refresh_tx, active_file_vcs_refresh_rx) = channel();
    let (file_tree_vcs_refresh_tx, file_tree_vcs_refresh_rx) = channel();
    let mut file_picker = FilePickerState::default();
    the_default::set_file_picker_options(
      &mut file_picker,
      defaults.editor.file_picker.clone().unwrap_or_default(),
    );
    the_default::set_file_picker_wake_sender(&mut file_picker, Some(render_wake_tx.clone()));
    the_default::set_file_picker_syntax_loader(&mut file_picker, loader.clone());
    let (syntax_parse_tx, syntax_parse_rx) = channel();
    let lsp_document = file_path
      .map(PathBuf::from)
      .as_deref()
      .and_then(|path| build_lsp_document_state(path, loader.as_deref()));
    let mut gutter_config = GutterConfig::default();
    if let Some(mode) = defaults.editor.line_numbers {
      gutter_config.line_numbers.mode = mode;
    }
    let cursor_shapes = defaults.editor.cursor_shapes.unwrap_or_default();
    let mut command_registry = CommandRegistry::new();
    install_default_wiring(&mut command_registry);
    let mut ctx = Self {
      editor,
      file_path: file_path.map(PathBuf::from),
      working_directory: WorkingDirectoryState {
        current:  Some(workspace_root.clone()),
        previous: None,
      },
      should_quit: false,
      needs_render: true,
      cursor_blink_generation: 0,
      messages: MessageCenter::default(),
      message_log,
      message_log_seq: 0,
      lsp_trace_log,
      render_wake_tx,
      render_wake_rx,
      mode: Mode::Normal,
      defaults: defaults.clone(),
      dispatch: Box::new(crate::dispatch::build_dispatch::<Self>()),
      keymaps: builtin_keymaps(),
      completion_menu_keymaps: builtin_completion_menu_keymaps(),
      command_registry,
      command_prompt: CommandPromptState::new(),
      command_palette: CommandPaletteState::default(),
      command_palette_style: CommandPaletteStyle::helix_bottom(),
      completion_menu: the_default::CompletionMenuState::default(),
      inline_completion: the_default::InlineCompletionState::from_defaults(
        defaults
          .editor
          .inline_completion
          .clone()
          .unwrap_or_default(),
      ),
      signature_help: the_default::SignatureHelpState::default(),
      hover_docs: None,
      hover_docs_scroll: 0,
      file_tree: FileTreeState::default(),
      file_picker,
      picker_runtime_store: the_default::PickerRuntimeStore::default(),
      lsp_services_started: false,
      next_lsp_runtime_id: 1,
      lsp_runtimes: BTreeMap::new(),
      buffer_lsp_states: BTreeMap::new(),
      active_lsp_runtime_id: None,
      lsp_ready: false,
      lsp_document,
      lsp_statusline: LspStatuslineState::off(Some("unavailable".into())),
      lsp_spinner_index: 0,
      lsp_spinner_last_tick: Instant::now(),
      lsp_active_progress_tokens: HashSet::new(),
      lsp_watched_file: None,
      lsp_pending_requests: HashMap::new(),
      lsp_completion_items: Vec::new(),
      lsp_completion_raw_items: Vec::new(),
      lsp_completion_resolved_indices: HashSet::new(),
      lsp_completion_resolve_supported: true,
      lsp_completion_inline_item_active: false,
      lsp_completion_visible_indices: Vec::new(),
      lsp_completion_fallback_start: None,
      lsp_code_action_items: Vec::new(),
      lsp_code_action_menu_active: false,
      lsp_completion_generation: 0,
      lsp_pending_auto_completion: None,
      lsp_pending_auto_signature_help: None,
      lsp_signature_help_presentation: None,
      diagnostics: DiagnosticsState::default(),
      vcs_watch: None,
      shared_vcs: SharedVcsState::default(),
      active_file_vcs_refresh_due_at: None,
      active_file_vcs_refresh_reason: None,
      active_file_vcs_refresh_in_flight: false,
      active_file_vcs_refresh_generation: 0,
      active_file_vcs_refresh_rerun: false,
      active_file_vcs_refresh_tx,
      active_file_vcs_refresh_rx,
      file_tree_vcs_refresh_due_at: None,
      file_tree_vcs_refresh_reason: None,
      file_tree_vcs_refresh_in_flight: false,
      file_tree_vcs_refresh_generation: 0,
      file_tree_vcs_refresh_rerun: false,
      file_tree_decoration_root: None,
      file_tree_diagnostics_seq: 0,
      file_tree_vcs_refresh_tx,
      file_tree_vcs_refresh_rx,
      file_tree_layout: None,
      file_picker_layout: None,
      file_picker_drag: None,
      completion_docs_layout: None,
      completion_docs_drag: None,
      pane_resize_drag: None,
      buffer_tab_drag: None,
      buffer_tab_hover: None,
      mouse_selection_drag_active: false,
      mouse_viewport_detached: false,
      pointer_drag_selection: None,
      mouse_last_primary_click: None,
      search_prompt: the_default::SearchPromptState::new(),
      global_search: GlobalSearchState::default(),
      vcs_diff_picker: VcsDiffPickerState::default(),
      ui_theme_catalog,
      ui_theme_name,
      ui_theme_base: ui_theme.clone(),
      ui_theme_preview_name: None,
      ui_theme,
      pending_input: None,
      dispatch_override: None,
      loader,
      highlight_cache: HighlightCache::default(),
      inactive_highlight_caches: BTreeMap::new(),
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
      cursor_shapes,
      gutter_config,
      gutter_diff_signs: BTreeMap::new(),
      vcs_provider: DiffProviderRegistry::default(),
      vcs_statusline: None,
      vcs_diff: None,
      inline_annotations: Vec::new(),
      overlay_annotations: Vec::new(),
      inline_completion_annotations: the_default::OwnedTextAnnotations::default(),
      word_jump_inline_annotations: Vec::new(),
      word_jump_overlay_annotations: Vec::new(),
      inline_diagnostic_lines: Vec::new(),
      diagnostic_underlines: Vec::new(),
      frame_inline_diagnostic_lines: BTreeMap::new(),
      frame_diagnostic_underlines: BTreeMap::new(),
      frame_diagnostics: BTreeMap::new(),
      frame_generation_state: the_lib::render::FrameGenerationState::default(),
      render_theme_generation: 0,
      scrolloff: 5,
      term_cursor_mode: TermCursorMode::Hidden,
    };
    let initial_lsp_path = ctx.file_path.clone();
    ctx.lsp_refresh_document_state(initial_lsp_path.as_deref());
    ctx.schedule_active_file_vcs_refresh(ActiveFileVcsRefreshReason::Startup, None);
    Ok(ctx)
  }

  pub fn set_dispatch<D>(&mut self, dispatch: &D)
  where
    D: DefaultApi<Ctx> + 'static,
  {
    self.dispatch_override = Some(NonNull::from(dispatch as &dyn DefaultApi<Ctx>));
  }

  fn on_buffer_saved(&mut self, buffer_id: BufferId, _path: &Path, text: &str) {
    if self.editor.active_buffer_id() == buffer_id
      && let Some(watch) = self.lsp_watched_file.as_mut()
    {
      watch.stream.suppress_until = Some(Instant::now() + lsp_self_save_suppress_window());
      clear_reload_state(&mut watch.stream.reload_state);
    }
    self.lsp_send_did_save_for_buffer(buffer_id, Some(text));
    self.schedule_active_file_vcs_refresh(ActiveFileVcsRefreshReason::VcsWatch, None);
    self.schedule_file_tree_vcs_refresh(FileTreeVcsRefreshReason::VcsWatch, None);
    self.queue_open_vcs_diff_picker_refresh();
    self.request_render();
  }

  fn reset_transient_input_state(&mut self) {
    self.keymaps.reset_pending();
    self.completion_menu_keymaps.reset_pending();
    self.pending_input = None;
    self.file_picker_drag = None;
    self.completion_docs_drag = None;
    self.pane_resize_drag = None;
    self.buffer_tab_drag = None;
    self.buffer_tab_hover = None;
    self.mouse_selection_drag_active = false;
    self.mouse_viewport_detached = false;
    self.pointer_drag_selection = None;
    self.mouse_last_primary_click = None;
  }

  pub(crate) fn handle_terminal_focus_lost(&mut self) {
    self.reset_transient_input_state();
    self.term_cursor_mode = TermCursorMode::Hidden;
    self.needs_render = true;
  }

  pub(crate) fn handle_terminal_focus_gained(&mut self) {
    self.reset_transient_input_state();
    self.needs_render = true;
  }

  fn apply_effective_theme(&mut self, theme: Theme) {
    if let Some(loader) = &self.loader {
      loader.set_scopes(theme.scopes().to_vec());
    }
    self.ui_theme = theme;
    self.invalidate_theme_render_state();
  }

  fn set_ui_theme_named(&mut self, theme_name: &str) -> Result<(), String> {
    let theme = self
      .ui_theme_catalog
      .load_theme(theme_name)
      .ok_or_else(|| format!("Could not load theme: {theme_name}"))?;
    self.ui_theme_name = theme_name.to_string();
    self.ui_theme_base = theme.clone();
    self.ui_theme_preview_name = None;
    self.apply_effective_theme(theme);
    Ok(())
  }

  fn set_ui_theme_preview_named(&mut self, theme_name: &str) -> Result<(), String> {
    let theme = self
      .ui_theme_catalog
      .load_theme(theme_name)
      .ok_or_else(|| format!("Could not load theme: {theme_name}"))?;
    self.ui_theme_preview_name = Some(theme_name.to_string());
    self.apply_effective_theme(theme);
    Ok(())
  }

  fn clear_ui_theme_preview_state(&mut self) {
    if self.ui_theme_preview_name.take().is_some() {
      self.apply_effective_theme(self.ui_theme_base.clone());
    }
  }

  fn invalidate_theme_render_state(&mut self) {
    self.render_theme_generation = self.render_theme_generation.wrapping_add(1);
    self.highlight_cache.clear();
    self.inactive_highlight_caches.clear();
    if self.editor.document().syntax().is_some() {
      self.syntax_parse_highlight_state.mark_parsed();
    } else {
      self.syntax_parse_highlight_state.mark_cleared();
    }
    self.needs_render = true;
  }

  fn clear_vcs_diff(&mut self) -> bool {
    let changed = self.vcs_diff.is_some() || !self.gutter_diff_signs.is_empty();
    self.vcs_diff = None;
    self.gutter_diff_signs.clear();
    changed
  }

  fn clear_active_file_vcs_state(&mut self) -> bool {
    let mut changed = false;
    if self.vcs_statusline.take().is_some() {
      changed = true;
    }
    changed | self.clear_vcs_diff()
  }

  fn refresh_active_file_vcs_after_path_change(
    &mut self,
    previous_path: Option<PathBuf>,
    reason: ActiveFileVcsRefreshReason,
  ) {
    if previous_path != self.file_path {
      self.clear_active_file_vcs_state();
    }
    self.schedule_active_file_vcs_refresh(reason, None);
  }

  fn apply_active_file_vcs_refresh_result(
    &mut self,
    statusline: Option<String>,
    diff_base: Option<Vec<u8>>,
  ) -> bool {
    let mut changed = false;
    if self.vcs_statusline != statusline {
      self.vcs_statusline = statusline;
      changed = true;
    }
    let Some(diff_base) = diff_base else {
      return changed | self.clear_vcs_diff();
    };

    let diff_base = Rope::from_str(String::from_utf8_lossy(&diff_base).as_ref());
    let doc = self.editor.document().text().clone();
    let handle = DiffHandle::new(diff_base, doc);
    let signs = vcs_gutter_signs(&handle);
    if self.gutter_diff_signs != signs {
      self.gutter_diff_signs = signs;
      changed = true;
    }
    self.vcs_diff = Some(handle);
    changed
  }

  fn store_shared_vcs_scan(&mut self, scan: VcsWorkspaceScan) -> bool {
    let head_changed = self.shared_vcs.scan.as_ref().is_none_or(|current| {
      current.repo_root != scan.repo_root || current.head_revision != scan.head_revision
    });
    let changed = self
      .shared_vcs
      .scan
      .as_ref()
      .is_none_or(|current| **current != scan);
    if head_changed {
      self
        .shared_vcs
        .base_cache
        .retain(|key, _| key.repo_root != scan.repo_root);
    }
    self.shared_vcs.scan = Some(Arc::new(scan));
    if changed {
      self.shared_vcs.generation = self.shared_vcs.generation.wrapping_add(1);
    }
    changed
  }

  fn shared_vcs_scan_for_path(&self, path: &Path) -> Option<Arc<VcsWorkspaceScan>> {
    let scan = self.shared_vcs.scan.as_ref()?;
    if path.starts_with(&scan.repo_root) {
      Some(scan.clone())
    } else {
      None
    }
  }

  fn shared_vcs_scan_for_cwd(&self, cwd: &Path) -> Option<Arc<VcsWorkspaceScan>> {
    let scan = self.shared_vcs.scan.as_ref()?;
    if cwd.starts_with(&scan.repo_root) || scan.repo_root.starts_with(cwd) {
      Some(scan.clone())
    } else {
      None
    }
  }

  fn cached_vcs_base_for_path(&mut self, scan: &VcsWorkspaceScan, path: &Path) -> Option<Vec<u8>> {
    let key = VcsBaseCacheKey {
      repo_root:     scan.repo_root.clone(),
      head_revision: scan.head_revision.clone(),
      path:          path.to_path_buf(),
    };
    if let Some(bytes) = self.shared_vcs.base_cache.get(&key) {
      return bytes.clone();
    }
    let bytes = self.vcs_provider.get_diff_base(path);
    self.shared_vcs.base_cache.insert(key, bytes.clone());
    bytes
  }

  fn cached_vcs_base_for_change(
    &mut self,
    scan: &VcsWorkspaceScan,
    change: &FileChange,
  ) -> Option<Vec<u8>> {
    let base_path = match change {
      FileChange::Untracked { .. } => return None,
      FileChange::Modified { path }
      | FileChange::Conflict { path }
      | FileChange::Deleted { path } => path.clone(),
      FileChange::Renamed { from_path, .. } => from_path.clone(),
    };
    let key = VcsBaseCacheKey {
      repo_root:     scan.repo_root.clone(),
      head_revision: scan.head_revision.clone(),
      path:          base_path.clone(),
    };
    if let Some(bytes) = self.shared_vcs.base_cache.get(&key) {
      return bytes.clone();
    }
    let bytes = self.vcs_provider.get_diff_base_for_change(change);
    self.shared_vcs.base_cache.insert(key, bytes.clone());
    bytes
  }

  fn shared_vcs_changed_file_items(
    &self,
    scan: &VcsWorkspaceScan,
  ) -> Vec<FilePickerChangedFileItem> {
    scan
      .changes
      .iter()
      .map(|change| {
        match change {
          FileChange::Untracked { path } => {
            FilePickerChangedFileItem {
              kind:      FilePickerChangedKind::Untracked,
              path:      path.clone(),
              from_path: None,
            }
          },
          FileChange::Modified { path } => {
            FilePickerChangedFileItem {
              kind:      FilePickerChangedKind::Modified,
              path:      path.clone(),
              from_path: None,
            }
          },
          FileChange::Conflict { path } => {
            FilePickerChangedFileItem {
              kind:      FilePickerChangedKind::Conflict,
              path:      path.clone(),
              from_path: None,
            }
          },
          FileChange::Deleted { path } => {
            FilePickerChangedFileItem {
              kind:      FilePickerChangedKind::Deleted,
              path:      path.clone(),
              from_path: None,
            }
          },
          FileChange::Renamed { from_path, to_path } => {
            FilePickerChangedFileItem {
              kind:      FilePickerChangedKind::Renamed,
              path:      to_path.clone(),
              from_path: Some(from_path.clone()),
            }
          },
        }
      })
      .collect()
  }

  fn merged_vcs_changed_file_items(
    &self,
    scan: &VcsWorkspaceScan,
  ) -> Vec<FilePickerChangedFileItem> {
    let mut items = self.shared_vcs_changed_file_items(scan);
    let mut seen = items
      .iter()
      .map(|item| item.path.clone())
      .collect::<std::collections::HashSet<_>>();

    for snapshot in self.editor.buffer_snapshots_mru() {
      if !snapshot.modified {
        continue;
      }
      let Some(path) = snapshot.file_path else {
        continue;
      };
      let path = vcs_repo_absolute_path(&path, &scan.repo_root);
      if !path.starts_with(&scan.repo_root) {
        continue;
      }
      if seen.contains(&path) {
        continue;
      }
      seen.insert(path.clone());
      items.push(FilePickerChangedFileItem {
        kind: FilePickerChangedKind::Modified,
        path,
        from_path: None,
      });
    }

    items.sort_by(|left, right| left.path.cmp(&right.path));
    items
  }

  fn refresh_shared_vcs_scan_for_cwd(&mut self, cwd: &Path) -> Option<Arc<VcsWorkspaceScan>> {
    match self.vcs_provider.scan_workspace(cwd) {
      Ok(scan) => {
        let _ = self.store_shared_vcs_scan(scan);
        self.shared_vcs_scan_for_cwd(cwd)
      },
      Err(_) => self.shared_vcs_scan_for_cwd(cwd),
    }
  }

  fn schedule_active_file_vcs_refresh(
    &mut self,
    reason: ActiveFileVcsRefreshReason,
    due_at: Option<Instant>,
  ) {
    let Some(path) = self.file_path.clone() else {
      self.active_file_vcs_refresh_due_at = None;
      self.active_file_vcs_refresh_reason = None;
      self.active_file_vcs_refresh_rerun = false;
      return;
    };
    let due_at = due_at.unwrap_or_else(Instant::now);
    self.active_file_vcs_refresh_due_at = Some(
      self
        .active_file_vcs_refresh_due_at
        .map_or(due_at, |current| current.min(due_at)),
    );
    self.active_file_vcs_refresh_reason = Some(
      self
        .active_file_vcs_refresh_reason
        .map_or(reason, |current| current.combine(reason)),
    );
    if self.active_file_vcs_refresh_in_flight {
      self.active_file_vcs_refresh_rerun = true;
    }
    log_active_file_vcs_refresh_event(
      "scheduled",
      self.active_file_vcs_refresh_generation + u64::from(self.active_file_vcs_refresh_in_flight),
      &path,
      reason,
      None,
      None,
      None,
      None,
    );
  }

  pub fn poll_active_file_vcs_refresh_dispatch(&mut self, now: Instant) -> bool {
    if self.active_file_vcs_refresh_in_flight {
      return false;
    }
    let Some(due_at) = self.active_file_vcs_refresh_due_at else {
      return false;
    };
    if now < due_at {
      return false;
    }
    let Some(path) = self.file_path.clone() else {
      self.active_file_vcs_refresh_due_at = None;
      self.active_file_vcs_refresh_reason = None;
      self.active_file_vcs_refresh_rerun = false;
      return false;
    };
    let reason = self
      .active_file_vcs_refresh_reason
      .take()
      .unwrap_or(ActiveFileVcsRefreshReason::VcsWatch);
    self.active_file_vcs_refresh_due_at = None;
    self.active_file_vcs_refresh_in_flight = true;
    self.active_file_vcs_refresh_rerun = false;
    self.active_file_vcs_refresh_generation += 1;
    let generation = self.active_file_vcs_refresh_generation;
    let vcs_provider = self.vcs_provider.clone();
    let shared_scan = self.shared_vcs_scan_for_path(&path);
    let tx = self.active_file_vcs_refresh_tx.clone();
    let wake_tx = self.render_wake_tx.clone();
    log_active_file_vcs_refresh_event("started", generation, &path, reason, None, None, None, None);
    thread::spawn(move || {
      let collect_start = Instant::now();
      let scan = if matches!(reason, ActiveFileVcsRefreshReason::VcsWatch) {
        vcs_provider
          .scan_workspace(&path)
          .ok()
          .or_else(|| shared_scan.map(|scan| (*scan).clone()))
      } else {
        shared_scan
          .map(|scan| (*scan).clone())
          .or_else(|| vcs_provider.scan_workspace(&path).ok())
      };
      let statusline = scan
        .as_ref()
        .and_then(|scan| scan.statusline_info.as_ref())
        .map(|info| info.statusline_text())
        .or_else(|| {
          vcs_provider
            .get_statusline_info(&path)
            .map(|info| info.statusline_text())
        });
      let collect_ms = collect_start.elapsed().as_secs_f64() * 1000.0;
      log_active_file_vcs_refresh_event(
        "finished",
        generation,
        &path,
        reason,
        Some(statusline.is_some()),
        None,
        Some(collect_ms),
        None,
      );
      let _ = tx.send(ActiveFileVcsRefreshResult {
        generation,
        path,
        reason,
        statusline,
        diff_base: None,
        scan,
        collect_ms,
      });
      let _ = wake_tx.send(());
    });
    false
  }

  pub fn poll_active_file_vcs_refresh_results(&mut self) -> bool {
    let mut needs_render = false;
    loop {
      let result = match self.active_file_vcs_refresh_rx.try_recv() {
        Ok(result) => result,
        Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
      };
      if result.generation == self.active_file_vcs_refresh_generation {
        self.active_file_vcs_refresh_in_flight = false;
      }
      let stale = self.file_path.as_deref() != Some(result.path.as_path())
        || result.generation != self.active_file_vcs_refresh_generation;
      if stale {
        log_active_file_vcs_refresh_event(
          "discarded",
          result.generation,
          &result.path,
          result.reason,
          Some(result.statusline.is_some()),
          Some(result.diff_base.is_some()),
          Some(result.collect_ms),
          None,
        );
      } else {
        let apply_start = Instant::now();
        if let Some(scan) = result.scan {
          let _ = self.store_shared_vcs_scan(scan);
        }
        let diff_base = result.diff_base.or_else(|| {
          let scan = self.shared_vcs_scan_for_path(&result.path)?;
          self.cached_vcs_base_for_path(&scan, &result.path)
        });
        let changed = self.apply_active_file_vcs_refresh_result(result.statusline, diff_base);
        let apply_ms = apply_start.elapsed().as_secs_f64() * 1000.0;
        log_active_file_vcs_refresh_event(
          "applied",
          result.generation,
          &result.path,
          result.reason,
          Some(self.vcs_statusline.is_some()),
          Some(self.vcs_diff.is_some()),
          Some(result.collect_ms),
          Some(apply_ms),
        );
        needs_render |= changed;
      }
      if self.active_file_vcs_refresh_rerun {
        self.active_file_vcs_refresh_rerun = false;
        if self.active_file_vcs_refresh_due_at.is_none() {
          self.active_file_vcs_refresh_due_at = Some(Instant::now());
        }
      }
    }
    needs_render
  }

  fn refresh_vcs_diff_document(&mut self) {
    let Some(handle) = self.vcs_diff.as_ref() else {
      return;
    };
    let _ = handle.update_document(self.editor.document().text().clone(), true);
    self.gutter_diff_signs = vcs_gutter_signs(handle);
  }

  fn queue_open_vcs_diff_picker_refresh(&mut self) {
    if self.file_picker.active && self.file_picker.kind == the_default::FilePickerKind::VcsDiff {
      self.vcs_diff_picker.live_refresh_pending = true;
    }
  }

  /// Handle terminal resize.
  pub fn resize(&mut self, width: u16, height: u16) {
    let viewport = Rect::new(0, 0, width, height);
    self.editor.set_layout_viewport(viewport);
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
        self.inactive_highlight_caches.clear();
        changed = true;
      } else {
        self.syntax_parse_highlight_state.mark_interpolated();
      }
    }

    changed
  }

  fn start_global_search(&mut self) {
    let root = the_default::workspace_root(self.effective_working_directory().as_path());
    if !root.exists() {
      let _ = <Self as the_default::DefaultContext>::push_error(
        self,
        "global_search",
        "Current working directory does not exist",
      );
      return;
    }

    let options = GlobalSearchOptions {
      smart_case:  true,
      file_picker: self.file_picker.options.clone(),
    };
    if let Err(err) = self.global_search.activate(root.as_path(), options) {
      let _ = <Self as the_default::DefaultContext>::push_error(
        self,
        "global_search",
        format!("Failed to initialize global search: {err}"),
      );
      return;
    }

    let initial_query = self.workspace_symbol_query_from_cursor();
    open_dynamic_picker(self, "Live Grep", root, None, initial_query.clone());

    if !initial_query.trim().is_empty() {
      self.schedule_global_search(initial_query);
    } else {
      self.needs_render = true;
    }
  }

  fn schedule_global_search(&mut self, query: String) {
    if !self.global_search.is_active() {
      return;
    }
    self
      .global_search
      .schedule(query, self.global_search_documents());
  }

  pub fn poll_global_search(&mut self) -> bool {
    if !self.global_search.is_active() {
      return false;
    }
    if !self.file_picker.active {
      self.global_search.deactivate();
      return false;
    }

    let Some(response) = self.global_search.poll_latest() else {
      return false;
    };

    let has_items = !response.items.is_empty();
    replace_file_picker_items(self, response.items, 0);
    {
      let picker = &mut self.file_picker;
      picker.query = response.query.clone();
      picker.cursor = response.query.len();
      if let Some(error) = response.error {
        picker.error = Some(error.clone());
        picker.preview = the_default::FilePickerPreview::Message(error);
      } else if response.indexing && !has_items {
        picker.error = None;
        picker.preview = the_default::FilePickerPreview::Message("Indexing files…".to_string());
      } else {
        picker.error = None;
        if picker.query.trim().is_empty() {
          picker.preview = the_default::FilePickerPreview::Message("Type to search".to_string());
        }
      }
    }
    self.needs_render = true;
    true
  }

  pub fn start_background_services(&mut self) {
    self.lsp_services_started = true;
    self.lsp_ready = false;
    self.lsp_active_progress_tokens.clear();
    self.lsp_pending_requests.clear();
    self.cancel_auto_signature_help();
    self.clear_signature_help_state();
    let active_path = self.file_path.clone();
    self.lsp_refresh_document_state(active_path.as_deref());
    self.lsp_sync_watched_file_state();
    let runtime_ids = self.lsp_runtimes.keys().copied().collect::<Vec<_>>();
    if runtime_ids.is_empty() {
      self.lsp_statusline = LspStatuslineState::off(Some("unavailable".into()));
      return;
    }
    for runtime_id in runtime_ids {
      self.ensure_lsp_runtime_started(runtime_id);
    }
    self.sync_active_lsp_mirror_state();
  }

  pub fn shutdown_background_services(&mut self) {
    self.lsp_services_started = false;
    let buffer_ids = self.buffer_lsp_states.keys().copied().collect::<Vec<_>>();
    for buffer_id in buffer_ids {
      let runtime_ids = self
        .buffer_lsp_states
        .get(&buffer_id)
        .map(|state| state.attached_runtime_ids.clone())
        .unwrap_or_default();
      for runtime_id in runtime_ids {
        self.close_buffer_document_for_runtime(buffer_id, runtime_id);
      }
    }
    self.lsp_ready = false;
    self.lsp_active_progress_tokens.clear();
    self.lsp_pending_requests.clear();
    self.cancel_auto_signature_help();
    self.clear_signature_help_state();
    self.lsp_statusline = LspStatuslineState::off(Some("stopped".into()));
    self.log_lsp_trace_value(json!({
      "ts_ms": SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|duration| duration.as_millis() as u64).unwrap_or(0),
      "kind": "shutdown",
    }));
    self.lsp_watched_file = None;
    self.syntax_parse_highlight_state.mark_cleared();
    let runtime_ids = self.lsp_runtimes.keys().copied().collect::<Vec<_>>();
    for runtime_id in runtime_ids {
      if let Some(runtime) = self.lsp_runtimes.get_mut(&runtime_id) {
        runtime.ready = false;
        runtime.active_progress_tokens.clear();
        runtime.pending_requests.clear();
        runtime.statusline = LspStatuslineState::off(Some("stopped".into()));
        if let Err(err) = runtime.runtime.shutdown_detached() {
          eprintln!("Warning: failed to stop LSP runtime {runtime_id}: {err}");
        }
      }
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

  pub(crate) fn log_render_trace_value(&mut self, stage: &'static str, data: Value) {
    let timestamp_ms = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .map(|duration| duration.as_millis() as u64)
      .unwrap_or(0);
    self.log_lsp_trace_value(json!({
      "ts_ms": timestamp_ms,
      "kind": "render",
      "stage": stage,
      "data": data,
    }));
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
    let has_server = self.active_lsp_runtime_id.is_some();
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
    let runtime_ids = self.lsp_runtimes.keys().copied().collect::<Vec<_>>();
    for runtime_id in runtime_ids {
      loop {
        let event = {
          let Some(runtime) = self.lsp_runtimes.get(&runtime_id) else {
            break;
          };
          runtime.runtime.try_recv_event()
        };
        let Some(event) = event else {
          break;
        };
        self.log_lsp_trace_event(&event);
        match event {
          LspEvent::Started { .. } => {
            let has_server = self
              .lsp_runtimes
              .get(&runtime_id)
              .is_some_and(|runtime| runtime.runtime.config().server().is_some());
            if has_server {
              self.set_lsp_status_for_runtime(
                runtime_id,
                LspStatusPhase::Starting,
                Some("starting".into()),
              );
            } else {
              self.set_lsp_status_for_runtime(
                runtime_id,
                LspStatusPhase::Off,
                Some("unavailable".into()),
              );
            }
            needs_render = true;
          },
          LspEvent::CapabilitiesRegistered { server_name } => {
            let matches_configured_server = self
              .lsp_runtimes
              .get(&runtime_id)
              .and_then(|runtime| runtime.runtime.config().server())
              .is_some_and(|server| server.name() == server_name);
            if matches_configured_server {
              if let Some(runtime) = self.lsp_runtimes.get_mut(&runtime_id) {
                runtime.ready = true;
                runtime.active_progress_tokens.clear();
              }
              self.open_attached_documents_for_runtime(runtime_id);
              self.set_lsp_status_for_runtime(runtime_id, LspStatusPhase::Ready, Some(server_name));
              self.sync_active_lsp_mirror_state();
              needs_render = true;
            }
          },
          LspEvent::ServerStarted { server_name, .. } => {
            if let Some(runtime) = self.lsp_runtimes.get_mut(&runtime_id) {
              runtime.ready = false;
              runtime.active_progress_tokens.clear();
              runtime.pending_requests.clear();
            }
            for buffer_state in self.buffer_lsp_states.values_mut() {
              buffer_state.opened_runtime_ids.remove(&runtime_id);
            }
            if self.active_lsp_runtime_id == Some(runtime_id) {
              self.cancel_auto_signature_help();
              self.clear_signature_help_state();
            }
            self.set_lsp_status_for_runtime(
              runtime_id,
              LspStatusPhase::Starting,
              Some(server_name),
            );
            self.sync_active_lsp_mirror_state();
            needs_render = true;
          },
          LspEvent::RequestDispatched { method, .. } => {
            if method == "initialize" {
              self.set_lsp_status_for_runtime(
                runtime_id,
                LspStatusPhase::Initializing,
                Some("initializing".into()),
              );
              needs_render = true;
            }
          },
          LspEvent::ServerStopped { .. } | LspEvent::Stopped => {
            if let Some(runtime) = self.lsp_runtimes.get_mut(&runtime_id) {
              runtime.ready = false;
              runtime.active_progress_tokens.clear();
              runtime.pending_requests.clear();
            }
            for buffer_state in self.buffer_lsp_states.values_mut() {
              buffer_state.opened_runtime_ids.remove(&runtime_id);
            }
            if self.active_lsp_runtime_id == Some(runtime_id) {
              self.cancel_auto_signature_help();
              self.clear_signature_help_state();
            }
            self.set_lsp_status_for_runtime(
              runtime_id,
              LspStatusPhase::Starting,
              Some("restarting".into()),
            );
            self.sync_active_lsp_mirror_state();
            needs_render = true;
          },
          LspEvent::RpcMessage { message } => {
            needs_render |= self.handle_lsp_rpc_message(runtime_id, message);
          },
          LspEvent::RequestTimedOut { id, method } => {
            let pending = self
              .lsp_runtimes
              .get_mut(&runtime_id)
              .and_then(|runtime| runtime.pending_requests.remove(&id));
            self.sync_active_lsp_mirror_state();
            if let Some(pending) = pending {
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
            self.set_lsp_status_for_runtime(
              runtime_id,
              LspStatusPhase::Error,
              Some("request timeout".into()),
            );
            needs_render = true;
          },
          LspEvent::Progress { progress } => {
            match progress.kind {
              LspProgressKind::Begin => {
                let text =
                  format_lsp_progress_text(progress.title.as_deref(), progress.message.as_deref());
                if let Some(runtime) = self.lsp_runtimes.get_mut(&runtime_id) {
                  runtime.active_progress_tokens.insert(progress.token);
                }
                self.set_lsp_status_for_runtime(
                  runtime_id,
                  LspStatusPhase::Busy,
                  Some(text.clone()),
                );
                self.messages.publish_with_disposition(
                  MessageLevel::Info,
                  Some("lsp".into()),
                  MessageDisposition::Background,
                  text,
                );
                self.sync_active_lsp_mirror_state();
                needs_render = true;
              },
              LspProgressKind::End => {
                if let Some(runtime) = self.lsp_runtimes.get_mut(&runtime_id) {
                  runtime.active_progress_tokens.remove(&progress.token);
                  if runtime.ready && runtime.active_progress_tokens.is_empty() {
                    self.set_lsp_status_for_runtime(runtime_id, LspStatusPhase::Ready, None);
                    needs_render = true;
                  }
                }
                if let Some(message) = progress.message.and_then(non_empty_trimmed) {
                  self.messages.publish_with_disposition(
                    MessageLevel::Info,
                    Some("lsp".into()),
                    MessageDisposition::Background,
                    message,
                  );
                  needs_render = true;
                }
                self.sync_active_lsp_mirror_state();
              },
              LspProgressKind::Report => {
                let active = self
                  .lsp_runtimes
                  .get(&runtime_id)
                  .is_some_and(|runtime| runtime.active_progress_tokens.contains(&progress.token));
                if active {
                  let text = format_lsp_progress_text(
                    progress.title.as_deref(),
                    progress.message.as_deref(),
                  );
                  self.set_lsp_status_for_runtime(runtime_id, LspStatusPhase::Busy, Some(text));
                  self.sync_active_lsp_mirror_state();
                  needs_render = true;
                }
              },
            }
          },
          LspEvent::Error(message) => {
            self.set_lsp_status_error_for_runtime(runtime_id, &message);
            self
              .messages
              .publish(MessageLevel::Error, Some("lsp".into()), message);
            self.sync_active_lsp_mirror_state();
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
            let next_counts = self.diagnostics.apply_document_for_provider(
              &Self::lsp_runtime_provider_key(runtime_id),
              diagnostics,
            );
            if active_uri.is_some_and(|uri| uri == diagnostic_uri) && previous_counts != next_counts
            {
              self.publish_lsp_diagnostic_message(next_counts);
              needs_render = true;
            }
          },
          LspEvent::WorkspaceApplyEdit { label, edit } => {
            let source = label.as_deref().unwrap_or("code action");
            let _ = self.apply_workspace_edit(&edit, source);
            needs_render = true;
          },
          _ => {},
        }
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

  pub fn poll_lsp_signature_help_auto_trigger(&mut self) -> bool {
    let Some(pending) = self.lsp_pending_auto_signature_help.clone() else {
      return false;
    };
    if self.mode != Mode::Insert {
      self.lsp_pending_auto_signature_help = None;
      return false;
    }
    if Instant::now() < pending.due_at {
      return false;
    }

    self.lsp_pending_auto_signature_help = None;
    let _ = self.dispatch_signature_help_request(pending.trigger, false);
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
      let runtime_ids = self
        .active_buffer_lsp_state()
        .map(|state| state.attached_runtime_ids.clone())
        .unwrap_or_default();
      for runtime_id in runtime_ids {
        if let Some(runtime) = self.lsp_runtimes.get(&runtime_id) {
          let _ = runtime
            .runtime
            .send_notification("workspace/didChangeWatchedFiles", Some(params.clone()));
        }
      }
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

  pub fn poll_vcs_watch(&mut self) -> bool {
    let mut needs_render = self.sync_vcs_watch_state();

    let outcome = match poll_watch_events(
      self.vcs_watch.as_mut().map(|watch| &mut watch.stream),
      Instant::now(),
      "vcs",
      |event, message| trace_file_watch_event(event, message),
    ) {
      WatchPollOutcome::NoChanges => return needs_render,
      WatchPollOutcome::Disconnected { .. } => {
        self.vcs_watch = None;
        self.schedule_file_tree_vcs_refresh(
          FileTreeVcsRefreshReason::VcsWatch,
          Some(Instant::now() + file_tree_changed_refresh_latency()),
        );
        needs_render = true;
        true
      },
      WatchPollOutcome::Changes { .. } => true,
    };

    if outcome {
      needs_render |= self.handle_vcs_watch_change();
    }

    needs_render
  }

  pub fn poll_file_tree_watch(&mut self) -> bool {
    let root_before = self.file_tree_decoration_root.clone();
    let mut needs_render = the_default::poll_file_tree_watch(self);
    let root_after = self.file_tree_watch_root();

    match root_after.as_deref() {
      Some(root) => {
        if root_before.as_deref() != Some(root) {
          let reason = if root_before.is_none() {
            FileTreeVcsRefreshReason::TreeOpen
          } else {
            FileTreeVcsRefreshReason::TreeRootChange
          };
          self.file_tree_decoration_root = Some(root.to_path_buf());
          self.file_tree_diagnostics_seq = 0;
          self.schedule_file_tree_vcs_refresh(reason, Some(Instant::now()));
          needs_render = true;
        }
      },
      None => {
        self.clear_pending_file_tree_vcs_refresh();
        let cleared_decorations = clear_file_tree_decorations(self);
        if self.file_tree_decoration_root.take().is_some() || cleared_decorations {
          self.file_tree_diagnostics_seq = 0;
          needs_render = true;
        }
      },
    }

    needs_render | self.refresh_file_tree_diagnostics_if_needed()
  }

  fn handle_vcs_watch_change(&mut self) -> bool {
    self.schedule_active_file_vcs_refresh(
      ActiveFileVcsRefreshReason::VcsWatch,
      Some(Instant::now() + vcs_watch_latency()),
    );
    self.schedule_file_tree_vcs_refresh(
      FileTreeVcsRefreshReason::VcsWatch,
      Some(Instant::now() + file_tree_changed_refresh_latency()),
    );
    self.queue_open_vcs_diff_picker_refresh();
    false
  }

  pub fn poll_file_tree_vcs_refresh_dispatch(&mut self, now: Instant) -> bool {
    if self.file_tree_vcs_refresh_in_flight {
      return false;
    }
    let Some(due_at) = self.file_tree_vcs_refresh_due_at else {
      return false;
    };
    if now < due_at {
      return false;
    }
    let Some(root) = self.file_tree_decoration_root.clone() else {
      self.clear_pending_file_tree_vcs_refresh();
      return false;
    };
    let reason = self
      .file_tree_vcs_refresh_reason
      .take()
      .unwrap_or(FileTreeVcsRefreshReason::VcsWatch);
    self.file_tree_vcs_refresh_due_at = None;
    self.file_tree_vcs_refresh_in_flight = true;
    self.file_tree_vcs_refresh_rerun = false;
    self.file_tree_vcs_refresh_generation += 1;
    let generation = self.file_tree_vcs_refresh_generation;
    let vcs_provider = self.vcs_provider.clone();
    let tx = self.file_tree_vcs_refresh_tx.clone();
    let wake_tx = self.render_wake_tx.clone();
    log_file_tree_vcs_refresh_event(
      "started", generation, &root, reason, None, None, None, None, None, None,
    );
    thread::spawn(move || {
      let collect_start = Instant::now();
      let (change_count, statuses, scan) = match vcs_provider.scan_workspace(&root) {
        Ok(scan) => {
          let collect_ms = collect_start.elapsed().as_secs_f64() * 1000.0;
          let collapse_start = Instant::now();
          let statuses = collapse_file_tree_vcs_statuses(&scan.changes, &root);
          let collapse_ms = collapse_start.elapsed().as_secs_f64() * 1000.0;
          log_file_tree_vcs_refresh_event(
            "finished",
            generation,
            &root,
            reason,
            Some(scan.changes.len()),
            Some(statuses.len()),
            Some(collect_ms),
            Some(collapse_ms),
            None,
            None,
          );
          let _ = tx.send(FileTreeVcsRefreshResult {
            generation,
            root,
            reason,
            status_entries: statuses.len(),
            statuses,
            change_count: scan.changes.len(),
            collect_ms,
            collapse_ms,
            scan: Some(scan),
          });
          let _ = wake_tx.send(());
          return;
        },
        Err(err) => {
          let _ = err;
          (0, BTreeMap::new(), None)
        },
      };
      let collect_ms = collect_start.elapsed().as_secs_f64() * 1000.0;
      log_file_tree_vcs_refresh_event(
        "finished",
        generation,
        &root,
        reason,
        Some(change_count),
        Some(statuses.len()),
        Some(collect_ms),
        Some(0.0),
        None,
        None,
      );
      let _ = tx.send(FileTreeVcsRefreshResult {
        generation,
        root,
        reason,
        status_entries: statuses.len(),
        statuses,
        change_count,
        collect_ms,
        collapse_ms: 0.0,
        scan,
      });
      let _ = wake_tx.send(());
    });
    false
  }

  pub fn poll_file_tree_vcs_refresh_results(&mut self) -> bool {
    let mut needs_render = false;
    loop {
      let result = match self.file_tree_vcs_refresh_rx.try_recv() {
        Ok(result) => result,
        Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
      };
      if result.generation == self.file_tree_vcs_refresh_generation {
        self.file_tree_vcs_refresh_in_flight = false;
      }
      let current_root = self.file_tree_decoration_root.as_deref();
      let stale = current_root != Some(result.root.as_path())
        || result.generation != self.file_tree_vcs_refresh_generation;
      if stale {
        log_file_tree_vcs_refresh_event(
          "discarded",
          result.generation,
          &result.root,
          result.reason,
          Some(result.change_count),
          Some(result.status_entries),
          Some(result.collect_ms),
          Some(result.collapse_ms),
          None,
          None,
        );
      } else {
        let apply_start = Instant::now();
        if let Some(scan) = result.scan {
          let _ = self.store_shared_vcs_scan(scan);
        }
        let status_entries = result.statuses.len();
        let decorations_changed = set_file_tree_vcs_statuses(self, result.statuses);
        let apply_ms = apply_start.elapsed().as_secs_f64() * 1000.0;
        log_file_tree_vcs_refresh_event(
          "applied",
          result.generation,
          &result.root,
          result.reason,
          Some(result.change_count),
          Some(status_entries),
          Some(result.collect_ms),
          Some(result.collapse_ms),
          Some(apply_ms),
          Some(decorations_changed),
        );
        needs_render |= decorations_changed;
      }
      if self.file_tree_vcs_refresh_rerun {
        self.file_tree_vcs_refresh_rerun = false;
        if self.file_tree_vcs_refresh_due_at.is_none() {
          self.file_tree_vcs_refresh_due_at = Some(Instant::now());
        }
      }
    }
    needs_render
  }

  fn sync_vcs_watch_state(&mut self) -> bool {
    let root = self.vcs_watch_root();
    let mut changed = false;

    match root {
      Some(root) => {
        let current = self
          .vcs_watch
          .as_ref()
          .map(|watch| watch.stream.path.as_path());
        if current != Some(root.as_path()) {
          let (events_rx, watch_handle) = watch_path(&root, vcs_watch_latency());
          let uri =
            file_uri_for_path(&root).unwrap_or_else(|| format!("file://{}", root.display()));
          self.vcs_watch = Some(VcsWatchState {
            stream:        WatchedFileEventsState {
              path: root,
              uri,
              events_rx,
              suppress_until: None,
              reload_state: FileWatchReloadState::Clean,
              reload_io: FileWatchReloadIoState::default(),
            },
            _watch_handle: watch_handle,
          });
          changed = true;
        }
      },
      None => {
        if self.vcs_watch.take().is_some() {
          changed = true;
        }
      },
    }

    changed
  }

  fn vcs_watch_root(&self) -> Option<PathBuf> {
    let cwd = self.effective_working_directory();
    if !cwd.exists() {
      return None;
    }
    self
      .vcs_provider
      .watch_root(cwd.as_path())
      .filter(|root| root.exists())
  }

  fn file_tree_watch_root(&self) -> Option<PathBuf> {
    let surface_id = self.file_tree.surface_id?;
    let attached = self
      .editor
      .client_surface_snapshots()
      .into_iter()
      .find(|surface| surface.client_surface_id == surface_id)
      .and_then(|surface| surface.attached_pane)
      .is_some();
    if !attached {
      return None;
    }
    self.file_tree.root.clone()
  }

  fn refresh_file_tree_diagnostics_if_needed(&mut self) -> bool {
    let Some(root) = self.file_tree_decoration_root.clone() else {
      return false;
    };

    let diagnostics_seq = self.diagnostics.latest_seq();
    if self.file_tree_diagnostics_seq == diagnostics_seq {
      return false;
    }
    let statuses = rebuild_file_tree_diagnostic_statuses(&self.diagnostics, &root);
    self.file_tree_diagnostics_seq = diagnostics_seq;
    set_file_tree_diagnostic_statuses(self, statuses)
  }

  fn schedule_file_tree_vcs_refresh(
    &mut self,
    reason: FileTreeVcsRefreshReason,
    due_at: Option<Instant>,
  ) {
    let Some(root) = self.file_tree_decoration_root.clone() else {
      return;
    };
    let due_at = due_at.unwrap_or_else(|| Instant::now());
    self.file_tree_vcs_refresh_due_at = Some(
      self
        .file_tree_vcs_refresh_due_at
        .map_or(due_at, |current| current.min(due_at)),
    );
    self.file_tree_vcs_refresh_reason = Some(
      self
        .file_tree_vcs_refresh_reason
        .map_or(reason, |current| current.combine(reason)),
    );
    if self.file_tree_vcs_refresh_in_flight {
      self.file_tree_vcs_refresh_rerun = true;
    }
    log_file_tree_vcs_refresh_event(
      "scheduled",
      self.file_tree_vcs_refresh_generation + u64::from(self.file_tree_vcs_refresh_in_flight),
      &root,
      reason,
      None,
      None,
      None,
      None,
      None,
      None,
    );
  }

  fn clear_pending_file_tree_vcs_refresh(&mut self) {
    self.file_tree_vcs_refresh_due_at = None;
    self.file_tree_vcs_refresh_reason = None;
    self.file_tree_vcs_refresh_in_flight = false;
    self.file_tree_vcs_refresh_rerun = false;
  }

  fn handle_lsp_rpc_message(
    &mut self,
    runtime_id: LspRuntimeId,
    message: jsonrpc::Message,
  ) -> bool {
    let jsonrpc::Message::Response(response) = message else {
      return false;
    };
    let jsonrpc::Id::Number(id) = response.id else {
      return false;
    };
    let Some(kind) = self
      .lsp_runtimes
      .get_mut(&runtime_id)
      .and_then(|runtime| runtime.pending_requests.remove(&id))
    else {
      return false;
    };
    self.sync_active_lsp_mirror_state();
    self.handle_lsp_response(response, kind)
  }

  fn handle_lsp_response(
    &mut self,
    response: jsonrpc::Response,
    kind: PendingLspRequestKind,
  ) -> bool {
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
      PendingLspRequestKind::GotoDeclaration { .. } => {
        let locations = match parse_locations_response(response.result.as_ref()) {
          Ok(locations) => locations,
          Err(err) => {
            self.messages.publish(
              MessageLevel::Error,
              Some("lsp".into()),
              format!("failed to parse goto-declaration response: {err}"),
            );
            return true;
          },
        };
        if locations.is_empty() {
          let _ = <Self as the_default::DefaultContext>::push_error(
            self,
            "goto",
            "No declaration found.",
          );
          return true;
        }
        self.apply_locations_result("declaration", locations)
      },
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
        if locations.is_empty() {
          let _ =
            <Self as the_default::DefaultContext>::push_error(self, "goto", "No definition found.");
          return true;
        }
        self.apply_locations_result("definition", locations)
      },
      PendingLspRequestKind::GotoTypeDefinition { .. } => {
        let locations = match parse_locations_response(response.result.as_ref()) {
          Ok(locations) => locations,
          Err(err) => {
            self.messages.publish(
              MessageLevel::Error,
              Some("lsp".into()),
              format!("failed to parse goto-type-definition response: {err}"),
            );
            return true;
          },
        };
        if locations.is_empty() {
          let _ = <Self as the_default::DefaultContext>::push_error(
            self,
            "goto",
            "No type definition found.",
          );
          return true;
        }
        self.apply_locations_result("type definition", locations)
      },
      PendingLspRequestKind::GotoImplementation { .. } => {
        let locations = match parse_locations_response(response.result.as_ref()) {
          Ok(locations) => locations,
          Err(err) => {
            self.messages.publish(
              MessageLevel::Error,
              Some("lsp".into()),
              format!("failed to parse goto-implementation response: {err}"),
            );
            return true;
          },
        };
        if locations.is_empty() {
          let _ = <Self as the_default::DefaultContext>::push_error(
            self,
            "goto",
            "No implementation found.",
          );
          return true;
        }
        self.apply_locations_result("implementation", locations)
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
            let trimmed = text.trim();
            if trimmed.is_empty() {
              self.clear_hover_state();
              self.messages.publish(
                MessageLevel::Info,
                Some("lsp".into()),
                "no hover information",
              );
            } else {
              self.hover_docs = Some(trimmed.to_string());
              self.hover_docs_scroll = 0;
            }
          },
          None => {
            self.clear_hover_state();
            self.messages.publish(
              MessageLevel::Info,
              Some("lsp".into()),
              "no hover information",
            );
          },
        }
        true
      },
      PendingLspRequestKind::DocumentHighlightSelect { .. } => {
        self.handle_document_highlight_selection_response(response.result.as_ref())
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
        self.handle_completion_resolve_response(index, &response)
      },
      PendingLspRequestKind::SignatureHelp { .. } => {
        self.handle_signature_help_response(response.result.as_ref())
      },
      PendingLspRequestKind::CodeActions { .. } => {
        self.handle_code_actions_response(response.result.as_ref())
      },
      PendingLspRequestKind::CodeActionResolve { action, .. } => {
        self.handle_code_action_resolve_response(action, response.result.as_ref())
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

    let root = the_default::workspace_root(self.effective_working_directory().as_path());
    let active_uri = self.current_lsp_uri();
    let mut external_rope_cache: HashMap<PathBuf, Rope> = HashMap::new();
    let mut items = Vec::with_capacity(symbols.len());
    let mut symbol_stack: Vec<String> = Vec::new();
    let mut previous_path: Option<PathBuf> = None;

    for symbol in symbols {
      let Some(location) = symbol.location.as_ref() else {
        continue;
      };
      let Some(path) = path_for_file_uri(&location.uri) else {
        continue;
      };

      let line = location.range.start.line as usize;
      let character = location.range.start.character as usize;
      let cursor_char = if active_uri.as_deref() == Some(location.uri.as_str()) {
        utf16_position_to_char_idx(
          self.editor.document().text(),
          location.range.start.line,
          location.range.start.character,
        )
      } else {
        let rope = external_rope_cache.entry(path.clone()).or_insert_with(|| {
          std::fs::read_to_string(&path)
            .map(|content| Rope::from(content.as_str()))
            .unwrap_or_else(|_| Rope::from(""))
        });
        utf16_position_to_char_idx(
          rope,
          location.range.start.line,
          location.range.start.character,
        )
      };

      let path_display = path
        .strip_prefix(&root)
        .unwrap_or(&path)
        .display()
        .to_string();
      if previous_path.as_ref().is_none_or(|prev| prev != &path) {
        symbol_stack.clear();
        previous_path = Some(path.clone());
      }
      let kind_label = lsp_symbol_kind_label(symbol.kind);
      let name = sanitize_picker_field(symbol.name.trim());
      let name = if name.is_empty() {
        "<unnamed>".to_string()
      } else {
        name
      };
      let container = sanitize_picker_field(symbol.container_name.as_deref().unwrap_or_default());
      let detail = sanitize_picker_field(symbol.detail.as_deref().unwrap_or_default());
      let path_field = sanitize_picker_field(path_display.as_str());
      let depth = lsp_symbol_tree_depth(container.as_str(), &mut symbol_stack);
      symbol_stack.truncate(depth);
      symbol_stack.push(name.clone());
      let display = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        name,
        container,
        detail,
        kind_label,
        path_field,
        line.saturating_add(1),
        character.saturating_add(1),
        depth
      );
      let icon = lsp_symbol_icon_name(symbol.kind).to_string();

      items.push(FilePickerItem {
        absolute: path.clone(),
        display,
        icon,
        is_dir: false,
        display_path: false,
        action: FilePickerItemAction::OpenLocation {
          path: path.clone(),
          cursor_char,
          line,
          column: None,
        },
        preview_path: Some(path),
        preview_line: Some(line),
        preview_col: None,
        row_data: None,
        preview: None,
        payload: None,
      });
    }

    if items.is_empty() {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        format!("{label}: results had no navigable locations"),
      );
      return true;
    }

    let title = if label.contains("workspace") {
      "Workspace Symbols"
    } else {
      "Lsp Symbols"
    };
    the_default::open_custom_picker(self, title, root, None, items, 0);
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
    let Some(current_cursor) = self.active_cursor_char_idx() else {
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
      self.lsp_completion_resolve_supported = self.lsp_completion_server_supports_resolve();
      self.lsp_completion_inline_item_active = false;
      self.lsp_completion_visible_indices.clear();
      self.lsp_completion_fallback_start = None;
      let keep_inline_item =
        self.completion_menu.active && self.lsp_completion_inline_menu_item().is_some();
      if keep_inline_item {
        the_default::show_builtin_completion_menu(
          self,
          the_default::BuiltinCompletionMenuKind::LspCompletion,
        );
      } else {
        self.completion_menu.clear();
      }
      if announce_empty && !keep_inline_item {
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
    self.lsp_completion_resolve_supported = self.lsp_completion_server_supports_resolve();
    self.lsp_completion_fallback_start = Some(replace_start.min(request_cursor));
    self.rebuild_completion_menu();
    true
  }

  fn handle_completion_resolve_response(
    &mut self,
    index: usize,
    response: &jsonrpc::Response,
  ) -> bool {
    if let Some(error) = response.error.as_ref() {
      self.lsp_completion_resolved_indices.insert(index);
      if lsp_method_is_unsupported(error) {
        self.lsp_completion_resolve_supported = false;
        return true;
      }
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        format!("completion resolve failed: {}", error.message),
      );
      return true;
    }

    let resolved = match parse_completion_item_response(response.result.as_ref()) {
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
    let visible_index = self.completion_visible_index_for_source_index(index);
    let Some(item) = self.lsp_completion_items.get_mut(index) else {
      return true;
    };
    merge_resolved_completion_item(item, resolved);

    if let Some(visible_index) = visible_index
      && let Some(ui_item) = self.completion_menu.items.get_mut(visible_index)
    {
      *ui_item = completion_menu_item_for_lsp_item(item);
      self.needs_render = true;
    }
    true
  }

  pub fn poll_vcs_diff_picker(&mut self) -> bool {
    if !self.file_picker.active || self.file_picker.kind != the_default::FilePickerKind::VcsDiff {
      if let Some(cancel) = self.vcs_diff_picker.cancel.take() {
        cancel.store(true, Ordering::Relaxed);
      }
      self.vcs_diff_picker.result_rx = None;
      self.vcs_diff_picker.live_refresh_pending = false;
      return false;
    }

    let cwd = self.effective_working_directory();
    let scan = if self.vcs_diff_picker.live_refresh_pending {
      self.refresh_shared_vcs_scan_for_cwd(&cwd)
    } else {
      self.shared_vcs_scan_for_cwd(&cwd)
    };
    let needs_restart = scan.as_ref().is_some_and(|_| {
      self.vcs_diff_picker.scan_generation != self.shared_vcs.generation
        || self.vcs_diff_picker.live_refresh_pending
    });

    let mut changed = false;
    let mut clear_receiver = false;
    if let Some(result_rx) = self.vcs_diff_picker.result_rx.as_ref() {
      loop {
        match result_rx.try_recv() {
          Ok(VcsDiffPickerFileResult::Entry {
            generation,
            index,
            entry,
          }) => {
            if generation == self.vcs_diff_picker.generation
              && let Some(slot) = self.vcs_diff_picker.entries.get_mut(index)
            {
              *slot = entry;
              changed = true;
            }
          },
          Ok(VcsDiffPickerFileResult::Complete { generation }) => {
            if generation == self.vcs_diff_picker.generation {
              clear_receiver = true;
            }
          },
          Err(TryRecvError::Empty) => break,
          Err(TryRecvError::Disconnected) => {
            clear_receiver = true;
            break;
          },
        }
      }
    }
    if clear_receiver {
      self.vcs_diff_picker.result_rx = None;
      self.vcs_diff_picker.cancel = None;
    }

    if needs_restart
      && self.vcs_diff_picker.result_rx.is_none()
      && let Some(scan) = scan
    {
      return restart_open_vcs_diff_picker_from_scan(self, scan);
    }

    if !changed {
      return false;
    }

    let submit_handler = self
      .file_picker
      .runtime_session()
      .map(the_default::PickerSubmitHandlerRef::Runtime);
    let specs =
      file_picker_vcs_diff_specs(&self.vcs_diff_picker.root, &self.vcs_diff_picker.entries);
    let items = file_picker_items_from_specs(specs, submit_handler);
    replace_file_picker_items_preserving_selection_and_viewport(self, items, 0);
    true
  }

  fn apply_completion_item(
    &mut self,
    item: LspCompletionItem,
    fallback_range: std::ops::Range<usize>,
  ) -> bool {
    let prepared = normalize_completion_item_for_apply(item);
    let item = prepared.item;
    let has_text_edits = item.primary_edit.is_some() || !item.additional_edits.is_empty();
    if has_text_edits {
      let Some(_uri) = self.current_lsp_uri() else {
        self.messages.publish(
          MessageLevel::Warning,
          Some("lsp".into()),
          "completion unavailable: no active LSP document",
        );
        return false;
      };

      let snippet_base =
        if prepared.cursor_origin == Some(CompletionSnippetCursorOrigin::PrimaryEdit) {
          item.primary_edit.as_ref().map(|edit| {
            utf16_position_to_char_idx(
              self.editor.document().text(),
              edit.range.start.line,
              edit.range.start.character,
            )
          })
        } else {
          None
        };

      let mut edits = Vec::with_capacity(1 + item.additional_edits.len());
      if let Some(primary) = item.primary_edit {
        edits.push(primary);
      }
      edits.extend(item.additional_edits);
      let tx = match build_transaction_from_lsp_text_edits(self.editor.document().text(), &edits) {
        Ok(tx) => tx,
        Err(err) => {
          self.messages.publish(
            MessageLevel::Error,
            Some("lsp".into()),
            format!("failed to build completion transaction: {err}"),
          );
          return false;
        },
      };

      if <Self as the_default::DefaultContext>::apply_transaction(self, &tx) {
        if let (Some(base), Some(range)) = (snippet_base, prepared.cursor_range.as_ref())
          && let Ok(mapped_base) = tx.changes().map_pos(base, Assoc::Before)
        {
          set_completion_snippet_selection(self.editor.document_mut(), mapped_base, range);
        }
        let _ = self.editor.document_mut().commit();
        <Self as the_default::DefaultContext>::request_render(self);
        return true;
      }

      self.messages.publish(
        MessageLevel::Error,
        Some("lsp".into()),
        "failed to apply completion",
      );
      return false;
    }

    let insert_text = item.insert_text.unwrap_or(item.label);
    if insert_text.is_empty() {
      return false;
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
        return false;
      },
    };

    if <Self as the_default::DefaultContext>::apply_transaction(self, &tx) {
      if prepared.cursor_origin == Some(CompletionSnippetCursorOrigin::InsertText)
        && let Some(range) = prepared.cursor_range.as_ref()
        && let Ok(mapped_base) = tx.changes().map_pos(from, Assoc::Before)
      {
        set_completion_snippet_selection(self.editor.document_mut(), mapped_base, range);
      }
      let _ = self.editor.document_mut().commit();
      <Self as the_default::DefaultContext>::request_render(self);
      self
        .messages
        .publish(MessageLevel::Info, Some("lsp".into()), "completion applied");
      return true;
    } else {
      self.messages.publish(
        MessageLevel::Error,
        Some("lsp".into()),
        "failed to apply completion",
      );
    }
    false
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
      self.lsp_signature_help_presentation = None;
      the_default::close_signature_help(self);
      return true;
    };

    if signature.signatures.is_empty() {
      self.lsp_signature_help_presentation = None;
      the_default::close_signature_help(self);
      return true;
    }

    let active_signature = signature.active_signature;
    let signatures = signature
      .signatures
      .into_iter()
      .map(|item| {
        the_default::SignatureHelpItem {
          label:                  item.label,
          documentation:          item.documentation,
          active_parameter:       item.active_parameter,
          active_parameter_range: item.active_parameter_range,
        }
      })
      .collect::<Vec<_>>();
    self.lsp_signature_help_presentation = Some(the_default::SignatureHelpPresentation::new(
      signatures,
      active_signature,
    ));
    the_default::show_builtin_signature_help(self);
    true
  }

  fn handle_code_actions_response(&mut self, result: Option<&Value>) -> bool {
    let actions = match parse_code_actions_response(result) {
      Ok(actions) => actions,
      Err(err) => {
        self.clear_code_action_menu_state();
        self.completion_menu.clear();
        self.messages.publish(
          MessageLevel::Error,
          Some("lsp".into()),
          format!("failed to parse code actions response: {err}"),
        );
        return true;
      },
    };

    if actions.is_empty() {
      self.clear_code_action_menu_state();
      self.completion_menu.clear();
      let _ = <Self as the_default::DefaultContext>::push_error(
        self,
        "code actions",
        "No code actions available",
      );
      return true;
    }

    self.show_code_action_menu(actions);
    true
  }

  fn handle_code_action_resolve_response(
    &mut self,
    action: LspCodeAction,
    result: Option<&Value>,
  ) -> bool {
    let resolved = match parse_code_action_response(result) {
      Ok(action) => action,
      Err(err) => {
        self.messages.publish(
          MessageLevel::Warning,
          Some("lsp".into()),
          format!("failed to parse code action resolve response: {err}"),
        );
        return true;
      },
    };

    let action = match resolved {
      Some(resolved) => action.merge_resolved(resolved),
      None => action,
    };
    self.apply_code_action_now(action)
  }

  fn handle_document_highlight_selection_response(&mut self, result: Option<&Value>) -> bool {
    let highlights = match parse_document_highlights_response(result) {
      Ok(highlights) => highlights,
      Err(err) => {
        self.messages.publish(
          MessageLevel::Error,
          Some("lsp".into()),
          format!("failed to parse document-highlight response: {err}"),
        );
        return true;
      },
    };

    if highlights.is_empty() {
      self.messages.publish(
        MessageLevel::Info,
        Some("lsp".into()),
        "no references under cursor",
      );
      return true;
    }

    let doc = self.editor.document();
    let text = doc.text();
    let text_slice = text.slice(..);
    let cursor_pos = self
      .active_or_first_selection_range()
      .map(|range| range.cursor(text_slice))
      .unwrap_or(0);

    let mut ranges: SmallVec<[Range; 1]> = SmallVec::new();
    let mut primary_index = 0usize;
    for highlight in highlights {
      let start = utf16_position_to_char_idx(text, highlight.start.line, highlight.start.character);
      let end = utf16_position_to_char_idx(text, highlight.end.line, highlight.end.character);
      let range = Range::new(start.min(end), start.max(end));
      if range.contains(cursor_pos) {
        primary_index = ranges.len();
      }
      ranges.push(range);
    }

    if ranges.is_empty() {
      self.messages.publish(
        MessageLevel::Info,
        Some("lsp".into()),
        "no references under cursor",
      );
      return true;
    }

    let next_selection = match Selection::new(ranges) {
      Ok(selection) => selection,
      Err(err) => {
        self.messages.publish(
          MessageLevel::Error,
          Some("lsp".into()),
          format!("failed to apply document highlights: {err}"),
        );
        return true;
      },
    };

    let next_active_cursor = next_selection
      .cursor_id_at(primary_index.min(next_selection.len().saturating_sub(1)))
      .ok();

    let _ = self.editor.document_mut().set_selection(next_selection);
    self.editor.view_mut().active_cursor = next_active_cursor;
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
    let Some(runtime_id) = self.active_lsp_runtime_for_capability(LspCapability::WorkspaceCommand)
    else {
      return false;
    };
    match self
      .lsp_runtimes
      .get(&runtime_id)
      .expect("active workspace command runtime")
      .runtime
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

  fn lsp_supports_code_action_resolve(&self) -> bool {
    let Some(runtime_id) = self.active_lsp_runtime_for_capability(LspCapability::CodeAction) else {
      return false;
    };
    self
      .managed_runtime_capabilities(runtime_id)
      .is_some_and(|capabilities| capabilities.supports_code_action_resolve())
  }

  fn resolve_code_action(&mut self, action: LspCodeAction) -> bool {
    if !self.lsp_supports_code_action_resolve() || !action.needs_resolve() {
      return false;
    }

    let Some(uri) = self.current_lsp_uri() else {
      return false;
    };
    let Some(params) = action.raw.clone() else {
      return false;
    };

    let Some(runtime_id) = self.active_lsp_runtime_for_capability(LspCapability::CodeAction) else {
      return false;
    };
    match self
      .lsp_runtimes
      .get(&runtime_id)
      .expect("active runtime")
      .runtime
      .send_request("codeAction/resolve", Some(params))
    {
      Ok(request_id) => {
        if let Some(runtime) = self.lsp_runtimes.get_mut(&runtime_id) {
          runtime
            .pending_requests
            .insert(request_id, PendingLspRequestKind::CodeActionResolve {
              uri,
              action,
            });
        }
        self.sync_active_lsp_mirror_state();
        true
      },
      Err(err) => {
        self.messages.publish(
          MessageLevel::Warning,
          Some("lsp".into()),
          format!("failed to dispatch codeAction/resolve: {err}"),
        );
        false
      },
    }
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

    // Match Helix behavior: record the origin before any goto jump so C-o can
    // return.
    let _ = <Self as the_default::DefaultContext>::save_selection_to_jumplist(self);

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
    self.active_lsp_runtime_for_capability(capability).is_some()
  }

  fn active_or_first_selection_range(&self) -> Option<Range> {
    let doc = self.editor.document();
    let selection = doc.selection();
    if let Some(active_cursor) = self.editor.view().active_cursor
      && let Some(range) = selection.range_by_id(active_cursor)
    {
      return Some(*range);
    }
    selection.ranges().first().copied()
  }

  pub(crate) fn buffer_tabs_snapshot_for_ui(&self) -> BufferTabsSnapshot {
    buffer_tabs_snapshot(self)
  }

  pub(crate) fn buffer_tabs_top_chrome_rows(&self) -> u16 {
    if self.buffer_tabs_snapshot_for_ui().visible {
      1
    } else {
      0
    }
  }

  pub(crate) fn buffer_tab_layout_slots(
    &self,
    width: u16,
  ) -> (BufferTabsSnapshot, Vec<BufferTabLayoutSlot>) {
    let snapshot = self.buffer_tabs_snapshot_for_ui();
    if !snapshot.visible || width == 0 || snapshot.tabs.is_empty() {
      return (snapshot, Vec::new());
    }

    const MIN_TAB_WIDTH: u16 = 12;
    const MAX_TAB_WIDTH: u16 = 34;
    let mut slots = Vec::new();
    let mut x = 0u16;
    for (tab_index, tab) in snapshot.tabs.iter().enumerate() {
      if x >= width {
        break;
      }
      let title_len = tab.title.chars().count() as u16;
      let modified_extra = if tab.modified { 2 } else { 0 }; // "● "
      let icon_extra = 2; // icon + gap
      let close_extra = 3; // space + "×" + trailing pad
      let padding = 2; // left + trailing room
      let desired = title_len
        .saturating_add(modified_extra)
        .saturating_add(icon_extra)
        .saturating_add(close_extra)
        .saturating_add(padding)
        .clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH);
      let remaining = width.saturating_sub(x);
      if remaining < MIN_TAB_WIDTH && !slots.is_empty() {
        break;
      }
      let tab_width = desired.min(remaining);
      if tab_width == 0 {
        break;
      }
      let close_x = if tab_width >= MIN_TAB_WIDTH {
        Some(x.saturating_add(tab_width.saturating_sub(2)))
      } else {
        None
      };
      slots.push(BufferTabLayoutSlot {
        tab_index,
        buffer_id: tab.buffer_id,
        x,
        width: tab_width,
        close_x,
      });
      x = x.saturating_add(tab_width);
    }
    (snapshot, slots)
  }

  pub(crate) fn buffer_tab_close_buffer_id_at(
    &self,
    x: u16,
    y: u16,
    width: u16,
  ) -> Option<BufferId> {
    if y >= self.buffer_tabs_top_chrome_rows() {
      return None;
    }
    let (_, slots) = self.buffer_tab_layout_slots(width);
    slots
      .into_iter()
      .find(|slot| slot.close_x == Some(x))
      .map(|slot| slot.buffer_id)
  }

  pub(crate) fn update_buffer_tab_hover(&mut self, x: u16, y: u16, width: u16) {
    let next = if y < self.buffer_tabs_top_chrome_rows() {
      if let Some(buffer_id) = self.buffer_tab_close_buffer_id_at(x, y, width) {
        Some(BufferTabHoverState {
          buffer_id,
          over_close: true,
        })
      } else {
        self.buffer_tab_slot_at(x, y, width).map(|slot| {
          BufferTabHoverState {
            buffer_id:  slot.buffer_id,
            over_close: false,
          }
        })
      }
    } else {
      None
    };
    if self.buffer_tab_hover != next {
      self.buffer_tab_hover = next;
      self.request_render();
    }
  }

  pub(crate) fn clear_buffer_tab_hover(&mut self) {
    if self.buffer_tab_hover.take().is_some() {
      self.request_render();
    }
  }

  pub(crate) fn buffer_tab_buffer_id_at(&self, x: u16, y: u16, width: u16) -> Option<BufferId> {
    if y >= self.buffer_tabs_top_chrome_rows() {
      return None;
    }
    let (_, slots) = self.buffer_tab_layout_slots(width);
    slots
      .into_iter()
      .find(|slot| x >= slot.x && x < slot.x.saturating_add(slot.width))
      .map(|slot| slot.buffer_id)
  }

  pub(crate) fn buffer_tab_slot_at(
    &self,
    x: u16,
    y: u16,
    width: u16,
  ) -> Option<BufferTabLayoutSlot> {
    if y >= self.buffer_tabs_top_chrome_rows() {
      return None;
    }
    let (_, slots) = self.buffer_tab_layout_slots(width);
    slots
      .into_iter()
      .find(|slot| x >= slot.x && x < slot.x.saturating_add(slot.width))
  }

  pub(crate) fn activate_buffer_tab(&mut self, buffer_id: BufferId) -> bool {
    the_default::activate_buffer_tab(self, buffer_id)
  }

  pub(crate) fn move_buffer_tab(&mut self, from: BufferId, to: BufferId) -> bool {
    if !self.editor.move_buffer(from, to) {
      return false;
    }
    self.request_render();
    true
  }

  pub(crate) fn close_buffer_tab(&mut self, buffer_id: BufferId) -> bool {
    let Some(snapshot) = self.editor.buffer_snapshot(buffer_id) else {
      return false;
    };
    if snapshot.modified {
      self.messages.publish(
        MessageLevel::Warning,
        Some("buffer".into()),
        format!("buffer '{}' has unsaved changes", snapshot.display_name),
      );
      self.request_render();
      return false;
    }
    if self.editor.buffer_count() <= 1 {
      self.messages.publish(
        MessageLevel::Warning,
        Some("buffer".into()),
        "cannot close the last buffer",
      );
      self.request_render();
      return false;
    }

    let closing_active = self.editor.active_buffer_id() == buffer_id;
    self.detach_buffer_lsp_state(buffer_id);
    if !self.editor.close_buffer(buffer_id) {
      return false;
    }

    if closing_active {
      self.syntax_parse_lifecycle.cancel_pending();
      self.highlight_cache.clear();
      self.inactive_highlight_caches.clear();
      if self.editor.document().syntax().is_some() {
        self.syntax_parse_highlight_state.mark_parsed();
      } else {
        self.syntax_parse_highlight_state.mark_cleared();
      }

      let active_path = self.editor.active_file_path().map(Path::to_path_buf);
      let previous_path = self.file_path.clone();
      self.file_path = active_path.clone();
      self.lsp_refresh_document_state(active_path.as_deref());
      self.lsp_open_current_document();
      self.clear_hover_state();
      self.refresh_active_file_vcs_after_path_change(
        previous_path,
        ActiveFileVcsRefreshReason::PathChange,
      );
    }

    self.request_render();
    true
  }

  pub(crate) fn begin_buffer_tab_drag(&mut self, slot: BufferTabLayoutSlot, pointer_x: u16) {
    let max_offset = slot.width.saturating_sub(1);
    self.buffer_tab_drag = Some(BufferTabPointerDragState {
      buffer_id: slot.buffer_id,
      pointer_x,
      press_x: pointer_x,
      grab_offset: pointer_x.saturating_sub(slot.x).min(max_offset),
      moved: false,
    });
  }

  pub(crate) fn update_buffer_tab_drag(&mut self, x: u16, y: u16, width: u16) {
    const DRAG_REORDER_THRESHOLD_CELLS: u16 = 2;
    let Some(mut drag) = self.buffer_tab_drag else {
      return;
    };
    drag.pointer_x = x;
    if drag.press_x.abs_diff(x) < DRAG_REORDER_THRESHOLD_CELLS {
      self.buffer_tab_drag = Some(drag);
      return;
    }

    let (_snapshot, slots) = self.buffer_tab_layout_slots(width);
    let Some(target_slot) = slots
      .iter()
      .find(|slot| {
        x >= slot.x
          && x < slot.x.saturating_add(slot.width)
          && y < self.buffer_tabs_top_chrome_rows()
      })
      .copied()
    else {
      self.buffer_tab_drag = Some(drag);
      return;
    };
    if target_slot.buffer_id == drag.buffer_id {
      self.buffer_tab_drag = Some(drag);
      return;
    }
    let Some(current_slot) = slots
      .iter()
      .find(|slot| slot.buffer_id == drag.buffer_id)
      .copied()
    else {
      self.buffer_tab_drag = Some(drag);
      return;
    };

    let target_mid = target_slot
      .x
      .saturating_add(target_slot.width.saturating_sub(1) / 2);
    if target_slot.x > current_slot.x {
      if x < target_mid {
        self.buffer_tab_drag = Some(drag);
        return;
      }
    } else if x >= target_mid {
      self.buffer_tab_drag = Some(drag);
      return;
    }

    if self.move_buffer_tab(drag.buffer_id, target_slot.buffer_id) {
      drag.buffer_id = target_slot.buffer_id;
      drag.moved = true;
      self.buffer_tab_drag = Some(drag);
    } else {
      self.buffer_tab_drag = Some(drag);
    }
  }

  pub(crate) fn finish_buffer_tab_drag(
    &mut self,
    x: u16,
    y: u16,
    width: u16,
  ) -> Option<(BufferId, bool)> {
    let drag = self.buffer_tab_drag.take()?;
    let slot = self.buffer_tab_slot_at(x, y, width)?;
    Some((slot.buffer_id, drag.moved))
  }

  fn pointer_event_screen_coords(&self, event: PointerEvent) -> Option<(u16, u16)> {
    let x = event.logical_col.or_else(|| {
      if event.x < 0 {
        None
      } else {
        Some(event.x.min(i32::from(u16::MAX)) as u16)
      }
    })?;
    let y = event.logical_row.or_else(|| {
      if event.y < 0 {
        None
      } else {
        Some(event.y.min(i32::from(u16::MAX)) as u16)
      }
    })?;
    Some((x, y))
  }

  fn pointer_hit_pane_at(&self, x: u16, y: u16) -> Option<PaneSnapshot> {
    let viewport = self.editor.layout_viewport();
    self
      .editor
      .pane_snapshots(viewport)
      .into_iter()
      .find(|pane| {
        x >= pane.rect.x
          && x < pane.rect.x.saturating_add(pane.rect.width)
          && y >= pane.rect.y
          && y < pane.rect.y.saturating_add(pane.rect.height)
      })
  }

  fn pointer_active_pane_snapshot(&self) -> Option<PaneSnapshot> {
    let viewport = self.editor.layout_viewport();
    self
      .editor
      .pane_snapshots(viewport)
      .into_iter()
      .find(|pane| pane.is_active_pane)
  }

  fn pointer_char_idx_for_pane_point(&self, pane: PaneSnapshot, x: u16, y: u16) -> Option<usize> {
    let max_x = pane
      .rect
      .x
      .saturating_add(pane.rect.width.saturating_sub(1));
    let max_y = pane
      .rect
      .y
      .saturating_add(pane.rect.height.saturating_sub(1));
    let x = x.clamp(pane.rect.x, max_x);
    let y = y.clamp(pane.rect.y, max_y);

    let doc = self.editor.buffer_document(pane.buffer_id)?;
    let view = self.editor.pane_view(pane.pane_id)?;
    let gutter_width = gutter_width_for_document(doc, view.viewport.width, &self.gutter_config);

    let local_row = usize::from(y.saturating_sub(pane.rect.y));
    let local_col = usize::from(x.saturating_sub(pane.rect.x).saturating_sub(gutter_width));

    let target = Position::new(
      view.scroll.row.saturating_add(local_row),
      view.scroll.col.saturating_add(local_col),
    );

    let mut text_format = <Self as DefaultContext>::text_format(self);
    text_format.viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1);
    let mut annotations = <Self as DefaultContext>::text_annotations(self);
    char_at_visual_pos(doc.text().slice(..), &text_format, &mut annotations, target)
  }

  fn pointer_set_primary_selection(&mut self, anchor: usize, head: usize) -> bool {
    self
      .editor
      .document_mut()
      .set_selection(Selection::single(anchor, head))
      .is_ok()
  }

  fn pointer_drag_mode_for_click_count(click_count: u8) -> PointerSelectionDragMode {
    if click_count >= 3 {
      PointerSelectionDragMode::Line
    } else if click_count == 2 {
      PointerSelectionDragMode::Word
    } else {
      PointerSelectionDragMode::Char
    }
  }

  fn pointer_selection_drag_state_for_target(
    &self,
    mode: PointerSelectionDragMode,
    target: usize,
    shift: bool,
  ) -> PointerSelectionDragState {
    match mode {
      PointerSelectionDragMode::Char => {
        let anchor = if shift {
          self
            .active_or_first_selection_range()
            .map(|range| range.anchor)
            .unwrap_or(target)
        } else {
          target
        };
        PointerSelectionDragState {
          mode,
          anchor,
          initial_from: target,
          initial_to: target,
        }
      },
      PointerSelectionDragMode::Word => {
        let (from, to) = self.pointer_word_range_at_char(target);
        PointerSelectionDragState {
          mode,
          anchor: from,
          initial_from: from,
          initial_to: to,
        }
      },
      PointerSelectionDragMode::Line => {
        let (from, to) = self.pointer_line_range_at_char(target);
        PointerSelectionDragState {
          mode,
          anchor: from,
          initial_from: from,
          initial_to: to,
        }
      },
    }
  }

  fn pointer_apply_drag_selection(
    &mut self,
    state: PointerSelectionDragState,
    target: usize,
  ) -> bool {
    match state.mode {
      PointerSelectionDragMode::Char => self.pointer_set_primary_selection(state.anchor, target),
      PointerSelectionDragMode::Word => {
        let (target_from, target_to) = self.pointer_word_range_at_char(target);
        if target_from < state.initial_from {
          self.pointer_set_primary_selection(state.initial_to, target_from)
        } else {
          self.pointer_set_primary_selection(state.initial_from, target_to)
        }
      },
      PointerSelectionDragMode::Line => {
        let (target_from, target_to) = self.pointer_line_range_at_char(target);
        if target_from < state.initial_from {
          self.pointer_set_primary_selection(state.initial_to, target_from)
        } else {
          self.pointer_set_primary_selection(state.initial_from, target_to)
        }
      },
    }
  }

  fn pointer_word_range_at_char(&self, target: usize) -> (usize, usize) {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum WordClass {
      Symbol,
      Whitespace,
      Other,
    }

    fn classify(ch: char) -> WordClass {
      if is_symbol_word_char(ch) {
        WordClass::Symbol
      } else if ch.is_whitespace() && ch != '\n' && ch != '\r' {
        WordClass::Whitespace
      } else {
        WordClass::Other
      }
    }

    let text = self.editor.document().text();
    let len = text.len_chars();
    if len == 0 {
      return (0, 0);
    }

    let clamped = target.min(len);
    let probe_char = if clamped == len {
      clamped.saturating_sub(1)
    } else {
      clamped
    };
    let line_idx = text.char_to_line(probe_char);
    let line_start = text.line_to_char(line_idx);
    let next_line_start = if line_idx + 1 < text.len_lines() {
      text.line_to_char(line_idx + 1)
    } else {
      len
    };

    let mut line: Vec<char> = text.slice(line_start..next_line_start).chars().collect();
    while matches!(line.last(), Some('\n' | '\r')) {
      line.pop();
    }
    if line.is_empty() {
      return (line_start, line_start);
    }

    let local = clamped
      .saturating_sub(line_start)
      .min(line.len().saturating_sub(1));
    let class = classify(line[local]);

    let mut start = local;
    while start > 0 && classify(line[start - 1]) == class {
      start -= 1;
    }
    let mut end = local + 1;
    while end < line.len() && classify(line[end]) == class {
      end += 1;
    }

    (line_start + start, line_start + end)
  }

  fn pointer_line_range_at_char(&self, target: usize) -> (usize, usize) {
    let text = self.editor.document().text();
    let len = text.len_chars();
    if len == 0 {
      return (0, 0);
    }
    let probe_char = target.min(len).saturating_sub(1).min(len.saturating_sub(1));
    let line_idx = text.char_to_line(probe_char);
    let line_start = text.line_to_char(line_idx);
    let line_end = if line_idx + 1 < text.len_lines() {
      text.line_to_char(line_idx + 1)
    } else {
      len
    };
    (line_start, line_end)
  }

  pub(crate) fn pointer_click_count_for_left_down(&mut self, x: u16, y: u16) -> u8 {
    const MULTI_CLICK_INTERVAL: Duration = Duration::from_millis(350);
    let now = Instant::now();
    let count = if let Some(prev) = self.mouse_last_primary_click {
      if now.duration_since(prev.at) <= MULTI_CLICK_INTERVAL
        && prev.x.abs_diff(x) <= 1
        && prev.y.abs_diff(y) <= 1
      {
        prev.count.saturating_add(1).min(3)
      } else {
        1
      }
    } else {
      1
    };
    self.mouse_last_primary_click = Some(PointerClickTracker {
      at: now,
      x,
      y,
      count,
    });
    count
  }

  fn pointer_scroll_active_view_by(&mut self, row_delta: i32, col_delta: i32) -> bool {
    if row_delta == 0 && col_delta == 0 {
      return false;
    }
    let soft_wrap = self.text_format.soft_wrap;
    let current = self.editor.view().scroll;
    let new_row = if row_delta >= 0 {
      current.row.saturating_add(row_delta as usize)
    } else {
      current.row.saturating_sub((-row_delta) as usize)
    };
    let new_col = if soft_wrap {
      0
    } else if col_delta >= 0 {
      current.col.saturating_add(col_delta as usize)
    } else {
      current.col.saturating_sub((-col_delta) as usize)
    };
    self.set_active_view_scroll_clamped(Position::new(new_row, new_col))
  }

  fn clamped_active_view_scroll(&self, scroll: Position) -> Position {
    let doc = self.editor.document();
    let view = self.editor.view();
    let mut clamped = scroll;

    let mut text_format = <Self as DefaultContext>::text_format(self);
    let gutter_width = gutter_width_for_document(doc, view.viewport.width, &self.gutter_config);
    text_format.viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1);
    if text_format.soft_wrap {
      clamped.col = 0;
    }

    let text = doc.text();
    let mut annotations = <Self as DefaultContext>::text_annotations(self);
    let text_slice = text.slice(..);
    let has_line_annotations = annotations.has_line_annotations();
    let eof_pos = if !text_format.soft_wrap && !has_line_annotations {
      Position::new(text.len_lines().saturating_sub(1), 0)
    } else {
      visual_pos_at_char(text_slice, &text_format, &mut annotations, text.len_chars())
        .unwrap_or_else(|| Position::new(0, 0))
    };
    let max_scroll_row = if text_format.soft_wrap {
      the_lib::view::max_scroll_row_for_content(eof_pos.row, view.viewport.height as usize)
    } else {
      eof_pos.row
    };
    clamped.row = clamped.row.min(max_scroll_row);

    if !text_format.soft_wrap && clamped.col != view.scroll.col {
      let max_col = self.max_visual_col_for_text(text, &text_format, &mut annotations);
      let viewport_cols = usize::from(text_format.viewport_width.max(1));
      let max_scroll_col = max_col.saturating_sub(viewport_cols.saturating_sub(1));
      clamped.col = clamped.col.min(max_scroll_col);
    }

    clamped
  }

  fn set_active_view_scroll_clamped(&mut self, scroll: Position) -> bool {
    let clamped = self.clamped_active_view_scroll(scroll);
    let view = self.editor.view_mut();
    if view.scroll == clamped {
      return false;
    }
    view.scroll = clamped;
    true
  }

  fn max_visual_col_for_text<'a>(
    &self,
    text: &'a Rope,
    text_format: &'a TextFormat,
    annotations: &mut TextAnnotations<'a>,
  ) -> usize {
    let text_slice = text.slice(..);
    let mut max_col = 0usize;
    for line_idx in 0..text.len_lines() {
      let line = text.line(line_idx);
      let mut line_end = line.len_chars();
      while line_end > 0 {
        let Some(ch) = line.get_char(line_end - 1) else {
          break;
        };
        if ch == '\n' || ch == '\r' {
          line_end -= 1;
        } else {
          break;
        }
      }
      let char_idx = text.line_to_char(line_idx).saturating_add(line_end);
      if let Some(pos) = visual_pos_at_char(text_slice, text_format, annotations, char_idx) {
        max_col = max_col.max(pos.col);
      }
    }
    max_col
  }

  fn pointer_drag_autoscroll_rows(&self, y: u16, pane: PaneSnapshot) -> i32 {
    let top = pane.rect.y;
    let bottom = pane
      .rect
      .y
      .saturating_add(pane.rect.height.saturating_sub(1));

    if y < top {
      return -i32::from((top - y).min(4));
    }
    if y > bottom {
      return i32::from((y - bottom).min(4));
    }
    if y == top {
      return -1;
    }
    if y == bottom {
      return 1;
    }
    0
  }

  fn handle_editor_pointer_event(&mut self, event: PointerEvent) -> PointerEventOutcome {
    let Some((x, y)) = self.pointer_event_screen_coords(event) else {
      return PointerEventOutcome::Continue;
    };

    let hit_pane = self.pointer_hit_pane_at(x, y);
    let previous_buffer_id = self.editor.active_buffer_id();
    let mut pane_changed = false;
    if let Some(pane) = hit_pane {
      pane_changed = self.editor.set_active_pane(pane.pane_id);
      if pane_changed {
        self.sync_state_after_active_pane_change(previous_buffer_id);
      }
    }

    match event.kind {
      PointerKind::Scroll => {
        let row_delta = if event.scroll_y > 0.0 {
          event.scroll_y.round() as i32
        } else if event.scroll_y < 0.0 {
          event.scroll_y.round() as i32
        } else {
          0
        };
        let col_delta = if event.scroll_x > 0.0 {
          event.scroll_x.round() as i32
        } else if event.scroll_x < 0.0 {
          event.scroll_x.round() as i32
        } else {
          0
        };
        let over_editor = hit_pane.is_some();
        if !over_editor {
          if pane_changed {
            self.request_render();
            return PointerEventOutcome::Handled;
          }
          return PointerEventOutcome::Continue;
        }
        let changed = self.pointer_scroll_active_view_by(row_delta, col_delta);
        if changed {
          self.mouse_viewport_detached = true;
        }
        if changed || pane_changed {
          self.request_render();
        }
        PointerEventOutcome::Handled
      },
      PointerKind::Down(PointerButton::Left) => {
        self.mouse_selection_drag_active = false;
        self.mouse_viewport_detached = false;
        self.pointer_drag_selection = None;
        let Some(pane) = hit_pane else {
          if pane_changed {
            self.request_render();
            return PointerEventOutcome::Handled;
          }
          return PointerEventOutcome::Continue;
        };
        let Some(target) = self.pointer_char_idx_for_pane_point(pane, x, y) else {
          if pane_changed {
            self.request_render();
          }
          return PointerEventOutcome::Handled;
        };
        let click_mode = Self::pointer_drag_mode_for_click_count(event.click_count.max(1));
        let drag_state =
          self.pointer_selection_drag_state_for_target(click_mode, target, event.modifiers.shift());
        self.pointer_drag_selection = Some(drag_state);
        let changed = self.pointer_apply_drag_selection(drag_state, target);
        self.mouse_selection_drag_active = true;
        self.clear_hover_state();
        if changed || pane_changed {
          self.request_render();
        }
        PointerEventOutcome::Handled
      },
      PointerKind::Drag(PointerButton::Left) => {
        let pane = hit_pane.or_else(|| self.pointer_active_pane_snapshot());
        let Some(pane) = pane else {
          if pane_changed {
            self.request_render();
            return PointerEventOutcome::Handled;
          }
          return PointerEventOutcome::Continue;
        };

        let scrolled =
          self.pointer_scroll_active_view_by(self.pointer_drag_autoscroll_rows(y, pane), 0);
        if scrolled {
          self.mouse_viewport_detached = true;
        }
        let Some(target) = self.pointer_char_idx_for_pane_point(pane, x, y) else {
          if scrolled || pane_changed {
            self.request_render();
          }
          return PointerEventOutcome::Handled;
        };
        let drag_state = self.pointer_drag_selection.unwrap_or_else(|| {
          self.pointer_selection_drag_state_for_target(
            PointerSelectionDragMode::Char,
            target,
            false,
          )
        });
        if self.pointer_drag_selection.is_none() {
          self.pointer_drag_selection = Some(drag_state);
        }
        let changed = self.pointer_apply_drag_selection(drag_state, target);
        if changed {
          self.clear_hover_state();
        }
        self.mouse_selection_drag_active = true;
        if changed || scrolled || pane_changed {
          self.request_render();
        }
        PointerEventOutcome::Handled
      },
      PointerKind::Up(PointerButton::Left) => {
        let was_drag_active = self.mouse_selection_drag_active;
        self.mouse_selection_drag_active = false;
        self.pointer_drag_selection = None;
        if pane_changed {
          self.request_render();
          return PointerEventOutcome::Handled;
        }
        if hit_pane.is_some() {
          return PointerEventOutcome::Handled;
        }
        if was_drag_active {
          self.request_render();
          return PointerEventOutcome::Handled;
        }
        PointerEventOutcome::Continue
      },
      PointerKind::Move => {
        if pane_changed {
          self.request_render();
          return PointerEventOutcome::Handled;
        }
        if hit_pane.is_some() {
          return PointerEventOutcome::Handled;
        }
        PointerEventOutcome::Continue
      },
      _ => PointerEventOutcome::Continue,
    }
  }

  fn active_cursor_char_idx(&self) -> Option<usize> {
    let doc = self.editor.document();
    let range = self.active_or_first_selection_range()?;
    Some(range.cursor(doc.text().slice(..)))
  }

  fn cursor_prev_char_is_word(&self) -> bool {
    let Some(cursor) = self.active_cursor_char_idx() else {
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

  fn lsp_provider_supports_single_char(
    &self,
    provider_key: &str,
    characters_key: &str,
    ch: char,
  ) -> bool {
    let Some(runtime_id) = self.active_lsp_runtime_id else {
      return false;
    };
    let Some(capabilities) = self.managed_runtime_capabilities(runtime_id) else {
      return false;
    };
    capabilities_support_single_char(capabilities.raw(), provider_key, characters_key, ch)
  }

  fn lsp_completion_supports_trigger_char(&self, ch: char) -> bool {
    self.lsp_provider_supports_single_char("completionProvider", "triggerCharacters", ch)
  }

  fn lsp_completion_server_supports_resolve(&self) -> bool {
    self
      .active_lsp_runtime_id
      .and_then(|runtime_id| self.managed_runtime_capabilities(runtime_id))
      .is_some_and(|capabilities| capabilities.supports_completion_item_resolve())
  }

  fn lsp_signature_help_supports_trigger_char(&self, ch: char) -> bool {
    self.lsp_provider_supports_single_char("signatureHelpProvider", "triggerCharacters", ch)
  }

  fn lsp_signature_help_supports_retrigger_char(&self, ch: char) -> bool {
    self.lsp_provider_supports_single_char("signatureHelpProvider", "retriggerCharacters", ch)
  }

  fn lsp_completion_inline_menu_item(&self) -> Option<the_default::CompletionMenuItem> {
    if self.mode != Mode::Insert || self.code_action_menu_is_active() {
      return None;
    }
    the_default::completion_menu_inline_item(self)
  }

  fn completion_visible_index_is_inline_item(&self, index: usize) -> bool {
    self.lsp_completion_inline_item_active && index == 0
  }

  fn completion_source_index_for_visible_index(&self, index: usize) -> Option<usize> {
    let visible_index = if self.lsp_completion_inline_item_active {
      index.checked_sub(1)?
    } else {
      index
    };
    self
      .lsp_completion_visible_indices
      .get(visible_index)
      .copied()
  }

  fn completion_visible_index_for_source_index(&self, index: usize) -> Option<usize> {
    self
      .lsp_completion_visible_indices
      .iter()
      .position(|candidate| *candidate == index)
      .map(|visible_index| {
        if self.lsp_completion_inline_item_active {
          visible_index + 1
        } else {
          visible_index
        }
      })
  }

  fn sync_completion_menu_inline_item(&mut self) {
    if !self.completion_menu.active || self.code_action_menu_is_active() {
      self.lsp_completion_inline_item_active = false;
      return;
    }

    let next_inline_item = self.lsp_completion_inline_menu_item();
    let current_inline_item = if self.lsp_completion_inline_item_active {
      self.completion_menu.items.first()
    } else {
      None
    };
    let needs_rebuild = self.lsp_completion_inline_item_active != next_inline_item.is_some()
      || match (current_inline_item, next_inline_item.as_ref()) {
        (Some(current), Some(next)) => current != next,
        _ => false,
      };
    if needs_rebuild {
      self.rebuild_completion_menu();
    }
  }

  fn completion_filter_fragment(&self) -> Option<String> {
    let cursor = self.active_cursor_char_idx()?;
    let start = self
      .lsp_completion_fallback_start
      .unwrap_or(cursor)
      .min(cursor);
    let doc = self.editor.document();
    let text = doc.text();
    let fragment = text.slice(start..cursor).to_string();
    Some(fragment)
  }

  fn rebuild_completion_menu(&mut self) {
    self.clear_code_action_menu_state();
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
    the_default::show_builtin_completion_menu(
      self,
      the_default::BuiltinCompletionMenuKind::LspCompletion,
    );
  }

  fn clear_completion_state(&mut self) {
    self.lsp_completion_items.clear();
    self.lsp_completion_raw_items.clear();
    self.lsp_completion_resolved_indices.clear();
    self.lsp_completion_visible_indices.clear();
    self.lsp_completion_inline_item_active = false;
    self.lsp_completion_fallback_start = None;
    self.clear_code_action_menu_state();
    self.completion_menu.clear();
  }

  fn code_action_menu_is_active(&self) -> bool {
    self.lsp_code_action_menu_active && self.completion_menu.active
  }

  fn clear_code_action_menu_state(&mut self) {
    self.lsp_code_action_menu_active = false;
    self.lsp_code_action_items.clear();
  }

  fn show_code_action_menu(&mut self, mut actions: Vec<LspCodeAction>) {
    actions.sort_by_key(|action| !action.is_preferred);
    self.clear_completion_state();
    self.lsp_code_action_items = actions;
    self.lsp_code_action_menu_active = !self.lsp_code_action_items.is_empty();
    the_default::show_builtin_completion_menu(
      self,
      the_default::BuiltinCompletionMenuKind::CodeActions,
    );
  }

  fn apply_code_action(&mut self, action: LspCodeAction) -> bool {
    if self.resolve_code_action(action.clone()) {
      return true;
    }
    self.apply_code_action_now(action)
  }

  fn apply_code_action_now(&mut self, action: LspCodeAction) -> bool {
    let title = action.title.clone();
    let mut handled = false;

    if let Some(edit) = action.edit.as_ref() {
      let _ = self.apply_workspace_edit(edit, "code action");
      handled = true;
    }

    if let Some(command) = action.command {
      let _ = self.execute_lsp_command_action(command, title.clone());
      handled = true;
    }

    if !handled {
      self.messages.publish(
        MessageLevel::Info,
        Some("lsp".into()),
        format!("code action '{}' had no edits", title),
      );
    }
    true
  }

  fn clear_hover_state(&mut self) {
    self.hover_docs = None;
    self.hover_docs_scroll = 0;
  }

  fn clear_signature_help_state(&mut self) {
    self.signature_help.clear();
  }

  fn dispatch_signature_help_request(
    &mut self,
    trigger: SignatureHelpTriggerSource,
    announce_failures: bool,
  ) -> bool {
    if !self.lsp_supports(LspCapability::SignatureHelp) {
      if announce_failures {
        self.messages.publish(
          MessageLevel::Warning,
          Some("lsp".into()),
          "signature help is not supported by the active server",
        );
      }
      return false;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      if announce_failures {
        self.messages.publish(
          MessageLevel::Warning,
          Some("lsp".into()),
          "signature help unavailable: no active LSP document",
        );
      }
      return false;
    };

    let context = trigger.to_lsp_context();
    self.dispatch_lsp_request(
      "textDocument/signatureHelp",
      signature_help_params(&uri, position, &context),
      PendingLspRequestKind::SignatureHelp { uri },
    );
    true
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
    let Some(cursor) = self.active_cursor_char_idx() else {
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

  fn schedule_auto_signature_help(
    &mut self,
    trigger: SignatureHelpTriggerSource,
    delay: Duration,
  ) -> bool {
    if self.mode != Mode::Insert || !self.lsp_supports(LspCapability::SignatureHelp) {
      self.lsp_pending_auto_signature_help = None;
      return false;
    }

    self.lsp_pending_auto_signature_help = Some(PendingAutoSignatureHelp {
      due_at: Instant::now() + delay,
      trigger,
    });
    true
  }

  fn cancel_auto_signature_help(&mut self) {
    self.lsp_pending_auto_signature_help = None;
  }

  fn handle_signature_help_action(&mut self, command: Command) -> bool {
    if self.mode != Mode::Insert {
      self.cancel_auto_signature_help();
      self.clear_signature_help_state();
      return false;
    }

    match command {
      Command::InsertChar(ch) => {
        if self.lsp_signature_help_supports_trigger_char(ch) {
          return self.schedule_auto_signature_help(
            SignatureHelpTriggerSource::TriggerCharacter {
              ch,
              is_retrigger: self.signature_help.active,
            },
            lsp_signature_help_trigger_char_latency(),
          );
        }
        if self.signature_help.active {
          let trigger = if self.lsp_signature_help_supports_retrigger_char(ch) {
            SignatureHelpTriggerSource::TriggerCharacter {
              ch,
              is_retrigger: true,
            }
          } else {
            SignatureHelpTriggerSource::ContentChangeRetrigger
          };
          return self
            .schedule_auto_signature_help(trigger, lsp_signature_help_retrigger_latency());
        }
        self.cancel_auto_signature_help();
        false
      },
      Command::DeleteChar
      | Command::DeleteCharForward { .. }
      | Command::DeleteWordBackward { .. }
      | Command::DeleteWordForward { .. }
      | Command::KillToLineStart
      | Command::KillToLineEnd => {
        if self.signature_help.active {
          return self.schedule_auto_signature_help(
            SignatureHelpTriggerSource::ContentChangeRetrigger,
            lsp_signature_help_retrigger_latency(),
          );
        }
        self.cancel_auto_signature_help();
        false
      },
      Command::CompletionAccept => {
        let trigger = if self.signature_help.active {
          SignatureHelpTriggerSource::ContentChangeRetrigger
        } else {
          SignatureHelpTriggerSource::Manual
        };
        self.schedule_auto_signature_help(trigger, lsp_signature_help_retrigger_latency())
      },
      Command::Motion(_)
      | Command::Move(_)
      | Command::GotoLineStart { .. }
      | Command::GotoLineEnd { .. }
      | Command::PageUp { .. }
      | Command::PageDown { .. }
      | Command::PageCursorHalfUp
      | Command::PageCursorHalfDown
      | Command::FindChar { .. }
      | Command::ParentNodeStart { .. }
      | Command::ParentNodeEnd { .. } => {
        let trigger = if self.signature_help.active {
          SignatureHelpTriggerSource::ContentChangeRetrigger
        } else {
          SignatureHelpTriggerSource::Manual
        };
        self.schedule_auto_signature_help(trigger, lsp_signature_help_retrigger_latency())
      },
      Command::LspSignatureHelp
      | Command::CompletionNext
      | Command::CompletionPrev
      | Command::CompletionDocsScrollUp
      | Command::CompletionDocsScrollDown => true,
      _ => {
        self.cancel_auto_signature_help();
        self.clear_signature_help_state();
        false
      },
    }
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
      | Command::CompletionCancel
      | Command::CompletionDocsScrollUp
      | Command::CompletionDocsScrollDown => true,
      _ => {
        self.cancel_auto_completion();
        false
      },
    }
  }

  fn current_lsp_position(&self) -> Option<(String, LspPosition)> {
    let state = self.lsp_document.as_ref()?.clone();
    if !self
      .active_buffer_lsp_state()
      .is_some_and(|buffer_state| !buffer_state.opened_runtime_ids.is_empty())
    {
      return None;
    }

    let doc = self.editor.document();
    let range = self.active_or_first_selection_range()?;
    let cursor = range.cursor(doc.text().slice(..));
    let (line, character) = char_idx_to_utf16_position(doc.text(), cursor);

    Some((state.uri, LspPosition { line, character }))
  }

  fn current_lsp_range(&self) -> Option<(String, the_lsp::LspRange)> {
    let state = self.lsp_document.as_ref()?.clone();
    if !self
      .active_buffer_lsp_state()
      .is_some_and(|buffer_state| !buffer_state.opened_runtime_ids.is_empty())
    {
      return None;
    }

    let doc = self.editor.document();
    let range = self.active_or_first_selection_range()?;
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

  fn current_lsp_code_action_range(&self) -> Option<(String, the_lsp::LspRange)> {
    let state = self.lsp_document.as_ref()?.clone();
    if !self
      .active_buffer_lsp_state()
      .is_some_and(|buffer_state| !buffer_state.opened_runtime_ids.is_empty())
    {
      return None;
    }

    let doc = self.editor.document();
    let range = self.active_or_first_selection_range()?;
    let mut start = range.anchor.min(range.head);
    let mut end = range.anchor.max(range.head);

    // Helix normal mode effectively requests code actions on a non-empty
    // selection under the cursor. Our normal mode cursor is a point selection,
    // so expand to one char to make clangd refactors (e.g. extract) appear.
    if start == end {
      let len = doc.text().len_chars();
      if end < len {
        end = end.saturating_add(1);
      } else if start > 0 {
        start = start.saturating_sub(1);
      }
    }

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
    self
      .lsp_document
      .as_ref()
      .filter(|_| {
        self
          .active_buffer_lsp_state()
          .is_some_and(|buffer_state| !buffer_state.opened_runtime_ids.is_empty())
      })
      .map(|state| state.uri.clone())
  }

  fn current_lsp_diagnostics_payload(
    &self,
    uri: &str,
    selection_range: &the_lsp::LspRange,
  ) -> Value {
    let Some(document_diagnostics) = self.diagnostics.document(uri) else {
      return json!([]);
    };

    Value::Array(
      document_diagnostics
        .diagnostics
        .iter()
        .filter(|diagnostic| {
          let diagnostic_range = the_lsp::LspRange {
            start: LspPosition {
              line:      diagnostic.range.start.line,
              character: diagnostic.range.start.character,
            },
            end:   LspPosition {
              line:      diagnostic.range.end.line,
              character: diagnostic.range.end.character,
            },
          };
          lsp_ranges_overlap(&diagnostic_range, selection_range)
        })
        .map(diagnostic_to_lsp_json)
        .collect(),
    )
  }

  fn cancel_pending_lsp_requests_for(&mut self, next: &PendingLspRequestKind) {
    let Some(runtime_id) = self.active_lsp_runtime_id else {
      return;
    };
    let target = next.cancellation_key();
    let ids_to_cancel = self
      .lsp_runtimes
      .get(&runtime_id)
      .map(|runtime| {
        runtime
          .pending_requests
          .iter()
          .filter_map(|(id, pending)| (pending.cancellation_key() == target).then_some(*id))
          .collect::<Vec<_>>()
      })
      .unwrap_or_default();

    for id in ids_to_cancel {
      if let Some(runtime) = self.lsp_runtimes.get_mut(&runtime_id) {
        let _ = runtime.pending_requests.remove(&id);
        if let Err(err) = runtime.runtime.cancel_request(id) {
          self.messages.publish(
            MessageLevel::Warning,
            Some("lsp".into()),
            format!("failed to cancel stale request {id}: {err}"),
          );
        }
      }
    }
    self.sync_active_lsp_mirror_state();
  }

  fn dispatch_lsp_request(
    &mut self,
    method: &'static str,
    params: Value,
    pending: PendingLspRequestKind,
  ) {
    let Some(runtime_id) = pending
      .uri()
      .and_then(|_| self.active_lsp_runtime_for_pending(&pending))
      .or_else(|| self.active_lsp_runtime_id)
    else {
      self.messages.publish(
        MessageLevel::Error,
        Some("lsp".into()),
        format!("failed to dispatch {method}: no active language server"),
      );
      return;
    };
    self.cancel_pending_lsp_requests_for(&pending);
    let runtime = self
      .lsp_runtimes
      .get(&runtime_id)
      .expect("selected runtime must exist");
    match runtime.runtime.send_request(method, Some(params)) {
      Ok(request_id) => {
        if let Some(runtime) = self.lsp_runtimes.get_mut(&runtime_id) {
          runtime.pending_requests.insert(request_id, pending);
        }
        self.sync_active_lsp_mirror_state();
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
    if !self.lsp_completion_resolve_supported {
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
    let Some(runtime_id) = self.active_lsp_runtime_for_capability(LspCapability::Completion) else {
      return;
    };
    let params = self.lsp_completion_raw_items[index].clone();
    match self
      .lsp_runtimes
      .get(&runtime_id)
      .expect("active completion runtime")
      .runtime
      .send_request("completionItem/resolve", Some(params))
    {
      Ok(request_id) => {
        if let Some(runtime) = self.lsp_runtimes.get_mut(&runtime_id) {
          runtime
            .pending_requests
            .insert(request_id, PendingLspRequestKind::CompletionResolve {
              uri,
              index,
            });
        }
        self.sync_active_lsp_mirror_state();
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
    let Some(range) = self.active_or_first_selection_range() else {
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

  fn lsp_runtime_config_for(server: LspServerConfig, workspace_root: PathBuf) -> LspRuntimeConfig {
    LspRuntimeConfig::new(workspace_root)
      .with_server(server)
      .with_restart_policy(true, Duration::from_millis(250))
      .with_restart_limits(6, Duration::from_secs(30))
      .with_request_policy(Duration::from_secs(8), 1)
  }

  fn next_lsp_runtime_id(&mut self) -> LspRuntimeId {
    let next = self.next_lsp_runtime_id;
    self.next_lsp_runtime_id = self.next_lsp_runtime_id.saturating_add(1);
    next
  }

  fn lsp_runtime_provider_key(runtime_id: LspRuntimeId) -> String {
    format!("runtime:{runtime_id}")
  }

  fn lsp_workspace_folder_name(root: &Path) -> String {
    root
      .file_name()
      .map(|name| name.to_string_lossy().to_string())
      .unwrap_or_else(|| root.display().to_string())
  }

  fn active_buffer_lsp_state(&self) -> Option<&BufferLspState> {
    self.buffer_lsp_states.get(&self.editor.active_buffer_id())
  }

  fn active_buffer_lsp_state_mut(&mut self) -> Option<&mut BufferLspState> {
    self
      .buffer_lsp_states
      .get_mut(&self.editor.active_buffer_id())
  }

  fn active_managed_lsp_runtime(&self) -> Option<&ManagedLspRuntime> {
    self
      .active_lsp_runtime_id
      .and_then(|runtime_id| self.lsp_runtimes.get(&runtime_id))
  }

  fn active_managed_lsp_runtime_mut(&mut self) -> Option<&mut ManagedLspRuntime> {
    self
      .active_lsp_runtime_id
      .and_then(|runtime_id| self.lsp_runtimes.get_mut(&runtime_id))
  }

  fn managed_runtime_supports_capability(
    &self,
    runtime_id: LspRuntimeId,
    capability: LspCapability,
  ) -> bool {
    let Some(runtime) = self.lsp_runtimes.get(&runtime_id) else {
      return false;
    };
    let Some(server_name) = runtime.configured_server_name() else {
      return false;
    };
    runtime
      .runtime
      .server_capabilities(server_name)
      .is_some_and(|capabilities| capabilities.supports(capability))
  }

  fn managed_runtime_capabilities(
    &self,
    runtime_id: LspRuntimeId,
  ) -> Option<ServerCapabilitiesSnapshot> {
    let runtime = self.lsp_runtimes.get(&runtime_id)?;
    let server_name = runtime.configured_server_name()?;
    runtime.runtime.server_capabilities(server_name)
  }

  fn active_lsp_runtime_for_capability(&self, capability: LspCapability) -> Option<LspRuntimeId> {
    let state = self.active_buffer_lsp_state()?;
    state
      .attached_runtime_ids
      .iter()
      .copied()
      .find(|runtime_id| {
        self
          .lsp_runtimes
          .get(runtime_id)
          .is_some_and(|runtime| runtime.ready)
          && self.managed_runtime_supports_capability(*runtime_id, capability)
      })
  }

  fn active_lsp_runtime_for_pending(
    &self,
    pending: &PendingLspRequestKind,
  ) -> Option<LspRuntimeId> {
    let capability = match pending {
      PendingLspRequestKind::GotoDeclaration { .. } => LspCapability::GotoDeclaration,
      PendingLspRequestKind::GotoDefinition { .. } => LspCapability::GotoDefinition,
      PendingLspRequestKind::GotoTypeDefinition { .. } => LspCapability::GotoTypeDefinition,
      PendingLspRequestKind::GotoImplementation { .. } => LspCapability::GotoImplementation,
      PendingLspRequestKind::Hover { .. } => LspCapability::Hover,
      PendingLspRequestKind::DocumentHighlightSelect { .. } => LspCapability::DocumentHighlight,
      PendingLspRequestKind::References { .. } => LspCapability::GotoReference,
      PendingLspRequestKind::DocumentSymbols { .. } => LspCapability::DocumentSymbols,
      PendingLspRequestKind::WorkspaceSymbols { .. } => LspCapability::WorkspaceSymbols,
      PendingLspRequestKind::Completion { .. }
      | PendingLspRequestKind::CompletionResolve { .. } => LspCapability::Completion,
      PendingLspRequestKind::SignatureHelp { .. } => LspCapability::SignatureHelp,
      PendingLspRequestKind::CodeActions { .. }
      | PendingLspRequestKind::CodeActionResolve { .. } => LspCapability::CodeAction,
      PendingLspRequestKind::Rename { .. } => LspCapability::RenameSymbol,
      PendingLspRequestKind::Format { .. } => LspCapability::Format,
    };
    self.active_lsp_runtime_for_capability(capability)
  }

  fn sync_active_lsp_mirror_state(&mut self) {
    let active_buffer_id = self.editor.active_buffer_id();
    let Some(buffer_state) = self.buffer_lsp_states.get(&active_buffer_id) else {
      self.active_lsp_runtime_id = None;
      self.lsp_document = None;
      self.lsp_ready = false;
      self.lsp_active_progress_tokens.clear();
      self.lsp_pending_requests.clear();
      self.lsp_statusline = LspStatuslineState::off(Some("unavailable".into()));
      return;
    };

    let primary_runtime_id = buffer_state.attached_runtime_ids.first().copied();
    self.active_lsp_runtime_id = primary_runtime_id;
    self.lsp_document = buffer_state.document.clone().map(|mut state| {
      state.opened = primary_runtime_id
        .is_some_and(|runtime_id| buffer_state.opened_runtime_ids.contains(&runtime_id));
      state
    });

    if let Some(runtime) =
      primary_runtime_id.and_then(|runtime_id| self.lsp_runtimes.get(&runtime_id))
    {
      self.lsp_ready = runtime.ready;
      self.lsp_statusline = runtime.statusline.clone();
      self.lsp_active_progress_tokens = runtime.active_progress_tokens.clone();
      self.lsp_pending_requests = runtime.pending_requests.clone();
      self.lsp_completion_resolve_supported = runtime
        .configured_server_name()
        .and_then(|server_name| runtime.runtime.server_capabilities(server_name))
        .is_some_and(|capabilities| capabilities.supports_completion_item_resolve());
    } else {
      self.lsp_ready = false;
      self.lsp_active_progress_tokens.clear();
      self.lsp_pending_requests.clear();
      self.lsp_statusline = LspStatuslineState::off(Some("unavailable".into()));
      self.lsp_completion_resolve_supported = false;
    }
  }

  fn set_lsp_status_for_runtime(
    &mut self,
    runtime_id: LspRuntimeId,
    phase: LspStatusPhase,
    detail: Option<String>,
  ) {
    if let Some(runtime) = self.lsp_runtimes.get_mut(&runtime_id) {
      runtime.statusline = LspStatuslineState {
        phase,
        detail: detail.clone(),
      };
    }
    if self.active_lsp_runtime_id == Some(runtime_id) {
      self.lsp_statusline = LspStatuslineState { phase, detail };
      if !self.lsp_statusline.is_loading() {
        self.lsp_spinner_index = 0;
      }
    }
  }

  fn set_lsp_status_error_for_runtime(&mut self, runtime_id: LspRuntimeId, summary: &str) {
    if let Some(runtime) = self.lsp_runtimes.get_mut(&runtime_id) {
      runtime.ready = false;
      runtime.active_progress_tokens.clear();
    }
    self.set_lsp_status_for_runtime(runtime_id, LspStatusPhase::Error, Some(summary.to_string()));
  }

  fn create_lsp_runtime(
    &mut self,
    server: LspServerConfig,
    workspace_root: PathBuf,
  ) -> LspRuntimeId {
    let runtime_id = self.next_lsp_runtime_id();
    let runtime_config = Self::lsp_runtime_config_for(server, workspace_root.clone());
    let workspace_name = Self::lsp_workspace_folder_name(&workspace_root);
    let workspace_uri = file_uri_for_path(&workspace_root);
    let mut workspace_folders = BTreeMap::new();
    if let Some(uri) = workspace_uri {
      workspace_folders.insert(uri, workspace_name.clone());
    }
    self.lsp_runtimes.insert(runtime_id, ManagedLspRuntime {
      id: runtime_id,
      runtime: LspRuntime::new(runtime_config),
      ready: false,
      statusline: LspStatuslineState {
        phase:  LspStatusPhase::Starting,
        detail: Some("booting".into()),
      },
      active_progress_tokens: HashSet::new(),
      pending_requests: HashMap::new(),
      workspace_folders,
    });
    runtime_id
  }

  fn ensure_lsp_runtime_started(&mut self, runtime_id: LspRuntimeId) {
    if !self.lsp_services_started {
      return;
    }
    let Some(runtime_snapshot) = self.lsp_runtimes.get(&runtime_id) else {
      return;
    };
    if runtime_snapshot.runtime.is_running() {
      return;
    }
    let server_snapshot = runtime_snapshot.runtime.config().server().map(|server| {
      (
        server.name().to_string(),
        server.command().to_string(),
        server.args().to_vec(),
      )
    });
    let workspace_root = runtime_snapshot
      .runtime
      .config()
      .workspace_root()
      .to_path_buf();
    if let Some((server_name, server_command, server_args)) = server_snapshot {
      self.log_lsp_trace_value(json!({
        "ts_ms": SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|duration| duration.as_millis() as u64).unwrap_or(0),
        "kind": "bootstrap",
        "runtime_id": runtime_id,
        "server": {
          "name": server_name,
          "command": server_command,
          "args": server_args,
        },
        "workspace_root": workspace_root,
      }));
      self.set_lsp_status_for_runtime(
        runtime_id,
        LspStatusPhase::Starting,
        Some("starting".into()),
      );
    }
    let Some(runtime) = self.lsp_runtimes.get_mut(&runtime_id) else {
      return;
    };
    if let Err(err) = runtime.runtime.start() {
      self.set_lsp_status_error_for_runtime(runtime_id, &err.to_string());
      eprintln!("Warning: failed to start LSP runtime: {err}");
    }
  }

  fn lsp_runtime_supports_workspace_folders(&self, runtime_id: LspRuntimeId) -> bool {
    self
      .managed_runtime_capabilities(runtime_id)
      .is_some_and(|capabilities| capabilities.supports_workspace_folders())
  }

  fn maybe_add_workspace_folder_to_runtime(
    &mut self,
    runtime_id: LspRuntimeId,
    root: &Path,
  ) -> bool {
    let Some(root_uri) = file_uri_for_path(root) else {
      return false;
    };
    let root_name = Self::lsp_workspace_folder_name(root);
    let Some(runtime) = self.lsp_runtimes.get(&runtime_id) else {
      return false;
    };
    if runtime.workspace_folders.contains_key(&root_uri) {
      return true;
    }
    if !self.lsp_runtime_supports_workspace_folders(runtime_id) {
      return false;
    }

    let send_now = self.lsp_services_started
      && self
        .lsp_runtimes
        .get(&runtime_id)
        .is_some_and(|entry| entry.runtime.is_running());
    if let Some(runtime) = self.lsp_runtimes.get_mut(&runtime_id) {
      runtime
        .workspace_folders
        .insert(root_uri.clone(), root_name.clone());
      if send_now {
        let _ = runtime
          .runtime
          .add_workspace_folder(root_uri.clone(), root_name.clone());
      }
    }
    true
  }

  fn resolve_or_create_lsp_runtime(
    &mut self,
    server: LspServerConfig,
    workspace_root: PathBuf,
  ) -> LspRuntimeId {
    let existing_ids = self.lsp_runtimes.keys().copied().collect::<Vec<_>>();
    for runtime_id in existing_ids {
      let Some(runtime) = self.lsp_runtimes.get(&runtime_id) else {
        continue;
      };
      if !lsp_server_configs_equal(runtime.runtime.config().server(), Some(&server)) {
        continue;
      }
      if runtime.runtime.config().workspace_root() == workspace_root.as_path() {
        return runtime_id;
      }
      if self.maybe_add_workspace_folder_to_runtime(runtime_id, &workspace_root) {
        return runtime_id;
      }
    }

    self.create_lsp_runtime(server, workspace_root)
  }

  fn open_buffer_document_for_runtime(&mut self, buffer_id: BufferId, runtime_id: LspRuntimeId) {
    let Some(runtime) = self.lsp_runtimes.get(&runtime_id) else {
      return;
    };
    if !runtime.ready {
      return;
    }
    let Some(buffer_state) = self.buffer_lsp_states.get(&buffer_id) else {
      return;
    };
    if buffer_state.opened_runtime_ids.contains(&runtime_id) {
      return;
    }
    let Some(state) = buffer_state.document.as_ref() else {
      return;
    };
    let Some(document) = self.editor.buffer_document(buffer_id) else {
      return;
    };
    let params = did_open_params(
      &state.uri,
      &state.language_id,
      state.version,
      document.text(),
    );
    if self.lsp_runtimes.get(&runtime_id).is_some_and(|managed| {
      managed
        .runtime
        .send_notification("textDocument/didOpen", Some(params))
        .is_ok()
    }) && let Some(buffer_state) = self.buffer_lsp_states.get_mut(&buffer_id)
    {
      buffer_state.opened_runtime_ids.insert(runtime_id);
    }
  }

  fn open_attached_documents_for_runtime(&mut self, runtime_id: LspRuntimeId) {
    let buffer_ids = self
      .buffer_lsp_states
      .iter()
      .filter_map(|(buffer_id, state)| {
        state
          .attached_runtime_ids
          .contains(&runtime_id)
          .then_some(*buffer_id)
      })
      .collect::<Vec<_>>();
    for buffer_id in buffer_ids {
      self.open_buffer_document_for_runtime(buffer_id, runtime_id);
    }
  }

  fn close_buffer_document_for_runtime(&mut self, buffer_id: BufferId, runtime_id: LspRuntimeId) {
    let Some(document) = self
      .buffer_lsp_states
      .get(&buffer_id)
      .and_then(|buffer_state| {
        buffer_state
          .opened_runtime_ids
          .contains(&runtime_id)
          .then(|| buffer_state.document.clone())
          .flatten()
      })
    else {
      return;
    };
    let params = did_close_params(&document.uri);
    if let Some(runtime) = self.lsp_runtimes.get(&runtime_id) {
      let _ = runtime
        .runtime
        .send_notification("textDocument/didClose", Some(params));
    }
    if let Some(buffer_state) = self.buffer_lsp_states.get_mut(&buffer_id) {
      buffer_state.opened_runtime_ids.remove(&runtime_id);
    }
    self
      .diagnostics
      .remove_document_for_provider(&Self::lsp_runtime_provider_key(runtime_id), &document.uri);
  }

  fn maybe_remove_workspace_folder_from_runtime(&mut self, runtime_id: LspRuntimeId, root: &Path) {
    let Some(root_uri) = file_uri_for_path(root) else {
      return;
    };
    let Some(runtime) = self.lsp_runtimes.get(&runtime_id) else {
      return;
    };
    let primary_uri = file_uri_for_path(runtime.runtime.config().workspace_root());
    if primary_uri.as_deref() == Some(root_uri.as_str()) {
      return;
    }

    let still_used = self.buffer_lsp_states.values().any(|state| {
      state.attached_runtime_ids.contains(&runtime_id)
        && state
          .document
          .as_ref()
          .is_some_and(|document| workspace_root_for_path(&document.path) == root)
    });
    if still_used {
      return;
    }

    if let Some(runtime) = self.lsp_runtimes.get_mut(&runtime_id)
      && let Some(name) = runtime.workspace_folders.remove(&root_uri)
      && self.lsp_services_started
      && runtime.runtime.is_running()
    {
      let _ = runtime.runtime.remove_workspace_folder(root_uri, name);
    }
  }

  fn refresh_buffer_lsp_state(&mut self, buffer_id: BufferId, path: Option<&Path>) {
    let old_state = self.buffer_lsp_states.remove(&buffer_id);
    let old_document = old_state.as_ref().and_then(|state| state.document.clone());
    let old_runtime_ids = old_state
      .as_ref()
      .map(|state| state.attached_runtime_ids.clone())
      .unwrap_or_default();

    let mut new_document =
      path.and_then(|path| build_lsp_document_state(path, self.loader.as_deref()));
    if let Some(document) = new_document.as_mut()
      && let Some(buffer_document) = self.editor.buffer_document(buffer_id)
    {
      document.version = buffer_document.version() as i32;
      document.opened = false;
    }

    let path_changed = old_document
      .as_ref()
      .zip(new_document.as_ref())
      .is_none_or(|(lhs, rhs)| lhs.uri != rhs.uri || lhs.language_id != rhs.language_id);

    let mut new_runtime_ids = Vec::new();
    if let Some(document) = new_document.as_ref() {
      let servers = resolve_lsp_servers(self.loader.as_deref(), Some(document.path.as_path()));
      for server in servers {
        let workspace_root = workspace_root_for_path(&document.path);
        let runtime_id = self.resolve_or_create_lsp_runtime(server, workspace_root);
        if !new_runtime_ids.contains(&runtime_id) {
          new_runtime_ids.push(runtime_id);
        }
      }
    }

    let removed_runtime_ids = if path_changed {
      old_runtime_ids.clone()
    } else {
      old_runtime_ids
        .iter()
        .copied()
        .filter(|runtime_id| !new_runtime_ids.contains(runtime_id))
        .collect::<Vec<_>>()
    };

    if let Some(old_document) = old_document.as_ref() {
      for runtime_id in &removed_runtime_ids {
        if let Some(state) = old_state.as_ref()
          && state.opened_runtime_ids.contains(runtime_id)
        {
          let params = did_close_params(&old_document.uri);
          if let Some(runtime) = self.lsp_runtimes.get(runtime_id) {
            let _ = runtime
              .runtime
              .send_notification("textDocument/didClose", Some(params.clone()));
          }
        }
        self.diagnostics.remove_document_for_provider(
          &Self::lsp_runtime_provider_key(*runtime_id),
          &old_document.uri,
        );
        let root = workspace_root_for_path(&old_document.path);
        self.maybe_remove_workspace_folder_from_runtime(*runtime_id, &root);
      }
    }

    let mut opened_runtime_ids = if path_changed {
      HashSet::new()
    } else {
      old_state
        .map(|state| {
          state
            .opened_runtime_ids
            .into_iter()
            .filter(|runtime_id| new_runtime_ids.contains(runtime_id))
            .collect()
        })
        .unwrap_or_default()
    };
    opened_runtime_ids.retain(|runtime_id| new_runtime_ids.contains(runtime_id));

    self.buffer_lsp_states.insert(buffer_id, BufferLspState {
      document: new_document.clone(),
      attached_runtime_ids: new_runtime_ids.clone(),
      opened_runtime_ids,
    });

    for runtime_id in &new_runtime_ids {
      self.ensure_lsp_runtime_started(*runtime_id);
      self.open_buffer_document_for_runtime(buffer_id, *runtime_id);
    }

    self.sync_active_lsp_mirror_state();
  }

  fn detach_buffer_lsp_state(&mut self, buffer_id: BufferId) {
    let Some(state) = self.buffer_lsp_states.remove(&buffer_id) else {
      return;
    };
    let Some(document) = state.document else {
      self.sync_active_lsp_mirror_state();
      return;
    };
    let root = workspace_root_for_path(&document.path);
    for runtime_id in state.attached_runtime_ids {
      if state.opened_runtime_ids.contains(&runtime_id) {
        let params = did_close_params(&document.uri);
        if let Some(runtime) = self.lsp_runtimes.get(&runtime_id) {
          let _ = runtime
            .runtime
            .send_notification("textDocument/didClose", Some(params));
        }
      }
      self
        .diagnostics
        .remove_document_for_provider(&Self::lsp_runtime_provider_key(runtime_id), &document.uri);
      self.maybe_remove_workspace_folder_from_runtime(runtime_id, &root);
    }
    self.sync_active_lsp_mirror_state();
  }

  fn lsp_sync_kind_for_runtime(&self, runtime_id: LspRuntimeId) -> Option<TextDocumentSyncKind> {
    self
      .managed_runtime_capabilities(runtime_id)
      .map(|capabilities| capabilities.text_document_sync().kind)
  }

  fn lsp_save_include_text_for_runtime(&self, runtime_id: LspRuntimeId) -> bool {
    self
      .managed_runtime_capabilities(runtime_id)
      .is_some_and(|capabilities| capabilities.text_document_sync().save_include_text)
  }

  fn lsp_open_current_document(&mut self) {
    let Some(state) = self.active_buffer_lsp_state() else {
      return;
    };
    let runtime_ids = state.attached_runtime_ids.clone();
    let buffer_id = self.editor.active_buffer_id();
    for runtime_id in runtime_ids {
      self.open_buffer_document_for_runtime(buffer_id, runtime_id);
    }
    self.sync_active_lsp_mirror_state();
  }

  fn lsp_close_current_document(&mut self) {
    let Some(state) = self.active_buffer_lsp_state() else {
      return;
    };
    let runtime_ids = state.attached_runtime_ids.clone();
    let buffer_id = self.editor.active_buffer_id();
    for runtime_id in runtime_ids {
      self.close_buffer_document_for_runtime(buffer_id, runtime_id);
    }
    self.sync_active_lsp_mirror_state();
  }

  fn lsp_send_did_change(&mut self, old_text: &Rope, changes: &ChangeSet) {
    let buffer_id = self.editor.active_buffer_id();
    self.lsp_send_did_change_for_buffer(buffer_id, old_text, changes);
  }

  fn lsp_send_did_change_for_buffer(
    &mut self,
    buffer_id: BufferId,
    old_text: &Rope,
    changes: &ChangeSet,
  ) {
    let Some(state) = self.buffer_lsp_states.get(&buffer_id).cloned() else {
      return;
    };
    let Some(document_state) = state.document else {
      return;
    };
    let Some(document) = self.editor.buffer_document(buffer_id) else {
      return;
    };
    let new_text = document.text().clone();
    let next_version = document.version() as i32;
    let runtime_ids = state.attached_runtime_ids;
    for runtime_id in runtime_ids {
      if !state.opened_runtime_ids.contains(&runtime_id) {
        continue;
      }
      let Some(sync_kind) = self.lsp_sync_kind_for_runtime(runtime_id) else {
        continue;
      };
      let Some(params) = did_change_params(
        &document_state.uri,
        next_version,
        old_text,
        &new_text,
        changes,
        sync_kind,
      ) else {
        continue;
      };
      if let Some(runtime) = self.lsp_runtimes.get(&runtime_id) {
        let _ = runtime
          .runtime
          .send_notification("textDocument/didChange", Some(params));
      }
    }
    if let Some(state) = self.buffer_lsp_states.get_mut(&buffer_id)
      && let Some(document_state) = state.document.as_mut()
    {
      document_state.version = next_version;
      document_state.opened = state
        .attached_runtime_ids
        .first()
        .is_some_and(|runtime_id| state.opened_runtime_ids.contains(runtime_id));
    }
    if self.editor.active_buffer_id() == buffer_id {
      self.sync_active_lsp_mirror_state();
    }
  }

  fn lsp_send_did_save(&mut self, text: Option<&str>) {
    let buffer_id = self.editor.active_buffer_id();
    self.lsp_send_did_save_for_buffer(buffer_id, text);
  }

  fn lsp_send_did_save_for_buffer(&mut self, buffer_id: BufferId, text: Option<&str>) {
    let Some(state) = self.buffer_lsp_states.get(&buffer_id).cloned() else {
      return;
    };
    let Some(document_state) = state.document else {
      return;
    };
    for runtime_id in state.attached_runtime_ids {
      if !state.opened_runtime_ids.contains(&runtime_id) {
        continue;
      }
      let payload_text = if self.lsp_save_include_text_for_runtime(runtime_id) {
        text
      } else {
        None
      };
      let params = did_save_params(&document_state.uri, payload_text);
      if let Some(runtime) = self.lsp_runtimes.get(&runtime_id) {
        let _ = runtime
          .runtime
          .send_notification("textDocument/didSave", Some(params));
      }
    }
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

  fn lsp_refresh_document_state(&mut self, path: Option<&Path>) {
    let buffer_id = self.editor.active_buffer_id();
    self.refresh_buffer_lsp_state(buffer_id, path);
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
    self.messages.publish_with_disposition(
      level,
      Some("lsp".into()),
      MessageDisposition::Background,
      text,
    );
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

fn lsp_signature_help_retrigger_latency() -> Duration {
  Duration::from_millis(80)
}

fn lsp_signature_help_trigger_char_latency() -> Duration {
  Duration::from_millis(20)
}

fn capabilities_support_single_char(
  raw_capabilities: &Value,
  provider_key: &str,
  characters_key: &str,
  ch: char,
) -> bool {
  let Some(values) = raw_capabilities
    .get(provider_key)
    .and_then(|provider| provider.get(characters_key))
    .and_then(Value::as_array)
  else {
    return false;
  };

  values.iter().filter_map(Value::as_str).any(|value| {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first == ch && chars.next().is_none())
  })
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
    LspEvent::WorkspaceApplyEdit { label, edit } => {
      json!({
        "name": "workspace_apply_edit",
        "label": label,
        "documents": edit.documents.len(),
        "edits": edit.documents.iter().map(|doc| doc.edits.len()).sum::<usize>(),
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

fn sanitize_picker_field(value: &str) -> String {
  value
    .replace('\t', " ")
    .replace(['\r', '\n'], " ")
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
}

fn lsp_symbol_tree_depth(container: &str, stack: &mut Vec<String>) -> usize {
  if container.is_empty() {
    return 0;
  }

  while let Some(last) = stack.last() {
    if last == container {
      return stack.len();
    }
    stack.pop();
  }

  0
}

fn lsp_symbol_kind_label(kind: u32) -> &'static str {
  match kind {
    1 => "FILE",
    2 => "MODULE",
    3 => "NAMESPACE",
    4 => "PACKAGE",
    5 => "CLASS",
    6 => "METHOD",
    7 => "PROPERTY",
    8 => "FIELD",
    9 => "CONSTRUCTOR",
    10 => "ENUM",
    11 => "INTERFACE",
    12 => "FUNCTION",
    13 => "VARIABLE",
    14 => "CONSTANT",
    15 => "STRING",
    16 => "NUMBER",
    17 => "BOOLEAN",
    18 => "ARRAY",
    19 => "OBJECT",
    20 => "KEY",
    21 => "NULL",
    22 => "ENUM_MEMBER",
    23 => "STRUCT",
    24 => "EVENT",
    25 => "OPERATOR",
    26 => "TYPE_PARAM",
    _ => "SYMBOL",
  }
}

fn lsp_symbol_icon_name(kind: u32) -> &'static str {
  match kind {
    2 | 3 | 4 | 5 | 10 | 11 | 19 | 23 => "folder",
    6 | 9 | 12 | 25 => "file_code",
    7 | 8 | 13 | 14 | 18 | 20 | 22 | 24 | 26 => "file_generic",
    15 | 16 | 17 | 21 => "file_doc",
    1 => "file_doc",
    _ => "file_generic",
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

fn completion_menu_item_for_code_action(action: &LspCodeAction) -> the_default::CompletionMenuItem {
  let mut menu_item = the_default::CompletionMenuItem::new(action.title.clone());
  let mut tags: Vec<&str> = Vec::new();
  if action.is_preferred {
    tags.push("preferred");
  }
  if action.edit.is_some() {
    tags.push("edit");
  }
  if action.command.is_some() {
    tags.push("command");
  }
  if !tags.is_empty() {
    menu_item.detail = Some(tags.join(" | "));
  }
  menu_item
}

fn completion_kind_icon(kind: LspCompletionItemKind) -> &'static str {
  use LspCompletionItemKind::*;
  match kind {
    Text => "w",
    Method => "f",
    Function => "f",
    Constructor => "f",
    Field => "m",
    Variable => "v",
    Class => "c",
    Interface => "i",
    Module => "M",
    Property => "m",
    Unit => "u",
    Value => "v",
    Enum => "e",
    Keyword => "k",
    Snippet => "S",
    Color => "v",
    File => "F",
    Reference => "r",
    Folder => "D",
    EnumMember => "e",
    Constant => "C",
    Struct => "s",
    Event => "E",
    Operator => "o",
    TypeParameter => "t",
  }
}

fn completion_kind_color(kind: LspCompletionItemKind) -> the_lib::render::graphics::Color {
  use LspCompletionItemKind::*;
  use the_lib::render::graphics::Color;
  match kind {
    Method | Function | Constructor | Operator => Color::Rgb(0xDB, 0xBF, 0xEF), // lilac
    Field | Variable | Property | Value | Reference => Color::Rgb(0xA4, 0xA0, 0xE8), // lavender
    Class | Interface | Enum | Struct | TypeParameter => Color::Rgb(0xEF, 0xBA, 0x5D), // honey
    Module | Folder | EnumMember | Constant => Color::Rgb(0xE8, 0xDC, 0xA0),    // chamois
    Keyword => Color::Rgb(0xEC, 0xCD, 0xBA),                                    // almond
    Snippet => Color::Rgb(0x9F, 0xF2, 0x8F),                                    // mint
    Event => Color::Rgb(0xF4, 0x78, 0x68),                                      // apricot
    Text | Unit | Color | File => Color::Rgb(0xCC, 0xCC, 0xCC),                 // silver
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
    .and_then(normalize_completion_documentation)
}

fn normalize_completion_documentation(value: &str) -> Option<String> {
  let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
  let trimmed = normalized.trim();
  if trimmed.is_empty() {
    None
  } else {
    Some(trimmed.to_string())
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompletionSnippetCursorOrigin {
  InsertText,
  PrimaryEdit,
}

#[derive(Clone, Debug)]
struct CompletionApplyItem {
  item:          LspCompletionItem,
  cursor_origin: Option<CompletionSnippetCursorOrigin>,
  cursor_range:  Option<std::ops::Range<usize>>,
}

fn normalize_completion_item_for_apply(mut item: LspCompletionItem) -> CompletionApplyItem {
  let mut cursor_origin = None;
  let mut cursor_range = None;
  if item.insert_text_format == Some(LspInsertTextFormat::Snippet) {
    if let Some(insert_text) = item.insert_text.as_mut() {
      let rendered = render_lsp_snippet(insert_text);
      if item.primary_edit.is_none() {
        cursor_origin = Some(CompletionSnippetCursorOrigin::InsertText);
        cursor_range = rendered.cursor_char_range.clone();
      }
      *insert_text = rendered.text;
    }
    if let Some(primary_edit) = item.primary_edit.as_mut() {
      let rendered = render_lsp_snippet(&primary_edit.new_text);
      cursor_origin = Some(CompletionSnippetCursorOrigin::PrimaryEdit);
      cursor_range = rendered.cursor_char_range.clone();
      primary_edit.new_text = rendered.text;
    }
    for additional in &mut item.additional_edits {
      additional.new_text = render_lsp_snippet(&additional.new_text).text;
    }
  }
  if cursor_origin.is_none()
    && let Some((origin, range)) = promote_callable_completion_fallback(&mut item)
  {
    cursor_origin = Some(origin);
    cursor_range = Some(range);
  }
  CompletionApplyItem {
    item,
    cursor_origin,
    cursor_range,
  }
}

fn promote_callable_completion_fallback(
  item: &mut LspCompletionItem,
) -> Option<(CompletionSnippetCursorOrigin, std::ops::Range<usize>)> {
  if !matches!(
    item.kind,
    Some(
      LspCompletionItemKind::Function
        | LspCompletionItemKind::Method
        | LspCompletionItemKind::Constructor
    )
  ) {
    return None;
  }

  let (text, origin) = if let Some(primary) = item.primary_edit.as_mut() {
    (
      &mut primary.new_text,
      CompletionSnippetCursorOrigin::PrimaryEdit,
    )
  } else {
    if item.insert_text.is_none() {
      item.insert_text = Some(item.label.clone());
    }
    (
      item.insert_text.as_mut()?,
      CompletionSnippetCursorOrigin::InsertText,
    )
  };

  let trimmed = text.trim();
  if trimmed.is_empty() || trimmed.contains('(') || trimmed.ends_with('!') {
    return None;
  }
  if !trimmed
    .chars()
    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '.'))
  {
    return None;
  }

  let cursor = text.chars().count().saturating_add(1);
  text.push_str("()");
  Some((origin, cursor..cursor))
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

fn set_completion_snippet_selection(
  doc: &mut Document,
  mapped_base: usize,
  cursor_range: &std::ops::Range<usize>,
) {
  let max = doc.text().len_chars();
  let anchor = mapped_base.saturating_add(cursor_range.start).min(max);
  let head = mapped_base.saturating_add(cursor_range.end).min(max);
  let _ = doc.set_selection(Selection::single(anchor, head));
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

fn lsp_position_lt(left: &LspPosition, right: &LspPosition) -> bool {
  (left.line, left.character) < (right.line, right.character)
}

fn lsp_position_le(left: &LspPosition, right: &LspPosition) -> bool {
  (left.line, left.character) <= (right.line, right.character)
}

fn lsp_range_is_empty(range: &the_lsp::LspRange) -> bool {
  range.start.line == range.end.line && range.start.character == range.end.character
}

fn lsp_range_contains_point(range: &the_lsp::LspRange, point: &LspPosition) -> bool {
  // LSP ranges are half-open: [start, end)
  lsp_position_le(&range.start, point) && lsp_position_lt(point, &range.end)
}

fn lsp_ranges_overlap(left: &the_lsp::LspRange, right: &the_lsp::LspRange) -> bool {
  let left_empty = lsp_range_is_empty(left);
  let right_empty = lsp_range_is_empty(right);

  if left_empty && right_empty {
    return left.start.line == right.start.line && left.start.character == right.start.character;
  }
  if left_empty {
    return lsp_range_contains_point(right, &left.start);
  }
  if right_empty {
    return lsp_range_contains_point(left, &right.start);
  }

  lsp_position_lt(&left.start, &right.end) && lsp_position_lt(&right.start, &left.end)
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

impl Ctx {
  pub(crate) fn visible_editor_pane_for_viewport(&self) -> Option<PaneId> {
    let active_pane = self.editor.active_pane_id();
    if matches!(
      self.editor.pane_content_kind(active_pane),
      Some(PaneContentKind::EditorBuffer)
    ) {
      return Some(active_pane);
    }

    if let Some(pane) = self.file_tree.last_editor_pane
      && matches!(
        self.editor.pane_content_kind(pane),
        Some(PaneContentKind::EditorBuffer)
      )
    {
      return Some(pane);
    }

    self
      .editor
      .pane_snapshots(self.editor.layout_viewport())
      .into_iter()
      .next()
      .map(|pane| pane.pane_id)
  }

  fn sync_state_after_active_pane_change(&mut self, previous_buffer_id: BufferId) {
    the_default::remember_active_editor_pane(self);
    self.clear_hover_state();
    self.clear_completion_state();
    self.cancel_auto_completion();
    self.clear_signature_help_state();
    self.cancel_auto_signature_help();

    if self.editor.active_buffer_id() == previous_buffer_id {
      return;
    }

    self.syntax_parse_lifecycle.cancel_pending();
    self.highlight_cache.clear();
    self.inactive_highlight_caches.clear();
    if self.editor.document().syntax().is_some() {
      self.syntax_parse_highlight_state.mark_parsed();
    } else {
      self.syntax_parse_highlight_state.mark_cleared();
    }

    let active_path = self.editor.active_file_path().map(Path::to_path_buf);
    let previous_path = self.file_path.clone();
    self.file_path = active_path.clone();
    self.lsp_refresh_document_state(active_path.as_deref());
    self.lsp_open_current_document();
    self.refresh_active_file_vcs_after_path_change(
      previous_path,
      ActiveFileVcsRefreshReason::PathChange,
    );
    the_default::sync_file_tree_to_active_file(self);
  }
}

fn file_tree_changed_refresh_latency() -> Duration {
  Duration::from_millis(200)
}

fn vcs_watch_latency() -> Duration {
  Duration::from_millis(75)
}

fn term_render_perf_enabled() -> bool {
  env::var("THE_TERM_DEBUG_RENDER_PERF").ok().as_deref() == Some("1")
}

fn append_perf_line(data: &[u8]) {
  let Some(path) = env::var("THE_TERM_DEBUG_RENDER_PERF_FILE")
    .ok()
    .map(|raw| raw.trim().to_string())
    .filter(|raw| !raw.is_empty())
    .map(PathBuf::from)
  else {
    return;
  };
  if let Some(parent) = path.parent() {
    let _ = std::fs::create_dir_all(parent);
  }
  if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
    let _ = file.write_all(data);
  }
}

fn log_file_tree_vcs_refresh_event(
  phase: &str,
  generation: u64,
  root: &Path,
  reason: FileTreeVcsRefreshReason,
  change_count: Option<usize>,
  status_entries: Option<usize>,
  collect_ms: Option<f64>,
  collapse_ms: Option<f64>,
  apply_ms: Option<f64>,
  decorations_changed: Option<bool>,
) {
  if !term_render_perf_enabled() {
    return;
  }
  let ts_ms = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .map(|duration| duration.as_millis())
    .unwrap_or(0);
  let line = format!(
    "[filetreevcs {ts_ms}] kind=file_tree_vcs_refresh phase={phase} generation={generation} \
     root={} reason={} changes={} status_entries={} collect={:.2}ms collapse={:.2}ms \
     apply={:.2}ms decorations_changed={}\n",
    root.display(),
    reason.log_label(),
    change_count.map_or(String::from("-"), |value| value.to_string()),
    status_entries.map_or(String::from("-"), |value| value.to_string()),
    collect_ms.unwrap_or(0.0),
    collapse_ms.unwrap_or(0.0),
    apply_ms.unwrap_or(0.0),
    decorations_changed.map_or("-", |value| if value { "1" } else { "0" }),
  );
  append_perf_line(line.as_bytes());
}

fn log_active_file_vcs_refresh_event(
  phase: &str,
  generation: u64,
  path: &Path,
  reason: ActiveFileVcsRefreshReason,
  statusline_present: Option<bool>,
  diff_base_present: Option<bool>,
  collect_ms: Option<f64>,
  apply_ms: Option<f64>,
) {
  if !term_render_perf_enabled() {
    return;
  }
  let ts_ms = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .map(|duration| duration.as_millis())
    .unwrap_or(0);
  let line = format!(
    "[activefilevcs {ts_ms}] kind=active_file_vcs_refresh phase={phase} generation={generation} \
     path={} reason={} statusline={} diff_base={} collect={:.2}ms apply={:.2}ms\n",
    path.display(),
    reason.log_label(),
    statusline_present.map_or("-", |value| if value { "1" } else { "0" }),
    diff_base_present.map_or("-", |value| if value { "1" } else { "0" }),
    collect_ms.unwrap_or(0.0),
    apply_ms.unwrap_or(0.0),
  );
  append_perf_line(line.as_bytes());
}

fn build_file_picker_vcs_diff_entry(
  ctx: &Ctx,
  change: &FileChange,
) -> Result<FilePickerVcsDiffEntry, String> {
  let current_text = vcs_worktree_text(ctx, change)?;
  let base_text = vcs_base_text_for_change(&ctx.vcs_provider, change)?;
  build_file_picker_vcs_diff_entry_with_text(
    change,
    &ctx.workspace_root(),
    &base_text,
    &current_text,
  )
}

fn vcs_diff_entries_match_changed_items(
  entries: &[FilePickerVcsDiffEntry],
  changed: &[FilePickerChangedFileItem],
) -> bool {
  entries.len() == changed.len()
    && entries.iter().zip(changed.iter()).all(|(entry, item)| {
      entry.kind == item.kind && entry.path == item.path && entry.from_path == item.from_path
    })
}

fn restart_open_vcs_diff_picker_from_scan(ctx: &mut Ctx, scan: Arc<VcsWorkspaceScan>) -> bool {
  if let Some(cancel) = ctx.vcs_diff_picker.cancel.take() {
    cancel.store(true, Ordering::Relaxed);
  }

  let changed = ctx.merged_vcs_changed_file_items(&scan);
  let root = scan.repo_root.clone();
  let open_buffers = ctx
    .editor
    .buffer_snapshots_mru()
    .iter()
    .filter_map(|buffer| {
      let path = vcs_repo_absolute_path(
        ctx.editor.buffer_file_path(buffer.buffer_id)?,
        &scan.repo_root,
      );
      let document = ctx.editor.document_for_buffer(buffer.buffer_id)?;
      Some((path, OpenBufferVcsSnapshot {
        text:     document.text().to_string(),
        modified: buffer.modified,
      }))
    })
    .collect::<HashMap<_, _>>();

  ctx.vcs_diff_picker.generation = ctx.vcs_diff_picker.generation.wrapping_add(1);
  ctx.vcs_diff_picker.scan_generation = ctx.shared_vcs.generation;
  ctx.vcs_diff_picker.live_refresh_pending = false;
  ctx.vcs_diff_picker.root = root.clone();

  let mut updated_ui = false;
  if ctx.vcs_diff_picker.entries.is_empty()
    || !vcs_diff_entries_match_changed_items(&ctx.vcs_diff_picker.entries, &changed)
  {
    ctx.vcs_diff_picker.entries = changed
      .iter()
      .map(file_picker_vcs_diff_placeholder_entry)
      .collect();

    let submit_handler = ctx
      .file_picker
      .runtime_session()
      .map(the_default::PickerSubmitHandlerRef::Runtime);
    let specs = file_picker_vcs_diff_specs(&root, &ctx.vcs_diff_picker.entries);
    let items = file_picker_items_from_specs(specs, submit_handler);
    replace_file_picker_items_preserving_selection(ctx, items, 0);
    updated_ui = true;
  }

  let generation = ctx.vcs_diff_picker.generation;
  let shared_scan = (*scan).clone();
  let vcs_provider = ctx.vcs_provider.clone();
  let wake_tx = ctx.render_wake_tx.clone();
  let cancel = Arc::new(AtomicBool::new(false));
  let (result_tx, result_rx) = channel();
  ctx.vcs_diff_picker.cancel = Some(cancel.clone());
  ctx.vcs_diff_picker.result_rx = Some(result_rx);

  thread::spawn(move || {
    const WAKE_BATCH_SIZE: usize = 8;

    let base_loader = picker_diff_base_loader_for_scan(&shared_scan, vcs_provider);
    let mut pending_since_wake = 0usize;
    for (index, item) in changed.iter().enumerate() {
      if cancel.load(Ordering::Relaxed) {
        return;
      }
      let entry = build_file_picker_vcs_diff_entry_from_snapshot(
        item,
        &shared_scan,
        &base_loader,
        &open_buffers,
      )
      .unwrap_or_else(|err| {
        FilePickerVcsDiffEntry {
          kind:      item.kind,
          path:      item.path.clone(),
          from_path: item.from_path.clone(),
          hunks:     vec![FilePickerVcsDiffHunk {
            summary:            err.clone(),
            target_line:        None,
            target_cursor_char: None,
            before_start:       0,
            before_end:         0,
            after_start:        0,
            after_end:          0,
            preview:            the_default::FilePickerPreview::Message(err),
          }],
        }
      });
      if result_tx
        .send(VcsDiffPickerFileResult::Entry {
          generation,
          index,
          entry,
        })
        .is_err()
      {
        return;
      }
      pending_since_wake = pending_since_wake.saturating_add(1);
      if pending_since_wake >= WAKE_BATCH_SIZE {
        pending_since_wake = 0;
        let _ = wake_tx.send(());
      }
    }
    if result_tx
      .send(VcsDiffPickerFileResult::Complete { generation })
      .is_ok()
    {
      let _ = wake_tx.send(());
    }
  });

  updated_ui
}

fn build_file_picker_vcs_diff_entry_from_snapshot(
  item: &FilePickerChangedFileItem,
  scan: &VcsWorkspaceScan,
  base_loader: &PickerDiffBaseLoader,
  open_buffers: &HashMap<PathBuf, OpenBufferVcsSnapshot>,
) -> Result<FilePickerVcsDiffEntry, String> {
  let change = file_picker_changed_file_to_vcs_change(item);
  let current_text = vcs_worktree_text_from_snapshot(open_buffers, &change)?;
  let base_text = base_loader.load_text_for_change(&change)?;
  build_file_picker_vcs_diff_entry_with_text(&change, &scan.repo_root, &base_text, &current_text)
}

fn build_file_picker_vcs_diff_entry_with_text(
  change: &FileChange,
  workspace_root: &Path,
  base_text: &str,
  current_text: &str,
) -> Result<FilePickerVcsDiffEntry, String> {
  let path = change.path().to_path_buf();
  let from_path = match change {
    FileChange::Renamed { from_path, .. } => Some(from_path.clone()),
    _ => None,
  };
  let kind = file_picker_changed_kind_for_vcs(change);
  let display_path = display_vcs_picker_path(&path, workspace_root);
  let from_display = from_path
    .as_ref()
    .map(|from_path| display_vcs_picker_path(from_path, workspace_root));
  let current_rope = Rope::from_str(current_text);
  let base_rope = Rope::from_str(base_text);
  let handle = DiffHandle::new(base_rope.clone(), current_rope.clone());
  let diff = handle.load();
  if diff.is_empty() {
    return Ok(FilePickerVcsDiffEntry {
      kind,
      path: path.clone(),
      from_path,
      hunks: vec![vcs_info_hunk(
        &display_path,
        from_display.as_deref(),
        &path,
        current_text,
        "No textual diff available",
      )],
    });
  }

  let mut hunks = Vec::with_capacity(diff.len() as usize);
  for index in 0..diff.len() {
    let hunk = diff.nth_hunk(index);
    let target_line = vcs_hunk_target_line(change, &current_rope, &hunk);
    let target_cursor_char = target_line.map(|line| current_rope.line_to_char(line));
    hunks.push(FilePickerVcsDiffHunk {
      summary: vcs_hunk_summary(&base_rope, &current_rope, &hunk),
      target_line,
      target_cursor_char,
      before_start: hunk.before.start as usize,
      before_end: hunk.before.end as usize,
      after_start: hunk.after.start as usize,
      after_end: hunk.after.end as usize,
      preview: FilePickerPreview::Message("Loading diff…".to_string()),
    });
  }

  Ok(FilePickerVcsDiffEntry {
    kind,
    path,
    from_path,
    hunks,
  })
}

impl PickerDiffBaseLoader {
  fn load_text_for_change(&self, change: &FileChange) -> Result<String, String> {
    match self {
      Self::GitRevision {
        repo,
        repo_root,
        revision,
      } => git_revision_text_for_change(repo, repo_root, revision, change),
      Self::Provider(vcs_provider) => vcs_base_text_for_change(vcs_provider, change),
    }
  }
}

fn picker_diff_base_loader_for_scan(
  scan: &VcsWorkspaceScan,
  vcs_provider: DiffProviderRegistry,
) -> PickerDiffBaseLoader {
  let git_loader = |revision: String| {
    Repository::open(&scan.repo_root).ok().map(|repo| {
      PickerDiffBaseLoader::GitRevision {
        repo,
        repo_root: scan.repo_root.clone(),
        revision,
      }
    })
  };

  match scan.provider_label.as_str() {
    "git" => {
      git_loader("HEAD".to_string()).unwrap_or_else(|| PickerDiffBaseLoader::Provider(vcs_provider))
    },
    "jj" => {
      jj_colocated_git_base_revision(&scan.repo_root)
        .ok()
        .flatten()
        .and_then(git_loader)
        .unwrap_or_else(|| PickerDiffBaseLoader::Provider(vcs_provider))
    },
    _ => PickerDiffBaseLoader::Provider(vcs_provider),
  }
}

fn jj_colocated_git_base_revision(repo_root: &Path) -> Result<Option<String>, String> {
  let output = std::process::Command::new("jj")
    .arg("-R")
    .arg(repo_root)
    .arg("log")
    .arg("-r")
    .arg("@-")
    .arg("--no-graph")
    .arg("-T")
    .arg("commit_id ++ \"\\n\"")
    .env_remove("GIT_DIR")
    .env_remove("GIT_WORK_TREE")
    .output()
    .map_err(|err| format!("failed to run jj in '{}': {err}", repo_root.display()))?;
  if !output.status.success() {
    return Ok(None);
  }
  let revision = String::from_utf8(output.stdout).map_err(|_| {
    format!(
      "jj returned non-utf8 revision for '{}'",
      repo_root.display()
    )
  })?;
  let revision = revision.trim();
  if revision.is_empty() {
    Ok(None)
  } else {
    Ok(Some(revision.to_string()))
  }
}

fn git_revision_text_for_change(
  repo: &Repository,
  repo_root: &Path,
  revision: &str,
  change: &FileChange,
) -> Result<String, String> {
  let base_path = match change {
    FileChange::Untracked { .. } => return Ok(String::new()),
    FileChange::Modified { path }
    | FileChange::Conflict { path }
    | FileChange::Deleted { path } => path,
    FileChange::Renamed { from_path, .. } => from_path,
  };

  match git_revision_blob_text(repo, repo_root, revision, base_path)? {
    Some(text) => Ok(text),
    None => Ok(String::new()),
  }
}

fn git_revision_blob_text(
  repo: &Repository,
  repo_root: &Path,
  revision: &str,
  path: &Path,
) -> Result<Option<String>, String> {
  let relative = path.strip_prefix(repo_root).map_err(|_| {
    format!(
      "{} is not under repo root {}",
      path.display(),
      repo_root.display()
    )
  })?;
  let object = repo
    .revparse_single(revision)
    .map_err(|err| format!("failed to resolve git revision '{revision}': {err}"))?;
  let tree = object
    .peel_to_tree()
    .map_err(|err| format!("failed to peel git revision '{revision}' to tree: {err}"))?;
  let entry = match tree.get_path(relative) {
    Ok(entry) => entry,
    Err(_) => return Ok(None),
  };
  if entry.kind() != Some(ObjectType::Blob) {
    return Ok(None);
  }
  let blob = repo
    .find_blob(entry.id())
    .map_err(|err| format!("failed to load git blob for '{}': {err}", path.display()))?;
  String::from_utf8(blob.content().to_vec())
    .map(Some)
    .map_err(|_| format!("base revision is not UTF-8 text for '{}'", path.display()))
}

fn vcs_base_text_for_change(
  vcs_provider: &DiffProviderRegistry,
  change: &FileChange,
) -> Result<String, String> {
  match vcs_provider.get_diff_base_for_change(change) {
    Some(bytes) => {
      String::from_utf8(bytes).map_err(|_| "Base revision is not UTF-8 text".to_string())
    },
    None => Ok(String::new()),
  }
}

fn file_picker_changed_kind_for_vcs(change: &FileChange) -> FilePickerChangedKind {
  match change {
    FileChange::Untracked { .. } => FilePickerChangedKind::Untracked,
    FileChange::Modified { .. } => FilePickerChangedKind::Modified,
    FileChange::Conflict { .. } => FilePickerChangedKind::Conflict,
    FileChange::Deleted { .. } => FilePickerChangedKind::Deleted,
    FileChange::Renamed { .. } => FilePickerChangedKind::Renamed,
  }
}

fn display_vcs_picker_path(path: &Path, root: &Path) -> String {
  path
    .strip_prefix(root)
    .unwrap_or(path)
    .display()
    .to_string()
}

fn vcs_worktree_text(ctx: &Ctx, change: &FileChange) -> Result<String, String> {
  match change {
    FileChange::Deleted { .. } => Ok(String::new()),
    FileChange::Untracked { path }
    | FileChange::Modified { path }
    | FileChange::Conflict { path }
    | FileChange::Renamed { to_path: path, .. } => {
      if let Some(buffer_id) = ctx.editor.find_buffer_by_path(path)
        && let Some(document) = ctx.editor.document_for_buffer(buffer_id)
      {
        return Ok(document.text().to_string());
      }
      std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read '{}': {err}", path.display()))
    },
  }
}

fn vcs_worktree_text_from_snapshot(
  open_buffers: &HashMap<PathBuf, OpenBufferVcsSnapshot>,
  change: &FileChange,
) -> Result<String, String> {
  match change {
    FileChange::Deleted { .. } => Ok(String::new()),
    FileChange::Untracked { path }
    | FileChange::Modified { path }
    | FileChange::Conflict { path }
    | FileChange::Renamed { to_path: path, .. } => {
      if let Some(snapshot) = open_buffers.get(path)
        && snapshot.modified
      {
        return Ok(snapshot.text.clone());
      }
      std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read '{}': {err}", path.display()))
    },
  }
}

fn file_picker_changed_file_to_vcs_change(item: &FilePickerChangedFileItem) -> FileChange {
  match item.kind {
    FilePickerChangedKind::Untracked => {
      FileChange::Untracked {
        path: item.path.clone(),
      }
    },
    FilePickerChangedKind::Modified => {
      FileChange::Modified {
        path: item.path.clone(),
      }
    },
    FilePickerChangedKind::Conflict => {
      FileChange::Conflict {
        path: item.path.clone(),
      }
    },
    FilePickerChangedKind::Deleted => {
      FileChange::Deleted {
        path: item.path.clone(),
      }
    },
    FilePickerChangedKind::Renamed => {
      FileChange::Renamed {
        from_path: item.from_path.clone().unwrap_or_else(|| item.path.clone()),
        to_path:   item.path.clone(),
      }
    },
  }
}

fn vcs_change_for_entry(entry: &FilePickerVcsDiffEntry) -> FileChange {
  match entry.kind {
    FilePickerChangedKind::Untracked => {
      FileChange::Untracked {
        path: entry.path.clone(),
      }
    },
    FilePickerChangedKind::Modified => {
      FileChange::Modified {
        path: entry.path.clone(),
      }
    },
    FilePickerChangedKind::Conflict => {
      FileChange::Conflict {
        path: entry.path.clone(),
      }
    },
    FilePickerChangedKind::Deleted => {
      FileChange::Deleted {
        path: entry.path.clone(),
      }
    },
    FilePickerChangedKind::Renamed => {
      FileChange::Renamed {
        from_path: entry
          .from_path
          .clone()
          .unwrap_or_else(|| entry.path.clone()),
        to_path:   entry.path.clone(),
      }
    },
  }
}

fn vcs_info_hunk(
  display_path: &str,
  from_display: Option<&str>,
  path: &Path,
  current_text: &str,
  message: &str,
) -> FilePickerVcsDiffHunk {
  FilePickerVcsDiffHunk {
    summary:            message.to_string(),
    target_line:        None,
    target_cursor_char: None,
    before_start:       0,
    before_end:         0,
    after_start:        0,
    after_end:          0,
    preview:            FilePickerPreview::VcsDiff(finalize_vcs_diff_preview(
      FilePickerVcsDiffPreview {
        title:        display_path.to_string(),
        from_title:   from_display.map(ToOwned::to_owned),
        left_label:   "BASE".to_string(),
        right_label:  "WORKTREE".to_string(),
        left:         file_picker_source_preview_from_text(path, "", None),
        right:        file_picker_source_preview_from_text(path, current_text, None),
        rows:         vec![FilePickerVcsDiffPreviewRow {
          kind:              FilePickerVcsDiffPreviewRowKind::Info,
          left_line_index:   None,
          right_line_index:  None,
          left_line_number:  None,
          right_line_number: None,
          message:           message.to_string(),
        }],
        cached_lines: Arc::new([]),
      },
    )),
  }
}

fn vcs_hunk_summary(base_rope: &Rope, current_rope: &Rope, hunk: &the_vcs::Hunk) -> String {
  let right_start = hunk.after.start as usize;
  let right_end = hunk.after.end as usize;
  let left_start = hunk.before.start as usize;
  let left_end = hunk.before.end as usize;

  let text = if let Some(text) = first_nonempty_rope_line(current_rope, right_start, right_end) {
    text
  } else if let Some(text) = first_nonempty_rope_line(base_rope, left_start, left_end) {
    text
  } else {
    "changed lines".to_string()
  };

  truncate_vcs_summary(&text, 84)
}

fn first_nonempty_rope_line(rope: &Rope, start: usize, end: usize) -> Option<String> {
  for line_index in start..end {
    let line = rope.line(line_index).to_string();
    let line = line.trim();
    if !line.is_empty() {
      return Some(line.to_string());
    }
  }
  None
}

fn truncate_vcs_summary(text: &str, max_chars: usize) -> String {
  let mut out = String::new();
  for ch in text.chars().take(max_chars) {
    out.push(ch);
  }
  if text.chars().count() > max_chars {
    out.push('…');
  }
  out
}

fn vcs_hunk_target_line(
  change: &FileChange,
  current_rope: &Rope,
  hunk: &the_vcs::Hunk,
) -> Option<usize> {
  if matches!(change, FileChange::Deleted { .. }) {
    return None;
  }
  let total_lines = current_rope.len_lines();
  if total_lines == 0 {
    return None;
  }
  let after_start = hunk.after.start as usize;
  if after_start < total_lines {
    Some(after_start)
  } else {
    Some(total_lines.saturating_sub(1))
  }
}

fn build_vcs_hunk_preview_from_bounds(
  path: &Path,
  display_path: &str,
  from_display: Option<&str>,
  base_rope: &Rope,
  current_rope: &Rope,
  before_start: usize,
  before_end: usize,
  after_start: usize,
  after_end: usize,
  loader: Option<&Loader>,
) -> FilePickerVcsDiffPreview {
  build_vcs_hunk_preview(
    path,
    display_path,
    from_display,
    base_rope,
    current_rope,
    &the_vcs::Hunk {
      before: before_start as u32..before_end as u32,
      after:  after_start as u32..after_end as u32,
    },
    loader,
  )
}

fn build_vcs_hunk_preview(
  path: &Path,
  display_path: &str,
  from_display: Option<&str>,
  base_rope: &Rope,
  current_rope: &Rope,
  hunk: &the_vcs::Hunk,
  _loader: Option<&Loader>,
) -> FilePickerVcsDiffPreview {
  const CONTEXT: usize = 3;

  let before_start = hunk.before.start as usize;
  let before_end = hunk.before.end as usize;
  let after_start = hunk.after.start as usize;
  let after_end = hunk.after.end as usize;

  let context_above = before_start.min(after_start).min(CONTEXT);
  let hidden_above = before_start.min(after_start).saturating_sub(context_above);
  let before_snippet_start = before_start.saturating_sub(context_above);
  let after_snippet_start = after_start.saturating_sub(context_above);
  let base_remaining = base_rope.len_lines().saturating_sub(before_end);
  let current_remaining = current_rope.len_lines().saturating_sub(after_end);
  let context_below = base_remaining.min(current_remaining).min(CONTEXT);
  let before_snippet_end = before_end.saturating_add(context_below);
  let after_snippet_end = after_end.saturating_add(context_below);

  let left = file_picker_source_preview_from_text(
    path,
    &rope_line_range_to_string(base_rope, before_snippet_start, before_snippet_end),
    None,
  );
  let right = file_picker_source_preview_from_text(
    path,
    &rope_line_range_to_string(current_rope, after_snippet_start, after_snippet_end),
    None,
  );

  let mut rows = Vec::new();
  if hidden_above > 0 {
    rows.push(FilePickerVcsDiffPreviewRow {
      kind:              FilePickerVcsDiffPreviewRowKind::CollapsedAbove,
      left_line_index:   None,
      right_line_index:  None,
      left_line_number:  None,
      right_line_number: None,
      message:           format!("… {} lines above", hidden_above),
    });
  }

  for offset in 0..context_above {
    rows.push(FilePickerVcsDiffPreviewRow {
      kind:              FilePickerVcsDiffPreviewRowKind::Context,
      left_line_index:   Some(offset),
      right_line_index:  Some(offset),
      left_line_number:  Some(before_snippet_start + offset + 1),
      right_line_number: Some(after_snippet_start + offset + 1),
      message:           String::new(),
    });
  }

  let before_len = before_end.saturating_sub(before_start);
  let after_len = after_end.saturating_sub(after_start);
  let changed_len = before_len.max(after_len).max(1);
  for offset in 0..changed_len {
    let left_line = (offset < before_len).then_some(before_start + offset);
    let right_line = (offset < after_len).then_some(after_start + offset);
    let kind = match (left_line, right_line) {
      (Some(_), Some(_)) if hunk.is_pure_insertion() => FilePickerVcsDiffPreviewRowKind::Added,
      (Some(_), Some(_)) if hunk.is_pure_removal() => FilePickerVcsDiffPreviewRowKind::Removed,
      (Some(_), Some(_)) => FilePickerVcsDiffPreviewRowKind::Modified,
      (Some(_), None) => FilePickerVcsDiffPreviewRowKind::Removed,
      (None, Some(_)) => FilePickerVcsDiffPreviewRowKind::Added,
      (None, None) => FilePickerVcsDiffPreviewRowKind::Info,
    };
    rows.push(FilePickerVcsDiffPreviewRow {
      kind,
      left_line_index: left_line.map(|line| line.saturating_sub(before_snippet_start)),
      right_line_index: right_line.map(|line| line.saturating_sub(after_snippet_start)),
      left_line_number: left_line.map(|line| line + 1),
      right_line_number: right_line.map(|line| line + 1),
      message: String::new(),
    });
  }

  for offset in 0..context_below {
    rows.push(FilePickerVcsDiffPreviewRow {
      kind:              FilePickerVcsDiffPreviewRowKind::Context,
      left_line_index:   Some(before_end + offset - before_snippet_start),
      right_line_index:  Some(after_end + offset - after_snippet_start),
      left_line_number:  Some(before_end + offset + 1),
      right_line_number: Some(after_end + offset + 1),
      message:           String::new(),
    });
  }

  let hidden_below = base_remaining
    .min(current_remaining)
    .saturating_sub(context_below);
  if hidden_below > 0 {
    rows.push(FilePickerVcsDiffPreviewRow {
      kind:              FilePickerVcsDiffPreviewRowKind::CollapsedBelow,
      left_line_index:   None,
      right_line_index:  None,
      left_line_number:  None,
      right_line_number: None,
      message:           format!("… {} lines below", hidden_below),
    });
  }

  finalize_vcs_diff_preview(FilePickerVcsDiffPreview {
    title: display_path.to_string(),
    from_title: from_display.map(ToOwned::to_owned),
    left_label: "BASE".to_string(),
    right_label: "WORKTREE".to_string(),
    left,
    right,
    rows,
    cached_lines: Arc::new([]),
  })
}

fn rope_line_range_to_string(rope: &Rope, start_line: usize, end_line: usize) -> String {
  if start_line >= end_line {
    return String::new();
  }

  let start_char = rope.line_to_char(start_line);
  let end_char = rope.line_to_char(end_line);
  rope.slice(start_char..end_char).to_string()
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

  fn workspace_root(&self) -> PathBuf {
    self
      .active_managed_lsp_runtime()
      .map(|runtime| runtime.runtime.config().workspace_root().to_path_buf())
      .or_else(|| self.file_path.as_deref().map(workspace_root_for_path))
      .unwrap_or_else(|| the_loader::find_workspace().0)
  }

  fn working_directory_state(&self) -> &WorkingDirectoryState {
    &self.working_directory
  }

  fn working_directory_state_mut(&mut self) -> &mut WorkingDirectoryState {
    &mut self.working_directory
  }

  fn request_render(&mut self) {
    self.needs_render = true;
  }

  fn render_waker(&self) -> the_default::RenderWaker {
    the_default::RenderWaker::new(self.render_wake_tx.clone())
  }

  fn messages(&self) -> &MessageCenter {
    &self.messages
  }

  fn messages_mut(&mut self) -> &mut MessageCenter {
    &mut self.messages
  }

  fn watch_statusline_text(&self) -> Option<String> {
    self
      .lsp_watched_file
      .as_ref()
      .and_then(|watch| watch_statusline_text_for_state(watch.stream.reload_state))
  }

  fn diagnostic_statusline_counts(&self) -> Option<DiagnosticCounts> {
    let state = self.lsp_document.as_ref().filter(|_| {
      self
        .active_buffer_lsp_state()
        .is_some_and(|buffer_state| !buffer_state.opened_runtime_ids.is_empty())
    })?;
    self
      .diagnostics
      .document(&state.uri)
      .map(|document| document.counts())
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
    let old_text_for_lsp = self.editor.document().text().clone();
    let loader = self.loader.clone();
    let (changed, has_syntax) = {
      let doc = self.editor.document_mut();
      if doc
        .apply_transaction_with_syntax(transaction, loader.as_deref())
        .is_err()
      {
        return false;
      }
      (!transaction.changes().is_empty(), doc.syntax().is_some())
    };

    if !changed {
      return true;
    }

    self.editor.mark_active_buffer_modified();
    self.clear_hover_state();
    self.syntax_parse_lifecycle.cancel_pending();
    self.highlight_cache.clear();
    self.inactive_highlight_caches.clear();
    if has_syntax {
      self.syntax_parse_highlight_state.mark_parsed();
    } else {
      self.syntax_parse_highlight_state.mark_cleared();
    }

    self.lsp_send_did_change(&old_text_for_lsp, transaction.changes());
    self.refresh_vcs_diff_document();
    self.queue_open_vcs_diff_picker_refresh();

    true
  }

  fn build_render_plan(&mut self) -> RenderPlan {
    self.sync_completion_menu_inline_item();
    crate::render::build_render_plan(self)
  }

  fn build_render_plan_with_styles(&mut self, styles: RenderStyles) -> RenderPlan {
    self.sync_completion_menu_inline_item();
    crate::render::build_render_plan_with_styles(self, styles)
  }

  fn build_frame_render_plan(&mut self) -> FrameRenderPlan {
    self.sync_completion_menu_inline_item();
    crate::render::build_frame_render_plan(self)
  }

  fn build_frame_render_plan_with_styles(&mut self, styles: RenderStyles) -> FrameRenderPlan {
    self.sync_completion_menu_inline_item();
    crate::render::build_frame_render_plan_with_styles(self, styles)
  }

  fn request_quit(&mut self) {
    self.should_quit = true;
  }

  fn mode(&self) -> Mode {
    self.mode
  }

  fn cursor_blink_generation(&self) -> u64 {
    self.cursor_blink_generation
  }

  fn cursor_shapes(&self) -> CursorShapes {
    self.cursor_shapes
  }

  fn bump_cursor_blink_generation(&mut self) {
    self.cursor_blink_generation = self.cursor_blink_generation.wrapping_add(1);
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

  fn completion_menu_keymaps(&self) -> &the_default::Keymaps {
    &self.completion_menu_keymaps
  }

  fn completion_menu_keymaps_mut(&mut self) -> &mut the_default::Keymaps {
    &mut self.completion_menu_keymaps
  }

  fn inline_completion(&self) -> &the_default::InlineCompletionState {
    &self.inline_completion
  }

  fn inline_completion_mut(&mut self) -> &mut the_default::InlineCompletionState {
    &mut self.inline_completion
  }

  fn signature_help(&self) -> Option<&the_default::SignatureHelpState> {
    Some(&self.signature_help)
  }

  fn signature_help_mut(&mut self) -> Option<&mut the_default::SignatureHelpState> {
    Some(&mut self.signature_help)
  }

  fn file_tree(&self) -> &FileTreeState {
    &self.file_tree
  }

  fn file_tree_mut(&mut self) -> &mut FileTreeState {
    &mut self.file_tree
  }

  fn completion_selection_changed(&mut self, index: usize) {
    if self.code_action_menu_is_active() || self.completion_visible_index_is_inline_item(index) {
      return;
    }
    let source_index = self
      .completion_source_index_for_visible_index(index)
      .unwrap_or(index);
    self.resolve_completion_item_if_needed(source_index);
  }

  fn completion_accept_on_commit_char(&mut self, ch: char) -> bool {
    if self.code_action_menu_is_active() {
      return false;
    }
    let Some(selected) = self.completion_menu.selected else {
      return false;
    };
    if self.completion_visible_index_is_inline_item(selected) {
      return false;
    }
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
    if self.lsp_code_action_menu_active {
      return match command {
        Command::LspCodeActions
        | Command::CompletionNext
        | Command::CompletionPrev
        | Command::CompletionAccept
        | Command::CompletionDocsScrollUp
        | Command::CompletionDocsScrollDown => true,
        Command::CompletionCancel => {
          self.clear_code_action_menu_state();
          true
        },
        _ => {
          self.clear_code_action_menu_state();
          false
        },
      };
    }

    let preserve_completion = self.handle_completion_action(command);
    let _ = self.handle_signature_help_action(command);
    preserve_completion
  }

  fn completion_accept_selected(&mut self, index: usize) -> bool {
    if self.code_action_menu_is_active() {
      if self.lsp_code_action_items.is_empty() {
        self.clear_code_action_menu_state();
        return false;
      }
      let Some(action) = self.lsp_code_action_items.get(index).cloned() else {
        return false;
      };
      let applied = self.apply_code_action(action);
      if applied {
        self.clear_code_action_menu_state();
      }
      return applied;
    }

    if self.completion_visible_index_is_inline_item(index) {
      return the_default::accept_inline_completion(self);
    }

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

  fn completion_menu_closed(&mut self) {
    self.lsp_completion_inline_item_active = false;
    self.clear_code_action_menu_state();
  }

  fn file_picker(&self) -> &FilePickerState {
    &self.file_picker
  }

  fn file_picker_mut(&mut self) -> &mut FilePickerState {
    &mut self.file_picker
  }

  fn picker_runtime_store(&self) -> &the_default::PickerRuntimeStore<Self> {
    &self.picker_runtime_store
  }

  fn picker_runtime_store_mut(&mut self) -> &mut the_default::PickerRuntimeStore<Self> {
    &mut self.picker_runtime_store
  }

  fn global_search(&mut self) {
    self.start_global_search();
  }

  fn file_picker_query_changed(&mut self, query: &str) {
    if self.global_search.is_active() {
      if query.trim().is_empty() {
        self.global_search.cancel_pending();
        replace_file_picker_items(self, Vec::new(), 0);
        self.file_picker.query = query.to_string();
        self.file_picker.cursor = query.len();
        self.file_picker.error = None;
        self.file_picker.preview =
          the_default::FilePickerPreview::Message("Type to search".to_string());
        self.needs_render = true;
      } else {
        self.schedule_global_search(query.to_string());
      }
    }
  }

  fn builtin_completion_menu_items(
    &mut self,
    kind: the_default::BuiltinCompletionMenuKind,
  ) -> Vec<the_default::CompletionMenuItem> {
    match kind {
      the_default::BuiltinCompletionMenuKind::LspCompletion => {
        let inline_item = self.lsp_completion_inline_menu_item();
        self.lsp_completion_inline_item_active = inline_item.is_some();
        let mut items = Vec::new();
        if let Some(item) = inline_item {
          items.push(item);
        }
        items.extend(
          self
            .lsp_completion_visible_indices
            .iter()
            .filter_map(|index| self.lsp_completion_items.get(*index))
            .map(completion_menu_item_for_lsp_item),
        );
        items
      },
      the_default::BuiltinCompletionMenuKind::CodeActions => {
        self.lsp_completion_inline_item_active = false;
        self
          .lsp_code_action_items
          .iter()
          .map(completion_menu_item_for_code_action)
          .collect()
      },
    }
  }

  fn builtin_signature_help_presentation(
    &mut self,
  ) -> Option<the_default::SignatureHelpPresentation> {
    self.lsp_signature_help_presentation.clone()
  }

  fn file_picker_closed(&mut self) {
    self.global_search.deactivate();
  }

  fn search_prompt_ref(&self) -> &the_default::SearchPromptState {
    &self.search_prompt
  }

  fn search_prompt_mut(&mut self) -> &mut the_default::SearchPromptState {
    &mut self.search_prompt
  }

  fn pointer_event(
    &mut self,
    event: the_default::PointerEvent,
  ) -> the_default::PointerEventOutcome {
    let outcome = crate::input::handle_pointer_event(self, event);
    if outcome.handled() {
      outcome
    } else {
      self.handle_editor_pointer_event(event)
    }
  }

  fn dispatch(&self) -> DispatchRef<Self> {
    if let Some(ptr) = self.dispatch_override {
      return DispatchRef::from_ptr(ptr.as_ptr());
    }
    DispatchRef::from_ptr(self.dispatch.as_ref() as *const dyn DefaultApi<Self>)
  }

  fn pending_input(&self) -> Option<&the_default::PendingInput> {
    self.pending_input.as_ref()
  }

  fn set_pending_input(&mut self, pending: Option<the_default::PendingInput>) {
    self.pending_input = pending;
  }

  fn set_word_jump_annotations(&mut self, inline: Vec<InlineAnnotation>, overlay: Vec<Overlay>) {
    self.word_jump_inline_annotations = inline;
    self.word_jump_overlay_annotations = overlay;
  }

  fn set_inline_completion_annotations(&mut self, annotations: the_default::OwnedTextAnnotations) {
    self.inline_completion_annotations = annotations;
  }

  fn clear_inline_completion_annotations(&mut self) {
    self.inline_completion_annotations = the_default::OwnedTextAnnotations::default();
  }

  fn clear_word_jump_annotations(&mut self) {
    self.word_jump_inline_annotations.clear();
    self.word_jump_overlay_annotations.clear();
  }

  fn active_diagnostic_ranges(&self) -> Vec<Range> {
    let Some(state) = self.lsp_document.as_ref().filter(|state| state.opened) else {
      return Vec::new();
    };
    let Some(document) = self.diagnostics.document(&state.uri) else {
      return Vec::new();
    };

    let text = self.editor.document().text();
    let mut ranges = Vec::with_capacity(document.diagnostics.len());
    for diagnostic in &document.diagnostics {
      let start = utf16_position_to_char_idx(
        text,
        diagnostic.range.start.line,
        diagnostic.range.start.character,
      );
      let end = utf16_position_to_char_idx(
        text,
        diagnostic.range.end.line,
        diagnostic.range.end.character,
      );
      ranges.push(Range::new(start, end));
    }
    ranges.sort_by_key(|range| (range.from(), range.to()));
    ranges
  }

  fn change_hunk_ranges(&self) -> Option<Vec<Range>> {
    let handle = self.vcs_diff.as_ref()?;
    let diff = handle.load();
    let text = self.editor.document().text();
    let len_lines = text.len_lines();
    if len_lines == 0 {
      return Some(Vec::new());
    }

    let mut ranges = Vec::with_capacity(diff.len() as usize);
    for idx in 0..diff.len() {
      let hunk = diff.nth_hunk(idx);
      let start_line = (hunk.after.start as usize).min(len_lines.saturating_sub(1));
      let start = text.line_to_char(start_line);
      let end = if hunk.after.is_empty() {
        text.line_to_char((start_line + 1).min(len_lines))
      } else {
        text.line_to_char((hunk.after.end as usize).min(len_lines))
      };
      ranges.push(Range::new(start, end));
    }
    Some(ranges)
  }

  fn file_picker_diagnostics(&self, workspace: bool) -> Vec<FilePickerDiagnosticItem> {
    let mut items = Vec::new();
    let mut rope_cache: HashMap<PathBuf, Rope> = HashMap::new();
    let active_uri = self.lsp_document.as_ref().map(|state| state.uri.as_str());

    let mut collect_document = |uri: &str, diagnostics: &[Diagnostic]| {
      let Some(path) = path_for_file_uri(uri) else {
        return;
      };

      for diagnostic in diagnostics {
        let line = diagnostic.range.start.line as usize;
        let character = diagnostic.range.start.character as usize;
        let cursor_char = if active_uri == Some(uri) {
          utf16_position_to_char_idx(
            self.editor.document().text(),
            diagnostic.range.start.line,
            diagnostic.range.start.character,
          )
        } else {
          let rope = rope_cache.entry(path.clone()).or_insert_with(|| {
            std::fs::read_to_string(&path)
              .map(|text| Rope::from_str(&text))
              .unwrap_or_else(|_| Rope::new())
          });
          utf16_position_to_char_idx(
            rope,
            diagnostic.range.start.line,
            diagnostic.range.start.character,
          )
        };

        items.push(FilePickerDiagnosticItem {
          path: path.clone(),
          line,
          character,
          cursor_char,
          severity: diagnostic.severity,
          code: diagnostic.code.clone(),
          source: diagnostic.source.clone(),
          message: diagnostic.message.clone(),
        });
      }
    };

    if workspace {
      for document in self.diagnostics.documents() {
        collect_document(&document.uri, &document.diagnostics);
      }
    } else if let Some(state) = self.lsp_document.as_ref().filter(|state| state.opened)
      && let Some(document) = self.diagnostics.document(&state.uri)
    {
      collect_document(&document.uri, &document.diagnostics);
    }

    items.sort_by(|left, right| {
      left
        .path
        .cmp(&right.path)
        .then_with(|| left.line.cmp(&right.line))
        .then_with(|| left.character.cmp(&right.character))
    });
    items
  }

  fn file_picker_changed_files(
    &self,
  ) -> std::result::Result<Vec<FilePickerChangedFileItem>, String> {
    let cwd = self.effective_working_directory();
    if !cwd.exists() {
      return Err("current working directory does not exist".to_string());
    }
    let scan = self
      .shared_vcs_scan_for_cwd(&cwd)
      .ok_or_else(|| "no shared vcs snapshot available".to_string())?;
    Ok(self.merged_vcs_changed_file_items(&scan))
  }

  fn file_picker_vcs_diff_bootstrap(
    &mut self,
  ) -> std::result::Result<the_default::FilePickerVcsDiffBootstrap, String> {
    let cwd = self.effective_working_directory();
    if !cwd.exists() {
      return Err("current working directory does not exist".to_string());
    }
    let scan = self
      .refresh_shared_vcs_scan_for_cwd(&cwd)
      .ok_or_else(|| "failed to load shared vcs snapshot".to_string())?;
    Ok(the_default::FilePickerVcsDiffBootstrap::Ready {
      root:    scan.repo_root.clone(),
      changed: self.merged_vcs_changed_file_items(&scan),
    })
  }

  fn file_picker_vcs_diff_did_open(&mut self) {
    let cwd = self.effective_working_directory();
    let Some(scan) = self.refresh_shared_vcs_scan_for_cwd(&cwd) else {
      return;
    };
    let _ = restart_open_vcs_diff_picker_from_scan(self, scan);
  }

  fn file_picker_vcs_diff_entries(
    &self,
  ) -> std::result::Result<Vec<FilePickerVcsDiffEntry>, String> {
    let cwd = self.effective_working_directory();
    if !cwd.exists() {
      return Err("current working directory does not exist".to_string());
    }

    let scan = self
      .shared_vcs_scan_for_cwd(&cwd)
      .ok_or_else(|| "no shared vcs snapshot available".to_string())?;

    let mut entries = Vec::with_capacity(scan.changes.len());
    for change in &scan.changes {
      entries.push(build_file_picker_vcs_diff_entry(self, change)?);
    }

    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
  }

  fn file_picker_selection_changed(&mut self) {
    if !self.file_picker.active || self.file_picker.kind != the_default::FilePickerKind::VcsDiff {
      return;
    }

    let Some(item) = self.file_picker.current_item() else {
      return;
    };
    let Some(payload) = item.payload::<FilePickerVcsDiffPayload>() else {
      return;
    };
    let Some(hunk_index) = payload.hunk_index else {
      return;
    };
    let Some(entry) = self
      .vcs_diff_picker
      .entries
      .get(payload.entry_index)
      .cloned()
    else {
      return;
    };
    let Some(hunk) = entry.hunks.get(hunk_index).cloned() else {
      return;
    };

    if matches!(hunk.preview, FilePickerPreview::VcsDiff(_)) {
      self.file_picker.preview = hunk.preview.clone();
      self.request_render();
      return;
    }

    let Some(scan) = self.shared_vcs_scan_for_path(&payload.path) else {
      self.file_picker.preview = item
        .preview
        .clone()
        .unwrap_or_else(|| FilePickerPreview::Message("No VCS snapshot available".to_string()));
      self.request_render();
      return;
    };

    let change = vcs_change_for_entry(&entry);
    let current_text = match vcs_worktree_text(self, &change) {
      Ok(text) => text,
      Err(err) => {
        self.file_picker.preview = FilePickerPreview::Message(err);
        self.request_render();
        return;
      },
    };
    let base_loader = picker_diff_base_loader_for_scan(&scan, self.vcs_provider.clone());
    let base_text = match base_loader.load_text_for_change(&change) {
      Ok(text) => text,
      Err(err) => {
        self.file_picker.preview = FilePickerPreview::Message(err);
        self.request_render();
        return;
      },
    };

    let display_path = display_vcs_picker_path(&entry.path, &scan.repo_root);
    let from_display = entry
      .from_path
      .as_ref()
      .map(|path| display_vcs_picker_path(path, &scan.repo_root));
    let base_rope = Rope::from_str(&base_text);
    let current_rope = Rope::from_str(&current_text);
    let preview = FilePickerPreview::VcsDiff(build_vcs_hunk_preview_from_bounds(
      &entry.path,
      &display_path,
      from_display.as_deref(),
      &base_rope,
      &current_rope,
      hunk.before_start,
      hunk.before_end,
      hunk.after_start,
      hunk.after_end,
      self.loader.as_deref(),
    ));
    if let Some(entry) = self.vcs_diff_picker.entries.get_mut(payload.entry_index)
      && let Some(hunk) = entry.hunks.get_mut(hunk_index)
    {
      hunk.preview = preview.clone();
    }
    self.file_picker.preview = preview;
    self.request_render();
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
    if !self.inline_completion_annotations.is_empty() {
      let _ = self.inline_completion_annotations.clone().extend_into(
        &mut annotations,
        self.editor.document().text().slice(..),
        self.text_format.viewport_width.max(1),
        self.editor.view().scroll.col,
      );
    }
    if !self.word_jump_inline_annotations.is_empty() {
      let _ = annotations.add_inline_annotations(&self.word_jump_inline_annotations, None);
    }
    if !self.word_jump_overlay_annotations.is_empty() {
      let jump_label_style = self.ui_theme.find_highlight("ui.virtual.jump-label");
      let _ = annotations.add_overlay(&self.word_jump_overlay_annotations, jump_label_style);
    }
    annotations
  }

  fn syntax_loader(&self) -> Option<&Loader> {
    self.loader.as_deref()
  }

  fn ui_theme(&self) -> &Theme {
    &self.ui_theme
  }

  fn ui_theme_name(&self) -> &str {
    &self.ui_theme_name
  }

  fn available_theme_names(&self) -> Vec<String> {
    self.ui_theme_catalog.names()
  }

  fn set_ui_theme(&mut self, theme_name: &str) -> Result<(), String> {
    self.set_ui_theme_named(theme_name)
  }

  fn set_ui_theme_preview(&mut self, theme_name: &str) -> Result<(), String> {
    self.set_ui_theme_preview_named(theme_name)
  }

  fn clear_ui_theme_preview(&mut self) {
    self.clear_ui_theme_preview_state();
  }

  fn set_file_path(&mut self, path: Option<PathBuf>) {
    self.clear_hover_state();
    self.lsp_refresh_document_state(path.as_deref());
    let previous_path = self.file_path.clone();
    self.file_path = path.clone();
    self.editor.set_active_file_path(path);
    self.refresh_active_file_vcs_after_path_change(
      previous_path,
      ActiveFileVcsRefreshReason::PathChange,
    );
    the_default::sync_file_tree_to_active_file(self);
  }

  fn did_change_active_pane(&mut self, previous_buffer_id: BufferId) {
    self.sync_state_after_active_pane_change(previous_buffer_id);
  }

  fn goto_buffer(&mut self, direction: the_default::Direction, count: usize) -> bool {
    let switched = match direction {
      the_default::Direction::Forward => self.editor.switch_buffer_forward(count),
      the_default::Direction::Backward => self.editor.switch_buffer_backward(count),
      _ => false,
    };
    if !switched {
      return false;
    }

    self.syntax_parse_lifecycle.cancel_pending();
    self.highlight_cache.clear();
    self.inactive_highlight_caches.clear();
    if self.editor.document().syntax().is_some() {
      self.syntax_parse_highlight_state.mark_parsed();
    } else {
      self.syntax_parse_highlight_state.mark_cleared();
    }

    let active_path = self
      .editor
      .active_file_path()
      .map(|path| path.to_path_buf());
    let previous_path = self.file_path.clone();
    self.file_path = active_path.clone();
    self.lsp_refresh_document_state(active_path.as_deref());
    self.lsp_open_current_document();
    self.clear_hover_state();
    self.refresh_active_file_vcs_after_path_change(
      previous_path,
      ActiveFileVcsRefreshReason::PathChange,
    );
    self.needs_render = true;
    true
  }

  fn activate_buffer_by_id(&mut self, buffer_id: BufferId) -> bool {
    if self.editor.active_buffer_id() == buffer_id {
      self.request_render();
      return true;
    }

    if !self.editor.set_active_buffer(buffer_id) {
      return false;
    }

    self.syntax_parse_lifecycle.cancel_pending();
    self.highlight_cache.clear();
    self.inactive_highlight_caches.clear();
    if self.editor.document().syntax().is_some() {
      self.syntax_parse_highlight_state.mark_parsed();
    } else {
      self.syntax_parse_highlight_state.mark_cleared();
    }

    let active_path = self.editor.active_file_path().map(Path::to_path_buf);
    let previous_path = self.file_path.clone();
    self.file_path = active_path.clone();
    self.lsp_refresh_document_state(active_path.as_deref());
    self.lsp_open_current_document();
    self.clear_hover_state();
    self.refresh_active_file_vcs_after_path_change(
      previous_path,
      ActiveFileVcsRefreshReason::PathChange,
    );
    self.needs_render = true;
    true
  }

  fn goto_last_accessed_buffer(&mut self) -> bool {
    if !self.editor.goto_last_accessed_buffer() {
      return false;
    }

    self.syntax_parse_lifecycle.cancel_pending();
    self.highlight_cache.clear();
    self.inactive_highlight_caches.clear();
    if self.editor.document().syntax().is_some() {
      self.syntax_parse_highlight_state.mark_parsed();
    } else {
      self.syntax_parse_highlight_state.mark_cleared();
    }

    let active_path = self
      .editor
      .active_file_path()
      .map(|path| path.to_path_buf());
    let previous_path = self.file_path.clone();
    self.file_path = active_path.clone();
    self.lsp_refresh_document_state(active_path.as_deref());
    self.lsp_open_current_document();
    self.clear_hover_state();
    self.refresh_active_file_vcs_after_path_change(
      previous_path,
      ActiveFileVcsRefreshReason::PathChange,
    );
    self.needs_render = true;
    true
  }

  fn goto_last_modified_buffer(&mut self) -> bool {
    if !self.editor.goto_last_modified_buffer() {
      return false;
    }

    self.syntax_parse_lifecycle.cancel_pending();
    self.highlight_cache.clear();
    self.inactive_highlight_caches.clear();
    if self.editor.document().syntax().is_some() {
      self.syntax_parse_highlight_state.mark_parsed();
    } else {
      self.syntax_parse_highlight_state.mark_cleared();
    }

    let active_path = self
      .editor
      .active_file_path()
      .map(|path| path.to_path_buf());
    let previous_path = self.file_path.clone();
    self.file_path = active_path.clone();
    self.lsp_refresh_document_state(active_path.as_deref());
    self.lsp_open_current_document();
    self.clear_hover_state();
    self.refresh_active_file_vcs_after_path_change(
      previous_path,
      ActiveFileVcsRefreshReason::PathChange,
    );
    self.needs_render = true;
    true
  }

  fn jump_forward_in_jumplist(&mut self, count: usize) -> bool {
    let previous_buffer = self.editor.active_buffer_id();
    if !self.editor.jump_forward(count.max(1)) {
      return false;
    }

    if self.editor.active_buffer_id() != previous_buffer {
      self.syntax_parse_lifecycle.cancel_pending();
      self.highlight_cache.clear();
      self.inactive_highlight_caches.clear();
      if self.editor.document().syntax().is_some() {
        self.syntax_parse_highlight_state.mark_parsed();
      } else {
        self.syntax_parse_highlight_state.mark_cleared();
      }

      let active_path = self
        .editor
        .active_file_path()
        .map(|path| path.to_path_buf());
      let previous_path = self.file_path.clone();
      self.file_path = active_path.clone();
      self.lsp_refresh_document_state(active_path.as_deref());
      self.lsp_open_current_document();
      self.clear_hover_state();
      self.refresh_active_file_vcs_after_path_change(
        previous_path,
        ActiveFileVcsRefreshReason::PathChange,
      );
    }

    self.needs_render = true;
    true
  }

  fn jump_backward_in_jumplist(&mut self, count: usize) -> bool {
    let previous_buffer = self.editor.active_buffer_id();
    if !self.editor.jump_backward(count.max(1)) {
      return false;
    }

    if self.editor.active_buffer_id() != previous_buffer {
      self.syntax_parse_lifecycle.cancel_pending();
      self.highlight_cache.clear();
      self.inactive_highlight_caches.clear();
      if self.editor.document().syntax().is_some() {
        self.syntax_parse_highlight_state.mark_parsed();
      } else {
        self.syntax_parse_highlight_state.mark_cleared();
      }

      let active_path = self
        .editor
        .active_file_path()
        .map(|path| path.to_path_buf());
      let previous_path = self.file_path.clone();
      self.file_path = active_path.clone();
      self.lsp_refresh_document_state(active_path.as_deref());
      self.lsp_open_current_document();
      self.clear_hover_state();
      self.refresh_active_file_vcs_after_path_change(
        previous_path,
        ActiveFileVcsRefreshReason::PathChange,
      );
    }

    self.needs_render = true;
    true
  }

  fn log_target_names(&self) -> &'static [&'static str] {
    &["messages", "lsp", "watch", "inline"]
  }

  fn log_path_for_target(&self, target: &str) -> Option<PathBuf> {
    match target {
      "messages" => resolve_message_log_path(),
      "lsp" => resolve_lsp_trace_log_path(),
      "watch" => resolve_file_watch_trace_log_path(),
      "inline" => the_default::resolve_inline_completion_trace_log_path(),
      _ => None,
    }
  }

  fn lsp_goto_definition(&mut self) {
    if !self.lsp_supports(LspCapability::GotoDefinition) {
      let _ =
        <Self as the_default::DefaultContext>::push_error(self, "goto", "No definition found.");
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      let _ =
        <Self as the_default::DefaultContext>::push_error(self, "goto", "No definition found.");
      return;
    };

    self.dispatch_lsp_request(
      "textDocument/definition",
      goto_definition_params(&uri, position),
      PendingLspRequestKind::GotoDefinition { uri },
    );
  }

  fn lsp_goto_declaration(&mut self) {
    if !self.lsp_supports(LspCapability::GotoDeclaration) {
      let _ =
        <Self as the_default::DefaultContext>::push_error(self, "goto", "No declaration found.");
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      let _ =
        <Self as the_default::DefaultContext>::push_error(self, "goto", "No declaration found.");
      return;
    };

    self.dispatch_lsp_request(
      "textDocument/declaration",
      goto_declaration_params(&uri, position),
      PendingLspRequestKind::GotoDeclaration { uri },
    );
  }

  fn lsp_goto_type_definition(&mut self) {
    if !self.lsp_supports(LspCapability::GotoTypeDefinition) {
      let _ = <Self as the_default::DefaultContext>::push_error(
        self,
        "goto",
        "No type definition found.",
      );
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      let _ = <Self as the_default::DefaultContext>::push_error(
        self,
        "goto",
        "No type definition found.",
      );
      return;
    };

    self.dispatch_lsp_request(
      "textDocument/typeDefinition",
      goto_type_definition_params(&uri, position),
      PendingLspRequestKind::GotoTypeDefinition { uri },
    );
  }

  fn lsp_goto_implementation(&mut self) {
    if !self.lsp_supports(LspCapability::GotoImplementation) {
      let _ =
        <Self as the_default::DefaultContext>::push_error(self, "goto", "No implementation found.");
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      let _ =
        <Self as the_default::DefaultContext>::push_error(self, "goto", "No implementation found.");
      return;
    };

    self.dispatch_lsp_request(
      "textDocument/implementation",
      goto_implementation_params(&uri, position),
      PendingLspRequestKind::GotoImplementation { uri },
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

    self.clear_hover_state();
    self.dispatch_lsp_request(
      "textDocument/hover",
      hover_params(&uri, position),
      PendingLspRequestKind::Hover { uri },
    );
  }

  fn lsp_select_references_to_symbol_under_cursor(&mut self) {
    if !self.lsp_supports(LspCapability::DocumentHighlight) {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "document highlights are not supported by the active server",
      );
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "document highlights unavailable: no active LSP document",
      );
      return;
    };

    self.dispatch_lsp_request(
      "textDocument/documentHighlight",
      document_highlight_params(&uri, position),
      PendingLspRequestKind::DocumentHighlightSelect { uri },
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
    self.cancel_auto_signature_help();
    let _ = self.dispatch_signature_help_request(SignatureHelpTriggerSource::Manual, true);
  }

  fn lsp_signature_help_on_insert_mode_entry(&mut self) {
    self.cancel_auto_signature_help();
    let _ = self.dispatch_signature_help_request(SignatureHelpTriggerSource::Manual, false);
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

    let Some((uri, range)) = self.current_lsp_code_action_range() else {
      self.messages.publish(
        MessageLevel::Warning,
        Some("lsp".into()),
        "code actions unavailable: no active LSP document",
      );
      return;
    };

    let diagnostics = self.current_lsp_diagnostics_payload(&uri, &range);
    self.clear_completion_state();
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

  fn suspend_editor(&mut self) -> Result<(), String> {
    #[cfg(unix)]
    {
      let _ = crossterm::terminal::disable_raw_mode();
      let pid = std::process::id().to_string();
      let status = std::process::Command::new("kill")
        .args(["-TSTP", &pid])
        .status()
        .map_err(|err| format!("failed to suspend process: {err}"))?;
      let _ = crossterm::terminal::enable_raw_mode();
      if status.success() {
        Ok(())
      } else {
        Err(format!("suspend command failed with status {status}"))
      }
    }
    #[cfg(not(unix))]
    {
      Err("suspend is not supported on this platform".to_string())
    }
  }

  fn on_file_saved(&mut self, path: &Path, text: &str) {
    let buffer_id = self.editor.active_buffer_id();
    self.on_buffer_saved(buffer_id, path, text);
  }

  fn on_before_quit(&mut self) {
    self.shutdown_background_services();
  }

  fn open_file(&mut self, path: &Path) -> std::io::Result<()> {
    self.clear_hover_state();
    if let Some(index) = self.editor.find_buffer_by_path(path) {
      let _ = self.editor.set_active_buffer(index);
    } else {
      let content = std::fs::read_to_string(path)?;
      let viewport = self.editor.view().viewport;
      let reused_untitled = self.editor.should_reuse_active_untitled_buffer_for_open();
      if reused_untitled {
        let _ = self
          .editor
          .replace_active_buffer(Rope::from_str(&content), Some(path.to_path_buf()));
      } else {
        let view = ViewState::new(viewport, Position::new(0, 0));
        let _ = self
          .editor
          .open_buffer(Rope::from_str(&content), view, Some(path.to_path_buf()));
      }

      {
        let doc = self.editor.document_mut();
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
    }

    self.syntax_parse_lifecycle.cancel_pending();
    self.highlight_cache.clear();
    self.inactive_highlight_caches.clear();
    if self.editor.document().syntax().is_some() {
      self.syntax_parse_highlight_state.mark_parsed();
    } else {
      self.syntax_parse_highlight_state.mark_cleared();
    }

    <Self as the_default::DefaultContext>::set_file_path(self, Some(path.to_path_buf()));
    self.lsp_open_current_document();
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

fn lsp_method_is_unsupported(error: &jsonrpc::ResponseError) -> bool {
  error.code == -32601 || error.message.eq_ignore_ascii_case("method not found")
}

#[cfg(test)]
mod tests {
  use std::{
    collections::{
      BTreeMap,
      HashMap,
    },
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
      Instant,
      SystemTime,
    },
  };

  use ropey::Rope;
  use serde_json::json;
  use the_default::{
    Command,
    CommandEvent,
    CompletionMenuItem,
    DefaultContext,
    FilePickerChangedFileItem,
    FilePickerChangedKind,
    InlineCompletionBackendStatus,
    InlineCompletionDefaults,
    InlineCompletionProvider,
    Key,
    KeyEvent,
    Mode,
    Modifiers,
    PendingInput,
    SearchPromptKind,
    handle_command,
    handle_key,
    scroll_file_tree,
    set_file_tree_visible_rows,
    show_completion_menu,
    toggle_file_tree,
  };
  use the_lib::{
    clipboard::NoClipboard,
    diagnostics::{
      DiagnosticSeverity,
      DiagnosticsState,
    },
    messages::MessageEventKind,
    movement::Direction as SelectionDirection,
    position::{
      Position,
      char_idx_at_coords,
      coords_at_pos,
    },
    render::{
      RenderGutterDiffKind,
      VirtualLineSpec,
    },
    selection::{
      Range,
      Selection,
    },
    split_tree::SplitAxis,
    syntax::OverlayHighlights,
    transaction::Transaction,
    view::ViewState,
  };
  use the_lsp::{
    LspCompletionItem,
    LspCompletionItemKind,
    LspInsertTextFormat,
    LspLocation,
    LspPosition,
    LspRange,
    LspSymbol,
    render_lsp_snippet,
  };
  use the_runtime::file_watch::{
    PathEvent,
    PathEventKind,
  };
  use the_vcs::{
    FileChange,
    VcsWorkspaceScan,
  };

  use super::{
    ActiveFileVcsRefreshReason,
    ActiveFileVcsRefreshResult,
    CompletionSnippetCursorOrigin,
    Ctx,
    DiffHandle,
    FileTreeVcsRefreshReason,
    FileTreeVcsRefreshResult,
    OpenBufferVcsSnapshot,
    PendingAutoSignatureHelp,
    SignatureHelpTriggerSource,
    WatchedFileEventsState,
    build_lsp_document_state,
    capabilities_support_single_char,
    completion_item_accepts_commit_char,
    completion_match_score,
    completion_menu_detail_text,
    completion_menu_documentation_text,
    file_picker_vcs_diff_placeholder_entry,
    merge_resolved_completion_item,
    normalize_completion_item_for_apply,
    vcs_diff_entries_match_changed_items,
    vcs_worktree_text_from_snapshot,
  };
  use crate::{
    ctx::TermCursorMode,
    render::{
      build_render_plan,
      ensure_cursor_visible,
    },
  };

  struct TempTestFile {
    path: PathBuf,
  }

  struct TempTestDir {
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

  fn ctrl_modifiers() -> Modifiers {
    let mut modifiers = Modifiers::empty();
    modifiers.insert(Modifiers::CTRL);
    modifiers
  }

  #[test]
  fn snippet_fallback_renders_placeholders_and_choices() {
    assert_eq!(
      render_lsp_snippet("foo($1, ${2:bar}, ${3|x,y|})$0").text,
      "foo(, bar, x)"
    );
    assert_eq!(render_lsp_snippet("${TM_FILENAME:main}.rs").text, "main.rs");
    assert_eq!(render_lsp_snippet("a\\$b\\}").text, "a$b}");
  }

  #[test]
  fn callable_completion_fallback_adds_parens_and_cursor() {
    let mut item = empty_completion_item();
    item.kind = Some(LspCompletionItemKind::Function);
    item.insert_text = Some("add".to_string());
    item.insert_text_format = Some(LspInsertTextFormat::PlainText);

    let prepared = normalize_completion_item_for_apply(item);
    assert_eq!(prepared.item.insert_text.as_deref(), Some("add()"));
    assert_eq!(
      prepared.cursor_origin,
      Some(CompletionSnippetCursorOrigin::InsertText)
    );
    assert_eq!(prepared.cursor_range, Some(4..4));
  }

  #[test]
  fn pointer_hit_testing_uses_pane_local_view_state_for_shared_buffers() {
    let mut ctx = Ctx::new(None).expect("ctx");
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("alpha\nbeta\ngamma\n".into()))),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    assert!(ctx.editor.split_active_pane(SplitAxis::Vertical));
    let bottom_pane_id = ctx.editor.active_pane_id();
    assert!(ctx.editor.set_active_pane(bottom_pane_id));
    ctx.editor.view_mut().scroll = Position::new(1, 0);

    let panes = ctx.editor.pane_snapshots(ctx.editor.layout_viewport());
    assert_eq!(panes.len(), 2);

    let top_pane = panes
      .iter()
      .find(|pane| pane.pane_id != bottom_pane_id)
      .copied()
      .expect("top pane");
    let bottom_pane = panes
      .iter()
      .find(|pane| pane.pane_id == bottom_pane_id)
      .copied()
      .expect("bottom pane");

    let top_target = ctx
      .pointer_char_idx_for_pane_point(top_pane, top_pane.rect.x, top_pane.rect.y)
      .expect("top pane target");
    let bottom_target = ctx
      .pointer_char_idx_for_pane_point(bottom_pane, bottom_pane.rect.x, bottom_pane.rect.y)
      .expect("bottom pane target");

    assert_eq!(top_target, 0);
    assert_eq!(bottom_target, 6);
  }

  #[test]
  fn active_pane_change_rebinds_file_path_and_lsp_state() {
    let rust = TempTestFile::new("pane-switch-main", "fn main() {}\n");
    let cargo = rust
      .as_path()
      .parent()
      .expect("temp parent")
      .join("Cargo.toml");
    fs::write(
      &cargo,
      "[package]\nname = \"pane-switch\"\nversion = \"0.1.0\"\n",
    )
    .expect("write cargo");

    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.loader = None;
    assert!(ctx.editor.replace_active_buffer(
      Rope::from_str("fn main() {}\n"),
      Some(rust.as_path().to_path_buf())
    ));
    ctx.file_path = Some(rust.as_path().to_path_buf());
    ctx.lsp_document = build_lsp_document_state(rust.as_path(), None);
    assert!(ctx.editor.split_active_pane(SplitAxis::Vertical));
    let cargo_view = ViewState::new(ctx.editor.view().viewport, Position::new(0, 0));
    let _ = ctx.editor.open_buffer(
      Rope::from_str("[package]\nname = \"pane-switch\"\nversion = \"0.1.0\"\n"),
      cargo_view,
      Some(cargo.clone()),
    );
    ctx.file_path = Some(cargo.clone());
    ctx.lsp_document = build_lsp_document_state(cargo.as_path(), None);
    ctx.hover_docs = Some("resolver docs".to_string());
    ctx.completion_menu.active = true;

    let rust_pane = ctx
      .editor
      .pane_snapshots(ctx.editor.layout_viewport())
      .into_iter()
      .find(|pane| ctx.editor.buffer_file_path(pane.buffer_id) == Some(rust.as_path()))
      .expect("rust pane");
    let previous_buffer_id = ctx.editor.active_buffer_id();
    assert!(ctx.editor.set_active_pane(rust_pane.pane_id));
    <Ctx as DefaultContext>::did_change_active_pane(&mut ctx, previous_buffer_id);

    assert_eq!(ctx.file_path.as_deref(), Some(rust.as_path()));
    assert_eq!(ctx.editor.active_file_path(), Some(rust.as_path()));
    assert_eq!(
      ctx.lsp_document.as_ref().map(|state| state.path.as_path()),
      Some(rust.as_path())
    );
    assert!(ctx.hover_docs.is_none());
    assert!(!ctx.completion_menu.active);
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
  fn completion_menu_documentation_preserves_markdown_blocks() {
    let mut item = empty_completion_item();
    item.documentation = Some("```rust\nfn test() {}\n```\n\nMore details".to_string());
    assert_eq!(
      completion_menu_documentation_text(&item).as_deref(),
      Some("```rust\nfn test() {}\n```\n\nMore details")
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
  fn completion_visible_index_maps_past_inline_provider_item() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.lsp_completion_inline_item_active = true;
    ctx.lsp_completion_visible_indices = vec![3, 7];

    assert_eq!(ctx.completion_source_index_for_visible_index(0), None);
    assert_eq!(ctx.completion_source_index_for_visible_index(1), Some(3));
    assert_eq!(ctx.completion_source_index_for_visible_index(2), Some(7));
  }

  #[test]
  fn completion_accept_selected_uses_shifted_lsp_index_with_inline_item() {
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
    ctx.lsp_completion_visible_indices = vec![0];
    ctx.lsp_completion_inline_item_active = true;
    ctx.completion_menu.items = vec![
      the_default::CompletionMenuItem::new("printf(\"hello world\");").detail("Copilot"),
      the_default::CompletionMenuItem::new("hello"),
    ];
    ctx.lsp_completion_fallback_start = Some("say ".chars().count());

    assert!(<Ctx as DefaultContext>::completion_accept_selected(
      &mut ctx, 1
    ));
    assert_eq!(ctx.editor.document().text().to_string(), "say hello");
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
    let mut ctx = Ctx::new(Some(
      fixture
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");
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

    handle_command(&mut ctx, Command::Undo { count: 1 });
    assert_eq!(ctx.editor.document().text().to_string(), before_text);
    assert!(ctx.editor.document().syntax().is_some());
    let undo_plan = build_render_plan(&mut ctx);
    assert!(!undo_plan.lines.is_empty());

    handle_command(&mut ctx, Command::Redo { count: 1 });
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

  impl TempTestDir {
    fn new(prefix: &str) -> Self {
      let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
      let path = std::env::temp_dir().join(format!(
        "the-editor-{prefix}-{}-{nonce}",
        std::process::id(),
      ));
      fs::create_dir_all(&path).expect("create temp dir");
      Self { path }
    }

    fn as_path(&self) -> &Path {
      &self.path
    }

    fn write_file(&self, relative: &str, content: &str) -> PathBuf {
      let path = self.path.join(relative);
      if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create temp file parent");
      }
      fs::write(&path, content).expect("write temp file");
      path
    }

    fn mkdir(&self, relative: &str) -> PathBuf {
      let path = self.path.join(relative);
      fs::create_dir_all(&path).expect("create temp subdir");
      path
    }
  }

  impl Drop for TempTestDir {
    fn drop(&mut self) {
      let _ = fs::remove_dir_all(&self.path);
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

  fn attach_test_file_tree(ctx: &mut Ctx, root: &Path) {
    let surface_id = ctx.editor.create_client_surface();
    ctx.file_tree.surface_id = Some(surface_id);
    ctx.file_tree.sidebar_pane = Some(ctx.editor.active_pane_id());
    ctx.file_tree.root = Some(root.to_path_buf());
    ctx.file_tree.expanded_dirs.clear();
    ctx.file_tree.expanded_dirs.insert(root.to_path_buf());
    assert!(ctx.editor.open_client_surface_in_active_pane(surface_id));
    the_default::refresh_file_tree(ctx);
    ctx.file_tree_decoration_root = Some(root.to_path_buf());
    ctx.clear_pending_file_tree_vcs_refresh();
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
    for (fixture_index, (fixture_name, fixture_text)) in fixture_matrix().into_iter().enumerate() {
      let mut ctx = Ctx::new(None).expect("ctx");

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
  fn syntax_edits_use_synchronous_parse_and_keep_tree_aligned() {
    let mut ctx = Ctx::new(None).expect("ctx");
    if ctx.loader.is_none() {
      return;
    }

    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("let value = 1;\n".repeat(64).into()))),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    let fixture = TempTestFile::with_extension("syntax-sync-edit", "rs", "");
    if let Some(loader) = ctx.loader.clone() {
      let _ = super::setup_syntax(ctx.editor.document_mut(), fixture.as_path(), &loader);
    }
    assert!(ctx.editor.document().syntax().is_some());

    let before = ctx.editor.document().text().clone();
    let edit_tx = Transaction::change(
      &before,
      std::iter::once((0, 0, Some("let inserted = 0;\n".into()))),
    )
    .expect("edit transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &edit_tx));

    assert!(
      ctx.syntax_parse_lifecycle.in_flight().is_none(),
      "editing should not leave a syntax parse job in-flight"
    );
    assert!(
      ctx.syntax_parse_lifecycle.queued().is_none(),
      "editing should not queue deferred syntax parse jobs"
    );
    assert!(
      !ctx.syntax_parse_highlight_state.is_interpolated(),
      "highlight state should remain parsed after synchronous syntax update"
    );

    let doc = ctx.editor.document();
    let syntax = doc.syntax().expect("syntax should remain available");
    let root_end = syntax.tree().root_node().end_byte() as usize;
    assert_eq!(
      root_end,
      doc.text().len_bytes(),
      "syntax tree byte range should stay aligned after synchronous parse"
    );
  }

  #[test]
  fn wrap_command_toggles_soft_wrap_and_changes_render_lines() {
    let mut ctx = Ctx::new(None).expect("ctx");
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
    let mut ctx = Ctx::new(None).expect("ctx");

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
        .iter()
        .any(|slot| slot.is_builtin(the_lib::render::GutterType::LineNumbers))
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
        .iter()
        .any(|slot| slot.is_builtin(the_lib::render::GutterType::LineNumbers))
    );
  }

  #[test]
  fn bootstrap_defaults_apply_theme_picker_line_numbers_and_cursor_shapes() {
    let defaults = the_default::Defaults::new()
      .theme("base16_default")
      .line_numbers(the_lib::render::LineNumberMode::Relative)
      .cursor_shapes(the_default::CursorShapes::new(
        the_default::CursorKind::Underline,
        the_default::CursorKind::Bar,
        the_default::CursorKind::Block,
      ))
      .file_picker(the_default::FilePickerOptions {
        hidden: false,
        ..Default::default()
      });
    let ctx = Ctx::new_with_defaults(None, &defaults).expect("ctx");

    assert_eq!(ctx.ui_theme_name, "base16_default");
    assert_eq!(
      ctx.gutter_config.line_numbers.mode,
      the_lib::render::LineNumberMode::Relative
    );
    assert_eq!(
      ctx.cursor_shapes,
      the_default::CursorShapes::new(
        the_default::CursorKind::Underline,
        the_default::CursorKind::Bar,
        the_default::CursorKind::Block,
      )
    );
    assert!(!ctx.file_picker.options.hidden);
  }

  #[test]
  fn bootstrap_defaults_apply_inline_completion_defaults() {
    let defaults = the_default::Defaults::new().inline_completion(
      InlineCompletionDefaults::new()
        .enabled(false)
        .provider(InlineCompletionProvider::Supermaven),
    );
    let ctx = Ctx::new_with_defaults(None, &defaults).expect("ctx");

    assert!(!ctx.inline_completion.enabled);
    assert_eq!(
      ctx.inline_completion.provider,
      InlineCompletionProvider::Supermaven
    );
    assert_eq!(
      ctx.inline_completion.status,
      InlineCompletionBackendStatus::Idle
    );
  }

  #[test]
  fn default_wiring_registers_inline_completion_commands_and_completions() {
    let ctx = Ctx::new(None).expect("ctx");
    let registry = ctx.command_registry_ref();

    assert!(registry.get("inline-provider").is_some());
    assert!(registry.get("inline-toggle").is_some());
    assert!(registry.get("copilot-sign-in").is_some());
    assert!(registry.get("supermaven-use-free").is_some());
    assert!(registry.get("supermaven-use-pro").is_some());
    assert!(registry.get("supermaven-logout").is_some());
    assert!(registry.get("inline-status").is_some());
    assert!(registry.get("inline-accept").is_some());
    assert!(registry.get("inline-accept-word").is_some());
    assert!(registry.get("inline-accept-line").is_some());
    assert!(registry.get("inline-dismiss").is_some());
    assert!(registry.get("inline-retry").is_some());
    assert!(registry.get("copilot-status").is_some());
    assert!(registry.get("supermaven-accept").is_some());

    let completions = registry.complete_command_line(&ctx, "inline-provider s");
    assert!(
      completions
        .iter()
        .any(|completion| completion.text == "supermaven"),
      "expected supermaven completion, got {:?}",
      completions
        .iter()
        .map(|completion| &completion.text)
        .collect::<Vec<_>>()
    );
  }

  #[test]
  fn inline_debug_log_target_is_exposed() {
    let ctx = Ctx::new(None).expect("ctx");

    assert!(ctx.log_target_names().contains(&"inline"));
    assert_eq!(
      ctx.log_path_for_target("inline"),
      the_default::resolve_inline_completion_trace_log_path()
    );
  }

  #[test]
  fn text_annotations_merge_inline_completion_annotations() {
    let mut ctx = Ctx::new(None).expect("ctx");
    let highlight = ctx.ui_theme.find_highlight("ui.virtual.inline");
    let mut owned = the_default::OwnedTextAnnotations::default();
    let _ = owned.add_overlay_grapheme(0, "x", highlight);
    let _ = owned.add_virtual_line(VirtualLineSpec::after(0).text("ghost line").single_line());
    <Ctx as DefaultContext>::set_inline_completion_annotations(&mut ctx, owned);

    let annotations = <Ctx as DefaultContext>::text_annotations(&ctx);
    assert!(annotations.has_line_annotations());
    assert!(matches!(
      annotations.collect_overlay_highlights(0..1),
      OverlayHighlights::Homogeneous { ranges, .. } if ranges == vec![0..1]
    ));
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
  fn ensure_cursor_visible_tracks_wrapped_visual_rows() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.resize(14, 3);
    DefaultContext::set_soft_wrap_enabled(&mut ctx, true);

    let long_line = "aaaa bbbb cccc dddd eeee ffff gggg";
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some(long_line.into()))),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    let cursor_char = ctx.editor.document().text().len_chars().saturating_sub(1);
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::single(cursor_char, cursor_char));

    ensure_cursor_visible(&mut ctx);

    let view = ctx.editor.view();
    let doc = ctx.editor.document();
    let mut text_format = <Ctx as DefaultContext>::text_format(&ctx);
    let gutter_width =
      the_lib::render::gutter_width_for_document(doc, view.viewport.width, &ctx.gutter_config);
    text_format.viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1);
    let mut annotations = <Ctx as DefaultContext>::text_annotations(&ctx);
    let visual_pos = the_lib::render::visual_pos_at_char(
      doc.text().slice(..),
      &text_format,
      &mut annotations,
      cursor_char,
    )
    .expect("wrapped visual position");
    let expected = the_lib::view::scroll_row_to_keep_visible(
      visual_pos.row,
      0,
      view.viewport.height as usize,
      ctx.scrolloff,
    )
    .expect("scroll adjustment");

    assert!(visual_pos.row > 0);
    assert_eq!(view.scroll.row, expected);
  }

  #[test]
  fn ensure_cursor_visible_uses_editor_pane_when_tree_is_active() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.resize(40, 10);

    let mut source = String::new();
    for idx in 0..80 {
      source.push_str(&format!("line {idx}\n"));
    }
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some(source.into()))),
    )
    .expect("seed long text");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    let cursor_char = ctx.editor.document().text().line_to_char(79);
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::single(cursor_char, cursor_char));

    let editor_pane = ctx.editor.active_pane_id();
    toggle_file_tree(&mut ctx);
    assert!(ctx.editor.is_active_pane_client_surface());
    assert_eq!(ctx.visible_editor_pane_for_viewport(), Some(editor_pane));

    let before = ctx
      .editor
      .pane_view(editor_pane)
      .expect("editor pane view")
      .scroll
      .row;
    ensure_cursor_visible(&mut ctx);
    let after = ctx
      .editor
      .pane_view(editor_pane)
      .expect("editor pane view")
      .scroll
      .row;

    assert!(after > before);
  }

  #[test]
  fn toggle_file_tree_adds_left_edge_sidebar_without_replacing_existing_splits() {
    let mut ctx = Ctx::new(None).expect("ctx");
    let view = ctx.editor.view();
    ctx.editor.open_buffer(Rope::from("two\n"), view, None);
    assert!(ctx.editor.split_active_pane(SplitAxis::Vertical));

    let right_pane = ctx.editor.active_pane_id();
    let left_pane = ctx
      .editor
      .pane_in_direction(right_pane, the_lib::split_tree::PaneDirection::Left)
      .expect("left pane");
    let pane_count_before = ctx.editor.pane_count();

    assert_eq!(
      ctx.editor.pane_content_kind(left_pane),
      Some(the_lib::editor::PaneContentKind::EditorBuffer)
    );
    assert_eq!(
      ctx.editor.pane_content_kind(right_pane),
      Some(the_lib::editor::PaneContentKind::EditorBuffer)
    );

    toggle_file_tree(&mut ctx);

    assert_eq!(ctx.editor.pane_count(), pane_count_before + 1);
    assert_eq!(
      ctx.editor.pane_content_kind(left_pane),
      Some(the_lib::editor::PaneContentKind::EditorBuffer)
    );
    assert_eq!(
      ctx.editor.pane_content_kind(right_pane),
      Some(the_lib::editor::PaneContentKind::EditorBuffer)
    );

    let tree_pane = ctx.file_tree.sidebar_pane.expect("tree sidebar pane");
    assert_ne!(tree_pane, left_pane);
    assert_ne!(tree_pane, right_pane);
    assert_eq!(
      ctx
        .editor
        .pane_in_direction(tree_pane, the_lib::split_tree::PaneDirection::Left),
      None
    );
    assert_eq!(
      ctx
        .editor
        .pane_in_direction(tree_pane, the_lib::split_tree::PaneDirection::Right),
      Some(left_pane)
    );
    assert_eq!(
      ctx.editor.pane_content_kind(tree_pane),
      Some(the_lib::editor::PaneContentKind::ClientSurface)
    );

    toggle_file_tree(&mut ctx);

    assert_eq!(ctx.editor.pane_count(), pane_count_before);
    assert_eq!(
      ctx.editor.pane_content_kind(left_pane),
      Some(the_lib::editor::PaneContentKind::EditorBuffer)
    );
    assert_eq!(
      ctx.editor.pane_content_kind(right_pane),
      Some(the_lib::editor::PaneContentKind::EditorBuffer)
    );
  }

  #[test]
  fn file_tree_visible_rows_clamp_stale_scroll_offset() {
    let mut ctx = Ctx::new(None).expect("ctx");
    toggle_file_tree(&mut ctx);

    set_file_tree_visible_rows(&mut ctx, 3);
    assert!(scroll_file_tree(&mut ctx, 999, 3));
    let before = ctx.file_tree.scroll_offset;
    assert!(before > 0);
    let large_visible_rows = ctx.file_tree.rows.len().saturating_add(8);

    set_file_tree_visible_rows(&mut ctx, large_visible_rows);

    assert!(ctx.file_tree.scroll_offset < before);
    assert_eq!(ctx.file_tree.scroll_offset, 0);
  }

  #[test]
  fn terminal_focus_reset_clears_pending_keymap_state() {
    let mut ctx = Ctx::new(None).expect("ctx");

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(' '),
      modifiers: Modifiers::empty(),
    });
    assert!(!ctx.keymaps.pending().is_empty());

    ctx.handle_terminal_focus_lost();

    assert!(ctx.keymaps.pending().is_empty());
    assert!(ctx.completion_menu_keymaps.pending().is_empty());
    assert!(ctx.pending_input.is_none());
    assert!(ctx.needs_render);
    assert!(matches!(ctx.term_cursor_mode, TermCursorMode::Hidden));
  }

  #[test]
  fn soft_wrap_scroll_clamps_to_last_visible_visual_row() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.resize(14, 3);
    DefaultContext::set_soft_wrap_enabled(&mut ctx, true);

    let long_line = "aaaa bbbb cccc dddd eeee ffff gggg";
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some(long_line.into()))),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    assert!(ctx.set_active_view_scroll_clamped(Position::new(99, 7)));

    let view = ctx.editor.view();
    let doc = ctx.editor.document();
    let mut text_format = <Ctx as DefaultContext>::text_format(&ctx);
    let gutter_width =
      the_lib::render::gutter_width_for_document(doc, view.viewport.width, &ctx.gutter_config);
    text_format.viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1);
    let mut annotations = <Ctx as DefaultContext>::text_annotations(&ctx);
    let eof_pos = the_lib::render::visual_pos_at_char(
      doc.text().slice(..),
      &text_format,
      &mut annotations,
      doc.text().len_chars(),
    )
    .expect("eof visual position");
    let expected =
      the_lib::view::max_scroll_row_for_content(eof_pos.row, view.viewport.height as usize);

    assert_eq!(view.scroll.col, 0);
    assert_eq!(view.scroll.row, expected);
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
      .active_managed_lsp_runtime()
      .and_then(|runtime| runtime.runtime.config().server())
      .map(|server| server.name().to_string());
    assert_eq!(server_name.as_deref(), Some("rust-analyzer"));
  }

  #[test]
  fn set_file_path_attaches_runtime_without_replacing_registry_state() {
    let rust_fixture = TempTestFile::with_extension("lsp-running-reconfig", "rs", "fn main() {}\n");
    let mut ctx = Ctx::new(None).expect("ctx");
    assert!(ctx.lsp_runtimes.is_empty());

    <Ctx as DefaultContext>::set_file_path(&mut ctx, Some(rust_fixture.as_path().to_path_buf()));

    let runtime = ctx.active_managed_lsp_runtime().expect("active runtime");
    let server_name = runtime
      .runtime
      .config()
      .server()
      .map(|server| server.name().to_string());
    assert_eq!(server_name.as_deref(), Some("rust-analyzer"));
    assert_eq!(ctx.lsp_runtimes.len(), 1);
    assert!(!runtime.runtime.is_running());
  }

  #[test]
  fn switching_between_languages_reuses_existing_runtime_registry_entries() {
    let rust = TempTestFile::with_extension("lsp-runtime-rust", "rs", "fn main() {}\n");
    let cargo = rust
      .as_path()
      .parent()
      .expect("temp parent")
      .join("Cargo.toml");
    fs::write(
      &cargo,
      "[package]\nname = \"runtime-reuse\"\nversion = \"0.1.0\"\n",
    )
    .expect("write cargo");
    let c_file =
      TempTestFile::with_extension("lsp-runtime-c", "c", "int main(void) { return 0; }\n");

    let mut ctx = Ctx::new(Some(
      rust.as_path().to_str().expect("temp path should be utf-8"),
    ))
    .expect("ctx");

    let rust_runtime_id = ctx.active_lsp_runtime_id.expect("rust runtime");
    assert_eq!(ctx.lsp_runtimes.len(), 1);

    <Ctx as DefaultContext>::open_file(&mut ctx, c_file.as_path()).expect("open c file");
    let c_runtime_id = ctx.active_lsp_runtime_id.expect("c runtime");
    assert_ne!(c_runtime_id, rust_runtime_id);
    assert_eq!(ctx.lsp_runtimes.len(), 2);

    assert!(<Ctx as DefaultContext>::goto_buffer(
      &mut ctx,
      the_default::Direction::Backward,
      1,
    ));
    assert_eq!(ctx.active_lsp_runtime_id, Some(rust_runtime_id));
    assert_eq!(ctx.lsp_runtimes.len(), 2);
  }

  #[test]
  fn goto_buffer_cycles_open_files() {
    let first = TempTestFile::new("buffer-cycle-one", "one\n");
    let second = TempTestFile::new("buffer-cycle-two", "two\n");
    let mut ctx = Ctx::new(Some(
      first
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    <Ctx as DefaultContext>::open_file(&mut ctx, second.as_path()).expect("open second buffer");
    assert_eq!(ctx.file_path.as_deref(), Some(second.as_path()));
    assert_eq!(ctx.editor.document().text().to_string(), "two\n");

    assert!(<Ctx as DefaultContext>::goto_buffer(
      &mut ctx,
      the_default::Direction::Backward,
      1,
    ));
    assert_eq!(ctx.file_path.as_deref(), Some(first.as_path()));
    assert_eq!(ctx.editor.document().text().to_string(), "one\n");

    assert!(<Ctx as DefaultContext>::goto_buffer(
      &mut ctx,
      the_default::Direction::Forward,
      1,
    ));
    assert_eq!(ctx.file_path.as_deref(), Some(second.as_path()));
    assert_eq!(ctx.editor.document().text().to_string(), "two\n");
  }

  #[test]
  fn goto_buffer_keymap_sequence_cycles_open_files() {
    let first = TempTestFile::new("buffer-key-cycle-one", "one\n");
    let second = TempTestFile::new("buffer-key-cycle-two", "two\n");
    let mut ctx = Ctx::new(Some(
      first
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");
    <Ctx as DefaultContext>::open_file(&mut ctx, second.as_path()).expect("open second buffer");

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('g'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('n'),
      modifiers: Modifiers::empty(),
    });
    assert_eq!(ctx.file_path.as_deref(), Some(first.as_path()));
    assert_eq!(ctx.editor.document().text().to_string(), "one\n");

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('g'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('p'),
      modifiers: Modifiers::empty(),
    });
    assert_eq!(ctx.file_path.as_deref(), Some(second.as_path()));
    assert_eq!(ctx.editor.document().text().to_string(), "two\n");
  }

  #[test]
  fn lsp_goto_variant_keymaps_emit_errors_when_unavailable() {
    let mut ctx = Ctx::new(None).expect("ctx");

    for (suffix, expected) in [
      ('D', "No declaration found."),
      ('y', "No type definition found."),
      ('i', "No implementation found."),
    ] {
      let before_seq = ctx.messages.latest_seq();
      handle_key(&mut ctx, KeyEvent {
        key:       Key::Char('g'),
        modifiers: Modifiers::empty(),
      });
      handle_key(&mut ctx, KeyEvent {
        key:       Key::Char(suffix),
        modifiers: Modifiers::empty(),
      });

      let events = ctx.messages.events_since(before_seq);
      let error = events
        .iter()
        .find_map(|event| {
          match &event.kind {
            the_lib::messages::MessageEventKind::Published { message } => {
              (message.level == the_lib::messages::MessageLevel::Error
                && message.source.as_deref() == Some("goto"))
              .then_some(message.text.as_str())
            },
            _ => None,
          }
        })
        .expect("goto error message");
      assert_eq!(error, expected, "unexpected error: {error}");
    }
  }

  #[test]
  fn goto_last_accessed_file_keymap_sequence_toggles_between_buffers() {
    let first = TempTestFile::new("buffer-key-access-one", "one\n");
    let second = TempTestFile::new("buffer-key-access-two", "two\n");
    let mut ctx = Ctx::new(Some(
      first
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");
    <Ctx as DefaultContext>::open_file(&mut ctx, second.as_path()).expect("open second buffer");

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('g'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('a'),
      modifiers: Modifiers::empty(),
    });
    assert_eq!(ctx.file_path.as_deref(), Some(first.as_path()));
    assert_eq!(ctx.editor.document().text().to_string(), "one\n");

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('g'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('a'),
      modifiers: Modifiers::empty(),
    });
    assert_eq!(ctx.file_path.as_deref(), Some(second.as_path()));
    assert_eq!(ctx.editor.document().text().to_string(), "two\n");
  }

  #[test]
  fn goto_last_modified_file_keymap_sequence_uses_recent_edit_order() {
    let first = TempTestFile::new("buffer-key-modified-one", "one\n");
    let second = TempTestFile::new("buffer-key-modified-two", "two\n");
    let mut ctx = Ctx::new(Some(
      first
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    let first_edit = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("first-edit ".into()))),
    )
    .expect("first edit transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &first_edit));

    <Ctx as DefaultContext>::open_file(&mut ctx, second.as_path()).expect("open second buffer");
    let second_edit = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("second-edit ".into()))),
    )
    .expect("second edit transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &second_edit));

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('g'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('m'),
      modifiers: Modifiers::empty(),
    });
    assert_eq!(ctx.file_path.as_deref(), Some(first.as_path()));
    assert!(
      ctx
        .editor
        .document()
        .text()
        .to_string()
        .starts_with("first-edit ")
    );

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('g'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('m'),
      modifiers: Modifiers::empty(),
    });
    assert_eq!(ctx.file_path.as_deref(), Some(second.as_path()));
    assert!(
      ctx
        .editor
        .document()
        .text()
        .to_string()
        .starts_with("second-edit ")
    );
  }

  #[test]
  fn jumplist_keymap_sequence_saves_and_navigates_selections() {
    let mut ctx = Ctx::new(None).expect("ctx");

    let _ = ctx.editor.document_mut().set_selection(Selection::point(0));
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('s'),
      modifiers: ctrl_modifiers(),
    });

    let _ = ctx.editor.document_mut().set_selection(Selection::point(3));
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('s'),
      modifiers: ctrl_modifiers(),
    });

    let _ = ctx.editor.document_mut().set_selection(Selection::point(6));
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('o'),
      modifiers: ctrl_modifiers(),
    });
    assert_eq!(
      ctx.editor.document().selection().ranges()[0],
      Range::point(3)
    );

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('o'),
      modifiers: ctrl_modifiers(),
    });
    assert_eq!(
      ctx.editor.document().selection().ranges()[0],
      Range::point(0)
    );

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('i'),
      modifiers: ctrl_modifiers(),
    });
    assert_eq!(
      ctx.editor.document().selection().ranges()[0],
      Range::point(3)
    );

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('i'),
      modifiers: ctrl_modifiers(),
    });
    assert_eq!(
      ctx.editor.document().selection().ranges()[0],
      Range::point(6)
    );
  }

  #[test]
  fn goto_motion_keymaps_save_jumps_for_file_edges_and_column() {
    let mut ctx = Ctx::new(None).expect("ctx");

    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("alpha\nbeta\ngamma\n".into()))),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    let set_cursor_at = |ctx: &mut Ctx, row: usize, col: usize| {
      let pos = {
        let text = ctx.editor.document().text().slice(..);
        char_idx_at_coords(text, Position::new(row, col))
      };
      let _ = ctx
        .editor
        .document_mut()
        .set_selection(Selection::point(pos));
      pos
    };

    let go_back = |ctx: &mut Ctx| {
      handle_key(ctx, KeyEvent {
        key:       Key::Char('o'),
        modifiers: ctrl_modifiers(),
      });
    };

    let press = |ctx: &mut Ctx, ch: char| {
      handle_key(ctx, KeyEvent {
        key:       Key::Char(ch),
        modifiers: Modifiers::empty(),
      });
    };

    let ge_origin = set_cursor_at(&mut ctx, 1, 1);
    press(&mut ctx, 'g');
    press(&mut ctx, 'e');
    go_back(&mut ctx);
    assert_eq!(
      ctx.editor.document().selection().ranges()[0],
      Range::point(ge_origin)
    );

    let gg_origin = set_cursor_at(&mut ctx, 2, 2);
    press(&mut ctx, 'g');
    press(&mut ctx, 'g');
    go_back(&mut ctx);
    assert_eq!(
      ctx.editor.document().selection().ranges()[0],
      Range::point(gg_origin)
    );

    let gbar_origin = set_cursor_at(&mut ctx, 1, 3);
    press(&mut ctx, 'g');
    press(&mut ctx, '|');
    go_back(&mut ctx);
    assert_eq!(
      ctx.editor.document().selection().ranges()[0],
      Range::point(gbar_origin)
    );
  }

  #[test]
  fn lsp_jump_saves_origin_for_jumplist_back_navigation() {
    let first = TempTestFile::new("lsp-jump-origin", "first file\n");
    let second = TempTestFile::new("lsp-jump-target", "second file\n");
    let mut ctx = Ctx::new(Some(
      first
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    let origin = 3;
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::point(origin));

    let uri = the_lsp::text_sync::file_uri_for_path(second.as_path()).expect("file uri");
    assert!(ctx.jump_to_location(&LspLocation {
      uri,
      range: LspRange {
        start: LspPosition {
          line:      0,
          character: 0,
        },
        end:   LspPosition {
          line:      0,
          character: 0,
        },
      },
    }));
    assert_eq!(ctx.file_path.as_deref(), Some(second.as_path()));

    assert!(ctx.jump_backward_in_jumplist(1));
    assert_eq!(ctx.file_path.as_deref(), Some(first.as_path()));
    assert_eq!(
      ctx.editor.document().selection().ranges()[0],
      Range::point(origin)
    );
  }

  #[test]
  fn lsp_symbols_result_opens_picker_instead_of_jumping() {
    let fixture =
      TempTestFile::with_extension("lsp-symbol-picker", "rs", "fn alpha() {}\nfn beta() {}\n");
    let mut ctx = Ctx::new(Some(
      fixture
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    let _ = ctx.editor.document_mut().set_selection(Selection::point(0));
    let before_path = ctx.file_path.clone();
    let before_selection = ctx.editor.document().selection().ranges()[0];
    let uri = the_lsp::text_sync::file_uri_for_path(fixture.as_path()).expect("file uri");

    let symbols = vec![LspSymbol {
      name:           "beta".to_string(),
      detail:         None,
      kind:           12,
      container_name: Some("crate".to_string()),
      location:       Some(LspLocation {
        uri,
        range: LspRange {
          start: LspPosition {
            line:      1,
            character: 3,
          },
          end:   LspPosition {
            line:      1,
            character: 7,
          },
        },
      }),
    }];

    assert!(ctx.apply_symbols_result("document symbols", symbols));
    assert!(ctx.file_picker.active);
    assert_eq!(ctx.file_picker.selected, Some(0));
    assert_eq!(ctx.file_picker.matched_count(), 1);
    assert!(ctx.file_picker.title.starts_with("Lsp Symbols"));
    assert_eq!(ctx.file_path, before_path);
    assert_eq!(
      ctx.editor.document().selection().ranges()[0],
      before_selection
    );
  }

  #[test]
  fn toggle_comments_keymap_sequence_comments_current_line() {
    let fixture = TempTestFile::with_extension("toggle-comments", "rs", "fn main() {}\n");
    let mut ctx = Ctx::new(Some(
      fixture
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    let _ = ctx.editor.document_mut().set_selection(Selection::point(0));
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('c'),
      modifiers: ctrl_modifiers(),
    });
    assert_eq!(
      ctx.editor.document().text().to_string(),
      "// fn main() {}\n"
    );

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('c'),
      modifiers: ctrl_modifiers(),
    });
    assert_eq!(ctx.editor.document().text().to_string(), "fn main() {}\n");
  }

  #[test]
  fn goto_window_keymap_sequence_moves_cursor_to_window_alignments() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.resize(80, 24);

    let mut content = String::new();
    for line in 0..96usize {
      content.push_str(&format!("line-{line}\n"));
    }
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((
        0,
        ctx.editor.document().text().len_chars(),
        Some(content.as_str().into()),
      )),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));
    ctx.editor.view_mut().scroll = Position::new(10, 0);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('g'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('t'),
      modifiers: Modifiers::empty(),
    });
    let top_row = {
      let text = ctx.editor.document().text().slice(..);
      let head = ctx.editor.document().selection().ranges()[0].head;
      coords_at_pos(text, head).row
    };
    assert_eq!(top_row, 15);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('g'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('c'),
      modifiers: Modifiers::empty(),
    });
    let center_row = {
      let text = ctx.editor.document().text().slice(..);
      let head = ctx.editor.document().selection().ranges()[0].head;
      coords_at_pos(text, head).row
    };
    assert_eq!(center_row, 21);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('g'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('b'),
      modifiers: Modifiers::empty(),
    });
    let bottom_row = {
      let text = ctx.editor.document().text().slice(..);
      let head = ctx.editor.document().selection().ranges()[0].head;
      coords_at_pos(text, head).row
    };
    assert_eq!(bottom_row, 28);
  }

  #[test]
  fn goto_last_modification_keymap_sequence_moves_cursor_to_last_edit() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.resize(80, 24);

    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("edited ".into()))),
    )
    .expect("edit transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));
    let _ = ctx.editor.document_mut().commit();

    let end = ctx.editor.document().text().len_chars();
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::point(end));
    let expected = ctx
      .editor
      .last_modification_position()
      .expect("last modification position");

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('g'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('.'),
      modifiers: Modifiers::empty(),
    });

    let actual = ctx.editor.document().selection().ranges()[0].head;
    assert_eq!(actual, expected);
  }

  #[test]
  fn copy_selection_on_next_line_keeps_single_line_height_at_line_start() {
    let mut ctx = Ctx::new(None).expect("ctx");

    let content = "zero\none\ntwo\nthree\n";
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((
        0,
        ctx.editor.document().text().len_chars(),
        Some(content.into()),
      )),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    let line_start = ctx.editor.document().text().line_to_char(1);
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::point(line_start));

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('C'),
      modifiers: Modifiers::empty(),
    });

    let text = ctx.editor.document().text().slice(..);
    let rows: Vec<_> = ctx
      .editor
      .document()
      .selection()
      .ranges()
      .iter()
      .map(|range| coords_at_pos(text, range.cursor(text)).row)
      .collect();
    assert_eq!(rows, vec![1, 2]);
  }

  #[test]
  fn goto_word_keymap_sequence_moves_cursor_using_jump_labels() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.resize(80, 24);

    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((
        0,
        ctx.editor.document().text().len_chars(),
        Some("alpha bravo charlie delta\n".into()),
      )),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));
    let _ = ctx.editor.document_mut().set_selection(Selection::point(0));

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('g'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('w'),
      modifiers: Modifiers::empty(),
    });
    let targets = match ctx.pending_input().cloned() {
      Some(PendingInput::WordJump { targets, .. }) => targets,
      _ => panic!("expected word jump pending input"),
    };
    assert!(matches!(
      ctx.pending_input(),
      Some(PendingInput::WordJump {
        first: None,
        targets,
        ..
      }) if targets.len() >= 2
    ));
    assert!(ctx.word_jump_inline_annotations.is_empty());
    assert!(!ctx.word_jump_overlay_annotations.is_empty());

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('a'),
      modifiers: Modifiers::empty(),
    });
    assert!(matches!(
      ctx.pending_input(),
      Some(PendingInput::WordJump {
        first: Some(0),
        targets,
        ..
      }) if targets.len() >= 2
    ));
    assert!(ctx.word_jump_inline_annotations.is_empty());
    assert!(!ctx.word_jump_overlay_annotations.is_empty());

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('b'),
      modifiers: Modifiers::empty(),
    });

    assert!(ctx.pending_input().is_none());
    assert!(ctx.word_jump_inline_annotations.is_empty());
    assert!(ctx.word_jump_overlay_annotations.is_empty());
    let expected = targets
      .get(1)
      .expect("expected at least two jump targets")
      .range
      .with_direction(SelectionDirection::Forward);
    assert_eq!(ctx.editor.document().selection().ranges()[0], expected);
  }

  #[test]
  fn extend_to_word_keymap_sequence_extends_selection_using_jump_labels() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.resize(80, 24);

    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((
        0,
        ctx.editor.document().text().len_chars(),
        Some("alpha bravo charlie delta\n".into()),
      )),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));
    let _ = ctx.editor.document_mut().set_selection(Selection::point(0));
    ctx.set_mode(Mode::Select);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('g'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('w'),
      modifiers: Modifiers::empty(),
    });
    let targets = match ctx.pending_input().cloned() {
      Some(PendingInput::WordJump { targets, .. }) => targets,
      _ => panic!("expected word jump pending input"),
    };
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('a'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('b'),
      modifiers: Modifiers::empty(),
    });

    let target = targets
      .get(1)
      .expect("expected at least two jump targets")
      .range;
    let expected = if target.anchor < target.head {
      Range::new(0, target.head)
    } else {
      Range::new(target.anchor.max(0), target.head)
    };
    assert_eq!(ctx.editor.document().selection().ranges()[0], expected);
  }

  #[test]
  fn split_selection_keymap_sequence_uses_split_prompt_and_partitions_selection() {
    let mut ctx = Ctx::new(None).expect("ctx");

    let content = "alpha,beta,gamma\n";
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((
        0,
        ctx.editor.document().text().len_chars(),
        Some(content.into()),
      )),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));
    let split_end = content.trim_end_matches('\n').chars().count();
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::single(0, split_end));

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('S'),
      modifiers: Modifiers::empty(),
    });
    assert!(ctx.search_prompt.active);
    assert_eq!(ctx.search_prompt.kind, SearchPromptKind::SplitSelection);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(','),
      modifiers: Modifiers::empty(),
    });
    assert_eq!(ctx.editor.document().selection().ranges().len(), 3);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Enter,
      modifiers: Modifiers::empty(),
    });
    assert!(!ctx.search_prompt.active);
    assert_eq!(ctx.editor.document().selection().ranges().len(), 3);
  }

  #[test]
  fn join_selections_keymap_sequence_joins_lines() {
    let mut ctx = Ctx::new(None).expect("ctx");

    let content = "alpha\nbeta\ngamma\n";
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((
        0,
        ctx.editor.document().text().len_chars(),
        Some(content.into()),
      )),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));
    let join_end = content.trim_end_matches('\n').chars().count();
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::single(0, join_end));

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('J'),
      modifiers: Modifiers::empty(),
    });

    assert_eq!(
      ctx.editor.document().text().to_string(),
      "alpha beta gamma\n"
    );
  }

  #[test]
  fn join_selections_space_keymap_sequence_selects_inserted_space() {
    let mut ctx = Ctx::new(None).expect("ctx");

    let content = "alpha\nbeta\n";
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((
        0,
        ctx.editor.document().text().len_chars(),
        Some(content.into()),
      )),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));
    let join_end = content.trim_end_matches('\n').chars().count();
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::single(0, join_end));

    let mut alt = Modifiers::empty();
    alt.insert(Modifiers::ALT);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('J'),
      modifiers: alt,
    });

    assert_eq!(ctx.editor.document().text().to_string(), "alpha beta\n");
    assert_eq!(ctx.editor.document().selection().ranges(), &[Range::point(
      "alpha".chars().count()
    )]);
  }

  #[test]
  fn keep_selections_keymap_sequence_filters_selection_with_prompt() {
    let mut ctx = Ctx::new(None).expect("ctx");

    let content = "one two three\n";
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((
        0,
        ctx.editor.document().text().len_chars(),
        Some(content.into()),
      )),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));
    let select_end = content.trim_end_matches('\n').chars().count();
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::single(0, select_end));
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('S'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(' '),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Enter,
      modifiers: Modifiers::empty(),
    });
    assert_eq!(ctx.editor.document().selection().ranges().len(), 3);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('K'),
      modifiers: Modifiers::empty(),
    });
    assert!(ctx.search_prompt.active);
    assert_eq!(ctx.search_prompt.kind, SearchPromptKind::KeepSelections);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('o'),
      modifiers: Modifiers::empty(),
    });

    let text = ctx.editor.document().text().slice(..);
    let fragments: Vec<_> = ctx
      .editor
      .document()
      .selection()
      .fragments(text)
      .map(|fragment| fragment.into_owned())
      .collect();
    assert_eq!(fragments, vec!["one".to_string(), "two".to_string()]);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Enter,
      modifiers: Modifiers::empty(),
    });
    assert!(!ctx.search_prompt.active);
    assert_eq!(ctx.editor.document().selection().ranges().len(), 2);
  }

  #[test]
  fn remove_selections_keymap_sequence_filters_selection_with_prompt() {
    let mut ctx = Ctx::new(None).expect("ctx");

    let content = "one two three\n";
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((
        0,
        ctx.editor.document().text().len_chars(),
        Some(content.into()),
      )),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));
    let select_end = content.trim_end_matches('\n').chars().count();
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::single(0, select_end));
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('S'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(' '),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Enter,
      modifiers: Modifiers::empty(),
    });
    assert_eq!(ctx.editor.document().selection().ranges().len(), 3);

    let mut alt = Modifiers::empty();
    alt.insert(Modifiers::ALT);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('K'),
      modifiers: alt,
    });
    assert!(ctx.search_prompt.active);
    assert_eq!(ctx.search_prompt.kind, SearchPromptKind::RemoveSelections);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('o'),
      modifiers: Modifiers::empty(),
    });

    let text = ctx.editor.document().text().slice(..);
    let fragments: Vec<_> = ctx
      .editor
      .document()
      .selection()
      .fragments(text)
      .map(|fragment| fragment.into_owned())
      .collect();
    assert_eq!(fragments, vec!["three".to_string()]);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Enter,
      modifiers: Modifiers::empty(),
    });
    assert!(!ctx.search_prompt.active);
    assert_eq!(ctx.editor.document().selection().ranges().len(), 1);
  }

  #[test]
  fn clipboard_yank_keymaps_write_to_system_register() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx
      .registers
      .set_clipboard_provider(std::sync::Arc::new(NoClipboard));

    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((
        0,
        ctx.editor.document().text().len_chars(),
        Some("alpha beta\n".into()),
      )),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    let selection = Selection::single(0, 5).push(Range::new(6, 10));
    let _ = ctx.editor.document_mut().set_selection(selection);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(' '),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('y'),
      modifiers: Modifiers::empty(),
    });

    let values: Vec<_> = ctx
      .registers
      .read('+', ctx.editor.document())
      .expect("clipboard register")
      .map(|value| value.into_owned())
      .collect();
    assert_eq!(values, vec!["alpha".to_string(), "beta".to_string()]);

    let selection = Selection::single(0, 5).push(Range::new(6, 10));
    let _ = ctx.editor.document_mut().set_selection(selection);
    let second = ctx.editor.document().selection().cursor_ids()[1];
    ctx.editor.view_mut().active_cursor = Some(second);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(' '),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('Y'),
      modifiers: Modifiers::empty(),
    });

    let values: Vec<_> = ctx
      .registers
      .read('+', ctx.editor.document())
      .expect("clipboard register")
      .map(|value| value.into_owned())
      .collect();
    assert_eq!(values, vec!["beta".to_string()]);
  }

  #[test]
  fn clipboard_paste_and_replace_keymaps_use_system_register() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx
      .registers
      .set_clipboard_provider(std::sync::Arc::new(NoClipboard));
    let _ = ctx.registers.write('+', vec!["Z".to_string()]);

    let reset_text = |ctx: &mut Ctx| {
      let tx = Transaction::change(
        ctx.editor.document().text(),
        std::iter::once((
          0,
          ctx.editor.document().text().len_chars(),
          Some("abc\n".into()),
        )),
      )
      .expect("seed transaction");
      assert!(DefaultContext::apply_transaction(ctx, &tx));
    };
    let press = |ctx: &mut Ctx, ch: char| {
      handle_key(ctx, KeyEvent {
        key:       Key::Char(' '),
        modifiers: Modifiers::empty(),
      });
      handle_key(ctx, KeyEvent {
        key:       Key::Char(ch),
        modifiers: Modifiers::empty(),
      });
    };

    reset_text(&mut ctx);
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::single(0, 1));
    press(&mut ctx, 'p');
    assert_eq!(ctx.editor.document().text().to_string(), "aZbc\n");

    reset_text(&mut ctx);
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::single(0, 1));
    press(&mut ctx, 'P');
    assert_eq!(ctx.editor.document().text().to_string(), "Zabc\n");

    reset_text(&mut ctx);
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::single(1, 2));
    press(&mut ctx, 'R');
    assert_eq!(ctx.editor.document().text().to_string(), "aZc\n");
  }

  #[test]
  fn keep_active_selection_keymap_sequence_collapses_to_picked_cursor() {
    let mut ctx = Ctx::new(None).expect("ctx");

    let content = "a\nb\nc\n";
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((
        0,
        ctx.editor.document().text().len_chars(),
        Some(content.into()),
      )),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    let text = ctx.editor.document().text().clone();
    let selection = Selection::point(text.line_to_char(0))
      .push(Range::point(text.line_to_char(1)))
      .push(Range::point(text.line_to_char(2)));
    let _ = ctx.editor.document_mut().set_selection(selection);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(','),
      modifiers: Modifiers::empty(),
    });
    let candidates = match ctx.pending_input().cloned() {
      Some(PendingInput::CursorPick {
        remove,
        candidates,
        index,
        ..
      }) => {
        assert!(!remove);
        assert_eq!(index, 0);
        candidates
      },
      _ => panic!("expected cursor-pick pending input"),
    };

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Down,
      modifiers: Modifiers::empty(),
    });
    assert!(matches!(
      ctx.pending_input(),
      Some(PendingInput::CursorPick {
        remove: false,
        index: 1,
        ..
      })
    ));
    assert_eq!(ctx.editor.view().active_cursor, Some(candidates[1]));

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Enter,
      modifiers: Modifiers::empty(),
    });

    assert!(ctx.pending_input().is_none());
    assert_eq!(ctx.editor.document().selection().ranges().len(), 1);
    assert_eq!(
      ctx.editor.document().selection().cursor_ids()[0],
      candidates[1]
    );
    assert_eq!(ctx.editor.view().active_cursor, Some(candidates[1]));
  }

  #[test]
  fn cursor_pick_mode_uses_match_cursor_style_for_selected_cursor() {
    let mut ctx = Ctx::new(None).expect("ctx");

    let content = "a\nb\nc\n";
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((
        0,
        ctx.editor.document().text().len_chars(),
        Some(content.into()),
      )),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    let text = ctx.editor.document().text().clone();
    let selection = Selection::point(text.line_to_char(0))
      .push(Range::point(text.line_to_char(1)))
      .push(Range::point(text.line_to_char(2)));
    let _ = ctx.editor.document_mut().set_selection(selection);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(','),
      modifiers: Modifiers::empty(),
    });
    let active_cursor = ctx
      .editor
      .view()
      .active_cursor
      .expect("active cursor in cursor-pick mode");

    let plan = build_render_plan(&mut ctx);
    let selected = plan
      .cursors
      .iter()
      .find(|cursor| cursor.id == active_cursor)
      .expect("selected cursor should be rendered");
    let expected = ctx
      .ui_theme
      .try_get("ui.cursor.match")
      .or_else(|| ctx.ui_theme.try_get("ui.cursor.active"))
      .or_else(|| ctx.ui_theme.try_get("ui.cursor"))
      .unwrap_or_default();
    assert_eq!(selected.style, expected);
    assert!(
      plan
        .cursors
        .iter()
        .any(|cursor| cursor.id != active_cursor && cursor.style != selected.style),
      "selected cursor style should stand out from non-selected cursors"
    );
  }

  #[test]
  fn remove_active_selection_keymap_sequence_removes_picked_cursor() {
    let mut ctx = Ctx::new(None).expect("ctx");

    let content = "a\nb\nc\n";
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((
        0,
        ctx.editor.document().text().len_chars(),
        Some(content.into()),
      )),
    )
    .expect("seed transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    let text = ctx.editor.document().text().clone();
    let selection = Selection::point(text.line_to_char(0))
      .push(Range::point(text.line_to_char(1)))
      .push(Range::point(text.line_to_char(2)));
    let _ = ctx.editor.document_mut().set_selection(selection);

    let mut alt = Modifiers::empty();
    alt.insert(Modifiers::ALT);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(','),
      modifiers: alt,
    });
    let candidates = match ctx.pending_input().cloned() {
      Some(PendingInput::CursorPick {
        remove,
        candidates,
        index,
        ..
      }) => {
        assert!(remove);
        assert_eq!(index, 0);
        candidates
      },
      _ => panic!("expected cursor-pick pending input"),
    };

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Down,
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Enter,
      modifiers: Modifiers::empty(),
    });

    assert!(ctx.pending_input().is_none());
    assert_eq!(ctx.editor.document().selection().ranges().len(), 2);
    assert!(
      !ctx
        .editor
        .document()
        .selection()
        .cursor_ids()
        .contains(&candidates[1])
    );
  }

  #[test]
  fn goto_buffer_restores_syntax_for_target_buffer() {
    let mut ctx = Ctx::new(None).expect("ctx");
    if ctx.loader.is_none() {
      return;
    }

    let rust = TempTestFile::with_extension("buffer-syntax", "rs", "fn main() {}\n");
    let txt = TempTestFile::with_extension("buffer-syntax", "txt", "plain text\n");

    <Ctx as DefaultContext>::open_file(&mut ctx, rust.as_path()).expect("open rust");
    let rust_has_syntax = ctx.editor.document().syntax().is_some();
    if !rust_has_syntax {
      return;
    }

    <Ctx as DefaultContext>::open_file(&mut ctx, txt.as_path()).expect("open txt");
    assert_eq!(ctx.file_path.as_deref(), Some(txt.as_path()));

    assert!(<Ctx as DefaultContext>::goto_buffer(
      &mut ctx,
      the_default::Direction::Backward,
      1,
    ));
    assert_eq!(ctx.file_path.as_deref(), Some(rust.as_path()));
    assert!(ctx.editor.document().syntax().is_some());
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
    let mut ctx = Ctx::new(None).expect("ctx");

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

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('x'),
      modifiers: Modifiers::empty(),
    });

    let selected = ctx.editor.document().selection().ranges()[0];
    assert_eq!(selected.from(), line_two_start);
    assert_eq!(selected.to(), ctx.editor.document().text().line_to_char(2));

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('c'),
      modifiers: Modifiers::empty(),
    });

    assert_eq!(ctx.editor.document().text().to_string(), "one\n\nthree\n");
    assert_eq!(ctx.mode(), Mode::Insert);
  }

  #[test]
  fn command_palette_query_input_does_not_auto_select_item() {
    let mut ctx = Ctx::new(None).expect("ctx");

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(':'),
      modifiers: Modifiers::empty(),
    });
    assert_eq!(ctx.mode(), Mode::Command);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('w'),
      modifiers: Modifiers::empty(),
    });

    assert_eq!(ctx.command_palette.query, "w");
    assert_eq!(ctx.command_palette.selected, None);
  }

  #[test]
  fn command_palette_keeps_argument_mode_when_open_has_no_matches() {
    let mut ctx = Ctx::new(None).expect("ctx");

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(':'),
      modifiers: Modifiers::empty(),
    });
    for ch in "e definitely_missing_file_name_12345.c".chars() {
      handle_key(&mut ctx, KeyEvent {
        key:       Key::Char(ch),
        modifiers: Modifiers::empty(),
      });
    }

    assert!(ctx.command_palette.prefiltered);
    assert!(ctx.command_palette.items.is_empty());
    assert_eq!(ctx.command_palette.query, "");
    assert_eq!(
      ctx.command_palette.prompt_text.as_deref(),
      Some(":e definitely_missing_file_name_12345.c")
    );
  }

  #[test]
  fn command_palette_explicit_navigation_sets_selection() {
    let mut ctx = Ctx::new(None).expect("ctx");

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(':'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('w'),
      modifiers: Modifiers::empty(),
    });
    assert_eq!(ctx.command_palette.selected, None);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Down,
      modifiers: Modifiers::empty(),
    });

    assert!(ctx.command_palette.selected.is_some());
  }

  #[test]
  fn command_palette_enter_submits_typed_alias_without_selection() {
    let fixture = TempTestFile::new("command-palette-enter", "alpha\n");
    let mut ctx = Ctx::new(Some(
      fixture
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(':'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('w'),
      modifiers: Modifiers::empty(),
    });

    assert_eq!(ctx.mode(), Mode::Command);
    assert_eq!(ctx.command_palette.selected, None);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Enter,
      modifiers: Modifiers::empty(),
    });

    assert_eq!(ctx.mode(), Mode::Normal);
    assert!(!ctx.command_palette.is_open);
  }

  #[test]
  fn command_palette_enter_submits_selected_open_completion() {
    let fixture = TempTestFile::with_extension("command-open-completion", "toml", "toolchain\n");
    let mut ctx = Ctx::new(None).expect("ctx");

    let parent = fixture
      .as_path()
      .parent()
      .expect("temp test file should have a parent");
    let file_name = fixture
      .as_path()
      .file_name()
      .and_then(|name| name.to_str())
      .expect("temp test file name should be utf-8");
    let partial_len = file_name.len().saturating_sub(5).max(1);
    let partial = &file_name[..partial_len];

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(':'),
      modifiers: Modifiers::empty(),
    });
    for ch in format!("e {}/{}", parent.display(), partial).chars() {
      handle_key(&mut ctx, KeyEvent {
        key:       Key::Char(ch),
        modifiers: Modifiers::empty(),
      });
    }

    assert!(ctx.command_palette.prefiltered);
    assert!(!ctx.command_palette.items.is_empty());

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Enter,
      modifiers: Modifiers::empty(),
    });

    assert_eq!(ctx.mode(), Mode::Normal);
    assert_eq!(ctx.file_path(), Some(fixture.as_path()));
    assert_eq!(ctx.editor.document().text().to_string(), "toolchain\n");
  }

  #[test]
  fn command_palette_enter_submits_selected_theme_completion() {
    let mut ctx = Ctx::new(None).expect("ctx");

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(':'),
      modifiers: Modifiers::empty(),
    });
    for ch in "theme base16".chars() {
      handle_key(&mut ctx, KeyEvent {
        key:       Key::Char(ch),
        modifiers: Modifiers::empty(),
      });
    }

    assert!(ctx.command_palette.prefiltered);
    assert!(!ctx.command_palette.items.is_empty());
    assert_eq!(ctx.ui_theme_preview_name.as_deref(), Some("base16_default"));

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Enter,
      modifiers: Modifiers::empty(),
    });

    assert_eq!(ctx.mode(), Mode::Normal);
    assert_eq!(ctx.ui_theme_name(), "base16_default");
    assert_eq!(ctx.ui_theme_preview_name.as_deref(), None);
  }

  #[test]
  fn command_palette_enter_inserts_selected_command_name_for_arg_commands() {
    let mut ctx = Ctx::new(None).expect("ctx");

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(':'),
      modifiers: Modifiers::empty(),
    });
    for ch in "theme".chars() {
      handle_key(&mut ctx, KeyEvent {
        key:       Key::Char(ch),
        modifiers: Modifiers::empty(),
      });
    }
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Down,
      modifiers: Modifiers::empty(),
    });

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Enter,
      modifiers: Modifiers::empty(),
    });

    assert_eq!(ctx.mode(), Mode::Command);
    assert!(ctx.command_palette.is_open);
    assert_eq!(ctx.command_prompt.input, "theme ");
    assert!(ctx.command_palette.prefiltered);
    assert_eq!(ctx.command_palette.selected, None);
    assert!(!ctx.command_palette.items.is_empty());
  }

  #[test]
  fn command_palette_unrelated_input_does_not_revert_committed_theme() {
    let mut ctx = Ctx::new(None).expect("ctx");

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(':'),
      modifiers: Modifiers::empty(),
    });
    for ch in "theme base16".chars() {
      handle_key(&mut ctx, KeyEvent {
        key:       Key::Char(ch),
        modifiers: Modifiers::empty(),
      });
    }
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Enter,
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Enter,
      modifiers: Modifiers::empty(),
    });

    assert_eq!(ctx.ui_theme_name(), "base16_default");
    assert_eq!(ctx.ui_theme_preview_name.as_deref(), None);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(':'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('w'),
      modifiers: Modifiers::empty(),
    });

    assert_eq!(ctx.ui_theme_name(), "base16_default");
    assert_eq!(ctx.ui_theme_preview_name.as_deref(), None);
  }

  #[test]
  fn command_palette_enter_executes_selected_zero_arg_command() {
    let fixture = TempTestFile::new("command-palette-write", "alpha\n");
    let mut ctx = Ctx::new(Some(
      fixture
        .as_path()
        .to_str()
        .expect("temp test path should be utf-8"),
    ))
    .expect("ctx");

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(':'),
      modifiers: Modifiers::empty(),
    });
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Down,
      modifiers: Modifiers::empty(),
    });

    while ctx
      .command_palette
      .selected
      .and_then(|index| ctx.command_palette.items.get(index))
      .map(|item| item.title.as_str())
      != Some("write")
    {
      handle_key(&mut ctx, KeyEvent {
        key:       Key::Down,
        modifiers: Modifiers::empty(),
      });
    }

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Enter,
      modifiers: Modifiers::empty(),
    });

    assert_eq!(ctx.mode(), Mode::Normal);
    assert!(!ctx.command_palette.is_open);
  }

  #[test]
  fn command_open_creates_missing_file() {
    let mut ctx = Ctx::new(None).expect("ctx");

    let nonce = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .map(|d| d.as_nanos())
      .unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
      "the-editor-command-open-create-{}-{nonce}.txt",
      std::process::id()
    ));
    let _ = fs::remove_file(&path);
    assert!(!path.exists());

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char(':'),
      modifiers: Modifiers::empty(),
    });
    for ch in format!("open {}", path.display()).chars() {
      handle_key(&mut ctx, KeyEvent {
        key:       Key::Char(ch),
        modifiers: Modifiers::empty(),
      });
    }
    handle_key(&mut ctx, KeyEvent {
      key:       Key::Enter,
      modifiers: Modifiers::empty(),
    });

    assert_eq!(ctx.mode(), Mode::Normal);
    assert!(path.exists());
    assert_eq!(ctx.editor.document().text().to_string(), "");
    assert_eq!(ctx.file_path(), Some(path.as_path()));

    let _ = fs::remove_file(&path);
  }

  #[test]
  fn escape_with_completion_active_returns_to_normal_mode() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.set_mode(Mode::Insert);
    show_completion_menu(&mut ctx, vec![CompletionMenuItem::new("item")]);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Escape,
      modifiers: Modifiers::empty(),
    });

    assert_eq!(ctx.mode(), Mode::Normal);
    assert!(!ctx.completion_menu.active);
  }

  #[test]
  fn page_down_scrolls_completion_docs_when_menu_is_active() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.set_mode(Mode::Insert);

    let mut item = CompletionMenuItem::new("item");
    item.documentation = Some("line 1\nline 2\nline 3\nline 4\nline 5\nline 6".to_string());
    show_completion_menu(&mut ctx, vec![item]);
    assert_eq!(ctx.completion_menu.docs_scroll, 0);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::PageDown,
      modifiers: Modifiers::empty(),
    });

    assert!(ctx.completion_menu.docs_scroll > 0);
    assert_eq!(ctx.mode(), Mode::Insert);
    assert!(ctx.completion_menu.active);
  }

  #[test]
  fn ctrl_w_deletes_word_when_completion_menu_is_active() {
    let mut ctx = Ctx::new(None).expect("ctx");
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("hello world".into()))),
    )
    .expect("seed text");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    let cursor = ctx.editor.document().text().len_chars();
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::single(cursor, cursor));
    ctx.set_mode(Mode::Insert);
    show_completion_menu(&mut ctx, vec![CompletionMenuItem::new("world")]);

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('w'),
      modifiers: ctrl_modifiers(),
    });

    assert_eq!(ctx.editor.document().text().to_string(), "hello ");
  }

  #[test]
  fn page_down_falls_back_to_editor_scroll_when_completion_is_inactive() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.set_mode(Mode::Insert);

    let mut source = String::new();
    for idx in 0..120 {
      source.push_str(&format!("line {idx}\n"));
    }
    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some(source.into()))),
    )
    .expect("seed long text");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));
    let _ = ctx
      .editor
      .document_mut()
      .set_selection(Selection::single(0, 0));
    let before_scroll = ctx.editor.view().scroll.row;
    let before_cursor = {
      let doc = ctx.editor.document();
      let text = doc.text();
      let range = doc.selection().ranges()[0];
      let cursor = range.cursor(text.slice(..));
      text.char_to_line(cursor)
    };

    handle_key(&mut ctx, KeyEvent {
      key:       Key::PageDown,
      modifiers: Modifiers::empty(),
    });

    let after_scroll = ctx.editor.view().scroll.row;
    let after_cursor = {
      let doc = ctx.editor.document();
      let text = doc.text();
      let range = doc.selection().ranges()[0];
      let cursor = range.cursor(text.slice(..));
      text.char_to_line(cursor)
    };
    assert!(after_scroll > before_scroll || after_cursor > before_cursor);
  }

  #[test]
  fn capabilities_single_char_matches_trigger_and_retrigger_lists() {
    let raw = json!({
      "signatureHelpProvider": {
        "triggerCharacters": ["(", ","],
        "retriggerCharacters": [",", ")"],
      }
    });

    assert!(capabilities_support_single_char(
      &raw,
      "signatureHelpProvider",
      "triggerCharacters",
      '('
    ));
    assert!(capabilities_support_single_char(
      &raw,
      "signatureHelpProvider",
      "retriggerCharacters",
      ')'
    ));
    assert!(!capabilities_support_single_char(
      &raw,
      "signatureHelpProvider",
      "triggerCharacters",
      ';'
    ));
  }

  #[test]
  fn poll_signature_help_auto_trigger_clears_pending_outside_insert_mode() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.set_mode(Mode::Normal);
    ctx.lsp_pending_auto_signature_help = Some(PendingAutoSignatureHelp {
      due_at:  Instant::now() - Duration::from_millis(1),
      trigger: SignatureHelpTriggerSource::Manual,
    });

    assert!(!ctx.poll_lsp_signature_help_auto_trigger());
    assert!(ctx.lsp_pending_auto_signature_help.is_none());
  }

  #[test]
  fn signature_help_action_closes_state_on_non_edit_commands() {
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.set_mode(Mode::Insert);
    ctx.signature_help.active = true;
    ctx.lsp_pending_auto_signature_help = Some(PendingAutoSignatureHelp {
      due_at:  Instant::now() + Duration::from_millis(50),
      trigger: SignatureHelpTriggerSource::ContentChangeRetrigger,
    });

    assert!(!ctx.handle_signature_help_action(Command::Search));
    assert!(!ctx.signature_help.active);
    assert!(ctx.lsp_pending_auto_signature_help.is_none());
  }

  #[test]
  fn entering_insert_mode_does_not_warn_when_signature_help_is_unavailable() {
    let mut ctx = Ctx::new(None).expect("ctx");
    let before_seq = ctx.messages.latest_seq();

    handle_key(&mut ctx, KeyEvent {
      key:       Key::Char('i'),
      modifiers: Modifiers::empty(),
    });

    assert_eq!(ctx.mode(), Mode::Insert);
    let published_lsp_messages = ctx
      .messages
      .events_since(before_seq)
      .into_iter()
      .any(|event| {
        match event.kind {
          MessageEventKind::Published { message } => message.source.as_deref() == Some("lsp"),
          _ => false,
        }
      });
    assert!(!published_lsp_messages);
  }

  #[test]
  fn manual_signature_help_still_warns_when_unavailable() {
    let mut ctx = Ctx::new(None).expect("ctx");
    let before_seq = ctx.messages.latest_seq();

    ctx.lsp_signature_help();

    let warning = ctx
      .messages
      .events_since(before_seq)
      .into_iter()
      .find_map(|event| {
        match event.kind {
          MessageEventKind::Published { message } => {
            (message.level == the_lib::messages::MessageLevel::Warning
              && message.source.as_deref() == Some("lsp"))
            .then_some(message.text)
          },
          _ => None,
        }
      })
      .expect("signature help warning");
    assert_eq!(
      warning,
      "signature help is not supported by the active server"
    );
  }

  #[test]
  fn file_tree_diagnostics_aggregate_to_parent_directories() {
    let dir = TempTestDir::new("file-tree-diagnostics");
    let file = dir.write_file("src/main.rs", "fn main() {}\n");
    let uri = the_lsp::text_sync::file_uri_for_path(&file).expect("file uri");
    let mut diagnostics = DiagnosticsState::default();
    diagnostics.apply_document(the_lib::diagnostics::DocumentDiagnostics {
      uri,
      version: None,
      diagnostics: vec![the_lib::diagnostics::Diagnostic {
        range:    the_lib::diagnostics::DiagnosticRange {
          start: the_lib::diagnostics::DiagnosticPosition {
            line:      0,
            character: 0,
          },
          end:   the_lib::diagnostics::DiagnosticPosition {
            line:      0,
            character: 2,
          },
        },
        severity: Some(DiagnosticSeverity::Error),
        code:     None,
        source:   None,
        message:  "boom".to_string(),
      }],
    });

    let statuses = the_default::rebuild_file_tree_diagnostic_statuses(&diagnostics, dir.as_path());

    assert_eq!(statuses.get(&file), Some(&DiagnosticSeverity::Error));
    assert_eq!(
      statuses.get(&dir.as_path().join("src")),
      Some(&DiagnosticSeverity::Error)
    );
  }

  #[test]
  fn vcs_watch_events_schedule_file_tree_vcs_refresh() {
    let mut ctx = Ctx::new(None).expect("ctx");
    let dir = TempTestDir::new("vcs-watch-schedule");
    attach_test_file_tree(&mut ctx, dir.as_path());

    ctx.clear_pending_file_tree_vcs_refresh();
    let _ = ctx.handle_vcs_watch_change();
    assert!(ctx.file_tree_vcs_refresh_due_at.is_some());
    assert_eq!(
      ctx.file_tree_vcs_refresh_reason,
      Some(FileTreeVcsRefreshReason::VcsWatch)
    );
  }

  #[test]
  fn vcs_watch_events_schedule_active_file_vcs_refresh() {
    let dir = TempTestDir::new("active-file-vcs-watch-schedule");
    let file = dir.write_file("main.txt", "main\n");
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.file_path = Some(file);

    let _ = ctx.handle_vcs_watch_change();

    assert!(ctx.active_file_vcs_refresh_due_at.is_some());
    assert_eq!(
      ctx.active_file_vcs_refresh_reason,
      Some(ActiveFileVcsRefreshReason::VcsWatch)
    );
  }

  #[test]
  fn active_file_vcs_result_applies_diff_state() {
    let dir = TempTestDir::new("active-file-vcs-result-apply");
    let file = dir.write_file("main.txt", "current\n");
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.file_path = Some(file.clone());
    ctx.active_file_vcs_refresh_generation = 1;
    ctx.active_file_vcs_refresh_in_flight = true;

    ctx
      .active_file_vcs_refresh_tx
      .send(ActiveFileVcsRefreshResult {
        generation: 1,
        path:       file,
        reason:     ActiveFileVcsRefreshReason::VcsWatch,
        statusline: Some("status".to_string()),
        diff_base:  Some(b"base\n".to_vec()),
        scan:       None,
        collect_ms: 1.0,
      })
      .expect("send active file vcs result");

    assert!(ctx.poll_active_file_vcs_refresh_results());
    assert_eq!(ctx.vcs_statusline.as_deref(), Some("status"));
    assert!(ctx.vcs_diff.is_some());
    assert!(!ctx.active_file_vcs_refresh_in_flight);
  }

  #[test]
  fn stale_active_file_vcs_result_is_discarded() {
    let dir = TempTestDir::new("active-file-vcs-result-stale");
    let other_dir = TempTestDir::new("active-file-vcs-result-other");
    let file = dir.write_file("main.txt", "current\n");
    let other_file = other_dir.write_file("other.txt", "other\n");
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.file_path = Some(file);
    ctx.active_file_vcs_refresh_generation = 2;
    ctx.active_file_vcs_refresh_in_flight = true;

    ctx
      .active_file_vcs_refresh_tx
      .send(ActiveFileVcsRefreshResult {
        generation: 1,
        path:       other_file,
        reason:     ActiveFileVcsRefreshReason::VcsWatch,
        statusline: Some("stale".to_string()),
        diff_base:  Some(b"base\n".to_vec()),
        scan:       None,
        collect_ms: 1.0,
      })
      .expect("send stale active file vcs result");

    assert!(!ctx.poll_active_file_vcs_refresh_results());
    assert!(ctx.vcs_statusline.is_none());
    assert!(ctx.vcs_diff.is_none());
    assert!(ctx.active_file_vcs_refresh_in_flight);
  }

  #[test]
  fn active_file_path_change_clears_previous_vcs_state() {
    let dir = TempTestDir::new("active-file-vcs-path-change");
    let old_file = dir.write_file("old.txt", "old\n");
    let new_file = dir.write_file("new.txt", "new\n");
    let mut ctx = Ctx::new(None).expect("ctx");
    ctx.file_path = Some(old_file);
    ctx.vcs_statusline = Some("old-status".to_string());
    ctx.vcs_diff = Some(DiffHandle::new(
      Rope::from_str("base\n"),
      Rope::from_str("current\n"),
    ));
    ctx
      .gutter_diff_signs
      .insert(0, RenderGutterDiffKind::Modified);

    let previous_path = ctx.file_path.clone();
    ctx.file_path = Some(new_file);
    ctx.refresh_active_file_vcs_after_path_change(
      previous_path,
      ActiveFileVcsRefreshReason::PathChange,
    );

    assert!(ctx.vcs_statusline.is_none());
    assert!(ctx.vcs_diff.is_none());
    assert!(ctx.gutter_diff_signs.is_empty());
    assert!(ctx.active_file_vcs_refresh_due_at.is_some());
  }

  #[test]
  fn merged_vcs_changed_file_items_include_unsaved_modified_buffers() {
    let dir = TempTestDir::new("vcs-diff-picker-dirty-buffer");
    let file = dir.write_file("main.rs", "fn main() {}\n");
    let mut ctx = Ctx::new(None).expect("ctx");
    assert!(
      ctx
        .editor
        .replace_active_buffer(Rope::from_str("fn main() {}\n"), Some(file.clone()))
    );
    ctx.file_path = Some(file.clone());
    let _ = ctx.editor.document_mut().mark_saved();

    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("// dirty\n".into()))),
    )
    .expect("edit transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    let scan = VcsWorkspaceScan {
      provider_label:  "jj".to_string(),
      repo_root:       dir.as_path().to_path_buf(),
      statusline_info: None,
      head_revision:   Some("abc123".to_string()),
      changes:         Vec::new(),
    };

    let items = ctx.merged_vcs_changed_file_items(&scan);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].path, file);
    assert_eq!(items[0].kind, FilePickerChangedKind::Modified);
    ctx.shutdown_background_services();
  }

  #[test]
  fn merged_vcs_changed_file_items_normalize_relative_buffer_paths() {
    let dir = TempTestDir::new("vcs-diff-picker-relative-buffer");
    let file = dir.write_file("main.rs", "fn main() {}\n");
    let mut ctx = Ctx::new(None).expect("ctx");
    assert!(
      ctx
        .editor
        .replace_active_buffer(Rope::from_str("fn main() {}\n"), Some(file.clone()))
    );
    ctx.file_path = Some(file.clone());
    let _ = ctx.editor.document_mut().mark_saved();

    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("// dirty\n".into()))),
    )
    .expect("edit transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    ctx
      .editor
      .set_active_file_path(Some(PathBuf::from("main.rs")));
    ctx.file_path = Some(PathBuf::from("main.rs"));

    let scan = VcsWorkspaceScan {
      provider_label:  "jj".to_string(),
      repo_root:       dir.as_path().to_path_buf(),
      statusline_info: None,
      head_revision:   Some("abc123".to_string()),
      changes:         Vec::new(),
    };

    let items = ctx.merged_vcs_changed_file_items(&scan);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].path, file);
    assert_eq!(items[0].kind, FilePickerChangedKind::Modified);
    ctx.shutdown_background_services();
  }

  #[test]
  fn merged_vcs_changed_file_items_do_not_duplicate_existing_scan_entries() {
    let dir = TempTestDir::new("vcs-diff-picker-no-duplicate");
    let file = dir.write_file("main.rs", "fn main() {}\n");
    let mut ctx = Ctx::new(None).expect("ctx");
    assert!(
      ctx
        .editor
        .replace_active_buffer(Rope::from_str("fn main() {}\n"), Some(file.clone()))
    );
    ctx.file_path = Some(file.clone());
    let _ = ctx.editor.document_mut().mark_saved();

    let tx = Transaction::change(
      ctx.editor.document().text(),
      std::iter::once((0, 0, Some("// dirty\n".into()))),
    )
    .expect("edit transaction");
    assert!(DefaultContext::apply_transaction(&mut ctx, &tx));

    let scan = VcsWorkspaceScan {
      provider_label:  "jj".to_string(),
      repo_root:       dir.as_path().to_path_buf(),
      statusline_info: None,
      head_revision:   Some("abc123".to_string()),
      changes:         vec![FileChange::Modified { path: file.clone() }],
    };

    let items = ctx.merged_vcs_changed_file_items(&scan);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].path, file);
    assert_eq!(items[0].kind, FilePickerChangedKind::Modified);
    ctx.shutdown_background_services();
  }

  #[test]
  fn vcs_diff_entries_match_changed_items_requires_same_identity_and_order() {
    let changed = vec![
      FilePickerChangedFileItem {
        kind:      FilePickerChangedKind::Modified,
        path:      PathBuf::from("a.rs"),
        from_path: None,
      },
      FilePickerChangedFileItem {
        kind:      FilePickerChangedKind::Renamed,
        path:      PathBuf::from("b.rs"),
        from_path: Some(PathBuf::from("old-b.rs")),
      },
    ];
    let entries = changed
      .iter()
      .map(file_picker_vcs_diff_placeholder_entry)
      .collect::<Vec<_>>();

    assert!(vcs_diff_entries_match_changed_items(&entries, &changed));

    let reordered = vec![changed[1].clone(), changed[0].clone()];
    assert!(!vcs_diff_entries_match_changed_items(&entries, &reordered));
  }

  #[test]
  fn saving_with_open_vcs_picker_schedules_refreshes() {
    let dir = TempTestDir::new("vcs-diff-picker-save-refresh");
    let file = dir.write_file("main.rs", "fn main() {}\n");
    let mut ctx = Ctx::new(None).expect("ctx");
    assert!(
      ctx
        .editor
        .replace_active_buffer(Rope::from_str("fn main() {}\n"), Some(file.clone()))
    );
    ctx.file_path = Some(file.clone());
    let _ = ctx.editor.document_mut().mark_saved();
    ctx.file_picker.active = true;
    ctx.file_picker.kind = the_default::FilePickerKind::VcsDiff;
    attach_test_file_tree(&mut ctx, dir.as_path());

    assert!(ctx.file_tree_vcs_refresh_due_at.is_none());
    assert!(ctx.active_file_vcs_refresh_due_at.is_none());
    ctx.active_file_vcs_refresh_due_at = None;

    let buffer_id = ctx.editor.active_buffer_id();
    ctx.on_buffer_saved(buffer_id, &file, "fn main() {}\n");

    assert!(ctx.vcs_diff_picker.live_refresh_pending);
    assert!(ctx.file_tree_vcs_refresh_due_at.is_some());
    assert!(ctx.active_file_vcs_refresh_due_at.is_some());
    ctx.shutdown_background_services();
  }

  #[test]
  fn vcs_diff_snapshot_prefers_disk_for_clean_open_buffers() {
    let dir = TempTestDir::new("vcs-diff-picker-clean-buffer-disk");
    let file = dir.write_file("main.rs", "fn main() {}\n");

    let mut open_buffers = HashMap::new();
    open_buffers.insert(file.clone(), OpenBufferVcsSnapshot {
      text:     "fn main() {}\n".to_string(),
      modified: false,
    });

    fs::write(&file, "// disk change\nfn main() {}\n").expect("write file");

    let text =
      vcs_worktree_text_from_snapshot(&open_buffers, &FileChange::Modified { path: file.clone() })
        .expect("worktree text");

    assert_eq!(text, "// disk change\nfn main() {}\n");
  }

  #[test]
  fn file_tree_vcs_result_applies_decorations() {
    let dir = TempTestDir::new("file-tree-vcs-result-apply");
    let changed = dir.write_file("changed.txt", "changed\n");
    let mut ctx = Ctx::new(None).expect("ctx");
    attach_test_file_tree(&mut ctx, dir.as_path());
    ctx.file_tree_vcs_refresh_generation = 1;
    ctx.file_tree_vcs_refresh_in_flight = true;

    let mut statuses = BTreeMap::new();
    statuses.insert(changed.clone(), the_default::FileTreeVcsKind::Modified);
    ctx
      .file_tree_vcs_refresh_tx
      .send(FileTreeVcsRefreshResult {
        generation: 1,
        root: dir.as_path().to_path_buf(),
        reason: FileTreeVcsRefreshReason::VcsWatch,
        statuses,
        change_count: 1,
        status_entries: 1,
        collect_ms: 1.0,
        collapse_ms: 0.5,
        scan: None,
      })
      .expect("send vcs result");

    assert!(ctx.poll_file_tree_vcs_refresh_results());
    assert_eq!(
      ctx
        .file_tree
        .rows
        .iter()
        .find(|row| row.path == changed)
        .and_then(|row| row.decorations.vcs),
      Some(the_default::FileTreeVcsKind::Modified)
    );
  }

  #[test]
  fn stale_file_tree_vcs_result_is_discarded() {
    let dir = TempTestDir::new("file-tree-vcs-result-stale");
    let other_dir = TempTestDir::new("file-tree-vcs-result-other");
    let changed = dir.write_file("changed.txt", "changed\n");
    let mut ctx = Ctx::new(None).expect("ctx");
    attach_test_file_tree(&mut ctx, dir.as_path());
    ctx.file_tree_vcs_refresh_generation = 2;
    ctx.file_tree_vcs_refresh_in_flight = true;

    let mut statuses = BTreeMap::new();
    statuses.insert(changed, the_default::FileTreeVcsKind::Modified);
    ctx
      .file_tree_vcs_refresh_tx
      .send(FileTreeVcsRefreshResult {
        generation: 1,
        root: other_dir.as_path().to_path_buf(),
        reason: FileTreeVcsRefreshReason::VcsWatch,
        statuses,
        change_count: 1,
        status_entries: 1,
        collect_ms: 1.0,
        collapse_ms: 0.5,
        scan: None,
      })
      .expect("send stale vcs result");

    assert!(!ctx.poll_file_tree_vcs_refresh_results());
    assert!(
      ctx
        .file_tree
        .rows
        .iter()
        .all(|row| row.decorations == the_default::FileTreeDecorations::default())
    );
    assert!(ctx.file_tree_vcs_refresh_in_flight);
  }
}
