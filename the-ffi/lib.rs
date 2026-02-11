//! FFI bindings for the-editor, exposing core functionality to Swift via
//! swift-bridge.
//!
//! This crate provides a C-compatible interface to the-lib, allowing the
//! SwiftUI client to interact with the Rust editor core.

use std::{
  collections::{
    BTreeMap,
    HashMap,
    HashSet,
    VecDeque,
  },
  env,
  num::{
    NonZeroU64,
    NonZeroUsize,
  },
  path::{
    Path,
    PathBuf,
  },
  sync::{
    Arc,
    atomic::{
      AtomicUsize,
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
  },
};

use ropey::Rope;
use the_config::{
  build_dispatch as config_build_dispatch,
  build_keymaps as config_build_keymaps,
};
use the_default::{
  Command,
  CommandEvent,
  CommandPaletteLayout,
  CommandPaletteState,
  CommandPaletteStyle,
  CommandPaletteTheme,
  CommandPromptState,
  CommandRegistry,
  DefaultContext,
  DefaultDispatchStatic,
  Direction as CommandDirection,
  DispatchRef,
  FilePickerState,
  KeyBinding,
  KeyEvent,
  Keymaps,
  MessagePresentation,
  Mode,
  Motion,
  SearchPromptState,
  close_file_picker,
  command_palette_default_selected,
  command_palette_filtered_indices,
  command_palette_selected_filtered_index,
  finalize_search,
  handle_query_change as file_picker_handle_query_change,
  poll_scan_results as file_picker_poll_scan_results,
  refresh_matcher_state as file_picker_refresh_matcher_state,
  select_file_picker_index,
  set_file_picker_syntax_loader,
  submit_file_picker,
  update_search_preview,
};
use the_lib::{
  Tendril,
  app::App as LibApp,
  diagnostics::{
    DiagnosticCounts,
    DiagnosticSeverity,
    DiagnosticsState,
  },
  document::{
    Document as LibDocument,
    DocumentId,
  },
  editor::EditorId as LibEditorId,
  messages::MessageCenter,
  movement::{
    self,
    Direction,
    Movement,
  },
  position::Position as LibPosition,
  registers::Registers,
  render::{
    GutterConfig,
    NoHighlights,
    OverlayNode,
    OverlayRectKind,
    OverlayText,
    RenderDiagnosticGutterStyles,
    RenderDiffGutterStyles,
    RenderGutterDiffKind,
    RenderStyles,
    SyntaxHighlightAdapter,
    UiState,
    apply_diff_gutter_markers,
    apply_diagnostic_gutter_markers,
    build_plan,
    gutter_width_for_document,
    graphics::{
      Color as LibColor,
      CursorKind as LibCursorKind,
      Rect as LibRect,
      Style as LibStyle,
      UnderlineStyle as LibUnderlineStyle,
    },
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
  selection::{
    CursorId,
    CursorPick,
    Selection,
  },
  syntax::{
    Highlight,
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
  transaction::Transaction,
  view::ViewState,
};
use the_loader::config::user_lang_config;
use the_lsp::{
  LspCapability,
  LspEvent,
  LspLocation,
  LspPosition,
  LspProgressKind,
  LspRuntime,
  LspRuntimeConfig,
  LspServerConfig,
  goto_definition_params,
  jsonrpc,
  parse_locations_response,
  text_sync::{
    FileChangeType,
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
};
use the_runtime::file_watch::{
  PathEventKind,
  WatchHandle,
  resolve_trace_log_path as resolve_file_watch_trace_log_path,
  trace_event as trace_file_watch_event,
  watch as watch_path,
};
use the_vcs::{
  DiffHandle,
  DiffProviderRegistry,
  DiffSignKind,
};

/// Global document ID counter for FFI layer.
static NEXT_DOC_ID: AtomicUsize = AtomicUsize::new(1);

fn next_doc_id() -> DocumentId {
  let id = NEXT_DOC_ID.fetch_add(1, Ordering::Relaxed).max(1);
  DocumentId::new(NonZeroUsize::new(id).expect("document id overflow"))
}

fn mode_to_u8(mode: Mode) -> u8 {
  match mode {
    Mode::Normal => 0,
    Mode::Insert => 1,
    Mode::Select => 2,
    Mode::Command => 3,
  }
}

/// FFI-safe document wrapper.
///
/// This wraps the core `Document` type and provides simplified methods
/// suitable for FFI export.
pub struct Document {
  inner: LibDocument,
}

impl Document {
  /// Create a new empty document.
  pub fn new() -> Self {
    Self {
      inner: LibDocument::new(next_doc_id(), Rope::new()),
    }
  }

  /// Create a document from text content.
  pub fn from_text(text: &str) -> Self {
    Self {
      inner: LibDocument::new(next_doc_id(), Rope::from_str(text)),
    }
  }

  /// Get the full text content as a string.
  pub fn text(&self) -> String {
    self.inner.text().to_string()
  }

  /// Get the number of characters in the document.
  pub fn len_chars(&self) -> usize {
    self.inner.text().len_chars()
  }

  /// Get the number of lines in the document.
  pub fn len_lines(&self) -> usize {
    self.inner.text().len_lines()
  }

  /// Check if the document is empty.
  pub fn is_empty(&self) -> bool {
    self.inner.text().len_chars() == 0
  }

  /// Get the document version (increments on each change).
  pub fn version(&self) -> u64 {
    self.inner.version()
  }

  /// Check if the document has been modified since last commit.
  pub fn is_modified(&self) -> bool {
    self.inner.flags().modified
  }

  // --- Selection queries ---

  /// Get the primary cursor position (character index).
  pub fn primary_cursor(&self) -> usize {
    let slice = self.inner.text().slice(..);
    self.inner.selection().ranges()[0].cursor(slice)
  }

  /// Get the number of cursors (ranges) in the selection.
  pub fn cursor_count(&self) -> usize {
    self.inner.selection().len()
  }

  /// Get cursor position at the given index.
  /// Returns None if index is out of bounds.
  pub fn cursor_at(&self, index: usize) -> Option<usize> {
    let ranges = self.inner.selection().ranges();
    if index < ranges.len() {
      let slice = self.inner.text().slice(..);
      Some(ranges[index].cursor(slice))
    } else {
      None
    }
  }

  /// Get all cursor positions as a vector.
  pub fn all_cursors(&self) -> Vec<usize> {
    let slice = self.inner.text().slice(..);
    self
      .inner
      .selection()
      .ranges()
      .iter()
      .map(|r| r.cursor(slice))
      .collect()
  }

  // --- Text editing ---

  /// Insert text at all cursor positions.
  pub fn insert(&mut self, text: &str) -> bool {
    insert_text(&mut self.inner, text)
  }

  /// Delete one character before each cursor (backspace).
  pub fn delete_backward(&mut self) -> bool {
    delete_backward(&mut self.inner)
  }

  /// Delete one character after each cursor (delete key).
  pub fn delete_forward(&mut self) -> bool {
    delete_forward(&mut self.inner)
  }

  // --- Cursor movement ---

  /// Move all cursors left by one character.
  pub fn move_left(&mut self) {
    self.move_horizontal(Direction::Backward);
  }

  /// Move all cursors right by one character.
  pub fn move_right(&mut self) {
    self.move_horizontal(Direction::Forward);
  }

  /// Move all cursors up by one line.
  pub fn move_up(&mut self) {
    self.move_vertical(Direction::Backward);
  }

  /// Move all cursors down by one line.
  pub fn move_down(&mut self) {
    self.move_vertical(Direction::Forward);
  }

  fn move_horizontal(&mut self, dir: Direction) {
    let text_fmt = TextFormat::default();
    move_horizontal(&mut self.inner, dir, &text_fmt);
  }

  fn move_vertical(&mut self, dir: Direction) {
    let text_fmt = TextFormat::default();
    move_vertical(&mut self.inner, dir, &text_fmt);
  }

  // --- Multi-cursor ---

  /// Add a cursor on the line above the primary cursor.
  pub fn add_cursor_above(&mut self) -> bool {
    self.add_cursor_vertical(Direction::Backward)
  }

  /// Add a cursor on the line below the primary cursor.
  pub fn add_cursor_below(&mut self) -> bool {
    self.add_cursor_vertical(Direction::Forward)
  }

  fn add_cursor_vertical(&mut self, dir: Direction) -> bool {
    let text_fmt = TextFormat::default();
    add_cursor_vertical(&mut self.inner, dir, CursorPick::First, &text_fmt)
  }

  /// Remove all cursors except the primary.
  pub fn collapse_to_primary(&mut self) {
    let _ = collapse_selection(&mut self.inner, CursorPick::First);
  }

  // --- History ---

  /// Commit current changes to history.
  pub fn commit(&mut self) -> bool {
    self.inner.commit().is_ok()
  }

  /// Undo the last committed change.
  pub fn undo(&mut self) -> bool {
    self.inner.undo().unwrap_or(false)
  }

  /// Redo the last undone change.
  pub fn redo(&mut self) -> bool {
    self.inner.redo().unwrap_or(false)
  }

  // --- Line access (for rendering) ---

  /// Get a specific line's content.
  /// Returns None if line index is out of bounds.
  pub fn line(&self, line_idx: usize) -> Option<String> {
    let rope = self.inner.text();
    if line_idx < rope.len_lines() {
      Some(rope.line(line_idx).to_string())
    } else {
      None
    }
  }

  /// Get the line number for a character position.
  pub fn char_to_line(&self, char_idx: usize) -> usize {
    self.inner.text().char_to_line(char_idx)
  }

  /// Get the character position at the start of a line.
  pub fn line_to_char(&self, line_idx: usize) -> usize {
    self.inner.text().line_to_char(line_idx)
  }
}

#[derive(Debug, Clone)]
pub struct RenderSpan {
  inner: the_lib::render::RenderSpan,
}

impl Default for RenderSpan {
  fn default() -> Self {
    Self {
      inner: the_lib::render::RenderSpan {
        col:        0,
        cols:       0,
        text:       Tendril::new(),
        highlight:  None,
        is_virtual: false,
      },
    }
  }
}

impl RenderSpan {
  fn col(&self) -> u16 {
    self.inner.col
  }

  fn cols(&self) -> u16 {
    self.inner.cols
  }

  fn text(&self) -> String {
    self.inner.text.to_string()
  }

  fn has_highlight(&self) -> bool {
    self.inner.highlight.is_some()
  }

  fn highlight(&self) -> u32 {
    self
      .inner
      .highlight
      .map(|highlight| highlight.get())
      .unwrap_or(0)
  }

  fn is_virtual(&self) -> bool {
    self.inner.is_virtual
  }
}

impl From<the_lib::render::RenderSpan> for RenderSpan {
  fn from(span: the_lib::render::RenderSpan) -> Self {
    Self { inner: span }
  }
}

#[derive(Debug, Clone)]
pub struct RenderGutterSpan {
  inner: the_lib::render::RenderGutterSpan,
}

impl Default for RenderGutterSpan {
  fn default() -> Self {
    Self {
      inner: the_lib::render::RenderGutterSpan {
        col:   0,
        text:  Tendril::new(),
        style: LibStyle::default(),
      },
    }
  }
}

impl RenderGutterSpan {
  fn col(&self) -> u16 {
    self.inner.col
  }

  fn text(&self) -> String {
    self.inner.text.to_string()
  }

  fn style(&self) -> ffi::Style {
    self.inner.style.into()
  }
}

impl From<the_lib::render::RenderGutterSpan> for RenderGutterSpan {
  fn from(span: the_lib::render::RenderGutterSpan) -> Self {
    Self { inner: span }
  }
}

#[derive(Debug, Clone)]
pub struct RenderGutterLine {
  inner: the_lib::render::RenderGutterLine,
}

impl Default for RenderGutterLine {
  fn default() -> Self {
    Self {
      inner: the_lib::render::RenderGutterLine {
        row:   0,
        spans: Vec::new(),
      },
    }
  }
}

impl RenderGutterLine {
  fn row(&self) -> u16 {
    self.inner.row
  }

  fn span_count(&self) -> usize {
    self.inner.spans.len()
  }

  fn span_at(&self, index: usize) -> RenderGutterSpan {
    self
      .inner
      .spans
      .get(index)
      .cloned()
      .map(RenderGutterSpan::from)
      .unwrap_or_default()
  }
}

impl From<the_lib::render::RenderGutterLine> for RenderGutterLine {
  fn from(line: the_lib::render::RenderGutterLine) -> Self {
    Self { inner: line }
  }
}

#[derive(Debug, Clone)]
pub struct RenderLine {
  inner: the_lib::render::RenderLine,
}

impl Default for RenderLine {
  fn default() -> Self {
    Self {
      inner: the_lib::render::RenderLine {
        row:   0,
        spans: Vec::new(),
      },
    }
  }
}

impl RenderLine {
  fn row(&self) -> u16 {
    self.inner.row
  }

  fn span_count(&self) -> usize {
    self.inner.spans.len()
  }

  fn span_at(&self, index: usize) -> RenderSpan {
    self
      .inner
      .spans
      .get(index)
      .cloned()
      .map(RenderSpan::from)
      .unwrap_or_default()
  }
}

impl From<the_lib::render::RenderLine> for RenderLine {
  fn from(line: the_lib::render::RenderLine) -> Self {
    Self { inner: line }
  }
}

#[derive(Debug, Clone)]
pub struct RenderCursor {
  inner: the_lib::render::RenderCursor,
}

impl Default for RenderCursor {
  fn default() -> Self {
    Self {
      inner: the_lib::render::RenderCursor {
        id:    CursorId::new(NonZeroU64::new(1).expect("cursor id must be non-zero")),
        pos:   LibPosition::new(0, 0),
        kind:  LibCursorKind::Hidden,
        style: LibStyle::default(),
      },
    }
  }
}

impl RenderCursor {
  fn id(&self) -> u64 {
    self.inner.id.get()
  }

  fn pos(&self) -> ffi::Position {
    self.inner.pos.into()
  }

  fn kind(&self) -> u8 {
    cursor_kind_to_u8(self.inner.kind)
  }

  fn style(&self) -> ffi::Style {
    self.inner.style.into()
  }
}

impl From<the_lib::render::RenderCursor> for RenderCursor {
  fn from(cursor: the_lib::render::RenderCursor) -> Self {
    Self { inner: cursor }
  }
}

#[derive(Debug, Clone)]
pub struct RenderSelection {
  inner: the_lib::render::RenderSelection,
}

impl Default for RenderSelection {
  fn default() -> Self {
    Self {
      inner: the_lib::render::RenderSelection {
        rect:  LibRect::new(0, 0, 0, 0),
        style: LibStyle::default(),
      },
    }
  }
}

impl RenderSelection {
  fn rect(&self) -> ffi::Rect {
    self.inner.rect.into()
  }

  fn style(&self) -> ffi::Style {
    self.inner.style.into()
  }
}

impl From<the_lib::render::RenderSelection> for RenderSelection {
  fn from(selection: the_lib::render::RenderSelection) -> Self {
    Self { inner: selection }
  }
}

#[derive(Debug, Clone)]
pub struct RenderOverlayNode {
  inner: OverlayNode,
}

impl RenderOverlayNode {
  fn empty() -> Self {
    Self {
      inner: OverlayNode::Text(OverlayText {
        pos:   LibPosition::new(0, 0),
        text:  String::new(),
        style: LibStyle::default(),
      }),
    }
  }

  fn kind(&self) -> u8 {
    match self.inner {
      OverlayNode::Rect(_) => 1,
      OverlayNode::Text(_) => 2,
    }
  }

  fn rect_kind(&self) -> u8 {
    match self.inner {
      OverlayNode::Rect(ref rect) => overlay_rect_kind_to_u8(rect.kind),
      OverlayNode::Text(_) => 0,
    }
  }

  fn rect(&self) -> ffi::Rect {
    match self.inner {
      OverlayNode::Rect(ref rect) => rect.rect.into(),
      OverlayNode::Text(_) => LibRect::new(0, 0, 0, 0).into(),
    }
  }

  fn radius(&self) -> u16 {
    match self.inner {
      OverlayNode::Rect(ref rect) => rect.radius,
      OverlayNode::Text(_) => 0,
    }
  }

  fn pos(&self) -> ffi::Position {
    match self.inner {
      OverlayNode::Text(ref text) => text.pos.into(),
      OverlayNode::Rect(_) => LibPosition::new(0, 0).into(),
    }
  }

  fn text(&self) -> String {
    match self.inner {
      OverlayNode::Text(ref text) => text.text.clone(),
      OverlayNode::Rect(_) => String::new(),
    }
  }

  fn style(&self) -> ffi::Style {
    match self.inner {
      OverlayNode::Rect(ref rect) => rect.style.into(),
      OverlayNode::Text(ref text) => text.style.into(),
    }
  }
}

impl From<OverlayNode> for RenderOverlayNode {
  fn from(node: OverlayNode) -> Self {
    Self { inner: node }
  }
}

#[derive(Debug, Clone)]
pub struct RenderPlan {
  inner: the_lib::render::RenderPlan,
}

impl RenderPlan {
  fn empty() -> Self {
    Self {
      inner: the_lib::render::RenderPlan::empty(LibRect::new(0, 0, 0, 0), LibPosition::new(0, 0)),
    }
  }

  fn viewport(&self) -> ffi::Rect {
    self.inner.viewport.into()
  }

  fn scroll(&self) -> ffi::Position {
    self.inner.scroll.into()
  }

  fn content_offset_x(&self) -> u16 {
    self.inner.content_offset_x
  }

  fn gutter_line_count(&self) -> usize {
    self.inner.gutter_lines.len()
  }

  fn gutter_line_at(&self, index: usize) -> RenderGutterLine {
    self
      .inner
      .gutter_lines
      .get(index)
      .cloned()
      .map(RenderGutterLine::from)
      .unwrap_or_default()
  }

  fn line_count(&self) -> usize {
    self.inner.lines.len()
  }

  fn line_at(&self, index: usize) -> RenderLine {
    self
      .inner
      .lines
      .get(index)
      .cloned()
      .map(RenderLine::from)
      .unwrap_or_default()
  }

  fn cursor_count(&self) -> usize {
    self.inner.cursors.len()
  }

  fn cursor_at(&self, index: usize) -> RenderCursor {
    self
      .inner
      .cursors
      .get(index)
      .cloned()
      .map(RenderCursor::from)
      .unwrap_or_default()
  }

  fn selection_count(&self) -> usize {
    self.inner.selections.len()
  }

  fn selection_at(&self, index: usize) -> RenderSelection {
    self
      .inner
      .selections
      .get(index)
      .cloned()
      .map(RenderSelection::from)
      .unwrap_or_default()
  }

  fn overlay_count(&self) -> usize {
    self.inner.overlays.len()
  }

  fn overlay_at(&self, index: usize) -> RenderOverlayNode {
    self
      .inner
      .overlays
      .get(index)
      .cloned()
      .map(RenderOverlayNode::from)
      .unwrap_or_else(RenderOverlayNode::empty)
  }
}

impl From<the_lib::render::RenderPlan> for RenderPlan {
  fn from(plan: the_lib::render::RenderPlan) -> Self {
    Self { inner: plan }
  }
}

impl Default for Document {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(Debug)]
struct SyntaxParseResult {
  request_id:  u64,
  doc_version: u64,
  syntax:      Option<Syntax>,
}

type SyntaxParseJob = Box<dyn FnOnce() -> Option<Syntax> + Send>;

fn spawn_syntax_parse_request(
  tx: Sender<SyntaxParseResult>,
  request: ParseRequest<SyntaxParseJob>,
) {
  thread::spawn(move || {
    let parsed = (request.payload)();
    let _ = tx.send(SyntaxParseResult {
      request_id: request.meta.request_id,
      doc_version: request.meta.doc_version,
      syntax: parsed,
    });
  });
}

#[derive(Debug, Clone)]
struct LspDocumentSyncState {
  path:        PathBuf,
  uri:         String,
  language_id: String,
  version:     i32,
  opened:      bool,
}

struct LspWatchedFileState {
  path:           PathBuf,
  uri:            String,
  events_rx:      Receiver<Vec<the_runtime::file_watch::PathEvent>>,
  _watch_handle:  WatchHandle,
  suppress_until: Option<Instant>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingLspRequestKind {
  GotoDefinition { uri: String },
}

impl PendingLspRequestKind {
  fn label(&self) -> &'static str {
    match self {
      Self::GotoDefinition { .. } => "goto-definition",
    }
  }

  fn uri(&self) -> &str {
    match self {
      Self::GotoDefinition { uri } => uri.as_str(),
    }
  }
}

struct EditorState {
  mode:                  Mode,
  command_prompt:        CommandPromptState,
  command_palette:       CommandPaletteState,
  command_palette_style: CommandPaletteStyle,
  file_picker:           FilePickerState,
  search_prompt:         SearchPromptState,
  ui_state:              UiState,
  needs_render:          bool,
  messages:              MessageCenter,
  pending_input:         Option<the_default::PendingInput>,
  register:              Option<char>,
  macro_recording:       Option<(char, Vec<KeyBinding>)>,
  macro_replaying:       Vec<char>,
  macro_queue:           VecDeque<KeyEvent>,
  text_format:           TextFormat,
  gutter_config:         GutterConfig,
  gutter_diff_signs:     BTreeMap<usize, RenderGutterDiffKind>,
  vcs_statusline:        Option<String>,
  inline_annotations:    Vec<InlineAnnotation>,
  overlay_annotations:   Vec<Overlay>,
  highlight_cache:       HighlightCache,
  syntax_parse_tx:       Sender<SyntaxParseResult>,
  syntax_parse_rx:       Receiver<SyntaxParseResult>,
  syntax_parse_lifecycle: ParseLifecycle<SyntaxParseJob>,
  syntax_parse_highlight_state: ParseHighlightState,
  scrolloff:             usize,
}

impl EditorState {
  fn new(loader: Option<Arc<Loader>>) -> Self {
    let mut command_palette_style = CommandPaletteStyle::floating(CommandPaletteTheme::ghostty());
    command_palette_style.layout = CommandPaletteLayout::Custom;
    let mut file_picker = FilePickerState::default();
    set_file_picker_syntax_loader(&mut file_picker, loader);
    let (syntax_parse_tx, syntax_parse_rx) = channel();

    Self {
      mode: Mode::Normal,
      command_prompt: CommandPromptState::new(),
      command_palette: CommandPaletteState::default(),
      command_palette_style,
      file_picker,
      search_prompt: SearchPromptState::new(),
      ui_state: UiState::default(),
      needs_render: true,
      messages: MessageCenter::default(),
      pending_input: None,
      register: None,
      macro_recording: None,
      macro_replaying: Vec::new(),
      macro_queue: VecDeque::new(),
      text_format: TextFormat::default(),
      gutter_config: GutterConfig::default(),
      gutter_diff_signs: BTreeMap::new(),
      vcs_statusline: None,
      inline_annotations: Vec::new(),
      overlay_annotations: Vec::new(),
      highlight_cache: HighlightCache::default(),
      syntax_parse_tx,
      syntax_parse_rx,
      syntax_parse_lifecycle: ParseLifecycle::default(),
      syntax_parse_highlight_state: ParseHighlightState::default(),
      scrolloff: 5,
    }
  }
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

fn render_diagnostic_styles_from_theme(theme: &Theme) -> RenderDiagnosticGutterStyles {
  RenderDiagnosticGutterStyles {
    error:   theme
      .try_get("error")
      .or_else(|| theme.try_get("diagnostic.error"))
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    warning: theme
      .try_get("warning")
      .or_else(|| theme.try_get("diagnostic.warning"))
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    info:    theme
      .try_get("info")
      .or_else(|| theme.try_get("diagnostic.info"))
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    hint:    theme
      .try_get("hint")
      .or_else(|| theme.try_get("diagnostic.hint"))
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
  }
}

fn render_diff_styles_from_theme(theme: &Theme) -> RenderDiffGutterStyles {
  RenderDiffGutterStyles {
    added:    theme
      .try_get("diff.plus")
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    modified: theme
      .try_get("diff.delta")
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    removed:  theme
      .try_get("diff.minus")
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
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

fn diagnostic_severity_rank(severity: DiagnosticSeverity) -> u8 {
  match severity {
    DiagnosticSeverity::Error => 4,
    DiagnosticSeverity::Warning => 3,
    DiagnosticSeverity::Information => 2,
    DiagnosticSeverity::Hint => 1,
  }
}

fn init_loader(theme: &Theme) -> Result<Loader, String> {
  use the_lib::syntax::{
    config::Configuration,
    runtime_loader::RuntimeLoader,
  };

  let config_value = user_lang_config().map_err(|error| error.to_string())?;
  let config: Configuration = config_value
    .try_into()
    .map_err(|error| format!("{error}"))?;
  let loader = Loader::new(config, RuntimeLoader::new()).map_err(|error| format!("{error}"))?;
  loader.set_scopes(theme.scopes().iter().cloned().collect());
  Ok(loader)
}

fn setup_syntax(doc: &mut LibDocument, path: &Path, loader: &Arc<Loader>) -> Result<(), String> {
  let language = loader
    .language_for_filename(path)
    .ok_or_else(|| format!("unknown language for {}", path.display()))?;
  let syntax = Syntax::new(doc.text().slice(..), language, loader.as_ref())
    .map_err(|error| format!("{error}"))?;
  doc.set_syntax_with_loader(syntax, loader.clone());
  Ok(())
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

fn command_in_path(command: &str) -> bool {
  if command.trim().is_empty() {
    return false;
  }
  let command_path = Path::new(command);
  if command_path.is_absolute() || command.contains(std::path::MAIN_SEPARATOR) {
    return command_path.exists();
  }
  let Ok(path_var) = env::var("PATH") else {
    return false;
  };
  env::split_paths(&path_var).any(|dir| dir.join(command).exists())
}

fn lsp_server_from_language_config(loader: &Loader, path: &Path) -> Option<LspServerConfig> {
  let language = loader.language_for_filename(path)?;
  let language_config = loader.language(language).config();
  let mut fallback = None;

  for server_features in &language_config.services.language_servers {
    let server_name = server_features.name.clone();
    let Some(server_config) = loader.language_server_configs().get(&server_name) else {
      continue;
    };
    let server = LspServerConfig::new(server_name, server_config.command.clone())
      .with_args(server_config.args.clone())
      .with_env(
        server_config
          .environment
          .iter()
          .map(|(key, value)| (key.clone(), value.clone())),
      )
      .with_initialize_options(server_config.config.clone())
      .with_initialize_timeout(Duration::from_secs(server_config.timeout));

    if command_in_path(server.command()) {
      return Some(server);
    }
    if fallback.is_none() {
      fallback = Some(server);
    }
  }

  fallback
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

fn lsp_file_watch_latency() -> Duration {
  Duration::from_millis(120)
}

fn lsp_self_save_suppress_window() -> Duration {
  Duration::from_millis(500)
}

fn file_change_type_for_path_event(kind: PathEventKind) -> FileChangeType {
  match kind {
    PathEventKind::Created => FileChangeType::Created,
    PathEventKind::Changed => FileChangeType::Changed,
    PathEventKind::Removed => FileChangeType::Deleted,
  }
}

/// FFI-safe app wrapper with editor management.
pub struct App {
  inner:                      LibApp,
  dispatch:                   DefaultDispatchStatic<App>,
  keymaps:                    Keymaps,
  command_registry:           CommandRegistry<App>,
  states:                     HashMap<LibEditorId, EditorState>,
  file_paths:                 HashMap<LibEditorId, PathBuf>,
  vcs_provider:               DiffProviderRegistry,
  vcs_diff_handles:           HashMap<LibEditorId, DiffHandle>,
  active_editor:              Option<LibEditorId>,
  should_quit:                bool,
  registers:                  Registers,
  last_motion:                Option<Motion>,
  lsp_runtime:                LspRuntime,
  lsp_ready:                  bool,
  lsp_document:               Option<LspDocumentSyncState>,
  lsp_statusline:             LspStatuslineState,
  lsp_spinner_index:          usize,
  lsp_spinner_last_tick:      Instant,
  lsp_active_progress_tokens: HashSet<String>,
  lsp_watched_file:           Option<LspWatchedFileState>,
  lsp_pending_requests:       HashMap<u64, PendingLspRequestKind>,
  diagnostics:                DiagnosticsState,
  ui_theme:                   Theme,
  loader:                     Option<Arc<Loader>>,
}

impl App {
  pub fn new() -> Self {
    let dispatch = config_build_dispatch::<App>();
    let ui_theme = select_ui_theme();
    let loader = match init_loader(&ui_theme) {
      Ok(loader) => Some(Arc::new(loader)),
      Err(error) => {
        eprintln!("Warning: syntax highlighting unavailable in FFI: {error}");
        None
      },
    };
    let workspace_root = env::current_dir()
      .ok()
      .map(|path| the_loader::find_workspace_in(path).0)
      .unwrap_or_else(|| the_loader::find_workspace().0);
    let lsp_runtime = LspRuntime::new(
      LspRuntimeConfig::new(workspace_root)
        .with_restart_policy(true, Duration::from_millis(250))
        .with_restart_limits(6, Duration::from_secs(30))
        .with_request_policy(Duration::from_secs(8), 1),
    );

    Self {
      inner: LibApp::default(),
      dispatch,
      keymaps: config_build_keymaps(),
      command_registry: CommandRegistry::new(),
      states: HashMap::new(),
      file_paths: HashMap::new(),
      vcs_provider: DiffProviderRegistry::default(),
      vcs_diff_handles: HashMap::new(),
      active_editor: None,
      should_quit: false,
      registers: Registers::new(),
      last_motion: None,
      lsp_runtime,
      lsp_ready: false,
      lsp_document: None,
      lsp_statusline: LspStatuslineState::off(Some("unavailable".into())),
      lsp_spinner_index: 0,
      lsp_spinner_last_tick: Instant::now(),
      lsp_active_progress_tokens: HashSet::new(),
      lsp_watched_file: None,
      lsp_pending_requests: HashMap::new(),
      diagnostics: DiagnosticsState::default(),
      ui_theme,
      loader,
    }
  }

  pub fn create_editor(
    &mut self,
    text: &str,
    viewport: ffi::Rect,
    scroll: ffi::Position,
  ) -> ffi::EditorId {
    let view = ViewState::new(viewport.to_lib(), scroll.to_lib());
    let id = self.inner.create_editor(Rope::from_str(text), view);
    self
      .states
      .insert(id, EditorState::new(self.loader.clone()));
    self.active_editor.get_or_insert(id);
    ffi::EditorId::from(id)
  }

  pub fn remove_editor(&mut self, id: ffi::EditorId) -> bool {
    let Some(id) = id.to_lib() else {
      return false;
    };
    let removed = self.inner.remove_editor(id).is_some();
    if removed {
      self.states.remove(&id);
      self.file_paths.remove(&id);
      self.vcs_diff_handles.remove(&id);
      if self.active_editor == Some(id) {
        self.active_editor = None;
        self.lsp_close_current_document();
        let _ = self.lsp_runtime.shutdown();
        self.lsp_ready = false;
        self.lsp_document = None;
        self.lsp_watched_file = None;
        self.lsp_active_progress_tokens.clear();
        self.lsp_pending_requests.clear();
        self.set_lsp_status(LspStatusPhase::Off, Some("stopped".into()));
      }
    }
    removed
  }

  pub fn set_viewport(&mut self, id: ffi::EditorId, viewport: ffi::Rect) -> bool {
    let _ = self.activate(id);
    let Some(editor) = self.editor_mut(id) else {
      return false;
    };
    editor.view_mut().viewport = viewport.to_lib();
    true
  }

  pub fn set_scroll(&mut self, id: ffi::EditorId, scroll: ffi::Position) -> bool {
    let _ = self.activate(id);
    let Some(editor) = self.editor_mut(id) else {
      return false;
    };
    editor.view_mut().scroll = scroll.to_lib();
    true
  }

  pub fn set_file_path(&mut self, id: ffi::EditorId, path: &str) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    if path.is_empty() {
      DefaultContext::set_file_path(self, None);
    } else {
      DefaultContext::set_file_path(self, Some(PathBuf::from(path)));
    }
    true
  }

  pub fn set_active_cursor(&mut self, id: ffi::EditorId, cursor_id: u64) -> bool {
    let _ = self.activate(id);
    let Some(editor) = self.editor_mut(id) else {
      return false;
    };
    let Some(cursor_id) = NonZeroU64::new(cursor_id).map(CursorId::new) else {
      return false;
    };
    let has_cursor = editor
      .document()
      .selection()
      .cursor_ids()
      .iter()
      .any(|id| id.get() == cursor_id.get());
    if !has_cursor {
      return false;
    }
    editor.view_mut().active_cursor = Some(cursor_id);
    true
  }

  pub fn clear_active_cursor(&mut self, id: ffi::EditorId) -> bool {
    let _ = self.activate(id);
    let Some(editor) = self.editor_mut(id) else {
      return false;
    };
    editor.view_mut().active_cursor = None;
    true
  }

  pub fn cursor_ids(&self, id: ffi::EditorId) -> Vec<u64> {
    let Some(editor) = self.editor(id) else {
      return Vec::new();
    };
    editor
      .document()
      .selection()
      .cursor_ids()
      .iter()
      .map(|id| id.get())
      .collect()
  }

  pub fn render_plan(&mut self, id: ffi::EditorId) -> RenderPlan {
    if self.activate(id).is_none() {
      return RenderPlan::empty();
    }
    let _ = self.poll_background_active();

    let plan = the_default::render_plan(self);
    plan.into()
  }

  pub fn render_plan_with_styles(
    &mut self,
    id: ffi::EditorId,
    styles: ffi::RenderStyles,
  ) -> RenderPlan {
    if self.activate(id).is_none() {
      return RenderPlan::empty();
    }
    let _ = self.poll_background_active();

    let plan = the_default::render_plan_with_styles(self, styles.to_lib());
    plan.into()
  }

  pub fn ui_tree_json(&mut self, id: ffi::EditorId) -> String {
    if self.activate(id).is_none() {
      return "{}".to_string();
    }
    let _ = self.poll_background_active();

    let tree = the_default::ui_tree(self);
    serde_json::to_string(&tree).unwrap_or_else(|_| "{}".to_string())
  }

  pub fn pending_keys_json(&self, _id: ffi::EditorId) -> String {
    let pending = self.keymaps.pending();
    if pending.is_empty() {
      return "[]".to_string();
    }
    let keys: Vec<String> = pending.iter().map(ToString::to_string).collect();
    serde_json::to_string(&keys).unwrap_or_else(|_| "[]".to_string())
  }

  pub fn pending_key_hints_json(&self, id: ffi::EditorId) -> String {
    let Some(id) = id.to_lib() else {
      return "null".to_string();
    };
    let Some(state) = self.states.get(&id) else {
      return "null".to_string();
    };
    let Some(snapshot) = self.keymaps.pending_hints(state.mode) else {
      return "null".to_string();
    };

    let pending = snapshot
      .pending
      .iter()
      .map(ToString::to_string)
      .collect::<Vec<_>>();
    let options = snapshot
      .options
      .iter()
      .map(|option| {
        serde_json::json!({
          "key": option.key.to_string(),
          "label": option.label,
          "kind": option.kind.as_str(),
        })
      })
      .collect::<Vec<_>>();

    serde_json::json!({
      "pending": pending,
      "scope": snapshot.scope,
      "options": options,
    })
    .to_string()
  }

  pub fn message_snapshot_json(&mut self, id: ffi::EditorId) -> String {
    if self.activate(id).is_none() {
      return "{}".to_string();
    }

    let snapshot = self.active_state_ref().messages.snapshot();
    serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string())
  }

  pub fn message_events_since_json(&mut self, id: ffi::EditorId, seq: u64) -> String {
    if self.activate(id).is_none() {
      return "[]".to_string();
    }

    let events = self.active_state_ref().messages.events_since(seq);
    serde_json::to_string(&events).unwrap_or_else(|_| "[]".to_string())
  }

  pub fn ui_event_json(&mut self, id: ffi::EditorId, event_json: &str) -> bool {
    if self.activate(id).is_none() {
      return false;
    }

    let Ok(event) = serde_json::from_str::<the_lib::render::UiEvent>(event_json) else {
      return false;
    };

    let outcome = the_default::ui_event(self, event);
    outcome.handled
  }

  fn build_render_plan_with_styles_impl(
    &mut self,
    styles: RenderStyles,
  ) -> the_lib::render::RenderPlan {
    let _ = self.poll_active_syntax_parse_results();

    let (
      mut text_fmt,
      gutter_config,
      diff_signs,
      inline_annotations,
      overlay_annotations,
      allow_cache_refresh,
    ) = {
      let state = self.active_state_ref();
      (
        state.text_format.clone(),
        state.gutter_config.clone(),
        state.gutter_diff_signs.clone(),
        state.inline_annotations.clone(),
        state.overlay_annotations.clone(),
        state
          .syntax_parse_highlight_state
          .allow_cache_refresh(&state.syntax_parse_lifecycle),
      )
    };
    let mut highlight_cache = {
      let state = self.active_state_mut();
      std::mem::take(&mut state.highlight_cache)
    };
    let loader = self.loader.clone();
    let diagnostics_by_line = self.active_diagnostics_by_line();
    let diagnostic_styles = render_diagnostic_styles_from_theme(&self.ui_theme);
    let diff_styles = render_diff_styles_from_theme(&self.ui_theme);

    let mut plan = {
      let editor = self.active_editor_mut();
      let view = editor.view();

      let mut annotations = TextAnnotations::default();
      if !inline_annotations.is_empty() {
        let _ = annotations.add_inline_annotations(&inline_annotations, None);
      }
      if !overlay_annotations.is_empty() {
        let _ = annotations.add_overlay(&overlay_annotations, None);
      }

      let (doc, cache) = editor.document_and_cache();
      let gutter_width = gutter_width_for_document(doc, view.viewport.width, &gutter_config);
      text_fmt.viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1);
      if let (Some(loader), Some(syntax)) = (loader.as_deref(), doc.syntax()) {
        let line_range = view.scroll.row..(view.scroll.row + view.viewport.height as usize);
        let mut adapter = SyntaxHighlightAdapter::new(
          doc.text().slice(..),
          syntax,
          loader,
          &mut highlight_cache,
          line_range,
          doc.version(),
          doc.syntax_version(),
          allow_cache_refresh,
        );
        build_plan(
          doc,
          view,
          &text_fmt,
          &gutter_config,
          &mut annotations,
          &mut adapter,
          cache,
          styles,
        )
      } else {
        let mut highlights = NoHighlights;
        build_plan(
          doc,
          view,
          &text_fmt,
          &gutter_config,
          &mut annotations,
          &mut highlights,
          cache,
          styles,
        )
      }
    };
    apply_diagnostic_gutter_markers(&mut plan, &diagnostics_by_line, diagnostic_styles);
    apply_diff_gutter_markers(&mut plan, &diff_signs, diff_styles);

    self.active_state_mut().highlight_cache = highlight_cache;
    plan
  }

  pub fn text(&self, id: ffi::EditorId) -> String {
    self
      .editor(id)
      .map(|editor| editor.document().text().to_string())
      .unwrap_or_default()
  }

  pub fn mode(&self, id: ffi::EditorId) -> u8 {
    let Some(id) = id.to_lib() else {
      return mode_to_u8(Mode::Normal);
    };
    self
      .states
      .get(&id)
      .map(|state| mode_to_u8(state.mode))
      .unwrap_or(mode_to_u8(Mode::Normal))
  }

  pub fn theme_highlight_style(&self, highlight: u32) -> ffi::Style {
    let idx = highlight as usize;
    if idx >= self.ui_theme.scopes().len() {
      return ffi::Style::default();
    }
    ffi::Style::from(self.ui_theme.highlight(Highlight::new(highlight)))
  }

  pub fn command_palette_is_open(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    self.active_state_ref().command_palette.is_open
  }

  pub fn command_palette_query(&mut self, id: ffi::EditorId) -> String {
    if self.activate(id).is_none() {
      return String::new();
    }
    self.active_state_ref().command_palette.query.clone()
  }

  pub fn command_palette_layout(&mut self, id: ffi::EditorId) -> u8 {
    if self.activate(id).is_none() {
      return 0;
    }
    match self.active_state_ref().command_palette_style.layout {
      CommandPaletteLayout::Floating => 0,
      CommandPaletteLayout::Bottom => 1,
      CommandPaletteLayout::Top => 2,
      CommandPaletteLayout::Custom => 3,
    }
  }

  pub fn command_palette_filtered_count(&mut self, id: ffi::EditorId) -> usize {
    if self.activate(id).is_none() {
      return 0;
    }
    command_palette_filtered_indices(&self.active_state_ref().command_palette).len()
  }

  pub fn command_palette_filtered_selected_index(&mut self, id: ffi::EditorId) -> i64 {
    if self.activate(id).is_none() {
      return -1;
    }
    command_palette_selected_filtered_index(&self.active_state_ref().command_palette)
      .map(|idx| idx as i64)
      .unwrap_or(-1)
  }

  pub fn command_palette_filtered_title(&mut self, id: ffi::EditorId, index: usize) -> String {
    if self.activate(id).is_none() {
      return String::new();
    }
    let state = self.active_state_ref();
    let filtered = command_palette_filtered_indices(&state.command_palette);
    filtered
      .get(index)
      .and_then(|idx| state.command_palette.items.get(*idx))
      .map(|item| item.title.clone())
      .unwrap_or_default()
  }

  pub fn command_palette_filtered_subtitle(&mut self, id: ffi::EditorId, index: usize) -> String {
    if self.activate(id).is_none() {
      return String::new();
    }
    let state = self.active_state_ref();
    let filtered = command_palette_filtered_indices(&state.command_palette);
    filtered
      .get(index)
      .and_then(|idx| state.command_palette.items.get(*idx))
      .and_then(|item| item.subtitle.clone())
      .unwrap_or_default()
  }

  pub fn command_palette_filtered_description(
    &mut self,
    id: ffi::EditorId,
    index: usize,
  ) -> String {
    if self.activate(id).is_none() {
      return String::new();
    }
    let state = self.active_state_ref();
    let filtered = command_palette_filtered_indices(&state.command_palette);
    filtered
      .get(index)
      .and_then(|idx| state.command_palette.items.get(*idx))
      .and_then(|item| item.description.clone())
      .unwrap_or_default()
  }

  pub fn command_palette_filtered_shortcut(&mut self, id: ffi::EditorId, index: usize) -> String {
    if self.activate(id).is_none() {
      return String::new();
    }
    let state = self.active_state_ref();
    let filtered = command_palette_filtered_indices(&state.command_palette);
    filtered
      .get(index)
      .and_then(|idx| state.command_palette.items.get(*idx))
      .and_then(|item| item.shortcut.clone())
      .unwrap_or_default()
  }

  pub fn command_palette_filtered_badge(&mut self, id: ffi::EditorId, index: usize) -> String {
    if self.activate(id).is_none() {
      return String::new();
    }
    let state = self.active_state_ref();
    let filtered = command_palette_filtered_indices(&state.command_palette);
    filtered
      .get(index)
      .and_then(|idx| state.command_palette.items.get(*idx))
      .and_then(|item| item.badge.clone())
      .unwrap_or_default()
  }

  pub fn command_palette_filtered_leading_icon(
    &mut self,
    id: ffi::EditorId,
    index: usize,
  ) -> String {
    if self.activate(id).is_none() {
      return String::new();
    }
    let state = self.active_state_ref();
    let filtered = command_palette_filtered_indices(&state.command_palette);
    filtered
      .get(index)
      .and_then(|idx| state.command_palette.items.get(*idx))
      .and_then(|item| item.leading_icon.clone())
      .unwrap_or_default()
  }

  pub fn command_palette_filtered_leading_color(
    &mut self,
    id: ffi::EditorId,
    index: usize,
  ) -> ffi::Color {
    if self.activate(id).is_none() {
      return ffi::Color { kind: 0, value: 0 };
    }
    let state = self.active_state_ref();
    let filtered = command_palette_filtered_indices(&state.command_palette);
    filtered
      .get(index)
      .and_then(|idx| state.command_palette.items.get(*idx))
      .and_then(|item| item.leading_color)
      .map(ffi::Color::from)
      .unwrap_or(ffi::Color { kind: 0, value: 0 })
  }

  pub fn command_palette_filtered_symbol_count(
    &mut self,
    id: ffi::EditorId,
    index: usize,
  ) -> usize {
    if self.activate(id).is_none() {
      return 0;
    }
    let state = self.active_state_ref();
    let filtered = command_palette_filtered_indices(&state.command_palette);
    filtered
      .get(index)
      .and_then(|idx| state.command_palette.items.get(*idx))
      .and_then(|item| item.symbols.as_ref())
      .map(|symbols| symbols.len())
      .unwrap_or(0)
  }

  pub fn command_palette_filtered_symbol(
    &mut self,
    id: ffi::EditorId,
    index: usize,
    symbol_index: usize,
  ) -> String {
    if self.activate(id).is_none() {
      return String::new();
    }
    let state = self.active_state_ref();
    let filtered = command_palette_filtered_indices(&state.command_palette);
    filtered
      .get(index)
      .and_then(|idx| state.command_palette.items.get(*idx))
      .and_then(|item| item.symbols.as_ref())
      .and_then(|symbols| symbols.get(symbol_index))
      .cloned()
      .unwrap_or_default()
  }

  pub fn command_palette_select_filtered(&mut self, id: ffi::EditorId, index: usize) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let filtered = command_palette_filtered_indices(&self.active_state_ref().command_palette);
    let Some(item_idx) = filtered.get(index).copied() else {
      return false;
    };
    let palette = &mut self.active_state_mut().command_palette;
    palette.selected = Some(item_idx);
    self.request_render();
    true
  }

  pub fn command_palette_submit_filtered(&mut self, id: ffi::EditorId, index: usize) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let filtered = command_palette_filtered_indices(&self.active_state_ref().command_palette);
    let Some(item_idx) = filtered.get(index).copied() else {
      return false;
    };
    let command_name = {
      let palette = &self.active_state_ref().command_palette;
      palette
        .items
        .get(item_idx)
        .map(|item| item.title.clone())
        .unwrap_or_default()
    };

    if command_name.is_empty() {
      return false;
    }

    let registry = self.command_registry_ref() as *const CommandRegistry<App>;
    let result = unsafe { (&*registry).execute(self, &command_name, "", CommandEvent::Validate) };

    match result {
      Ok(()) => {
        self.set_mode(Mode::Normal);
        self.command_prompt_mut().clear();
        let palette = self.command_palette_mut();
        palette.is_open = false;
        palette.query.clear();
        palette.selected = None;
        self.request_render();
        true
      },
      Err(err) => {
        self.command_prompt_mut().error = Some(err.to_string());
        self.request_render();
        false
      },
    }
  }

  pub fn command_palette_close(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    self.set_mode(Mode::Normal);
    self.command_prompt_mut().clear();
    let palette = self.command_palette_mut();
    palette.is_open = false;
    palette.query.clear();
    palette.selected = None;
    self.request_render();
    true
  }

  pub fn command_palette_set_query(&mut self, id: ffi::EditorId, query: &str) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let input = query.to_string();
    let completions = self
      .command_registry_ref()
      .complete_command_line(self, &input);

    let prompt = self.command_prompt_mut();
    prompt.input = input.clone();
    prompt.cursor = prompt.input.len();
    prompt.completions = completions;
    prompt.error = None;

    let palette = self.command_palette_mut();
    palette.query = input;
    palette.selected = command_palette_default_selected(palette);
    self.request_render();
    true
  }

  pub fn search_prompt_set_query(&mut self, id: ffi::EditorId, query: &str) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let prompt = self.search_prompt_mut();
    prompt.query = query.to_string();
    prompt.cursor = query.len();
    prompt.selected = None;
    prompt.error = None;
    update_search_preview(self);
    self.request_render();
    true
  }

  pub fn search_prompt_close(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    if let Some(selection) = self.search_prompt_mut().original_selection.take() {
      let _ = self.editor().document_mut().set_selection(selection);
    }
    self.search_prompt_mut().clear();
    self.request_render();
    true
  }

  pub fn search_prompt_submit(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    if finalize_search(self) {
      self.search_prompt_mut().clear();
    }
    self.request_render();
    true
  }

  // ---- File picker methods ----

  pub fn file_picker_set_query(&mut self, id: ffi::EditorId, query: &str) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let picker = self.file_picker_mut();
    if !picker.active {
      return false;
    }
    let old_query = picker.query.clone();
    picker.query = query.to_string();
    picker.cursor = query.len();
    file_picker_handle_query_change(picker, &old_query);
    self.request_render();
    true
  }

  pub fn file_picker_submit(&mut self, id: ffi::EditorId, index: usize) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    if !self.file_picker().active {
      return false;
    }
    select_file_picker_index(self, index);
    submit_file_picker(self);
    true
  }

  pub fn file_picker_close(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    if !self.file_picker().active {
      return false;
    }
    close_file_picker(self);
    true
  }

  pub fn file_picker_snapshot_json(&mut self, id: ffi::EditorId, max_items: usize) -> String {
    if self.activate(id).is_none() {
      return "{}".to_string();
    }
    let picker = self.file_picker_mut();
    file_picker_poll_scan_results(picker);
    file_picker_refresh_matcher_state(picker);

    let picker = self.file_picker();
    if !picker.active {
      return r#"{"active":false}"#.to_string();
    }

    let matched_count = picker.matched_count();
    let total_count = picker.total_count();
    let scanning = picker.scanning || picker.matcher_running;
    let root_display = picker
      .root
      .file_name()
      .map(|n| n.to_string_lossy().into_owned())
      .unwrap_or_default();

    let limit = max_items.min(matched_count);
    let mut items = Vec::with_capacity(limit);
    let mut match_indices = Vec::new();
    for i in 0..limit {
      if let Some(item) = picker.matched_item_with_match_indices(i, &mut match_indices) {
        items.push(serde_json::json!({
          "display": item.display,
          "is_dir": item.is_dir,
          "icon": item.icon,
          "match_indices": &match_indices,
        }));
      }
    }

    let snapshot = serde_json::json!({
      "active": true,
      "query": picker.query,
      "matched_count": matched_count,
      "total_count": total_count,
      "scanning": scanning,
      "root": root_display,
      "items": items,
    });

    snapshot.to_string()
  }

  pub fn take_should_quit(&mut self) -> bool {
    let should_quit = self.should_quit;
    self.should_quit = false;
    should_quit
  }

  pub fn poll_background(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    self.poll_background_active()
  }

  fn poll_background_active(&mut self) -> bool {
    let mut changed = false;
    if self.poll_active_syntax_parse_results() {
      changed = true;
    }
    if self.poll_lsp_events() {
      changed = true;
    }
    if self.poll_lsp_file_watch() {
      changed = true;
    }
    if self.tick_lsp_statusline() {
      changed = true;
    }
    if changed {
      self.request_render();
    }
    changed
  }

  fn lsp_runtime_config_for_active_file(&self) -> (LspRuntimeConfig, bool) {
    let active_path = self.file_path();
    let workspace_root = active_path
      .and_then(|path| {
        let absolute = if path.is_absolute() {
          path.to_path_buf()
        } else {
          env::current_dir().ok()?.join(path)
        };
        let anchor = absolute.parent().map(|parent| parent.to_path_buf())?;
        Some(the_loader::find_workspace_in(anchor).0)
      })
      .unwrap_or_else(|| the_loader::find_workspace().0);

    let mut config = LspRuntimeConfig::new(workspace_root)
      .with_restart_policy(true, Duration::from_millis(250))
      .with_restart_limits(6, Duration::from_secs(30))
      .with_request_policy(Duration::from_secs(8), 1);
    let server_from_language = active_path.and_then(|path| {
      self
        .loader
        .as_deref()
        .and_then(|loader| lsp_server_from_language_config(loader, path))
    });
    if let Some(server) = server_from_language.or_else(lsp_server_from_env) {
      config = config.with_server(server);
    }
    let configured = config.server().is_some();
    (config, configured)
  }

  fn refresh_lsp_runtime_for_active_file(&mut self) {
    self.lsp_close_current_document();
    self.lsp_ready = false;
    self.lsp_spinner_index = 0;
    self.diagnostics.clear();
    self.lsp_active_progress_tokens.clear();
    self.lsp_pending_requests.clear();
    let _ = self.lsp_runtime.shutdown();

    let active_path = self.file_path().map(Path::to_path_buf);
    self.lsp_document =
      active_path.and_then(|path| build_lsp_document_state(&path, self.loader.as_deref()));
    self.lsp_sync_watched_file_state();

    let (config, configured) = self.lsp_runtime_config_for_active_file();
    self.lsp_runtime = LspRuntime::new(config);

    if configured {
      self.set_lsp_status(LspStatusPhase::Starting, Some("starting".into()));
      if let Err(err) = self.lsp_runtime.start() {
        self.set_lsp_status_error(&err.to_string());
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Error,
          format!("failed to start lsp server: {err}"),
        );
      }
    } else {
      self.set_lsp_status(LspStatusPhase::Off, Some("unavailable".into()));
    }
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
    self.set_lsp_status(LspStatusPhase::Error, Some(summarize_lsp_error(message)));
  }

  fn tick_lsp_statusline(&mut self) -> bool {
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

  fn publish_lsp_message(
    &mut self,
    level: the_lib::messages::MessageLevel,
    text: impl Into<String>,
  ) {
    if self.active_editor.is_none() {
      return;
    }
    self
      .active_state_mut()
      .messages
      .publish(level, Some("lsp".into()), text.into());
  }

  fn poll_lsp_events(&mut self) -> bool {
    let mut changed = false;
    while let Some(event) = self.lsp_runtime.try_recv_event() {
      match event {
        LspEvent::Started { .. } => {
          if self.lsp_runtime.config().server().is_none() {
            self.set_lsp_status(LspStatusPhase::Off, Some("unavailable".into()));
          } else {
            self.set_lsp_status(LspStatusPhase::Starting, Some("starting".into()));
          }
          changed = true;
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
            changed = true;
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
          changed = true;
        },
        LspEvent::RequestDispatched { method, .. } => {
          if method == "initialize" {
            self.set_lsp_status(LspStatusPhase::Initializing, Some("initializing".into()));
            changed = true;
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
          changed = true;
        },
        LspEvent::Progress { progress } => {
          match progress.kind {
            LspProgressKind::Begin => {
              let text =
                format_lsp_progress_text(progress.title.as_deref(), progress.message.as_deref());
              self.lsp_active_progress_tokens.insert(progress.token);
              self.set_lsp_status(LspStatusPhase::Busy, Some(text.clone()));
              self.publish_lsp_message(the_lib::messages::MessageLevel::Info, text);
              changed = true;
            },
            LspProgressKind::End => {
              self.lsp_active_progress_tokens.remove(&progress.token);
              if self.lsp_ready && self.lsp_active_progress_tokens.is_empty() {
                self.set_lsp_status(LspStatusPhase::Ready, None);
                changed = true;
              }
              if let Some(message) = progress.message.and_then(non_empty_trimmed) {
                self.publish_lsp_message(the_lib::messages::MessageLevel::Info, message);
                changed = true;
              }
            },
            LspProgressKind::Report => {
              if self.lsp_active_progress_tokens.contains(&progress.token) {
                let text =
                  format_lsp_progress_text(progress.title.as_deref(), progress.message.as_deref());
                self.set_lsp_status(LspStatusPhase::Busy, Some(text));
                changed = true;
              }
            },
          }
        },
        LspEvent::Error(message) => {
          self.set_lsp_status_error(&message);
          self.publish_lsp_message(the_lib::messages::MessageLevel::Error, message);
          changed = true;
        },
        LspEvent::RequestTimedOut { id, method } => {
          let text = if let Some(kind) = self.lsp_pending_requests.remove(&id) {
            format!("lsp {} timed out", kind.label())
          } else {
            format!("lsp {method} timed out")
          };
          self.publish_lsp_message(the_lib::messages::MessageLevel::Warning, text);
          self.set_lsp_status(LspStatusPhase::Error, Some("request timeout".into()));
          changed = true;
        },
        LspEvent::RpcMessage { message } => {
          changed |= self.handle_lsp_rpc_message(message);
        },
        LspEvent::DiagnosticsPublished { diagnostics } => {
          let diagnostic_uri = diagnostics.uri.clone();
          let active_uri = self.lsp_document.as_ref().map(|state| state.uri.as_str());
          let previous = self
            .diagnostics
            .document(&diagnostic_uri)
            .map(|document| document.counts())
            .unwrap_or_default();
          let next = self.diagnostics.apply_document(diagnostics);
          if active_uri.is_some_and(|uri| uri == diagnostic_uri) && previous != next {
            self.publish_lsp_diagnostic_message(next);
            changed = true;
          }
        },
        _ => {},
      }
    }

    changed
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
      the_lib::messages::MessageLevel::Error
    } else if counts.warnings > 0 {
      the_lib::messages::MessageLevel::Warning
    } else {
      the_lib::messages::MessageLevel::Info
    };
    self.publish_lsp_message(level, text);
  }

  fn active_diagnostics_by_line(&self) -> BTreeMap<usize, DiagnosticSeverity> {
    let Some(state) = self.lsp_document.as_ref().filter(|state| state.opened) else {
      return BTreeMap::new();
    };
    let Some(document) = self.diagnostics.document(&state.uri) else {
      return BTreeMap::new();
    };

    let mut out = BTreeMap::new();
    for diagnostic in &document.diagnostics {
      let line = diagnostic.range.start.line as usize;
      let severity = diagnostic.severity.unwrap_or(DiagnosticSeverity::Warning);
      match out.get(&line).copied() {
        Some(prev) if diagnostic_severity_rank(prev) >= diagnostic_severity_rank(severity) => {},
        _ => {
          out.insert(line, severity);
        },
      }
    }
    out
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

    if self
      .lsp_document
      .as_ref()
      .map(|state| state.uri.as_str())
      .is_some_and(|uri| uri != kind.uri())
    {
      return false;
    }

    if let Some(error) = response.error {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Error,
        format!("lsp {} failed: {}", kind.label(), error.message),
      );
      return true;
    }

    match kind {
      PendingLspRequestKind::GotoDefinition { .. } => {
        let locations = match parse_locations_response(response.result.as_ref()) {
          Ok(locations) => locations,
          Err(err) => {
            self.publish_lsp_message(
              the_lib::messages::MessageLevel::Error,
              format!("failed to parse goto-definition response: {err}"),
            );
            return true;
          },
        };
        self.apply_locations_result("definition", locations)
      },
    }
  }

  fn apply_locations_result(&mut self, label: &str, locations: Vec<LspLocation>) -> bool {
    if locations.is_empty() {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Info,
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
      self.publish_lsp_message(the_lib::messages::MessageLevel::Info, text);
    }
    jumped
  }

  fn jump_to_location(&mut self, location: &LspLocation) -> bool {
    let Some(path) = path_for_file_uri(&location.uri) else {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
        format!("unsupported location URI: {}", location.uri),
      );
      return true;
    };

    if self
      .file_path()
      .is_none_or(|current| current != path.as_path())
      && let Err(err) = <Self as DefaultContext>::open_file(self, &path)
    {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Error,
        format!("failed to open location '{}': {err}", path.display()),
      );
      return true;
    }

    let cursor = {
      let doc = self.active_editor_ref().document();
      utf16_position_to_char_idx(
        doc.text(),
        location.range.start.line,
        location.range.start.character,
      )
    };

    let _ = self
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::point(cursor));
    self.active_editor_mut().view_mut().scroll = LibPosition::new(
      (location.range.start.line as usize).saturating_sub(self.scrolloff()),
      0,
    );
    self.request_render();
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

    let doc = self.active_editor_ref().document();
    let range = doc.selection().ranges().first().copied()?;
    let cursor = range.cursor(doc.text().slice(..));
    let (line, character) = char_idx_to_utf16_position(doc.text(), cursor);

    Some((state.uri, LspPosition { line, character }))
  }

  fn dispatch_lsp_request(
    &mut self,
    method: &'static str,
    params: serde_json::Value,
    pending: PendingLspRequestKind,
  ) {
    match self.lsp_runtime.send_request(method, Some(params)) {
      Ok(request_id) => {
        self.lsp_pending_requests.insert(request_id, pending);
      },
      Err(err) => {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Error,
          format!("failed to dispatch {method}: {err}"),
        );
      },
    }
  }

  fn lsp_sync_kind(&self) -> Option<the_lsp::TextDocumentSyncKind> {
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
    let text = self.active_editor_ref().document().text().clone();
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

  fn lsp_send_did_change(&mut self, old_text: &Rope, changes: &the_lib::transaction::ChangeSet) {
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
    let new_text = self.active_editor_ref().document().text().clone();
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
        path: state.path.clone(),
        uri: state.uri.clone(),
        events_rx,
        _watch_handle: watch_handle,
        suppress_until: None,
      }
    });
  }

  fn poll_lsp_file_watch(&mut self) -> bool {
    let lsp_ready = self.lsp_ready;

    let mut watcher_disconnected = false;
    let mut pending_changes = Vec::new();
    let watched_uri;
    let watched_path;

    {
      let Some(watch) = self.lsp_watched_file.as_mut() else {
        trace_file_watch_event("consumer_poll_skip", "client=ffi reason=no_watch_state");
        return false;
      };

      watched_uri = watch.uri.clone();
      watched_path = watch.path.clone();

      loop {
        match watch.events_rx.try_recv() {
          Ok(batch) => {
            if batch.is_empty() {
              continue;
            }

            if let Some(until) = watch.suppress_until {
              if Instant::now() <= until {
                trace_file_watch_event(
                  "consumer_suppress_drop",
                  format!(
                    "client=ffi path={} reason=self_save_window",
                    watch.path.display()
                  ),
                );
                watch.suppress_until = None;
                continue;
              }
              watch.suppress_until = None;
            }

            let mut batch_change = None;
            for event in batch {
              batch_change = Some(file_change_type_for_path_event(event.kind));
            }
            if let Some(change_type) = batch_change {
              pending_changes.push(change_type);
            }
          },
          Err(TryRecvError::Empty) => break,
          Err(TryRecvError::Disconnected) => {
            watcher_disconnected = true;
            break;
          },
        }
      }
    }

    if watcher_disconnected {
      trace_file_watch_event(
        "consumer_watcher_disconnected",
        format!("client=ffi path={}", watched_path.display()),
      );
      self.lsp_sync_watched_file_state();
      return false;
    }

    if pending_changes.is_empty() {
      return false;
    }

    trace_file_watch_event(
      "consumer_changes_collected",
      format!(
        "client=ffi path={} changes={}",
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
          "client=ffi path={} changes={}",
          watched_path.display(),
          pending_changes.len()
        ),
      );
    } else {
      trace_file_watch_event(
        "consumer_lsp_notify_skipped",
        format!(
          "client=ffi path={} reason=lsp_not_ready changes={}",
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
        trace_file_watch_event(
          "consumer_external_deleted",
          format!("client=ffi path={}", watched_path.display()),
        );
        if self.active_editor.is_some() {
          self.active_state_mut().messages.publish(
            the_lib::messages::MessageLevel::Warning,
            Some("watch".into()),
            format!("file deleted on disk: {label}"),
          );
          self.request_render();
        }
        true
      },
      FileChangeType::Created | FileChangeType::Changed => {
        if self.active_editor_ref().document().flags().modified {
          trace_file_watch_event(
            "consumer_external_changed_dirty",
            format!("client=ffi path={}", watched_path.display()),
          );
          if self.active_editor.is_some() {
            self.active_state_mut().messages.publish(
              the_lib::messages::MessageLevel::Warning,
              Some("watch".into()),
              format!(
                "file changed on disk: {label} (buffer has unsaved changes; run :reload force to discard them)"
              ),
            );
            self.request_render();
          }
          return true;
        }

        match <Self as DefaultContext>::open_file(self, watched_path) {
          Ok(()) => {
            trace_file_watch_event(
              "consumer_external_reload_ok",
              format!("client=ffi path={}", watched_path.display()),
            );
            if self.active_editor.is_some() {
              self.active_state_mut().messages.publish(
                the_lib::messages::MessageLevel::Info,
                Some("watch".into()),
                format!("reloaded from disk: {label}"),
              );
              self.request_render();
            }
            true
          },
          Err(err) => {
            trace_file_watch_event(
              "consumer_external_reload_err",
              format!("client=ffi path={} err={err}", watched_path.display()),
            );
            if self.active_editor.is_some() {
              self.active_state_mut().messages.publish(
                the_lib::messages::MessageLevel::Error,
                Some("watch".into()),
                format!("failed to reload '{label}': {err}"),
              );
              self.request_render();
            }
            true
          },
        }
      },
    }
  }

  pub fn handle_key(&mut self, id: ffi::EditorId, event: ffi::KeyEvent) -> bool {
    if self.activate(id).is_none() {
      return false;
    }

    let key_event = key_event_from_ffi(event);
    let dispatch = self.dispatch();
    dispatch.pre_on_keypress(self, key_event);
    self.ensure_cursor_visible(id);
    true
  }

  pub fn ensure_cursor_visible(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }

    let scrolloff = self.active_state_ref().scrolloff;
    let soft_wrap = self.active_state_ref().text_format.soft_wrap;

    let Some(editor) = self.editor_mut(id) else {
      return false;
    };

    let doc = editor.document();
    let text = doc.text();
    let selection = doc.selection();
    let Some(range) = selection.ranges().first() else {
      return false;
    };

    let cursor_pos = range.cursor(text.slice(..));
    let cursor_line = text.char_to_line(cursor_pos);
    let cursor_col = cursor_pos - text.line_to_char(cursor_line);

    let view = editor.view();
    let viewport_height = view.viewport.height as usize;
    let viewport_width = view.viewport.width as usize;

    if soft_wrap {
      let mut changed = false;
      let mut new_scroll = view.scroll;
      if let Some(new_row) = the_lib::view::scroll_row_to_keep_visible(
        cursor_line,
        view.scroll.row,
        viewport_height,
        scrolloff,
      ) {
        new_scroll.row = new_row;
        changed = true;
      }
      if view.scroll.col != 0 {
        new_scroll.col = 0;
        changed = true;
      }

      if changed {
        editor.view_mut().scroll = new_scroll;
        return true;
      }

      return false;
    }

    if let Some(new_scroll) = the_lib::view::scroll_to_keep_visible(
      cursor_line,
      cursor_col,
      view.scroll,
      viewport_height,
      viewport_width,
      scrolloff,
    ) {
      editor.view_mut().scroll = new_scroll;
      return true;
    }

    false
  }

  pub fn insert(&mut self, id: ffi::EditorId, text: &str) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let dispatch = self.dispatch();
    for ch in text.chars() {
      dispatch.pre_on_action(self, Command::InsertChar(ch));
    }
    true
  }

  pub fn delete_backward(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let dispatch = self.dispatch();
    dispatch.pre_on_action(self, Command::DeleteChar);
    true
  }

  pub fn delete_forward(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let dispatch = self.dispatch();
    dispatch.pre_on_action(self, Command::delete_char_forward(1));
    true
  }

  pub fn move_left(&mut self, id: ffi::EditorId) {
    if self.activate(id).is_none() {
      return;
    }
    let dispatch = self.dispatch();
    dispatch.pre_on_action(self, Command::Move(CommandDirection::Left));
  }

  pub fn move_right(&mut self, id: ffi::EditorId) {
    if self.activate(id).is_none() {
      return;
    }
    let dispatch = self.dispatch();
    dispatch.pre_on_action(self, Command::Move(CommandDirection::Right));
  }

  pub fn move_up(&mut self, id: ffi::EditorId) {
    if self.activate(id).is_none() {
      return;
    }
    let dispatch = self.dispatch();
    dispatch.pre_on_action(self, Command::Move(CommandDirection::Up));
  }

  pub fn move_down(&mut self, id: ffi::EditorId) {
    if self.activate(id).is_none() {
      return;
    }
    let dispatch = self.dispatch();
    dispatch.pre_on_action(self, Command::Move(CommandDirection::Down));
  }

  pub fn add_cursor_above(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let dispatch = self.dispatch();
    dispatch.pre_on_action(self, Command::AddCursor(CommandDirection::Up));
    true
  }

  pub fn add_cursor_below(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let dispatch = self.dispatch();
    dispatch.pre_on_action(self, Command::AddCursor(CommandDirection::Down));
    true
  }

  pub fn collapse_to_cursor(&mut self, id: ffi::EditorId, cursor_id: u64) -> bool {
    let _ = self.activate(id);
    let Some(editor) = self.editor_mut(id) else {
      return false;
    };
    let Some(cursor_id) = NonZeroU64::new(cursor_id).map(CursorId::new) else {
      return false;
    };
    if collapse_selection(editor.document_mut(), CursorPick::Id(cursor_id)) {
      editor.view_mut().active_cursor = Some(cursor_id);
      true
    } else {
      false
    }
  }

  pub fn collapse_to_first(&mut self, id: ffi::EditorId) -> bool {
    let _ = self.activate(id);
    let Some(editor) = self.editor_mut(id) else {
      return false;
    };
    let pick = CursorPick::First;
    if collapse_selection(editor.document_mut(), pick) {
      if let Some(id) = editor.document().selection().cursor_ids().first().copied() {
        editor.view_mut().active_cursor = Some(id);
      }
      true
    } else {
      false
    }
  }

  fn set_active_editor(&mut self, id: LibEditorId) -> bool {
    if self.inner.editor(id).is_none() {
      return false;
    }
    let changed = self.active_editor != Some(id);
    self.active_editor = Some(id);
    let loader = self.loader.clone();
    self
      .states
      .entry(id)
      .or_insert_with(|| EditorState::new(loader.clone()));
    if changed {
      self.refresh_lsp_runtime_for_active_file();
    }
    true
  }

  fn activate(&mut self, id: ffi::EditorId) -> Option<LibEditorId> {
    let id = id.to_lib()?;
    if self.set_active_editor(id) {
      let _ = self.poll_editor_syntax_parse_results(id);
      Some(id)
    } else {
      None
    }
  }

  fn active_state_mut(&mut self) -> &mut EditorState {
    let id = self.active_editor.expect("active editor not set");
    self
      .states
      .get_mut(&id)
      .expect("missing editor state for active editor")
  }

  fn active_state_ref(&self) -> &EditorState {
    let id = self.active_editor.expect("active editor not set");
    self
      .states
      .get(&id)
      .expect("missing editor state for active editor")
  }

  fn active_editor_ref(&self) -> &the_lib::editor::Editor {
    let id = self.active_editor.expect("active editor not set");
    self
      .inner
      .editor(id)
      .expect("missing editor for active editor id")
  }

  fn active_editor_mut(&mut self) -> &mut the_lib::editor::Editor {
    let id = self.active_editor.expect("active editor not set");
    self
      .inner
      .editor_mut(id)
      .expect("missing editor for active editor id")
  }

  fn editor(&self, id: ffi::EditorId) -> Option<&the_lib::editor::Editor> {
    let id = id.to_lib()?;
    self.inner.editor(id)
  }

  fn editor_mut(&mut self, id: ffi::EditorId) -> Option<&mut the_lib::editor::Editor> {
    let id = id.to_lib()?;
    self.inner.editor_mut(id)
  }

  fn poll_editor_syntax_parse_results(&mut self, id: LibEditorId) -> bool {
    let current_doc_version = {
      let Some(editor) = self.inner.editor(id) else {
        return false;
      };
      editor.document().version()
    };

    let mut drained_results = Vec::new();
    {
      let Some(state) = self.states.get_mut(&id) else {
        return false;
      };
      loop {
        match state.syntax_parse_rx.try_recv() {
          Ok(result) => drained_results.push(result),
          Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
        }
      }
    }

    let mut changed = false;
    for result in drained_results {
      let (apply_result, start_next) = {
        let Some(state) = self.states.get_mut(&id) else {
          return changed;
        };
        let decision = state.syntax_parse_lifecycle.on_result(
          result.request_id,
          result.doc_version,
          current_doc_version,
        );
        (decision.apply, decision.start_next)
      };

      if let Some(next_request) = start_next {
        let tx = {
          let Some(state) = self.states.get(&id) else {
            return changed;
          };
          state.syntax_parse_tx.clone()
        };
        spawn_syntax_parse_request(tx, next_request);
      }

      if !apply_result {
        continue;
      }

      let loader = self.loader.clone();
      let parsed_state = {
        let Some(editor) = self.inner.editor_mut(id) else {
          continue;
        };
        let doc = editor.document_mut();
        match result.syntax {
          Some(syntax) => {
            if let Some(loader) = loader {
              doc.set_syntax_with_loader(syntax, loader);
            } else {
              doc.set_syntax(syntax);
            }
            Some(true)
          },
          None => None,
        }
      };

      if let Some(state) = self.states.get_mut(&id) {
        if parsed_state == Some(true) {
          state.highlight_cache.clear();
          state.syntax_parse_highlight_state.mark_parsed();
          changed = true;
          state.needs_render = true;
        } else {
          state.syntax_parse_highlight_state.mark_interpolated();
        }
      }
    }

    changed
  }

  fn queue_editor_syntax_parse_job(
    &mut self,
    id: LibEditorId,
    doc_version: u64,
    parse_job: SyntaxParseJob,
  ) {
    let start_request = {
      let Some(state) = self.states.get_mut(&id) else {
        return;
      };
      match state.syntax_parse_lifecycle.queue(doc_version, parse_job) {
        QueueParseDecision::Start(request) => Some((state.syntax_parse_tx.clone(), request)),
        QueueParseDecision::Queued(_) => None,
      }
    };

    if let Some((tx, request)) = start_request {
      spawn_syntax_parse_request(tx, request);
    }
  }

  fn poll_active_syntax_parse_results(&mut self) -> bool {
    let Some(id) = self.active_editor else {
      return false;
    };
    self.poll_editor_syntax_parse_results(id)
  }

  fn refresh_editor_syntax(&mut self, id: LibEditorId) {
    let path = self.file_paths.get(&id).cloned();
    let loader = self.loader.clone();
    let mut parsed = false;
    let Some(editor) = self.inner.editor_mut(id) else {
      return;
    };
    let doc = editor.document_mut();
    match (loader.as_ref(), path.as_deref()) {
      (Some(loader), Some(path)) => {
        if let Err(error) = setup_syntax(doc, path, loader) {
          eprintln!(
            "Warning: could not enable syntax for {}: {error}",
            path.display()
          );
          doc.clear_syntax();
        }
        parsed = doc.syntax().is_some();
      },
      _ => {
        doc.clear_syntax();
      },
    }

    if let Some(state) = self.states.get_mut(&id) {
      state.syntax_parse_lifecycle.cancel_pending();
      state.highlight_cache.clear();
      if parsed {
        state.syntax_parse_highlight_state.mark_parsed();
      } else {
        state.syntax_parse_highlight_state.mark_cleared();
      }
    }
  }

  fn clear_vcs_diff_for_editor(&mut self, id: LibEditorId) {
    self.vcs_diff_handles.remove(&id);
    if let Some(state) = self.states.get_mut(&id) {
      state.gutter_diff_signs.clear();
    }
  }

  fn refresh_vcs_diff_base_for_editor(&mut self, id: LibEditorId) {
    let path = self.file_paths.get(&id).cloned();
    let statusline = path
      .as_deref()
      .and_then(|path| self.vcs_provider.get_statusline_info(path))
      .map(|info| info.statusline_text());
    if let Some(state) = self.states.get_mut(&id) {
      state.vcs_statusline = statusline;
      state.needs_render = true;
    }

    let Some(path) = path else {
      self.clear_vcs_diff_for_editor(id);
      return;
    };
    let Some(diff_base) = self.vcs_provider.get_diff_base(&path) else {
      self.clear_vcs_diff_for_editor(id);
      return;
    };
    let Some(editor) = self.inner.editor(id) else {
      self.clear_vcs_diff_for_editor(id);
      return;
    };

    let diff_base = Rope::from_str(String::from_utf8_lossy(&diff_base).as_ref());
    let doc = editor.document().text().clone();
    let handle = DiffHandle::new(diff_base, doc);
    let signs = vcs_gutter_signs(&handle);
    self.vcs_diff_handles.insert(id, handle);
    if let Some(state) = self.states.get_mut(&id) {
      state.gutter_diff_signs = signs;
      state.needs_render = true;
    }
  }

  fn refresh_vcs_diff_document_for_editor(&mut self, id: LibEditorId) {
    let Some(handle) = self.vcs_diff_handles.get(&id) else {
      return;
    };
    let Some(editor) = self.inner.editor(id) else {
      return;
    };
    let _ = handle.update_document(editor.document().text().clone(), true);
    if let Some(state) = self.states.get_mut(&id) {
      state.gutter_diff_signs = vcs_gutter_signs(handle);
    }
  }
}

