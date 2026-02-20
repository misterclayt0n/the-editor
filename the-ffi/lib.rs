//! FFI bindings for the-editor, exposing core functionality to Swift via
//! swift-bridge.
//!
//! This crate provides a C-compatible interface to the-lib, allowing the
//! SwiftUI client to interact with the Rust editor core.

use std::{
  cell::RefCell,
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
  rc::Rc,
  sync::{
    Arc,
    OnceLock,
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
use serde::Serialize;
use serde_json::Value;
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
  FilePickerPreview,
  FilePickerState,
  KeyBinding,
  KeyEvent,
  Keymaps,
  MessagePresentation,
  Mode,
  Motion,
  OverlayRect as DefaultOverlayRect,
  SIGNATURE_HELP_ACTIVE_PARAM_END_MARKER,
  SIGNATURE_HELP_ACTIVE_PARAM_START_MARKER,
  SearchPromptKind,
  SearchPromptState,
  close_file_picker,
  command_palette_filtered_indices,
  command_palette_selected_filtered_index,
  completion_docs_panel_rect as default_completion_docs_panel_rect,
  completion_panel_rect as default_completion_panel_rect,
  finalize_search,
  finalize_select_regex,
  handle_query_change as file_picker_handle_query_change,
  poll_scan_results as file_picker_poll_scan_results,
  refresh_matcher_state as file_picker_refresh_matcher_state,
  select_file_picker_index,
  set_file_picker_syntax_loader,
  signature_help_panel_rect as default_signature_help_panel_rect,
  submit_file_picker,
  update_command_palette_for_input,
  update_search_prompt_preview,
};
use the_lib::{
  Tendril,
  app::App as LibApp,
  command_line::split as command_line_split,
  diagnostics::{
    Diagnostic,
    DiagnosticCounts,
    DiagnosticSeverity,
    DiagnosticsState,
  },
  docs_markdown::{
    DocsBlock,
    DocsInlineKind,
    DocsInlineRun,
    DocsListMarker,
    DocsSemanticKind,
    language_filename_hints,
    parse_markdown_blocks,
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
    InlineDiagnostic,
    InlineDiagnosticFilter,
    InlineDiagnosticRenderLine,
    InlineDiagnosticsConfig,
    InlineDiagnosticsLineAnnotation,
    InlineDiagnosticsRenderData,
    LayoutIntent,
    NoHighlights,
    OverlayNode,
    OverlayRectKind,
    OverlayText,
    RenderDiagnosticGutterStyles,
    RenderDiffGutterStyles,
    RenderGutterDiffKind,
    RenderStyles,
    SharedInlineDiagnosticsRenderData,
    SyntaxHighlightAdapter,
    UiNode,
    UiState,
    apply_diagnostic_gutter_markers,
    apply_diff_gutter_markers,
    build_plan,
    graphics::{
      Color as LibColor,
      CursorKind as LibCursorKind,
      Modifier as LibModifier,
      Rect as LibRect,
      Style as LibStyle,
      UnderlineStyle as LibUnderlineStyle,
    },
    gutter_width_for_document,
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
    visual_pos_at_char,
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
  },
  syntax_async::{
    ParseHighlightState,
    ParseLifecycle,
    ParseRequest,
  },
  transaction::{
    Assoc,
    Transaction,
  },
  view::ViewState,
};
use the_loader::config::user_lang_config;
use the_lsp::{
  LspCapability,
  LspCompletionContext,
  LspCompletionItem,
  LspCompletionItemKind,
  LspEvent,
  LspInsertTextFormat,
  LspLocation,
  LspPosition,
  LspProgressKind,
  LspRuntime,
  LspRuntimeConfig,
  LspServerConfig,
  LspSignatureHelpContext,
  completion_params,
  goto_definition_params,
  hover_params,
  jsonrpc,
  parse_completion_item_response,
  parse_completion_response_with_raw,
  parse_hover_response,
  parse_locations_response,
  parse_signature_help_response,
  render_lsp_snippet,
  signature_help_params,
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
use the_runtime::{
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
  inner:                   the_lib::render::RenderPlan,
  inline_diagnostic_lines: Vec<InlineDiagnosticRenderLine>,
  eol_diagnostics:         Vec<EolDiagnosticEntry>,
  diagnostic_underlines:   Vec<DiagnosticUnderlineEntry>,
}

impl RenderPlan {
  fn empty() -> Self {
    Self {
      inner:                   the_lib::render::RenderPlan::empty(
        LibRect::new(0, 0, 0, 0),
        LibPosition::new(0, 0),
      ),
      inline_diagnostic_lines: Vec::new(),
      eol_diagnostics:         Vec::new(),
      diagnostic_underlines:   Vec::new(),
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

  fn inline_diagnostic_line_count(&self) -> usize {
    self.inline_diagnostic_lines.len()
  }

  fn inline_diagnostic_line_at(&self, index: usize) -> RenderInlineDiagnosticLine {
    let scroll_row = self.inner.scroll.row;
    self
      .inline_diagnostic_lines
      .get(index)
      .cloned()
      .map(|line| RenderInlineDiagnosticLine::new(line, scroll_row))
      .unwrap_or_else(RenderInlineDiagnosticLine::empty)
  }

  fn eol_diagnostic_count(&self) -> usize {
    self.eol_diagnostics.len()
  }

  fn eol_diagnostic_at(&self, index: usize) -> RenderEolDiagnostic {
    self
      .eol_diagnostics
      .get(index)
      .map(|entry| {
        RenderEolDiagnostic {
          row:      entry.row,
          col:      entry.col,
          message:  entry.message.clone(),
          severity: entry.severity,
        }
      })
      .unwrap_or_else(RenderEolDiagnostic::empty)
  }

  fn diagnostic_underline_count(&self) -> usize {
    self.diagnostic_underlines.len()
  }

  fn diagnostic_underline_at(&self, index: usize) -> RenderDiagnosticUnderline {
    self
      .diagnostic_underlines
      .get(index)
      .map(|entry| {
        RenderDiagnosticUnderline {
          row:       entry.row,
          start_col: entry.start_col,
          end_col:   entry.end_col,
          severity:  entry.severity,
        }
      })
      .unwrap_or_else(RenderDiagnosticUnderline::empty)
  }
}

impl From<the_lib::render::RenderPlan> for RenderPlan {
  fn from(plan: the_lib::render::RenderPlan) -> Self {
    Self {
      inner:                   plan,
      inline_diagnostic_lines: Vec::new(),
      eol_diagnostics:         Vec::new(),
      diagnostic_underlines:   Vec::new(),
    }
  }
}

pub struct RenderInlineDiagnosticLine {
  inner:      InlineDiagnosticRenderLine,
  scroll_row: usize,
}

impl RenderInlineDiagnosticLine {
  fn empty() -> Self {
    Self {
      inner:      InlineDiagnosticRenderLine {
        row:      0,
        col:      0,
        text:     Tendril::new(),
        severity: DiagnosticSeverity::Warning,
      },
      scroll_row: 0,
    }
  }

  fn new(inner: InlineDiagnosticRenderLine, scroll_row: usize) -> Self {
    Self { inner, scroll_row }
  }

  fn row(&self) -> u16 {
    self.inner.row.saturating_sub(self.scroll_row) as u16
  }

  fn col(&self) -> u16 {
    self.inner.col as u16
  }

  fn text(&self) -> String {
    self.inner.text.to_string()
  }

  fn severity(&self) -> u8 {
    severity_to_u8(self.inner.severity)
  }
}

#[derive(Debug, Clone)]
struct EolDiagnosticEntry {
  row:      u16,
  col:      u16,
  message:  String,
  severity: DiagnosticSeverity,
}

pub struct RenderEolDiagnostic {
  row:      u16,
  col:      u16,
  message:  String,
  severity: DiagnosticSeverity,
}

impl RenderEolDiagnostic {
  fn empty() -> Self {
    Self {
      row:      0,
      col:      0,
      message:  String::new(),
      severity: DiagnosticSeverity::Warning,
    }
  }

  fn row(&self) -> u16 {
    self.row
  }

  fn col(&self) -> u16 {
    self.col
  }

  fn message(&self) -> String {
    self.message.clone()
  }

  fn severity(&self) -> u8 {
    severity_to_u8(self.severity)
  }
}

#[derive(Debug, Clone)]
struct DiagnosticUnderlineEntry {
  row:       u16,
  start_col: u16,
  end_col:   u16,
  severity:  DiagnosticSeverity,
}

pub struct RenderDiagnosticUnderline {
  row:       u16,
  start_col: u16,
  end_col:   u16,
  severity:  DiagnosticSeverity,
}

impl RenderDiagnosticUnderline {
  fn empty() -> Self {
    Self {
      row:       0,
      start_col: 0,
      end_col:   0,
      severity:  DiagnosticSeverity::Warning,
    }
  }

  fn row(&self) -> u16 {
    self.row
  }

  fn start_col(&self) -> u16 {
    self.start_col
  }

  fn end_col(&self) -> u16 {
    self.end_col
  }

  fn severity(&self) -> u8 {
    severity_to_u8(self.severity)
  }
}

fn severity_to_u8(severity: DiagnosticSeverity) -> u8 {
  match severity {
    DiagnosticSeverity::Error => 1,
    DiagnosticSeverity::Warning => 2,
    DiagnosticSeverity::Information => 3,
    DiagnosticSeverity::Hint => 4,
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
      request_id:  request.meta.request_id,
      doc_version: request.meta.doc_version,
      syntax:      parsed,
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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingLspRequestKind {
  GotoDefinition {
    uri: String,
  },
  Hover {
    uri: String,
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
}

impl PendingLspRequestKind {
  fn label(&self) -> &'static str {
    match self {
      Self::GotoDefinition { .. } => "goto-definition",
      Self::Hover { .. } => "hover",
      Self::Completion { .. } => "completion",
      Self::CompletionResolve { .. } => "completion-resolve",
      Self::SignatureHelp { .. } => "signature-help",
    }
  }

  fn uri(&self) -> &str {
    match self {
      Self::GotoDefinition { uri } => uri.as_str(),
      Self::Hover { uri } => uri.as_str(),
      Self::Completion { uri, .. } => uri.as_str(),
      Self::CompletionResolve { uri, .. } => uri.as_str(),
      Self::SignatureHelp { uri } => uri.as_str(),
    }
  }

  fn cancellation_key(&self) -> (&'static str, &str) {
    match self {
      Self::GotoDefinition { uri } => ("goto-definition", uri),
      Self::Hover { uri } => ("hover", uri),
      Self::Completion { uri, .. } => ("completion", uri),
      Self::CompletionResolve { uri, .. } => ("completion-resolve", uri),
      Self::SignatureHelp { uri } => ("signature-help", uri),
    }
  }
}

struct EditorState {
  mode:                          Mode,
  command_prompt:                CommandPromptState,
  command_palette:               CommandPaletteState,
  command_palette_style:         CommandPaletteStyle,
  completion_menu:               the_default::CompletionMenuState,
  signature_help:                the_default::SignatureHelpState,
  file_picker:                   FilePickerState,
  search_prompt:                 SearchPromptState,
  ui_state:                      UiState,
  needs_render:                  bool,
  messages:                      MessageCenter,
  pending_input:                 Option<the_default::PendingInput>,
  register:                      Option<char>,
  macro_recording:               Option<(char, Vec<KeyBinding>)>,
  macro_replaying:               Vec<char>,
  macro_queue:                   VecDeque<KeyEvent>,
  text_format:                   TextFormat,
  gutter_config:                 GutterConfig,
  gutter_diff_signs:             BTreeMap<usize, RenderGutterDiffKind>,
  vcs_statusline:                Option<String>,
  inline_annotations:            Vec<InlineAnnotation>,
  overlay_annotations:           Vec<Overlay>,
  word_jump_inline_annotations:  Vec<InlineAnnotation>,
  word_jump_overlay_annotations: Vec<Overlay>,
  highlight_cache:               HighlightCache,
  syntax_parse_tx:               Sender<SyntaxParseResult>,
  syntax_parse_rx:               Receiver<SyntaxParseResult>,
  syntax_parse_lifecycle:        ParseLifecycle<SyntaxParseJob>,
  syntax_parse_highlight_state:  ParseHighlightState,
  hover_docs:                    Option<String>,
  hover_docs_scroll:             usize,
  scrolloff:                     usize,
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
      completion_menu: the_default::CompletionMenuState::default(),
      signature_help: the_default::SignatureHelpState::default(),
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
      word_jump_inline_annotations: Vec::new(),
      word_jump_overlay_annotations: Vec::new(),
      highlight_cache: HighlightCache::default(),
      syntax_parse_tx,
      syntax_parse_rx,
      syntax_parse_lifecycle: ParseLifecycle::default(),
      syntax_parse_highlight_state: ParseHighlightState::default(),
      hover_docs: None,
      hover_docs_scroll: 0,
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

fn dedupe_inline_diagnostic_lines(lines: &mut Vec<InlineDiagnosticRenderLine>) {
  if lines.len() < 2 {
    return;
  }

  let mut seen: HashSet<(usize, usize, u8, Tendril)> = HashSet::with_capacity(lines.len());
  lines.retain(|line| {
    seen.insert((
      line.row,
      line.col,
      diagnostic_severity_rank(line.severity),
      line.text.clone(),
    ))
  });
}

fn compute_eol_diagnostics(
  diagnostics: &[Diagnostic],
  plan: &the_lib::render::RenderPlan,
) -> Vec<EolDiagnosticEntry> {
  if diagnostics.is_empty() {
    return Vec::new();
  }

  let content_width = plan.content_width();
  if content_width == 0 {
    return Vec::new();
  }

  // Build line end columns from the plan's lines
  let mut line_end_cols: BTreeMap<u16, usize> = BTreeMap::new();
  for line in &plan.lines {
    let end_col = line
      .spans
      .iter()
      .map(|span| span.col as usize + span.cols as usize)
      .max()
      .unwrap_or(0);
    line_end_cols
      .entry(line.row)
      .and_modify(|current| *current = (*current).max(end_col))
      .or_insert(end_col);
  }

  let eol_filter = InlineDiagnosticFilter::Enable(DiagnosticSeverity::Hint);
  let mut out = Vec::new();

  for visible_row in &plan.visible_rows {
    if !visible_row.first_visual_line {
      continue;
    }

    // Select the most severe diagnostic on this doc line
    let selected = diagnostics
      .iter()
      .filter(|d| d.range.start.line as usize == visible_row.doc_line)
      .filter(|d| {
        let severity = d.severity.unwrap_or(DiagnosticSeverity::Warning);
        let rank = diagnostic_severity_rank(severity);
        let InlineDiagnosticFilter::Enable(eol_min) = eol_filter else {
          return false;
        };
        rank >= diagnostic_severity_rank(eol_min)
      })
      .max_by_key(|d| diagnostic_severity_rank(d.severity.unwrap_or(DiagnosticSeverity::Warning)));

    let Some(diagnostic) = selected else {
      continue;
    };

    let message = diagnostic
      .message
      .lines()
      .map(str::trim)
      .filter(|line| !line.is_empty())
      .collect::<Vec<_>>()
      .join("  ");
    if message.is_empty() {
      continue;
    }

    let start_col = line_end_cols
      .get(&visible_row.row)
      .copied()
      .unwrap_or(0)
      .saturating_add(1);
    if start_col >= content_width {
      continue;
    }

    let severity = diagnostic.severity.unwrap_or(DiagnosticSeverity::Warning);
    out.push(EolDiagnosticEntry {
      row: visible_row.row,
      col: start_col as u16,
      message,
      severity,
    });
  }

  out
}

fn compute_diagnostic_underlines<'a>(
  text: &'a Rope,
  diagnostics: &[Diagnostic],
  plan: &the_lib::render::RenderPlan,
  text_fmt: &'a TextFormat,
  annotations: &mut TextAnnotations<'a>,
) -> Vec<DiagnosticUnderlineEntry> {
  if diagnostics.is_empty() {
    return Vec::new();
  }

  let row_start = plan.scroll.row;
  let row_end = row_start.saturating_add(plan.viewport.height as usize);
  let col_start = plan.scroll.col;
  let content_width = plan.content_width();
  let col_end = col_start.saturating_add(content_width);
  if row_start >= row_end || col_start >= col_end {
    return Vec::new();
  }

  // Build row end columns for clipping
  let viewport_height = plan.viewport.height as usize;
  let mut row_end_cols = vec![col_start; viewport_height];
  for line in &plan.lines {
    let row = line.row as usize;
    if row >= row_end_cols.len() {
      continue;
    }
    let end_col = line
      .spans
      .iter()
      .map(|span| col_start + span.col.saturating_add(span.cols) as usize)
      .max()
      .unwrap_or(col_start);
    row_end_cols[row] = row_end_cols[row].max(end_col);
  }

  let text_slice = text.slice(..);
  let text_len = text.len_chars();
  let mut out = Vec::with_capacity(diagnostics.len());

  for diagnostic in diagnostics {
    let severity = diagnostic.severity.unwrap_or(DiagnosticSeverity::Warning);

    let mut start_char = utf16_position_to_char_idx(
      text,
      diagnostic.range.start.line,
      diagnostic.range.start.character,
    )
    .min(text_len);
    let mut end_char = utf16_position_to_char_idx(
      text,
      diagnostic.range.end.line,
      diagnostic.range.end.character,
    )
    .min(text_len);

    if end_char < start_char {
      std::mem::swap(&mut start_char, &mut end_char);
    }
    if end_char == start_char {
      if start_char >= text_len {
        continue;
      }
      end_char = start_char.saturating_add(1).min(text_len);
    }

    let Some(start_pos) = visual_pos_at_char(text_slice, text_fmt, annotations, start_char) else {
      continue;
    };
    let Some(end_pos) = visual_pos_at_char(text_slice, text_fmt, annotations, end_char) else {
      continue;
    };

    let (start_pos, end_pos) = if (end_pos.row, end_pos.col) < (start_pos.row, start_pos.col) {
      (end_pos, start_pos)
    } else {
      (start_pos, end_pos)
    };

    for row in start_pos.row..=end_pos.row {
      if row < row_start || row >= row_end {
        continue;
      }
      let relative_row = row.saturating_sub(row_start);
      let row_end_col = row_end_cols.get(relative_row).copied().unwrap_or(col_start);

      let (mut from, mut to) = if row == start_pos.row && row == end_pos.row {
        (start_pos.col, end_pos.col)
      } else if row == start_pos.row {
        (start_pos.col, row_end_col)
      } else if row == end_pos.row {
        (col_start, end_pos.col)
      } else {
        (col_start, row_end_col)
      };

      from = from.max(col_start);
      to = to.min(row_end_col).min(col_end);
      if to <= from {
        continue;
      }

      out.push(DiagnosticUnderlineEntry {
        row: (row - row_start) as u16,
        start_col: (from - col_start) as u16,
        end_col: (to - col_start) as u16,
        severity,
      });
    }
  }

  out
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

fn is_symbol_word_char(ch: char) -> bool {
  ch == '_' || ch.is_alphanumeric()
}

fn is_completion_replace_char(ch: char) -> bool {
  is_symbol_word_char(ch)
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

fn completion_kind_color(kind: LspCompletionItemKind) -> LibColor {
  use LspCompletionItemKind::*;
  match kind {
    Method | Function | Constructor | Operator => LibColor::Rgb(0xDB, 0xBF, 0xEF),
    Field | Variable | Property | Value | Reference => LibColor::Rgb(0xA4, 0xA0, 0xE8),
    Class | Interface | Enum | Struct | TypeParameter => LibColor::Rgb(0xEF, 0xBA, 0x5D),
    Module | Folder | EnumMember | Constant => LibColor::Rgb(0xE8, 0xDC, 0xA0),
    Keyword => LibColor::Rgb(0xEC, 0xCD, 0xBA),
    Snippet => LibColor::Rgb(0x9F, 0xF2, 0x8F),
    Event => LibColor::Rgb(0xF4, 0x78, 0x68),
    Text | Unit | Color | File => LibColor::Rgb(0xCC, 0xCC, 0xCC),
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

fn set_completion_snippet_selection(
  doc: &mut LibDocument,
  mapped_base: usize,
  cursor_range: &std::ops::Range<usize>,
) {
  let max = doc.text().len_chars();
  let anchor = mapped_base.saturating_add(cursor_range.start).min(max);
  let head = mapped_base.saturating_add(cursor_range.end).min(max);
  let _ = doc.set_selection(Selection::single(anchor, head));
}

fn completion_item_accepts_commit_char(item: &LspCompletionItem, ch: char) -> bool {
  item.commit_characters.iter().any(|candidate| {
    let mut chars = candidate.chars();
    matches!(chars.next(), Some(first) if first == ch && chars.next().is_none())
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

#[derive(Clone)]
struct DocsStyledTextRun {
  text:  String,
  style: LibStyle,
  kind:  DocsSemanticKind,
  href:  Option<String>,
}

#[derive(Clone, Copy)]
struct DocsRenderStyles {
  base:             LibStyle,
  heading:          [LibStyle; 6],
  bullet:           LibStyle,
  quote:            LibStyle,
  code:             LibStyle,
  active_parameter: LibStyle,
  link:             LibStyle,
  rule:             LibStyle,
}

impl DocsRenderStyles {
  fn default(base: LibStyle) -> Self {
    let heading = [
      base.add_modifier(LibModifier::BOLD),
      base.add_modifier(LibModifier::BOLD),
      base.add_modifier(LibModifier::BOLD),
      base.add_modifier(LibModifier::BOLD),
      base.add_modifier(LibModifier::BOLD),
      base.add_modifier(LibModifier::BOLD),
    ];
    Self {
      base,
      heading,
      bullet: base.add_modifier(LibModifier::BOLD),
      quote: base.add_modifier(LibModifier::DIM),
      code: base.add_modifier(LibModifier::DIM),
      active_parameter: base
        .add_modifier(LibModifier::BOLD)
        .underline_style(LibUnderlineStyle::Line),
      link: base.underline_style(LibUnderlineStyle::Line),
      rule: base.add_modifier(LibModifier::DIM),
    }
  }
}

#[derive(Debug, Serialize)]
struct DocsRenderSnapshot {
  lines: Vec<Vec<DocsRunSnapshot>>,
}

#[derive(Debug, Serialize)]
struct DocsRunSnapshot {
  text:  String,
  kind:  DocsSemanticKind,
  #[serde(skip_serializing_if = "Option::is_none")]
  href:  Option<String>,
  style: DocsStyleSnapshot,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct DocsStyleSnapshot {
  has_fg:              bool,
  fg:                  DocsColorSnapshot,
  has_bg:              bool,
  bg:                  DocsColorSnapshot,
  has_underline_color: bool,
  underline_color:     DocsColorSnapshot,
  underline_style:     u8,
  add_modifier:        u16,
  sub_modifier:        u16,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct DocsColorSnapshot {
  kind:  u8,
  value: u32,
}

impl From<ffi::Color> for DocsColorSnapshot {
  fn from(color: ffi::Color) -> Self {
    Self {
      kind:  color.kind,
      value: color.value,
    }
  }
}

fn docs_theme_style_or(theme: &Theme, scope: &str, fallback: LibStyle) -> LibStyle {
  theme
    .try_get(scope)
    .map(|style| fallback.patch(style))
    .unwrap_or(fallback)
}

fn docs_render_styles(theme: &Theme, base: LibStyle) -> DocsRenderStyles {
  let mut styles = DocsRenderStyles::default(base);
  styles.heading = [
    docs_theme_style_or(theme, "markup.heading.1", styles.heading[0]),
    docs_theme_style_or(theme, "markup.heading.2", styles.heading[1]),
    docs_theme_style_or(theme, "markup.heading.3", styles.heading[2]),
    docs_theme_style_or(theme, "markup.heading.4", styles.heading[3]),
    docs_theme_style_or(theme, "markup.heading.5", styles.heading[4]),
    docs_theme_style_or(theme, "markup.heading.6", styles.heading[5]),
  ];
  styles.bullet = docs_theme_style_or(theme, "markup.list.unnumbered", styles.bullet);
  styles.quote = docs_theme_style_or(theme, "markup.quote", styles.quote);
  styles.code = docs_theme_style_or(theme, "markup.raw.inline", styles.code);
  styles.active_parameter = docs_theme_style_or(
    theme,
    "ui.selection.primary",
    docs_theme_style_or(theme, "ui.selection", styles.active_parameter),
  );
  styles.link = docs_theme_style_or(theme, "markup.link.text", styles.link);
  styles.rule = docs_theme_style_or(theme, "punctuation.special", styles.rule);
  styles
}

fn docs_push_styled_run(
  runs: &mut Vec<DocsStyledTextRun>,
  text: String,
  style: LibStyle,
  kind: DocsSemanticKind,
  href: Option<String>,
) {
  if text.is_empty() {
    return;
  }
  if let Some(last) = runs.last_mut()
    && last.style == style
    && last.kind == kind
    && last.href == href
  {
    last.text.push_str(&text);
    return;
  }
  runs.push(DocsStyledTextRun {
    text,
    style,
    kind,
    href,
  });
}

fn docs_runs_from_inline(
  inline_runs: &[DocsInlineRun],
  styles: &DocsRenderStyles,
  base_style: LibStyle,
  default_kind: DocsSemanticKind,
) -> Vec<DocsStyledTextRun> {
  let mut runs = Vec::new();
  for inline in inline_runs {
    let (kind, mut style) = match inline.kind {
      DocsInlineKind::Text => (default_kind, base_style),
      DocsInlineKind::Link => (DocsSemanticKind::Link, base_style.patch(styles.link)),
      DocsInlineKind::InlineCode => (DocsSemanticKind::InlineCode, base_style.patch(styles.code)),
    };
    if inline.strong {
      style = style.add_modifier(LibModifier::BOLD);
    }
    if inline.emphasis {
      style = style.add_modifier(LibModifier::ITALIC);
    }
    docs_push_styled_run(
      &mut runs,
      inline.text.clone(),
      style,
      kind,
      inline.link_destination.clone(),
    );
  }
  runs
}

fn docs_preview_highlight_at(
  highlights: &[(Highlight, std::ops::Range<usize>)],
  byte_idx: usize,
) -> Option<Highlight> {
  let mut active = None;
  for (highlight, range) in highlights {
    if byte_idx < range.start {
      break;
    }
    if byte_idx < range.end {
      active = Some(*highlight);
    }
  }
  active
}

fn docs_strip_signature_active_markers_from_line(
  line: &str,
) -> (String, Option<std::ops::Range<usize>>) {
  let mut cleaned = String::with_capacity(line.len());
  let mut idx = 0usize;
  let mut start = None;
  let mut end = None;

  while idx < line.len() {
    if line[idx..].starts_with(SIGNATURE_HELP_ACTIVE_PARAM_START_MARKER) {
      if start.is_none() {
        start = Some(cleaned.len());
      }
      idx += SIGNATURE_HELP_ACTIVE_PARAM_START_MARKER.len();
      continue;
    }
    if line[idx..].starts_with(SIGNATURE_HELP_ACTIVE_PARAM_END_MARKER) {
      if start.is_some() && end.is_none() {
        end = Some(cleaned.len());
      }
      idx += SIGNATURE_HELP_ACTIVE_PARAM_END_MARKER.len();
      continue;
    }

    let mut chars = line[idx..].chars();
    let Some(ch) = chars.next() else {
      break;
    };
    cleaned.push(ch);
    idx += ch.len_utf8();
  }

  let range = match (start, end) {
    (Some(start), Some(end)) if start < end => Some(start..end),
    (Some(start), None) if start < cleaned.len() => Some(start..cleaned.len()),
    _ => None,
  };
  (cleaned, range)
}

fn docs_strip_signature_active_markers_from_lines(
  code_lines: &[String],
) -> (Vec<String>, Option<std::ops::Range<usize>>) {
  let mut cleaned_lines = Vec::with_capacity(code_lines.len());
  let mut active_range = None;
  let mut line_start = 0usize;

  for (idx, line) in code_lines.iter().enumerate() {
    let (cleaned, line_range) = docs_strip_signature_active_markers_from_line(line);
    if active_range.is_none()
      && let Some(range) = line_range
    {
      active_range = Some((line_start + range.start)..(line_start + range.end));
    }
    line_start += cleaned.len();
    if idx + 1 < code_lines.len() {
      line_start += 1;
    }
    cleaned_lines.push(cleaned);
  }

  (cleaned_lines, active_range)
}

fn docs_byte_range_overlaps_active(
  byte_start: usize,
  byte_end: usize,
  active_range: Option<&std::ops::Range<usize>>,
) -> bool {
  active_range.is_some_and(|active| byte_start < active.end && byte_end > active.start)
}

fn docs_render_code_lines_with_active_style(
  code_lines: &[String],
  base_style: LibStyle,
  active_parameter_style: LibStyle,
  active_range: Option<&std::ops::Range<usize>>,
) -> Vec<Vec<DocsStyledTextRun>> {
  let mut rendered = Vec::with_capacity(code_lines.len());
  let mut line_start_byte = 0usize;

  for (idx, line) in code_lines.iter().enumerate() {
    let mut runs = Vec::new();
    let mut piece = String::new();
    let mut run_style = base_style;
    let mut run_kind = DocsSemanticKind::Code;
    let mut byte_idx = line_start_byte;

    for ch in line.chars() {
      let byte_end = byte_idx.saturating_add(ch.len_utf8());
      let mut style = base_style;
      let mut kind = DocsSemanticKind::Code;
      if docs_byte_range_overlaps_active(byte_idx, byte_end, active_range) {
        style = style.patch(active_parameter_style);
        kind = DocsSemanticKind::ActiveParameter;
      }
      if (style != run_style || kind != run_kind) && !piece.is_empty() {
        docs_push_styled_run(
          &mut runs,
          std::mem::take(&mut piece),
          run_style,
          run_kind,
          None,
        );
      }
      run_style = style;
      run_kind = kind;
      piece.push(ch);
      byte_idx = byte_end;
    }

    docs_push_styled_run(&mut runs, piece, run_style, run_kind, None);
    if runs.is_empty() {
      runs.push(DocsStyledTextRun {
        text:  String::new(),
        style: base_style,
        kind:  DocsSemanticKind::Code,
        href:  None,
      });
    }
    rendered.push(runs);

    line_start_byte += line.len();
    if idx + 1 < code_lines.len() {
      line_start_byte += 1;
    }
  }

  rendered
}

fn docs_highlighted_code_block_lines(
  code_lines: &[String],
  styles: &DocsRenderStyles,
  theme: &Theme,
  loader: Option<&Loader>,
  language: Option<&str>,
  language_hint: Option<&str>,
) -> Vec<Vec<DocsStyledTextRun>> {
  if code_lines.is_empty() {
    return vec![Vec::new()];
  }
  let (code_lines, active_range) = docs_strip_signature_active_markers_from_lines(code_lines);
  if code_lines.is_empty() {
    return vec![Vec::new()];
  }

  let Some(loader) = loader else {
    return docs_render_code_lines_with_active_style(
      &code_lines,
      styles.code,
      styles.active_parameter,
      active_range.as_ref(),
    );
  };

  let resolve_language = |marker: &str| {
    let marker = marker.trim();
    let marker_lower = marker.to_ascii_lowercase();
    loader
      .language_for_name(marker)
      .or_else(|| loader.language_for_name(marker_lower.as_str()))
      .or_else(|| loader.language_for_scope(marker))
      .or_else(|| loader.language_for_scope(marker_lower.as_str()))
      .or_else(|| loader.language_for_filename(Path::new(marker)))
      .or_else(|| {
        language_filename_hints(marker)
          .into_iter()
          .find_map(|hint| loader.language_for_filename(Path::new(format!("tmp.{hint}").as_str())))
      })
  };

  let resolved_language = language.and_then(resolve_language);
  let hinted_language = language_hint.and_then(resolve_language);
  let Some(language) = resolved_language.or(hinted_language) else {
    return docs_render_code_lines_with_active_style(
      &code_lines,
      styles.code,
      styles.active_parameter,
      active_range.as_ref(),
    );
  };

  let joined = code_lines.join("\n");
  let rope = Rope::from_str(&joined);
  let Ok(syntax) = Syntax::new(rope.slice(..), language, loader) else {
    return docs_render_code_lines_with_active_style(
      &code_lines,
      styles.code,
      styles.active_parameter,
      active_range.as_ref(),
    );
  };

  let mut highlights = syntax.collect_highlights(rope.slice(..), loader, 0..rope.len_bytes());
  highlights.sort_by_key(|(_highlight, range)| (range.start, std::cmp::Reverse(range.end)));

  let mut rendered = Vec::with_capacity(code_lines.len());
  let mut line_start_byte = 0usize;

  for (idx, line) in code_lines.iter().enumerate() {
    let mut runs = Vec::new();
    let mut piece = String::new();
    let mut active_style = styles.code;
    let mut active_kind = DocsSemanticKind::Code;
    let mut byte_idx = line_start_byte;

    for ch in line.chars() {
      let byte_end = byte_idx.saturating_add(ch.len_utf8());
      let mut style = docs_preview_highlight_at(&highlights, byte_idx)
        .map(|highlight| styles.code.patch(theme.highlight(highlight)))
        .unwrap_or(styles.code);
      let mut kind = DocsSemanticKind::Code;
      if docs_byte_range_overlaps_active(byte_idx, byte_end, active_range.as_ref()) {
        style = style.patch(styles.active_parameter);
        kind = DocsSemanticKind::ActiveParameter;
      }
      if (style != active_style || kind != active_kind) && !piece.is_empty() {
        docs_push_styled_run(
          &mut runs,
          std::mem::take(&mut piece),
          active_style,
          active_kind,
          None,
        );
      }
      active_style = style;
      active_kind = kind;
      piece.push(ch);
      byte_idx = byte_end;
    }
    docs_push_styled_run(&mut runs, piece, active_style, active_kind, None);
    if runs.is_empty() {
      runs.push(DocsStyledTextRun {
        text:  String::new(),
        style: styles.code,
        kind:  DocsSemanticKind::Code,
        href:  None,
      });
    }
    rendered.push(runs);

    line_start_byte = line_start_byte.saturating_add(line.len());
    if idx + 1 < code_lines.len() {
      line_start_byte = line_start_byte.saturating_add(1);
    }
  }

  rendered
}

fn docs_markdown_lines(
  markdown: &str,
  styles: &DocsRenderStyles,
  theme: &Theme,
  loader: Option<&Loader>,
  language_hint: Option<&str>,
) -> Vec<Vec<DocsStyledTextRun>> {
  let mut lines = Vec::new();
  for block in parse_markdown_blocks(markdown) {
    match block {
      DocsBlock::Paragraph(inline_runs) => {
        lines.push(docs_runs_from_inline(
          &inline_runs,
          styles,
          styles.base,
          DocsSemanticKind::Body,
        ));
      },
      DocsBlock::Heading { level, runs } => {
        let level_idx = level.saturating_sub(1).min(5) as usize;
        lines.push(docs_runs_from_inline(
          &runs,
          styles,
          styles.heading[level_idx],
          DocsSemanticKind::from_heading_level(level),
        ));
      },
      DocsBlock::ListItem {
        marker,
        runs: inline_runs,
      } => {
        let marker_text = match marker {
          DocsListMarker::Bullet => "• ".to_string(),
          DocsListMarker::Ordered(marker) => format!("{marker} "),
        };
        let mut runs = Vec::new();
        docs_push_styled_run(
          &mut runs,
          marker_text,
          styles.bullet,
          DocsSemanticKind::ListMarker,
          None,
        );
        runs.extend(docs_runs_from_inline(
          &inline_runs,
          styles,
          styles.base,
          DocsSemanticKind::Body,
        ));
        lines.push(runs);
      },
      DocsBlock::Quote(inline_runs) => {
        let mut runs = Vec::new();
        docs_push_styled_run(
          &mut runs,
          "│ ".to_string(),
          styles.quote,
          DocsSemanticKind::QuoteMarker,
          None,
        );
        runs.extend(docs_runs_from_inline(
          &inline_runs,
          styles,
          styles.quote,
          DocsSemanticKind::QuoteText,
        ));
        lines.push(runs);
      },
      DocsBlock::CodeFence {
        language,
        lines: code_lines,
      } => {
        lines.extend(docs_highlighted_code_block_lines(
          &code_lines,
          styles,
          theme,
          loader,
          language.as_deref(),
          language_hint,
        ));
      },
      DocsBlock::Rule => {
        lines.push(vec![DocsStyledTextRun {
          text:  "───".to_string(),
          style: styles.rule,
          kind:  DocsSemanticKind::Rule,
          href:  None,
        }]);
      },
      DocsBlock::BlankLine => lines.push(Vec::new()),
    }
  }

  if lines.is_empty() {
    lines.push(Vec::new());
  }
  lines
}

fn docs_wrap_styled_runs(runs: &[DocsStyledTextRun], width: usize) -> Vec<Vec<DocsStyledTextRun>> {
  if width == 0 {
    return Vec::new();
  }
  if runs.is_empty() {
    return vec![Vec::new()];
  }

  let mut wrapped = Vec::new();
  let mut current = Vec::new();
  let mut col = 0usize;

  for run in runs {
    let mut piece = String::new();
    for ch in run.text.chars() {
      if col >= width {
        if !piece.is_empty() {
          docs_push_styled_run(
            &mut current,
            std::mem::take(&mut piece),
            run.style,
            run.kind,
            run.href.clone(),
          );
        }
        wrapped.push(current);
        current = Vec::new();
        col = 0;
      }
      piece.push(ch);
      col += 1;
    }
    if !piece.is_empty() {
      docs_push_styled_run(&mut current, piece, run.style, run.kind, run.href.clone());
    }
  }

  if current.is_empty() {
    wrapped.push(Vec::new());
  } else {
    wrapped.push(current);
  }
  wrapped
}

fn docs_rows_with_context(
  markdown: &str,
  styles: &DocsRenderStyles,
  width: usize,
  theme: &Theme,
  loader: Option<&Loader>,
  language_hint: Option<&str>,
) -> Vec<Vec<DocsStyledTextRun>> {
  let mut rows = Vec::new();
  for line in docs_markdown_lines(markdown, styles, theme, loader, language_hint) {
    rows.extend(docs_wrap_styled_runs(&line, width));
  }
  if rows.is_empty() {
    rows.push(Vec::new());
  }
  rows
}

fn docs_style_snapshot(style: LibStyle) -> DocsStyleSnapshot {
  let ffi_style = ffi::Style::from(style);
  DocsStyleSnapshot {
    has_fg:              ffi_style.has_fg,
    fg:                  DocsColorSnapshot::from(ffi_style.fg),
    has_bg:              ffi_style.has_bg,
    bg:                  DocsColorSnapshot::from(ffi_style.bg),
    has_underline_color: ffi_style.has_underline_color,
    underline_color:     DocsColorSnapshot::from(ffi_style.underline_color),
    underline_style:     ffi_style.underline_style,
    add_modifier:        ffi_style.add_modifier,
    sub_modifier:        ffi_style.sub_modifier,
  }
}

struct DocsRenderRuntime {
  theme:  Theme,
  loader: Option<Arc<Loader>>,
}

fn docs_render_runtime() -> &'static DocsRenderRuntime {
  static RUNTIME: OnceLock<DocsRenderRuntime> = OnceLock::new();
  RUNTIME.get_or_init(|| {
    let theme = select_ui_theme();
    let loader = init_loader(&theme).ok().map(Arc::new);
    DocsRenderRuntime { theme, loader }
  })
}

fn completion_docs_render_json_impl(
  markdown: &str,
  content_width: usize,
  language_hint: &str,
) -> String {
  let runtime = docs_render_runtime();
  let base = runtime
    .theme
    .try_get("ui.text")
    .or_else(|| runtime.theme.try_get("ui.text.focus"))
    .unwrap_or_default();
  let styles = docs_render_styles(&runtime.theme, base);
  let width = content_width.max(1);
  let language_hint = (!language_hint.trim().is_empty()).then_some(language_hint.trim());
  let rows = docs_rows_with_context(
    markdown,
    &styles,
    width,
    &runtime.theme,
    runtime.loader.as_deref(),
    language_hint,
  );

  let lines = rows
    .into_iter()
    .map(|line| {
      line
        .into_iter()
        .map(|run| {
          DocsRunSnapshot {
            text:  run.text,
            kind:  run.kind,
            href:  run.href,
            style: docs_style_snapshot(run.style),
          }
        })
        .collect::<Vec<_>>()
    })
    .collect::<Vec<_>>();
  let snapshot = DocsRenderSnapshot { lines };
  serde_json::to_string(&snapshot).unwrap_or_else(|_| "{\"lines\":[]}".to_string())
}

#[derive(Serialize)]
struct CompletionPopupRectSnapshot {
  x:      u16,
  y:      u16,
  width:  u16,
  height: u16,
}

impl From<DefaultOverlayRect> for CompletionPopupRectSnapshot {
  fn from(value: DefaultOverlayRect) -> Self {
    Self {
      x:      value.x,
      y:      value.y,
      width:  value.width,
      height: value.height,
    }
  }
}

#[derive(Serialize)]
struct CompletionPopupLayoutSnapshot {
  list: CompletionPopupRectSnapshot,
  docs: Option<CompletionPopupRectSnapshot>,
}

fn completion_popup_layout_json_impl(
  area_width: usize,
  area_height: usize,
  cursor_x: i64,
  cursor_y: i64,
  list_width: usize,
  list_height: usize,
  docs_width: usize,
  docs_height: usize,
) -> String {
  let area = DefaultOverlayRect::new(
    0,
    0,
    area_width.min(u16::MAX as usize) as u16,
    area_height.min(u16::MAX as usize) as u16,
  );
  let list_width = list_width.min(u16::MAX as usize) as u16;
  let list_height = list_height.min(u16::MAX as usize) as u16;
  let docs_width = docs_width.min(u16::MAX as usize) as u16;
  let docs_height = docs_height.min(u16::MAX as usize) as u16;

  let cursor = if cursor_x >= 0 && cursor_y >= 0 {
    Some((
      cursor_x.min(u16::MAX as i64) as u16,
      cursor_y.min(u16::MAX as i64) as u16,
    ))
  } else {
    None
  };

  let list = default_completion_panel_rect(area, list_width, list_height, cursor);
  let docs = if docs_width > 0 && docs_height > 0 {
    default_completion_docs_panel_rect(area, docs_width, docs_height, list)
  } else {
    None
  };

  let snapshot = CompletionPopupLayoutSnapshot {
    list: CompletionPopupRectSnapshot::from(list),
    docs: docs.map(CompletionPopupRectSnapshot::from),
  };
  serde_json::to_string(&snapshot).unwrap_or_else(|_| {
    "{\"list\":{\"x\":0,\"y\":0,\"width\":1,\"height\":1},\"docs\":null}".to_string()
  })
}

#[derive(Serialize)]
struct SignatureHelpPopupLayoutSnapshot {
  panel: CompletionPopupRectSnapshot,
}

fn signature_help_popup_layout_json_impl(
  area_width: usize,
  area_height: usize,
  cursor_x: i64,
  cursor_y: i64,
  panel_width: usize,
  panel_height: usize,
) -> String {
  let area = DefaultOverlayRect::new(
    0,
    0,
    area_width.min(u16::MAX as usize) as u16,
    area_height.min(u16::MAX as usize) as u16,
  );
  let panel_width = panel_width.min(u16::MAX as usize) as u16;
  let panel_height = panel_height.min(u16::MAX as usize) as u16;

  let cursor = if cursor_x >= 0 && cursor_y >= 0 {
    Some((
      cursor_x.min(u16::MAX as i64) as u16,
      cursor_y.min(u16::MAX as i64) as u16,
    ))
  } else {
    None
  };

  let panel = default_signature_help_panel_rect(area, panel_width, panel_height, cursor);
  let snapshot = SignatureHelpPopupLayoutSnapshot {
    panel: CompletionPopupRectSnapshot::from(panel),
  };
  serde_json::to_string(&snapshot)
    .unwrap_or_else(|_| "{\"panel\":{\"x\":0,\"y\":0,\"width\":1,\"height\":1}}".to_string())
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

fn build_transaction_from_lsp_text_edits(
  text: &Rope,
  edits: &[the_lsp::LspTextEdit],
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

fn ffi_ui_profile_enabled() -> bool {
  static ENABLED: OnceLock<bool> = OnceLock::new();
  *ENABLED.get_or_init(|| {
    env::var("THE_FFI_PROFILE_UI")
      .ok()
      .as_deref()
      .is_some_and(|value| value == "1")
  })
}

fn ffi_ui_profile_min_duration() -> Duration {
  static MIN_MS: OnceLock<u64> = OnceLock::new();
  let min_ms = *MIN_MS.get_or_init(|| {
    env::var("THE_FFI_PROFILE_UI_MIN_MS")
      .ok()
      .and_then(|value| value.parse::<u64>().ok())
      .unwrap_or(8)
  });
  Duration::from_millis(min_ms)
}

fn ffi_ui_profile_should_log(elapsed: Duration) -> bool {
  ffi_ui_profile_enabled() && elapsed >= ffi_ui_profile_min_duration()
}

fn ffi_ui_profile_log(message: impl AsRef<str>) {
  if ffi_ui_profile_enabled() {
    eprintln!("[the-ffi profile] {}", message.as_ref());
  }
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

/// FFI-safe app wrapper with editor management.
pub struct App {
  inner:                           LibApp,
  dispatch:                        DefaultDispatchStatic<App>,
  keymaps:                         Keymaps,
  command_registry:                CommandRegistry<App>,
  states:                          HashMap<LibEditorId, EditorState>,
  vcs_provider:                    DiffProviderRegistry,
  vcs_diff_handles:                HashMap<LibEditorId, DiffHandle>,
  active_editor:                   Option<LibEditorId>,
  should_quit:                     bool,
  registers:                       Registers,
  last_motion:                     Option<Motion>,
  lsp_runtime:                     LspRuntime,
  lsp_ready:                       bool,
  lsp_document:                    Option<LspDocumentSyncState>,
  lsp_statusline:                  LspStatuslineState,
  lsp_spinner_index:               usize,
  lsp_spinner_last_tick:           Instant,
  lsp_active_progress_tokens:      HashSet<String>,
  lsp_watched_file:                Option<LspWatchedFileState>,
  lsp_pending_requests:            HashMap<u64, PendingLspRequestKind>,
  lsp_completion_items:            Vec<LspCompletionItem>,
  lsp_completion_raw_items:        Vec<Value>,
  lsp_completion_resolved:         HashSet<usize>,
  lsp_completion_visible:          Vec<usize>,
  lsp_completion_start:            Option<usize>,
  lsp_completion_generation:       u64,
  lsp_pending_auto_completion:     Option<PendingAutoCompletion>,
  lsp_pending_auto_signature_help: Option<PendingAutoSignatureHelp>,
  diagnostics:                     DiagnosticsState,
  inline_diagnostic_lines:         Vec<InlineDiagnosticRenderLine>,
  eol_diagnostics:                 Vec<EolDiagnosticEntry>,
  diagnostic_underlines:           Vec<DiagnosticUnderlineEntry>,
  ui_theme:                        Theme,
  loader:                          Option<Arc<Loader>>,
}

// MARK: - File picker preview FFI types

/// A single highlight span within a preview line, using char offsets.
pub struct PreviewLineSpan {
  char_start:   u32,
  char_end:     u32,
  highlight_id: u32,
}

/// A single line in a source preview, with pre-computed char-offset spans.
pub struct PreviewLine {
  text:  String,
  spans: Vec<PreviewLineSpan>,
}

impl Default for PreviewLine {
  fn default() -> Self {
    Self {
      text:  String::new(),
      spans: Vec::new(),
    }
  }
}

impl PreviewLine {
  fn text(&self) -> String {
    self.text.clone()
  }

  fn span_count(&self) -> usize {
    self.spans.len()
  }

  fn span_char_start(&self, index: usize) -> u32 {
    self.spans.get(index).map(|s| s.char_start).unwrap_or(0)
  }

  fn span_char_end(&self, index: usize) -> u32 {
    self.spans.get(index).map(|s| s.char_end).unwrap_or(0)
  }

  fn span_highlight(&self, index: usize) -> u32 {
    self.spans.get(index).map(|s| s.highlight_id).unwrap_or(0)
  }
}

/// Snapshot of the file picker preview, ready for Swift consumption.
/// Built once per selection change — Swift reads fields via accessors.
pub struct PreviewData {
  kind:        u8, // 0=empty, 1=source, 2=text, 3=message
  path:        String,
  text:        String,
  loading:     bool,
  truncated:   bool,
  total_lines: usize,
  show:        bool,
  lines:       Vec<PreviewLine>,
}

impl Default for PreviewData {
  fn default() -> Self {
    Self {
      kind:        0,
      path:        String::new(),
      text:        String::new(),
      loading:     false,
      truncated:   false,
      total_lines: 0,
      show:        true,
      lines:       Vec::new(),
    }
  }
}

impl PreviewData {
  fn kind(&self) -> u8 {
    self.kind
  }

  fn path(&self) -> String {
    self.path.clone()
  }

  fn text(&self) -> String {
    self.text.clone()
  }

  fn loading(&self) -> bool {
    self.loading
  }

  fn truncated(&self) -> bool {
    self.truncated
  }

  fn total_lines(&self) -> usize {
    self.total_lines
  }

  fn show(&self) -> bool {
    self.show
  }

  fn line_count(&self) -> usize {
    self.lines.len()
  }

  fn line_at(&self, index: usize) -> PreviewLine {
    self
      .lines
      .get(index)
      .map(|l| {
        PreviewLine {
          text:  l.text.clone(),
          spans: l
            .spans
            .iter()
            .map(|s| {
              PreviewLineSpan {
                char_start:   s.char_start,
                char_end:     s.char_end,
                highlight_id: s.highlight_id,
              }
            })
            .collect(),
        }
      })
      .unwrap_or_default()
  }
}

fn build_preview_data(picker: &FilePickerState) -> PreviewData {
  let preview_path = picker
    .preview_path
    .as_ref()
    .map(|p| {
      p.strip_prefix(&picker.root)
        .unwrap_or(p)
        .display()
        .to_string()
    })
    .unwrap_or_default();

  match &picker.preview {
    FilePickerPreview::Empty => {
      PreviewData {
        kind: 0,
        path: preview_path,
        show: picker.show_preview,
        ..Default::default()
      }
    },
    FilePickerPreview::Source(source) => {
      let lines: Vec<PreviewLine> = source
        .lines
        .iter()
        .enumerate()
        .map(|(line_idx, line_text)| {
          let line_start = source.line_starts.get(line_idx).copied().unwrap_or(0);
          let line_end = source
            .line_starts
            .get(line_idx + 1)
            .copied()
            .unwrap_or(line_start + line_text.len());

          let spans: Vec<PreviewLineSpan> = source
            .highlights
            .iter()
            .filter(|(_hl, range)| range.start < line_end && range.end > line_start)
            .map(|(hl, range)| {
              let span_start_byte = range.start.saturating_sub(line_start).min(line_text.len());
              let span_end_byte = range.end.saturating_sub(line_start).min(line_text.len());
              let char_start = line_text[..span_start_byte].chars().count() as u32;
              let char_end = line_text[..span_end_byte].chars().count() as u32;
              PreviewLineSpan {
                char_start,
                char_end,
                highlight_id: hl.get(),
              }
            })
            .collect();

          PreviewLine {
            text: line_text.clone(),
            spans,
          }
        })
        .collect();

      PreviewData {
        kind: 1,
        path: preview_path,
        loading: picker.preview_loading(),
        truncated: source.truncated,
        total_lines: source.lines.len(),
        show: picker.show_preview,
        lines,
        ..Default::default()
      }
    },
    FilePickerPreview::Text(t) => {
      PreviewData {
        kind: 2,
        path: preview_path,
        text: t.clone(),
        show: picker.show_preview,
        ..Default::default()
      }
    },
    FilePickerPreview::Message(msg) => {
      PreviewData {
        kind: 3,
        path: preview_path,
        text: msg.clone(),
        show: picker.show_preview,
        ..Default::default()
      }
    },
  }
}

// ── File picker snapshot (direct FFI, no JSON) ─────────────────────────

pub struct FilePickerSnapshotData {
  active:        bool,
  query:         String,
  matched_count: usize,
  total_count:   usize,
  scanning:      bool,
  root:          String,
  items:         Vec<FilePickerItemFFI>,
}

impl Default for FilePickerSnapshotData {
  fn default() -> Self {
    Self {
      active:        false,
      query:         String::new(),
      matched_count: 0,
      total_count:   0,
      scanning:      false,
      root:          String::new(),
      items:         Vec::new(),
    }
  }
}

impl FilePickerSnapshotData {
  fn active(&self) -> bool {
    self.active
  }
  fn query(&self) -> String {
    self.query.clone()
  }
  fn matched_count(&self) -> usize {
    self.matched_count
  }
  fn total_count(&self) -> usize {
    self.total_count
  }
  fn scanning(&self) -> bool {
    self.scanning
  }
  fn root(&self) -> String {
    self.root.clone()
  }

  fn item_count(&self) -> usize {
    self.items.len()
  }

  fn item_at(&self, index: usize) -> FilePickerItemFFI {
    self.items.get(index).cloned().unwrap_or_default()
  }
}

#[derive(Clone)]
pub struct FilePickerItemFFI {
  display:       String,
  is_dir:        bool,
  icon:          String,
  match_indices: Vec<u32>,
}

impl Default for FilePickerItemFFI {
  fn default() -> Self {
    Self {
      display:       String::new(),
      is_dir:        false,
      icon:          String::new(),
      match_indices: Vec::new(),
    }
  }
}

impl FilePickerItemFFI {
  fn display(&self) -> String {
    self.display.clone()
  }
  fn is_dir(&self) -> bool {
    self.is_dir
  }
  fn icon(&self) -> String {
    self.icon.clone()
  }
  fn match_index_count(&self) -> usize {
    self.match_indices.len()
  }
  fn match_index_at(&self, index: usize) -> u32 {
    self.match_indices.get(index).copied().unwrap_or(0)
  }
}

fn build_file_picker_snapshot(
  picker: &FilePickerState,
  max_items: usize,
) -> FilePickerSnapshotData {
  if !picker.active {
    return FilePickerSnapshotData::default();
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
  let mut match_buf = Vec::new();
  for i in 0..limit {
    if let Some(item) = picker.matched_item_with_match_indices(i, &mut match_buf) {
      items.push(FilePickerItemFFI {
        display:       item.display.to_string(),
        is_dir:        item.is_dir,
        icon:          item.icon.to_string(),
        match_indices: match_buf.iter().map(|&idx| idx as u32).collect(),
      });
    }
  }

  FilePickerSnapshotData {
    active: true,
    query: picker.query.clone(),
    matched_count,
    total_count,
    scanning,
    root: root_display,
    items,
  }
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
      lsp_completion_items: Vec::new(),
      lsp_completion_raw_items: Vec::new(),
      lsp_completion_resolved: HashSet::new(),
      lsp_completion_visible: Vec::new(),
      lsp_completion_start: None,
      lsp_completion_generation: 0,
      lsp_pending_auto_completion: None,
      lsp_pending_auto_signature_help: None,
      diagnostics: DiagnosticsState::default(),
      inline_diagnostic_lines: Vec::new(),
      eol_diagnostics: Vec::new(),
      diagnostic_underlines: Vec::new(),
      ui_theme,
      loader,
    }
  }

  pub fn completion_docs_render_json(
    markdown: &str,
    content_width: usize,
    language_hint: &str,
  ) -> String {
    completion_docs_render_json_impl(markdown, content_width, language_hint)
  }

  pub fn completion_popup_layout_json(
    area_width: usize,
    area_height: usize,
    cursor_x: i64,
    cursor_y: i64,
    list_width: usize,
    list_height: usize,
    docs_width: usize,
    docs_height: usize,
  ) -> String {
    completion_popup_layout_json_impl(
      area_width,
      area_height,
      cursor_x,
      cursor_y,
      list_width,
      list_height,
      docs_width,
      docs_height,
    )
  }

  pub fn signature_help_popup_layout_json(
    area_width: usize,
    area_height: usize,
    cursor_x: i64,
    cursor_y: i64,
    panel_width: usize,
    panel_height: usize,
  ) -> String {
    signature_help_popup_layout_json_impl(
      area_width,
      area_height,
      cursor_x,
      cursor_y,
      panel_width,
      panel_height,
    )
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
    let started = Instant::now();
    if self.activate(id).is_none() {
      return RenderPlan::empty();
    }
    let _ = self.poll_background_active();

    let plan = the_default::render_plan(self);
    let inline_diagnostic_lines = std::mem::take(&mut self.inline_diagnostic_lines);
    let eol_diagnostics = std::mem::take(&mut self.eol_diagnostics);
    let diagnostic_underlines = std::mem::take(&mut self.diagnostic_underlines);
    let elapsed = started.elapsed();
    if ffi_ui_profile_should_log(elapsed) {
      ffi_ui_profile_log(format!("render_plan elapsed={}ms", elapsed.as_millis()));
    }
    RenderPlan {
      inner: plan,
      inline_diagnostic_lines,
      eol_diagnostics,
      diagnostic_underlines,
    }
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
    let inline_diagnostic_lines = std::mem::take(&mut self.inline_diagnostic_lines);
    let eol_diagnostics = std::mem::take(&mut self.eol_diagnostics);
    let diagnostic_underlines = std::mem::take(&mut self.diagnostic_underlines);
    RenderPlan {
      inner: plan,
      inline_diagnostic_lines,
      eol_diagnostics,
      diagnostic_underlines,
    }
  }

  pub fn ui_tree_json(&mut self, id: ffi::EditorId) -> String {
    let started = Instant::now();
    if self.activate(id).is_none() {
      return "{}".to_string();
    }
    let _ = self.poll_background_active();

    let mut tree = the_default::ui_tree(self);
    self.append_lsp_hover_overlay(&mut tree);
    let json = serde_json::to_string(&tree).unwrap_or_else(|_| "{}".to_string());
    let elapsed = started.elapsed();
    if ffi_ui_profile_should_log(elapsed) {
      ffi_ui_profile_log(format!(
        "ui_tree_json elapsed={}ms bytes={}",
        elapsed.as_millis(),
        json.len()
      ));
    }
    json
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
      word_jump_inline_annotations,
      word_jump_overlay_annotations,
      allow_cache_refresh,
    ) = {
      let state = self.active_state_ref();
      (
        state.text_format.clone(),
        state.gutter_config.clone(),
        state.gutter_diff_signs.clone(),
        state.inline_annotations.clone(),
        state.overlay_annotations.clone(),
        state.word_jump_inline_annotations.clone(),
        state.word_jump_overlay_annotations.clone(),
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
    let inline_diagnostics = self.active_inline_diagnostics();
    let enable_cursor_line = self.active_state_ref().mode != Mode::Insert;
    let jump_label_style = self.ui_theme.find_highlight("ui.virtual.jump-label");

    let raw_diagnostics = self
      .lsp_document
      .as_ref()
      .filter(|state| state.opened)
      .and_then(|state| self.diagnostics.document(&state.uri))
      .map(|doc| doc.diagnostics.clone())
      .unwrap_or_default();

    let inline_diagnostic_render_data: SharedInlineDiagnosticsRenderData =
      Rc::new(RefCell::new(InlineDiagnosticsRenderData::default()));

    let (mut plan, underlines, inline_lines) = {
      let editor = self.active_editor_mut();
      let view = editor.view();

      let mut annotations = TextAnnotations::default();
      if !inline_annotations.is_empty() {
        let _ = annotations.add_inline_annotations(&inline_annotations, None);
      }
      if !overlay_annotations.is_empty() {
        let _ = annotations.add_overlay(&overlay_annotations, None);
      }
      if !word_jump_inline_annotations.is_empty() {
        let _ = annotations.add_inline_annotations(&word_jump_inline_annotations, None);
      }
      if !word_jump_overlay_annotations.is_empty() {
        let _ = annotations.add_overlay(&word_jump_overlay_annotations, jump_label_style);
      }

      let (doc, cache) = editor.document_and_cache();
      let gutter_width = gutter_width_for_document(doc, view.viewport.width, &gutter_config);
      text_fmt.viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1);

      let inline_config = if !inline_diagnostics.is_empty() {
        InlineDiagnosticsConfig::default()
          .prepare(text_fmt.viewport_width.max(1), enable_cursor_line)
      } else {
        InlineDiagnosticsConfig {
          cursor_line: InlineDiagnosticFilter::Disable,
          other_lines: InlineDiagnosticFilter::Disable,
          ..InlineDiagnosticsConfig::default()
        }
      };
      if !inline_diagnostics.is_empty() && !inline_config.disabled() {
        let cursor_char_idx = doc
          .selection()
          .ranges()
          .first()
          .map(|r| r.cursor(doc.text().slice(..)))
          .unwrap_or(0);
        let cursor_line_idx = doc
          .selection()
          .ranges()
          .first()
          .map(|r| r.cursor_line(doc.text().slice(..)));
        let _ = annotations.add_line_annotation(Box::new(InlineDiagnosticsLineAnnotation::new(
          inline_diagnostics,
          cursor_char_idx,
          cursor_line_idx,
          text_fmt.viewport_width.max(1),
          view.scroll.col,
          inline_config,
          inline_diagnostic_render_data.clone(),
        )));
      }

      let plan = if let (Some(loader), Some(syntax)) = (loader.as_deref(), doc.syntax()) {
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
      };

      // Snapshot inline diagnostic render output now. Subsequent visual position
      // queries (e.g. underline mapping) can traverse annotations again and
      // should not duplicate overlay lines.
      let mut inline_lines = inline_diagnostic_render_data.borrow().lines.clone();
      dedupe_inline_diagnostic_lines(&mut inline_lines);

      // Compute diagnostic underlines after build_plan while annotations are still
      // alive.
      let underlines = compute_diagnostic_underlines(
        doc.text(),
        &raw_diagnostics,
        &plan,
        &text_fmt,
        &mut annotations,
      );

      (plan, underlines, inline_lines)
    };
    apply_diagnostic_gutter_markers(&mut plan, &diagnostics_by_line, diagnostic_styles);
    apply_diff_gutter_markers(&mut plan, &diff_signs, diff_styles);

    self.inline_diagnostic_lines = inline_lines;
    self.eol_diagnostics = compute_eol_diagnostics(&raw_diagnostics, &plan);
    self.diagnostic_underlines = underlines;
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

    let is_prefiltered = self.active_state_ref().command_palette.prefiltered;

    if is_prefiltered {
      // Check if the selected item is a directory (ends with '/').
      let is_dir = self
        .active_state_ref()
        .command_palette
        .items
        .get(item_idx)
        .is_some_and(|item| item.title.ends_with('/'));

      // Apply the completion to the prompt input.
      let completion = self.command_prompt_ref().completions.get(item_idx).cloned();
      if let Some(completion) = completion {
        let prompt = self.command_prompt_mut();
        let start = completion.range.start.min(prompt.input.len());
        prompt.input.replace_range(start.., &completion.text);
        prompt.cursor = prompt.input.len();
      }

      if is_dir {
        // Directory — expand instead of executing.
        let input = self.command_prompt_ref().input.clone();
        update_command_palette_for_input(self, &input);
        return true;
      }

      // File — execute the full command line.
      let line = self
        .command_prompt_ref()
        .input
        .trim()
        .trim_start_matches(':')
        .to_string();
      let (command, args, _) = command_line_split(&line);

      if command.is_empty() {
        return false;
      }

      let registry = self.command_registry_ref() as *const CommandRegistry<App>;
      let result = unsafe { (&*registry).execute(self, command, args, CommandEvent::Validate) };

      match result {
        Ok(()) => {
          self.set_mode(Mode::Normal);
          self.command_prompt_mut().clear();
          let palette = self.command_palette_mut();
          palette.is_open = false;
          palette.query.clear();
          palette.selected = None;
          palette.prompt_text = None;
          self.request_render();
          true
        },
        Err(err) => {
          self.command_prompt_mut().error = Some(err.to_string());
          self.request_render();
          false
        },
      }
    } else {
      // Normal mode — item title is the command name.
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
          palette.prompt_text = None;
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
    palette.prompt_text = None;
    self.request_render();
    true
  }

  pub fn command_palette_set_query(&mut self, id: ffi::EditorId, query: &str) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    update_command_palette_for_input(self, query);
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
    update_search_prompt_preview(self);
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
    let should_close = match self.search_prompt_ref().kind {
      SearchPromptKind::Search => finalize_search(self),
      SearchPromptKind::SelectRegex => finalize_select_regex(self),
    };
    if should_close {
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

  pub fn file_picker_select_index(&mut self, id: ffi::EditorId, index: usize) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    if !self.file_picker().active {
      return false;
    }
    select_file_picker_index(self, index);
    true
  }

  pub fn file_picker_snapshot(
    &mut self,
    id: ffi::EditorId,
    max_items: usize,
  ) -> FilePickerSnapshotData {
    let started = Instant::now();
    if self.activate(id).is_none() {
      return FilePickerSnapshotData::default();
    }
    let picker = self.file_picker_mut();
    file_picker_poll_scan_results(picker);
    file_picker_refresh_matcher_state(picker);

    let picker = self.file_picker();
    let data = build_file_picker_snapshot(picker, max_items);
    let elapsed = started.elapsed();
    ffi_ui_profile_log(format!(
      "file_picker_snapshot items={} elapsed={:.2}ms",
      data.items.len(),
      elapsed.as_secs_f64() * 1000.0
    ));
    data
  }

  /// Direct FFI preview data — no JSON serialization.
  pub fn file_picker_preview(&mut self, id: ffi::EditorId) -> PreviewData {
    if self.activate(id).is_none() {
      return PreviewData::default();
    }
    let picker = self.file_picker();
    if !picker.active {
      return PreviewData::default();
    }
    build_preview_data(picker)
  }

  pub fn take_should_quit(&mut self) -> bool {
    let should_quit = self.should_quit;
    self.should_quit = false;
    should_quit
  }

  pub fn poll_background(&mut self, id: ffi::EditorId) -> bool {
    let started = Instant::now();
    if self.activate(id).is_none() {
      return false;
    }
    let changed = self.poll_background_active();
    let elapsed = started.elapsed();
    if ffi_ui_profile_should_log(elapsed) {
      ffi_ui_profile_log(format!(
        "poll_background changed={} elapsed={}ms",
        changed,
        elapsed.as_millis()
      ));
    }
    changed
  }

  fn poll_background_active(&mut self) -> bool {
    let mut changed = false;
    if self.poll_lsp_completion_auto_trigger() {
      changed = true;
    }
    if self.poll_lsp_signature_help_auto_trigger() {
      changed = true;
    }
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
    if self.file_picker().active {
      let picker = self.file_picker_mut();
      if file_picker_poll_scan_results(picker) {
        changed = true;
      }
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
    self.clear_hover_state();
    self.clear_completion_state();
    self.cancel_auto_completion();
    self.clear_signature_help_state();
    self.cancel_auto_signature_help();
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

  fn poll_lsp_completion_auto_trigger(&mut self) -> bool {
    let Some(pending) = self.lsp_pending_auto_completion.clone() else {
      return false;
    };
    if self.active_state_ref().mode != Mode::Insert {
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

  fn poll_lsp_signature_help_auto_trigger(&mut self) -> bool {
    let Some(pending) = self.lsp_pending_auto_signature_help.clone() else {
      return false;
    };
    if self.active_state_ref().mode != Mode::Insert {
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
          self.clear_hover_state();
          self.clear_completion_state();
          self.cancel_auto_completion();
          self.clear_signature_help_state();
          self.cancel_auto_signature_help();
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
          self.clear_hover_state();
          self.clear_completion_state();
          self.cancel_auto_completion();
          self.clear_signature_help_state();
          self.cancel_auto_signature_help();
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

  fn active_inline_diagnostics(&self) -> Vec<InlineDiagnostic> {
    let Some(state) = self.lsp_document.as_ref().filter(|state| state.opened) else {
      return Vec::new();
    };
    let Some(document) = self.diagnostics.document(&state.uri) else {
      return Vec::new();
    };
    let text = self.active_editor_ref().document().text();
    let mut out = Vec::with_capacity(document.diagnostics.len());
    for diagnostic in &document.diagnostics {
      let message = diagnostic.message.trim();
      if message.is_empty() {
        continue;
      }
      let start_char_idx = utf16_position_to_char_idx(
        text,
        diagnostic.range.start.line,
        diagnostic.range.start.character,
      );
      let severity = diagnostic.severity.unwrap_or(DiagnosticSeverity::Warning);
      out.push(InlineDiagnostic::new(
        start_char_idx,
        severity,
        message.to_string(),
      ));
    }
    out.sort_by_key(|d| d.start_char_idx);
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
      PendingLspRequestKind::Hover { .. } => {
        let hover = match parse_hover_response(response.result.as_ref()) {
          Ok(hover) => hover,
          Err(err) => {
            self.publish_lsp_message(
              the_lib::messages::MessageLevel::Error,
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
              self.publish_lsp_message(
                the_lib::messages::MessageLevel::Info,
                "no hover information",
              );
            } else {
              let state = self.active_state_mut();
              state.hover_docs = Some(trimmed.to_string());
              state.hover_docs_scroll = 0;
            }
          },
          None => {
            self.clear_hover_state();
            self.publish_lsp_message(
              the_lib::messages::MessageLevel::Info,
              "no hover information",
            );
          },
        }
        true
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
    }
  }

  fn handle_completion_response(
    &mut self,
    result: Option<&Value>,
    generation: u64,
    request_cursor: usize,
    replace_start: usize,
    announce_empty: bool,
  ) -> bool {
    let started = Instant::now();
    if generation != self.lsp_completion_generation {
      return false;
    }
    if self.active_state_ref().mode != Mode::Insert {
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
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Error,
          format!("failed to parse completion response: {err}"),
        );
        return true;
      },
    };

    if completion.items.is_empty() {
      self.clear_completion_state();
      if announce_empty {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Info,
          "no completion candidates",
        );
      }
      return true;
    }

    self.lsp_completion_items = completion.items;
    self.lsp_completion_raw_items = completion.raw_items;
    self.lsp_completion_resolved.clear();
    self.lsp_completion_start = Some(replace_start.min(request_cursor));
    self.rebuild_completion_menu();
    let elapsed = started.elapsed();
    if ffi_ui_profile_should_log(elapsed) {
      ffi_ui_profile_log(format!(
        "handle_completion_response elapsed={}ms total_items={}",
        elapsed.as_millis(),
        self.lsp_completion_items.len()
      ));
    }
    true
  }

  fn handle_completion_resolve_response(&mut self, index: usize, result: Option<&Value>) -> bool {
    let resolved = match parse_completion_item_response(result) {
      Ok(item) => item,
      Err(err) => {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Warning,
          format!("failed to parse completion resolve response: {err}"),
        );
        return true;
      },
    };

    self.lsp_completion_resolved.insert(index);

    let Some(resolved) = resolved else {
      return true;
    };
    let updated_ui_item = {
      let Some(item) = self.lsp_completion_items.get_mut(index) else {
        return true;
      };
      merge_resolved_completion_item(item, resolved);
      completion_menu_item_for_lsp_item(item)
    };

    if let Some(visible_index) = self
      .lsp_completion_visible
      .iter()
      .position(|candidate| *candidate == index)
      && let Some(ui_item) = self.completion_menu_mut().items.get_mut(visible_index)
    {
      *ui_item = updated_ui_item;
      self.request_render();
    }
    true
  }

  fn handle_signature_help_response(&mut self, result: Option<&Value>) -> bool {
    let signature = match parse_signature_help_response(result) {
      Ok(signature) => signature,
      Err(err) => {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Error,
          format!("failed to parse signature help response: {err}"),
        );
        return true;
      },
    };

    let Some(signature) = signature else {
      the_default::close_signature_help(self);
      return true;
    };

    if signature.signatures.is_empty() {
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
    the_default::show_signature_help(self, signatures, active_signature);
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
      let snippet_base =
        if prepared.cursor_origin == Some(CompletionSnippetCursorOrigin::PrimaryEdit) {
          item.primary_edit.as_ref().map(|edit| {
            let doc = self.active_editor_ref().document();
            utf16_position_to_char_idx(
              doc.text(),
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

      let tx = {
        let doc = self.active_editor_ref().document();
        match build_transaction_from_lsp_text_edits(doc.text(), &edits) {
          Ok(tx) => tx,
          Err(err) => {
            self.publish_lsp_message(
              the_lib::messages::MessageLevel::Error,
              format!("failed to build completion transaction: {err}"),
            );
            return false;
          },
        }
      };

      if <Self as DefaultContext>::apply_transaction(self, &tx) {
        if let (Some(base), Some(range)) = (snippet_base, prepared.cursor_range.as_ref())
          && let Ok(mapped_base) = tx.changes().map_pos(base, Assoc::Before)
        {
          let doc = self.active_editor_mut().document_mut();
          set_completion_snippet_selection(doc, mapped_base, range);
        }
        let _ = self.active_editor_mut().document_mut().commit();
        self.request_render();
        return true;
      }

      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Error,
        "failed to apply completion",
      );
      return false;
    }

    let insert_text = item.insert_text.unwrap_or(item.label);
    if insert_text.is_empty() {
      return false;
    }

    let (from, tx) = {
      let doc = self.active_editor_ref().document();
      let text = doc.text();
      let text_len = text.len_chars();
      let from = fallback_range.start.min(text_len);
      let to = fallback_range.end.min(text_len).max(from);
      let tx = match Transaction::change(text, vec![(from, to, Some(insert_text.into()))]) {
        Ok(tx) => tx,
        Err(err) => {
          self.publish_lsp_message(
            the_lib::messages::MessageLevel::Error,
            format!("failed to build completion transaction: {err}"),
          );
          return false;
        },
      };
      (from, tx)
    };

    if <Self as DefaultContext>::apply_transaction(self, &tx) {
      if prepared.cursor_origin == Some(CompletionSnippetCursorOrigin::InsertText)
        && let Some(range) = prepared.cursor_range.as_ref()
        && let Ok(mapped_base) = tx.changes().map_pos(from, Assoc::Before)
      {
        let doc = self.active_editor_mut().document_mut();
        set_completion_snippet_selection(doc, mapped_base, range);
      }
      let _ = self.active_editor_mut().document_mut().commit();
      self.request_render();
      return true;
    }

    self.publish_lsp_message(
      the_lib::messages::MessageLevel::Error,
      "failed to apply completion",
    );
    false
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

  fn primary_cursor_char_idx(&self) -> Option<usize> {
    let doc = self.active_editor_ref().document();
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
      .active_editor_ref()
      .document()
      .text()
      .get_char(cursor.saturating_sub(1))
      .is_some_and(is_symbol_word_char)
  }

  fn completion_replace_start_at_cursor(&self, cursor: usize) -> usize {
    let text = self.active_editor_ref().document().text();
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
    let Some(server) = self.lsp_runtime.config().server() else {
      return false;
    };
    let Some(capabilities) = self.lsp_runtime.server_capabilities(server.name()) else {
      return false;
    };
    capabilities_support_single_char(capabilities.raw(), provider_key, characters_key, ch)
  }

  fn lsp_completion_supports_trigger_char(&self, ch: char) -> bool {
    self.lsp_provider_supports_single_char("completionProvider", "triggerCharacters", ch)
  }

  fn lsp_signature_help_supports_trigger_char(&self, ch: char) -> bool {
    self.lsp_provider_supports_single_char("signatureHelpProvider", "triggerCharacters", ch)
  }

  fn lsp_signature_help_supports_retrigger_char(&self, ch: char) -> bool {
    self.lsp_provider_supports_single_char("signatureHelpProvider", "retriggerCharacters", ch)
  }

  fn completion_source_index_for_visible_index(&self, index: usize) -> Option<usize> {
    self.lsp_completion_visible.get(index).copied()
  }

  fn completion_filter_fragment(&self) -> Option<String> {
    let cursor = self.primary_cursor_char_idx()?;
    let start = self.lsp_completion_start.unwrap_or(cursor).min(cursor);
    let text = self.active_editor_ref().document().text();
    Some(text.slice(start..cursor).to_string())
  }

  fn rebuild_completion_menu(&mut self) {
    let started = Instant::now();
    if self.lsp_completion_items.is_empty() {
      self.lsp_completion_visible.clear();
      self.completion_menu_mut().clear();
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

    self.lsp_completion_visible = visible.into_iter().map(|(index, _)| index).collect();
    if self.lsp_completion_visible.is_empty() {
      self.completion_menu_mut().clear();
      return;
    }

    let menu_items = self
      .lsp_completion_visible
      .iter()
      .filter_map(|index| self.lsp_completion_items.get(*index))
      .map(completion_menu_item_for_lsp_item)
      .collect();
    the_default::show_completion_menu(self, menu_items);
    let elapsed = started.elapsed();
    if ffi_ui_profile_should_log(elapsed) {
      ffi_ui_profile_log(format!(
        "rebuild_completion_menu elapsed={}ms total_items={} visible_items={}",
        elapsed.as_millis(),
        self.lsp_completion_items.len(),
        self.lsp_completion_visible.len()
      ));
    }
  }

  fn clear_completion_state(&mut self) {
    self.lsp_completion_items.clear();
    self.lsp_completion_raw_items.clear();
    self.lsp_completion_resolved.clear();
    self.lsp_completion_visible.clear();
    self.lsp_completion_start = None;
    if self.active_editor.is_some() {
      self.completion_menu_mut().clear();
    }
  }

  fn clear_signature_help_state(&mut self) {
    if self.active_editor.is_some() {
      self.active_state_mut().signature_help.clear();
    }
  }

  fn clear_hover_state(&mut self) {
    let Some(id) = self.active_editor else {
      return;
    };
    let Some(state) = self.states.get_mut(&id) else {
      return;
    };
    state.hover_docs = None;
    state.hover_docs_scroll = 0;
  }

  fn hover_docs_text(&self) -> Option<&str> {
    let id = self.active_editor?;
    self
      .states
      .get(&id)
      .and_then(|state| state.hover_docs.as_deref())
      .map(str::trim)
      .filter(|text| !text.is_empty())
  }

  fn build_lsp_hover_overlay(&self) -> Option<UiNode> {
    let docs = self.hover_docs_text()?;
    Some(UiNode::panel(
      "lsp_hover",
      LayoutIntent::Custom("lsp_hover".to_string()),
      UiNode::text("lsp_hover_text", docs),
    ))
  }

  fn append_lsp_hover_overlay(&self, tree: &mut the_lib::render::UiTree) {
    if let Some(node) = self.build_lsp_hover_overlay() {
      tree.overlays.push(node);
    }
  }

  fn dispatch_signature_help_request(
    &mut self,
    trigger: SignatureHelpTriggerSource,
    announce_failures: bool,
  ) -> bool {
    if !self.lsp_supports(LspCapability::SignatureHelp) {
      if announce_failures {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Warning,
          "signature help is not supported by the active server",
        );
      }
      return false;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      if announce_failures {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Warning,
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
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Warning,
          "completion is not supported by the active server",
        );
      }
      return false;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      if matches!(trigger, CompletionTriggerSource::Manual) {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Warning,
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
    if self.active_state_ref().mode != Mode::Insert || !self.lsp_supports(LspCapability::Completion)
    {
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
    if self.active_state_ref().mode != Mode::Insert
      || !self.lsp_supports(LspCapability::SignatureHelp)
    {
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
    if self.active_state_ref().mode != Mode::Insert {
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
              is_retrigger: self.active_state_ref().signature_help.active,
            },
            lsp_signature_help_trigger_char_latency(),
          );
        }
        if self.active_state_ref().signature_help.active {
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
        if self.active_state_ref().signature_help.active {
          return self.schedule_auto_signature_help(
            SignatureHelpTriggerSource::ContentChangeRetrigger,
            lsp_signature_help_retrigger_latency(),
          );
        }
        self.cancel_auto_signature_help();
        false
      },
      Command::CompletionAccept => {
        let trigger = if self.active_state_ref().signature_help.active {
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
      | Command::FindChar { .. }
      | Command::ParentNodeStart { .. }
      | Command::ParentNodeEnd { .. } => {
        let trigger = if self.active_state_ref().signature_help.active {
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
    if self.active_state_ref().mode != Mode::Insert {
      self.cancel_auto_completion();
      return false;
    }

    match command {
      Command::InsertChar(ch) => {
        if self.completion_menu().active {
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
        if self.completion_menu().active {
          self.rebuild_completion_menu();
        }
        if self.completion_menu().active || self.cursor_prev_char_is_word() {
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

  fn resolve_completion_item_if_needed(&mut self, index: usize) {
    if !self.completion_menu().active {
      return;
    }
    if self.lsp_completion_resolved.contains(&index) {
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
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Warning,
          format!("failed to dispatch completionItem/resolve: {err}"),
        );
      },
    }
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
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Warning,
          format!("failed to cancel stale request {id}: {err}"),
        );
      }
    }
  }

  fn dispatch_lsp_request(
    &mut self,
    method: &'static str,
    params: serde_json::Value,
    pending: PendingLspRequestKind,
  ) {
    self.cancel_pending_lsp_requests_for(&pending);
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

  fn poll_lsp_file_watch(&mut self) -> bool {
    let lsp_ready = self.lsp_ready;
    let (watched_uri, watched_path, pending_changes) = match poll_watch_events(
      self
        .lsp_watched_file
        .as_mut()
        .map(|watch| &mut watch.stream),
      Instant::now(),
      "ffi",
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
        if let Some(watch) = self.lsp_watched_file.as_mut() {
          clear_reload_state(&mut watch.stream.reload_state);
        }
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
        let current = self.active_editor_ref().document().text().clone();
        let buffer_modified = self.active_editor_ref().document().flags().modified;
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
                        "client=ffi path={} retry_in_ms={retry_in_ms}",
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
                        "client=ffi path={} err={} retry_in_ms={retry_in_ms}",
                        watched_path.display(),
                        error
                      ),
                    );
                    if self.active_editor.is_some() {
                      self.active_state_mut().messages.publish(
                        the_lib::messages::MessageLevel::Warning,
                        Some("watch".into()),
                        format!(
                          "failed to read '{label}' from disk: {error} (retrying in \
                           {retry_in_ms}ms)"
                        ),
                      );
                      self.request_render();
                    }
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
              format!("client=ffi path={}", watched_path.display()),
            );
            false
          },
          FileWatchReloadDecision::ConflictEntered => {
            trace_file_watch_event(
              "consumer_external_changed_dirty",
              format!("client=ffi path={}", watched_path.display()),
            );
            if self.active_editor.is_some() {
              self.active_state_mut().messages.publish(
                the_lib::messages::MessageLevel::Warning,
                Some("watch".into()),
                format!(
                  "file changed on disk: {label} (buffer has unsaved changes; run :rl to reload \
                   disk or :w! to overwrite disk)"
                ),
              );
              self.request_render();
            }
            true
          },
          FileWatchReloadDecision::ConflictOngoing => {
            trace_file_watch_event(
              "consumer_external_conflict_ongoing",
              format!("client=ffi path={}", watched_path.display()),
            );
            false
          },
          FileWatchReloadDecision::ReloadNeeded => {
            match <Self as DefaultContext>::reload_file_preserving_view(self, watched_path) {
              Ok(()) => {
                if let Some(watch) = self.lsp_watched_file.as_mut() {
                  mark_reload_applied(&mut watch.stream.reload_state);
                }
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
      },
    }
  }

  pub fn handle_key(&mut self, id: ffi::EditorId, event: ffi::KeyEvent) -> bool {
    let started = Instant::now();
    if self.activate(id).is_none() {
      return false;
    }

    let event_kind = event.kind;
    let event_codepoint = event.codepoint;
    if event_kind == 3 && self.hover_docs_text().is_some() && !self.completion_menu().active {
      self.clear_hover_state();
      self.request_render();
      return true;
    }
    if event_kind == 3
      && self.active_state_ref().signature_help.active
      && !self.completion_menu().active
    {
      the_default::close_signature_help(self);
      self.cancel_auto_signature_help();
      // Fall through so escape also transitions to normal mode.
    }
    let key_event = key_event_from_ffi(event);
    let dispatch = self.dispatch();
    dispatch.pre_on_keypress(self, key_event);
    self.ensure_cursor_visible(id);
    let elapsed = started.elapsed();
    if ffi_ui_profile_should_log(elapsed) {
      ffi_ui_profile_log(format!(
        "handle_key kind={} codepoint={} elapsed={}ms",
        event_kind,
        event_codepoint,
        elapsed.as_millis()
      ));
    }
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

  fn goto_buffer_impl(&mut self, direction: CommandDirection, count: usize) -> bool {
    let Some(current) = self.active_editor else {
      return false;
    };

    self.lsp_close_current_document();

    let switched_in_editor = {
      let Some(editor) = self.inner.editor_mut(current) else {
        return false;
      };
      match direction {
        CommandDirection::Forward => editor.switch_buffer_forward(count),
        CommandDirection::Backward => editor.switch_buffer_backward(count),
        _ => false,
      }
    };

    if switched_in_editor {
      let active_path = self
        .inner
        .editor(current)
        .and_then(|editor| editor.active_file_path().map(|path| path.to_path_buf()));
      <Self as DefaultContext>::set_file_path(self, active_path);
      self.request_render();
      return true;
    }

    let editor_ids = self
      .inner
      .editors()
      .map(|editor| editor.id())
      .collect::<Vec<_>>();
    if editor_ids.len() <= 1 {
      return false;
    }

    let Some(current_index) = editor_ids.iter().position(|id| *id == current) else {
      return false;
    };

    let len = editor_ids.len();
    let step = count.max(1) % len;
    let target_index = match direction {
      CommandDirection::Forward => (current_index + step) % len,
      CommandDirection::Backward => (current_index + len - step) % len,
      _ => return false,
    };
    let target = editor_ids[target_index];

    if !self.set_active_editor(target) {
      return false;
    }

    let _ = self.poll_editor_syntax_parse_results(target);
    self.active_state_mut().needs_render = true;
    true
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

  fn poll_active_syntax_parse_results(&mut self) -> bool {
    let Some(id) = self.active_editor else {
      return false;
    };
    self.poll_editor_syntax_parse_results(id)
  }

  fn refresh_editor_syntax(&mut self, id: LibEditorId) {
    let path = self
      .inner
      .editor(id)
      .and_then(|editor| editor.active_file_path().map(Path::to_path_buf));
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
    let path = self
      .inner
      .editor(id)
      .and_then(|editor| editor.active_file_path().map(Path::to_path_buf));
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
    self
      .inner
      .editor(id)
      .and_then(|editor| editor.active_file_path())
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
    let Some(editor_id) = self.active_editor else {
      return false;
    };

    let _ = self.poll_editor_syntax_parse_results(editor_id);

    let old_text_for_lsp = self.active_editor_ref().document().text().clone();
    let loader = self.loader.clone();
    let (changed, has_syntax) = {
      let Some(editor) = self.inner.editor_mut(editor_id) else {
        return false;
      };
      let doc = editor.document_mut();
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

    if let Some(editor) = self.inner.editor_mut(editor_id) {
      editor.mark_active_buffer_modified();
    }
    self.clear_hover_state();

    if let Some(state) = self.states.get_mut(&editor_id) {
      state.syntax_parse_lifecycle.cancel_pending();
      state.highlight_cache.clear();
      if has_syntax {
        state.syntax_parse_highlight_state.mark_parsed();
      } else {
        state.syntax_parse_highlight_state.mark_cleared();
      }
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
    if mode != Mode::Insert {
      self.cancel_auto_completion();
    }
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

  fn completion_menu(&self) -> &the_default::CompletionMenuState {
    &self.active_state_ref().completion_menu
  }

  fn completion_menu_mut(&mut self) -> &mut the_default::CompletionMenuState {
    &mut self.active_state_mut().completion_menu
  }

  fn signature_help(&self) -> Option<&the_default::SignatureHelpState> {
    Some(&self.active_state_ref().signature_help)
  }

  fn signature_help_mut(&mut self) -> Option<&mut the_default::SignatureHelpState> {
    Some(&mut self.active_state_mut().signature_help)
  }

  fn completion_selection_changed(&mut self, index: usize) {
    let source_index = self
      .completion_source_index_for_visible_index(index)
      .unwrap_or(index);
    self.resolve_completion_item_if_needed(source_index);
  }

  fn completion_accept_on_commit_char(&mut self, ch: char) -> bool {
    let Some(selected) = self.completion_menu().selected else {
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
    let preserve_completion = self.handle_completion_action(command);
    let _ = self.handle_signature_help_action(command);
    preserve_completion
  }

  fn completion_accept_selected(&mut self, index: usize) -> bool {
    let source_index = self
      .completion_source_index_for_visible_index(index)
      .unwrap_or(index);
    let Some(item) = self.lsp_completion_items.get(source_index).cloned() else {
      return false;
    };

    let fallback_end = self.primary_cursor_char_idx().unwrap_or(0);
    let fallback_start = self
      .lsp_completion_start
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

  fn set_word_jump_annotations(&mut self, inline: Vec<InlineAnnotation>, overlay: Vec<Overlay>) {
    let state = self.active_state_mut();
    state.word_jump_inline_annotations = inline;
    state.word_jump_overlay_annotations = overlay;
  }

  fn clear_word_jump_annotations(&mut self) {
    let state = self.active_state_mut();
    state.word_jump_inline_annotations.clear();
    state.word_jump_overlay_annotations.clear();
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
    if !state.word_jump_inline_annotations.is_empty() {
      let _ = annotations.add_inline_annotations(&state.word_jump_inline_annotations, None);
    }
    if !state.word_jump_overlay_annotations.is_empty() {
      let jump_label_style = self.ui_theme.find_highlight("ui.virtual.jump-label");
      let _ = annotations.add_overlay(&state.word_jump_overlay_annotations, jump_label_style);
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
      self.clear_hover_state();
      self.clear_signature_help_state();
      if let Some(editor) = self.inner.editor_mut(id) {
        editor.set_active_file_path(path);
      } else {
        return;
      }
      self.refresh_editor_syntax(id);
      self.refresh_lsp_runtime_for_active_file();
      self.refresh_vcs_diff_base_for_editor(id);
    }
  }

  fn goto_buffer(&mut self, direction: CommandDirection, count: usize) -> bool {
    self.goto_buffer_impl(direction, count)
  }

  fn goto_last_accessed_buffer(&mut self) -> bool {
    let Some(current) = self.active_editor else {
      return false;
    };

    self.lsp_close_current_document();
    let switched = {
      let Some(editor) = self.inner.editor_mut(current) else {
        return false;
      };
      editor.goto_last_accessed_buffer()
    };
    if !switched {
      return false;
    }

    let active_path = self
      .inner
      .editor(current)
      .and_then(|editor| editor.active_file_path().map(Path::to_path_buf));
    <Self as DefaultContext>::set_file_path(self, active_path);
    self.request_render();
    true
  }

  fn goto_last_modified_buffer(&mut self) -> bool {
    let Some(current) = self.active_editor else {
      return false;
    };

    self.lsp_close_current_document();
    let switched = {
      let Some(editor) = self.inner.editor_mut(current) else {
        return false;
      };
      editor.goto_last_modified_buffer()
    };
    if !switched {
      return false;
    }

    let active_path = self
      .inner
      .editor(current)
      .and_then(|editor| editor.active_file_path().map(Path::to_path_buf));
    <Self as DefaultContext>::set_file_path(self, active_path);
    self.request_render();
    true
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
    self.lsp_close_current_document();
    self.clear_hover_state();
    self.clear_signature_help_state();
    let reused = {
      let editor = self.active_editor_mut();
      if let Some(index) = editor.find_buffer_by_path(path) {
        let _ = editor.set_active_buffer(index);
        true
      } else {
        false
      }
    };

    if !reused {
      let content = std::fs::read_to_string(path)?;
      let viewport = self.active_editor_ref().view().viewport;
      {
        let editor = self.active_editor_mut();
        let view = ViewState::new(viewport, LibPosition::new(0, 0));
        let _ = editor.open_buffer(Rope::from_str(&content), view, Some(path.to_path_buf()));
        let doc = editor.document_mut();
        doc.set_display_name(
          path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string()),
        );
        let _ = doc.mark_saved();
      }
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

  fn lsp_completion(&mut self) {
    self.cancel_auto_completion();
    let _ = self.dispatch_completion_request(CompletionTriggerSource::Manual, true);
  }

  fn lsp_signature_help(&mut self) {
    self.cancel_auto_signature_help();
    let _ = self.dispatch_signature_help_request(SignatureHelpTriggerSource::Manual, true);
  }

  fn lsp_hover(&mut self) {
    if !self.lsp_supports(LspCapability::Hover) {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
        "hover is not supported by the active server",
      );
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
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

  fn on_file_saved(&mut self, _path: &Path, text: &str) {
    if let Some(watch) = self.lsp_watched_file.as_mut() {
      watch.stream.suppress_until = Some(Instant::now() + lsp_self_save_suppress_window());
      clear_reload_state(&mut watch.stream.reload_state);
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
    self.clear_hover_state();
    self.clear_completion_state();
    self.cancel_auto_completion();
    self.clear_signature_help_state();
    self.cancel_auto_signature_help();
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
    #[swift_bridge(associated_to = App)]
    fn completion_docs_render_json(
      markdown: &str,
      content_width: usize,
      language_hint: &str,
    ) -> String;
    #[swift_bridge(associated_to = App)]
    fn completion_popup_layout_json(
      area_width: usize,
      area_height: usize,
      cursor_x: i64,
      cursor_y: i64,
      list_width: usize,
      list_height: usize,
      docs_width: usize,
      docs_height: usize,
    ) -> String;
    #[swift_bridge(associated_to = App)]
    fn signature_help_popup_layout_json(
      area_width: usize,
      area_height: usize,
      cursor_x: i64,
      cursor_y: i64,
      panel_width: usize,
      panel_height: usize,
    ) -> String;
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
    fn file_picker_select_index(self: &mut App, id: EditorId, index: usize) -> bool;
    fn file_picker_snapshot(
      self: &mut App,
      id: EditorId,
      max_items: usize,
    ) -> FilePickerSnapshotData;
    fn file_picker_preview(self: &mut App, id: EditorId) -> PreviewData;
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
    fn inline_diagnostic_line_count(self: &RenderPlan) -> usize;
    fn inline_diagnostic_line_at(self: &RenderPlan, index: usize) -> RenderInlineDiagnosticLine;

    type RenderInlineDiagnosticLine;
    fn row(self: &RenderInlineDiagnosticLine) -> u16;
    fn col(self: &RenderInlineDiagnosticLine) -> u16;
    fn text(self: &RenderInlineDiagnosticLine) -> String;
    fn severity(self: &RenderInlineDiagnosticLine) -> u8;

    fn eol_diagnostic_count(self: &RenderPlan) -> usize;
    fn eol_diagnostic_at(self: &RenderPlan, index: usize) -> RenderEolDiagnostic;

    type RenderEolDiagnostic;
    fn row(self: &RenderEolDiagnostic) -> u16;
    fn col(self: &RenderEolDiagnostic) -> u16;
    fn message(self: &RenderEolDiagnostic) -> String;
    fn severity(self: &RenderEolDiagnostic) -> u8;

    fn diagnostic_underline_count(self: &RenderPlan) -> usize;
    fn diagnostic_underline_at(self: &RenderPlan, index: usize) -> RenderDiagnosticUnderline;

    type RenderDiagnosticUnderline;
    fn row(self: &RenderDiagnosticUnderline) -> u16;
    fn start_col(self: &RenderDiagnosticUnderline) -> u16;
    fn end_col(self: &RenderDiagnosticUnderline) -> u16;
    fn severity(self: &RenderDiagnosticUnderline) -> u8;
  }

  // File picker snapshot (direct FFI, no JSON)
  extern "Rust" {
    type FilePickerSnapshotData;
    fn active(self: &FilePickerSnapshotData) -> bool;
    fn query(self: &FilePickerSnapshotData) -> String;
    fn matched_count(self: &FilePickerSnapshotData) -> usize;
    fn total_count(self: &FilePickerSnapshotData) -> usize;
    fn scanning(self: &FilePickerSnapshotData) -> bool;
    fn root(self: &FilePickerSnapshotData) -> String;
    fn item_count(self: &FilePickerSnapshotData) -> usize;
    fn item_at(self: &FilePickerSnapshotData, index: usize) -> FilePickerItemFFI;
  }

  extern "Rust" {
    type FilePickerItemFFI;
    fn display(self: &FilePickerItemFFI) -> String;
    fn is_dir(self: &FilePickerItemFFI) -> bool;
    fn icon(self: &FilePickerItemFFI) -> String;
    fn match_index_count(self: &FilePickerItemFFI) -> usize;
    fn match_index_at(self: &FilePickerItemFFI, index: usize) -> u32;
  }

  // File picker preview (direct FFI, no JSON)
  extern "Rust" {
    type PreviewData;
    fn kind(self: &PreviewData) -> u8;
    fn path(self: &PreviewData) -> String;
    fn text(self: &PreviewData) -> String;
    fn loading(self: &PreviewData) -> bool;
    fn truncated(self: &PreviewData) -> bool;
    fn total_lines(self: &PreviewData) -> usize;
    fn show(self: &PreviewData) -> bool;
    fn line_count(self: &PreviewData) -> usize;
    fn line_at(self: &PreviewData, index: usize) -> PreviewLine;
  }

  extern "Rust" {
    type PreviewLine;
    fn text(self: &PreviewLine) -> String;
    fn span_count(self: &PreviewLine) -> usize;
    fn span_char_start(self: &PreviewLine, index: usize) -> u32;
    fn span_char_end(self: &PreviewLine, index: usize) -> u32;
    fn span_highlight(self: &PreviewLine, index: usize) -> u32;
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
    fs,
    path::{
      Path,
      PathBuf,
    },
    sync::{
      Mutex,
      OnceLock,
      mpsc::{
        Sender,
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

  use serde_json::json;
  use the_default::{
    Command,
    CommandEvent,
    CommandRegistry,
    DefaultContext,
    Direction as CommandDirection,
    Mode,
    PendingInput,
  };
  use the_lib::{
    messages::MessageEventKind,
    movement::Direction as SelectionDirection,
    position::{
      Position as LibPosition,
      char_idx_at_coords,
      coords_at_pos,
    },
    selection::{
      Range,
      Selection,
    },
    syntax::Highlight,
    transaction::Transaction,
  };
  use the_runtime::file_watch::{
    PathEvent,
    PathEventKind,
  };

  use super::{
    App,
    DiagnosticSeverity,
    InlineDiagnosticRenderLine,
    LibStyle,
    PendingAutoSignatureHelp,
    SignatureHelpTriggerSource,
    capabilities_support_single_char,
    dedupe_inline_diagnostic_lines,
    ffi,
  };

  #[test]
  fn app_render_plan_basic() {
    let _guard = ffi_test_guard();
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
  fn dedupe_inline_diagnostics_removes_exact_duplicates() {
    let mut lines = vec![
      InlineDiagnosticRenderLine {
        row:      6,
        col:      9,
        text:     "└".into(),
        severity: DiagnosticSeverity::Error,
      },
      InlineDiagnosticRenderLine {
        row:      6,
        col:      10,
        text:     "─".into(),
        severity: DiagnosticSeverity::Error,
      },
      InlineDiagnosticRenderLine {
        row:      6,
        col:      11,
        text:     "declared and not used: err".into(),
        severity: DiagnosticSeverity::Error,
      },
      InlineDiagnosticRenderLine {
        row:      6,
        col:      9,
        text:     "└".into(),
        severity: DiagnosticSeverity::Error,
      },
      InlineDiagnosticRenderLine {
        row:      6,
        col:      10,
        text:     "─".into(),
        severity: DiagnosticSeverity::Error,
      },
      InlineDiagnosticRenderLine {
        row:      6,
        col:      11,
        text:     "declared and not used: err".into(),
        severity: DiagnosticSeverity::Error,
      },
    ];

    dedupe_inline_diagnostic_lines(&mut lines);

    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].row, 6);
    assert_eq!(lines[0].col, 9);
    assert_eq!(lines[1].col, 10);
    assert_eq!(lines[2].col, 11);
    assert_eq!(lines[2].text.as_str(), "declared and not used: err");
  }

  #[test]
  fn app_insert_updates_text_and_plan() {
    let _guard = ffi_test_guard();
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
  fn command_palette_query_does_not_auto_select_without_explicit_navigation() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });

    let opened = app.handle_key(id, ffi::KeyEvent {
      kind:      0,
      codepoint: ':' as u32,
      modifiers: 0,
    });
    assert!(opened);
    assert!(app.command_palette_is_open(id));

    assert!(app.command_palette_set_query(id, "w"));
    assert!(app.command_palette_filtered_count(id) > 0);
    assert_eq!(app.command_palette_filtered_selected_index(id), -1);

    assert!(app.command_palette_select_filtered(id, 0));
    assert_eq!(app.command_palette_filtered_selected_index(id), 0);
  }

  #[test]
  fn command_open_creates_missing_file() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });

    let nonce = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .map(|d| d.as_nanos())
      .unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
      "the-editor-ffi-command-open-create-{}-{nonce}.txt",
      std::process::id()
    ));
    let _ = fs::remove_file(&path);
    assert!(!path.exists());

    assert!(app.handle_key(id, key_char(':')));
    for ch in format!("open {}", path.display()).chars() {
      assert!(app.handle_key(id, key_char(ch)));
    }
    assert!(app.handle_key(id, ffi::KeyEvent {
      kind:      1,
      codepoint: 0,
      modifiers: 0,
    }));

    assert!(path.exists());
    assert_eq!(app.active_editor_ref().document().text().to_string(), "");
    assert_eq!(DefaultContext::file_path(&app), Some(path.as_path()));

    let _ = fs::remove_file(&path);
  }

  #[test]
  fn theme_highlight_style_out_of_bounds_returns_default() {
    let _guard = ffi_test_guard();
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

  fn ffi_test_guard() -> std::sync::MutexGuard<'static, ()> {
    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_LOCK
      .get_or_init(|| Mutex::new(()))
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  struct TempTestFile {
    path: PathBuf,
  }

  impl TempTestFile {
    fn new(prefix: &str, content: &str) -> Self {
      let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
      let path = std::env::temp_dir().join(format!(
        "the-editor-ffi-{prefix}-{}-{nonce}.txt",
        std::process::id()
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

  fn default_viewport() -> ffi::Rect {
    ffi::Rect {
      x:      0,
      y:      0,
      width:  80,
      height: 24,
    }
  }

  fn key_char(ch: char) -> ffi::KeyEvent {
    ffi::KeyEvent {
      kind:      0,
      codepoint: ch as u32,
      modifiers: 0,
    }
  }

  fn install_test_watch_state(
    app: &mut App,
    id: ffi::EditorId,
    path: &Path,
  ) -> Sender<Vec<PathEvent>> {
    assert!(app.activate(id).is_some());
    let (events_tx, events_rx) = channel();
    let (_unused_rx, watch_handle) = super::watch_path(path, Duration::from_millis(0));
    let uri = the_lsp::text_sync::file_uri_for_path(path).expect("file uri");

    app.lsp_document = Some(super::LspDocumentSyncState {
      path:        path.to_path_buf(),
      uri:         uri.clone(),
      language_id: "text".into(),
      version:     0,
      opened:      false,
    });
    app.lsp_watched_file = Some(super::LspWatchedFileState {
      stream:        super::WatchedFileEventsState {
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

  #[test]
  fn syntax_highlight_updates_after_insert() {
    let _guard = ffi_test_guard();
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

  #[test]
  fn syntax_edits_use_synchronous_parse_and_keep_tree_aligned() {
    let _guard = ffi_test_guard();
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
    let id = app.create_editor(&"let value = 1;\n".repeat(64), viewport, scroll);
    assert!(app.activate(id).is_some());
    assert!(app.set_file_path(id, "main.rs"));

    let before = app.active_editor_ref().document().text().clone();
    let tx = Transaction::change(
      &before,
      std::iter::once((0, 0, Some("let inserted = 0;\n".into()))),
    )
    .expect("insert transaction");
    assert!(DefaultContext::apply_transaction(&mut app, &tx));

    let state = app.active_state_ref();
    assert!(
      state.syntax_parse_lifecycle.in_flight().is_none(),
      "editing should not leave a syntax parse job in-flight"
    );
    assert!(
      state.syntax_parse_lifecycle.queued().is_none(),
      "editing should not queue deferred syntax parse jobs"
    );
    assert!(
      !state.syntax_parse_highlight_state.is_interpolated(),
      "highlight state should remain parsed after synchronous syntax update"
    );

    let doc = app.active_editor_ref().document();
    let syntax = doc.syntax().expect("syntax should remain available");
    let root_end = syntax.tree().root_node().end_byte() as usize;
    assert_eq!(
      root_end,
      doc.text().len_bytes(),
      "syntax tree byte range should stay aligned after synchronous parse"
    );
  }

  #[test]
  fn interpolated_inflight_parse_ignores_stale_highlight_cache_spans() {
    let _guard = ffi_test_guard();
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
    assert!(app.activate(id).is_some());
    assert!(app.set_file_path(id, "main.rs"));

    let lib_id = id.to_lib().expect("editor id");
    let doc = app
      .inner
      .editor(lib_id)
      .expect("editor")
      .document()
      .text()
      .clone();
    let stale_highlight = Highlight::new(424_242);

    let current_doc_version = app
      .inner
      .editor(lib_id)
      .expect("editor")
      .document()
      .version();

    {
      let state = app.states.get_mut(&lib_id).expect("editor state");
      state.highlight_cache.update_range(
        0..doc.len_bytes(),
        vec![(stale_highlight, 0..3)],
        doc.slice(..),
        0,
        0,
      );
      state.syntax_parse_highlight_state.mark_interpolated();
      let _ = state
        .syntax_parse_lifecycle
        .queue(current_doc_version, Box::new(|| None));
    }

    let plan = app.render_plan(id);
    assert_ne!(
      first_highlight_id(&plan),
      Some(stale_highlight.get()),
      "stale highlight cache spans should not be rendered while interpolation is active"
    );
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
    let _guard = ffi_test_guard();
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
      app
        .inner
        .editor_mut(lib_id)
        .expect("editor")
        .set_active_file_path(Some(PathBuf::from(fixture_name)));
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
    let _guard = ffi_test_guard();
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
  fn goto_buffer_cycles_active_editor_in_both_directions() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id1 = app.create_editor("one", default_viewport(), ffi::Position { row: 0, col: 0 });
    let id2 = app.create_editor("two", default_viewport(), ffi::Position { row: 0, col: 0 });
    let id3 = app.create_editor("three", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });

    assert_eq!(app.active_editor, id1.to_lib());

    assert!(<App as DefaultContext>::goto_buffer(
      &mut app,
      CommandDirection::Forward,
      1,
    ));
    assert_eq!(app.active_editor, id2.to_lib());

    assert!(<App as DefaultContext>::goto_buffer(
      &mut app,
      CommandDirection::Forward,
      1,
    ));
    assert_eq!(app.active_editor, id3.to_lib());

    assert!(<App as DefaultContext>::goto_buffer(
      &mut app,
      CommandDirection::Forward,
      1,
    ));
    assert_eq!(app.active_editor, id1.to_lib());

    assert!(<App as DefaultContext>::goto_buffer(
      &mut app,
      CommandDirection::Backward,
      1,
    ));
    assert_eq!(app.active_editor, id3.to_lib());

    assert!(<App as DefaultContext>::goto_buffer(
      &mut app,
      CommandDirection::Forward,
      2,
    ));
    assert_eq!(app.active_editor, id2.to_lib());

    assert!(<App as DefaultContext>::goto_buffer(
      &mut app,
      CommandDirection::Backward,
      2,
    ));
    assert_eq!(app.active_editor, id3.to_lib());
  }

  #[test]
  fn goto_buffer_keymap_cycles_buffers_for_single_editor() {
    let _guard = ffi_test_guard();
    let first = TempTestFile::new("goto-buffer-first", "first file\n");
    let second = TempTestFile::new("goto-buffer-second", "second file\n");
    let mut app = App::new();
    let id = app.create_editor("first file\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    assert!(
      app.set_file_path(
        id,
        first
          .as_path()
          .to_str()
          .expect("temp test path should be utf-8")
      )
    );

    <App as DefaultContext>::open_file(&mut app, second.as_path()).expect("open second file");

    assert_eq!(app.text(id), "second file\n");
    assert_eq!(
      <App as DefaultContext>::file_path(&app),
      Some(second.as_path())
    );

    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char('n')));
    assert_eq!(app.text(id), "first file\n");
    assert_eq!(
      <App as DefaultContext>::file_path(&app),
      Some(first.as_path())
    );

    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char('p')));
    assert_eq!(app.text(id), "second file\n");
    assert_eq!(
      <App as DefaultContext>::file_path(&app),
      Some(second.as_path())
    );
  }

  #[test]
  fn goto_last_accessed_file_keymap_toggles_between_buffers() {
    let _guard = ffi_test_guard();
    let first = TempTestFile::new("goto-access-first", "first file\n");
    let second = TempTestFile::new("goto-access-second", "second file\n");
    let mut app = App::new();
    let id = app.create_editor("first file\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    assert!(
      app.set_file_path(
        id,
        first
          .as_path()
          .to_str()
          .expect("temp test path should be utf-8")
      )
    );
    <App as DefaultContext>::open_file(&mut app, second.as_path()).expect("open second file");

    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char('a')));
    assert_eq!(app.text(id), "first file\n");
    assert_eq!(
      <App as DefaultContext>::file_path(&app),
      Some(first.as_path())
    );

    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char('a')));
    assert_eq!(app.text(id), "second file\n");
    assert_eq!(
      <App as DefaultContext>::file_path(&app),
      Some(second.as_path())
    );
  }

  #[test]
  fn goto_last_modified_file_keymap_uses_recent_edit_order() {
    let _guard = ffi_test_guard();
    let first = TempTestFile::new("goto-modified-first", "first file\n");
    let second = TempTestFile::new("goto-modified-second", "second file\n");
    let mut app = App::new();
    let id = app.create_editor("first file\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    assert!(
      app.set_file_path(
        id,
        first
          .as_path()
          .to_str()
          .expect("temp test path should be utf-8")
      )
    );

    assert!(app.insert(id, "first-edit "));
    <App as DefaultContext>::open_file(&mut app, second.as_path()).expect("open second file");
    assert!(app.insert(id, "second-edit "));

    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char('m')));
    assert!(app.text(id).starts_with("first-edit "));
    assert_eq!(
      <App as DefaultContext>::file_path(&app),
      Some(first.as_path())
    );

    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char('m')));
    assert!(app.text(id).starts_with("second-edit "));
    assert_eq!(
      <App as DefaultContext>::file_path(&app),
      Some(second.as_path())
    );
  }

  #[test]
  fn goto_window_keymap_moves_cursor_to_window_alignments() {
    let _guard = ffi_test_guard();
    let mut content = String::new();
    for line in 0..96usize {
      content.push_str(&format!("line-{line}\n"));
    }

    let mut app = App::new();
    let id = app.create_editor(&content, default_viewport(), ffi::Position {
      row: 10,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    let initial_cursor = {
      let text = app.active_editor_ref().document().text().slice(..);
      char_idx_at_coords(text, LibPosition::new(20, 0))
    };
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::point(initial_cursor));

    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char('t')));
    let top_row = {
      let text = app.active_editor_ref().document().text().slice(..);
      let head = app.active_editor_ref().document().selection().ranges()[0].head;
      coords_at_pos(text, head).row
    };
    assert_eq!(top_row, 15);

    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char('c')));
    let center_row = {
      let text = app.active_editor_ref().document().text().slice(..);
      let head = app.active_editor_ref().document().selection().ranges()[0].head;
      coords_at_pos(text, head).row
    };
    assert_eq!(center_row, 21);

    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char('b')));
    let bottom_row = {
      let text = app.active_editor_ref().document().text().slice(..);
      let head = app.active_editor_ref().document().selection().ranges()[0].head;
      coords_at_pos(text, head).row
    };
    assert_eq!(bottom_row, 28);
  }

  #[test]
  fn goto_last_modification_keymap_moves_cursor_to_last_edit() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("first file\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());

    assert!(app.insert(id, "edited "));
    let _ = app.active_editor_mut().document_mut().commit();

    let end = app.active_editor_ref().document().text().len_chars();
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::point(end));
    let expected = app
      .active_editor_ref()
      .last_modification_position()
      .expect("last modification position");

    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char('.')));

    let actual = app.active_editor_ref().document().selection().ranges()[0].head;
    assert_eq!(actual, expected);
  }

  #[test]
  fn goto_word_keymap_moves_cursor_using_jump_labels() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor(
      "alpha bravo charlie delta\n",
      default_viewport(),
      ffi::Position { row: 0, col: 0 },
    );
    assert!(app.activate(id).is_some());
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::point(0));

    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char('w')));
    let targets = match app.active_state_ref().pending_input.clone() {
      Some(PendingInput::WordJump { targets, .. }) => targets,
      _ => panic!("expected word jump pending input"),
    };
    assert!(matches!(
      app.active_state_ref().pending_input.as_ref(),
      Some(PendingInput::WordJump {
        first: None,
        targets,
        ..
      }) if targets.len() >= 2
    ));
    assert!(
      app
        .active_state_ref()
        .word_jump_inline_annotations
        .is_empty()
    );
    assert!(
      !app
        .active_state_ref()
        .word_jump_overlay_annotations
        .is_empty()
    );

    assert!(app.handle_key(id, key_char('a')));
    assert!(matches!(
      app.active_state_ref().pending_input.as_ref(),
      Some(PendingInput::WordJump {
        first: Some(0),
        targets,
        ..
      }) if targets.len() >= 2
    ));
    assert!(
      app
        .active_state_ref()
        .word_jump_inline_annotations
        .is_empty()
    );
    assert!(
      !app
        .active_state_ref()
        .word_jump_overlay_annotations
        .is_empty()
    );

    assert!(app.handle_key(id, key_char('b')));
    assert!(app.active_state_ref().pending_input.is_none());
    assert!(
      app
        .active_state_ref()
        .word_jump_inline_annotations
        .is_empty()
    );
    assert!(
      app
        .active_state_ref()
        .word_jump_overlay_annotations
        .is_empty()
    );
    let expected = targets
      .get(1)
      .expect("expected at least two jump targets")
      .range
      .with_direction(SelectionDirection::Forward);
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      expected
    );
  }

  #[test]
  fn extend_to_word_keymap_extends_selection_using_jump_labels() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor(
      "alpha bravo charlie delta\n",
      default_viewport(),
      ffi::Position { row: 0, col: 0 },
    );
    assert!(app.activate(id).is_some());
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::point(0));
    app.active_state_mut().mode = Mode::Select;

    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char('w')));
    let targets = match app.active_state_ref().pending_input.clone() {
      Some(PendingInput::WordJump { targets, .. }) => targets,
      _ => panic!("expected word jump pending input"),
    };
    assert!(app.handle_key(id, key_char('a')));
    assert!(app.handle_key(id, key_char('b')));

    let target = targets
      .get(1)
      .expect("expected at least two jump targets")
      .range;
    let expected = if target.anchor < target.head {
      Range::new(0, target.head)
    } else {
      Range::new(target.anchor.max(0), target.head)
    };
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      expected
    );
  }

  #[test]
  fn ensure_cursor_visible_keeps_horizontal_scroll_zero_with_soft_wrap() {
    let _guard = ffi_test_guard();
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

  #[test]
  fn watch_reload_preserves_cursor_and_scroll_semantically_after_external_edit() {
    let _guard = ffi_test_guard();
    let fixture = TempTestFile::new("semantic-reload", "zero\none\ntwo\nthree\n");
    let mut app = App::new();
    let id = app.create_editor(
      &fs::read_to_string(fixture.as_path()).expect("read fixture"),
      default_viewport(),
      ffi::Position { row: 0, col: 0 },
    );
    assert!(app.activate(id).is_some());
    assert!(
      app.set_file_path(
        id,
        fixture
          .as_path()
          .to_str()
          .expect("temp test path should be utf-8")
      )
    );

    let cursor = {
      let text = app.active_editor_ref().document().text().slice(..);
      char_idx_at_coords(text, LibPosition::new(2, 1))
    };
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::point(cursor));
    app.active_editor_mut().view_mut().scroll = LibPosition::new(2, 7);

    let before_cursor_coords = {
      let text = app.active_editor_ref().document().text().slice(..);
      let head = app.active_editor_ref().document().selection().ranges()[0].head;
      coords_at_pos(text, head)
    };
    let before_scroll = app.active_editor_ref().view().scroll;

    let watch_tx = install_test_watch_state(&mut app, id, fixture.as_path());
    fs::write(fixture.as_path(), "inserted\nzero\none\ntwo\nthree\n").expect("update fixture");
    watch_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send watch event");

    assert!(app.poll_lsp_file_watch());
    let after_cursor_coords = {
      let text = app.active_editor_ref().document().text().slice(..);
      let head = app.active_editor_ref().document().selection().ranges()[0].head;
      coords_at_pos(text, head)
    };
    assert_eq!(app.text(id), "inserted\nzero\none\ntwo\nthree\n");
    assert_eq!(after_cursor_coords, before_cursor_coords);
    assert_eq!(app.active_editor_ref().view().scroll, before_scroll);
  }

  #[test]
  fn watch_dirty_buffer_external_change_keeps_buffer_and_warns() {
    let _guard = ffi_test_guard();
    let fixture = TempTestFile::new("dirty-watch", "alpha\nbeta\n");
    let mut app = App::new();
    let id = app.create_editor(
      &fs::read_to_string(fixture.as_path()).expect("read fixture"),
      default_viewport(),
      ffi::Position { row: 0, col: 0 },
    );
    assert!(app.activate(id).is_some());
    assert!(
      app.set_file_path(
        id,
        fixture
          .as_path()
          .to_str()
          .expect("temp test path should be utf-8")
      )
    );

    let local_edit = Transaction::change(
      app.active_editor_ref().document().text(),
      std::iter::once((0, 0, Some("local-".into()))),
    )
    .expect("local edit");
    assert!(DefaultContext::apply_transaction(&mut app, &local_edit));
    let dirty_snapshot = app.text(id);
    assert!(app.active_editor_ref().document().flags().modified);

    let watch_tx = install_test_watch_state(&mut app, id, fixture.as_path());
    fs::write(fixture.as_path(), "disk-alpha\ndisk-beta\n").expect("update fixture");
    watch_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send watch event");

    let before_seq = app.active_state_ref().messages.latest_seq();
    assert!(app.poll_lsp_file_watch());
    assert_eq!(app.text(id), dirty_snapshot);

    let events = app.active_state_ref().messages.events_since(before_seq);
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
      <App as DefaultContext>::watch_statusline_text(&app).as_deref(),
      Some("watch: conflict")
    );
  }

  #[test]
  fn watch_conflict_write_requires_force_and_w_bang_overwrites_disk() {
    let _guard = ffi_test_guard();
    let fixture = TempTestFile::new("conflict-write-force", "alpha\nbeta\n");
    let mut app = App::new();
    let id = app.create_editor(
      &fs::read_to_string(fixture.as_path()).expect("read fixture"),
      default_viewport(),
      ffi::Position { row: 0, col: 0 },
    );
    assert!(app.activate(id).is_some());
    assert!(
      app.set_file_path(
        id,
        fixture
          .as_path()
          .to_str()
          .expect("temp test path should be utf-8")
      )
    );

    let local_edit = Transaction::change(
      app.active_editor_ref().document().text(),
      std::iter::once((0, 0, Some("local-".into()))),
    )
    .expect("local edit");
    assert!(DefaultContext::apply_transaction(&mut app, &local_edit));
    let local_snapshot = app.text(id);

    let watch_tx = install_test_watch_state(&mut app, id, fixture.as_path());
    fs::write(fixture.as_path(), "disk-alpha\ndisk-beta\n").expect("update fixture");
    watch_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send watch event");
    assert!(app.poll_lsp_file_watch());
    assert!(<App as DefaultContext>::watch_conflict_active(&app));

    let registry = app.command_registry_ref() as *const CommandRegistry<App>;
    let write_err = unsafe { (&*registry).execute(&mut app, "write", "", CommandEvent::Validate) }
      .expect_err("write should fail with conflict");
    assert!(write_err.to_string().contains(":w!"));

    unsafe { (&*registry).execute(&mut app, "w!", "", CommandEvent::Validate) }
      .expect("force write");
    assert_eq!(
      fs::read_to_string(fixture.as_path()).expect("read disk"),
      local_snapshot
    );
    assert!(!<App as DefaultContext>::watch_conflict_active(&app));
  }

  #[test]
  fn watch_conflict_rl_and_rla_aliases_reload_and_clear_conflict() {
    let _guard = ffi_test_guard();
    let fixture = TempTestFile::new("conflict-reload-alias", "alpha\nbeta\n");
    let mut app = App::new();
    let id = app.create_editor(
      &fs::read_to_string(fixture.as_path()).expect("read fixture"),
      default_viewport(),
      ffi::Position { row: 0, col: 0 },
    );
    assert!(app.activate(id).is_some());
    assert!(
      app.set_file_path(
        id,
        fixture
          .as_path()
          .to_str()
          .expect("temp test path should be utf-8")
      )
    );

    let local_edit = Transaction::change(
      app.active_editor_ref().document().text(),
      std::iter::once((0, 0, Some("local-".into()))),
    )
    .expect("local edit");
    assert!(DefaultContext::apply_transaction(&mut app, &local_edit));

    let watch_tx = install_test_watch_state(&mut app, id, fixture.as_path());
    fs::write(fixture.as_path(), "disk-alpha\ndisk-beta\n").expect("update fixture");
    watch_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send watch event");
    assert!(app.poll_lsp_file_watch());
    assert!(<App as DefaultContext>::watch_conflict_active(&app));

    let registry = app.command_registry_ref() as *const CommandRegistry<App>;
    unsafe { (&*registry).execute(&mut app, "rl", "", CommandEvent::Validate) }
      .expect("reload alias");
    assert_eq!(app.text(id), "disk-alpha\ndisk-beta\n");
    assert!(!<App as DefaultContext>::watch_conflict_active(&app));

    let local_edit_again = Transaction::change(
      app.active_editor_ref().document().text(),
      std::iter::once((0, 0, Some("local-".into()))),
    )
    .expect("local edit");
    assert!(DefaultContext::apply_transaction(
      &mut app,
      &local_edit_again
    ));
    fs::write(fixture.as_path(), "disk-gamma\ndisk-delta\n").expect("update fixture");
    watch_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send watch event");
    assert!(app.poll_lsp_file_watch());
    assert!(<App as DefaultContext>::watch_conflict_active(&app));

    unsafe { (&*registry).execute(&mut app, "rla", "", CommandEvent::Validate) }
      .expect("reload-all alias");
    assert_eq!(app.text(id), "disk-gamma\ndisk-delta\n");
    assert!(!<App as DefaultContext>::watch_conflict_active(&app));
  }

  #[test]
  fn watch_rapid_external_changes_reload_to_latest_on_disk_content() {
    let _guard = ffi_test_guard();
    let fixture = TempTestFile::new("rapid-watch", "first\n");
    let mut app = App::new();
    let id = app.create_editor("first\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());

    let watch_tx = install_test_watch_state(&mut app, id, fixture.as_path());
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

    assert!(app.poll_lsp_file_watch());
    assert_eq!(app.text(id), "third\n");
  }

  #[test]
  fn watch_self_save_suppression_window_ignores_events_until_expiry() {
    let _guard = ffi_test_guard();
    let fixture = TempTestFile::new("suppression-watch", "one\n");
    let mut app = App::new();
    let id = app.create_editor("one\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());

    let watch_tx = install_test_watch_state(&mut app, id, fixture.as_path());
    let before = app.text(id);
    if let Some(watch) = app.lsp_watched_file.as_mut() {
      watch.stream.suppress_until = Some(Instant::now() + Duration::from_secs(2));
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

    assert!(!app.poll_lsp_file_watch());
    assert_eq!(app.text(id), before);
  }

  #[test]
  fn watch_disconnect_rebinds_and_keeps_processing_changes() {
    let _guard = ffi_test_guard();
    let fixture = TempTestFile::new("disconnect-watch", "one\n");
    let mut app = App::new();
    let id = app.create_editor("one\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());

    let watch_tx = install_test_watch_state(&mut app, id, fixture.as_path());
    drop(watch_tx);

    assert!(!app.poll_lsp_file_watch());
    let rebound_watch_path = app
      .lsp_watched_file
      .as_ref()
      .map(|watch| watch.stream.path.clone())
      .expect("watch should be rebound");
    assert_eq!(rebound_watch_path, fixture.as_path());

    let rebound_tx = install_test_watch_state(&mut app, id, fixture.as_path());
    fs::write(fixture.as_path(), "two\n").expect("update fixture");
    rebound_tx
      .send(vec![PathEvent {
        path: fixture.as_path().to_path_buf(),
        kind: PathEventKind::Changed,
      }])
      .expect("send rebound event");

    assert!(app.poll_lsp_file_watch());
    assert_eq!(app.text(id), "two\n");
  }

  #[test]
  fn watch_missing_file_then_create_triggers_reload() {
    let _guard = ffi_test_guard();
    let nonce = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .map(|d| d.as_nanos())
      .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
      "the-editor-ffi-watch-missing-{}-{nonce}",
      std::process::id()
    ));
    fs::create_dir_all(&root).expect("create temp root");
    let missing_path = root.join("created-later.txt");
    let _ = fs::remove_file(&missing_path);

    let mut app = App::new();
    let id = app.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });
    let watch_tx = install_test_watch_state(&mut app, id, &missing_path);

    fs::write(&missing_path, "created\n").expect("create watched file");
    watch_tx
      .send(vec![PathEvent {
        path: missing_path.clone(),
        kind: PathEventKind::Created,
      }])
      .expect("send created event");

    assert!(app.poll_lsp_file_watch());
    assert_eq!(app.text(id), "created\n");
    let _ = fs::remove_file(&missing_path);
    let _ = fs::remove_dir_all(&root);
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
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });
    assert!(app.activate(id).is_some());
    app.active_state_mut().mode = Mode::Normal;
    app.lsp_pending_auto_signature_help = Some(PendingAutoSignatureHelp {
      due_at:  Instant::now() - Duration::from_millis(1),
      trigger: SignatureHelpTriggerSource::Manual,
    });

    assert!(!app.poll_lsp_signature_help_auto_trigger());
    assert!(app.lsp_pending_auto_signature_help.is_none());
  }

  #[test]
  fn signature_help_action_closes_state_on_non_edit_commands() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });
    assert!(app.activate(id).is_some());
    app.active_state_mut().mode = Mode::Insert;
    app.active_state_mut().signature_help.active = true;
    app.lsp_pending_auto_signature_help = Some(PendingAutoSignatureHelp {
      due_at:  Instant::now() + Duration::from_millis(50),
      trigger: SignatureHelpTriggerSource::ContentChangeRetrigger,
    });

    assert!(!app.handle_signature_help_action(Command::Search));
    assert!(!app.active_state_ref().signature_help.active);
    assert!(app.lsp_pending_auto_signature_help.is_none());
  }
}