impl Default for App {
  fn default() -> Self {
    Self::new()
  }
}

impl Drop for App {
  fn drop(&mut self) {
    self.lsp_close_current_document();
    let _ = self.lsp_runtime.shutdown();
  }
}

impl DefaultContext for App {
  fn editor(&mut self) -> &mut the_lib::editor::Editor {
    self.active_editor_mut()
  }

  fn editor_ref(&self) -> &the_lib::editor::Editor {
    self.active_editor_ref()
  }

  fn file_path(&self) -> Option<&Path> {
    let id = self.active_editor?;
    self.file_paths.get(&id).map(|path| path.as_path())
  }

  fn request_render(&mut self) {
    self.active_state_mut().needs_render = true;
  }

  fn messages(&self) -> &MessageCenter {
    &self.active_state_ref().messages
  }

  fn messages_mut(&mut self) -> &mut MessageCenter {
    &mut self.active_state_mut().messages
  }

  fn message_presentation(&self) -> MessagePresentation {
    MessagePresentation::Toast
  }

  fn lsp_statusline_text(&self) -> Option<String> {
    self.lsp_statusline_text_value()
  }

  fn vcs_statusline_text(&self) -> Option<String> {
    let id = self.active_editor?;
    self
      .states
      .get(&id)
      .and_then(|state| state.vcs_statusline.clone())
  }

  fn apply_transaction(&mut self, transaction: &Transaction) -> bool {
    enum SyntaxParseHighlightUpdate {
      Parsed,
      Interpolated,
    }

    let Some(editor_id) = self.active_editor else {
      return false;
    };

    let _ = self.poll_editor_syntax_parse_results(editor_id);

    let old_text_for_lsp = self.active_editor_ref().document().text().clone();
    let loader = self.loader.clone();
    let mut async_parse_job: Option<SyntaxParseJob> = None;
    let mut async_parse_doc_version = None;
    let mut syntax_highlight_update: Option<SyntaxParseHighlightUpdate> = None;

    {
      let Some(editor) = self.inner.editor_mut(editor_id) else {
        return false;
      };
      let doc = editor.document_mut();
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

    if let Some(update) = syntax_highlight_update
      && let Some(state) = self.states.get_mut(&editor_id)
    {
      match update {
        SyntaxParseHighlightUpdate::Parsed => state.syntax_parse_highlight_state.mark_parsed(),
        SyntaxParseHighlightUpdate::Interpolated => {
          state.syntax_parse_highlight_state.mark_interpolated();
        },
      }
    }

    if let (Some(parse_job), Some(doc_version)) = (async_parse_job, async_parse_doc_version) {
      self.queue_editor_syntax_parse_job(editor_id, doc_version, parse_job);
    }

    self.lsp_send_did_change(&old_text_for_lsp, transaction.changes());
    self.refresh_vcs_diff_document_for_editor(editor_id);

    true
  }

  fn build_render_plan(&mut self) -> the_lib::render::RenderPlan {
    self.build_render_plan_with_styles_impl(RenderStyles::default())
  }

  fn build_render_plan_with_styles(&mut self, styles: RenderStyles) -> the_lib::render::RenderPlan {
    self.build_render_plan_with_styles_impl(styles)
  }

  fn request_quit(&mut self) {
    self.should_quit = true;
  }

  fn mode(&self) -> Mode {
    self.active_state_ref().mode
  }

  fn set_mode(&mut self, mode: Mode) {
    self.active_state_mut().mode = mode;
  }

  fn keymaps(&mut self) -> &mut Keymaps {
    &mut self.keymaps
  }

  fn command_prompt_mut(&mut self) -> &mut CommandPromptState {
    &mut self.active_state_mut().command_prompt
  }

  fn command_prompt_ref(&self) -> &CommandPromptState {
    &self.active_state_ref().command_prompt
  }

  fn command_registry_mut(&mut self) -> &mut CommandRegistry<Self> {
    &mut self.command_registry
  }

  fn command_registry_ref(&self) -> &CommandRegistry<Self> {
    &self.command_registry
  }

  fn command_palette(&self) -> &CommandPaletteState {
    &self.active_state_ref().command_palette
  }

  fn command_palette_mut(&mut self) -> &mut CommandPaletteState {
    &mut self.active_state_mut().command_palette
  }

  fn command_palette_style(&self) -> &CommandPaletteStyle {
    &self.active_state_ref().command_palette_style
  }

  fn command_palette_style_mut(&mut self) -> &mut CommandPaletteStyle {
    &mut self.active_state_mut().command_palette_style
  }

  fn file_picker(&self) -> &FilePickerState {
    &self.active_state_ref().file_picker
  }

  fn file_picker_mut(&mut self) -> &mut FilePickerState {
    &mut self.active_state_mut().file_picker
  }

  fn search_prompt_ref(&self) -> &SearchPromptState {
    &self.active_state_ref().search_prompt
  }

  fn search_prompt_mut(&mut self) -> &mut SearchPromptState {
    &mut self.active_state_mut().search_prompt
  }

  fn ui_state(&self) -> &UiState {
    &self.active_state_ref().ui_state
  }

  fn ui_state_mut(&mut self) -> &mut UiState {
    &mut self.active_state_mut().ui_state
  }

  fn dispatch(&self) -> DispatchRef<Self> {
    DispatchRef::from_ptr(&self.dispatch as *const _)
  }

  fn pending_input(&self) -> Option<&the_default::PendingInput> {
    self.active_state_ref().pending_input.as_ref()
  }

  fn set_pending_input(&mut self, pending: Option<the_default::PendingInput>) {
    self.active_state_mut().pending_input = pending;
  }

  fn registers(&self) -> &Registers {
    &self.registers
  }

  fn registers_mut(&mut self) -> &mut Registers {
    &mut self.registers
  }

  fn register(&self) -> Option<char> {
    self.active_state_ref().register
  }

  fn set_register(&mut self, register: Option<char>) {
    self.active_state_mut().register = register;
  }

  fn macro_recording(&self) -> &Option<(char, Vec<KeyBinding>)> {
    &self.active_state_ref().macro_recording
  }

  fn set_macro_recording(&mut self, recording: Option<(char, Vec<KeyBinding>)>) {
    self.active_state_mut().macro_recording = recording;
  }

  fn macro_replaying(&self) -> &Vec<char> {
    &self.active_state_ref().macro_replaying
  }

  fn macro_replaying_mut(&mut self) -> &mut Vec<char> {
    &mut self.active_state_mut().macro_replaying
  }

  fn macro_queue(&self) -> &VecDeque<KeyEvent> {
    &self.active_state_ref().macro_queue
  }

  fn macro_queue_mut(&mut self) -> &mut VecDeque<KeyEvent> {
    &mut self.active_state_mut().macro_queue
  }

  fn last_motion(&self) -> Option<Motion> {
    self.last_motion
  }

  fn set_last_motion(&mut self, motion: Option<Motion>) {
    self.last_motion = motion;
  }

  fn text_format(&self) -> TextFormat {
    let mut text_fmt = self.active_state_ref().text_format.clone();
    text_fmt.viewport_width = self.active_editor_ref().view().viewport.width;
    text_fmt
  }

  fn soft_wrap_enabled(&self) -> bool {
    self.active_state_ref().text_format.soft_wrap
  }

  fn set_soft_wrap_enabled(&mut self, enabled: bool) {
    self.active_state_mut().text_format.soft_wrap = enabled;
    if enabled {
      self.active_editor_mut().view_mut().scroll.col = 0;
    }
  }

  fn gutter_config(&self) -> &GutterConfig {
    &self.active_state_ref().gutter_config
  }

  fn gutter_config_mut(&mut self) -> &mut GutterConfig {
    &mut self.active_state_mut().gutter_config
  }

  fn text_annotations(&self) -> TextAnnotations<'_> {
    let state = self.active_state_ref();
    let mut annotations = TextAnnotations::default();
    if !state.inline_annotations.is_empty() {
      let _ = annotations.add_inline_annotations(&state.inline_annotations, None);
    }
    if !state.overlay_annotations.is_empty() {
      let _ = annotations.add_overlay(&state.overlay_annotations, None);
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
    if let Some(id) = self.active_editor {
      match path {
        Some(path) => {
          self.file_paths.insert(id, path);
        },
        None => {
          self.file_paths.remove(&id);
        },
      }
      self.refresh_editor_syntax(id);
      self.refresh_lsp_runtime_for_active_file();
      self.refresh_vcs_diff_base_for_editor(id);
    }
  }

  fn log_target_names(&self) -> &'static [&'static str] {
    &["watch"]
  }

  fn log_path_for_target(&self, target: &str) -> Option<PathBuf> {
    match target {
      "watch" => resolve_file_watch_trace_log_path(),
      _ => None,
    }
  }

  fn open_file(&mut self, path: &Path) -> std::io::Result<()> {
    let content = std::fs::read_to_string(path)?;
    {
      let editor = self.active_editor_mut();
      let doc = editor.document_mut();
      let len = doc.text().len_chars();
      let tx = Transaction::change(doc.text(), vec![(0, len, Some(content.as_str().into()))])
        .map_err(|err| std::io::Error::other(err.to_string()))?;
      doc
        .apply_transaction(&tx)
        .map_err(|err| std::io::Error::other(err.to_string()))?;
      let _ = doc.set_selection(Selection::point(0));
      doc.set_display_name(
        path
          .file_name()
          .map(|name| name.to_string_lossy().to_string())
          .unwrap_or_else(|| path.display().to_string()),
      );
      let _ = doc.mark_saved();
      editor.view_mut().scroll = LibPosition::new(0, 0);
    }
    DefaultContext::set_file_path(self, Some(path.to_path_buf()));
    self.request_render();
    Ok(())
  }

  fn lsp_goto_definition(&mut self) {
    if !self.lsp_supports(LspCapability::GotoDefinition) {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
        "goto-definition is not supported by the active server",
      );
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
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

  fn on_file_saved(&mut self, _path: &Path, text: &str) {
    if let Some(watch) = self.lsp_watched_file.as_mut() {
      watch.suppress_until = Some(Instant::now() + lsp_self_save_suppress_window());
    }
    self.lsp_send_did_save(Some(text));
  }

  fn on_before_quit(&mut self) {
    self.lsp_close_current_document();
    let _ = self.lsp_runtime.shutdown();
    self.lsp_ready = false;
    self.lsp_document = None;
    self.lsp_watched_file = None;
    self.lsp_active_progress_tokens.clear();
    self.lsp_pending_requests.clear();
    self.set_lsp_status(LspStatusPhase::Off, Some("stopped".into()));
  }

  fn scrolloff(&self) -> usize {
    self.active_state_ref().scrolloff
  }
}

fn key_event_from_ffi(event: ffi::KeyEvent) -> the_default::KeyEvent {
  use the_default::{
    Key as LibKey,
    KeyEvent as LibKeyEvent,
    Modifiers as LibModifiers,
  };

  let key = match event.kind {
    0 => {
      char::from_u32(event.codepoint)
        .map(LibKey::Char)
        .unwrap_or(LibKey::Other)
    },
    1 => LibKey::Enter,
    2 => LibKey::NumpadEnter,
    3 => LibKey::Escape,
    4 => LibKey::Backspace,
    5 => LibKey::Tab,
    6 => LibKey::Delete,
    7 => LibKey::Insert,
    8 => LibKey::Home,
    9 => LibKey::End,
    10 => LibKey::PageUp,
    11 => LibKey::PageDown,
    12 => LibKey::Left,
    13 => LibKey::Right,
    14 => LibKey::Up,
    15 => LibKey::Down,
    16 => LibKey::F1,
    17 => LibKey::F2,
    18 => LibKey::F3,
    19 => LibKey::F4,
    20 => LibKey::F5,
    21 => LibKey::F6,
    22 => LibKey::F7,
    23 => LibKey::F8,
    24 => LibKey::F9,
    25 => LibKey::F10,
    26 => LibKey::F11,
    27 => LibKey::F12,
    _ => LibKey::Other,
  };

  let mut modifiers = LibModifiers::empty();
  if (event.modifiers & LibModifiers::CTRL) != 0 {
    modifiers.insert(LibModifiers::CTRL);
  }
  if (event.modifiers & LibModifiers::ALT) != 0 {
    modifiers.insert(LibModifiers::ALT);
  }
  if (event.modifiers & LibModifiers::SHIFT) != 0 {
    modifiers.insert(LibModifiers::SHIFT);
  }

  LibKeyEvent { key, modifiers }
}

fn insert_text(doc: &mut LibDocument, text: &str) -> bool {
  let rope = doc.text();
  let changes: Vec<_> = doc
    .selection()
    .iter()
    .map(|range| {
      let pos = range.cursor(rope.slice(..));
      (pos, pos, Some(text.into()))
    })
    .collect();

  if let Ok(tx) = Transaction::change(rope, changes) {
    doc.apply_transaction(&tx).is_ok()
  } else {
    false
  }
}

fn delete_backward(doc: &mut LibDocument) -> bool {
  let rope = doc.text();
  let changes: Vec<_> = doc
    .selection()
    .iter()
    .filter_map(|range| {
      let pos = range.cursor(rope.slice(..));
      if pos > 0 {
        Some((pos - 1, pos, None))
      } else {
        None
      }
    })
    .collect();

  if changes.is_empty() {
    return false;
  }

  if let Ok(tx) = Transaction::change(rope, changes) {
    doc.apply_transaction(&tx).is_ok()
  } else {
    false
  }
}

fn delete_forward(doc: &mut LibDocument) -> bool {
  let rope = doc.text();
  let len = rope.len_chars();
  let changes: Vec<_> = doc
    .selection()
    .iter()
    .filter_map(|range| {
      let pos = range.cursor(rope.slice(..));
      if pos < len {
        Some((pos, pos + 1, None))
      } else {
        None
      }
    })
    .collect();

  if changes.is_empty() {
    return false;
  }

  if let Ok(tx) = Transaction::change(rope, changes) {
    doc.apply_transaction(&tx).is_ok()
  } else {
    false
  }
}

fn move_horizontal(doc: &mut LibDocument, dir: Direction, text_fmt: &TextFormat) {
  let selection = {
    let slice = doc.text().slice(..);
    let mut annotations = TextAnnotations::default();
    doc.selection().clone().transform(|range| {
      movement::move_horizontally(
        slice,
        range,
        dir,
        1,
        Movement::Move,
        text_fmt,
        &mut annotations,
      )
    })
  };

  let _ = doc.set_selection(selection);
}

fn move_vertical(doc: &mut LibDocument, dir: Direction, text_fmt: &TextFormat) {
  let selection = {
    let slice = doc.text().slice(..);
    let mut annotations = TextAnnotations::default();
    doc.selection().clone().transform(|range| {
      movement::move_vertically(
        slice,
        range,
        dir,
        1,
        Movement::Move,
        text_fmt,
        &mut annotations,
      )
    })
  };

  let _ = doc.set_selection(selection);
}

fn add_cursor_vertical(
  doc: &mut LibDocument,
  dir: Direction,
  pick: CursorPick,
  text_fmt: &TextFormat,
) -> bool {
  let (base_cursor, new_cursor, new_range) = {
    let slice = doc.text().slice(..);
    let mut annotations = TextAnnotations::default();

    let Ok((_, base_range)) = doc.selection().pick(pick) else {
      return false;
    };

    let new_range = movement::move_vertically(
      slice,
      base_range,
      dir,
      1,
      Movement::Move,
      text_fmt,
      &mut annotations,
    );

    let base_cursor = base_range.cursor(slice);
    let new_cursor = new_range.cursor(slice);
    (base_cursor, new_cursor, new_range)
  };

  if new_cursor == base_cursor {
    return false;
  }

  let selection = doc.selection().clone().push(new_range);
  doc.set_selection(selection).is_ok()
}

fn collapse_selection(doc: &mut LibDocument, pick: CursorPick) -> bool {
  match doc.selection().clone().collapse(pick) {
    Ok(selection) => doc.set_selection(selection).is_ok(),
    Err(_) => false,
  }
}

// Swift bridge module
#[swift_bridge::bridge]
mod ffi {
  extern "Rust" {
    type Document;
    type App;

    // App lifecycle
    #[swift_bridge(init)]
    fn new() -> App;
    fn create_editor(self: &mut App, text: &str, viewport: Rect, scroll: Position) -> EditorId;
    fn remove_editor(self: &mut App, id: EditorId) -> bool;
    fn set_viewport(self: &mut App, id: EditorId, viewport: Rect) -> bool;
    fn set_scroll(self: &mut App, id: EditorId, scroll: Position) -> bool;
    fn set_file_path(self: &mut App, id: EditorId, path: &str) -> bool;
    fn set_active_cursor(self: &mut App, id: EditorId, cursor_id: u64) -> bool;
    fn clear_active_cursor(self: &mut App, id: EditorId) -> bool;
    fn cursor_ids(self: &App, id: EditorId) -> Vec<u64>;
    fn render_plan(self: &mut App, id: EditorId) -> RenderPlan;
    fn render_plan_with_styles(self: &mut App, id: EditorId, styles: RenderStyles) -> RenderPlan;
    fn ui_tree_json(self: &mut App, id: EditorId) -> String;
    fn message_snapshot_json(self: &mut App, id: EditorId) -> String;
    fn message_events_since_json(self: &mut App, id: EditorId, seq: u64) -> String;
    fn ui_event_json(self: &mut App, id: EditorId, event_json: &str) -> bool;
    fn text(self: &App, id: EditorId) -> String;
    fn pending_keys_json(self: &App, id: EditorId) -> String;
    fn pending_key_hints_json(self: &App, id: EditorId) -> String;
    fn mode(self: &App, id: EditorId) -> u8;
    fn theme_highlight_style(self: &App, highlight: u32) -> Style;
    fn command_palette_is_open(self: &mut App, id: EditorId) -> bool;
    fn command_palette_query(self: &mut App, id: EditorId) -> String;
    fn command_palette_layout(self: &mut App, id: EditorId) -> u8;
    fn command_palette_filtered_count(self: &mut App, id: EditorId) -> usize;
    fn command_palette_filtered_selected_index(self: &mut App, id: EditorId) -> i64;
    fn command_palette_filtered_title(self: &mut App, id: EditorId, index: usize) -> String;
    fn command_palette_filtered_subtitle(self: &mut App, id: EditorId, index: usize) -> String;
    fn command_palette_filtered_description(self: &mut App, id: EditorId, index: usize) -> String;
    fn command_palette_filtered_shortcut(self: &mut App, id: EditorId, index: usize) -> String;
    fn command_palette_filtered_badge(self: &mut App, id: EditorId, index: usize) -> String;
    fn command_palette_filtered_leading_icon(self: &mut App, id: EditorId, index: usize) -> String;
    fn command_palette_filtered_leading_color(self: &mut App, id: EditorId, index: usize) -> Color;
    fn command_palette_filtered_symbol_count(self: &mut App, id: EditorId, index: usize) -> usize;
    fn command_palette_filtered_symbol(
      self: &mut App,
      id: EditorId,
      index: usize,
      symbol_index: usize,
    ) -> String;
    fn command_palette_select_filtered(self: &mut App, id: EditorId, index: usize) -> bool;
    fn command_palette_submit_filtered(self: &mut App, id: EditorId, index: usize) -> bool;
    fn command_palette_close(self: &mut App, id: EditorId) -> bool;
    fn command_palette_set_query(self: &mut App, id: EditorId, query: &str) -> bool;
    fn search_prompt_set_query(self: &mut App, id: EditorId, query: &str) -> bool;
    fn search_prompt_close(self: &mut App, id: EditorId) -> bool;
    fn search_prompt_submit(self: &mut App, id: EditorId) -> bool;
    fn file_picker_set_query(self: &mut App, id: EditorId, query: &str) -> bool;
    fn file_picker_submit(self: &mut App, id: EditorId, index: usize) -> bool;
    fn file_picker_close(self: &mut App, id: EditorId) -> bool;
    fn file_picker_snapshot_json(self: &mut App, id: EditorId, max_items: usize) -> String;
    fn poll_background(self: &mut App, id: EditorId) -> bool;
    fn take_should_quit(self: &mut App) -> bool;
    fn handle_key(self: &mut App, id: EditorId, event: KeyEvent) -> bool;
    fn ensure_cursor_visible(self: &mut App, id: EditorId) -> bool;

    // Editor editing
    fn insert(self: &mut App, id: EditorId, text: &str) -> bool;
    fn delete_backward(self: &mut App, id: EditorId) -> bool;
    fn delete_forward(self: &mut App, id: EditorId) -> bool;
    fn move_left(self: &mut App, id: EditorId);
    fn move_right(self: &mut App, id: EditorId);
    fn move_up(self: &mut App, id: EditorId);
    fn move_down(self: &mut App, id: EditorId);
    fn add_cursor_above(self: &mut App, id: EditorId) -> bool;
    fn add_cursor_below(self: &mut App, id: EditorId) -> bool;
    fn collapse_to_cursor(self: &mut App, id: EditorId, cursor_id: u64) -> bool;
    fn collapse_to_first(self: &mut App, id: EditorId) -> bool;

    // Constructors
    #[swift_bridge(init)]
    fn new() -> Document;

    #[swift_bridge(associated_to = Document)]
    fn from_text(text: &str) -> Document;

    // Content access
    fn text(self: &Document) -> String;
    fn len_chars(self: &Document) -> usize;
    fn len_lines(self: &Document) -> usize;
    fn is_empty(self: &Document) -> bool;
    fn version(self: &Document) -> u64;
    fn is_modified(self: &Document) -> bool;

    // Selection queries
    fn primary_cursor(self: &Document) -> usize;
    fn cursor_count(self: &Document) -> usize;
    fn all_cursors(self: &Document) -> Vec<usize>;

    // Text editing
    fn insert(self: &mut Document, text: &str) -> bool;
    fn delete_backward(self: &mut Document) -> bool;
    fn delete_forward(self: &mut Document) -> bool;

    // Cursor movement
    fn move_left(self: &mut Document);
    fn move_right(self: &mut Document);
    fn move_up(self: &mut Document);
    fn move_down(self: &mut Document);

    // Multi-cursor
    fn add_cursor_above(self: &mut Document) -> bool;
    fn add_cursor_below(self: &mut Document) -> bool;
    fn collapse_to_primary(self: &mut Document);

    // History
    fn commit(self: &mut Document) -> bool;
    fn undo(self: &mut Document) -> bool;
    fn redo(self: &mut Document) -> bool;

    // Line access
    fn char_to_line(self: &Document, char_idx: usize) -> usize;
    fn line_to_char(self: &Document, line_idx: usize) -> usize;
  }

  #[swift_bridge(swift_repr = "struct")]
  struct KeyEvent {
    kind:      u8,
    codepoint: u32,
    modifiers: u8,
  }

  #[swift_bridge(swift_repr = "struct")]
  struct EditorId {
    value: u64,
  }

  #[swift_bridge(swift_repr = "struct")]
  struct Rect {
    x:      u16,
    y:      u16,
    width:  u16,
    height: u16,
  }

  #[swift_bridge(swift_repr = "struct")]
  struct Position {
    row: u64,
    col: u64,
  }

  #[swift_bridge(swift_repr = "struct")]
  struct Color {
    kind:  u8,
    value: u32,
  }

  #[swift_bridge(swift_repr = "struct")]
  struct Style {
    has_fg:              bool,
    fg:                  Color,
    has_bg:              bool,
    bg:                  Color,
    has_underline_color: bool,
    underline_color:     Color,
    underline_style:     u8,
    add_modifier:        u16,
    sub_modifier:        u16,
  }

  #[swift_bridge(swift_repr = "struct")]
  struct RenderStyles {
    selection:     Style,
    cursor:        Style,
    active_cursor: Style,
    gutter:        Style,
    gutter_active: Style,
  }

  extern "Rust" {
    type RenderSpan;
    fn col(self: &RenderSpan) -> u16;
    fn cols(self: &RenderSpan) -> u16;
    fn text(self: &RenderSpan) -> String;
    fn has_highlight(self: &RenderSpan) -> bool;
    fn highlight(self: &RenderSpan) -> u32;
    fn is_virtual(self: &RenderSpan) -> bool;
  }

  extern "Rust" {
    type RenderLine;
    fn row(self: &RenderLine) -> u16;
    fn span_count(self: &RenderLine) -> usize;
    fn span_at(self: &RenderLine, index: usize) -> RenderSpan;
  }

  extern "Rust" {
    type RenderGutterSpan;
    fn col(self: &RenderGutterSpan) -> u16;
    fn text(self: &RenderGutterSpan) -> String;
    fn style(self: &RenderGutterSpan) -> Style;
  }

  extern "Rust" {
    type RenderGutterLine;
    fn row(self: &RenderGutterLine) -> u16;
    fn span_count(self: &RenderGutterLine) -> usize;
    fn span_at(self: &RenderGutterLine, index: usize) -> RenderGutterSpan;
  }

  extern "Rust" {
    type RenderCursor;
    fn id(self: &RenderCursor) -> u64;
    fn pos(self: &RenderCursor) -> Position;
    fn kind(self: &RenderCursor) -> u8;
    fn style(self: &RenderCursor) -> Style;
  }

  extern "Rust" {
    type RenderSelection;
    fn rect(self: &RenderSelection) -> Rect;
    fn style(self: &RenderSelection) -> Style;
  }

  extern "Rust" {
    type RenderOverlayNode;
    fn kind(self: &RenderOverlayNode) -> u8;
    fn rect_kind(self: &RenderOverlayNode) -> u8;
    fn rect(self: &RenderOverlayNode) -> Rect;
    fn radius(self: &RenderOverlayNode) -> u16;
    fn pos(self: &RenderOverlayNode) -> Position;
    fn text(self: &RenderOverlayNode) -> String;
    fn style(self: &RenderOverlayNode) -> Style;
  }

  extern "Rust" {
    type RenderPlan;
    fn viewport(self: &RenderPlan) -> Rect;
    fn scroll(self: &RenderPlan) -> Position;
    fn content_offset_x(self: &RenderPlan) -> u16;
    fn gutter_line_count(self: &RenderPlan) -> usize;
    fn gutter_line_at(self: &RenderPlan, index: usize) -> RenderGutterLine;
    fn line_count(self: &RenderPlan) -> usize;
    fn line_at(self: &RenderPlan, index: usize) -> RenderLine;
    fn cursor_count(self: &RenderPlan) -> usize;
    fn cursor_at(self: &RenderPlan, index: usize) -> RenderCursor;
    fn selection_count(self: &RenderPlan) -> usize;
    fn selection_at(self: &RenderPlan, index: usize) -> RenderSelection;
    fn overlay_count(self: &RenderPlan) -> usize;
    fn overlay_at(self: &RenderPlan, index: usize) -> RenderOverlayNode;
  }
}

impl ffi::EditorId {
  fn to_lib(self) -> Option<LibEditorId> {
    let value = usize::try_from(self.value).ok()?;
    NonZeroUsize::new(value).map(LibEditorId::new)
  }
}

impl Copy for ffi::EditorId {}

impl Clone for ffi::EditorId {
  fn clone(&self) -> Self {
    *self
  }
}

impl From<LibEditorId> for ffi::EditorId {
  fn from(id: LibEditorId) -> Self {
    Self {
      value: id.get().get() as u64,
    }
  }
}

impl ffi::Rect {
  fn to_lib(self) -> LibRect {
    LibRect::new(self.x, self.y, self.width, self.height)
  }
}

impl From<LibRect> for ffi::Rect {
  fn from(rect: LibRect) -> Self {
    Self {
      x:      rect.x,
      y:      rect.y,
      width:  rect.width,
      height: rect.height,
    }
  }
}

impl ffi::Position {
  fn to_lib(self) -> LibPosition {
    LibPosition::new(u64_to_usize(self.row), u64_to_usize(self.col))
  }
}

impl From<LibPosition> for ffi::Position {
  fn from(pos: LibPosition) -> Self {
    Self {
      row: pos.row as u64,
      col: pos.col as u64,
    }
  }
}

impl ffi::RenderStyles {
  fn to_lib(self) -> RenderStyles {
    RenderStyles {
      selection:     self.selection.to_lib(),
      cursor:        self.cursor.to_lib(),
      active_cursor: self.active_cursor.to_lib(),
      gutter:        self.gutter.to_lib(),
      gutter_active: self.gutter_active.to_lib(),
    }
  }
}

impl Default for ffi::RenderStyles {
  fn default() -> Self {
    Self {
      selection:     ffi::Style::default(),
      cursor:        ffi::Style::default(),
      active_cursor: ffi::Style::default(),
      gutter:        ffi::Style::default(),
      gutter_active: ffi::Style::default(),
    }
  }
}

impl From<LibStyle> for ffi::Style {
  fn from(style: LibStyle) -> Self {
    Self {
      has_fg:              style.fg.is_some(),
      fg:                  style.fg.map(ffi::Color::from).unwrap_or_default(),
      has_bg:              style.bg.is_some(),
      bg:                  style.bg.map(ffi::Color::from).unwrap_or_default(),
      has_underline_color: style.underline_color.is_some(),
      underline_color:     style
        .underline_color
        .map(ffi::Color::from)
        .unwrap_or_default(),
      underline_style:     style
        .underline_style
        .map(underline_style_to_u8)
        .unwrap_or(0),
      add_modifier:        style.add_modifier.bits(),
      sub_modifier:        style.sub_modifier.bits(),
    }
  }
}

impl ffi::Style {
  fn to_lib(self) -> LibStyle {
    let mut style = LibStyle::new();
    if self.has_fg {
      style.fg = Some(self.fg.to_lib());
    }
    if self.has_bg {
      style.bg = Some(self.bg.to_lib());
    }
    if self.has_underline_color {
      style.underline_color = Some(self.underline_color.to_lib());
    }
    style.underline_style = underline_style_from_u8(self.underline_style);
    style.add_modifier = the_lib::render::graphics::Modifier::from_bits_truncate(self.add_modifier);
    style.sub_modifier = the_lib::render::graphics::Modifier::from_bits_truncate(self.sub_modifier);
    style
  }
}

impl Default for ffi::Style {
  fn default() -> Self {
    Self {
      has_fg:              false,
      fg:                  ffi::Color::default(),
      has_bg:              false,
      bg:                  ffi::Color::default(),
      has_underline_color: false,
      underline_color:     ffi::Color::default(),
      underline_style:     0,
      add_modifier:        0,
      sub_modifier:        0,
    }
  }
}

impl From<LibColor> for ffi::Color {
  fn from(color: LibColor) -> Self {
    match color {
      LibColor::Reset => Self { kind: 0, value: 0 },
      LibColor::Black => Self { kind: 1, value: 0 },
      LibColor::Red => Self { kind: 1, value: 1 },
      LibColor::Green => Self { kind: 1, value: 2 },
      LibColor::Yellow => Self { kind: 1, value: 3 },
      LibColor::Blue => Self { kind: 1, value: 4 },
      LibColor::Magenta => Self { kind: 1, value: 5 },
      LibColor::Cyan => Self { kind: 1, value: 6 },
      LibColor::Gray => Self { kind: 1, value: 7 },
      LibColor::LightRed => Self { kind: 1, value: 8 },
      LibColor::LightGreen => Self { kind: 1, value: 9 },
      LibColor::LightYellow => {
        Self {
          kind:  1,
          value: 10,
        }
      },
      LibColor::LightBlue => {
        Self {
          kind:  1,
          value: 11,
        }
      },
      LibColor::LightMagenta => {
        Self {
          kind:  1,
          value: 12,
        }
      },
      LibColor::LightCyan => {
        Self {
          kind:  1,
          value: 13,
        }
      },
      LibColor::LightGray => {
        Self {
          kind:  1,
          value: 14,
        }
      },
      LibColor::White => {
        Self {
          kind:  1,
          value: 15,
        }
      },
      LibColor::Rgb(r, g, b) => {
        Self {
          kind:  2,
          value: ((r as u32) << 16) | ((g as u32) << 8) | b as u32,
        }
      },
      LibColor::Indexed(idx) => {
        Self {
          kind:  3,
          value: idx as u32,
        }
      },
    }
  }
}

impl ffi::Color {
  fn to_lib(self) -> LibColor {
    match self.kind {
      0 => LibColor::Reset,
      1 => named_color_from_index(self.value),
      2 => {
        let r = ((self.value >> 16) & 0xFF) as u8;
        let g = ((self.value >> 8) & 0xFF) as u8;
        let b = (self.value & 0xFF) as u8;
        LibColor::Rgb(r, g, b)
      },
      3 => LibColor::Indexed(self.value as u8),
      _ => LibColor::Reset,
    }
  }
}

impl Default for ffi::Color {
  fn default() -> Self {
    Self { kind: 0, value: 0 }
  }
}

fn u64_to_usize(value: u64) -> usize {
  usize::try_from(value).unwrap_or(usize::MAX)
}

fn named_color_from_index(value: u32) -> LibColor {
  match value {
    0 => LibColor::Black,
    1 => LibColor::Red,
    2 => LibColor::Green,
    3 => LibColor::Yellow,
    4 => LibColor::Blue,
    5 => LibColor::Magenta,
    6 => LibColor::Cyan,
    7 => LibColor::Gray,
    8 => LibColor::LightRed,
    9 => LibColor::LightGreen,
    10 => LibColor::LightYellow,
    11 => LibColor::LightBlue,
    12 => LibColor::LightMagenta,
    13 => LibColor::LightCyan,
    14 => LibColor::LightGray,
    15 => LibColor::White,
    _ => LibColor::Reset,
  }
}

fn underline_style_to_u8(style: LibUnderlineStyle) -> u8 {
  match style {
    LibUnderlineStyle::Reset => 1,
    LibUnderlineStyle::Line => 2,
    LibUnderlineStyle::Curl => 3,
    LibUnderlineStyle::Dotted => 4,
    LibUnderlineStyle::Dashed => 5,
    LibUnderlineStyle::DoubleLine => 6,
  }
}

fn underline_style_from_u8(value: u8) -> Option<LibUnderlineStyle> {
  match value {
    1 => Some(LibUnderlineStyle::Reset),
    2 => Some(LibUnderlineStyle::Line),
    3 => Some(LibUnderlineStyle::Curl),
    4 => Some(LibUnderlineStyle::Dotted),
    5 => Some(LibUnderlineStyle::Dashed),
    6 => Some(LibUnderlineStyle::DoubleLine),
    _ => None,
  }
}

fn cursor_kind_to_u8(kind: LibCursorKind) -> u8 {
  match kind {
    LibCursorKind::Block => 0,
    LibCursorKind::Bar => 1,
    LibCursorKind::Underline => 2,
    LibCursorKind::Hollow => 3,
    LibCursorKind::Hidden => 4,
  }
}

fn overlay_rect_kind_to_u8(kind: OverlayRectKind) -> u8 {
  match kind {
    OverlayRectKind::Panel => 1,
    OverlayRectKind::Divider => 2,
    OverlayRectKind::Highlight => 3,
    OverlayRectKind::Backdrop => 4,
  }
}

#[cfg(test)]
mod tests {
  use std::{
    path::PathBuf,
    thread,
    time::Duration,
  };

  use the_default::{
    CommandEvent,
    CommandRegistry,
    DefaultContext,
  };
  use the_lib::transaction::Transaction;

  use super::{
    App,
    LibStyle,
    ffi,
  };

  #[test]
  fn app_render_plan_basic() {
    let mut app = App::new();
    let viewport = ffi::Rect {
      x:      0,
      y:      0,
      width:  80,
      height: 24,
    };
    let scroll = ffi::Position { row: 0, col: 0 };
    let id = app.create_editor("hello", viewport, scroll);

    let plan = app.render_plan(id);
    assert_eq!(plan.line_count(), 1);
    let line = plan.line_at(0);
    assert_eq!(line.span_count(), 1);
    assert_eq!(line.span_at(0).text(), "hello");
  }

  #[test]
  fn app_insert_updates_text_and_plan() {
    let mut app = App::new();
    let viewport = ffi::Rect {
      x:      0,
      y:      0,
      width:  80,
      height: 24,
    };
    let scroll = ffi::Position { row: 0, col: 0 };
    let id = app.create_editor("hello", viewport, scroll);

    assert!(app.insert(id, "yo "));
    assert_eq!(app.text(id), "yo hello");

    let plan = app.render_plan(id);
    let line = plan.line_at(0);
    assert_eq!(line.span_at(0).text(), "yo hello");
  }

  #[test]
  fn theme_highlight_style_out_of_bounds_returns_default() {
    let app = App::new();
    let style = app.theme_highlight_style(u32::MAX);
    assert_eq!(style.to_lib(), LibStyle::default());
  }

  fn first_highlight_id(plan: &super::RenderPlan) -> Option<u32> {
    for line_index in 0..plan.line_count() {
      let line = plan.line_at(line_index);
      for span_index in 0..line.span_count() {
        let span = line.span_at(span_index);
        if span.has_highlight() {
          return Some(span.highlight());
        }
      }
    }
    None
  }

  fn wait_for_plan<F>(app: &mut App, id: ffi::EditorId, predicate: F) -> Option<super::RenderPlan>
  where
    F: Fn(&super::RenderPlan) -> bool,
  {
    for _ in 0..80 {
      let plan = app.render_plan(id);
      if predicate(&plan) {
        return Some(plan);
      }
      thread::sleep(Duration::from_millis(5));
    }
    None
  }

  #[test]
  fn syntax_highlight_updates_after_insert() {
    let mut app = App::new();
    if app.loader.is_none() {
      return;
    }

    let viewport = ffi::Rect {
      x:      0,
      y:      0,
      width:  80,
      height: 24,
    };
    let scroll = ffi::Position { row: 0, col: 0 };
    let id = app.create_editor("let value = 1;\n", viewport, scroll);
    assert!(app.set_file_path(id, "main.rs"));

    let Some(initial_plan) = wait_for_plan(&mut app, id, |plan| first_highlight_id(plan).is_some())
    else {
      return;
    };
    let Some(initial_highlight) = first_highlight_id(&initial_plan) else {
      return;
    };

    assert!(app.insert(id, "// "));

    let Some(updated_plan) = wait_for_plan(&mut app, id, |plan| {
      let Some(highlight) = first_highlight_id(plan) else {
        return false;
      };
      highlight != initial_highlight
    }) else {
      return;
    };

    let Some(updated_highlight) = first_highlight_id(&updated_plan) else {
      return;
    };
    assert_ne!(updated_highlight, initial_highlight);
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
        .repeat(16),
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
        .repeat(18),
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
        .repeat(16),
      ),
      (
        "fixture.txt",
        "unicode: 🙂🚀 café e\u{301} こんにちは Привет عربى हिन्दी\n".repeat(24),
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
    let mut app = App::new();

    for (fixture_index, (fixture_name, fixture_text)) in fixture_matrix().into_iter().enumerate() {
      let viewport = ffi::Rect {
        x:      0,
        y:      0,
        width:  100,
        height: 30,
      };
      let scroll = ffi::Position { row: 0, col: 0 };
      let id = app.create_editor(&fixture_text, viewport, scroll);
      let lib_id = id.to_lib().expect("editor id");
      app.file_paths.insert(lib_id, PathBuf::from(fixture_name));
      app.refresh_editor_syntax(lib_id);
      app.active_editor = Some(lib_id);

      let mut rng = SimRng::new(0xBEEF_CAFE ^ fixture_index as u64);
      for step in 0..96usize {
        let old = app
          .inner
          .editor(lib_id)
          .expect("editor")
          .document()
          .text()
          .clone();
        let (from, to, insert) = next_edit(&mut rng, old.len_chars());
        let tx = Transaction::change(
          &old,
          std::iter::once((from, to, insert.map(|text| text.into()))),
        )
        .expect("edit transaction");
        assert!(
          DefaultContext::apply_transaction(&mut app, &tx),
          "failed apply for fixture={fixture_name} step={step}"
        );

        if step % 3 == 0 {
          let plan = app.render_plan(id);
          assert!(
            plan.line_count() > 0,
            "empty render plan for fixture={fixture_name} step={step}"
          );
        }
      }

      for _ in 0..12 {
        let plan = app.render_plan(id);
        assert!(
          plan.line_count() > 0,
          "empty render plan during settle for fixture={fixture_name}"
        );
        thread::sleep(Duration::from_millis(1));
      }

      assert!(app.remove_editor(id));
    }
  }

  #[test]
  fn wrap_command_toggles_soft_wrap_and_changes_render_lines() {
    let mut app = App::new();
    let viewport = ffi::Rect {
      x:      0,
      y:      0,
      width:  24,
      height: 12,
    };
    let scroll = ffi::Position { row: 0, col: 0 };
    let id = app.create_editor(&"wrap-me-".repeat(40), viewport, scroll);
    assert!(app.activate(id).is_some());

    assert!(!app.soft_wrap_enabled());
    let no_wrap = app.render_plan(id);
    assert_eq!(no_wrap.line_count(), 1);

    let registry = app.command_registry_ref() as *const CommandRegistry<App>;
    unsafe { (&*registry).execute(&mut app, "wrap", "on", CommandEvent::Validate) }
      .expect("wrap on");
    assert!(app.soft_wrap_enabled());

    let wrapped = app.render_plan(id);
    assert!(wrapped.line_count() > no_wrap.line_count());

    unsafe { (&*registry).execute(&mut app, "wrap", "status", CommandEvent::Validate) }
      .expect("wrap status");
    assert!(app.soft_wrap_enabled());

    unsafe { (&*registry).execute(&mut app, "wrap", "toggle", CommandEvent::Validate) }
      .expect("wrap toggle");
    assert!(!app.soft_wrap_enabled());

    let toggled = app.render_plan(id);
    assert_eq!(toggled.line_count(), no_wrap.line_count());
  }

  #[test]
  fn ensure_cursor_visible_keeps_horizontal_scroll_zero_with_soft_wrap() {
    let mut app = App::new();
    let viewport = ffi::Rect {
      x:      0,
      y:      0,
      width:  24,
      height: 12,
    };
    let scroll = ffi::Position { row: 0, col: 0 };
    let id = app.create_editor(&"horizontal-scroll-".repeat(20), viewport, scroll);
    assert!(app.activate(id).is_some());

    DefaultContext::set_soft_wrap_enabled(&mut app, true);
    assert!(app.set_scroll(id, ffi::Position { row: 0, col: 40 }));
    assert!(app.ensure_cursor_visible(id));
    assert_eq!(app.active_editor_ref().view().scroll.col, 0);
  }
}
