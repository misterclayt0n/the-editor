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
  hash::{
    DefaultHasher,
    Hash,
    Hasher,
  },
  num::{
    NonZeroU64,
    NonZeroUsize,
  },
  path::{
    Component,
    Path,
    PathBuf,
  },
  sync::{
    Arc,
    OnceLock,
    RwLock,
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
use smallvec::SmallVec;
use the_config::{
  build_dispatch as config_build_dispatch,
  build_keymaps as config_build_keymaps,
};
use the_default::{
  BufferTabItemSnapshot as DefaultBufferTabItemSnapshot,
  BufferTabsSnapshot as DefaultBufferTabsSnapshot,
  Command,
  CommandEvent,
  CommandPaletteAction,
  CommandPaletteLayout,
  CommandPaletteSource,
  CommandPaletteState,
  CommandPaletteStyle,
  CommandPaletteTheme,
  CommandPromptState,
  CommandRegistry,
  DefaultContext,
  DefaultDispatchStatic,
  Direction as CommandDirection,
  DispatchRef,
  FilePickerChangedFileItem,
  FilePickerChangedKind,
  FilePickerDiagnosticItem,
  FilePickerItem,
  FilePickerItemAction,
  FilePickerKind,
  FilePickerPreview,
  FilePickerPreviewLineKind,
  FilePickerPreviewWindow,
  FilePickerRowData,
  FilePickerRowKind,
  FilePickerState,
  FileTreeMode as DefaultFileTreeMode,
  FileTreeNodeKind as DefaultFileTreeNodeKind,
  FileTreeSnapshot as DefaultFileTreeSnapshot,
  FileTreeState,
  GlobalSearchConfig,
  GlobalSearchState,
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
  ThemeCatalog,
  buffer_tabs_snapshot,
  close_file_picker,
  command_palette_filtered_indices,
  command_palette_selected_filtered_index,
  completion_docs_panel_rect as default_completion_docs_panel_rect,
  completion_panel_rect as default_completion_panel_rect,
  file_picker_kind_from_title,
  file_picker_preview_window,
  file_picker_row_data,
  finalize_keep_selections,
  finalize_remove_selections,
  finalize_rename_symbol,
  finalize_search,
  finalize_select_regex,
  finalize_shell_append_output,
  finalize_shell_insert_output,
  finalize_shell_keep_pipe,
  finalize_shell_pipe,
  finalize_shell_pipe_to,
  finalize_split_selection,
  open_dynamic_picker,
  poll_scan_results as file_picker_poll_scan_results,
  refresh_matcher_state as file_picker_refresh_matcher_state,
  replace_file_picker_items,
  select_file_picker_index,
  set_file_picker_query_text,
  set_file_picker_syntax_loader,
  signature_help_panel_rect as default_signature_help_panel_rect,
  submit_file_picker,
  update_action_palette_for_input,
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
  editor::{
    EditorId as LibEditorId,
    EditorSurfaceSnapshot as LibEditorSurfaceSnapshot,
    PaneContent,
    PaneContentKind,
    TerminalSurfaceSnapshot as LibTerminalSurfaceSnapshot,
  },
  messages::MessageCenter,
  movement::{
    self,
    Direction,
    Movement,
  },
  position::Position as LibPosition,
  registers::Registers,
  render::{
    FrameGenerationState,
    GutterConfig,
    InlineDiagnostic,
    InlineDiagnosticFilter,
    InlineDiagnosticRenderLine,
    InlineDiagnosticsConfig,
    LayoutIntent,
    NoHighlights,
    OverlayNode,
    OverlayRectKind,
    OverlayText,
    RenderGenerationState,
    RenderDiagnosticGutterStyles,
    RenderLayerRowHashes,
    RenderDiffGutterStyles,
    RenderGutterDiffKind,
    RenderStyles,
    SelectionMatchHighlightOptions,
    SyntaxHighlightAdapter,
    UiAlign,
    UiAlignPair,
    UiConstraints,
    UiContainer,
    UiInsets,
    UiLayer,
    UiNode,
    UiPanel,
    UiState,
    UiText,
    add_selection_match_highlights,
    apply_row_insertions,
    apply_diagnostic_gutter_markers,
    apply_diff_gutter_markers,
    base_render_layer_row_hashes,
    build_plan,
    char_at_visual_pos,
    finish_frame_generations,
    finish_render_generations,
    graphics::{
      Color as LibColor,
      CursorKind as LibCursorKind,
      Modifier as LibModifier,
      Rect as LibRect,
      Style as LibStyle,
      UnderlineStyle as LibUnderlineStyle,
    },
    gutter_width_for_document,
    render_inline_diagnostics_for_viewport,
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
    CursorId,
    CursorPick,
    Range,
    Selection,
  },
  split_tree::{
    PaneDirection,
    PaneId,
    SplitAxis,
    SplitNodeId,
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
  LspCodeAction,
  LspCompletionContext,
  LspCompletionItem,
  LspCompletionItemKind,
  LspEvent,
  LspExecuteCommand,
  LspHoverDetails,
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
  code_action_params,
  completion_params,
  document_highlight_params,
  document_symbols_params,
  execute_command_params,
  goto_declaration_params,
  goto_definition_params,
  goto_implementation_params,
  goto_type_definition_params,
  hover_params,
  jsonrpc,
  parse_code_actions_response,
  parse_completion_item_response,
  parse_completion_response_with_raw,
  parse_document_highlights_response,
  parse_document_symbols_response,
  parse_hover_details_response,
  parse_locations_response,
  parse_signature_help_response,
  parse_workspace_edit_response,
  parse_workspace_symbols_response,
  rename_params,
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
  workspace_symbols_params,
};
use the_runtime::{
  clipboard::ClipboardProvider as RuntimeClipboardProvider,
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

  /// Get the first cursor position (character index).
  pub fn first_cursor(&self) -> usize {
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

  /// Add a cursor on the line above the first cursor.
  pub fn add_cursor_above(&mut self) -> bool {
    self.add_cursor_vertical(Direction::Backward)
  }

  /// Add a cursor on the line below the first cursor.
  pub fn add_cursor_below(&mut self) -> bool {
    self.add_cursor_vertical(Direction::Forward)
  }

  fn add_cursor_vertical(&mut self, dir: Direction) -> bool {
    let text_fmt = TextFormat::default();
    add_cursor_vertical(&mut self.inner, dir, CursorPick::First, &text_fmt)
  }

  /// Remove all cursors except the first.
  pub fn collapse_to_first(&mut self) {
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
        kind:  the_lib::render::RenderSelectionKind::Primary,
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

  fn kind(&self) -> u8 {
    match self.inner.kind {
      the_lib::render::RenderSelectionKind::Primary => 1,
      the_lib::render::RenderSelectionKind::Match => 2,
      the_lib::render::RenderSelectionKind::Hover => 3,
    }
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

  fn layout_generation(&self) -> u64 {
    self.inner.layout_generation
  }

  fn text_generation(&self) -> u64 {
    self.inner.text_generation
  }

  fn decoration_generation(&self) -> u64 {
    self.inner.decoration_generation
  }

  fn cursor_generation(&self) -> u64 {
    self.inner.cursor_generation
  }

  fn scroll_generation(&self) -> u64 {
    self.inner.scroll_generation
  }

  fn theme_generation(&self) -> u64 {
    self.inner.theme_generation
  }

  fn damage_start_row(&self) -> u16 {
    self.inner.damage_start_row
  }

  fn damage_end_row(&self) -> u16 {
    self.inner.damage_end_row
  }

  fn damage_is_full(&self) -> bool {
    self.inner.damage_is_full
  }

  fn damage_reason(&self) -> u8 {
    self.inner.damage_reason.code()
  }

  fn cursor_blink_enabled(&self) -> bool {
    self.inner.cursor_blink_enabled
  }

  fn cursor_blink_interval_ms(&self) -> u16 {
    self.inner.cursor_blink_interval_ms
  }

  fn cursor_blink_delay_ms(&self) -> u16 {
    self.inner.cursor_blink_delay_ms
  }

  fn cursor_blink_generation(&self) -> u64 {
    self.inner.cursor_blink_generation
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

#[derive(Debug, Clone)]
pub struct RenderFramePane {
  pane_id:     u64,
  rect:        LibRect,
  is_active:   bool,
  pane_kind:   u8,
  terminal_id: u64,
  plan:        RenderPlan,
}

impl RenderFramePane {
  fn empty() -> Self {
    Self {
      pane_id:     0,
      rect:        LibRect::new(0, 0, 0, 0),
      is_active:   false,
      pane_kind:   0,
      terminal_id: 0,
      plan:        RenderPlan::empty(),
    }
  }

  fn pane_id(&self) -> u64 {
    self.pane_id
  }

  fn rect(&self) -> ffi::Rect {
    self.rect.into()
  }

  fn is_active(&self) -> bool {
    self.is_active
  }

  fn pane_kind(&self) -> u8 {
    self.pane_kind
  }

  fn terminal_id(&self) -> u64 {
    self.terminal_id
  }

  fn plan(&self) -> RenderPlan {
    self.plan.clone()
  }
}

#[derive(Debug, Clone)]
pub struct RenderFramePlan {
  active_pane_id:             u64,
  panes:                      Vec<RenderFramePane>,
  frame_generation:           u64,
  pane_structure_generation:  u64,
  changed_pane_ids:           Vec<u64>,
  damage_is_full:             bool,
  damage_reason:              u8,
}

impl RenderFramePlan {
  fn empty() -> Self {
    Self {
      active_pane_id: 0,
      panes:          Vec::new(),
      frame_generation: 0,
      pane_structure_generation: 0,
      changed_pane_ids: Vec::new(),
      damage_is_full: false,
      damage_reason: 0,
    }
  }

  fn from_lib(
    frame: the_lib::render::FrameRenderPlan,
    inline_diagnostic_lines: Vec<InlineDiagnosticRenderLine>,
    eol_diagnostics: Vec<EolDiagnosticEntry>,
    diagnostic_underlines: Vec<DiagnosticUnderlineEntry>,
  ) -> Self {
    let active = frame.active_pane;
    let active_pane_id = active.get().get() as u64;
    let frame_generation = frame.frame_generation;
    let pane_structure_generation = frame.pane_structure_generation;
    let changed_pane_ids = frame
      .changed_pane_ids
      .iter()
      .map(|pane_id| pane_id.get().get() as u64)
      .collect::<Vec<_>>();
    let damage_is_full = frame.damage_is_full;
    let damage_reason = frame.damage_reason.code();
    let panes = frame
      .panes
      .into_iter()
      .map(|pane| {
        let is_active = pane.pane_id == active;
        let mut wrapped_plan: RenderPlan = pane.plan.into();
        if is_active {
          wrapped_plan.inline_diagnostic_lines = inline_diagnostic_lines.clone();
          wrapped_plan.eol_diagnostics = eol_diagnostics.clone();
          wrapped_plan.diagnostic_underlines = diagnostic_underlines.clone();
        }
        RenderFramePane {
          pane_id: pane.pane_id.get().get() as u64,
          rect: pane.rect,
          is_active,
          pane_kind: pane_content_kind_to_u8(pane.pane_kind),
          terminal_id: pane.terminal_id.map_or(0, |id| id.get().get() as u64),
          plan: wrapped_plan,
        }
      })
      .collect();
    Self {
      active_pane_id,
      panes,
      frame_generation,
      pane_structure_generation,
      changed_pane_ids,
      damage_is_full,
      damage_reason,
    }
  }

  fn active_pane_id(&self) -> u64 {
    self.active_pane_id
  }

  fn frame_generation(&self) -> u64 {
    self.frame_generation
  }

  fn pane_count(&self) -> usize {
    self.panes.len()
  }

  fn pane_at(&self, index: usize) -> RenderFramePane {
    self
      .panes
      .get(index)
      .cloned()
      .unwrap_or_else(RenderFramePane::empty)
  }

  fn active_plan(&self) -> RenderPlan {
    self
      .panes
      .iter()
      .find(|pane| pane.is_active)
      .map(|pane| pane.plan.clone())
      .unwrap_or_else(RenderPlan::empty)
  }

  fn pane_structure_generation(&self) -> u64 {
    self.pane_structure_generation
  }

  fn changed_pane_count(&self) -> usize {
    self.changed_pane_ids.len()
  }

  fn changed_pane_id_at(&self, index: usize) -> u64 {
    self.changed_pane_ids.get(index).copied().unwrap_or(0)
  }

  fn damage_is_full(&self) -> bool {
    self.damage_is_full
  }

  fn damage_reason(&self) -> u8 {
    self.damage_reason
  }
}

#[derive(Debug, Clone)]
pub struct TerminalSurfaceSnapshot {
  terminal_id: u64,
  pane_id:     u64,
  is_active:   bool,
}

impl TerminalSurfaceSnapshot {
  fn empty() -> Self {
    Self {
      terminal_id: 0,
      pane_id:     0,
      is_active:   false,
    }
  }

  fn from_lib(snapshot: LibTerminalSurfaceSnapshot) -> Self {
    Self {
      terminal_id: snapshot.terminal_id.get().get() as u64,
      pane_id:     snapshot
        .attached_pane
        .map_or(0, |pane| pane.get().get() as u64),
      is_active:   snapshot.is_active,
    }
  }

  fn terminal_id(&self) -> u64 {
    self.terminal_id
  }

  fn pane_id(&self) -> u64 {
    self.pane_id
  }

  fn is_active(&self) -> bool {
    self.is_active
  }
}

#[derive(Debug, Clone)]
pub struct EditorSurfaceSnapshot {
  pane_id:      u64,
  buffer_id:    u64,
  buffer_index: usize,
  title:        String,
  modified:     bool,
  file_path:    String,
  is_active:    bool,
}

impl EditorSurfaceSnapshot {
  fn empty() -> Self {
    Self {
      pane_id:      0,
      buffer_id:    0,
      buffer_index: 0,
      title:        String::new(),
      modified:     false,
      file_path:    String::new(),
      is_active:    false,
    }
  }

  fn from_lib(snapshot: LibEditorSurfaceSnapshot) -> Self {
    Self {
      pane_id:      snapshot.pane_id.get().get() as u64,
      buffer_id:    snapshot.buffer_id,
      buffer_index: snapshot.buffer_index,
      title:        snapshot.display_name,
      modified:     snapshot.modified,
      file_path:    snapshot
        .file_path
        .map(|path| path.display().to_string())
        .unwrap_or_default(),
      is_active:    snapshot.is_active,
    }
  }

  fn pane_id(&self) -> u64 {
    self.pane_id
  }

  fn buffer_id(&self) -> u64 {
    self.buffer_id
  }

  fn buffer_index(&self) -> usize {
    self.buffer_index
  }

  fn title(&self) -> String {
    self.title.clone()
  }

  fn modified(&self) -> bool {
    self.modified
  }

  fn file_path(&self) -> String {
    self.file_path.clone()
  }

  fn is_active(&self) -> bool {
    self.is_active
  }
}

#[derive(Debug, Clone)]
pub struct SplitSeparator {
  split_id:   u64,
  axis:       u8,
  line:       u16,
  span_start: u16,
  span_end:   u16,
}

impl SplitSeparator {
  fn empty() -> Self {
    Self {
      split_id:   0,
      axis:       0,
      line:       0,
      span_start: 0,
      span_end:   0,
    }
  }

  fn split_id(&self) -> u64 {
    self.split_id
  }

  fn axis(&self) -> u8 {
    self.axis
  }

  fn line(&self) -> u16 {
    self.line
  }

  fn span_start(&self) -> u16 {
    self.span_start
  }

  fn span_end(&self) -> u16 {
    self.span_end
  }
}

fn split_axis_to_u8(axis: SplitAxis) -> u8 {
  match axis {
    SplitAxis::Vertical => 0,
    SplitAxis::Horizontal => 1,
  }
}

fn split_axis_from_u8(axis: u8) -> Option<SplitAxis> {
  match axis {
    0 => Some(SplitAxis::Vertical),
    1 => Some(SplitAxis::Horizontal),
    _ => None,
  }
}

fn pane_direction_from_u8(direction: u8) -> Option<PaneDirection> {
  match direction {
    0 => Some(PaneDirection::Left),
    1 => Some(PaneDirection::Right),
    2 => Some(PaneDirection::Up),
    3 => Some(PaneDirection::Down),
    _ => None,
  }
}

fn pane_id_from_u64(pane: u64) -> Option<PaneId> {
  let raw = usize::try_from(pane).ok()?;
  let raw = NonZeroUsize::new(raw)?;
  Some(PaneId::from(raw))
}

fn pane_content_kind_to_u8(kind: PaneContentKind) -> u8 {
  match kind {
    PaneContentKind::EditorBuffer => 0,
    PaneContentKind::Terminal => 1,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoverTriggerSource {
  Keyboard,
  Mouse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HoverUiState {
  trigger:        HoverTriggerSource,
  anchor_char:    usize,
  highlight_from: usize,
  highlight_to:   usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingMouseHover {
  editor_id:      LibEditorId,
  pane_id:        PaneId,
  anchor_char:    usize,
  highlight_from: usize,
  highlight_to:   usize,
  due_at:         Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    uri:            String,
    generation:     u64,
    trigger:        HoverTriggerSource,
    anchor_char:    usize,
    fallback_range: Option<(usize, usize)>,
  },
  DocumentHighlightSelect {
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
      Self::DocumentSymbols { .. } => "document-symbols",
      Self::WorkspaceSymbols { .. } => "workspace-symbols",
      Self::Completion { .. } => "completion",
      Self::CompletionResolve { .. } => "completion-resolve",
      Self::SignatureHelp { .. } => "signature-help",
      Self::CodeActions { .. } => "code-actions",
      Self::Rename { .. } => "rename",
    }
  }

  fn uri(&self) -> Option<&str> {
    match self {
      Self::GotoDeclaration { uri } => Some(uri.as_str()),
      Self::GotoDefinition { uri } => Some(uri.as_str()),
      Self::GotoTypeDefinition { uri } => Some(uri.as_str()),
      Self::GotoImplementation { uri } => Some(uri.as_str()),
      Self::Hover { uri, .. } => Some(uri.as_str()),
      Self::DocumentHighlightSelect { uri } => Some(uri.as_str()),
      Self::DocumentSymbols { uri } => Some(uri.as_str()),
      Self::WorkspaceSymbols { .. } => None,
      Self::Completion { uri, .. } => Some(uri.as_str()),
      Self::CompletionResolve { uri, .. } => Some(uri.as_str()),
      Self::SignatureHelp { uri } => Some(uri.as_str()),
      Self::CodeActions { uri } => Some(uri.as_str()),
      Self::Rename { uri } => Some(uri.as_str()),
    }
  }

  fn cancellation_key(&self) -> (&'static str, Option<&str>) {
    match self {
      Self::GotoDeclaration { uri } => ("goto-declaration", Some(uri)),
      Self::GotoDefinition { uri } => ("goto-definition", Some(uri)),
      Self::GotoTypeDefinition { uri } => ("goto-type-definition", Some(uri)),
      Self::GotoImplementation { uri } => ("goto-implementation", Some(uri)),
      Self::Hover { uri, .. } => ("hover", Some(uri)),
      Self::DocumentHighlightSelect { uri } => ("document-highlight-select", Some(uri)),
      Self::DocumentSymbols { uri } => ("document-symbols", Some(uri)),
      Self::WorkspaceSymbols { .. } => ("workspace-symbols", None),
      Self::Completion { uri, .. } => ("completion", Some(uri)),
      Self::CompletionResolve { uri, .. } => ("completion-resolve", Some(uri)),
      Self::SignatureHelp { uri } => ("signature-help", Some(uri)),
      Self::CodeActions { uri } => ("code-actions", Some(uri)),
      Self::Rename { uri } => ("rename", Some(uri)),
    }
  }
}

struct EditorState {
  mode:                              Mode,
  insert_mouse_selection_edit_armed: bool,
  append_restore_cursor_pending:     bool,
  command_prompt:                    CommandPromptState,
  command_palette:                   CommandPaletteState,
  command_palette_style:             CommandPaletteStyle,
  completion_menu:                   the_default::CompletionMenuState,
  signature_help:                    the_default::SignatureHelpState,
  file_picker:                       FilePickerState,
  file_tree:                         FileTreeState,
  search_prompt:                     SearchPromptState,
  ui_state:                          UiState,
  needs_render:                      bool,
  messages:                          MessageCenter,
  pending_input:                     Option<the_default::PendingInput>,
  register:                          Option<char>,
  macro_recording:                   Option<(char, Vec<KeyBinding>)>,
  macro_replaying:                   Vec<char>,
  macro_queue:                       VecDeque<KeyEvent>,
  text_format:                       TextFormat,
  gutter_config:                     GutterConfig,
  gutter_diff_signs:                 BTreeMap<usize, RenderGutterDiffKind>,
  vcs_statusline:                    Option<String>,
  inline_annotations:                Vec<InlineAnnotation>,
  overlay_annotations:               Vec<Overlay>,
  word_jump_inline_annotations:      Vec<InlineAnnotation>,
  word_jump_overlay_annotations:     Vec<Overlay>,
  highlight_cache:                   HighlightCache,
  inactive_highlight_caches:         BTreeMap<usize, HighlightCache>,
  syntax_parse_tx:                   Sender<SyntaxParseResult>,
  syntax_parse_rx:                   Receiver<SyntaxParseResult>,
  syntax_parse_lifecycle:            ParseLifecycle<SyntaxParseJob>,
  syntax_parse_highlight_state:      ParseHighlightState,
  frame_generation_state:            FrameGenerationState,
  diagnostic_popup:                  Option<DiagnosticPopupState>,
  hover_docs:                        Option<String>,
  hover_docs_scroll:                 usize,
  hover_ui:                          Option<HoverUiState>,
  scrolloff:                         usize,
  pointer_drag_selection:            Option<PointerSelectionDragState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiagnosticPopupState {
  markdown: String,
  severity: DiagnosticSeverity,
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

impl EditorState {
  fn new(loader: Option<Arc<Loader>>, workspace_root: &Path) -> Self {
    let mut command_palette_style = CommandPaletteStyle::floating(CommandPaletteTheme::ghostty());
    command_palette_style.layout = CommandPaletteLayout::Custom;
    let mut file_picker = FilePickerState::default();
    set_file_picker_syntax_loader(&mut file_picker, loader);
    let file_tree = FileTreeState::with_workspace_root(workspace_root.to_path_buf());
    let (syntax_parse_tx, syntax_parse_rx) = channel();

    Self {
      mode: Mode::Normal,
      insert_mouse_selection_edit_armed: false,
      append_restore_cursor_pending: false,
      command_prompt: CommandPromptState::new(),
      command_palette: CommandPaletteState::default(),
      command_palette_style,
      completion_menu: the_default::CompletionMenuState::default(),
      signature_help: the_default::SignatureHelpState::default(),
      file_picker,
      file_tree,
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
      inactive_highlight_caches: BTreeMap::new(),
      syntax_parse_tx,
      syntax_parse_rx,
      syntax_parse_lifecycle: ParseLifecycle::default(),
      syntax_parse_highlight_state: ParseHighlightState::default(),
      frame_generation_state: FrameGenerationState::default(),
      diagnostic_popup: None,
      hover_docs: None,
      hover_docs_scroll: 0,
      hover_ui: None,
      scrolloff: 5,
      pointer_drag_selection: None,
    }
  }

  fn clear_highlight_caches(&mut self) {
    self.highlight_cache.clear();
    self.inactive_highlight_caches.clear();
  }
}

fn select_ui_theme(catalog: &ThemeCatalog) -> (String, Theme) {
  match env::var("THE_EDITOR_THEME").ok() {
    Some(theme_name) => {
      let theme_name = theme_name.trim();
      if let Some(theme) = catalog.load_theme(theme_name) {
        (theme_name.to_string(), theme)
      } else {
        eprintln!("Unknown theme '{theme_name}', falling back to default theme.");
        (
          default_theme().name().to_string(),
          catalog
            .load_theme(default_theme().name())
            .unwrap_or_else(|| default_theme().clone()),
        )
      }
    },
    None => {
      (
        default_theme().name().to_string(),
        catalog
          .load_theme(default_theme().name())
          .unwrap_or_else(|| default_theme().clone()),
      )
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

fn diagnostic_severity_label(severity: DiagnosticSeverity) -> &'static str {
  match severity {
    DiagnosticSeverity::Error => "Error",
    DiagnosticSeverity::Warning => "Warning",
    DiagnosticSeverity::Information => "Information",
    DiagnosticSeverity::Hint => "Hint",
  }
}

fn diagnostic_popup_role(severity: DiagnosticSeverity) -> &'static str {
  match severity {
    DiagnosticSeverity::Error => "diagnostic_docs.error",
    DiagnosticSeverity::Warning => "diagnostic_docs.warning",
    DiagnosticSeverity::Information => "diagnostic_docs.information",
    DiagnosticSeverity::Hint => "diagnostic_docs.hint",
  }
}

fn escape_docs_markdown(text: &str) -> String {
  let mut escaped = String::with_capacity(text.len());
  for ch in text.chars() {
    match ch {
      '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '#' | '+' | '-' | '!' | '|'
      | '>' => {
        escaped.push('\\');
        escaped.push(ch);
      },
      _ => escaped.push(ch),
    }
  }
  escaped
}

fn escape_docs_code_span(text: &str) -> String {
  text.replace('\\', "\\\\").replace('`', "\\`")
}

fn lsp_position_tuple(position: LspPosition) -> (u32, u32) {
  (position.line, position.character)
}

fn diagnostic_range_tuples(diagnostic: &Diagnostic) -> ((u32, u32), (u32, u32)) {
  let start = (
    diagnostic.range.start.line,
    diagnostic.range.start.character,
  );
  let end = (diagnostic.range.end.line, diagnostic.range.end.character);
  if start <= end {
    (start, end)
  } else {
    (end, start)
  }
}

fn diagnostic_contains_lsp_position(diagnostic: &Diagnostic, position: LspPosition) -> bool {
  let point = lsp_position_tuple(position);
  let (start, end) = diagnostic_range_tuples(diagnostic);
  start <= point && point <= end
}

fn diagnostic_touches_lsp_line(diagnostic: &Diagnostic, line: u32) -> bool {
  let (start, end) = diagnostic_range_tuples(diagnostic);
  start.0 <= line && line <= end.0
}

fn diagnostic_popup_range_len(diagnostic: &Diagnostic) -> u64 {
  let (start, end) = diagnostic_range_tuples(diagnostic);
  let line_span = end.0.saturating_sub(start.0) as u64;
  let char_span = end.1.saturating_sub(start.1) as u64;
  line_span
    .saturating_mul(1_000_000)
    .saturating_add(char_span)
}

fn sort_diagnostics_for_popup(diagnostics: &mut [Diagnostic]) {
  diagnostics.sort_by(|left, right| {
    diagnostic_severity_rank(right.severity.unwrap_or(DiagnosticSeverity::Warning))
      .cmp(&diagnostic_severity_rank(
        left.severity.unwrap_or(DiagnosticSeverity::Warning),
      ))
      .then_with(|| diagnostic_popup_range_len(left).cmp(&diagnostic_popup_range_len(right)))
      .then_with(|| left.range.start.line.cmp(&right.range.start.line))
      .then_with(|| left.range.start.character.cmp(&right.range.start.character))
      .then_with(|| left.message.cmp(&right.message))
  });
}

fn build_diagnostic_popup_state(diagnostics: &[Diagnostic]) -> Option<DiagnosticPopupState> {
  let mut markdown = String::new();
  let mut wrote_any = false;
  let mut popup_severity = None;

  for diagnostic in diagnostics {
    let message = diagnostic.message.trim();
    if message.is_empty() {
      continue;
    }

    if wrote_any {
      markdown.push_str("\n\n---\n\n");
    }

    let severity = diagnostic.severity.unwrap_or(DiagnosticSeverity::Warning);
    popup_severity.get_or_insert(severity);
    markdown.push_str("### ");
    markdown.push_str(diagnostic_severity_label(severity));
    markdown.push_str("\n\n");

    let escaped_message = message
      .lines()
      .map(escape_docs_markdown)
      .collect::<Vec<_>>()
      .join("\n");
    markdown.push_str(escaped_message.trim_end());

    let mut metadata_lines = Vec::new();
    if let Some(source) = diagnostic
      .source
      .as_deref()
      .map(str::trim)
      .filter(|text| !text.is_empty())
    {
      metadata_lines.push(format!("Source: `{}`", escape_docs_code_span(source)));
    }
    if let Some(code) = diagnostic
      .code
      .as_deref()
      .map(str::trim)
      .filter(|text| !text.is_empty())
    {
      metadata_lines.push(format!("Code: `{}`", escape_docs_code_span(code)));
    }
    if !metadata_lines.is_empty() {
      markdown.push_str("\n\n");
      for line in metadata_lines {
        markdown.push_str("- ");
        markdown.push_str(&line);
        markdown.push('\n');
      }
      markdown.truncate(markdown.trim_end_matches('\n').len());
    }

    wrote_any = true;
  }

  match (wrote_any, popup_severity) {
    (true, Some(severity)) => Some(DiagnosticPopupState { markdown, severity }),
    _ => None,
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

fn render_plan_row_end_cols(plan: &the_lib::render::RenderPlan) -> Vec<usize> {
  let col_start = plan.scroll.col;
  let mut row_end_cols = vec![col_start; plan.viewport.height as usize];
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
  row_end_cols
}

fn render_plan_row_visible_end_col(
  plan: &the_lib::render::RenderPlan,
  row: usize,
  row_visible_end_cols: &[usize],
) -> usize {
  let row_start = plan.scroll.row;
  let relative_row = row.saturating_sub(row_start);
  row_visible_end_cols
    .get(relative_row)
    .copied()
    .unwrap_or(plan.scroll.col)
}

fn push_render_selection_rects(
  out: &mut Vec<the_lib::render::RenderSelection>,
  plan: &the_lib::render::RenderPlan,
  start: LibPosition,
  end: LibPosition,
  style: LibStyle,
  kind: the_lib::render::RenderSelectionKind,
  row_visible_end_cols: &[usize],
) {
  let row_start = plan.scroll.row;
  let row_end = row_start + plan.viewport.height as usize;
  let col_start = plan.scroll.col;
  let col_end = col_start + plan.content_width();

  let start_row = start.row;
  let end_row = end.row;

  if start_row == end_row {
    let row = start_row;
    if row < row_start || row >= row_end {
      return;
    }
    let from = start.col.min(end.col).max(col_start);
    let mut to = start.col.max(end.col);
    to = to.min(render_plan_row_visible_end_col(
      plan,
      row,
      row_visible_end_cols,
    ));
    if to <= from {
      return;
    }
    out.push(the_lib::render::RenderSelection {
      rect: LibRect::new(
        (from - col_start) as u16,
        (row - row_start) as u16,
        (to - from) as u16,
        1,
      ),
      style,
      kind,
    });
    return;
  }

  for row in start_row..=end_row {
    if row < row_start || row >= row_end {
      continue;
    }
    let row_end_col = render_plan_row_visible_end_col(plan, row, row_visible_end_cols);
    let (from, to) = if row == start_row {
      (start.col, row_end_col)
    } else if row == end_row {
      (col_start, end.col.min(row_end_col))
    } else {
      (col_start, row_end_col)
    };

    let from = from.max(col_start);
    let to = to.min(col_end);
    if to <= from {
      continue;
    }

    out.push(the_lib::render::RenderSelection {
      rect: LibRect::new(
        (from - col_start) as u16,
        (row - row_start) as u16,
        (to - from) as u16,
        1,
      ),
      style,
      kind,
    });
  }
}

fn compute_hover_highlight_selections<'a>(
  text: &'a Rope,
  hover_ui: HoverUiState,
  plan: &the_lib::render::RenderPlan,
  text_fmt: &'a TextFormat,
  annotations: &mut TextAnnotations<'a>,
  style: LibStyle,
) -> Vec<the_lib::render::RenderSelection> {
  let Some((highlight_from, highlight_to)) =
    App::normalize_char_range(text, hover_ui.highlight_from, hover_ui.highlight_to)
  else {
    return Vec::new();
  };

  let text_slice = text.slice(..);
  let Some(start_pos) = visual_pos_at_char(text_slice, text_fmt, annotations, highlight_from)
  else {
    return Vec::new();
  };
  let Some(end_pos) = visual_pos_at_char(text_slice, text_fmt, annotations, highlight_to) else {
    return Vec::new();
  };

  let (start_pos, end_pos) = if (end_pos.row, end_pos.col) < (start_pos.row, start_pos.col) {
    (end_pos, start_pos)
  } else {
    (start_pos, end_pos)
  };

  let row_visible_end_cols = render_plan_row_end_cols(plan);
  let mut selections = Vec::new();
  push_render_selection_rects(
    &mut selections,
    plan,
    start_pos,
    end_pos,
    style,
    the_lib::render::RenderSelectionKind::Hover,
    &row_visible_end_cols,
  );
  selections
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

fn remap_relative_row_with_insertions(
  relative_row: usize,
  scroll_row: usize,
  viewport_height: usize,
  row_insertions: &[the_lib::render::RenderRowInsertion],
) -> Option<u16> {
  let absolute_row = scroll_row.saturating_add(relative_row);
  let inserted_before = row_insertions
    .iter()
    .filter(|insertion| insertion.base_row < absolute_row)
    .map(|insertion| insertion.inserted_rows)
    .sum::<usize>();
  let shifted_row = relative_row.saturating_add(inserted_before);
  (shifted_row < viewport_height).then_some(shifted_row as u16)
}

fn apply_row_insertions_to_underlines(
  entries: &mut Vec<DiagnosticUnderlineEntry>,
  plan: &the_lib::render::RenderPlan,
  row_insertions: &[the_lib::render::RenderRowInsertion],
) {
  if row_insertions.is_empty() {
    return;
  }

  entries.retain_mut(|entry| {
    let Some(row) = remap_relative_row_with_insertions(
      entry.row as usize,
      plan.scroll.row,
      plan.viewport.height as usize,
      row_insertions,
    ) else {
      return false;
    };
    entry.row = row;
    true
  });
}

fn update_render_row_hash(row_hashes: &mut [u64], row: usize, value: impl Hash) {
  let Some(slot) = row_hashes.get_mut(row) else {
    return;
  };
  let mut hasher = DefaultHasher::new();
  slot.hash(&mut hasher);
  value.hash(&mut hasher);
  *slot = hasher.finish();
}

fn append_inline_diagnostic_row_hashes(
  row_hashes: &mut [u64],
  lines: &[InlineDiagnosticRenderLine],
) {
  for line in lines {
    update_render_row_hash(
      row_hashes,
      line.row as usize,
      (
        line.col,
        line.text.as_str(),
        severity_to_u8(line.severity),
      ),
    );
  }
}

fn append_eol_diagnostic_row_hashes(
  row_hashes: &mut [u64],
  entries: &[EolDiagnosticEntry],
) {
  for entry in entries {
    update_render_row_hash(
      row_hashes,
      entry.row as usize,
      (
        entry.col,
        entry.message.as_str(),
        severity_to_u8(entry.severity),
      ),
    );
  }
}

fn append_diagnostic_underline_row_hashes(
  row_hashes: &mut [u64],
  entries: &[DiagnosticUnderlineEntry],
) {
  for entry in entries {
    update_render_row_hash(
      row_hashes,
      entry.row as usize,
      (
        entry.start_col,
        entry.end_col,
        severity_to_u8(entry.severity),
      ),
    );
  }
}

fn build_render_layer_row_hashes(
  plan: &the_lib::render::RenderPlan,
  inline_diagnostic_lines: &[InlineDiagnosticRenderLine],
  eol_diagnostics: &[EolDiagnosticEntry],
  diagnostic_underlines: &[DiagnosticUnderlineEntry],
) -> RenderLayerRowHashes {
  let mut row_hashes = base_render_layer_row_hashes(plan);
  append_inline_diagnostic_row_hashes(&mut row_hashes.decoration_rows, inline_diagnostic_lines);
  append_eol_diagnostic_row_hashes(&mut row_hashes.decoration_rows, eol_diagnostics);
  append_diagnostic_underline_row_hashes(&mut row_hashes.decoration_rows, diagnostic_underlines);
  row_hashes
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[repr(u8)]
enum VcsFileStatusKind {
  #[default]
  None      = 0,
  Modified  = 1,
  Untracked = 2,
  Conflict  = 3,
  Deleted   = 4,
  Renamed   = 5,
}

impl VcsFileStatusKind {
  fn from_change(change: &the_vcs::FileChange) -> Self {
    match change {
      the_vcs::FileChange::Modified { .. } => Self::Modified,
      the_vcs::FileChange::Untracked { .. } => Self::Untracked,
      the_vcs::FileChange::Conflict { .. } => Self::Conflict,
      the_vcs::FileChange::Deleted { .. } => Self::Deleted,
      the_vcs::FileChange::Renamed { .. } => Self::Renamed,
    }
  }

  fn merge(self, other: Self) -> Self {
    if self.priority() >= other.priority() {
      self
    } else {
      other
    }
  }

  fn priority(self) -> u8 {
    match self {
      Self::None => 0,
      Self::Untracked => 1,
      Self::Modified => 2,
      Self::Renamed => 3,
      Self::Deleted => 4,
      Self::Conflict => 5,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct VcsChangeCounts {
  modified:  usize,
  untracked: usize,
  conflict:  usize,
  deleted:   usize,
  renamed:   usize,
}

impl VcsChangeCounts {
  fn record(&mut self, status: VcsFileStatusKind) {
    match status {
      VcsFileStatusKind::None => {},
      VcsFileStatusKind::Modified => self.modified += 1,
      VcsFileStatusKind::Untracked => self.untracked += 1,
      VcsFileStatusKind::Conflict => self.conflict += 1,
      VcsFileStatusKind::Deleted => self.deleted += 1,
      VcsFileStatusKind::Renamed => self.renamed += 1,
    }
  }

  fn summary_text(&self) -> Option<String> {
    let mut parts = Vec::new();
    if self.conflict > 0 {
      parts.push(format!("U{}", self.conflict));
    }
    if self.modified > 0 {
      parts.push(format!("M{}", self.modified));
    }
    if self.deleted > 0 {
      parts.push(format!("D{}", self.deleted));
    }
    if self.renamed > 0 {
      parts.push(format!("R{}", self.renamed));
    }
    if self.untracked > 0 {
      parts.push(format!("?{}", self.untracked));
    }
    if parts.is_empty() {
      None
    } else {
      Some(parts.join(" "))
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct VcsUiState {
  generation:               u64,
  counts:                   VcsChangeCounts,
  status_by_path:           HashMap<PathBuf, VcsFileStatusKind>,
  directory_changed_counts: HashMap<PathBuf, usize>,
}

impl VcsUiState {
  fn from_changes(changes: Vec<the_vcs::FileChange>) -> Self {
    let mut status_by_path: HashMap<PathBuf, VcsFileStatusKind> = HashMap::new();
    for change in changes {
      let status = VcsFileStatusKind::from_change(&change);
      let path = normalize_vcs_lookup_path(change.path());
      status_by_path
        .entry(path)
        .and_modify(|existing| *existing = existing.merge(status))
        .or_insert(status);
    }

    let mut counts = VcsChangeCounts::default();
    let mut directory_changed_counts = HashMap::new();
    for (path, status) in &status_by_path {
      counts.record(*status);
      let mut ancestor = path.parent();
      while let Some(dir) = ancestor {
        *directory_changed_counts
          .entry(dir.to_path_buf())
          .or_insert(0) += 1;
        ancestor = dir.parent();
      }
    }

    Self {
      generation: 0,
      counts,
      status_by_path,
      directory_changed_counts,
    }
  }

  fn semantically_eq(&self, other: &Self) -> bool {
    self.counts == other.counts
      && self.status_by_path == other.status_by_path
      && self.directory_changed_counts == other.directory_changed_counts
  }

  fn status_for_path(&self, path: &Path) -> VcsFileStatusKind {
    let normalized = normalize_vcs_lookup_path(path);
    self
      .status_by_path
      .get(&normalized)
      .copied()
      .unwrap_or(VcsFileStatusKind::None)
  }

  fn changed_descendant_count(&self, path: &Path) -> usize {
    let normalized = normalize_vcs_lookup_path(path);
    self
      .directory_changed_counts
      .get(&normalized)
      .copied()
      .unwrap_or(0)
  }
}

fn normalize_vcs_lookup_path(path: &Path) -> PathBuf {
  let absolute = if path.is_absolute() {
    path.to_path_buf()
  } else {
    env::current_dir()
      .unwrap_or_else(|_| PathBuf::from("."))
      .join(path)
  };

  let mut normalized = PathBuf::new();
  for component in absolute.components() {
    match component {
      Component::CurDir => {},
      Component::ParentDir => {
        normalized.pop();
      },
      other => normalized.push(other.as_os_str()),
    }
  }
  normalized
}

fn format_vcs_statusline_text(info: &the_vcs::VcsStatuslineInfo) -> String {
  info.statusline_text()
}

fn is_symbol_word_char(ch: char) -> bool {
  ch == '_' || ch.is_alphanumeric()
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

fn lsp_hover_auto_trigger_latency() -> Duration {
  Duration::from_millis(350)
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
    "ui.selection.active",
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

fn docs_render_runtime() -> &'static RwLock<DocsRenderRuntime> {
  static RUNTIME: OnceLock<RwLock<DocsRenderRuntime>> = OnceLock::new();
  RUNTIME.get_or_init(|| {
    let catalog = ThemeCatalog::load(None);
    let (_, theme) = select_ui_theme(&catalog);
    let loader = init_loader(&theme).ok().map(Arc::new);
    RwLock::new(DocsRenderRuntime { theme, loader })
  })
}

fn set_docs_render_theme(theme: &Theme) {
  let runtime = docs_render_runtime();
  let mut runtime = runtime.write().unwrap_or_else(|err| err.into_inner());
  runtime.theme = theme.clone();
  runtime.loader = init_loader(theme).ok().map(Arc::new);
}

fn completion_docs_render_json_impl(
  markdown: &str,
  content_width: usize,
  language_hint: &str,
) -> String {
  let runtime = docs_render_runtime()
    .read()
    .unwrap_or_else(|err| err.into_inner());
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
  let mut value = serde_json::json!({
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
        serde_json::json!(diagnostic_severity_to_lsp_code(severity)),
      );
    }
    if let Some(code) = &diagnostic.code {
      object.insert("code".into(), serde_json::json!(code));
    }
    if let Some(source) = &diagnostic.source {
      object.insert("source".into(), serde_json::json!(source));
    }
  }

  value
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

fn shared_lsp_trace_enabled() -> bool {
  static ENABLED: OnceLock<bool> = OnceLock::new();
  *ENABLED.get_or_init(|| {
    env::var("THE_EDITOR_SWIFT_SHARED_LSP_TRACE")
      .ok()
      .map(|value| {
        let normalized = value.trim().to_ascii_lowercase();
        normalized == "1" || normalized == "true" || normalized == "yes" || normalized == "on"
      })
      .unwrap_or(false)
  })
}

fn log_shared_lsp_debug(context: &str, message: impl AsRef<str>) {
  if !shared_lsp_trace_enabled() {
    return;
  }
  eprintln!("[the-ffi lsp] {context} {}", message.as_ref());
}

fn shared_lsp_editor_id_label(id: Option<LibEditorId>) -> String {
  id.map(|editor_id| editor_id.get().get().to_string())
    .unwrap_or_else(|| "none".to_string())
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

fn normalize_path_for_open(path: &Path) -> PathBuf {
  let absolute = if path.is_absolute() {
    path.to_path_buf()
  } else {
    env::current_dir()
      .ok()
      .map(|cwd| cwd.join(path))
      .unwrap_or_else(|| path.to_path_buf())
  };
  std::fs::canonicalize(&absolute).unwrap_or(absolute)
}

/// FFI-safe app wrapper with editor management.
pub struct App {
  inner:                           LibApp,
  workspace_root:                  PathBuf,
  dispatch:                        DefaultDispatchStatic<App>,
  keymaps:                         Keymaps,
  command_registry:                CommandRegistry<App>,
  states:                          HashMap<LibEditorId, EditorState>,
  vcs_provider:                    DiffProviderRegistry,
  vcs_diff_handles:                HashMap<LibEditorId, DiffHandle>,
  active_editor:                   Option<LibEditorId>,
  should_quit:                     bool,
  cursor_blink_generation:         u64,
  registers:                       Registers,
  last_motion:                     Option<Motion>,
  lsp_server_name:                 Option<String>,
  lsp_runtime:                     LspRuntime,
  lsp_ready:                       bool,
  lsp_document:                    Option<LspDocumentSyncState>,
  lsp_statusline:                  LspStatuslineState,
  lsp_spinner_index:               usize,
  lsp_spinner_last_tick:           Instant,
  vcs_ui:                          VcsUiState,
  lsp_active_progress_tokens:      HashSet<String>,
  lsp_watched_file:                Option<LspWatchedFileState>,
  lsp_pending_requests:            HashMap<u64, PendingLspRequestKind>,
  lsp_completion_items:            Vec<LspCompletionItem>,
  lsp_completion_raw_items:        Vec<Value>,
  lsp_completion_resolved:         HashSet<usize>,
  lsp_completion_visible:          Vec<usize>,
  lsp_completion_start:            Option<usize>,
  lsp_completion_generation:       u64,
  lsp_code_action_items:           Vec<LspCodeAction>,
  lsp_code_action_menu_active:     bool,
  lsp_pending_auto_completion:     Option<PendingAutoCompletion>,
  lsp_pending_auto_signature_help: Option<PendingAutoSignatureHelp>,
  lsp_pending_mouse_hover:         Option<PendingMouseHover>,
  lsp_hover_generation:            u64,
  diagnostics:                     DiagnosticsState,
  global_search:                   GlobalSearchState,
  inline_diagnostic_lines:         Vec<InlineDiagnosticRenderLine>,
  eol_diagnostics:                 Vec<EolDiagnosticEntry>,
  diagnostic_underlines:           Vec<DiagnosticUnderlineEntry>,
  render_inline_diagnostic_lines:  bool,
  render_theme_generation:         u64,
  ui_theme_catalog:                ThemeCatalog,
  ui_theme_name:                   String,
  ui_theme_base:                   Theme,
  ui_theme_preview_name:           Option<String>,
  ui_theme:                        Theme,
  loader:                          Option<Arc<Loader>>,
  native_tab_open_gateway_enabled: bool,
  native_tab_open_requests:        VecDeque<NativeTabOpenRequest>,
}

// MARK: - File picker preview FFI types

#[derive(Clone)]
pub struct PreviewLineSegment {
  text:         String,
  highlight_id: u32,
  is_match:     bool,
}

impl Default for PreviewLineSegment {
  fn default() -> Self {
    Self {
      text:         String::new(),
      highlight_id: 0,
      is_match:     false,
    }
  }
}

impl PreviewLineSegment {
  fn text(&self) -> String {
    self.text.clone()
  }

  fn highlight_id(&self) -> u32 {
    self.highlight_id
  }

  fn is_match(&self) -> bool {
    self.is_match
  }
}

/// A single render-ready preview row.
#[derive(Clone)]
pub struct PreviewLine {
  kind:        u8, // 0=content, 1=truncated-above marker, 2=truncated-below marker
  virtual_row: usize,
  line_number: usize, // 1-based source line, 0 for non-source marker rows
  focused:     bool,
  marker:      String,
  segments:    Vec<PreviewLineSegment>,
}

impl Default for PreviewLine {
  fn default() -> Self {
    Self {
      kind:        0,
      virtual_row: 0,
      line_number: 0,
      focused:     false,
      marker:      String::new(),
      segments:    Vec::new(),
    }
  }
}

impl PreviewLine {
  fn kind(&self) -> u8 {
    self.kind
  }

  fn virtual_row(&self) -> usize {
    self.virtual_row
  }

  fn line_number(&self) -> usize {
    self.line_number
  }

  fn focused(&self) -> bool {
    self.focused
  }

  fn marker(&self) -> String {
    self.marker.clone()
  }

  fn segment_count(&self) -> usize {
    self.segments.len()
  }

  fn segment_at(&self, index: usize) -> PreviewLineSegment {
    self.segments.get(index).cloned().unwrap_or_default()
  }
}

/// Snapshot of the file picker preview, ready for Swift consumption.
/// Built as a small window around the visible rows.
pub struct PreviewData {
  kind:         u8, // 0=empty, 1=source, 2=text, 3=message
  path:         String,
  loading:      bool,
  truncated:    bool,
  total_lines:  usize,
  show:         bool,
  offset:       usize,
  window_start: usize,
  lines:        Vec<PreviewLine>,
}

impl Default for PreviewData {
  fn default() -> Self {
    Self {
      kind:         0,
      path:         String::new(),
      loading:      false,
      truncated:    false,
      total_lines:  0,
      show:         true,
      offset:       0,
      window_start: 0,
      lines:        Vec::new(),
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

  fn offset(&self) -> usize {
    self.offset
  }

  fn window_start(&self) -> usize {
    self.window_start
  }

  fn line_count(&self) -> usize {
    self.lines.len()
  }

  fn line_at(&self, index: usize) -> PreviewLine {
    self.lines.get(index).cloned().unwrap_or_default()
  }
}

fn build_preview_data(
  picker: &FilePickerState,
  offset: usize,
  visible_rows: usize,
  overscan: usize,
) -> PreviewData {
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

  let window: FilePickerPreviewWindow =
    file_picker_preview_window(picker, offset, visible_rows, overscan);
  let truncated = matches!(
    &picker.preview,
    FilePickerPreview::Source(source)
      if source.truncated_above_lines > 0 || source.truncated_below_lines > 0
  );

  let lines = window
    .lines
    .into_iter()
    .map(|line| {
      PreviewLine {
        kind:        match line.kind {
          FilePickerPreviewLineKind::Content => 0,
          FilePickerPreviewLineKind::TruncatedAbove => 1,
          FilePickerPreviewLineKind::TruncatedBelow => 2,
        },
        virtual_row: line.virtual_row,
        line_number: line.line_number.unwrap_or(0),
        focused:     line.focused,
        marker:      line.marker,
        segments:    line
          .segments
          .into_iter()
          .map(|segment| {
            PreviewLineSegment {
              text:         segment.text,
              highlight_id: segment.highlight_id.unwrap_or(0),
              is_match:     segment.is_match,
            }
          })
          .collect(),
      }
    })
    .collect();

  PreviewData {
    kind: window.kind,
    path: preview_path,
    loading: picker.preview_loading(),
    truncated,
    total_lines: window.total_virtual_rows,
    show: picker.show_preview,
    offset: window.offset,
    window_start: window.window_start,
    lines,
  }
}

// ── File picker snapshot (direct FFI, no JSON) ─────────────────────────

#[derive(Serialize)]
struct BufferTabItemSnapshotJson {
  buffer_id:      u64,
  buffer_index:   usize,
  title:          String,
  modified:       bool,
  is_active:      bool,
  file_path:      Option<String>,
  directory_hint: Option<String>,
  vcs_status:     u8,
}

impl BufferTabItemSnapshotJson {
  fn from_snapshot(value: DefaultBufferTabItemSnapshot, vcs_ui: &VcsUiState) -> Self {
    let vcs_status = value
      .file_path
      .as_deref()
      .map(|path| vcs_ui.status_for_path(path) as u8)
      .unwrap_or(VcsFileStatusKind::None as u8);
    Self {
      buffer_id: value.buffer_id,
      buffer_index: value.buffer_index,
      title: value.title,
      modified: value.modified,
      is_active: value.is_active,
      file_path: value.file_path.map(|path| path.display().to_string()),
      directory_hint: value.directory_hint,
      vcs_status,
    }
  }
}

#[derive(Clone, Copy)]
enum NativeTabOpenRequestKind {
  FocusExisting,
  OpenNew,
}

#[derive(Clone)]
struct NativeTabOpenRequest {
  kind:      NativeTabOpenRequestKind,
  buffer_id: u64,
  file_path: Option<PathBuf>,
}

impl NativeTabOpenRequest {
  fn focus_existing(buffer_id: u64, file_path: Option<PathBuf>) -> Self {
    Self {
      kind: NativeTabOpenRequestKind::FocusExisting,
      buffer_id,
      file_path,
    }
  }

  fn open_new(buffer_id: u64, file_path: Option<PathBuf>) -> Self {
    Self {
      kind: NativeTabOpenRequestKind::OpenNew,
      buffer_id,
      file_path,
    }
  }
}

#[derive(Serialize)]
struct NativeTabOpenRequestJson {
  kind:      &'static str,
  buffer_id: u64,
  file_path: Option<String>,
}

impl From<NativeTabOpenRequest> for NativeTabOpenRequestJson {
  fn from(value: NativeTabOpenRequest) -> Self {
    let kind = match value.kind {
      NativeTabOpenRequestKind::FocusExisting => "focus_existing",
      NativeTabOpenRequestKind::OpenNew => "open_new",
    };
    Self {
      kind,
      buffer_id: value.buffer_id,
      file_path: value
        .file_path
        .map(|path| path.to_string_lossy().into_owned()),
    }
  }
}

#[derive(Serialize)]
struct BufferTabsSnapshotJson {
  visible:             bool,
  active_tab:          Option<usize>,
  active_buffer_index: Option<usize>,
  tabs:                Vec<BufferTabItemSnapshotJson>,
}

impl BufferTabsSnapshotJson {
  fn from_snapshot(value: DefaultBufferTabsSnapshot, vcs_ui: &VcsUiState) -> Self {
    Self {
      visible:             value.visible,
      active_tab:          value.active_tab,
      active_buffer_index: value.active_buffer_index,
      tabs:                value
        .tabs
        .into_iter()
        .map(|tab| BufferTabItemSnapshotJson::from_snapshot(tab, vcs_ui))
        .collect(),
    }
  }
}

pub struct FileTreeSnapshotData {
  visible:            bool,
  mode:               u8, // 0=workspace-root, 1=current-buffer-directory
  root:               String,
  selected_path:      String,
  refresh_generation: u64,
  vcs_generation:     u64,
  nodes:              Vec<FileTreeNodeFFI>,
}

impl Default for FileTreeSnapshotData {
  fn default() -> Self {
    Self {
      visible:            false,
      mode:               0,
      root:               String::new(),
      selected_path:      String::new(),
      refresh_generation: 0,
      vcs_generation:     0,
      nodes:              Vec::new(),
    }
  }
}

impl FileTreeSnapshotData {
  fn visible(&self) -> bool {
    self.visible
  }
  fn mode(&self) -> u8 {
    self.mode
  }
  fn root(&self) -> String {
    self.root.clone()
  }
  fn selected_path(&self) -> String {
    self.selected_path.clone()
  }
  fn refresh_generation(&self) -> u64 {
    self.refresh_generation
  }
  fn vcs_generation(&self) -> u64 {
    self.vcs_generation
  }
  fn node_count(&self) -> usize {
    self.nodes.len()
  }
  fn node_at(&self, index: usize) -> FileTreeNodeFFI {
    self.nodes.get(index).cloned().unwrap_or_default()
  }
}

#[derive(Clone)]
pub struct FileTreeNodeFFI {
  id:                    String,
  path:                  String,
  name:                  String,
  depth:                 usize,
  kind:                  u8, // 0=file, 1=directory
  expanded:              bool,
  selected:              bool,
  has_unloaded_children: bool,
  vcs_status:            u8,
  vcs_descendant_count:  usize,
}

impl Default for FileTreeNodeFFI {
  fn default() -> Self {
    Self {
      id:                    String::new(),
      path:                  String::new(),
      name:                  String::new(),
      depth:                 0,
      kind:                  0,
      expanded:              false,
      selected:              false,
      has_unloaded_children: false,
      vcs_status:            0,
      vcs_descendant_count:  0,
    }
  }
}

impl FileTreeNodeFFI {
  fn id(&self) -> String {
    self.id.clone()
  }
  fn path(&self) -> String {
    self.path.clone()
  }
  fn name(&self) -> String {
    self.name.clone()
  }
  fn depth(&self) -> usize {
    self.depth
  }
  fn kind(&self) -> u8 {
    self.kind
  }
  fn expanded(&self) -> bool {
    self.expanded
  }
  fn selected(&self) -> bool {
    self.selected
  }
  fn has_unloaded_children(&self) -> bool {
    self.has_unloaded_children
  }
  fn vcs_status(&self) -> u8 {
    self.vcs_status
  }
  fn vcs_descendant_count(&self) -> usize {
    self.vcs_descendant_count
  }
}

fn build_file_tree_snapshot_data(
  snapshot: DefaultFileTreeSnapshot,
  max_nodes: usize,
  vcs_ui: &VcsUiState,
) -> FileTreeSnapshotData {
  let mode = match snapshot.mode {
    DefaultFileTreeMode::WorkspaceRoot => 0,
    DefaultFileTreeMode::CurrentBufferDirectory => 1,
  };

  let limit = max_nodes.max(1);
  let nodes = snapshot
    .nodes
    .into_iter()
    .take(limit)
    .map(|node| {
      let kind = match node.kind {
        DefaultFileTreeNodeKind::File => 0,
        DefaultFileTreeNodeKind::Directory => 1,
      };
      FileTreeNodeFFI {
        id: node.id,
        path: node.path.to_string_lossy().into_owned(),
        name: node.name,
        depth: node.depth,
        kind,
        expanded: node.expanded,
        selected: node.selected,
        has_unloaded_children: node.has_unloaded_children,
        vcs_status: if matches!(node.kind, DefaultFileTreeNodeKind::File) {
          vcs_ui.status_for_path(node.path.as_path()) as u8
        } else {
          VcsFileStatusKind::None as u8
        },
        vcs_descendant_count: if matches!(node.kind, DefaultFileTreeNodeKind::Directory) {
          vcs_ui.changed_descendant_count(node.path.as_path())
        } else {
          0
        },
      }
    })
    .collect::<Vec<_>>();

  FileTreeSnapshotData {
    visible: snapshot.visible,
    mode,
    root: snapshot.root.to_string_lossy().into_owned(),
    selected_path: snapshot
      .selected_path
      .map(|path| path.to_string_lossy().into_owned())
      .unwrap_or_default(),
    refresh_generation: snapshot.refresh_generation,
    vcs_generation: vcs_ui.generation,
    nodes,
  }
}

pub struct FilePickerSnapshotData {
  active:        bool,
  title:         String,
  picker_kind:   u8, // 0=generic, 1=diagnostics, 2=symbols, 3=live_grep
  query:         String,
  matched_count: usize,
  total_count:   usize,
  scanning:      bool,
  root:          String,
  selected_index: i64,
  window_start:  usize,
  items:         Vec<FilePickerItemFFI>,
}

impl Default for FilePickerSnapshotData {
  fn default() -> Self {
    Self {
      active:        false,
      title:         String::new(),
      picker_kind:   0,
      query:         String::new(),
      matched_count: 0,
      total_count:   0,
      scanning:      false,
      root:          String::new(),
      selected_index: -1,
      window_start:  0,
      items:         Vec::new(),
    }
  }
}

impl FilePickerSnapshotData {
  fn active(&self) -> bool {
    self.active
  }
  fn title(&self) -> String {
    self.title.clone()
  }
  fn picker_kind(&self) -> u8 {
    self.picker_kind
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

  fn selected_index(&self) -> i64 {
    self.selected_index
  }

  fn window_start(&self) -> usize {
    self.window_start
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
  row_kind:      u8, // 0=generic, 1=diagnostics, 2=symbols, 3=live-grep-header, 4=live-grep-match
  severity:      u8, // 0=none, 1=error, 2=warning, 3=info, 4=hint
  primary:       String,
  secondary:     String,
  tertiary:      String,
  quaternary:    String,
  line:          usize,
  column:        usize,
  depth:         usize,
}

impl Default for FilePickerItemFFI {
  fn default() -> Self {
    Self {
      display:       String::new(),
      is_dir:        false,
      icon:          String::new(),
      match_indices: Vec::new(),
      row_kind:      0,
      severity:      0,
      primary:       String::new(),
      secondary:     String::new(),
      tertiary:      String::new(),
      quaternary:    String::new(),
      line:          0,
      column:        0,
      depth:         0,
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
  fn row_kind(&self) -> u8 {
    self.row_kind
  }
  fn severity(&self) -> u8 {
    self.severity
  }
  fn primary(&self) -> String {
    self.primary.clone()
  }
  fn secondary(&self) -> String {
    self.secondary.clone()
  }
  fn tertiary(&self) -> String {
    self.tertiary.clone()
  }
  fn quaternary(&self) -> String {
    self.quaternary.clone()
  }
  fn line(&self) -> usize {
    self.line
  }
  fn column(&self) -> usize {
    self.column
  }
  fn depth(&self) -> usize {
    self.depth
  }
}

fn build_file_picker_snapshot_window(
  picker: &FilePickerState,
  window_start: usize,
  max_items: usize,
) -> FilePickerSnapshotData {
  if !picker.active {
    return FilePickerSnapshotData::default();
  }

  let matched_count = picker.matched_count();
  let total_count = picker.total_count();
  let scanning = picker.scanning || picker.matcher_running;
  let picker_kind = file_picker_kind_from_title(picker.title.as_str());
  let picker_kind = match picker_kind {
    FilePickerKind::Generic => 0,
    FilePickerKind::Diagnostics => 1,
    FilePickerKind::Symbols => 2,
    FilePickerKind::LiveGrep => 3,
  };
  let root_display = picker
    .root
    .file_name()
    .map(|n| n.to_string_lossy().into_owned())
    .unwrap_or_default();

  let clamped_start = window_start.min(matched_count);
  let limit = max_items.min(matched_count.saturating_sub(clamped_start));
  let mut items = Vec::with_capacity(limit);
  let mut match_buf = Vec::new();
  for matched_index in clamped_start..clamped_start.saturating_add(limit) {
    if let Some(item) = picker.matched_item_with_match_indices(matched_index, &mut match_buf) {
      let row: FilePickerRowData = file_picker_row_data(picker.title.as_str(), &item);
      let row_kind = match row.kind {
        FilePickerRowKind::Generic => 0,
        FilePickerRowKind::Diagnostics => 1,
        FilePickerRowKind::Symbols => 2,
        FilePickerRowKind::LiveGrepHeader => 3,
        FilePickerRowKind::LiveGrepMatch => 4,
      };
      let severity = match row.severity {
        Some(DiagnosticSeverity::Error) => 1,
        Some(DiagnosticSeverity::Warning) => 2,
        Some(DiagnosticSeverity::Information) => 3,
        Some(DiagnosticSeverity::Hint) => 4,
        None => 0,
      };
      items.push(FilePickerItemFFI {
        display: item.display.to_string(),
        is_dir: item.is_dir,
        icon: item.icon.to_string(),
        match_indices: match_buf.iter().map(|&idx| idx as u32).collect(),
        row_kind,
        severity,
        primary: row.primary,
        secondary: row.secondary,
        tertiary: row.tertiary,
        quaternary: row.quaternary,
        line: row.line,
        column: row.column,
        depth: row.depth,
      });
    }
  }

  FilePickerSnapshotData {
    active: true,
    title: picker.title.clone(),
    picker_kind,
    query: picker.query.clone(),
    matched_count,
    total_count,
    scanning,
    root: root_display,
    selected_index: picker.selected.map(|index| index as i64).unwrap_or(-1),
    window_start: clamped_start,
    items,
  }
}

impl App {
  fn shared_lsp_editor_state_summary(&self, editor_id: Option<LibEditorId>) -> String {
    let Some(editor_id) = editor_id else {
      return "editor=none".to_string();
    };
    let mode = self
      .states
      .get(&editor_id)
      .map(|state| {
        match state.mode {
          Mode::Normal => "normal",
          Mode::Insert => "insert",
          Mode::Select => "select",
          Mode::Command => "command",
        }
      })
      .unwrap_or("missing");
    let completion = self
      .states
      .get(&editor_id)
      .map(|state| &state.completion_menu);
    let signature = self
      .states
      .get(&editor_id)
      .map(|state| &state.signature_help);
    let hover_present = self
      .states
      .get(&editor_id)
      .and_then(|state| state.hover_docs.as_deref())
      .map(str::trim)
      .is_some_and(|text| !text.is_empty());
    let diagnostic_present = self
      .states
      .get(&editor_id)
      .and_then(|state| state.diagnostic_popup.as_ref())
      .is_some_and(|popup| !popup.markdown.trim().is_empty());
    let completion_active = completion.is_some_and(|state| state.active);
    let completion_items = completion.map(|state| state.items.len()).unwrap_or(0);
    let completion_selected = completion
      .and_then(|state| state.selected)
      .map(|index| index.to_string())
      .unwrap_or_else(|| "none".to_string());
    let signature_active = signature.is_some_and(|state| state.active);
    let signature_count = signature.map(|state| state.signatures.len()).unwrap_or(0);
    format!(
      "editor={} mode={} completion_active={} completion_items={} completion_selected={} \
       signature_active={} signature_items={} hover_present={} diagnostic_present={}",
      editor_id.get().get(),
      mode,
      completion_active as u8,
      completion_items,
      completion_selected,
      signature_active as u8,
      signature_count,
      hover_present as u8,
      diagnostic_present as u8
    )
  }

  pub fn new() -> Self {
    let dispatch = config_build_dispatch::<App>();
    let workspace_root = env::current_dir()
      .ok()
      .map(|path| the_loader::find_workspace_in(path).0)
      .unwrap_or_else(|| the_loader::find_workspace().0);
    let ui_theme_catalog = ThemeCatalog::load(Some(workspace_root.as_path()));
    let (ui_theme_name, ui_theme) = select_ui_theme(&ui_theme_catalog);
    let loader = match init_loader(&ui_theme) {
      Ok(loader) => Some(Arc::new(loader)),
      Err(error) => {
        eprintln!("Warning: syntax highlighting unavailable in FFI: {error}");
        None
      },
    };
    let lsp_runtime = LspRuntime::new(
      LspRuntimeConfig::new(workspace_root.clone())
        .with_restart_policy(true, Duration::from_millis(250))
        .with_restart_limits(6, Duration::from_secs(30))
        .with_request_policy(Duration::from_secs(8), 1),
    );
    let clipboard = Arc::new(RuntimeClipboardProvider::detect());
    set_docs_render_theme(&ui_theme);

    let mut app = Self {
      inner: LibApp::default(),
      workspace_root,
      dispatch,
      keymaps: config_build_keymaps(),
      command_registry: CommandRegistry::new(),
      states: HashMap::new(),
      vcs_provider: DiffProviderRegistry::default(),
      vcs_diff_handles: HashMap::new(),
      active_editor: None,
      should_quit: false,
      cursor_blink_generation: 0,
      registers: Registers::with_clipboard(clipboard),
      last_motion: None,
      lsp_server_name: None,
      lsp_runtime,
      lsp_ready: false,
      lsp_document: None,
      lsp_statusline: LspStatuslineState::off(Some("unavailable".into())),
      lsp_spinner_index: 0,
      lsp_spinner_last_tick: Instant::now(),
      vcs_ui: VcsUiState::default(),
      lsp_active_progress_tokens: HashSet::new(),
      lsp_watched_file: None,
      lsp_pending_requests: HashMap::new(),
      lsp_completion_items: Vec::new(),
      lsp_completion_raw_items: Vec::new(),
      lsp_completion_resolved: HashSet::new(),
      lsp_completion_visible: Vec::new(),
      lsp_completion_start: None,
      lsp_completion_generation: 0,
      lsp_code_action_items: Vec::new(),
      lsp_code_action_menu_active: false,
      lsp_pending_auto_completion: None,
      lsp_pending_auto_signature_help: None,
      lsp_pending_mouse_hover: None,
      lsp_hover_generation: 0,
      diagnostics: DiagnosticsState::default(),
      global_search: GlobalSearchState::default(),
      inline_diagnostic_lines: Vec::new(),
      eol_diagnostics: Vec::new(),
      diagnostic_underlines: Vec::new(),
      render_inline_diagnostic_lines: true,
      render_theme_generation: 0,
      ui_theme_catalog,
      ui_theme_name,
      ui_theme_base: ui_theme.clone(),
      ui_theme_preview_name: None,
      ui_theme,
      loader,
      native_tab_open_gateway_enabled: false,
      native_tab_open_requests: VecDeque::new(),
    };
    let _ = app.refresh_vcs_ui_state();
    app
  }

  fn apply_effective_theme(&mut self, theme: Theme) {
    if let Some(loader) = &self.loader {
      loader.set_scopes(theme.scopes().to_vec());
    }
    set_docs_render_theme(&theme);
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
    let editor_ids: Vec<_> = self.states.keys().copied().collect();
    for editor_id in editor_ids {
      let has_syntax = self
        .inner
        .editor(editor_id)
        .is_some_and(|editor| editor.document().syntax().is_some());
      if let Some(state) = self.states.get_mut(&editor_id) {
        state.clear_highlight_caches();
        if has_syntax {
          state.syntax_parse_highlight_state.mark_parsed();
        } else {
          state.syntax_parse_highlight_state.mark_cleared();
        }
        state.needs_render = true;
      }
    }
  }

  fn editor_render_styles_from_theme(&self) -> RenderStyles {
    let theme = &self.ui_theme;
    RenderStyles {
      selection:                  theme.try_get("ui.selection").unwrap_or_default(),
      cursor:                     theme.try_get("ui.cursor").unwrap_or_default(),
      active_cursor:              theme
        .try_get("ui.cursor.active")
        .or_else(|| theme.try_get("ui.cursor"))
        .unwrap_or_default(),
      cursor_kind:                LibCursorKind::Block,
      active_cursor_kind:         LibCursorKind::Block,
      non_block_cursor_uses_head: true,
      gutter:                     theme.try_get("ui.linenr").unwrap_or_default(),
      gutter_active:              theme
        .try_get("ui.linenr.selected")
        .or_else(|| theme.try_get("ui.linenr"))
        .unwrap_or_default(),
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
    self.states.insert(
      id,
      EditorState::new(self.loader.clone(), self.workspace_root.as_path()),
    );
    self.active_editor.get_or_insert(id);
    let _ = self.refresh_vcs_diff_base_for_editor(id);
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
        self.stop_lsp_runtime(Some("stopped"));
        self.global_search.deactivate();
      }
    }
    removed
  }

  pub fn set_viewport(&mut self, id: ffi::EditorId, viewport: ffi::Rect) -> bool {
    let _ = self.activate(id);
    let Some(editor) = self.editor_mut(id) else {
      return false;
    };
    let viewport = viewport.to_lib();
    editor.set_layout_viewport(viewport);
    editor.view_mut().viewport = viewport;
    true
  }

  pub fn set_scroll(&mut self, id: ffi::EditorId, scroll: ffi::Position) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let clamped = self.clamped_scroll_for_active_editor(scroll.to_lib());
    let Some(editor) = self.editor_mut(id) else {
      return false;
    };
    editor.view_mut().scroll = clamped;
    true
  }

  fn clamped_scroll_for_active_editor(&self, scroll: LibPosition) -> LibPosition {
    let editor = self.active_editor_ref();
    let view = editor.view();
    let doc = editor.document();
    let mut clamped = scroll;

    let mut text_fmt = self.text_format();
    let gutter_width = gutter_width_for_document(doc, view.viewport.width, self.gutter_config());
    text_fmt.viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1);
    if text_fmt.soft_wrap {
      clamped.col = 0;
    }

    let text = doc.text();
    let mut annotations = self.text_annotations();
    let text_slice = text.slice(..);
    let has_line_annotations = annotations.has_line_annotations();
    let eof_pos = if !text_fmt.soft_wrap && !has_line_annotations {
      LibPosition::new(text.len_lines().saturating_sub(1), 0)
    } else {
      visual_pos_at_char(text_slice, &text_fmt, &mut annotations, text.len_chars())
        .unwrap_or_else(|| LibPosition::new(0, 0))
    };
    // Zed-like page padding vertically: allow the last visual row to reach the
    // top edge of the viewport (up to one viewport of empty space after content).
    // Horizontally we clamp to the actual scrollable width (no page padding).
    clamped.row = clamped.row.min(eof_pos.row);

    if !text_fmt.soft_wrap && clamped.col != view.scroll.col {
      let max_col = Self::max_visual_col_for_text(text, &text_fmt, &mut annotations);
      let viewport_cols = usize::from(text_fmt.viewport_width.max(1));
      let max_scroll_col = max_col.saturating_sub(viewport_cols.saturating_sub(1));
      clamped.col = clamped.col.min(max_scroll_col);
    }
    clamped
  }

  fn set_active_editor_scroll_clamped(&mut self, scroll: LibPosition) -> bool {
    let clamped = self.clamped_scroll_for_active_editor(scroll);
    let view = self.active_editor_mut().view_mut();
    if view.scroll == clamped {
      return false;
    }
    view.scroll = clamped;
    true
  }

  fn max_visual_col_for_text<'a>(
    text: &'a Rope,
    text_fmt: &'a TextFormat,
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
      if let Some(pos) = visual_pos_at_char(text_slice, text_fmt, annotations, char_idx) {
        max_col = max_col.max(pos.col);
      }
    }
    max_col
  }

  fn open_file_in_new_buffer_for_native_tab(&mut self, path: &Path) -> std::io::Result<u64> {
    let content = std::fs::read_to_string(path)?;
    let viewport = self.active_editor_ref().view().viewport;
    let opened_index = {
      let editor = self.active_editor_mut();
      let view = ViewState::new(viewport, LibPosition::new(0, 0));
      editor.open_buffer_without_activation(
        Rope::from_str(&content),
        view,
        Some(path.to_path_buf()),
      )
    };

    {
      let editor = self.active_editor_mut();
      if let Some(doc) = editor.buffer_document_mut(opened_index) {
        doc.set_display_name(
          path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string()),
        );
        let _ = doc.mark_saved();
      }
    }

    Ok(
      self
        .active_editor_ref()
        .buffer_snapshot(opened_index)
        .map(|snapshot| snapshot.buffer_id)
        .unwrap_or(0),
    )
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

  pub fn open_file_path(&mut self, id: ffi::EditorId, path: &str) -> bool {
    if path.is_empty() {
      return false;
    }
    if self.activate(id).is_none() {
      return false;
    }
    <Self as DefaultContext>::open_file(self, Path::new(path)).is_ok()
  }

  pub fn open_file_path_in_new_tab(&mut self, id: ffi::EditorId, path: &str) -> bool {
    if path.is_empty() {
      return false;
    }
    if self.activate(id).is_none() {
      return false;
    }
    let normalized_path = normalize_path_for_open(Path::new(path));
    if let Some(existing_index) = self
      .active_editor_ref()
      .find_buffer_by_path(&normalized_path)
    {
      if let Some(buffer_id) = self
        .active_editor_ref()
        .buffer_snapshot(existing_index)
        .map(|snapshot| snapshot.buffer_id)
      {
        if self.native_tab_open_gateway_enabled {
          self
            .native_tab_open_requests
            .push_back(NativeTabOpenRequest::focus_existing(
              buffer_id,
              Some(normalized_path),
            ));
          self.request_render();
          return true;
        }
        return self.active_editor_mut().set_active_buffer(existing_index);
      }
    }
    let buffer_id = self
      .open_file_in_new_buffer_for_native_tab(&normalized_path)
      .unwrap_or(0);
    if buffer_id == 0 {
      return false;
    }
    if self.native_tab_open_gateway_enabled {
      self
        .native_tab_open_requests
        .push_back(NativeTabOpenRequest::open_new(
          buffer_id,
          Some(normalized_path),
        ));
    }
    self.request_render();
    true
  }

  pub fn open_untitled_buffer(&mut self, id: ffi::EditorId) -> u64 {
    if self.activate(id).is_none() {
      return 0;
    }

    self.lsp_close_current_document();
    self.clear_hover_state();
    self.clear_signature_help_state();

    let viewport = self.active_editor_ref().view().viewport;
    let opened_index = {
      let editor = self.active_editor_mut();
      let view = ViewState::new(viewport, LibPosition::new(0, 0));
      editor.open_buffer(Rope::new(), view, None)
    };
    let active_path = self
      .active_editor_ref()
      .active_file_path()
      .map(Path::to_path_buf);
    DefaultContext::set_file_path(self, active_path);
    self.request_render();

    self
      .active_editor_ref()
      .buffer_snapshot(opened_index)
      .map(|snapshot| snapshot.buffer_id)
      .unwrap_or(0)
  }

  pub fn open_untitled_buffer_in_new_tab(&mut self, id: ffi::EditorId) -> u64 {
    if self.activate(id).is_none() {
      return 0;
    }

    let viewport = self.active_editor_ref().view().viewport;
    let opened_index = {
      let editor = self.active_editor_mut();
      let view = ViewState::new(viewport, LibPosition::new(0, 0));
      editor.open_buffer_without_activation(Rope::new(), view, None)
    };
    let buffer_id = self
      .active_editor_ref()
      .buffer_snapshot(opened_index)
      .map(|snapshot| snapshot.buffer_id)
      .unwrap_or(0);
    if buffer_id == 0 {
      return 0;
    }

    if self.native_tab_open_gateway_enabled {
      self
        .native_tab_open_requests
        .push_back(NativeTabOpenRequest::open_new(buffer_id, None));
    }
    self.request_render();
    buffer_id
  }

  pub fn supports_embedded_terminal(&self) -> bool {
    <Self as DefaultContext>::supports_embedded_terminal(self)
  }

  pub fn open_terminal_in_active_pane(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let opened = <Self as DefaultContext>::open_terminal_in_active_pane(self);
    if opened {
      self.request_render();
    }
    opened
  }

  pub fn close_terminal_in_active_pane(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let closed = <Self as DefaultContext>::close_terminal_in_active_pane(self);
    if closed {
      self.request_render();
    }
    closed
  }

  pub fn hide_active_terminal_surface(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let hidden = self.active_editor_mut().hide_active_terminal_surface();
    if hidden {
      self.request_render();
    }
    hidden
  }

  pub fn execute_command_named(&mut self, id: ffi::EditorId, name: &str) -> bool {
    if self.activate(id).is_none() {
      return false;
    }

    let command_name = name.trim();
    if command_name.is_empty() {
      return false;
    }

    if command_name == "command_palette" {
      the_default::open_command_palette(self);
      return true;
    }

    let Some(command) = the_default::command_from_name(command_name) else {
      return false;
    };

    let dispatch = self.dispatch();
    the_default::handle_command(&*dispatch, self, command);
    let _ = self.ensure_cursor_visible(id);
    true
  }

  pub fn is_active_pane_terminal(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    <Self as DefaultContext>::is_active_pane_terminal(self)
  }

  pub fn active_file_path(&self, id: ffi::EditorId) -> String {
    self
      .editor(id)
      .and_then(|editor| editor.active_file_path())
      .map(|path| path.to_string_lossy().into_owned())
      .unwrap_or_default()
  }

  pub fn set_native_tab_open_gateway(&mut self, enabled: bool) {
    self.native_tab_open_gateway_enabled = enabled;
    if !enabled {
      self.native_tab_open_requests.clear();
    }
  }

  pub fn set_inline_diagnostic_rendering_enabled(&mut self, enabled: bool) {
    self.render_inline_diagnostic_lines = enabled;
    if !enabled {
      self.inline_diagnostic_lines.clear();
    }
  }

  pub fn take_native_tab_open_request_path(&mut self) -> String {
    self
      .native_tab_open_requests
      .pop_front()
      .and_then(|request| serde_json::to_string(&NativeTabOpenRequestJson::from(request)).ok())
      .unwrap_or_default()
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

  pub fn split_separator_count(&mut self, id: ffi::EditorId) -> usize {
    if self.activate(id).is_none() {
      return 0;
    }
    let editor = self.active_editor_ref();
    editor.pane_separators(editor.layout_viewport()).len()
  }

  pub fn split_separator_at(&mut self, id: ffi::EditorId, index: usize) -> SplitSeparator {
    if self.activate(id).is_none() {
      return SplitSeparator::empty();
    }
    let editor = self.active_editor_ref();
    editor
      .pane_separators(editor.layout_viewport())
      .get(index)
      .map(|separator| {
        SplitSeparator {
          split_id:   separator.split_id.get().get() as u64,
          axis:       split_axis_to_u8(separator.axis),
          line:       separator.line,
          span_start: separator.span_start,
          span_end:   separator.span_end,
        }
      })
      .unwrap_or_else(SplitSeparator::empty)
  }

  pub fn split_active_pane(&mut self, id: ffi::EditorId, axis: u8) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let Some(axis) = split_axis_from_u8(axis) else {
      return false;
    };
    let Some(editor_id) = self.active_editor else {
      return false;
    };
    let split = {
      let editor = self.active_editor_mut();
      editor.split_active_pane(axis)
    };
    if split && let Some(state) = self.states.get_mut(&editor_id) {
      state.needs_render = true;
    }
    split
  }

  pub fn jump_active_pane(&mut self, id: ffi::EditorId, direction: u8) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let Some(direction) = pane_direction_from_u8(direction) else {
      return false;
    };
    let Some(editor_id) = self.active_editor else {
      return false;
    };
    let jumped = {
      let editor = self.active_editor_mut();
      editor.jump_active_pane(direction)
    };
    if jumped && let Some(state) = self.states.get_mut(&editor_id) {
      state.needs_render = true;
    }
    jumped
  }

  pub fn move_pane(
    &mut self,
    id: ffi::EditorId,
    source_pane: u64,
    destination_pane: u64,
    direction: u8,
  ) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let Some(source_pane) = pane_id_from_u64(source_pane) else {
      return false;
    };
    let Some(destination_pane) = pane_id_from_u64(destination_pane) else {
      return false;
    };
    let Some(direction) = pane_direction_from_u8(direction) else {
      return false;
    };
    let Some(editor_id) = self.active_editor else {
      return false;
    };
    let moved = {
      let editor = self.active_editor_mut();
      editor.move_pane(source_pane, destination_pane, direction)
    };
    if moved && let Some(state) = self.states.get_mut(&editor_id) {
      state.needs_render = true;
    }
    moved
  }

  pub fn resize_split(&mut self, id: ffi::EditorId, split_id: u64, x: u16, y: u16) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let split_raw = usize::try_from(split_id).ok().and_then(NonZeroUsize::new);
    let Some(split_raw) = split_raw else {
      return false;
    };
    let Some(editor_id) = self.active_editor else {
      return false;
    };
    let resized = {
      let editor = self.active_editor_mut();
      editor.resize_split(SplitNodeId::new(split_raw), x, y)
    };
    if resized && let Some(state) = self.states.get_mut(&editor_id) {
      state.needs_render = true;
    }
    resized
  }

  pub fn terminal_surface_count(&mut self, id: ffi::EditorId) -> usize {
    if self.activate(id).is_none() {
      return 0;
    }
    self.active_editor_ref().terminal_surface_snapshots().len()
  }

  pub fn terminal_surface_at(
    &mut self,
    id: ffi::EditorId,
    index: usize,
  ) -> TerminalSurfaceSnapshot {
    if self.activate(id).is_none() {
      return TerminalSurfaceSnapshot::empty();
    }
    self
      .active_editor_ref()
      .terminal_surface_snapshots()
      .get(index)
      .copied()
      .map(TerminalSurfaceSnapshot::from_lib)
      .unwrap_or_else(TerminalSurfaceSnapshot::empty)
  }

  pub fn editor_surface_count(&mut self, id: ffi::EditorId) -> usize {
    if self.activate(id).is_none() {
      return 0;
    }
    self.active_editor_ref().editor_surface_snapshots().len()
  }

  pub fn editor_surface_at(&mut self, id: ffi::EditorId, index: usize) -> EditorSurfaceSnapshot {
    if self.activate(id).is_none() {
      return EditorSurfaceSnapshot::empty();
    }
    self
      .active_editor_ref()
      .editor_surface_snapshots()
      .get(index)
      .cloned()
      .map(EditorSurfaceSnapshot::from_lib)
      .unwrap_or_else(EditorSurfaceSnapshot::empty)
  }

  pub fn focus_terminal_surface(&mut self, id: ffi::EditorId, terminal_id: u64) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let Some(raw) = usize::try_from(terminal_id)
      .ok()
      .and_then(NonZeroUsize::new)
    else {
      return false;
    };
    self
      .active_editor_mut()
      .focus_terminal_surface(the_lib::editor::TerminalId::new(raw))
  }

  pub fn render_plan(&mut self, id: ffi::EditorId) -> RenderPlan {
    let started = Instant::now();
    if self.activate(id).is_none() {
      return RenderPlan::empty();
    }
    let _ = self.poll_background_active();

    let styles = self.editor_render_styles_from_theme();
    let plan = the_default::render_plan_with_styles(self, styles);
    let inline_diagnostic_lines = std::mem::take(&mut self.inline_diagnostic_lines);
    let eol_diagnostics = std::mem::take(&mut self.eol_diagnostics);
    let diagnostic_underlines = std::mem::take(&mut self.diagnostic_underlines);
    let elapsed = started.elapsed();
    if ffi_ui_profile_should_log(elapsed) {
      ffi_ui_profile_log(format!(
        "render_plan elapsed={}ms lines={} overlays={} inline_diag={} eol_diag={} underlines={}",
        elapsed.as_millis(),
        plan.lines.len(),
        plan.overlays.len(),
        inline_diagnostic_lines.len(),
        eol_diagnostics.len(),
        diagnostic_underlines.len()
      ));
    }
    RenderPlan {
      inner: plan,
      inline_diagnostic_lines,
      eol_diagnostics,
      diagnostic_underlines,
    }
  }

  pub fn frame_render_plan(&mut self, id: ffi::EditorId) -> RenderFramePlan {
    let started = Instant::now();
    if self.activate(id).is_none() {
      return RenderFramePlan::empty();
    }
    let _ = self.poll_background_active();

    let styles = self.editor_render_styles_from_theme();
    let frame = the_default::frame_render_plan_with_styles(self, styles);
    let inline_diagnostic_lines = std::mem::take(&mut self.inline_diagnostic_lines);
    let eol_diagnostics = std::mem::take(&mut self.eol_diagnostics);
    let diagnostic_underlines = std::mem::take(&mut self.diagnostic_underlines);
    let elapsed = started.elapsed();
    if ffi_ui_profile_should_log(elapsed) {
      ffi_ui_profile_log(format!(
        "frame_render_plan elapsed={}ms panes={} inline_diag={} eol_diag={} underlines={}",
        elapsed.as_millis(),
        frame.panes.len(),
        inline_diagnostic_lines.len(),
        eol_diagnostics.len(),
        diagnostic_underlines.len()
      ));
    }
    RenderFramePlan::from_lib(
      frame,
      inline_diagnostic_lines,
      eol_diagnostics,
      diagnostic_underlines,
    )
  }

  pub fn docs_popup_anchor(&mut self, id: ffi::EditorId) -> ffi::DocsPopupAnchor {
    if self.activate(id).is_none() {
      return ffi::DocsPopupAnchor::default();
    }
    if self.active_editor_ref().is_active_pane_terminal() {
      return ffi::DocsPopupAnchor::default();
    }

    let Some(hover_ui) = self.active_state_ref().hover_ui else {
      return ffi::DocsPopupAnchor::default();
    };
    let (
      mut text_fmt,
      gutter_config,
      inline_annotations,
      overlay_annotations,
      word_jump_inline_annotations,
      word_jump_overlay_annotations,
    ) = {
      let state = self.active_state_ref();
      (
        state.text_format.clone(),
        state.gutter_config.clone(),
        state.inline_annotations.clone(),
        state.overlay_annotations.clone(),
        state.word_jump_inline_annotations.clone(),
        state.word_jump_overlay_annotations.clone(),
      )
    };

    let editor = self.active_editor_ref();
    let view = editor.view();
    let doc = editor.document();
    let gutter_width = gutter_width_for_document(doc, view.viewport.width, &gutter_config);
    text_fmt.viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1);

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
      let _ = annotations.add_overlay(&word_jump_overlay_annotations, None);
    }

    let anchor_char = hover_ui.anchor_char.min(doc.text().len_chars());
    let Some(pos) = visual_pos_at_char(
      doc.text().slice(..),
      &text_fmt,
      &mut annotations,
      anchor_char,
    ) else {
      return ffi::DocsPopupAnchor::default();
    };

    let row_start = view.scroll.row;
    let row_end = row_start.saturating_add(view.viewport.height as usize);
    let col_start = view.scroll.col;
    let col_end = col_start.saturating_add(text_fmt.viewport_width.max(1) as usize);
    if pos.row < row_start || pos.row >= row_end || pos.col < col_start || pos.col >= col_end {
      return ffi::DocsPopupAnchor::default();
    }

    ffi::DocsPopupAnchor {
      has_value: true,
      pane_id:   self.active_editor_ref().active_pane_id().get().get() as u64,
      row:       (pos.row - row_start) as u16,
      col:       (pos.col - col_start) as u16,
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

    let mut tree = the_default::ui_tree(self);
    self.append_docs_popup_overlays(&mut tree);
    if shared_lsp_trace_enabled() {
      let overlay_ids = tree
        .overlays
        .iter()
        .filter_map(|node| {
          if let the_lib::render::UiNode::Panel(panel) = node {
            Some(panel.id.clone())
          } else {
            None
          }
        })
        .collect::<Vec<_>>()
        .join(",");
      log_shared_lsp_debug(
        "ui_tree_json",
        format!(
          "editor={} overlays=[{}] {}",
          shared_lsp_editor_id_label(self.active_editor),
          overlay_ids,
          self.shared_lsp_editor_state_summary(self.active_editor)
        ),
      );
    }
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

  pub fn buffer_tabs_snapshot_json(&mut self, id: ffi::EditorId) -> String {
    if self.activate(id).is_none() {
      return "null".to_string();
    }
    let snapshot = BufferTabsSnapshotJson::from_snapshot(buffer_tabs_snapshot(self), &self.vcs_ui);
    serde_json::to_string(&snapshot).unwrap_or_else(|_| "null".to_string())
  }

  pub fn activate_buffer_tab(&mut self, id: ffi::EditorId, buffer_index: usize) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    the_default::activate_buffer_tab(self, buffer_index)
  }

  pub fn close_buffer_tab(&mut self, id: ffi::EditorId, buffer_index: usize) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    the_default::close_buffer_tab(self, buffer_index)
  }

  pub fn close_buffer_by_id(&mut self, id: ffi::EditorId, buffer_id: u64) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let Some(buffer_index) = self.active_editor_ref().find_buffer_by_id(buffer_id) else {
      return false;
    };
    the_default::close_buffer_tab(self, buffer_index)
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
    let active_pane_id = self.active_editor_ref().active_pane_id();
    let previous_generation_state = self
      .active_state_ref()
      .frame_generation_state
      .pane_states
      .get(&active_pane_id)
      .cloned();

    let (
      mut text_fmt,
      gutter_config,
      diff_signs,
      inline_annotations,
      overlay_annotations,
      word_jump_inline_annotations,
      word_jump_overlay_annotations,
      hover_ui,
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
        state.hover_ui,
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
    let inline_diagnostics = if self.render_inline_diagnostic_lines {
      self.active_inline_diagnostics()
    } else {
      Vec::new()
    };
    let enable_cursor_line = self.active_state_ref().mode != Mode::Insert;
    let jump_label_style = self.ui_theme.find_highlight("ui.virtual.jump-label");
    let selection_match_style = self
      .ui_theme
      .try_get("ui.selection.match")
      .unwrap_or_else(|| LibStyle::default().bg(LibColor::Rgb(47, 63, 116)));
    let hover_highlight_style = self
      .ui_theme
      .try_get("ui.hover.highlight")
      .or_else(|| self.ui_theme.try_get("ui.cursor.match"))
      .or_else(|| self.ui_theme.try_get("ui.selection.match"))
      .unwrap_or_else(|| LibStyle::default().bg(LibColor::Rgb(47, 63, 116)));
    let enable_point_selection_match = self.active_state_ref().mode == Mode::Select;

    let raw_diagnostics = self
      .lsp_document
      .as_ref()
      .filter(|state| state.opened)
      .and_then(|state| self.diagnostics.document(&state.uri))
      .map(|doc| doc.diagnostics.clone())
      .unwrap_or_default();

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
      let cursor_line_idx = doc
        .selection()
        .ranges()
        .first()
        .map(|range| range.cursor_line(doc.text().slice(..)));
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

      let mut plan = if let (Some(loader), Some(syntax)) = (loader.as_deref(), doc.syntax()) {
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

      add_selection_match_highlights(
        &mut plan,
        doc,
        &text_fmt,
        &mut annotations,
        view,
        selection_match_style,
        SelectionMatchHighlightOptions {
          enable_point_cursor_match: enable_point_selection_match,
          ..SelectionMatchHighlightOptions::default()
        },
      );

      let hover_selections = hover_ui
        .map(|hover_ui| {
          compute_hover_highlight_selections(
            doc.text(),
            hover_ui,
            &plan,
            &text_fmt,
            &mut annotations,
            hover_highlight_style,
          )
        })
        .unwrap_or_default();
      if !hover_selections.is_empty() {
        plan.selections.splice(0..0, hover_selections);
      }

      // Compute diagnostic underlines after build_plan while annotations are still
      // alive.
      let mut underlines = compute_diagnostic_underlines(
        doc.text(),
        &raw_diagnostics,
        &plan,
        &text_fmt,
        &mut annotations,
      );
      let mut inline_lines = render_inline_diagnostics_for_viewport(
        doc.text().slice(..),
        &plan,
        &text_fmt,
        &mut annotations,
        &inline_diagnostics,
        cursor_line_idx,
        &inline_config,
      );
      dedupe_inline_diagnostic_lines(&mut inline_lines.lines);
      apply_row_insertions_to_underlines(&mut underlines, &plan, &inline_lines.row_insertions);
      apply_row_insertions(&mut plan, &inline_lines.row_insertions);

      (plan, underlines, inline_lines.lines)
    };
    apply_diagnostic_gutter_markers(&mut plan, &diagnostics_by_line, diagnostic_styles);
    apply_diff_gutter_markers(&mut plan, &diff_signs, diff_styles);

    let eol_diagnostics = compute_eol_diagnostics(&raw_diagnostics, &plan);
    let row_hashes =
      build_render_layer_row_hashes(&plan, &inline_lines, &eol_diagnostics, &underlines);
    let generation_state = finish_render_generations(
      &mut plan,
      previous_generation_state.as_ref(),
      self.render_theme_generation,
      row_hashes,
    );

    self.inline_diagnostic_lines = inline_lines;
    self.eol_diagnostics = eol_diagnostics;
    self.diagnostic_underlines = underlines;
    let state = self.active_state_mut();
    state.highlight_cache = highlight_cache;
    state
      .frame_generation_state
      .pane_states
      .insert(active_pane_id, generation_state);
    plan
  }

  fn build_inactive_pane_render_plan_with_styles_impl(
    &mut self,
    buffer_index: usize,
    styles: RenderStyles,
  ) -> the_lib::render::RenderPlan {
    let (mut text_fmt, gutter_config, allow_cache_refresh) = {
      let state = self.active_state_ref();
      (
        state.text_format.clone(),
        state.gutter_config.clone(),
        state
          .syntax_parse_highlight_state
          .allow_cache_refresh(&state.syntax_parse_lifecycle),
      )
    };
    let mut annotations = TextAnnotations::default();
    let mut local_highlight_cache = {
      let state = self.active_state_mut();
      state
        .inactive_highlight_caches
        .remove(&buffer_index)
        .unwrap_or_default()
    };
    let loader = self.loader.clone();

    let plan = {
      let editor = self.active_editor_mut();
      let Some(view) = editor.buffer_view(buffer_index) else {
        return the_lib::render::RenderPlan::default();
      };
      let Some((doc, cache)) = editor.document_and_cache_at_mut(buffer_index) else {
        return the_lib::render::RenderPlan::default();
      };
      let gutter_width = gutter_width_for_document(doc, view.viewport.width, &gutter_config);
      text_fmt.viewport_width = view.viewport.width.saturating_sub(gutter_width).max(1);

      if let (Some(loader), Some(syntax)) = (loader.as_deref(), doc.syntax()) {
        let line_range = view.scroll.row..(view.scroll.row + view.viewport.height as usize);
        let mut adapter = SyntaxHighlightAdapter::new(
          doc.text().slice(..),
          syntax,
          loader,
          &mut local_highlight_cache,
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
    drop(annotations);

    self
      .active_state_mut()
      .inactive_highlight_caches
      .insert(buffer_index, local_highlight_cache);
    plan
  }

  fn build_frame_render_plan_with_styles_impl(
    &mut self,
    styles: RenderStyles,
  ) -> the_lib::render::FrameRenderPlan {
    let previous_frame_generation_state = self.active_state_ref().frame_generation_state.clone();
    let (active_pane, panes) = {
      let editor = self.active_editor_ref();
      let viewport = editor.layout_viewport();
      (
        editor.active_pane_id(),
        editor.frame_pane_snapshots(viewport),
      )
    };
    if panes.is_empty() {
      self.inline_diagnostic_lines.clear();
      self.eol_diagnostics.clear();
      self.diagnostic_underlines.clear();
      self.active_state_mut().frame_generation_state = FrameGenerationState::default();
      return the_lib::render::FrameRenderPlan::empty();
    }

    {
      let editor = self.active_editor_mut();
      for pane in &panes {
        if let PaneContent::EditorBuffer { buffer_index } = pane.content {
          let _ = editor.set_buffer_viewport(buffer_index, pane.rect);
        }
      }
    }

    let mut pane_plans = Vec::with_capacity(panes.len());
    let mut pane_generation_states = BTreeMap::new();
    for pane in panes {
      let (pane_kind, terminal_id, plan) = match pane.content {
        PaneContent::EditorBuffer { buffer_index } => {
          let mut plan = if pane.is_active_pane {
            self.build_render_plan_with_styles_impl(styles)
          } else {
            self.build_inactive_pane_render_plan_with_styles_impl(buffer_index, styles)
          };
          let generation_state = if pane.is_active_pane {
            self
              .active_state_ref()
              .frame_generation_state
              .pane_states
              .get(&pane.pane_id)
              .cloned()
              .unwrap_or_else(|| RenderGenerationState {
                layout_generation: plan.layout_generation,
                text_generation: plan.text_generation,
                decoration_generation: plan.decoration_generation,
                cursor_generation: plan.cursor_generation,
                cursor_blink_generation: plan.cursor_blink_generation,
                scroll_generation: plan.scroll_generation,
                theme_generation: plan.theme_generation,
                text_rows: Vec::new(),
                decoration_rows: Vec::new(),
                cursor_rows: Vec::new(),
              })
          } else {
            let previous = previous_frame_generation_state.pane_states.get(&pane.pane_id);
            let row_hashes = build_render_layer_row_hashes(&plan, &[], &[], &[]);
            finish_render_generations(
              &mut plan,
              previous,
              self.render_theme_generation,
              row_hashes,
            )
          };
          pane_generation_states.insert(pane.pane_id, generation_state);
          (PaneContentKind::EditorBuffer, None, plan)
        },
        PaneContent::Terminal { terminal_id } => {
          pane_generation_states.insert(pane.pane_id, RenderGenerationState::default());
          (
            PaneContentKind::Terminal,
            Some(terminal_id),
            the_lib::render::RenderPlan::default(),
          )
        },
      };
      pane_plans.push(the_lib::render::PaneRenderPlan {
        pane_id: pane.pane_id,
        rect: pane.rect,
        pane_kind,
        terminal_id,
        plan,
      });
    }

    let mut frame = the_lib::render::FrameRenderPlan {
      active_pane,
      panes: pane_plans,
      frame_generation: 0,
      pane_structure_generation: 0,
      changed_pane_ids: Vec::new(),
      damage_is_full: false,
      damage_reason: the_lib::render::RenderDamageReason::None,
    };
    let next_frame_generation_state = finish_frame_generations(
      &mut frame,
      Some(&previous_frame_generation_state),
      pane_generation_states,
    );
    self.active_state_mut().frame_generation_state = next_frame_generation_state;
    frame
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

  pub fn theme_ui_style(&self, scope: &str) -> ffi::Style {
    self
      .ui_theme
      .try_get(scope)
      .map(ffi::Style::from)
      .unwrap_or_default()
  }

  pub fn theme_effective_name(&self) -> String {
    self
      .ui_theme_preview_name
      .as_deref()
      .unwrap_or(self.ui_theme_name.as_str())
      .to_string()
  }

  pub fn theme_ghostty_snapshot(&self) -> ffi::GhosttyThemeSnapshot {
    ghostty_theme_snapshot_from_theme(&self.ui_theme)
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

  pub fn command_palette_placeholder(&mut self, id: ffi::EditorId) -> String {
    if self.activate(id).is_none() {
      return "Execute a command…".to_string();
    }
    let palette = &self.active_state_ref().command_palette;
    match palette.source {
      CommandPaletteSource::ActionPalette => "Search commands…".to_string(),
      CommandPaletteSource::CommandLine => {
        if palette.prefiltered {
          "Open file…".to_string()
        } else {
          "Execute a command…".to_string()
        }
      },
    }
  }

  pub fn command_palette_is_file_mode(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let palette = &self.active_state_ref().command_palette;
    matches!(palette.source, CommandPaletteSource::CommandLine) && palette.prefiltered
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

  pub fn command_palette_filtered_emphasis(&mut self, id: ffi::EditorId, index: usize) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let state = self.active_state_ref();
    let filtered = command_palette_filtered_indices(&state.command_palette);
    filtered
      .get(index)
      .and_then(|idx| state.command_palette.items.get(*idx))
      .is_some_and(|item| item.emphasis)
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
    the_default::sync_command_palette_preview(self);
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

    let source = self.active_state_ref().command_palette.source;
    let is_prefiltered = self.active_state_ref().command_palette.prefiltered;
    if matches!(source, CommandPaletteSource::ActionPalette) {
      let action = self
        .active_state_ref()
        .command_palette
        .items
        .get(item_idx)
        .and_then(|item| item.action.clone());

      let Some(action) = action else {
        return false;
      };

      match action {
        CommandPaletteAction::StaticCommand(command) => {
          self.set_mode(Mode::Normal);
          self.command_prompt_mut().clear();
          let palette = self.command_palette_mut();
          palette.is_open = false;
          palette.source = CommandPaletteSource::CommandLine;
          palette.query.clear();
          palette.items.clear();
          palette.selected = None;
          palette.prefiltered = false;
          palette.max_results = usize::MAX;
          palette.scroll_offset = 0;
          palette.prompt_text = None;
          let dispatch = self.dispatch();
          dispatch.post_on_keypress(self, command);
          self.request_render();
          true
        },
        CommandPaletteAction::TypableCommand { name, args } => {
          let registry = self.command_registry_ref() as *const CommandRegistry<App>;
          let result = unsafe { (&*registry).execute(self, &name, &args, CommandEvent::Validate) };
          match result {
            Ok(()) => {
              self.set_mode(Mode::Normal);
              self.command_prompt_mut().clear();
              let palette = self.command_palette_mut();
              palette.is_open = false;
              palette.source = CommandPaletteSource::CommandLine;
              palette.query.clear();
              palette.items.clear();
              palette.selected = None;
              palette.prefiltered = false;
              palette.max_results = usize::MAX;
              palette.scroll_offset = 0;
              palette.prompt_text = None;
              self.request_render();
              true
            },
            Err(err) => {
              self.clear_ui_theme_preview_state();
              self.command_prompt_mut().error = Some(err.to_string());
              self.request_render();
              false
            },
          }
        },
      }
    } else if is_prefiltered {
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
          palette.source = CommandPaletteSource::CommandLine;
          palette.query.clear();
          palette.items.clear();
          palette.selected = None;
          palette.prefiltered = false;
          palette.max_results = usize::MAX;
          palette.scroll_offset = 0;
          palette.prompt_text = None;
          self.request_render();
          true
        },
        Err(err) => {
          self.clear_ui_theme_preview_state();
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
          palette.source = CommandPaletteSource::CommandLine;
          palette.query.clear();
          palette.items.clear();
          palette.selected = None;
          palette.prefiltered = false;
          palette.max_results = usize::MAX;
          palette.scroll_offset = 0;
          palette.prompt_text = None;
          self.request_render();
          true
        },
        Err(err) => {
          self.clear_ui_theme_preview_state();
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
    self.clear_ui_theme_preview_state();
    self.set_mode(Mode::Normal);
    self.command_prompt_mut().clear();
    let palette = self.command_palette_mut();
    palette.is_open = false;
    palette.source = CommandPaletteSource::CommandLine;
    palette.query.clear();
    palette.items.clear();
    palette.selected = None;
    palette.prefiltered = false;
    palette.max_results = usize::MAX;
    palette.scroll_offset = 0;
    palette.prompt_text = None;
    self.request_render();
    true
  }

  pub fn command_palette_set_query(&mut self, id: ffi::EditorId, query: &str) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    if matches!(
      self.active_state_ref().command_palette.source,
      CommandPaletteSource::ActionPalette
    ) {
      update_action_palette_for_input(self, query);
    } else {
      update_command_palette_for_input(self, query);
    }
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
      SearchPromptKind::SplitSelection => finalize_split_selection(self),
      SearchPromptKind::KeepSelections => finalize_keep_selections(self),
      SearchPromptKind::RemoveSelections => finalize_remove_selections(self),
      SearchPromptKind::RenameSymbol => finalize_rename_symbol(self),
      SearchPromptKind::ShellPipe => finalize_shell_pipe(self),
      SearchPromptKind::ShellPipeTo => finalize_shell_pipe_to(self),
      SearchPromptKind::ShellInsertOutput => finalize_shell_insert_output(self),
      SearchPromptKind::ShellAppendOutput => finalize_shell_append_output(self),
      SearchPromptKind::ShellKeepPipe => finalize_shell_keep_pipe(self),
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
    let should_notify_dynamic_query = {
      let picker = self.file_picker_mut();
      if !picker.active {
        return false;
      }
      set_file_picker_query_text(picker, query)
    };
    if should_notify_dynamic_query {
      <Self as DefaultContext>::file_picker_query_changed(self, query);
    }
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
    self.file_picker_window_snapshot(id, 0, max_items)
  }

  pub fn file_picker_window_snapshot(
    &mut self,
    id: ffi::EditorId,
    window_start: usize,
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
    let data = build_file_picker_snapshot_window(picker, window_start, max_items);
    let elapsed = started.elapsed();
    ffi_ui_profile_log(format!(
      "file_picker_window_snapshot start={} items={} elapsed={:.2}ms",
      data.window_start,
      data.items.len(),
      elapsed.as_secs_f64() * 1000.0
    ));
    data
  }

  pub fn file_tree_set_visible(&mut self, id: ffi::EditorId, visible: bool) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    self.active_state_mut().file_tree.set_visible(visible);
    self.request_render();
    true
  }

  pub fn file_tree_toggle(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    self.active_state_mut().file_tree.toggle_visible();
    self.request_render();
    true
  }

  pub fn file_tree_open_workspace_root(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let workspace_root = self.workspace_root.clone();
    self
      .active_state_mut()
      .file_tree
      .open_workspace_root(workspace_root.as_path());
    self.request_render();
    true
  }

  pub fn file_tree_open_current_buffer_directory(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let workspace_root = self.workspace_root.clone();
    let active_path = self
      .active_editor_ref()
      .active_file_path()
      .map(Path::to_path_buf);
    self
      .active_state_mut()
      .file_tree
      .open_current_buffer_directory(active_path.as_deref(), workspace_root.as_path());
    self.request_render();
    true
  }

  pub fn file_tree_set_expanded(&mut self, id: ffi::EditorId, path: &str, expanded: bool) -> bool {
    if path.is_empty() || self.activate(id).is_none() {
      return false;
    }
    let changed = self
      .active_state_mut()
      .file_tree
      .set_expanded(Path::new(path), expanded);
    if changed {
      self.request_render();
    }
    changed
  }

  pub fn file_tree_select_path(&mut self, id: ffi::EditorId, path: &str) -> bool {
    if path.is_empty() || self.activate(id).is_none() {
      return false;
    }
    let selected = self
      .active_state_mut()
      .file_tree
      .select_path(Path::new(path));
    if selected {
      self.request_render();
    }
    selected
  }

  pub fn file_tree_open_selected(&mut self, id: ffi::EditorId) -> bool {
    if self.activate(id).is_none() {
      return false;
    }
    let selected = self.active_state_mut().file_tree.open_selected();
    if let Some(path) = selected {
      return <Self as DefaultContext>::open_file(self, path.as_path()).is_ok();
    }
    self.request_render();
    true
  }

  pub fn file_tree_snapshot(
    &mut self,
    id: ffi::EditorId,
    max_nodes: usize,
  ) -> FileTreeSnapshotData {
    if self.activate(id).is_none() {
      return FileTreeSnapshotData::default();
    }
    let snapshot = self.active_state_mut().file_tree.snapshot(max_nodes);
    build_file_tree_snapshot_data(snapshot, max_nodes, &self.vcs_ui)
  }

  /// Direct FFI preview data — no JSON serialization.
  pub fn file_picker_preview(&mut self, id: ffi::EditorId) -> PreviewData {
    self.file_picker_preview_window(id, usize::MAX, 24, 24)
  }

  /// Windowed file-picker preview (offset + visible + overscan).
  pub fn file_picker_preview_window(
    &mut self,
    id: ffi::EditorId,
    offset: usize,
    visible_rows: usize,
    overscan: usize,
  ) -> PreviewData {
    if self.activate(id).is_none() {
      return PreviewData::default();
    }
    let picker = self.file_picker();
    if !picker.active {
      return PreviewData::default();
    }
    let effective_offset = if offset == usize::MAX {
      picker.preview_scroll
    } else {
      offset
    };
    build_preview_data(picker, effective_offset, visible_rows, overscan)
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
    if self.poll_lsp_mouse_hover() {
      changed = true;
    }
    if self.poll_active_syntax_parse_results() {
      changed = true;
    }
    if self.poll_global_search() {
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

  fn lsp_poll_events_for_active_transport(&mut self) -> Vec<LspEvent> {
    let mut events = Vec::new();
    while let Some(event) = self.lsp_runtime.try_recv_event() {
      events.push(event);
    }
    events
  }

  fn lsp_server_capabilities_snapshot(&self) -> Option<ServerCapabilitiesSnapshot> {
    let server_name = self.lsp_server_name.as_deref()?;
    self.lsp_runtime.server_capabilities(server_name)
  }

  fn lsp_has_configured_server(&self) -> bool {
    self.lsp_runtime.config().server().is_some()
  }

  fn lsp_send_request_raw(
    &mut self,
    method: &'static str,
    params: serde_json::Value,
  ) -> Result<u64, String> {
    self
      .lsp_runtime
      .send_request(method, Some(params))
      .map_err(|err| format!("failed to dispatch {method}: {err}"))
  }

  fn lsp_cancel_request_raw(&mut self, request_id: u64) -> Result<(), String> {
    self
      .lsp_runtime
      .cancel_request(request_id)
      .map_err(|err| format!("failed to cancel stale request {request_id}: {err}"))
  }

  fn lsp_send_notification_raw(
    &mut self,
    method: &'static str,
    params: serde_json::Value,
  ) -> Result<(), String> {
    self
      .lsp_runtime
      .send_notification(method, Some(params))
      .map_err(|err| format!("failed to send {method}: {err}"))
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
    log_shared_lsp_debug(
      "refresh_begin",
      format!(
        "file_path={} prev_ready={} prev_doc_opened={}",
        self
          .file_path()
          .map(|path| path.display().to_string())
          .unwrap_or_else(|| "<none>".to_string()),
        self.lsp_ready,
        self.lsp_document.as_ref().is_some_and(|doc| doc.opened)
      ),
    );

    let (config, configured) = self.lsp_runtime_config_for_active_file();
    log_shared_lsp_debug(
      "refresh_plan",
      format!(
        "configured={} server={} workspace={}",
        configured,
        config
          .server()
          .map(|server| server.name().to_string())
          .unwrap_or_else(|| "<none>".to_string()),
        config.workspace_root().display()
      ),
    );

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
    self.lsp_runtime = LspRuntime::new(config);
    self.lsp_server_name = self
      .lsp_runtime
      .config()
      .server()
      .map(|server| server.name().to_string());

    let active_path = self.file_path().map(Path::to_path_buf);
    self.lsp_document =
      active_path.and_then(|path| build_lsp_document_state(&path, self.loader.as_deref()));
    self.lsp_sync_watched_file_state();

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
      self.lsp_ready = false;
      self.set_lsp_status(LspStatusPhase::Off, Some("unavailable".into()));
    }
  }

  fn stop_lsp_runtime(&mut self, status_detail: Option<&str>) {
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
    if let Some(detail) = status_detail {
      self.set_lsp_status(LspStatusPhase::Off, Some(detail.into()));
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
    let has_server = self.lsp_has_configured_server();
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

  fn poll_lsp_mouse_hover(&mut self) -> bool {
    let Some(pending) = self.lsp_pending_mouse_hover else {
      return false;
    };
    if Instant::now() < pending.due_at {
      return false;
    }

    self.lsp_pending_mouse_hover = None;
    if self.active_editor != Some(pending.editor_id) {
      return false;
    }
    if self.active_editor_ref().active_pane_id() != pending.pane_id {
      return false;
    }

    self.start_hover_at_char(HoverTriggerSource::Mouse, pending.anchor_char, false, false)
  }

  fn poll_lsp_events(&mut self) -> bool {
    let mut changed = false;
    for event in self.lsp_poll_events_for_active_transport() {
      match event {
        LspEvent::Started { .. } => {
          if !self.lsp_has_configured_server() {
            self.set_lsp_status(LspStatusPhase::Off, Some("unavailable".into()));
          } else {
            self.set_lsp_status(LspStatusPhase::Starting, Some("starting".into()));
          }
          changed = true;
        },
        LspEvent::CapabilitiesRegistered { server_name } => {
          let matches_configured_server = self
            .lsp_server_name
            .as_deref()
            .is_some_and(|name| name == server_name);
          log_shared_lsp_debug(
            "event_capabilities_registered",
            format!(
              "server={} matches_configured={} configured_server={}",
              server_name,
              matches_configured_server,
              self
                .lsp_server_name
                .clone()
                .unwrap_or_else(|| "<none>".to_string())
            ),
          );
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
          if self.lsp_has_configured_server() {
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
          log_shared_lsp_debug(
            "event_diagnostics",
            format!(
              "uri={} active_uri={}",
              diagnostic_uri,
              self
                .lsp_document
                .as_ref()
                .map(|state| state.uri.clone())
                .unwrap_or_else(|| "<none>".to_string())
            ),
          );
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
        LspEvent::WorkspaceApplyEdit { label, edit } => {
          let source = label.as_deref().unwrap_or("code action");
          let _ = self.apply_workspace_edit(&edit, source);
          changed = true;
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
    log_shared_lsp_debug(
      "handle_response_begin",
      format!(
        "request_id={} kind={} active_editor={} request_uri={} active_uri={} {}",
        id,
        kind.label(),
        shared_lsp_editor_id_label(self.active_editor),
        kind.uri().unwrap_or("<none>"),
        self
          .lsp_document
          .as_ref()
          .map(|state| state.uri.as_str())
          .unwrap_or("<none>"),
        self.shared_lsp_editor_state_summary(self.active_editor)
      ),
    );

    if let Some(request_uri) = kind.uri()
      && self
        .lsp_document
        .as_ref()
        .map(|state| state.uri.as_str())
        .is_some_and(|uri| uri != request_uri)
    {
      log_shared_lsp_debug(
        "handle_response_skip",
        format!(
          "request_id={} kind={} reason=uri_mismatch request_uri={} active_uri={}",
          id,
          kind.label(),
          request_uri,
          self
            .lsp_document
            .as_ref()
            .map(|state| state.uri.as_str())
            .unwrap_or("<none>")
        ),
      );
      return false;
    }

    if let Some(error) = response.error {
      if let PendingLspRequestKind::Hover {
        generation,
        trigger,
        anchor_char,
        fallback_range,
        ..
      } = &kind
      {
        if *trigger == HoverTriggerSource::Mouse {
          return self.apply_hover_response(
            None,
            *generation,
            *trigger,
            *anchor_char,
            *fallback_range,
          );
        }
      }

      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Error,
        format!("lsp {} failed: {}", kind.label(), error.message),
      );
      return true;
    }

    match kind {
      PendingLspRequestKind::GotoDeclaration { .. } => {
        let locations = match parse_locations_response(response.result.as_ref()) {
          Ok(locations) => locations,
          Err(err) => {
            self.publish_lsp_message(
              the_lib::messages::MessageLevel::Error,
              format!("failed to parse goto-declaration response: {err}"),
            );
            return true;
          },
        };
        if locations.is_empty() {
          let _ = <Self as DefaultContext>::push_error(self, "goto", "No declaration found.");
          return true;
        }
        self.apply_locations_result("declaration", locations)
      },
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
        if locations.is_empty() {
          let _ = <Self as DefaultContext>::push_error(self, "goto", "No definition found.");
          return true;
        }
        self.apply_locations_result("definition", locations)
      },
      PendingLspRequestKind::GotoTypeDefinition { .. } => {
        let locations = match parse_locations_response(response.result.as_ref()) {
          Ok(locations) => locations,
          Err(err) => {
            self.publish_lsp_message(
              the_lib::messages::MessageLevel::Error,
              format!("failed to parse goto-type-definition response: {err}"),
            );
            return true;
          },
        };
        if locations.is_empty() {
          let _ = <Self as DefaultContext>::push_error(self, "goto", "No type definition found.");
          return true;
        }
        self.apply_locations_result("type definition", locations)
      },
      PendingLspRequestKind::GotoImplementation { .. } => {
        let locations = match parse_locations_response(response.result.as_ref()) {
          Ok(locations) => locations,
          Err(err) => {
            self.publish_lsp_message(
              the_lib::messages::MessageLevel::Error,
              format!("failed to parse goto-implementation response: {err}"),
            );
            return true;
          },
        };
        if locations.is_empty() {
          let _ = <Self as DefaultContext>::push_error(self, "goto", "No implementation found.");
          return true;
        }
        self.apply_locations_result("implementation", locations)
      },
      PendingLspRequestKind::Hover {
        generation,
        trigger,
        anchor_char,
        fallback_range,
        ..
      } => {
        let hover = match parse_hover_details_response(response.result.as_ref()) {
          Ok(hover) => hover,
          Err(err) => {
            if trigger == HoverTriggerSource::Mouse {
              return self.apply_hover_response(
                None,
                generation,
                trigger,
                anchor_char,
                fallback_range,
              );
            }
            self.publish_lsp_message(
              the_lib::messages::MessageLevel::Error,
              format!("failed to parse hover response: {err}"),
            );
            return true;
          },
        };
        self.apply_hover_response(
          Some(hover),
          generation,
          trigger,
          anchor_char,
          fallback_range,
        )
      },
      PendingLspRequestKind::DocumentHighlightSelect { .. } => {
        self.handle_document_highlight_selection_response(response.result.as_ref())
      },
      PendingLspRequestKind::DocumentSymbols { uri } => {
        let symbols = match parse_document_symbols_response(&uri, response.result.as_ref()) {
          Ok(symbols) => symbols,
          Err(err) => {
            self.publish_lsp_message(
              the_lib::messages::MessageLevel::Error,
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
            self.publish_lsp_message(
              the_lib::messages::MessageLevel::Error,
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
    }
  }

  fn handle_rename_response(&mut self, result: Option<&Value>) -> bool {
    let workspace_edit = match parse_workspace_edit_response(result) {
      Ok(edit) => edit,
      Err(err) => {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Error,
          format!("failed to parse rename response: {err}"),
        );
        return true;
      },
    };

    let Some(workspace_edit) = workspace_edit else {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Info,
        "rename produced no edits",
      );
      return true;
    };

    self.apply_workspace_edit(&workspace_edit, "rename")
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
      log_shared_lsp_debug(
        "completion_response_skip",
        format!(
          "reason=generation_mismatch response_generation={} current_generation={} \
           active_editor={}",
          generation,
          self.lsp_completion_generation,
          shared_lsp_editor_id_label(self.active_editor)
        ),
      );
      return false;
    }
    if self.active_state_ref().mode != Mode::Insert {
      log_shared_lsp_debug(
        "completion_response_skip",
        format!(
          "reason=mode active_editor={} mode={:?}",
          shared_lsp_editor_id_label(self.active_editor),
          self.active_state_ref().mode
        ),
      );
      return false;
    }
    let Some(current_cursor) = self.active_cursor_char_idx() else {
      log_shared_lsp_debug(
        "completion_response_skip",
        format!(
          "reason=no_cursor active_editor={}",
          shared_lsp_editor_id_label(self.active_editor)
        ),
      );
      return false;
    };
    if current_cursor != request_cursor {
      log_shared_lsp_debug(
        "completion_response_skip",
        format!(
          "reason=cursor_mismatch request_cursor={} current_cursor={} active_editor={}",
          request_cursor,
          current_cursor,
          shared_lsp_editor_id_label(self.active_editor)
        ),
      );
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
    log_shared_lsp_debug(
      "completion_state_set",
      format!(
        "active_editor={} total_items={} visible_items={} {}",
        shared_lsp_editor_id_label(self.active_editor),
        self.lsp_completion_items.len(),
        self.lsp_completion_visible.len(),
        self.shared_lsp_editor_state_summary(self.active_editor)
      ),
    );
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
    log_shared_lsp_debug(
      "signature_state_set",
      format!(
        "active_editor={} active_signature={} {}",
        shared_lsp_editor_id_label(self.active_editor),
        active_signature,
        self.shared_lsp_editor_state_summary(self.active_editor)
      ),
    );
    true
  }

  fn handle_code_actions_response(&mut self, result: Option<&Value>) -> bool {
    let actions = match parse_code_actions_response(result) {
      Ok(actions) => actions,
      Err(err) => {
        self.clear_code_action_menu_state();
        if self.active_editor.is_some() {
          self.completion_menu_mut().clear();
        }
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Error,
          format!("failed to parse code actions response: {err}"),
        );
        return true;
      },
    };

    if actions.is_empty() {
      self.clear_code_action_menu_state();
      if self.active_editor.is_some() {
        self.completion_menu_mut().clear();
      }
      let _ =
        <Self as DefaultContext>::push_error(self, "code actions", "No code actions available");
      return true;
    }

    self.show_code_action_menu(actions);
    true
  }

  fn handle_document_highlight_selection_response(&mut self, result: Option<&Value>) -> bool {
    let highlights = match parse_document_highlights_response(result) {
      Ok(highlights) => highlights,
      Err(err) => {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Error,
          format!("failed to parse document-highlight response: {err}"),
        );
        return true;
      },
    };

    if highlights.is_empty() {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Info,
        "no references under cursor",
      );
      return true;
    }

    let editor = self.active_editor_ref();
    let doc = editor.document();
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
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Info,
        "no references under cursor",
      );
      return true;
    }

    let next_selection = match Selection::new(ranges) {
      Ok(selection) => selection,
      Err(err) => {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Error,
          format!("failed to apply document highlights: {err}"),
        );
        return true;
      },
    };

    let next_active_cursor = next_selection
      .cursor_id_at(primary_index.min(next_selection.len().saturating_sub(1)))
      .ok();
    let _ = self
      .active_editor_mut()
      .document_mut()
      .set_selection(next_selection);
    self.active_editor_mut().view_mut().active_cursor = next_active_cursor;
    self.request_render();
    true
  }

  fn execute_lsp_command_action(&mut self, command: LspExecuteCommand, title: String) -> bool {
    self.lsp_open_current_document();
    let params = execute_command_params(&command.command, command.arguments);
    match self.lsp_send_request_raw("workspace/executeCommand", params) {
      Ok(_) => {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Info,
          format!("executed code action: {title}"),
        );
      },
      Err(err) => {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Error,
          format!("failed to execute code action '{title}': {err}"),
        );
      },
    }
    true
  }

  fn apply_workspace_edit(&mut self, workspace_edit: &LspWorkspaceEdit, source: &str) -> bool {
    if workspace_edit.documents.is_empty() {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Info,
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
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Info,
        format!("{source}: applied {applied_edits} edit(s) across {applied_documents} file(s)"),
      );
    } else {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
        format!("{source}: no edits were applied"),
      );
    }
    true
  }

  fn apply_text_edits_to_current_document(&mut self, edits: &[LspTextEdit]) -> bool {
    let tx = {
      let doc = self.active_editor_ref().document();
      match build_transaction_from_lsp_text_edits(doc.text(), edits) {
        Ok(tx) => tx,
        Err(err) => {
          self.publish_lsp_message(
            the_lib::messages::MessageLevel::Error,
            format!("failed to build edit transaction: {err}"),
          );
          return false;
        },
      }
    };

    if <Self as DefaultContext>::apply_transaction(self, &tx) {
      <Self as DefaultContext>::request_render(self);
      true
    } else {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Error,
        "failed to apply edit transaction",
      );
      false
    }
  }

  fn apply_text_edits_to_file_uri(&mut self, uri: &str, edits: &[LspTextEdit]) -> bool {
    let Some(path) = path_for_file_uri(uri) else {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
        format!("unsupported file URI in workspace edit: {uri}"),
      );
      return false;
    };

    let content = match std::fs::read_to_string(&path) {
      Ok(content) => content,
      Err(err) => {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Error,
          format!("failed to read '{}': {err}", path.display()),
        );
        return false;
      },
    };
    let mut rope = Rope::from(content.as_str());

    let tx = match build_transaction_from_lsp_text_edits(&rope, edits) {
      Ok(tx) => tx,
      Err(err) => {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Error,
          format!("failed to build workspace edit transaction: {err}"),
        );
        return false;
      },
    };

    if let Err(err) = tx.apply(&mut rope) {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Error,
        format!("failed to apply edits to '{}': {err}", path.display()),
      );
      return false;
    }

    if let Err(err) = std::fs::write(&path, rope.to_string()) {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Error,
        format!("failed to write '{}': {err}", path.display()),
      );
      return false;
    }
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

  fn apply_symbols_result(&mut self, label: &str, symbols: Vec<LspSymbol>) -> bool {
    if symbols.is_empty() {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Info,
        format!("no {label} found"),
      );
      return true;
    }

    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let root = the_default::workspace_root(&cwd);
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
          self.active_editor_ref().document().text(),
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
        .strip_prefix(&cwd)
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
      });
    }

    if items.is_empty() {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
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

  fn jump_to_location(&mut self, location: &LspLocation) -> bool {
    let Some(path) = path_for_file_uri(&location.uri) else {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
        format!("unsupported location URI: {}", location.uri),
      );
      return true;
    };

    // Match Helix behavior: record the origin before any goto jump so C-o can
    // return.
    let _ = <Self as DefaultContext>::save_selection_to_jumplist(self);

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

  fn active_or_first_selection_range(&self) -> Option<Range> {
    let editor = self.active_editor_ref();
    let doc = editor.document();
    let selection = doc.selection();
    if let Some(active_cursor) = editor.view().active_cursor
      && let Some(range) = selection.range_by_id(active_cursor)
    {
      return Some(*range);
    }
    selection.ranges().first().copied()
  }

  fn active_cursor_char_idx(&self) -> Option<usize> {
    let doc = self.active_editor_ref().document();
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
    let Some(capabilities) = self.lsp_server_capabilities_snapshot() else {
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
    let cursor = self.active_cursor_char_idx()?;
    let start = self.lsp_completion_start.unwrap_or(cursor).min(cursor);
    let text = self.active_editor_ref().document().text();
    Some(text.slice(start..cursor).to_string())
  }

  fn rebuild_completion_menu(&mut self) {
    let started = Instant::now();
    self.clear_code_action_menu_state();
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
    self.clear_code_action_menu_state();
    if self.active_editor.is_some() {
      self.completion_menu_mut().clear();
    }
  }

  fn code_action_menu_is_active(&self) -> bool {
    self.lsp_code_action_menu_active && self.completion_menu().active
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
    let menu_items = self
      .lsp_code_action_items
      .iter()
      .map(completion_menu_item_for_code_action)
      .collect();
    the_default::show_completion_menu(self, menu_items);
  }

  fn apply_code_action(&mut self, action: LspCodeAction) -> bool {
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
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Info,
        format!("code action '{title}' had no edits"),
      );
    }
    true
  }

  fn clear_signature_help_state(&mut self) {
    if self.active_editor.is_some() {
      self.active_state_mut().signature_help.clear();
    }
  }

  fn bump_hover_generation(&mut self) -> u64 {
    self.lsp_hover_generation = self.lsp_hover_generation.wrapping_add(1);
    if self.lsp_hover_generation == 0 {
      self.lsp_hover_generation = 1;
    }
    self.lsp_hover_generation
  }

  fn hover_request_pending_for_current_generation(&self) -> bool {
    self.lsp_pending_requests.values().any(|kind| {
      matches!(
        kind,
        PendingLspRequestKind::Hover { generation, .. }
          if *generation == self.lsp_hover_generation
      )
    })
  }

  fn prune_hover_ui_state(&mut self) {
    let Some(id) = self.active_editor else {
      return;
    };
    let keep = self.docs_popup_visible() || self.hover_request_pending_for_current_generation();
    if keep {
      return;
    }
    if let Some(state) = self.states.get_mut(&id) {
      state.hover_ui = None;
    }
  }

  fn clear_hover_state(&mut self) {
    self.bump_hover_generation();
    self.lsp_pending_mouse_hover = None;
    let Some(id) = self.active_editor else {
      return;
    };
    let Some(state) = self.states.get_mut(&id) else {
      return;
    };
    state.hover_docs = None;
    state.hover_docs_scroll = 0;
    state.diagnostic_popup = None;
    state.hover_ui = None;
  }

  fn clear_hover_docs_state(&mut self) {
    let Some(id) = self.active_editor else {
      return;
    };
    let Some(state) = self.states.get_mut(&id) else {
      return;
    };
    state.hover_docs = None;
    state.hover_docs_scroll = 0;
    self.prune_hover_ui_state();
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

  fn diagnostic_popup_state(&self) -> Option<&DiagnosticPopupState> {
    let id = self.active_editor?;
    self
      .states
      .get(&id)
      .and_then(|state| state.diagnostic_popup.as_ref())
      .filter(|popup| !popup.markdown.trim().is_empty())
  }

  fn diagnostic_popup_text(&self) -> Option<&str> {
    self
      .diagnostic_popup_state()
      .map(|state| state.markdown.as_str())
      .map(str::trim)
      .filter(|text| !text.is_empty())
  }

  fn docs_popup_visible(&self) -> bool {
    self.hover_docs_text().is_some() || self.diagnostic_popup_text().is_some()
  }

  fn hover_ui_contains_char(hover_ui: HoverUiState, target: usize) -> bool {
    if hover_ui.highlight_to > hover_ui.highlight_from {
      target >= hover_ui.highlight_from && target < hover_ui.highlight_to
    } else {
      target == hover_ui.anchor_char
    }
  }

  fn pending_mouse_hover_contains_char(pending: &PendingMouseHover, target: usize) -> bool {
    if pending.highlight_to > pending.highlight_from {
      target >= pending.highlight_from && target < pending.highlight_to
    } else {
      target == pending.anchor_char
    }
  }

  fn set_hover_ui_state(
    &mut self,
    trigger: HoverTriggerSource,
    anchor_char: usize,
    highlight_range: Option<(usize, usize)>,
  ) -> bool {
    let text_len = self.active_editor_ref().document().text().len_chars();
    let anchor_char = anchor_char.min(text_len);
    let (mut highlight_from, mut highlight_to) =
      highlight_range.unwrap_or((anchor_char, anchor_char));
    highlight_from = highlight_from.min(text_len);
    highlight_to = highlight_to.min(text_len);
    if highlight_to < highlight_from {
      std::mem::swap(&mut highlight_from, &mut highlight_to);
    }

    let next = HoverUiState {
      trigger,
      anchor_char,
      highlight_from,
      highlight_to,
    };
    let Some(id) = self.active_editor else {
      return false;
    };
    let Some(state) = self.states.get_mut(&id) else {
      return false;
    };
    let changed = state.hover_ui != Some(next);
    state.hover_ui = Some(next);
    changed
  }

  fn clear_mouse_hover_preview(&mut self) -> bool {
    self.lsp_pending_mouse_hover = None;
    let visible_mouse_hover = self
      .active_editor
      .and_then(|id| self.states.get(&id))
      .and_then(|state| state.hover_ui)
      .is_some_and(|hover_ui| hover_ui.trigger == HoverTriggerSource::Mouse);
    if visible_mouse_hover {
      self.clear_hover_state();
      return true;
    }
    false
  }

  fn diagnostics_for_lsp_position(
    &self,
    uri: &str,
    position: LspPosition,
    line_fallback: bool,
  ) -> Vec<Diagnostic> {
    let Some(document) = self.diagnostics.document(uri) else {
      return Vec::new();
    };

    let mut diagnostics: Vec<Diagnostic> = document
      .diagnostics
      .iter()
      .filter(|diagnostic| diagnostic_contains_lsp_position(diagnostic, position))
      .cloned()
      .collect();

    if diagnostics.is_empty() && line_fallback {
      diagnostics = document
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_touches_lsp_line(diagnostic, position.line))
        .cloned()
        .collect();
    }

    sort_diagnostics_for_popup(&mut diagnostics);
    diagnostics
  }

  fn update_diagnostic_popup_for_lsp_position(
    &mut self,
    uri: &str,
    position: LspPosition,
    line_fallback: bool,
  ) -> bool {
    let next_popup = build_diagnostic_popup_state(&self.diagnostics_for_lsp_position(
      uri,
      position,
      line_fallback,
    ));
    let Some(id) = self.active_editor else {
      return false;
    };
    let Some(state) = self.states.get_mut(&id) else {
      return false;
    };
    let changed = state.diagnostic_popup.as_ref() != next_popup.as_ref();
    state.diagnostic_popup = next_popup;
    changed
  }

  fn hover_preview_range_at_char(&self, target: usize) -> Option<(usize, usize)> {
    let (from, to) = self.pointer_word_range_at_char(target);
    if to <= from {
      return None;
    }
    let text = self.active_editor_ref().document().text();
    let slice = text.slice(from..to);
    if slice.chars().all(char::is_whitespace) {
      return None;
    }
    Some((from, to))
  }

  fn lsp_position_for_active_char(&self, char_idx: usize) -> Option<(String, LspPosition)> {
    if !self.lsp_ready {
      return None;
    }
    let state = self.lsp_document.as_ref()?.clone();
    if !state.opened {
      return None;
    }

    let doc = self.active_editor_ref().document();
    let cursor = char_idx.min(doc.text().len_chars());
    let (line, character) = char_idx_to_utf16_position(doc.text(), cursor);
    Some((state.uri, LspPosition { line, character }))
  }

  fn normalize_char_range(text: &Rope, mut from: usize, mut to: usize) -> Option<(usize, usize)> {
    let text_len = text.len_chars();
    from = from.min(text_len);
    to = to.min(text_len);
    if to < from {
      std::mem::swap(&mut from, &mut to);
    }
    if to == from {
      if from < text_len {
        to = from.saturating_add(1).min(text_len);
      } else if from > 0 {
        from = from.saturating_sub(1);
      } else {
        return None;
      }
    }
    Some((from, to))
  }

  fn char_range_from_lsp_range(&self, range: &the_lsp::LspRange) -> Option<(usize, usize)> {
    let text = self.active_editor_ref().document().text();
    let start = utf16_position_to_char_idx(text, range.start.line, range.start.character);
    let end = utf16_position_to_char_idx(text, range.end.line, range.end.character);
    Self::normalize_char_range(text, start, end)
  }

  fn start_hover_at_char(
    &mut self,
    trigger: HoverTriggerSource,
    anchor_char: usize,
    line_fallback_diagnostics: bool,
    announce_failures: bool,
  ) -> bool {
    let before_popup = self.docs_popup_visible();
    let before_hover_ui = self.active_state_ref().hover_ui;
    let fallback_range = self.hover_preview_range_at_char(anchor_char);
    let generation = self.bump_hover_generation();
    self.lsp_pending_mouse_hover = None;

    let Some(id) = self.active_editor else {
      return false;
    };
    if let Some(state) = self.states.get_mut(&id) {
      state.hover_docs = None;
      state.hover_docs_scroll = 0;
      state.diagnostic_popup = None;
    }
    let mut hover_ui_changed = false;

    let Some((uri, position)) = self.lsp_position_for_active_char(anchor_char) else {
      let has_diagnostic = self.diagnostic_popup_text().is_some();
      if !has_diagnostic {
        if let Some(state) = self.states.get_mut(&id) {
          state.hover_ui = None;
        }
        if announce_failures {
          self.publish_lsp_message(
            the_lib::messages::MessageLevel::Warning,
            "hover unavailable: no active LSP document",
          );
        }
      }
      let after_popup = self.docs_popup_visible();
      let after_hover_ui = self.active_state_ref().hover_ui;
      return before_popup != after_popup
        || before_hover_ui != after_hover_ui
        || hover_ui_changed
        || announce_failures;
    };

    let diagnostic_changed =
      self.update_diagnostic_popup_for_lsp_position(&uri, position, line_fallback_diagnostics);
    if self.diagnostic_popup_text().is_some() {
      hover_ui_changed = self.set_hover_ui_state(trigger, anchor_char, fallback_range);
    }

    if !self.lsp_supports(LspCapability::Hover) {
      if self.diagnostic_popup_text().is_none() {
        if let Some(state) = self.states.get_mut(&id) {
          state.hover_ui = None;
        }
        if announce_failures {
          self.publish_lsp_message(
            the_lib::messages::MessageLevel::Warning,
            "hover is not supported by the active server",
          );
        }
      }
      let after_popup = self.docs_popup_visible();
      let after_hover_ui = self.active_state_ref().hover_ui;
      return before_popup != after_popup
        || before_hover_ui != after_hover_ui
        || diagnostic_changed
        || hover_ui_changed
        || announce_failures;
    }

    self.dispatch_lsp_request(
      "textDocument/hover",
      hover_params(&uri, position),
      PendingLspRequestKind::Hover {
        uri,
        generation,
        trigger,
        anchor_char,
        fallback_range,
      },
    );

    let after_popup = self.docs_popup_visible();
    let after_hover_ui = self.active_state_ref().hover_ui;
    before_popup != after_popup
      || before_hover_ui != after_hover_ui
      || diagnostic_changed
      || hover_ui_changed
  }

  fn build_docs_popup_overlay(
    &self,
    panel_id: &str,
    text_id: &str,
    source: &str,
    role: &str,
    docs: &str,
  ) -> UiNode {
    let mut text = UiText::new(text_id, docs);
    text.source = Some(source.to_string());
    text.style = text.style.with_role(role);
    text.clip = false;

    let mut container =
      UiContainer::column(format!("{panel_id}_container"), 0, vec![UiNode::Text(text)]);
    container.style = container.style.with_role(role);

    let mut panel = UiPanel::new(
      panel_id,
      LayoutIntent::Custom(panel_id.to_string()),
      UiNode::Container(container),
    );
    panel.source = Some(source.to_string());
    panel.style = panel.style.with_role(role);
    panel.layer = UiLayer::Tooltip;
    panel.constraints = UiConstraints {
      min_width:  Some(30),
      max_width:  Some(100),
      min_height: None,
      max_height: Some(22),
      padding:    UiInsets {
        left:   1,
        right:  1,
        top:    1,
        bottom: 1,
      },
      align:      UiAlignPair {
        horizontal: UiAlign::Start,
        vertical:   UiAlign::End,
      },
    };

    UiNode::Panel(panel)
  }

  fn build_lsp_diagnostic_overlay(&self) -> Option<UiNode> {
    let popup = self.diagnostic_popup_state()?;
    Some(self.build_docs_popup_overlay(
      "lsp_diagnostic",
      "lsp_diagnostic_text",
      "diagnostic",
      diagnostic_popup_role(popup.severity),
      popup.markdown.trim(),
    ))
  }

  fn build_lsp_hover_overlay(&self) -> Option<UiNode> {
    let docs = self.hover_docs_text()?;
    Some(self.build_docs_popup_overlay("lsp_hover", "lsp_hover_text", "hover", "hover_docs", docs))
  }

  fn append_docs_popup_overlays(&self, tree: &mut the_lib::render::UiTree) {
    if let Some(node) = self.build_lsp_diagnostic_overlay() {
      tree.overlays.push(node);
    }
    if let Some(node) = self.build_lsp_hover_overlay() {
      tree.overlays.push(node);
    }
  }

  fn apply_hover_response(
    &mut self,
    hover: Option<LspHoverDetails>,
    generation: u64,
    trigger: HoverTriggerSource,
    anchor_char: usize,
    fallback_range: Option<(usize, usize)>,
  ) -> bool {
    if generation != self.lsp_hover_generation {
      return false;
    }

    let before_popup = self.docs_popup_visible();
    let before_hover_ui = self.active_state_ref().hover_ui;
    let hover_text = hover
      .as_ref()
      .and_then(|details| details.text.as_deref())
      .map(str::trim)
      .filter(|text| !text.is_empty())
      .map(ToOwned::to_owned);
    let next_range = hover
      .as_ref()
      .and_then(|details| details.range.as_ref())
      .and_then(|range| self.char_range_from_lsp_range(range))
      .or(fallback_range);

    match hover_text {
      Some(ref text) => {
        {
          let state = self.active_state_mut();
          state.hover_docs = Some(text.clone());
          state.hover_docs_scroll = 0;
        }
        let _ = self.set_hover_ui_state(trigger, anchor_char, next_range);
        log_shared_lsp_debug(
          "hover_state_set",
          format!(
            "active_editor={} hover_len={} {}",
            shared_lsp_editor_id_label(self.active_editor),
            text.len(),
            self.shared_lsp_editor_state_summary(self.active_editor)
          ),
        );
      },
      None => {
        self.clear_hover_docs_state();
        if self.diagnostic_popup_text().is_some() {
          let _ = self.set_hover_ui_state(trigger, anchor_char, next_range);
        } else if let Some(id) = self.active_editor
          && let Some(state) = self.states.get_mut(&id)
        {
          state.hover_ui = None;
        }
      },
    }

    let mut published_message = false;
    if hover_text.is_none()
      && self.diagnostic_popup_text().is_none()
      && trigger == HoverTriggerSource::Keyboard
    {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Info,
        "no hover information",
      );
      published_message = true;
    }

    let after_popup = self.docs_popup_visible();
    let after_hover_ui = self.active_state_ref().hover_ui;
    before_popup != after_popup || before_hover_ui != after_hover_ui || published_message
  }

  fn dispatch_signature_help_request(
    &mut self,
    trigger: SignatureHelpTriggerSource,
    announce_failures: bool,
  ) -> bool {
    log_shared_lsp_debug(
      "signature_help_begin",
      format!(
        "trigger={:?} announce_failures={} ready={} doc_opened={} uri={}",
        trigger,
        announce_failures,
        self.lsp_ready,
        self.lsp_document.as_ref().is_some_and(|doc| doc.opened),
        self
          .lsp_document
          .as_ref()
          .map(|doc| doc.uri.clone())
          .unwrap_or_else(|| "<none>".to_string())
      ),
    );
    if !self.lsp_supports(LspCapability::SignatureHelp) {
      log_shared_lsp_debug("signature_help_skip", "reason=unsupported");
      if announce_failures {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Warning,
          "signature help is not supported by the active server",
        );
      }
      return false;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      log_shared_lsp_debug("signature_help_skip", "reason=no_position");
      if announce_failures {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Warning,
          "signature help unavailable: no active LSP document",
        );
      }
      return false;
    };

    let context = trigger.to_lsp_context();
    log_shared_lsp_debug(
      "signature_help_dispatch",
      format!(
        "uri={} line={} char={}",
        uri, position.line, position.character
      ),
    );
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
    log_shared_lsp_debug(
      "completion_begin",
      format!(
        "trigger={:?} announce_empty={} ready={} doc_opened={} uri={}",
        trigger,
        announce_empty,
        self.lsp_ready,
        self.lsp_document.as_ref().is_some_and(|doc| doc.opened),
        self
          .lsp_document
          .as_ref()
          .map(|doc| doc.uri.clone())
          .unwrap_or_else(|| "<none>".to_string())
      ),
    );
    if !self.lsp_supports(LspCapability::Completion) {
      log_shared_lsp_debug("completion_skip", "reason=unsupported");
      if matches!(trigger, CompletionTriggerSource::Manual) {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Warning,
          "completion is not supported by the active server",
        );
      }
      return false;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      log_shared_lsp_debug("completion_skip", "reason=no_position");
      if matches!(trigger, CompletionTriggerSource::Manual) {
        self.publish_lsp_message(
          the_lib::messages::MessageLevel::Warning,
          "completion unavailable: no active LSP document",
        );
      }
      return false;
    };
    let Some(cursor) = self.active_cursor_char_idx() else {
      log_shared_lsp_debug("completion_skip", "reason=no_cursor");
      return false;
    };
    let replace_start = self.completion_replace_start_at_cursor(cursor);

    self.lsp_completion_generation = self.lsp_completion_generation.wrapping_add(1);
    let generation = self.lsp_completion_generation;
    let context = trigger.to_lsp_context();
    log_shared_lsp_debug(
      "completion_dispatch",
      format!(
        "uri={} line={} char={} cursor={} replace_start={} generation={}",
        uri, position.line, position.character, cursor, replace_start, generation
      ),
    );
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
      | Command::PageCursorHalfUp
      | Command::PageCursorHalfDown
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
    match self.lsp_send_request_raw("completionItem/resolve", params) {
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
    let supported = self
      .lsp_server_capabilities_snapshot()
      .is_some_and(|capabilities| capabilities.supports(capability));
    if !supported && shared_lsp_trace_enabled() {
      log_shared_lsp_debug(
        "supports_false",
        format!(
          "capability={:?} ready={} server={} has_caps={} doc_opened={}",
          capability,
          self.lsp_ready,
          self
            .lsp_server_name
            .clone()
            .unwrap_or_else(|| "<none>".to_string()),
          self.lsp_server_capabilities_snapshot().is_some(),
          self.lsp_document.as_ref().is_some_and(|state| state.opened)
        ),
      );
    }
    supported
  }

  fn current_lsp_position(&self) -> Option<(String, LspPosition)> {
    if !self.lsp_ready {
      log_shared_lsp_debug("current_pos_none", "reason=not_ready");
      return None;
    }
    let state = self.lsp_document.as_ref()?.clone();
    if !state.opened {
      log_shared_lsp_debug(
        "current_pos_none",
        format!("reason=doc_not_opened uri={}", state.uri),
      );
      return None;
    }

    let doc = self.active_editor_ref().document();
    let range = self.active_or_first_selection_range()?;
    let cursor = range.cursor(doc.text().slice(..));
    let (line, character) = char_idx_to_utf16_position(doc.text(), cursor);

    Some((state.uri, LspPosition { line, character }))
  }

  fn current_lsp_code_action_range(&self) -> Option<(String, the_lsp::LspRange)> {
    if !self.lsp_ready {
      return None;
    }
    let state = self.lsp_document.as_ref()?.clone();
    if !state.opened {
      return None;
    }

    let doc = self.active_editor_ref().document();
    let range = self.active_or_first_selection_range()?;
    let mut start = range.anchor.min(range.head);
    let mut end = range.anchor.max(range.head);

    // Helix requests code actions on a non-empty range in normal mode.
    // Expand point selections so refactors appear consistently.
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

  fn current_lsp_diagnostics_payload(
    &self,
    uri: &str,
    selection_range: &the_lsp::LspRange,
  ) -> Value {
    let Some(document_diagnostics) = self.diagnostics.document(uri) else {
      return serde_json::json!([]);
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

  fn workspace_symbol_query_from_cursor(&self) -> String {
    let doc = self.active_editor_ref().document();
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

  fn start_global_search(&mut self) {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let root = the_default::workspace_root(&cwd);
    if !root.exists() {
      let _ = <Self as DefaultContext>::push_error(
        self,
        "global_search",
        "Current working directory does not exist",
      );
      return;
    }

    let config = GlobalSearchConfig {
      smart_case:  true,
      file_picker: self.file_picker().config.clone(),
    };
    if let Err(err) = self.global_search.activate(root.as_path(), config) {
      let _ = <Self as DefaultContext>::push_error(
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
      self.request_render();
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

  fn poll_global_search(&mut self) -> bool {
    if !self.global_search.is_active() {
      return false;
    }
    if !self.file_picker().active {
      self.global_search.deactivate();
      return false;
    }

    let Some(response) = self.global_search.poll_latest() else {
      return false;
    };

    let has_items = !response.items.is_empty();
    replace_file_picker_items(self, response.items, 0);
    {
      let picker = self.file_picker_mut();
      picker.query = response.query.clone();
      picker.cursor = response.query.len();
      if let Some(error) = response.error {
        picker.error = Some(error.clone());
        picker.preview = FilePickerPreview::Message(error);
      } else if response.indexing && !has_items {
        picker.error = None;
        picker.preview = FilePickerPreview::Message("Indexing files…".to_string());
      } else {
        picker.error = None;
        if picker.query.trim().is_empty() {
          picker.preview = FilePickerPreview::Message("Type to search".to_string());
        }
      }
    }

    self.request_render();
    true
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
      if let Err(err) = self.lsp_cancel_request_raw(id) {
        self.publish_lsp_message(the_lib::messages::MessageLevel::Warning, err);
      }
    }
  }

  fn dispatch_lsp_request(
    &mut self,
    method: &'static str,
    params: serde_json::Value,
    pending: PendingLspRequestKind,
  ) {
    let request_editor = self.active_editor;
    log_shared_lsp_debug(
      "dispatch_request",
      format!(
        "method={} pending_kind={} request_editor={} ready={} doc_opened={} uri={} {}",
        method,
        pending.label(),
        shared_lsp_editor_id_label(request_editor),
        self.lsp_ready,
        self.lsp_document.as_ref().is_some_and(|doc| doc.opened),
        self
          .lsp_document
          .as_ref()
          .map(|doc| doc.uri.clone())
          .unwrap_or_else(|| "<none>".to_string()),
        self.shared_lsp_editor_state_summary(request_editor)
      ),
    );
    self.lsp_open_current_document();
    self.cancel_pending_lsp_requests_for(&pending);
    match self.lsp_send_request_raw(method, params) {
      Ok(request_id) => {
        log_shared_lsp_debug(
          "dispatch_request_insert",
          format!(
            "request_id={} pending_kind={} request_editor={}",
            request_id,
            pending.label(),
            shared_lsp_editor_id_label(request_editor)
          ),
        );
        self.lsp_pending_requests.insert(request_id, pending);
      },
      Err(err) => {
        self.publish_lsp_message(the_lib::messages::MessageLevel::Error, err);
      },
    }
  }

  fn lsp_sync_kind(&self) -> Option<the_lsp::TextDocumentSyncKind> {
    self
      .lsp_server_capabilities_snapshot()
      .map(|capabilities| capabilities.text_document_sync().kind)
  }

  fn lsp_save_include_text(&self) -> bool {
    self
      .lsp_server_capabilities_snapshot()
      .is_some_and(|capabilities| capabilities.text_document_sync().save_include_text)
  }

  fn lsp_open_current_document(&mut self) {
    if !self.lsp_ready {
      log_shared_lsp_debug("open_doc_skip", "reason=not_ready");
      return;
    }

    let Some(state) = self.lsp_document.as_ref() else {
      log_shared_lsp_debug("open_doc_skip", "reason=no_document");
      return;
    };
    let uri = state.uri.clone();
    if state.opened {
      log_shared_lsp_debug("open_doc_skip", format!("reason=already_open uri={}", uri));
      return;
    }

    let language_id = state.language_id.clone();
    let version = state.version;
    let text = self.active_editor_ref().document().text().clone();
    let params = did_open_params(&uri, &language_id, version, &text);
    let opened = self
      .lsp_send_notification_raw("textDocument/didOpen", params)
      .is_ok();

    if opened && let Some(state) = self.lsp_document.as_mut() {
      state.opened = true;
      log_shared_lsp_debug(
        "open_doc_done",
        format!("uri={} state_opened={}", state.uri, state.opened),
      );
    } else {
      log_shared_lsp_debug("open_doc_done", format!("uri={} opened=false", uri));
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
    log_shared_lsp_debug("close_doc_begin", format!("uri={}", uri));

    let params = did_close_params(&uri);
    let _ = self.lsp_send_notification_raw("textDocument/didClose", params);
    if let Some(state) = self.lsp_document.as_mut() {
      state.opened = false;
      log_shared_lsp_debug("close_doc_done", format!("uri={} state_opened=false", uri));
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

    let changed = self
      .lsp_send_notification_raw("textDocument/didChange", params)
      .is_ok();

    if changed && let Some(state) = self.lsp_document.as_mut() {
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
    let _ = self.lsp_send_notification_raw("textDocument/didSave", params);
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
      let _ = self.lsp_send_notification_raw("workspace/didChangeWatchedFiles", params);
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

    let before_selection = self.active_editor_ref().document().selection().clone();
    let before_active_pane = self.active_editor_ref().active_pane_id();

    let event_kind = event.kind;
    let event_codepoint = event.codepoint;
    if event_kind == 3 && self.docs_popup_visible() && !self.completion_menu().active {
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
    let cursor_changed = self.active_editor_ref().active_pane_id() != before_active_pane
      || self.active_editor_ref().document().selection() != &before_selection;
    let hover_cleared = if cursor_changed {
      self.clear_mouse_hover_preview()
    } else {
      false
    };
    self.ensure_cursor_visible(id);
    if hover_cleared {
      self.request_render();
    }
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

  pub fn handle_mouse(
    &mut self,
    id: ffi::EditorId,
    packed: u64,
    logical_col: u16,
    logical_row: u16,
    surface_id: u64,
  ) -> bool {
    if self.activate(id).is_none() {
      return false;
    }

    let Some(pointer_event) = pointer_event_from_ffi_parts(
      (packed & 0xFF) as u8,
      ((packed >> 8) & 0xFF) as u8,
      logical_col,
      logical_row,
      ((packed >> 16) & 0xFF) as u8,
      ((packed >> 24) & 0xFF) as u8,
      surface_id,
    ) else {
      return false;
    };

    let dispatch = self.dispatch();
    the_default::handle_pointer_event(&*dispatch, self, pointer_event).handled()
  }

  fn handle_editor_pointer_event(
    &mut self,
    event: the_default::PointerEvent,
  ) -> the_default::PointerEventOutcome {
    use the_default::{
      PointerButton,
      PointerEventOutcome,
      PointerKind,
    };

    if event.kind == PointerKind::Move {
      let Some(surface_id) = event.surface_id else {
        if self.clear_mouse_hover_preview() {
          self.request_render();
          return PointerEventOutcome::Handled;
        }
        return PointerEventOutcome::Continue;
      };
      let Some(target_pane) = pane_id_from_u64(surface_id) else {
        if self.clear_mouse_hover_preview() {
          self.request_render();
          return PointerEventOutcome::Handled;
        }
        return PointerEventOutcome::Continue;
      };
      if self.active_editor_ref().active_pane_id() != target_pane
        || self.active_editor_ref().is_active_pane_terminal()
      {
        if self.clear_mouse_hover_preview() {
          self.request_render();
          return PointerEventOutcome::Handled;
        }
        return PointerEventOutcome::Continue;
      }

      let Some(target) = self.pointer_char_idx_for_event(event) else {
        if self.clear_mouse_hover_preview() {
          self.request_render();
          return PointerEventOutcome::Handled;
        }
        return PointerEventOutcome::Continue;
      };
      let highlight_range = self.hover_preview_range_at_char(target);
      let has_diagnostic = self
        .lsp_position_for_active_char(target)
        .map(|(uri, position)| {
          !self
            .diagnostics_for_lsp_position(&uri, position, false)
            .is_empty()
        })
        .unwrap_or(false);

      if highlight_range.is_none() && !has_diagnostic {
        if self.clear_mouse_hover_preview() {
          self.request_render();
          return PointerEventOutcome::Handled;
        }
        return PointerEventOutcome::Continue;
      }

      if self.active_state_ref().hover_ui.is_some_and(|hover_ui| {
        hover_ui.trigger == HoverTriggerSource::Mouse
          && Self::hover_ui_contains_char(hover_ui, target)
      }) {
        self.lsp_pending_mouse_hover = None;
        return PointerEventOutcome::Continue;
      }

      if self
        .lsp_pending_mouse_hover
        .as_ref()
        .is_some_and(|pending| {
          pending.editor_id == self.active_editor.expect("active editor must exist")
            && pending.pane_id == target_pane
            && Self::pending_mouse_hover_contains_char(pending, target)
        })
      {
        return PointerEventOutcome::Continue;
      }

      let mut changed = false;
      if self
        .active_state_ref()
        .hover_ui
        .is_some_and(|hover_ui| hover_ui.trigger == HoverTriggerSource::Mouse)
      {
        self.clear_hover_state();
        changed = true;
      } else {
        self.lsp_pending_mouse_hover = None;
      }

      self.lsp_pending_mouse_hover = Some(PendingMouseHover {
        editor_id:      self.active_editor.expect("active editor must exist"),
        pane_id:        target_pane,
        anchor_char:    target,
        highlight_from: highlight_range.map(|range| range.0).unwrap_or(target),
        highlight_to:   highlight_range.map(|range| range.1).unwrap_or(target),
        due_at:         Instant::now() + lsp_hover_auto_trigger_latency(),
      });
      if changed {
        self.request_render();
        return PointerEventOutcome::Handled;
      }
      return PointerEventOutcome::Continue;
    }

    let mut pane_changed = false;
    if let Some(surface_id) = event.surface_id {
      pane_changed = self.set_active_pane_from_pointer_surface(surface_id);
    }

    // Terminal panes own pointer semantics in the Swift Ghostty host. The Rust
    // editor pointer path should only update active-pane focus for terminal
    // surfaces and must not attempt text selection/drag behavior.
    if self.active_editor_ref().is_active_pane_terminal() {
      if pane_changed {
        self.request_render();
      }
      return PointerEventOutcome::Handled;
    }

    match event.kind {
      PointerKind::Scroll => {
        let row_delta = event.scroll_y.trunc() as i32;
        let col_delta = event.scroll_x.trunc() as i32;
        if row_delta == 0 && col_delta == 0 {
          if pane_changed {
            self.request_render();
          }
          return PointerEventOutcome::Handled;
        }

        let soft_wrap = self.active_state_ref().text_format.soft_wrap;
        let current = self.active_editor_ref().view().scroll;
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
        let changed = self.set_active_editor_scroll_clamped(LibPosition::new(new_row, new_col));
        let hover_cleared = self.clear_mouse_hover_preview();

        if changed || pane_changed || hover_cleared {
          self.request_render();
        }
        return PointerEventOutcome::Handled;
      },
      PointerKind::Down(PointerButton::Left) => {
        self.active_state_mut().pointer_drag_selection = None;
        let Some(target) = self.pointer_char_idx_for_event(event) else {
          if pane_changed {
            self.request_render();
          }
          return PointerEventOutcome::Handled;
        };
        let click_mode = Self::pointer_drag_mode_for_click_count(event.click_count.max(1));
        let drag_state =
          self.pointer_selection_drag_state_for_target(click_mode, target, event.modifiers.shift());
        self.active_state_mut().pointer_drag_selection = Some(drag_state);
        let changed = match click_mode {
          PointerSelectionDragMode::Char if !event.modifiers.shift() => {
            self.pointer_set_primary_range(Range::point(target))
          },
          _ => self.pointer_apply_drag_selection(drag_state, target),
        };
        self.clear_hover_state();
        if changed || pane_changed {
          self.request_render();
        }
        return PointerEventOutcome::Handled;
      },
      PointerKind::Drag(PointerButton::Left) => {
        let Some(target) = self.pointer_char_idx_for_event(event) else {
          return PointerEventOutcome::Handled;
        };
        let drag_state = self
          .active_state_ref()
          .pointer_drag_selection
          .unwrap_or_else(|| {
            self.pointer_selection_drag_state_for_target(
              PointerSelectionDragMode::Char,
              target,
              false,
            )
          });
        if self.active_state_ref().pointer_drag_selection.is_none() {
          self.active_state_mut().pointer_drag_selection = Some(drag_state);
        }
        let changed = self.pointer_apply_drag_selection(drag_state, target);
        if changed {
          self.clear_hover_state();
          self.request_render();
        }
        return PointerEventOutcome::Handled;
      },
      PointerKind::Up(PointerButton::Left) => {
        self.active_state_mut().pointer_drag_selection = None;
        if pane_changed {
          self.request_render();
        }
        return PointerEventOutcome::Handled;
      },
      _ => {},
    }

    PointerEventOutcome::Continue
  }

  fn set_active_pane_from_pointer_surface(&mut self, surface_id: u64) -> bool {
    let Ok(raw) = usize::try_from(surface_id) else {
      return false;
    };
    let Some(raw) = NonZeroUsize::new(raw) else {
      return false;
    };
    let pane = PaneId::from(raw);
    if self.active_editor_ref().active_pane_id() == pane {
      return false;
    }
    self.active_editor_mut().set_active_pane(pane)
  }

  fn pointer_char_idx_for_event(&self, event: the_default::PointerEvent) -> Option<usize> {
    let logical_row = usize::from(event.logical_row?);
    let logical_col = usize::from(event.logical_col?);

    let editor = self.active_editor_ref();
    let view = editor.view();
    let target = LibPosition::new(
      view.scroll.row.saturating_add(logical_row),
      view.scroll.col.saturating_add(logical_col),
    );

    let text = editor.document().text();
    let text_fmt = self.text_format();
    let mut annotations = self.text_annotations();
    char_at_visual_pos(text.slice(..), &text_fmt, &mut annotations, target)
  }

  fn sync_insert_mouse_selection_edit_arm(&mut self) {
    let armed = self.active_state_ref().mode == Mode::Insert
      && self
        .active_editor_ref()
        .document()
        .selection()
        .ranges()
        .iter()
        .any(|range| !range.is_empty());
    self.active_state_mut().insert_mouse_selection_edit_armed = armed;
  }

  fn pointer_set_primary_selection(&mut self, anchor: usize, head: usize) -> bool {
    let changed = self
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(anchor, head))
      .is_ok();
    if changed {
      self.sync_insert_mouse_selection_edit_arm();
    }
    changed
  }

  fn pointer_set_primary_range(&mut self, range: Range) -> bool {
    let changed = self
      .active_editor_mut()
      .document_mut()
      .set_selection(range.into())
      .is_ok();
    if changed {
      self.sync_insert_mouse_selection_edit_arm();
    }
    changed
  }

  fn pointer_char_drag_range(&self, anchor: usize, target: usize) -> Range {
    if target == anchor {
      return Range::point(target);
    }

    let text = self.active_editor_ref().document().text().slice(..);
    Range::point(anchor).put_cursor(text, target, true)
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
      PointerSelectionDragMode::Char => {
        let range = self.pointer_char_drag_range(state.anchor, target);
        self.pointer_set_primary_range(range)
      },
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

    let text = self.active_editor_ref().document().text();
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
    let text = self.active_editor_ref().document().text();
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
    let range = if let Some(active_cursor) = editor.view().active_cursor {
      selection.range_by_id(active_cursor).copied()
    } else {
      selection.ranges().first().copied()
    };
    let Some(range) = range else {
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
    let previous = self.active_editor;
    let changed = self.active_editor != Some(id);
    self.active_editor = Some(id);
    let loader = self.loader.clone();
    self
      .states
      .entry(id)
      .or_insert_with(|| EditorState::new(loader.clone(), self.workspace_root.as_path()));
    if changed {
      log_shared_lsp_debug(
        "activate_editor",
        format!(
          "prev={} next={} {}",
          shared_lsp_editor_id_label(previous),
          id.get().get(),
          self.shared_lsp_editor_state_summary(Some(id))
        ),
      );
      self.refresh_lsp_runtime_for_active_file();
      let _ = self.refresh_vcs_diff_base_for_editor(id);
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
          state.clear_highlight_caches();
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
      state.clear_highlight_caches();
      if parsed {
        state.syntax_parse_highlight_state.mark_parsed();
      } else {
        state.syntax_parse_highlight_state.mark_cleared();
      }
    }
  }

  fn refresh_vcs_ui_state(&mut self) -> bool {
    let next = if self.workspace_root.exists() {
      match self
        .vcs_provider
        .collect_changed_files(&self.workspace_root)
      {
        Ok(changes) => VcsUiState::from_changes(changes),
        Err(_) => VcsUiState::default(),
      }
    } else {
      VcsUiState::default()
    };

    if self.vcs_ui.semantically_eq(&next) {
      return false;
    }

    let mut next = next;
    next.generation = self.vcs_ui.generation.saturating_add(1);
    self.vcs_ui = next;
    true
  }

  fn refresh_vcs_diff_base_for_editor(&mut self, id: LibEditorId) -> bool {
    let path = self
      .inner
      .editor(id)
      .and_then(|editor| editor.active_file_path().map(Path::to_path_buf));
    let statusline = self
      .vcs_provider
      .get_statusline_info(path.as_deref().unwrap_or(self.workspace_root.as_path()))
      .map(|info| format_vcs_statusline_text(&info));
    let mut next_handle: Option<DiffHandle> = None;
    let mut next_signs: BTreeMap<usize, RenderGutterDiffKind> = BTreeMap::new();

    if let Some(path) = path
      && let Some(diff_base) = self.vcs_provider.get_diff_base(&path)
      && let Some(editor) = self.inner.editor(id)
    {
      let diff_base = Rope::from_str(String::from_utf8_lossy(&diff_base).as_ref());
      let doc = editor.document().text().clone();
      let handle = DiffHandle::new(diff_base, doc);
      next_signs = vcs_gutter_signs(&handle);
      next_handle = Some(handle);
    }

    if let Some(handle) = next_handle {
      self.vcs_diff_handles.insert(id, handle);
    } else {
      self.vcs_diff_handles.remove(&id);
    }

    let mut changed = false;
    if let Some(state) = self.states.get_mut(&id) {
      if state.vcs_statusline != statusline {
        state.vcs_statusline = statusline;
        changed = true;
      }
      if state.gutter_diff_signs != next_signs {
        state.gutter_diff_signs = next_signs;
        changed = true;
      }
      if changed {
        state.needs_render = true;
      }
    }
    changed
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
    self.stop_lsp_runtime(None);
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

  fn cursor_blink_generation(&self) -> u64 {
    self.cursor_blink_generation
  }

  fn bump_cursor_blink_generation(&mut self) {
    self.cursor_blink_generation = self.cursor_blink_generation.wrapping_add(1);
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
      state.clear_highlight_caches();
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

  fn build_frame_render_plan(&mut self) -> the_lib::render::FrameRenderPlan {
    self.build_frame_render_plan_with_styles_impl(RenderStyles::default())
  }

  fn build_frame_render_plan_with_styles(
    &mut self,
    styles: RenderStyles,
  ) -> the_lib::render::FrameRenderPlan {
    self.build_frame_render_plan_with_styles_impl(styles)
  }

  fn request_quit(&mut self) {
    self.should_quit = true;
  }

  fn mode(&self) -> Mode {
    self.active_state_ref().mode
  }

  fn set_mode(&mut self, mode: Mode) {
    let state = self.active_state_mut();
    state.mode = mode;
    if mode != Mode::Insert {
      state.insert_mouse_selection_edit_armed = false;
    }
    if mode != Mode::Insert {
      self.cancel_auto_completion();
    }
  }

  fn insert_mouse_selection_edit_armed(&self) -> bool {
    self.active_state_ref().insert_mouse_selection_edit_armed
  }

  fn set_insert_mouse_selection_edit_armed(&mut self, armed: bool) {
    self.active_state_mut().insert_mouse_selection_edit_armed = armed;
  }

  fn append_restore_cursor_pending(&self) -> bool {
    self.active_state_ref().append_restore_cursor_pending
  }

  fn set_append_restore_cursor_pending(&mut self, pending: bool) {
    self.active_state_mut().append_restore_cursor_pending = pending;
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
    if self.code_action_menu_is_active() {
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

    let source_index = self
      .completion_source_index_for_visible_index(index)
      .unwrap_or(index);
    let Some(item) = self.lsp_completion_items.get(source_index).cloned() else {
      return false;
    };

    let fallback_end = self.active_cursor_char_idx().unwrap_or(0);
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

  fn completion_menu_closed(&mut self) {
    self.clear_code_action_menu_state();
  }

  fn file_picker(&self) -> &FilePickerState {
    &self.active_state_ref().file_picker
  }

  fn file_picker_mut(&mut self) -> &mut FilePickerState {
    &mut self.active_state_mut().file_picker
  }

  fn supports_native_file_explorer(&self) -> bool {
    true
  }

  fn open_native_file_explorer(&mut self, current_buffer_directory: bool) -> bool {
    let workspace_root = self.workspace_root.clone();
    let active_path = self
      .active_editor_ref()
      .active_file_path()
      .map(Path::to_path_buf);
    let state = self.active_state_mut();

    if current_buffer_directory {
      state
        .file_tree
        .open_current_buffer_directory(active_path.as_deref(), workspace_root.as_path());
      self.request_render();
      return true;
    }

    state
      .file_tree
      .toggle_workspace_root(workspace_root.as_path());
    if state.file_tree.visible
      && let Some(active_path) = active_path.as_deref()
    {
      let _ = state.file_tree.select_path(active_path);
    }
    self.request_render();
    true
  }

  fn supports_embedded_terminal(&self) -> bool {
    true
  }

  fn open_terminal_in_active_pane(&mut self) -> bool {
    if self.active_editor.is_none() {
      return false;
    }
    let _ = self.active_editor_mut().open_terminal_in_active_pane();
    true
  }

  fn close_terminal_in_active_pane(&mut self) -> bool {
    if self.active_editor.is_none() {
      return false;
    }
    self.active_editor_mut().close_terminal_in_active_pane()
  }

  fn is_active_pane_terminal(&self) -> bool {
    self
      .active_editor
      .and_then(|id| self.inner.editor(id))
      .is_some_and(the_lib::editor::Editor::is_active_pane_terminal)
  }

  fn global_search(&mut self) {
    self.start_global_search();
  }

  fn file_picker_query_changed(&mut self, query: &str) {
    if self.global_search.is_active() {
      if query.trim().is_empty() {
        self.global_search.cancel_pending();
        replace_file_picker_items(self, Vec::new(), 0);
        let picker = self.file_picker_mut();
        picker.query = query.to_string();
        picker.cursor = query.len();
        picker.error = None;
        picker.preview = FilePickerPreview::Message("Type to search".to_string());
        self.request_render();
      } else {
        self.schedule_global_search(query.to_string());
      }
    }
  }

  fn file_picker_closed(&mut self) {
    self.global_search.deactivate();
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

  fn pointer_event(
    &mut self,
    event: the_default::PointerEvent,
  ) -> the_default::PointerEventOutcome {
    self.handle_editor_pointer_event(event)
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

  fn active_diagnostic_ranges(&self) -> Vec<Range> {
    let Some(state) = self.lsp_document.as_ref().filter(|state| state.opened) else {
      return Vec::new();
    };
    let Some(document) = self.diagnostics.document(&state.uri) else {
      return Vec::new();
    };

    let text = self.active_editor_ref().document().text();
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
    let id = self.active_editor?;
    let handle = self.vcs_diff_handles.get(&id)?;
    let diff = handle.load();
    let text = self.active_editor_ref().document().text();
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
            self.active_editor_ref().document().text(),
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
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if !cwd.exists() {
      return Err("current working directory does not exist".to_string());
    }

    let changes = self
      .vcs_provider
      .collect_changed_files(&cwd)
      .map_err(|err| err.to_string())?;

    let mut items = Vec::with_capacity(changes.len());
    for change in changes {
      match change {
        the_vcs::FileChange::Untracked { path } => {
          items.push(FilePickerChangedFileItem {
            kind: FilePickerChangedKind::Untracked,
            path,
            from_path: None,
          });
        },
        the_vcs::FileChange::Modified { path } => {
          items.push(FilePickerChangedFileItem {
            kind: FilePickerChangedKind::Modified,
            path,
            from_path: None,
          });
        },
        the_vcs::FileChange::Conflict { path } => {
          items.push(FilePickerChangedFileItem {
            kind: FilePickerChangedKind::Conflict,
            path,
            from_path: None,
          });
        },
        the_vcs::FileChange::Deleted { path } => {
          items.push(FilePickerChangedFileItem {
            kind: FilePickerChangedKind::Deleted,
            path,
            from_path: None,
          });
        },
        the_vcs::FileChange::Renamed { from_path, to_path } => {
          items.push(FilePickerChangedFileItem {
            kind:      FilePickerChangedKind::Renamed,
            path:      to_path,
            from_path: Some(from_path),
          });
        },
      }
    }

    items.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(items)
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
    if let Some(id) = self.active_editor {
      self.clear_hover_state();
      self.clear_signature_help_state();
      if let Some(editor) = self.inner.editor_mut(id) {
        editor.set_active_file_path(path);
      } else {
        return;
      }

      let active_path = self
        .inner
        .editor(id)
        .and_then(|editor| editor.active_file_path().map(Path::to_path_buf));
      if let Some(state) = self.states.get_mut(&id) {
        state
          .file_tree
          .sync_for_active_file(self.workspace_root.as_path(), active_path.as_deref());
      }

      self.refresh_editor_syntax(id);
      self.refresh_lsp_runtime_for_active_file();
      self.refresh_vcs_diff_base_for_editor(id);
    }
  }

  fn goto_buffer(&mut self, direction: CommandDirection, count: usize) -> bool {
    self.goto_buffer_impl(direction, count)
  }

  fn activate_buffer_by_index(&mut self, index: usize) -> bool {
    let Some(current) = self.active_editor else {
      return false;
    };

    self.lsp_close_current_document();
    let switched = {
      let Some(editor) = self.inner.editor_mut(current) else {
        return false;
      };
      editor.set_active_buffer(index)
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

  fn jump_forward_in_jumplist(&mut self, count: usize) -> bool {
    let Some(current) = self.active_editor else {
      return false;
    };

    let previous_buffer = self.active_editor_ref().active_buffer_index();
    let switched = {
      let Some(editor) = self.inner.editor_mut(current) else {
        return false;
      };
      editor.jump_forward(count.max(1))
    };
    if !switched {
      return false;
    }

    if self.active_editor_ref().active_buffer_index() != previous_buffer {
      self.lsp_close_current_document();
      let active_path = self
        .inner
        .editor(current)
        .and_then(|editor| editor.active_file_path().map(Path::to_path_buf));
      <Self as DefaultContext>::set_file_path(self, active_path);
    }

    self.request_render();
    true
  }

  fn jump_backward_in_jumplist(&mut self, count: usize) -> bool {
    let Some(current) = self.active_editor else {
      return false;
    };

    let previous_buffer = self.active_editor_ref().active_buffer_index();
    let switched = {
      let Some(editor) = self.inner.editor_mut(current) else {
        return false;
      };
      editor.jump_backward(count.max(1))
    };
    if !switched {
      return false;
    }

    if self.active_editor_ref().active_buffer_index() != previous_buffer {
      self.lsp_close_current_document();
      let active_path = self
        .inner
        .editor(current)
        .and_then(|editor| editor.active_file_path().map(Path::to_path_buf));
      <Self as DefaultContext>::set_file_path(self, active_path);
    }

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

  fn lsp_workspace_command_names(&self) -> Vec<String> {
    self
      .lsp_server_capabilities_snapshot()
      .map(|capabilities| capabilities.workspace_commands())
      .unwrap_or_default()
  }

  fn lsp_execute_workspace_command(
    &mut self,
    command: &str,
    raw_args: Option<&str>,
  ) -> Result<String, String> {
    let command = command.trim();
    if command.is_empty() {
      return Err("workspace command name required".to_string());
    }
    if self.active_language_server_names().is_empty() {
      return Err("no active language server for current file".to_string());
    }
    if !self.lsp_ready {
      return Err("language server is not ready".to_string());
    }
    if !self.lsp_supports(LspCapability::WorkspaceCommand) {
      return Err("workspace commands are not supported by the active server".to_string());
    }

    let available = self.lsp_workspace_command_names();
    if !available.is_empty() && !available.iter().any(|name| name == command) {
      return Err(format!("unknown workspace command '{command}'"));
    }

    let arguments = raw_args
      .map(str::trim)
      .filter(|raw| !raw.is_empty())
      .map(|raw| {
        serde_json::Deserializer::from_str(raw)
          .into_iter::<Value>()
          .collect::<Result<Vec<_>, _>>()
          .map_err(|err| format!("failed to parse workspace command arguments: {err}"))
      })
      .transpose()?
      .filter(|arguments| !arguments.is_empty());

    self.lsp_open_current_document();
    let params = execute_command_params(command, arguments);
    self
      .lsp_send_request_raw("workspace/executeCommand", params)
      .map_err(|err| format!("failed to dispatch workspace command '{command}': {err}"))?;

    Ok(format!("dispatched workspace command: {command}"))
  }

  fn configured_language_server_names(&self) -> Vec<String> {
    self
      .lsp_runtime
      .config()
      .server()
      .map(|server| vec![server.name().to_string()])
      .unwrap_or_default()
  }

  fn active_language_server_names(&self) -> Vec<String> {
    if !self.lsp_runtime.is_running() {
      return Vec::new();
    }

    self
      .lsp_server_name
      .clone()
      .or_else(|| {
        self
          .lsp_runtime
          .config()
          .server()
          .map(|server| server.name().to_string())
      })
      .into_iter()
      .collect()
  }

  fn lsp_restart_servers(&mut self, names: &[String]) -> Result<String, String> {
    let configured = self.configured_language_server_names();
    let Some(server_name) = configured.first().cloned() else {
      return Err("no configured language server for current file".to_string());
    };

    let invalid = names
      .iter()
      .filter(|name| {
        !configured
          .iter()
          .any(|configured_name| configured_name == *name)
      })
      .cloned()
      .collect::<Vec<_>>();
    if !invalid.is_empty() {
      let noun = if invalid.len() == 1 {
        "language server"
      } else {
        "language servers"
      };
      return Err(format!("unknown {noun}: {}", invalid.join(", ")));
    }

    if self.lsp_runtime.is_running() {
      self
        .lsp_runtime
        .restart_server()
        .map_err(|err| format!("failed to restart language server '{server_name}': {err}"))?;
    } else {
      self.refresh_lsp_runtime_for_active_file();
      if !self.lsp_runtime.is_running() {
        return Err(format!("failed to start language server '{server_name}'"));
      }
    }

    Ok(format!("restarting language server: {server_name}"))
  }

  fn lsp_stop_servers(&mut self, names: &[String]) -> Result<String, String> {
    let active = self.active_language_server_names();
    let Some(server_name) = active.first().cloned() else {
      return Err("no active language server for current file".to_string());
    };

    let invalid = names
      .iter()
      .filter(|name| !active.iter().any(|active_name| active_name == *name))
      .cloned()
      .collect::<Vec<_>>();
    if !invalid.is_empty() {
      let noun = if invalid.len() == 1 {
        "language server"
      } else {
        "language servers"
      };
      return Err(format!("unknown {noun}: {}", invalid.join(", ")));
    }

    self.stop_lsp_runtime(Some("stopped"));
    Ok(format!("stopped language server: {server_name}"))
  }

  fn open_file(&mut self, path: &Path) -> std::io::Result<()> {
    let normalized_path = normalize_path_for_open(path);

    if self.native_tab_open_gateway_enabled {
      let route_context = {
        let editor = self.active_editor_ref();
        (
          editor.active_buffer_index(),
          editor.find_buffer_by_path(&normalized_path),
        )
      };
      if let Some(existing_index) = route_context.1
        && existing_index != route_context.0
        && let Some(buffer_id) = self
          .active_editor_ref()
          .buffer_snapshot(existing_index)
          .map(|snapshot| snapshot.buffer_id)
      {
        self
          .native_tab_open_requests
          .push_back(NativeTabOpenRequest::focus_existing(
            buffer_id,
            Some(normalized_path),
          ));
        self.request_render();
        return Ok(());
      }
    }

    if self.active_editor_ref().is_active_pane_terminal() {
      let _ = self.active_editor_mut().hide_active_terminal_surface();
    }

    self.lsp_close_current_document();
    self.clear_hover_state();
    self.clear_signature_help_state();
    let reused = {
      let editor = self.active_editor_mut();
      if let Some(index) = editor.find_buffer_by_path(&normalized_path) {
        let _ = editor.set_active_buffer(index);
        true
      } else {
        false
      }
    };

    if !reused {
      let content = std::fs::read_to_string(&normalized_path)?;
      let viewport = self.active_editor_ref().view().viewport;
      let native_tab_gateway_enabled = self.native_tab_open_gateway_enabled;
      {
        let editor = self.active_editor_mut();
        let replace_active = if native_tab_gateway_enabled {
          !editor.is_active_pane_terminal() && !editor.document().flags().modified
        } else {
          !editor.is_active_pane_terminal() && editor.can_reuse_active_untitled_buffer_for_open()
        };
        if replace_active {
          let _ =
            editor.replace_active_buffer(Rope::from_str(&content), Some(normalized_path.clone()));
        } else {
          let view = ViewState::new(viewport, LibPosition::new(0, 0));
          let _ = editor.open_buffer(
            Rope::from_str(&content),
            view,
            Some(normalized_path.clone()),
          );
        }
        let doc = editor.document_mut();
        doc.set_display_name(
          normalized_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| normalized_path.display().to_string()),
        );
        let _ = doc.mark_saved();
      }
    }
    DefaultContext::set_file_path(self, Some(normalized_path));
    self.request_render();
    Ok(())
  }

  fn lsp_goto_definition(&mut self) {
    log_shared_lsp_debug("goto_definition_begin", "entered");
    if !self.lsp_supports(LspCapability::GotoDefinition) {
      log_shared_lsp_debug("goto_definition_skip", "reason=unsupported");
      let _ = <Self as DefaultContext>::push_error(self, "goto", "No definition found.");
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      log_shared_lsp_debug("goto_definition_skip", "reason=no_position");
      let _ = <Self as DefaultContext>::push_error(self, "goto", "No definition found.");
      return;
    };

    log_shared_lsp_debug(
      "goto_definition_dispatch",
      format!(
        "uri={} line={} char={}",
        uri, position.line, position.character
      ),
    );
    self.dispatch_lsp_request(
      "textDocument/definition",
      goto_definition_params(&uri, position),
      PendingLspRequestKind::GotoDefinition { uri },
    );
  }

  fn lsp_goto_declaration(&mut self) {
    log_shared_lsp_debug("goto_declaration_begin", "entered");
    if !self.lsp_supports(LspCapability::GotoDeclaration) {
      log_shared_lsp_debug("goto_declaration_skip", "reason=unsupported");
      let _ = <Self as DefaultContext>::push_error(self, "goto", "No declaration found.");
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      log_shared_lsp_debug("goto_declaration_skip", "reason=no_position");
      let _ = <Self as DefaultContext>::push_error(self, "goto", "No declaration found.");
      return;
    };

    log_shared_lsp_debug(
      "goto_declaration_dispatch",
      format!(
        "uri={} line={} char={}",
        uri, position.line, position.character
      ),
    );
    self.dispatch_lsp_request(
      "textDocument/declaration",
      goto_declaration_params(&uri, position),
      PendingLspRequestKind::GotoDeclaration { uri },
    );
  }

  fn lsp_goto_type_definition(&mut self) {
    log_shared_lsp_debug("goto_type_definition_begin", "entered");
    if !self.lsp_supports(LspCapability::GotoTypeDefinition) {
      log_shared_lsp_debug("goto_type_definition_skip", "reason=unsupported");
      let _ = <Self as DefaultContext>::push_error(self, "goto", "No type definition found.");
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      log_shared_lsp_debug("goto_type_definition_skip", "reason=no_position");
      let _ = <Self as DefaultContext>::push_error(self, "goto", "No type definition found.");
      return;
    };

    log_shared_lsp_debug(
      "goto_type_definition_dispatch",
      format!(
        "uri={} line={} char={}",
        uri, position.line, position.character
      ),
    );
    self.dispatch_lsp_request(
      "textDocument/typeDefinition",
      goto_type_definition_params(&uri, position),
      PendingLspRequestKind::GotoTypeDefinition { uri },
    );
  }

  fn lsp_goto_implementation(&mut self) {
    log_shared_lsp_debug("goto_implementation_begin", "entered");
    if !self.lsp_supports(LspCapability::GotoImplementation) {
      log_shared_lsp_debug("goto_implementation_skip", "reason=unsupported");
      let _ = <Self as DefaultContext>::push_error(self, "goto", "No implementation found.");
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      log_shared_lsp_debug("goto_implementation_skip", "reason=no_position");
      let _ = <Self as DefaultContext>::push_error(self, "goto", "No implementation found.");
      return;
    };

    log_shared_lsp_debug(
      "goto_implementation_dispatch",
      format!(
        "uri={} line={} char={}",
        uri, position.line, position.character
      ),
    );
    self.dispatch_lsp_request(
      "textDocument/implementation",
      goto_implementation_params(&uri, position),
      PendingLspRequestKind::GotoImplementation { uri },
    );
  }

  fn lsp_document_symbols(&mut self) {
    if !self.lsp_supports(LspCapability::DocumentSymbols) {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
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
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
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
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
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

  fn lsp_code_actions(&mut self) {
    if !self.lsp_supports(LspCapability::CodeAction) {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
        "code actions are not supported by the active server",
      );
      return;
    }

    let Some((uri, range)) = self.current_lsp_code_action_range() else {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
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
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
        "rename is not supported by the active server",
      );
      return;
    }

    let new_name = new_name.trim();
    if new_name.is_empty() {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
        "rename requires a non-empty name",
      );
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
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

  fn lsp_hover(&mut self) {
    log_shared_lsp_debug("hover_begin", "entered");
    self.lsp_pending_mouse_hover = None;
    let anchor_char = self
      .active_or_first_selection_range()
      .map(|range| {
        let doc = self.active_editor_ref().document();
        range.cursor(doc.text().slice(..))
      })
      .unwrap_or(0);
    if self.start_hover_at_char(HoverTriggerSource::Keyboard, anchor_char, true, true) {
      self.request_render();
    }
  }

  fn lsp_select_references_to_symbol_under_cursor(&mut self) {
    if !self.lsp_supports(LspCapability::DocumentHighlight) {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
        "document highlights are not supported by the active server",
      );
      return;
    }

    let Some((uri, position)) = self.current_lsp_position() else {
      self.publish_lsp_message(
        the_lib::messages::MessageLevel::Warning,
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

  fn on_file_saved(&mut self, _path: &Path, text: &str) {
    if let Some(watch) = self.lsp_watched_file.as_mut() {
      watch.stream.suppress_until = Some(Instant::now() + lsp_self_save_suppress_window());
      clear_reload_state(&mut watch.stream.reload_state);
    }
    self.lsp_send_did_save(Some(text));
    if self.refresh_vcs_ui_state() {
      self.request_render();
    }
  }

  fn on_before_quit(&mut self) {
    self.stop_lsp_runtime(Some("stopped"));
  }

  fn scrolloff(&self) -> usize {
    self.active_state_ref().scrolloff
  }
}

fn key_event_from_ffi(event: ffi::KeyEvent) -> the_default::KeyEvent {
  use the_default::{
    Key as LibKey,
    KeyEvent as LibKeyEvent,
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

  let modifiers = modifiers_from_ffi_bits(event.modifiers);

  LibKeyEvent { key, modifiers }
}

fn pointer_event_from_ffi_parts(
  kind: u8,
  button: u8,
  logical_col: u16,
  logical_row: u16,
  modifiers: u8,
  click_count: u8,
  surface_id: u64,
) -> Option<the_default::PointerEvent> {
  use the_default::{
    PointerEvent as LibPointerEvent,
    PointerKind as LibPointerKind,
  };

  let kind = match kind {
    0 => LibPointerKind::Down(pointer_button_from_ffi(button)?),
    1 => LibPointerKind::Drag(pointer_button_from_ffi(button)?),
    2 => LibPointerKind::Up(pointer_button_from_ffi(button)?),
    3 => LibPointerKind::Move,
    4 => LibPointerKind::Scroll,
    _ => return None,
  };

  let mut pointer_event = LibPointerEvent::new(kind, 0, 0)
    .with_modifiers(modifiers_from_ffi_bits(modifiers))
    .with_click_count(click_count);

  if logical_row != u16::MAX && logical_col != u16::MAX {
    pointer_event = pointer_event.with_logical_pos(logical_col, logical_row);
  }
  if surface_id != 0 {
    pointer_event = pointer_event.with_surface_id(surface_id);
  }

  Some(pointer_event)
}

fn pointer_button_from_ffi(button: u8) -> Option<the_default::PointerButton> {
  match button {
    1 => Some(the_default::PointerButton::Left),
    2 => Some(the_default::PointerButton::Middle),
    3 => Some(the_default::PointerButton::Right),
    _ => None,
  }
}

fn modifiers_from_ffi_bits(bits: u8) -> the_default::Modifiers {
  let mut modifiers = the_default::Modifiers::empty();
  if (bits & the_default::Modifiers::CTRL) != 0 {
    modifiers.insert(the_default::Modifiers::CTRL);
  }
  if (bits & the_default::Modifiers::ALT) != 0 {
    modifiers.insert(the_default::Modifiers::ALT);
  }
  if (bits & the_default::Modifiers::SHIFT) != 0 {
    modifiers.insert(the_default::Modifiers::SHIFT);
  }
  modifiers
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
    fn open_file_path(self: &mut App, id: EditorId, path: &str) -> bool;
    fn open_file_path_in_new_tab(self: &mut App, id: EditorId, path: &str) -> bool;
    fn open_untitled_buffer(self: &mut App, id: EditorId) -> u64;
    fn open_untitled_buffer_in_new_tab(self: &mut App, id: EditorId) -> u64;
    fn supports_embedded_terminal(self: &App) -> bool;
    fn open_terminal_in_active_pane(self: &mut App, id: EditorId) -> bool;
    fn close_terminal_in_active_pane(self: &mut App, id: EditorId) -> bool;
    fn hide_active_terminal_surface(self: &mut App, id: EditorId) -> bool;
    fn execute_command_named(self: &mut App, id: EditorId, name: &str) -> bool;
    fn is_active_pane_terminal(self: &mut App, id: EditorId) -> bool;
    fn active_file_path(self: &App, id: EditorId) -> String;
    fn set_native_tab_open_gateway(self: &mut App, enabled: bool);
    fn set_inline_diagnostic_rendering_enabled(self: &mut App, enabled: bool);
    fn take_native_tab_open_request_path(self: &mut App) -> String;
    fn set_active_cursor(self: &mut App, id: EditorId, cursor_id: u64) -> bool;
    fn clear_active_cursor(self: &mut App, id: EditorId) -> bool;
    fn cursor_ids(self: &App, id: EditorId) -> Vec<u64>;
    fn split_separator_count(self: &mut App, id: EditorId) -> usize;
    fn split_separator_at(self: &mut App, id: EditorId, index: usize) -> SplitSeparator;
    fn split_active_pane(self: &mut App, id: EditorId, axis: u8) -> bool;
    fn jump_active_pane(self: &mut App, id: EditorId, direction: u8) -> bool;
    fn move_pane(
      self: &mut App,
      id: EditorId,
      source_pane: u64,
      destination_pane: u64,
      direction: u8,
    ) -> bool;
    fn resize_split(self: &mut App, id: EditorId, split_id: u64, x: u16, y: u16) -> bool;
    fn terminal_surface_count(self: &mut App, id: EditorId) -> usize;
    fn terminal_surface_at(self: &mut App, id: EditorId, index: usize) -> TerminalSurfaceSnapshot;
    fn editor_surface_count(self: &mut App, id: EditorId) -> usize;
    fn editor_surface_at(self: &mut App, id: EditorId, index: usize) -> EditorSurfaceSnapshot;
    fn focus_terminal_surface(self: &mut App, id: EditorId, terminal_id: u64) -> bool;
    fn render_plan(self: &mut App, id: EditorId) -> RenderPlan;
    fn frame_render_plan(self: &mut App, id: EditorId) -> RenderFramePlan;
    fn docs_popup_anchor(self: &mut App, id: EditorId) -> DocsPopupAnchor;
    fn render_plan_with_styles(self: &mut App, id: EditorId, styles: RenderStyles) -> RenderPlan;
    fn ui_tree_json(self: &mut App, id: EditorId) -> String;
    fn buffer_tabs_snapshot_json(self: &mut App, id: EditorId) -> String;
    fn activate_buffer_tab(self: &mut App, id: EditorId, buffer_index: usize) -> bool;
    fn close_buffer_tab(self: &mut App, id: EditorId, buffer_index: usize) -> bool;
    fn close_buffer_by_id(self: &mut App, id: EditorId, buffer_id: u64) -> bool;
    fn message_snapshot_json(self: &mut App, id: EditorId) -> String;
    fn message_events_since_json(self: &mut App, id: EditorId, seq: u64) -> String;
    fn ui_event_json(self: &mut App, id: EditorId, event_json: &str) -> bool;
    fn text(self: &App, id: EditorId) -> String;
    fn pending_keys_json(self: &App, id: EditorId) -> String;
    fn pending_key_hints_json(self: &App, id: EditorId) -> String;
    fn mode(self: &App, id: EditorId) -> u8;
    fn theme_highlight_style(self: &App, highlight: u32) -> Style;
    fn theme_ui_style(self: &App, scope: &str) -> Style;
    fn theme_effective_name(self: &App) -> String;
    fn theme_ghostty_snapshot(self: &App) -> GhosttyThemeSnapshot;
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
    fn command_palette_placeholder(self: &mut App, id: EditorId) -> String;
    fn command_palette_is_file_mode(self: &mut App, id: EditorId) -> bool;
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
    fn command_palette_filtered_emphasis(self: &mut App, id: EditorId, index: usize) -> bool;
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
    fn file_tree_set_visible(self: &mut App, id: EditorId, visible: bool) -> bool;
    fn file_tree_toggle(self: &mut App, id: EditorId) -> bool;
    fn file_tree_open_workspace_root(self: &mut App, id: EditorId) -> bool;
    fn file_tree_open_current_buffer_directory(self: &mut App, id: EditorId) -> bool;
    fn file_tree_set_expanded(self: &mut App, id: EditorId, path: &str, expanded: bool) -> bool;
    fn file_tree_select_path(self: &mut App, id: EditorId, path: &str) -> bool;
    fn file_tree_open_selected(self: &mut App, id: EditorId) -> bool;
    fn file_tree_snapshot(self: &mut App, id: EditorId, max_nodes: usize) -> FileTreeSnapshotData;
    fn file_picker_snapshot(
      self: &mut App,
      id: EditorId,
      max_items: usize,
    ) -> FilePickerSnapshotData;
    fn file_picker_window_snapshot(
      self: &mut App,
      id: EditorId,
      window_start: usize,
      max_items: usize,
    ) -> FilePickerSnapshotData;
    fn file_picker_preview(self: &mut App, id: EditorId) -> PreviewData;
    fn file_picker_preview_window(
      self: &mut App,
      id: EditorId,
      offset: usize,
      visible_rows: usize,
      overscan: usize,
    ) -> PreviewData;
    fn poll_background(self: &mut App, id: EditorId) -> bool;
    fn take_should_quit(self: &mut App) -> bool;
    fn handle_key(self: &mut App, id: EditorId, event: KeyEvent) -> bool;
    fn handle_mouse(
      self: &mut App,
      id: EditorId,
      packed: u64,
      logical_col: u16,
      logical_row: u16,
      surface_id: u64,
    ) -> bool;
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
    fn first_cursor(self: &Document) -> usize;
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
    fn collapse_to_first(self: &mut Document);

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
  struct DocsPopupAnchor {
    has_value: bool,
    pane_id:   u64,
    row:       u16,
    col:       u16,
  }

  #[swift_bridge(swift_repr = "struct")]
  struct Color {
    kind:  u8,
    value: u32,
  }

  #[swift_bridge(swift_repr = "struct")]
  struct OptionalColor {
    has_value: bool,
    color:     Color,
  }

  #[swift_bridge(swift_repr = "struct")]
  struct GhosttyThemeSnapshot {
    background:           OptionalColor,
    foreground:           OptionalColor,
    cursor_color:         OptionalColor,
    cursor_text:          OptionalColor,
    selection_background: OptionalColor,
    selection_foreground: OptionalColor,
    palette0:             OptionalColor,
    palette1:             OptionalColor,
    palette2:             OptionalColor,
    palette3:             OptionalColor,
    palette4:             OptionalColor,
    palette5:             OptionalColor,
    palette6:             OptionalColor,
    palette7:             OptionalColor,
    palette8:             OptionalColor,
    palette9:             OptionalColor,
    palette10:            OptionalColor,
    palette11:            OptionalColor,
    palette12:            OptionalColor,
    palette13:            OptionalColor,
    palette14:            OptionalColor,
    palette15:            OptionalColor,
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
    fn kind(self: &RenderSelection) -> u8;
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
    fn layout_generation(self: &RenderPlan) -> u64;
    fn text_generation(self: &RenderPlan) -> u64;
    fn decoration_generation(self: &RenderPlan) -> u64;
    fn cursor_generation(self: &RenderPlan) -> u64;
    fn scroll_generation(self: &RenderPlan) -> u64;
    fn theme_generation(self: &RenderPlan) -> u64;
    fn damage_start_row(self: &RenderPlan) -> u16;
    fn damage_end_row(self: &RenderPlan) -> u16;
    fn damage_is_full(self: &RenderPlan) -> bool;
    fn damage_reason(self: &RenderPlan) -> u8;
    fn cursor_blink_enabled(self: &RenderPlan) -> bool;
    fn cursor_blink_interval_ms(self: &RenderPlan) -> u16;
    fn cursor_blink_delay_ms(self: &RenderPlan) -> u16;
    fn cursor_blink_generation(self: &RenderPlan) -> u64;
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

  extern "Rust" {
    type RenderFramePane;
    fn pane_id(self: &RenderFramePane) -> u64;
    fn rect(self: &RenderFramePane) -> Rect;
    fn is_active(self: &RenderFramePane) -> bool;
    fn pane_kind(self: &RenderFramePane) -> u8;
    fn terminal_id(self: &RenderFramePane) -> u64;
    fn plan(self: &RenderFramePane) -> RenderPlan;
  }

  extern "Rust" {
    type RenderFramePlan;
    fn active_pane_id(self: &RenderFramePlan) -> u64;
    fn frame_generation(self: &RenderFramePlan) -> u64;
    fn pane_structure_generation(self: &RenderFramePlan) -> u64;
    fn pane_count(self: &RenderFramePlan) -> usize;
    fn pane_at(self: &RenderFramePlan, index: usize) -> RenderFramePane;
    fn active_plan(self: &RenderFramePlan) -> RenderPlan;
    fn changed_pane_count(self: &RenderFramePlan) -> usize;
    fn changed_pane_id_at(self: &RenderFramePlan, index: usize) -> u64;
    fn damage_is_full(self: &RenderFramePlan) -> bool;
    fn damage_reason(self: &RenderFramePlan) -> u8;
  }

  extern "Rust" {
    type EditorSurfaceSnapshot;
    fn pane_id(self: &EditorSurfaceSnapshot) -> u64;
    fn buffer_id(self: &EditorSurfaceSnapshot) -> u64;
    fn buffer_index(self: &EditorSurfaceSnapshot) -> usize;
    fn title(self: &EditorSurfaceSnapshot) -> String;
    fn modified(self: &EditorSurfaceSnapshot) -> bool;
    fn file_path(self: &EditorSurfaceSnapshot) -> String;
    fn is_active(self: &EditorSurfaceSnapshot) -> bool;
  }

  extern "Rust" {
    type TerminalSurfaceSnapshot;
    fn terminal_id(self: &TerminalSurfaceSnapshot) -> u64;
    fn pane_id(self: &TerminalSurfaceSnapshot) -> u64;
    fn is_active(self: &TerminalSurfaceSnapshot) -> bool;
  }

  extern "Rust" {
    type SplitSeparator;
    fn split_id(self: &SplitSeparator) -> u64;
    fn axis(self: &SplitSeparator) -> u8;
    fn line(self: &SplitSeparator) -> u16;
    fn span_start(self: &SplitSeparator) -> u16;
    fn span_end(self: &SplitSeparator) -> u16;
  }

  // File picker snapshot (direct FFI, no JSON)
  extern "Rust" {
    type FileTreeSnapshotData;
    fn visible(self: &FileTreeSnapshotData) -> bool;
    fn mode(self: &FileTreeSnapshotData) -> u8;
    fn root(self: &FileTreeSnapshotData) -> String;
    fn selected_path(self: &FileTreeSnapshotData) -> String;
    fn refresh_generation(self: &FileTreeSnapshotData) -> u64;
    fn vcs_generation(self: &FileTreeSnapshotData) -> u64;
    fn node_count(self: &FileTreeSnapshotData) -> usize;
    fn node_at(self: &FileTreeSnapshotData, index: usize) -> FileTreeNodeFFI;
  }

  extern "Rust" {
    type FileTreeNodeFFI;
    fn id(self: &FileTreeNodeFFI) -> String;
    fn path(self: &FileTreeNodeFFI) -> String;
    fn name(self: &FileTreeNodeFFI) -> String;
    fn depth(self: &FileTreeNodeFFI) -> usize;
    fn kind(self: &FileTreeNodeFFI) -> u8;
    fn expanded(self: &FileTreeNodeFFI) -> bool;
    fn selected(self: &FileTreeNodeFFI) -> bool;
    fn has_unloaded_children(self: &FileTreeNodeFFI) -> bool;
    fn vcs_status(self: &FileTreeNodeFFI) -> u8;
    fn vcs_descendant_count(self: &FileTreeNodeFFI) -> usize;
  }

  extern "Rust" {
    type FilePickerSnapshotData;
    fn active(self: &FilePickerSnapshotData) -> bool;
    fn title(self: &FilePickerSnapshotData) -> String;
    fn picker_kind(self: &FilePickerSnapshotData) -> u8;
    fn query(self: &FilePickerSnapshotData) -> String;
    fn matched_count(self: &FilePickerSnapshotData) -> usize;
    fn total_count(self: &FilePickerSnapshotData) -> usize;
    fn scanning(self: &FilePickerSnapshotData) -> bool;
    fn root(self: &FilePickerSnapshotData) -> String;
    fn selected_index(self: &FilePickerSnapshotData) -> i64;
    fn window_start(self: &FilePickerSnapshotData) -> usize;
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
    fn row_kind(self: &FilePickerItemFFI) -> u8;
    fn severity(self: &FilePickerItemFFI) -> u8;
    fn primary(self: &FilePickerItemFFI) -> String;
    fn secondary(self: &FilePickerItemFFI) -> String;
    fn tertiary(self: &FilePickerItemFFI) -> String;
    fn quaternary(self: &FilePickerItemFFI) -> String;
    fn line(self: &FilePickerItemFFI) -> usize;
    fn column(self: &FilePickerItemFFI) -> usize;
    fn depth(self: &FilePickerItemFFI) -> usize;
  }

  // File picker preview (direct FFI, no JSON)
  extern "Rust" {
    type PreviewData;
    fn kind(self: &PreviewData) -> u8;
    fn path(self: &PreviewData) -> String;
    fn loading(self: &PreviewData) -> bool;
    fn truncated(self: &PreviewData) -> bool;
    fn total_lines(self: &PreviewData) -> usize;
    fn show(self: &PreviewData) -> bool;
    fn offset(self: &PreviewData) -> usize;
    fn window_start(self: &PreviewData) -> usize;
    fn line_count(self: &PreviewData) -> usize;
    fn line_at(self: &PreviewData, index: usize) -> PreviewLine;
  }

  extern "Rust" {
    type PreviewLine;
    fn kind(self: &PreviewLine) -> u8;
    fn virtual_row(self: &PreviewLine) -> usize;
    fn line_number(self: &PreviewLine) -> usize;
    fn focused(self: &PreviewLine) -> bool;
    fn marker(self: &PreviewLine) -> String;
    fn segment_count(self: &PreviewLine) -> usize;
    fn segment_at(self: &PreviewLine, index: usize) -> PreviewLineSegment;
  }

  extern "Rust" {
    type PreviewLineSegment;
    fn text(self: &PreviewLineSegment) -> String;
    fn highlight_id(self: &PreviewLineSegment) -> u32;
    fn is_match(self: &PreviewLineSegment) -> bool;
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

impl Default for ffi::DocsPopupAnchor {
  fn default() -> Self {
    Self {
      has_value: false,
      pane_id:   0,
      row:       0,
      col:       0,
    }
  }
}

impl ffi::RenderStyles {
  fn to_lib(self) -> RenderStyles {
    RenderStyles {
      selection:                  self.selection.to_lib(),
      cursor:                     self.cursor.to_lib(),
      active_cursor:              self.active_cursor.to_lib(),
      cursor_kind:                LibCursorKind::Block,
      active_cursor_kind:         LibCursorKind::Block,
      non_block_cursor_uses_head: true,
      gutter:                     self.gutter.to_lib(),
      gutter_active:              self.gutter_active.to_lib(),
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

impl Default for ffi::OptionalColor {
  fn default() -> Self {
    Self {
      has_value: false,
      color:     ffi::Color::default(),
    }
  }
}

impl Default for ffi::GhosttyThemeSnapshot {
  fn default() -> Self {
    Self {
      background:           ffi::OptionalColor::default(),
      foreground:           ffi::OptionalColor::default(),
      cursor_color:         ffi::OptionalColor::default(),
      cursor_text:          ffi::OptionalColor::default(),
      selection_background: ffi::OptionalColor::default(),
      selection_foreground: ffi::OptionalColor::default(),
      palette0:             ffi::OptionalColor::default(),
      palette1:             ffi::OptionalColor::default(),
      palette2:             ffi::OptionalColor::default(),
      palette3:             ffi::OptionalColor::default(),
      palette4:             ffi::OptionalColor::default(),
      palette5:             ffi::OptionalColor::default(),
      palette6:             ffi::OptionalColor::default(),
      palette7:             ffi::OptionalColor::default(),
      palette8:             ffi::OptionalColor::default(),
      palette9:             ffi::OptionalColor::default(),
      palette10:            ffi::OptionalColor::default(),
      palette11:            ffi::OptionalColor::default(),
      palette12:            ffi::OptionalColor::default(),
      palette13:            ffi::OptionalColor::default(),
      palette14:            ffi::OptionalColor::default(),
      palette15:            ffi::OptionalColor::default(),
    }
  }
}

fn optional_color_snapshot(color: Option<LibColor>) -> ffi::OptionalColor {
  let rgb = color.and_then(color_to_rgb);
  ffi::OptionalColor {
    has_value: rgb.is_some(),
    color:     rgb
      .map(|value| ffi::Color { kind: 2, value })
      .unwrap_or_default(),
  }
}

fn style_fg(theme: &Theme, scope: &str) -> Option<LibColor> {
  theme.try_get(scope).and_then(|style| style.fg)
}

fn style_bg(theme: &Theme, scope: &str) -> Option<LibColor> {
  theme.try_get(scope).and_then(|style| style.bg)
}

fn palette_named(theme: &Theme, name: &str) -> Option<LibColor> {
  theme.palette_color(name)
}

fn color_to_rgb(color: LibColor) -> Option<u32> {
  match color {
    LibColor::Reset => None,
    LibColor::Rgb(r, g, b) => Some(((r as u32) << 16) | ((g as u32) << 8) | b as u32),
    LibColor::Indexed(idx) => ansi_index_to_rgb(idx),
    LibColor::Black => ansi_index_to_rgb(0),
    LibColor::Red => ansi_index_to_rgb(1),
    LibColor::Green => ansi_index_to_rgb(2),
    LibColor::Yellow => ansi_index_to_rgb(3),
    LibColor::Blue => ansi_index_to_rgb(4),
    LibColor::Magenta => ansi_index_to_rgb(5),
    LibColor::Cyan => ansi_index_to_rgb(6),
    LibColor::Gray => ansi_index_to_rgb(7),
    LibColor::LightRed => ansi_index_to_rgb(8),
    LibColor::LightGreen => ansi_index_to_rgb(9),
    LibColor::LightYellow => ansi_index_to_rgb(10),
    LibColor::LightBlue => ansi_index_to_rgb(11),
    LibColor::LightMagenta => ansi_index_to_rgb(12),
    LibColor::LightCyan => ansi_index_to_rgb(13),
    LibColor::LightGray => ansi_index_to_rgb(14),
    LibColor::White => ansi_index_to_rgb(15),
  }
}

fn ansi_index_to_rgb(index: u8) -> Option<u32> {
  let rgb = match index {
    0 => 0x000000,
    1 => 0xCD0000,
    2 => 0x00CD00,
    3 => 0xCDCD00,
    4 => 0x0000EE,
    5 => 0xCD00CD,
    6 => 0x00CDCD,
    7 => 0xE5E5E5,
    8 => 0x7F7F7F,
    9 => 0xFF0000,
    10 => 0x00FF00,
    11 => 0xFFFF00,
    12 => 0x5C5CFF,
    13 => 0xFF00FF,
    14 => 0x00FFFF,
    15 => 0xFFFFFF,
    _ => return None,
  };
  Some(rgb)
}

fn ghostty_theme_snapshot_from_theme(theme: &Theme) -> ffi::GhosttyThemeSnapshot {
  let ghostty = theme.ghostty();
  let background = ghostty.background().or_else(|| {
    style_bg(theme, "ui.background")
      .or_else(|| style_bg(theme, "ui.window"))
      .or_else(|| style_bg(theme, "ui.popup"))
  });
  let foreground = ghostty.foreground().or_else(|| {
    style_fg(theme, "ui.text")
      .or_else(|| style_fg(theme, "ui.statusline"))
      .or_else(|| style_fg(theme, "ui.popup"))
  });
  let cursor_color = ghostty
    .cursor_color()
    .or_else(|| style_bg(theme, "ui.cursor").or_else(|| style_fg(theme, "ui.cursor")));
  let cursor_text = ghostty
    .cursor_text()
    .or_else(|| style_fg(theme, "ui.cursor").or(foreground));
  let selection_background = ghostty.selection_background().or_else(|| {
    style_bg(theme, "ui.selection")
      .or_else(|| style_bg(theme, "ui.menu.selected"))
      .or_else(|| style_bg(theme, "ui.cursor.match"))
  });
  let selection_foreground = ghostty
    .selection_foreground()
    .or_else(|| style_fg(theme, "ui.selection").or(foreground));
  let derived_palette = [
    ghostty
      .palette_color(0)
      .or_else(|| palette_named(theme, "black")),
    ghostty
      .palette_color(1)
      .or_else(|| palette_named(theme, "red")),
    ghostty
      .palette_color(2)
      .or_else(|| palette_named(theme, "green")),
    ghostty
      .palette_color(3)
      .or_else(|| palette_named(theme, "yellow")),
    ghostty
      .palette_color(4)
      .or_else(|| palette_named(theme, "blue")),
    ghostty
      .palette_color(5)
      .or_else(|| palette_named(theme, "magenta")),
    ghostty
      .palette_color(6)
      .or_else(|| palette_named(theme, "cyan")),
    ghostty
      .palette_color(7)
      .or_else(|| palette_named(theme, "gray")),
    ghostty
      .palette_color(8)
      .or_else(|| palette_named(theme, "light-red")),
    ghostty
      .palette_color(9)
      .or_else(|| palette_named(theme, "light-green")),
    ghostty
      .palette_color(10)
      .or_else(|| palette_named(theme, "light-yellow")),
    ghostty
      .palette_color(11)
      .or_else(|| palette_named(theme, "light-blue")),
    ghostty
      .palette_color(12)
      .or_else(|| palette_named(theme, "light-magenta")),
    ghostty
      .palette_color(13)
      .or_else(|| palette_named(theme, "light-cyan")),
    ghostty
      .palette_color(14)
      .or_else(|| palette_named(theme, "light-gray")),
    ghostty
      .palette_color(15)
      .or_else(|| palette_named(theme, "white")),
  ];
  ffi::GhosttyThemeSnapshot {
    background:           optional_color_snapshot(background),
    foreground:           optional_color_snapshot(foreground),
    cursor_color:         optional_color_snapshot(cursor_color),
    cursor_text:          optional_color_snapshot(cursor_text),
    selection_background: optional_color_snapshot(selection_background),
    selection_foreground: optional_color_snapshot(selection_foreground),
    palette0:             optional_color_snapshot(derived_palette[0]),
    palette1:             optional_color_snapshot(derived_palette[1]),
    palette2:             optional_color_snapshot(derived_palette[2]),
    palette3:             optional_color_snapshot(derived_palette[3]),
    palette4:             optional_color_snapshot(derived_palette[4]),
    palette5:             optional_color_snapshot(derived_palette[5]),
    palette6:             optional_color_snapshot(derived_palette[6]),
    palette7:             optional_color_snapshot(derived_palette[7]),
    palette8:             optional_color_snapshot(derived_palette[8]),
    palette9:             optional_color_snapshot(derived_palette[9]),
    palette10:            optional_color_snapshot(derived_palette[10]),
    palette11:            optional_color_snapshot(derived_palette[11]),
    palette12:            optional_color_snapshot(derived_palette[12]),
    palette13:            optional_color_snapshot(derived_palette[13]),
    palette14:            optional_color_snapshot(derived_palette[14]),
    palette15:            optional_color_snapshot(derived_palette[15]),
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

  use ropey::Rope;
  use serde_json::{
    Value,
    json,
  };
  use the_default::{
    Command,
    CommandEvent,
    CommandRegistry,
    DefaultContext,
    Direction as CommandDirection,
    Mode,
    Modifiers as KeyModifiers,
    PendingInput,
    SearchPromptKind,
  };
  use the_lib::{
    clipboard::NoClipboard,
    messages::MessageEventKind,
    movement::Direction as SelectionDirection,
    position::{
      Position as LibPosition,
      char_idx_at_coords,
      coords_at_pos,
    },
    render::RenderStyles,
    selection::{
      Range,
      Selection,
    },
    syntax::Highlight,
    transaction::Transaction,
    view::ViewState,
  };
  use the_lsp::{
    LspLocation,
    LspPosition,
    LspRange,
    LspRuntime,
  };
  use the_runtime::file_watch::{
    PathEvent,
    PathEventKind,
  };
  use the_vcs::{
    FileChange,
    VcsStatuslineInfo,
  };

  use super::{
    App,
    DiagnosticSeverity,
    InlineDiagnosticRenderLine,
    LibStyle,
    PendingAutoSignatureHelp,
    SignatureHelpTriggerSource,
    VcsFileStatusKind,
    VcsUiState,
    build_diagnostic_popup_state,
    build_lsp_document_state,
    capabilities_support_single_char,
    dedupe_inline_diagnostic_lines,
    ffi,
    format_vcs_statusline_text,
  };
  use crate::HoverTriggerSource;

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
  fn vcs_ui_state_tracks_statuses_and_collapsed_directory_counts() {
    let state = VcsUiState::from_changes(vec![
      FileChange::Modified {
        path: PathBuf::from("/tmp/the-editor-vcs/src/main.c"),
      },
      FileChange::Untracked {
        path: PathBuf::from("/tmp/the-editor-vcs/src/new.c"),
      },
      FileChange::Conflict {
        path: PathBuf::from("/tmp/the-editor-vcs/README.md"),
      },
      FileChange::Renamed {
        from_path: PathBuf::from("/tmp/the-editor-vcs/old.rs"),
        to_path:   PathBuf::from("/tmp/the-editor-vcs/src/renamed.rs"),
      },
    ]);

    assert_eq!(
      state.status_for_path(Path::new("/tmp/the-editor-vcs/src/main.c")),
      VcsFileStatusKind::Modified
    );
    assert_eq!(
      state.status_for_path(Path::new("/tmp/the-editor-vcs/src/new.c")),
      VcsFileStatusKind::Untracked
    );
    assert_eq!(
      state.status_for_path(Path::new("/tmp/the-editor-vcs/src/renamed.rs")),
      VcsFileStatusKind::Renamed
    );
    assert_eq!(
      state.status_for_path(Path::new("/tmp/the-editor-vcs/old.rs")),
      VcsFileStatusKind::None
    );
    assert_eq!(
      state.changed_descendant_count(Path::new("/tmp/the-editor-vcs/src")),
      3
    );
    assert_eq!(
      state.changed_descendant_count(Path::new("/tmp/the-editor-vcs")),
      4
    );
    assert_eq!(state.counts.summary_text().as_deref(), Some("U1 M1 R1 ?1"));
  }

  #[test]
  fn format_vcs_statusline_text_returns_branch_only() {
    let info = VcsStatuslineInfo::Git {
      branch: "main".to_string(),
    };

    assert_eq!(format_vcs_statusline_text(&info), "main");
  }

  #[test]
  fn frame_render_plan_exposes_terminal_pane_metadata() {
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

    let terminal_id = app.active_editor_mut().open_terminal_in_active_pane();
    let frame = app.frame_render_plan(id);
    assert_eq!(frame.pane_count(), 1);
    let pane = frame.pane_at(0);
    assert!(pane.is_active());
    assert_eq!(pane.pane_kind(), 1);
    assert_eq!(pane.terminal_id(), terminal_id.get().get() as u64);
    assert_eq!(pane.plan().line_count(), 0);
  }

  #[test]
  fn editor_surface_snapshots_expose_editor_pane_metadata() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("one", default_viewport(), ffi::Position { row: 0, col: 0 });

    assert!(app.split_active_pane(id, 0));
    let viewport = app.active_editor_ref().layout_viewport();
    let second_index = app.active_editor_mut().open_buffer(
      Rope::from("two"),
      ViewState::new(viewport, LibPosition::new(0, 0)),
      Some(PathBuf::from("/tmp/project/src/two.rs")),
    );

    assert_eq!(app.editor_surface_count(id), 2);
    let mut seen_indices = Vec::new();
    let mut active_count = 0usize;
    for index in 0..app.editor_surface_count(id) {
      let snapshot = app.editor_surface_at(id, index);
      seen_indices.push(snapshot.buffer_index());
      if snapshot.is_active() {
        active_count += 1;
      }
      if snapshot.buffer_index() == second_index {
        assert_eq!(snapshot.file_path(), "/tmp/project/src/two.rs");
      }
    }
    assert_eq!(active_count, 1);
    assert!(seen_indices.contains(&0));
    assert!(seen_indices.contains(&second_index));
  }

  #[test]
  fn split_and_jump_active_pane_through_ffi() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("hello", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });

    assert!(app.split_active_pane(id, 0));
    let frame = app.frame_render_plan(id);
    assert_eq!(frame.pane_count(), 2);
    let active_after_split = frame.active_pane_id();

    assert!(app.jump_active_pane(id, 0));
    let frame = app.frame_render_plan(id);
    assert_eq!(frame.pane_count(), 2);
    assert_ne!(frame.active_pane_id(), active_after_split);
  }

  #[test]
  fn open_untitled_buffer_preserves_active_terminal_pane() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("hello", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });

    assert!(app.open_terminal_in_active_pane(id));
    let terminal_id = {
      let frame = app.frame_render_plan(id);
      assert_eq!(frame.pane_count(), 1);
      frame.pane_at(0).terminal_id()
    };

    let opened_buffer = app.open_untitled_buffer(id);
    assert_ne!(opened_buffer, 0);
    assert!(!App::is_active_pane_terminal(&mut app, id));

    let frame = app.frame_render_plan(id);
    assert_eq!(frame.pane_count(), 2);
    let mut terminal_count = 0usize;
    let mut found_original_terminal = false;
    for index in 0..frame.pane_count() {
      let pane = frame.pane_at(index);
      if pane.pane_kind() == 1 {
        terminal_count += 1;
        if pane.terminal_id() == terminal_id {
          found_original_terminal = true;
        }
      }
    }
    assert_eq!(terminal_count, 1);
    assert!(found_original_terminal);
  }

  #[test]
  fn open_file_path_replaces_active_terminal_pane() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });
    let fixture = TempTestFile::new("terminal-open-file-preserve", "alpha\nbeta\n");

    assert!(app.open_terminal_in_active_pane(id));
    let terminal_id = app.active_editor_ref().active_terminal_id().unwrap();

    assert!(app.open_file_path(id, fixture.as_path().to_string_lossy().as_ref()));
    assert!(!App::is_active_pane_terminal(&mut app, id));
    assert_eq!(app.text(id).as_str(), "alpha\nbeta\n");

    let frame = app.frame_render_plan(id);
    assert_eq!(frame.pane_count(), 1);
    assert_eq!(app.terminal_surface_count(id), 1);
    let snapshot = app.terminal_surface_at(id, 0);
    assert_eq!(snapshot.terminal_id(), terminal_id.get().get() as u64);
    assert_eq!(snapshot.pane_id(), 0);
    assert!(!snapshot.is_active());
  }

  #[test]
  fn open_file_path_from_terminal_reuses_existing_buffer_in_same_pane() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });
    let fixture = TempTestFile::new("terminal-open-file-existing", "alpha\nbeta\n");

    assert!(app.open_file_path(id, fixture.as_path().to_string_lossy().as_ref()));
    let original_buffer = app.active_editor_ref().active_buffer_index();
    assert!(app.open_terminal_in_active_pane(id));

    assert!(app.open_file_path(id, fixture.as_path().to_string_lossy().as_ref()));
    assert!(!App::is_active_pane_terminal(&mut app, id));
    assert_eq!(
      app.active_editor_ref().active_buffer_index(),
      original_buffer
    );

    let frame = app.frame_render_plan(id);
    assert_eq!(frame.pane_count(), 1);
    assert_eq!(app.terminal_surface_count(id), 1);
    assert!(!app.terminal_surface_at(id, 0).is_active());
  }

  #[test]
  fn terminal_surface_snapshot_persists_when_terminal_detaches() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("hello", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });

    let active_pane = app.active_editor_ref().active_pane_id();
    let terminal_id = app.active_editor_mut().open_terminal_in_active_pane();
    assert!(
      app
        .active_editor_mut()
        .set_active_buffer_in_pane(active_pane, 0)
    );

    assert_eq!(app.terminal_surface_count(id), 1);
    let snapshot = app.terminal_surface_at(id, 0);
    assert_eq!(snapshot.terminal_id(), terminal_id.get().get() as u64);
    assert_eq!(snapshot.pane_id(), 0);
    assert!(!snapshot.is_active());

    assert!(app.focus_terminal_surface(id, terminal_id.get().get() as u64));
    let snapshot = app.terminal_surface_at(id, 0);
    assert_ne!(snapshot.pane_id(), 0);
    assert!(snapshot.is_active());

    let frame = app.frame_render_plan(id);
    assert_eq!(frame.pane_count(), 1);
    let pane = frame.pane_at(0);
    assert_eq!(pane.pane_kind(), 1);
    assert_eq!(pane.terminal_id(), terminal_id.get().get() as u64);
  }

  #[test]
  fn hide_active_terminal_surface_detaches_without_destroying_surface() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("hello", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });

    let terminal_id = app.active_editor_mut().open_terminal_in_active_pane();
    assert!(app.hide_active_terminal_surface(id));
    assert!(!App::is_active_pane_terminal(&mut app, id));
    assert_eq!(app.terminal_surface_count(id), 1);
    let snapshot = app.terminal_surface_at(id, 0);
    assert_eq!(snapshot.terminal_id(), terminal_id.get().get() as u64);
    assert_eq!(snapshot.pane_id(), 0);
    assert!(!snapshot.is_active());
  }

  #[test]
  fn execute_command_named_runs_default_commands() {
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

    assert!(app.execute_command_named(id, "file_picker"));
    assert!(app.file_picker_snapshot(id, 128).active());
    assert!(app.command_palette_close(id));

    let initial_tree_visible = app.file_tree_snapshot(id, 128).visible();
    assert!(app.execute_command_named(id, "file_explorer"));
    assert_ne!(
      app.file_tree_snapshot(id, 128).visible(),
      initial_tree_visible
    );

    assert!(app.execute_command_named(id, "terminal_open"));
    assert!(App::is_active_pane_terminal(&mut app, id));
    assert!(app.execute_command_named(id, "terminal_close"));
    assert!(!App::is_active_pane_terminal(&mut app, id));
  }

  #[test]
  fn execute_command_named_opens_command_palette() {
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

    assert!(app.execute_command_named(id, "command_palette"));
    assert_eq!(app.mode(id), 3);
    assert!(app.command_palette_is_open(id));
    assert!(matches!(
      app.active_state_ref().command_palette.source,
      the_default::CommandPaletteSource::CommandLine
    ));

    assert!(!app.execute_command_named(id, "does_not_exist"));
  }

  #[test]
  fn file_picker_window_snapshot_tracks_window_start_and_selection() {
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

    assert!(app.execute_command_named(id, "file_picker"));
    let full = app.file_picker_snapshot(id, 8);
    assert!(full.active());
    assert_eq!(full.window_start(), 0);
    assert_eq!(full.selected_index(), 0);

    let windowed = app.file_picker_window_snapshot(id, 3, 5);
    assert!(windowed.active());
    assert_eq!(windowed.window_start(), 3);
    assert_eq!(windowed.selected_index(), 0);
    assert!(windowed.item_count() <= 5);
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
  fn diagnostic_popup_markdown_includes_metadata() {
    let popup = build_diagnostic_popup_state(&[the_lib::diagnostics::Diagnostic {
      range:    the_lib::diagnostics::DiagnosticRange {
        start: the_lib::diagnostics::DiagnosticPosition {
          line:      3,
          character: 5,
        },
        end:   the_lib::diagnostics::DiagnosticPosition {
          line:      3,
          character: 8,
        },
      },
      severity: Some(DiagnosticSeverity::Error),
      code:     Some("typecheck_call_too_few_args".to_string()),
      source:   Some("clang".to_string()),
      message:  "Too few arguments to function call".to_string(),
    }])
    .expect("diagnostic popup");

    assert!(popup.markdown.contains("### Error"));
    assert!(
      popup
        .markdown
        .contains("Too few arguments to function call")
    );
    assert!(popup.markdown.contains("Source: `clang`"));
    assert!(
      popup
        .markdown
        .contains("Code: `typecheck_call_too_few_args`")
    );
  }

  #[test]
  fn lsp_hover_seeds_diagnostic_popup_without_hover_support() {
    let _guard = ffi_test_guard();
    let workspace = TempTestWorkspace::new("hover-diagnostic", "main.c", "add()\n");
    let mut app = App::new();
    let id = app.create_editor("add()\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    install_test_lsp_state(&mut app, id, workspace.file_path());
    app.lsp_ready = true;
    app.lsp_document.as_mut().expect("lsp document").opened = true;

    let uri = app
      .lsp_document
      .as_ref()
      .map(|state| state.uri.clone())
      .expect("lsp document uri");
    let _ = app
      .diagnostics
      .apply_document(the_lib::diagnostics::DocumentDiagnostics {
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
              character: 3,
            },
          },
          severity: Some(DiagnosticSeverity::Error),
          code:     Some("call_too_few_args".to_string()),
          source:   Some("clang".to_string()),
          message:  "Too few arguments to function call".to_string(),
        }],
      });

    let before_seq = app.active_state_ref().messages.latest_seq();
    app.lsp_hover();

    assert!(app.diagnostic_popup_text().is_some());
    assert!(app.hover_docs_text().is_none());
    assert!(
      app
        .active_state_ref()
        .messages
        .events_since(before_seq)
        .is_empty()
    );
  }

  #[test]
  fn empty_hover_response_keeps_diagnostic_popup_without_info_message() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });
    assert!(app.activate(id).is_some());

    app.active_state_mut().diagnostic_popup =
      build_diagnostic_popup_state(&[the_lib::diagnostics::Diagnostic {
        range:    the_lib::diagnostics::DiagnosticRange {
          start: the_lib::diagnostics::DiagnosticPosition {
            line:      0,
            character: 0,
          },
          end:   the_lib::diagnostics::DiagnosticPosition {
            line:      0,
            character: 1,
          },
        },
        severity: Some(DiagnosticSeverity::Warning),
        code:     None,
        source:   Some("swift".to_string()),
        message:  "Expected ';' at end of declaration".to_string(),
      }]);

    let before_seq = app.active_state_ref().messages.latest_seq();
    let generation = app.lsp_hover_generation;
    app.apply_hover_response(None, generation, HoverTriggerSource::Keyboard, 0, None);

    assert!(app.diagnostic_popup_text().is_some());
    assert!(app.hover_docs_text().is_none());
    assert!(
      app
        .active_state_ref()
        .messages
        .events_since(before_seq)
        .is_empty()
    );
  }

  #[test]
  fn empty_mouse_hover_response_clears_hover_ui_without_diagnostics() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("hello", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    assert!(app.set_hover_ui_state(HoverTriggerSource::Mouse, 1, Some((0, 5))));

    let generation = app.lsp_hover_generation;
    app.apply_hover_response(None, generation, HoverTriggerSource::Mouse, 1, Some((0, 5)));

    assert!(app.hover_docs_text().is_none());
    assert!(app.diagnostic_popup_text().is_none());
    assert!(app.active_state_ref().hover_ui.is_none());
  }

  #[test]
  fn keyboard_cursor_move_closes_visible_mouse_hover() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("printf\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    assert!(app.set_hover_ui_state(HoverTriggerSource::Mouse, 1, Some((0, 6))));
    let before = app.active_editor_ref().document().selection().clone();

    assert!(app.handle_key(id, key_char('l')));

    assert_ne!(app.active_editor_ref().document().selection(), &before);
    assert!(app.active_state_ref().hover_ui.is_none());
  }

  #[test]
  fn mouse_move_within_visible_hover_range_preserves_mouse_hover() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("printf\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    assert!(app.set_hover_ui_state(HoverTriggerSource::Mouse, 1, Some((0, 6))));
    let pane_id = app.active_editor_ref().active_pane_id().get().get() as u64;

    let move_event = the_default::PointerEvent::new(the_default::PointerKind::Move, 0, 0)
      .with_logical_pos(2, 0)
      .with_surface_id(pane_id);

    assert_eq!(
      app.pointer_event(move_event),
      the_default::PointerEventOutcome::Continue
    );
    assert!(
      app
        .active_state_ref()
        .hover_ui
        .is_some_and(|hover_ui| hover_ui.trigger == HoverTriggerSource::Mouse)
    );
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
  fn command_palette_keeps_argument_mode_when_open_has_no_matches() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });

    assert!(app.handle_key(id, key_char(':')));
    assert!(app.command_palette_set_query(id, "e definitely_missing_file_name_12345.c"));

    assert_eq!(app.command_palette_query(id), "");
    assert_eq!(app.command_palette_filtered_count(id), 0);
    assert_eq!(app.command_palette_filtered_selected_index(id), -1);
  }

  #[test]
  fn theme_command_preview_updates_effective_theme_and_reverts_on_close() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });

    assert_eq!(app.theme_effective_name(), "default");
    assert!(app.handle_key(id, key_char(':')));
    assert!(app.command_palette_set_query(id, "theme base16"));
    assert!(app.command_palette_filtered_count(id) > 0);
    assert!(app.command_palette_select_filtered(id, 0));
    assert_eq!(app.theme_effective_name(), "base16_default");

    assert!(app.command_palette_close(id));
    assert_eq!(app.theme_effective_name(), "default");
  }

  #[test]
  fn theme_command_validate_commits_selected_theme() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });

    assert!(app.handle_key(id, key_char(':')));
    assert!(app.command_palette_set_query(id, "theme base16"));
    assert!(app.command_palette_select_filtered(id, 0));
    assert!(app.command_palette_submit_filtered(id, 0));
    assert_eq!(app.theme_effective_name(), "base16_default");
  }

  #[test]
  fn theme_command_preview_updates_ghostty_snapshot() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });

    let default_snapshot = app.theme_ghostty_snapshot();
    assert!(default_snapshot.background.has_value);

    assert!(app.handle_key(id, key_char(':')));
    assert!(app.command_palette_set_query(id, "theme base16"));
    assert!(app.command_palette_filtered_count(id) > 0);
    assert!(app.command_palette_select_filtered(id, 0));

    let preview_snapshot = app.theme_ghostty_snapshot();
    assert!(preview_snapshot.background.has_value);
    assert_ne!(
      default_snapshot.background.color.value,
      preview_snapshot.background.color.value
    );
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

  struct TempTestWorkspace {
    root: PathBuf,
    file: PathBuf,
  }

  impl TempTestWorkspace {
    fn new(prefix: &str, file_name: &str, content: &str) -> Self {
      let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
      let root = std::env::temp_dir().join(format!(
        "the-editor-ffi-workspace-{prefix}-{}-{nonce}",
        std::process::id()
      ));
      fs::create_dir_all(root.join(".the-editor")).expect("create workspace marker");
      let file = root.join(file_name);
      fs::write(&file, content).expect("write workspace file");
      Self { root, file }
    }

    fn root_path(&self) -> &Path {
      &self.root
    }

    fn file_path(&self) -> &Path {
      &self.file
    }
  }

  impl Drop for TempTestWorkspace {
    fn drop(&mut self) {
      let _ = fs::remove_dir_all(&self.root);
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

  fn key_char_alt(ch: char) -> ffi::KeyEvent {
    ffi::KeyEvent {
      kind:      0,
      codepoint: ch as u32,
      modifiers: KeyModifiers::ALT,
    }
  }

  fn key_char_ctrl(ch: char) -> ffi::KeyEvent {
    ffi::KeyEvent {
      kind:      0,
      codepoint: ch as u32,
      modifiers: KeyModifiers::CTRL,
    }
  }

  fn key_backspace() -> ffi::KeyEvent {
    ffi::KeyEvent {
      kind:      4,
      codepoint: 0,
      modifiers: 0,
    }
  }

  fn key_delete() -> ffi::KeyEvent {
    ffi::KeyEvent {
      kind:      6,
      codepoint: 0,
      modifiers: 0,
    }
  }

  fn key_left() -> ffi::KeyEvent {
    ffi::KeyEvent {
      kind:      12,
      codepoint: 0,
      modifiers: 0,
    }
  }

  fn install_test_lsp_state(app: &mut App, id: ffi::EditorId, path: &Path) {
    assert!(app.activate(id).is_some());
    app
      .active_editor_mut()
      .set_active_file_path(Some(path.to_path_buf()));
    let (config, _configured) = app.lsp_runtime_config_for_active_file();
    app.lsp_runtime = LspRuntime::new(config);
    app.lsp_server_name = app
      .lsp_runtime
      .config()
      .server()
      .map(|server| server.name().to_string());
    app.lsp_document = build_lsp_document_state(path, app.loader.as_deref());
  }

  fn statusline_left_and_segments(
    app: &mut App,
    id: ffi::EditorId,
  ) -> Option<(String, Vec<String>)> {
    let tree: Value = serde_json::from_str(app.ui_tree_json(id).as_str()).ok()?;
    let overlays = tree.get("overlays")?.as_array()?;
    for overlay in overlays {
      let Some("panel") = overlay.get("type").and_then(Value::as_str) else {
        continue;
      };
      let Some(panel) = overlay.get("data") else {
        continue;
      };
      if panel.get("id").and_then(Value::as_str) != Some("statusline") {
        continue;
      }

      let child = panel.get("child")?;
      if child.get("type").and_then(Value::as_str) != Some("status_bar") {
        continue;
      }
      let status = child.get("data")?;
      let left = status.get("left")?.as_str()?.to_string();
      let right_segments = status
        .get("right_segments")
        .and_then(Value::as_array)
        .map(|segments| {
          segments
            .iter()
            .filter_map(|segment| segment.get("text").and_then(Value::as_str))
            .map(str::to_string)
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();
      return Some((left, right_segments));
    }
    None
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
  fn lsp_goto_variant_keymaps_emit_errors_when_unavailable() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });
    assert!(app.activate(id).is_some());

    for (suffix, expected) in [
      ('D', "No declaration found."),
      ('y', "No type definition found."),
      ('i', "No implementation found."),
    ] {
      let before_seq = app.active_state_ref().messages.latest_seq();
      assert!(app.handle_key(id, key_char('g')));
      assert!(app.handle_key(id, key_char(suffix)));

      let events = app.active_state_ref().messages.events_since(before_seq);
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
  fn lsp_jump_saves_origin_for_jumplist_back_navigation() {
    let _guard = ffi_test_guard();
    let first = TempTestFile::new("lsp-jump-origin", "first file\n");
    let second = TempTestFile::new("lsp-jump-target", "second file\n");

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

    let origin = 3;
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::point(origin));

    let uri = the_lsp::text_sync::file_uri_for_path(second.as_path()).expect("file uri");
    assert!(app.jump_to_location(&LspLocation {
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
    assert_eq!(
      <App as DefaultContext>::file_path(&app),
      Some(second.as_path())
    );

    assert!(app.handle_key(id, key_char_ctrl('o')));
    assert_eq!(
      <App as DefaultContext>::file_path(&app),
      Some(first.as_path())
    );
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::point(origin)
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
  fn goto_motion_keymaps_save_jumps_for_file_edges_and_column() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("alpha\nbeta\ngamma\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());

    let set_cursor_at = |app: &mut App, row: usize, col: usize| {
      let pos = {
        let text = app.active_editor_ref().document().text().slice(..);
        char_idx_at_coords(text, LibPosition::new(row, col))
      };
      let _ = app
        .active_editor_mut()
        .document_mut()
        .set_selection(Selection::point(pos));
      pos
    };

    let ge_origin = set_cursor_at(&mut app, 1, 1);
    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char('e')));
    assert!(app.handle_key(id, key_char_ctrl('o')));
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::point(ge_origin)
    );

    let gg_origin = set_cursor_at(&mut app, 2, 2);
    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char_ctrl('o')));
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::point(gg_origin)
    );

    let gbar_origin = set_cursor_at(&mut app, 1, 3);
    assert!(app.handle_key(id, key_char('g')));
    assert!(app.handle_key(id, key_char('|')));
    assert!(app.handle_key(id, key_char_ctrl('o')));
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::point(gbar_origin)
    );
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
  fn page_cursor_half_down_keymap_moves_by_half_viewport_height() {
    let _guard = ffi_test_guard();
    let mut content = String::new();
    for line in 0..64usize {
      content.push_str(&format!("line-{line}\n"));
    }

    let mut app = App::new();
    let id = app.create_editor(&content, default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());

    let start = {
      let text = app.active_editor_ref().document().text().slice(..);
      char_idx_at_coords(text, LibPosition::new(5, 0))
    };
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::point(start));

    assert!(app.handle_key(id, key_char_ctrl('d')));

    let row = {
      let text = app.active_editor_ref().document().text().slice(..);
      let head = app.active_editor_ref().document().selection().ranges()[0].head;
      coords_at_pos(text, head).row
    };
    assert_eq!(row, 17);
  }

  #[test]
  fn page_cursor_half_up_keymap_moves_by_half_viewport_height() {
    let _guard = ffi_test_guard();
    let mut content = String::new();
    for line in 0..64usize {
      content.push_str(&format!("line-{line}\n"));
    }

    let mut app = App::new();
    let id = app.create_editor(&content, default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());

    let start = {
      let text = app.active_editor_ref().document().text().slice(..);
      char_idx_at_coords(text, LibPosition::new(20, 0))
    };
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::point(start));

    assert!(app.handle_key(id, key_char_ctrl('u')));

    let row = {
      let text = app.active_editor_ref().document().text().slice(..);
      let head = app.active_editor_ref().document().selection().ranges()[0].head;
      coords_at_pos(text, head).row
    };
    assert_eq!(row, 8);
  }

  #[test]
  fn copy_selection_on_next_line_keeps_single_line_height_at_line_start() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor(
      "zero\none\ntwo\nthree\n",
      default_viewport(),
      ffi::Position { row: 0, col: 0 },
    );
    assert!(app.activate(id).is_some());

    let line_start = app.active_editor_ref().document().text().line_to_char(1);
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::point(line_start));

    assert!(app.handle_key(id, key_char('C')));

    let text = app.active_editor_ref().document().text().slice(..);
    let rows: Vec<_> = app
      .active_editor_ref()
      .document()
      .selection()
      .ranges()
      .iter()
      .map(|range| coords_at_pos(text, range.cursor(text)).row)
      .collect();
    assert_eq!(rows, vec![1, 2]);
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
  fn split_selection_keymap_uses_split_prompt_and_partitions_selection() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("alpha,beta,gamma\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    let split_end = "alpha,beta,gamma".chars().count();
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(0, split_end));

    assert!(app.handle_key(id, key_char('S')));
    assert!(app.active_state_ref().search_prompt.active);
    assert_eq!(
      app.active_state_ref().search_prompt.kind,
      SearchPromptKind::SplitSelection
    );

    assert!(app.handle_key(id, key_char(',')));
    assert_eq!(
      app
        .active_editor_ref()
        .document()
        .selection()
        .ranges()
        .len(),
      3
    );

    assert!(app.handle_key(id, ffi::KeyEvent {
      kind:      1,
      codepoint: 0,
      modifiers: 0,
    }));
    assert!(!app.active_state_ref().search_prompt.active);
    assert_eq!(
      app
        .active_editor_ref()
        .document()
        .selection()
        .ranges()
        .len(),
      3
    );
  }

  #[test]
  fn join_selections_keymap_joins_lines() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("alpha\nbeta\ngamma\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    let join_end = "alpha\nbeta\ngamma".chars().count();
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(0, join_end));

    assert!(app.handle_key(id, key_char('J')));
    assert_eq!(
      app.active_editor_ref().document().text().to_string(),
      "alpha beta gamma\n"
    );
  }

  #[test]
  fn join_selections_space_keymap_selects_inserted_space() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("alpha\nbeta\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    let join_end = "alpha\nbeta".chars().count();
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(0, join_end));

    assert!(app.handle_key(id, key_char_alt('J')));
    assert_eq!(
      app.active_editor_ref().document().text().to_string(),
      "alpha beta\n"
    );
    assert_eq!(app.active_editor_ref().document().selection().ranges(), &[
      Range::point("alpha".chars().count())
    ]);
  }

  #[test]
  fn keep_selections_keymap_uses_prompt_to_filter_selection() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("one two three\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    let select_end = "one two three".chars().count();
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(0, select_end));
    assert!(app.handle_key(id, key_char('S')));
    assert!(app.handle_key(id, key_char(' ')));
    assert!(app.handle_key(id, ffi::KeyEvent {
      kind:      1,
      codepoint: 0,
      modifiers: 0,
    }));
    assert_eq!(
      app
        .active_editor_ref()
        .document()
        .selection()
        .ranges()
        .len(),
      3
    );

    assert!(app.handle_key(id, key_char('K')));
    assert!(app.active_state_ref().search_prompt.active);
    assert_eq!(
      app.active_state_ref().search_prompt.kind,
      SearchPromptKind::KeepSelections
    );

    assert!(app.handle_key(id, key_char('o')));
    let text = app.active_editor_ref().document().text().slice(..);
    let fragments: Vec<_> = app
      .active_editor_ref()
      .document()
      .selection()
      .fragments(text)
      .map(|fragment| fragment.into_owned())
      .collect();
    assert_eq!(fragments, vec!["one".to_string(), "two".to_string()]);

    assert!(app.handle_key(id, ffi::KeyEvent {
      kind:      1,
      codepoint: 0,
      modifiers: 0,
    }));
    assert!(!app.active_state_ref().search_prompt.active);
    assert_eq!(
      app
        .active_editor_ref()
        .document()
        .selection()
        .ranges()
        .len(),
      2
    );
  }

  #[test]
  fn remove_selections_keymap_uses_prompt_to_filter_selection() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("one two three\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    let select_end = "one two three".chars().count();
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(0, select_end));
    assert!(app.handle_key(id, key_char('S')));
    assert!(app.handle_key(id, key_char(' ')));
    assert!(app.handle_key(id, ffi::KeyEvent {
      kind:      1,
      codepoint: 0,
      modifiers: 0,
    }));
    assert_eq!(
      app
        .active_editor_ref()
        .document()
        .selection()
        .ranges()
        .len(),
      3
    );

    assert!(app.handle_key(id, key_char_alt('K')));
    assert!(app.active_state_ref().search_prompt.active);
    assert_eq!(
      app.active_state_ref().search_prompt.kind,
      SearchPromptKind::RemoveSelections
    );

    assert!(app.handle_key(id, key_char('o')));
    let text = app.active_editor_ref().document().text().slice(..);
    let fragments: Vec<_> = app
      .active_editor_ref()
      .document()
      .selection()
      .fragments(text)
      .map(|fragment| fragment.into_owned())
      .collect();
    assert_eq!(fragments, vec!["three".to_string()]);

    assert!(app.handle_key(id, ffi::KeyEvent {
      kind:      1,
      codepoint: 0,
      modifiers: 0,
    }));
    assert!(!app.active_state_ref().search_prompt.active);
    assert_eq!(
      app
        .active_editor_ref()
        .document()
        .selection()
        .ranges()
        .len(),
      1
    );
  }

  #[test]
  fn clipboard_yank_keymaps_write_to_system_register() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    app
      .registers
      .set_clipboard_provider(std::sync::Arc::new(NoClipboard));
    let id = app.create_editor("alpha beta\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());

    let selection = Selection::single(0, 5).push(Range::new(6, 10));
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(selection);

    assert!(app.handle_key(id, key_char(' ')));
    assert!(app.handle_key(id, key_char('y')));

    let values: Vec<_> = app
      .registers
      .read('+', app.active_editor_ref().document())
      .expect("clipboard register")
      .map(|value| value.into_owned())
      .collect();
    assert_eq!(values, vec!["alpha".to_string(), "beta".to_string()]);

    let selection = Selection::single(0, 5).push(Range::new(6, 10));
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(selection);
    let second = app.active_editor_ref().document().selection().cursor_ids()[1];
    app.active_editor_mut().view_mut().active_cursor = Some(second);

    assert!(app.handle_key(id, key_char(' ')));
    assert!(app.handle_key(id, key_char('Y')));

    let values: Vec<_> = app
      .registers
      .read('+', app.active_editor_ref().document())
      .expect("clipboard register")
      .map(|value| value.into_owned())
      .collect();
    assert_eq!(values, vec!["beta".to_string()]);
  }

  #[test]
  fn clipboard_paste_and_replace_keymaps_use_system_register() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    app
      .registers
      .set_clipboard_provider(std::sync::Arc::new(NoClipboard));
    let id = app.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });
    assert!(app.activate(id).is_some());
    let _ = app.registers.write('+', vec!["Z".to_string()]);

    let reset_text = |app: &mut App| {
      let tx = Transaction::change(
        app.active_editor_ref().document().text(),
        std::iter::once((
          0,
          app.active_editor_ref().document().text().len_chars(),
          Some("abc\n".into()),
        )),
      )
      .expect("seed transaction");
      assert!(<App as DefaultContext>::apply_transaction(app, &tx));
    };

    reset_text(&mut app);
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(0, 1));
    assert!(app.handle_key(id, key_char(' ')));
    assert!(app.handle_key(id, key_char('p')));
    assert_eq!(
      app.active_editor_ref().document().text().to_string(),
      "aZbc\n"
    );

    reset_text(&mut app);
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(0, 1));
    assert!(app.handle_key(id, key_char(' ')));
    assert!(app.handle_key(id, key_char('P')));
    assert_eq!(
      app.active_editor_ref().document().text().to_string(),
      "Zabc\n"
    );

    reset_text(&mut app);
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(1, 2));
    assert!(app.handle_key(id, key_char(' ')));
    assert!(app.handle_key(id, key_char('R')));
    assert_eq!(
      app.active_editor_ref().document().text().to_string(),
      "aZc\n"
    );
  }

  #[test]
  fn keep_active_selection_keymap_collapses_to_picked_cursor() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("a\nb\nc\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());

    let text = app.active_editor_ref().document().text().clone();
    let selection = Selection::point(text.line_to_char(0))
      .push(Range::point(text.line_to_char(1)))
      .push(Range::point(text.line_to_char(2)));
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(selection);

    assert!(app.handle_key(id, key_char(',')));
    let (left, right_segments) =
      statusline_left_and_segments(&mut app, id).expect("statusline should be present");
    assert!(
      left.starts_with("COL "),
      "unexpected statusline left while picking cursor: {left}"
    );
    assert!(
      right_segments
        .iter()
        .any(|segment| segment == "collapse 1/3"),
      "missing collapse segment in statusline: {right_segments:?}"
    );
    let candidates = match app.active_state_ref().pending_input.clone() {
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

    assert!(app.handle_key(id, ffi::KeyEvent {
      kind:      15,
      codepoint: 0,
      modifiers: 0,
    }));
    assert!(matches!(
      app.active_state_ref().pending_input.as_ref(),
      Some(PendingInput::CursorPick {
        remove: false,
        index: 1,
        ..
      })
    ));
    assert_eq!(
      app.active_editor_ref().view().active_cursor,
      Some(candidates[1])
    );
    let (_left, right_segments) =
      statusline_left_and_segments(&mut app, id).expect("statusline should remain visible");
    assert!(
      right_segments
        .iter()
        .any(|segment| segment == "collapse 2/3"),
      "missing updated collapse segment in statusline: {right_segments:?}"
    );

    assert!(app.handle_key(id, ffi::KeyEvent {
      kind:      1,
      codepoint: 0,
      modifiers: 0,
    }));
    assert!(app.active_state_ref().pending_input.is_none());
    assert_eq!(
      app
        .active_editor_ref()
        .document()
        .selection()
        .ranges()
        .len(),
      1
    );
    assert_eq!(
      app.active_editor_ref().document().selection().cursor_ids()[0],
      candidates[1]
    );
    assert_eq!(
      app.active_editor_ref().view().active_cursor,
      Some(candidates[1])
    );
  }

  #[test]
  fn cursor_pick_mode_uses_match_cursor_style_for_selected_cursor() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("a\nb\nc\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());

    let text = app.active_editor_ref().document().text().clone();
    let selection = Selection::point(text.line_to_char(0))
      .push(Range::point(text.line_to_char(1)))
      .push(Range::point(text.line_to_char(2)));
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(selection);

    assert!(app.handle_key(id, key_char(',')));
    let active_cursor = app
      .active_editor_ref()
      .view()
      .active_cursor
      .expect("active cursor in cursor-pick mode");

    let plan = the_default::render_plan_with_styles(&mut app, RenderStyles::default());
    let selected = plan
      .cursors
      .iter()
      .find(|cursor| cursor.id == active_cursor)
      .expect("selected cursor should be rendered");
    let expected = app
      .ui_theme
      .try_get("ui.cursor.match")
      .or_else(|| app.ui_theme.try_get("ui.cursor.active"))
      .or_else(|| app.ui_theme.try_get("ui.cursor"))
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
  fn remove_active_selection_keymap_removes_picked_cursor() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("a\nb\nc\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());

    let text = app.active_editor_ref().document().text().clone();
    let selection = Selection::point(text.line_to_char(0))
      .push(Range::point(text.line_to_char(1)))
      .push(Range::point(text.line_to_char(2)));
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(selection);

    assert!(app.handle_key(id, key_char_alt(',')));
    let (left, right_segments) =
      statusline_left_and_segments(&mut app, id).expect("statusline should be present");
    assert!(
      left.starts_with("REM "),
      "unexpected statusline left while removing cursor: {left}"
    );
    assert!(
      right_segments.iter().any(|segment| segment == "remove 1/3"),
      "missing remove segment in statusline: {right_segments:?}"
    );
    let candidates = match app.active_state_ref().pending_input.clone() {
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

    assert!(app.handle_key(id, ffi::KeyEvent {
      kind:      15,
      codepoint: 0,
      modifiers: 0,
    }));
    assert!(app.handle_key(id, ffi::KeyEvent {
      kind:      1,
      codepoint: 0,
      modifiers: 0,
    }));

    assert!(app.active_state_ref().pending_input.is_none());
    assert_eq!(
      app
        .active_editor_ref()
        .document()
        .selection()
        .ranges()
        .len(),
      2
    );
    assert!(
      !app
        .active_editor_ref()
        .document()
        .selection()
        .cursor_ids()
        .contains(&candidates[1])
    );
  }

  #[test]
  fn align_selections_keymap_aligns_columns_and_exits_select_mode() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor(
      "a = 1;\nlong_name = 2;\nmid = 3;\n",
      default_viewport(),
      ffi::Position { row: 0, col: 0 },
    );
    assert!(app.activate(id).is_some());

    let selection = {
      let text = app.active_editor_ref().document().text().slice(..);
      let first = char_idx_at_coords(text, LibPosition::new(0, 4));
      let second = char_idx_at_coords(text, LibPosition::new(1, 12));
      let third = char_idx_at_coords(text, LibPosition::new(2, 6));
      Selection::point(first)
        .push(Range::point(second))
        .push(Range::point(third))
    };
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(selection);
    app.active_state_mut().mode = Mode::Select;

    assert!(app.handle_key(id, key_char('&')));

    assert_eq!(
      app.text(id),
      "a =         1;\nlong_name = 2;\nmid =       3;\n"
    );
    assert_eq!(app.active_state_ref().mode, Mode::Normal);
  }

  #[test]
  fn align_selections_keymap_errors_on_multiline_selection() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("alpha\nbeta\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(0, 6));

    let before_seq = app.active_state_ref().messages.latest_seq();
    assert!(app.handle_key(id, key_char('&')));

    let events = app.active_state_ref().messages.events_since(before_seq);
    let error = events
      .iter()
      .find_map(|event| {
        match &event.kind {
          the_lib::messages::MessageEventKind::Published { message } => {
            (message.level == the_lib::messages::MessageLevel::Error
              && message.source.as_deref() == Some("align"))
            .then_some(message.text.as_str())
          },
          _ => None,
        }
      })
      .expect("align error message");
    assert_eq!(error, "align cannot work with multi line selections");
    assert_eq!(app.text(id), "alpha\nbeta\n");
  }

  #[test]
  fn trim_selections_keymap_trims_ranges_and_drops_whitespace_only() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor(
      "  alpha  \n   \n  beta  \n",
      default_viewport(),
      ffi::Position { row: 0, col: 0 },
    );
    assert!(app.activate(id).is_some());

    let text = app.active_editor_ref().document().text().clone();
    let line0 = text.line_to_char(0);
    let line1 = text.line_to_char(1);
    let line2 = text.line_to_char(2);
    let selection = Selection::single(line0, line0 + "  alpha  ".chars().count())
      .push(Range::new(line1, line1 + 3))
      .push(Range::new(line2, line2 + "  beta  ".chars().count()));
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(selection);

    assert!(app.handle_key(id, key_char('_')));

    let text = app.active_editor_ref().document().text().slice(..);
    let selection = app.active_editor_ref().document().selection();
    let fragments: Vec<_> = selection
      .fragments(text)
      .map(|fragment| fragment.into_owned())
      .collect();
    assert_eq!(fragments, vec!["alpha".to_string(), "beta".to_string()]);
    assert_eq!(selection.ranges().len(), 2);
  }

  #[test]
  fn trim_selections_keymap_collapses_to_active_cursor_when_all_whitespace() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("   \n  \n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());

    let text = app.active_editor_ref().document().text().clone();
    let line0 = text.line_to_char(0);
    let line1 = text.line_to_char(1);
    let selection = Selection::single(line0, line0 + 3).push(Range::new(line1, line1 + 2));
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(selection);

    let active_cursor_id = app.active_editor_ref().document().selection().cursor_ids()[1];
    let active_cursor_point = {
      let text = app.active_editor_ref().document().text().slice(..);
      app.active_editor_ref().document().selection().ranges()[1].cursor(text)
    };
    app.active_editor_mut().view_mut().active_cursor = Some(active_cursor_id);

    assert!(app.handle_key(id, key_char('_')));

    let selection = app.active_editor_ref().document().selection();
    assert_eq!(selection.ranges().len(), 1);
    assert_eq!(selection.cursor_ids()[0], active_cursor_id);
    assert_eq!(selection.ranges()[0], Range::point(active_cursor_point));
  }

  #[test]
  fn insert_mode_key_moves_cursor_to_selection_start() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("printf(\"hello\")\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(0, 6));

    assert!(app.handle_key(id, key_char('i')));
    assert_eq!(app.active_state_ref().mode, Mode::Insert);
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::new(6, 0)
    );

    let plan = app.render_plan(id);
    assert_eq!(plan.cursor_count(), 1);
    assert_eq!(plan.cursor_at(0).kind(), 1);
    assert_eq!(plan.cursor_at(0).pos().col, 0);
  }

  #[test]
  fn insert_mode_mouse_drag_keeps_selection_and_bar_cursor_on_active_edge() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("printf\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    app.set_mode(Mode::Insert);

    let down = the_default::PointerEvent::new(
      the_default::PointerKind::Down(the_default::PointerButton::Left),
      0,
      0,
    )
    .with_logical_pos(0, 0)
    .with_click_count(1);
    assert_eq!(
      app.pointer_event(down),
      the_default::PointerEventOutcome::Handled
    );
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::point(0)
    );

    let drag = the_default::PointerEvent::new(
      the_default::PointerKind::Drag(the_default::PointerButton::Left),
      0,
      0,
    )
    .with_logical_pos(5, 0)
    .with_click_count(1);
    assert_eq!(
      app.pointer_event(drag),
      the_default::PointerEventOutcome::Handled
    );
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::new(0, 6)
    );

    let plan = app.render_plan(id);
    assert_eq!(plan.selection_count(), 1);
    assert_eq!(plan.cursor_count(), 1);
    assert_eq!(plan.cursor_at(0).kind(), 1);
    assert_eq!(plan.cursor_at(0).pos().row, 0);
    assert_eq!(plan.cursor_at(0).pos().col, 6);
  }

  #[test]
  fn insert_mode_typing_inserts_at_selection_active_edge() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("printf\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    app.set_mode(Mode::Insert);
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(0, 6));

    assert!(app.handle_key(id, key_char('x')));
    assert_eq!(app.text(id), "printfx\n");
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::new(0, 7)
    );
  }

  #[test]
  fn insert_mode_typing_moves_bar_cursor_with_backward_selection() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("factorial\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(0, 9));

    assert!(app.handle_key(id, key_char('i')));
    assert_eq!(app.active_state_ref().mode, Mode::Insert);
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::new(9, 0)
    );

    assert!(app.handle_key(id, key_char('x')));
    assert_eq!(app.text(id), "xfactorial\n");
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::new(10, 1)
    );

    let plan = app.render_plan(id);
    assert_eq!(plan.cursor_count(), 1);
    assert_eq!(plan.cursor_at(0).kind(), 1);
    assert_eq!(plan.cursor_at(0).pos().col, 1);
  }

  #[test]
  fn append_mode_typing_inserts_after_cursor() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("factorial()\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(8, 9));

    assert!(app.handle_key(id, key_char('a')));
    assert_eq!(app.active_state_ref().mode, Mode::Insert);
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::new(8, 9)
    );

    assert!(app.handle_key(id, key_char('x')));
    assert_eq!(app.text(id), "factorialx()\n");
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::new(8, 10)
    );
  }

  #[test]
  fn append_mode_with_selection_inserts_at_selection_end() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("main()\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(0, 4));

    assert!(app.handle_key(id, key_char('a')));
    assert_eq!(app.active_state_ref().mode, Mode::Insert);
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::new(0, 4)
    );

    assert!(app.handle_key(id, key_char('x')));
    assert_eq!(app.text(id), "mainx()\n");
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::new(0, 5)
    );
  }

  #[test]
  fn append_mode_with_selection_auto_pair_inserts_at_selection_end() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("printf\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    let _ = app
      .active_editor_mut()
      .document_mut()
      .set_selection(Selection::single(0, 6));

    assert!(app.handle_key(id, key_char('a')));
    assert_eq!(app.active_state_ref().mode, Mode::Insert);
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::new(0, 6)
    );

    assert!(app.handle_key(id, key_char('[')));
    assert_eq!(app.text(id), "printf[\n");
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::new(0, 7)
    );
  }

  #[test]
  fn insert_mode_fresh_mouse_selection_typing_replaces_once_then_expands() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("printf\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    app.set_mode(Mode::Insert);

    let down = the_default::PointerEvent::new(
      the_default::PointerKind::Down(the_default::PointerButton::Left),
      0,
      0,
    )
    .with_logical_pos(0, 0)
    .with_click_count(1);
    assert_eq!(
      app.pointer_event(down),
      the_default::PointerEventOutcome::Handled
    );
    let drag = the_default::PointerEvent::new(
      the_default::PointerKind::Drag(the_default::PointerButton::Left),
      0,
      0,
    )
    .with_logical_pos(5, 0)
    .with_click_count(1);
    assert_eq!(
      app.pointer_event(drag),
      the_default::PointerEventOutcome::Handled
    );
    assert!(app.active_state_ref().insert_mouse_selection_edit_armed);

    assert!(app.handle_key(id, key_char('x')));
    assert_eq!(app.text(id), "x\n");
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::point(1)
    );
    assert!(!app.active_state_ref().insert_mouse_selection_edit_armed);

    assert!(app.handle_key(id, key_char('y')));
    assert_eq!(app.text(id), "xy\n");
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::point(2)
    );
  }

  #[test]
  fn insert_mode_fresh_mouse_selection_backspace_deletes_selection() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("printf\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    app.set_mode(Mode::Insert);

    let down = the_default::PointerEvent::new(
      the_default::PointerKind::Down(the_default::PointerButton::Left),
      0,
      0,
    )
    .with_logical_pos(0, 0)
    .with_click_count(1);
    let drag = the_default::PointerEvent::new(
      the_default::PointerKind::Drag(the_default::PointerButton::Left),
      0,
      0,
    )
    .with_logical_pos(5, 0)
    .with_click_count(1);
    assert_eq!(
      app.pointer_event(down),
      the_default::PointerEventOutcome::Handled
    );
    assert_eq!(
      app.pointer_event(drag),
      the_default::PointerEventOutcome::Handled
    );

    assert!(app.handle_key(id, key_backspace()));
    assert_eq!(app.text(id), "\n");
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::point(0)
    );
    assert!(!app.active_state_ref().insert_mouse_selection_edit_armed);
  }

  #[test]
  fn insert_mode_fresh_mouse_selection_delete_key_deletes_selection() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("printf\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    app.set_mode(Mode::Insert);

    let down = the_default::PointerEvent::new(
      the_default::PointerKind::Down(the_default::PointerButton::Left),
      0,
      0,
    )
    .with_logical_pos(0, 0)
    .with_click_count(1);
    let drag = the_default::PointerEvent::new(
      the_default::PointerKind::Drag(the_default::PointerButton::Left),
      0,
      0,
    )
    .with_logical_pos(5, 0)
    .with_click_count(1);
    assert_eq!(
      app.pointer_event(down),
      the_default::PointerEventOutcome::Handled
    );
    assert_eq!(
      app.pointer_event(drag),
      the_default::PointerEventOutcome::Handled
    );

    assert!(app.handle_key(id, key_delete()));
    assert_eq!(app.text(id), "\n");
    assert_eq!(
      app.active_editor_ref().document().selection().ranges()[0],
      Range::point(0)
    );
    assert!(!app.active_state_ref().insert_mouse_selection_edit_armed);
  }

  #[test]
  fn insert_mode_fresh_mouse_selection_alt_d_cuts_selection() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("printf\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    app.set_mode(Mode::Insert);

    let down = the_default::PointerEvent::new(
      the_default::PointerKind::Down(the_default::PointerButton::Left),
      0,
      0,
    )
    .with_logical_pos(0, 0)
    .with_click_count(1);
    let drag = the_default::PointerEvent::new(
      the_default::PointerKind::Drag(the_default::PointerButton::Left),
      0,
      0,
    )
    .with_logical_pos(5, 0)
    .with_click_count(1);
    assert_eq!(
      app.pointer_event(down),
      the_default::PointerEventOutcome::Handled
    );
    assert_eq!(
      app.pointer_event(drag),
      the_default::PointerEventOutcome::Handled
    );

    assert!(app.handle_key(id, key_char_alt('d')));
    assert_eq!(app.text(id), "\n");
    let values: Vec<_> = app
      .registers
      .read('"', app.active_editor_ref().document())
      .expect("default register")
      .map(|value| value.into_owned())
      .collect();
    assert_eq!(values, vec!["printf".to_string()]);
  }

  #[test]
  fn insert_mode_keyboard_move_clears_mouse_selection_edit_arm() {
    let _guard = ffi_test_guard();
    let mut app = App::new();
    let id = app.create_editor("printf\n", default_viewport(), ffi::Position {
      row: 0,
      col: 0,
    });
    assert!(app.activate(id).is_some());
    app.set_mode(Mode::Insert);

    let down = the_default::PointerEvent::new(
      the_default::PointerKind::Down(the_default::PointerButton::Left),
      0,
      0,
    )
    .with_logical_pos(0, 0)
    .with_click_count(1);
    let drag = the_default::PointerEvent::new(
      the_default::PointerKind::Drag(the_default::PointerButton::Left),
      0,
      0,
    )
    .with_logical_pos(5, 0)
    .with_click_count(1);
    assert_eq!(
      app.pointer_event(down),
      the_default::PointerEventOutcome::Handled
    );
    assert_eq!(
      app.pointer_event(drag),
      the_default::PointerEventOutcome::Handled
    );
    assert!(app.active_state_ref().insert_mouse_selection_edit_armed);

    assert!(app.handle_key(id, key_left()));
    assert!(!app.active_state_ref().insert_mouse_selection_edit_armed);

    assert!(app.handle_key(id, key_char('x')));
    assert_ne!(app.text(id), "x\n");
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
  #[ignore = "profiling helper for repeated down-arrow movement on plain-text buffers"]
  fn profile_plain_text_scroll_rebuild_cost() {
    let _guard = ffi_test_guard();
    let text = fs::read_to_string(
      Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("t8.shakespeare.txt"),
    )
    .expect("read plain text fixture");
    let viewport = ffi::Rect {
      x:      0,
      y:      0,
      width:  80,
      height: 24,
    };

    let mut app = App::new();
    let id = app.create_editor(&text, viewport, ffi::Position { row: 0, col: 0 });

    // Warm the initial frame so subsequent timings reflect steady-state movement.
    let _ = app.frame_render_plan(id);

    let mut total_handle_ns = 0u128;
    let mut total_frame_ns = 0u128;
    let mut total_scroll_handle_ns = 0u128;
    let mut total_scroll_frame_ns = 0u128;
    let mut total_in_view_handle_ns = 0u128;
    let mut total_in_view_frame_ns = 0u128;
    let mut scroll_steps = 0usize;
    let mut in_view_steps = 0usize;
    let mut max_frame_ms = 0.0f64;
    let mut max_handle_ms = 0.0f64;
    let mut notable_steps = Vec::new();

    for step in 0..96usize {
      let before_scroll = app.active_editor_ref().view().scroll.row;
      let before_cursor = app.active_editor_ref().document().selection().ranges()[0]
        .cursor_line(app.active_editor_ref().document().text().slice(..));

      let handle_started = Instant::now();
      assert!(app.handle_key(id, ffi::KeyEvent {
        kind:      15,
        codepoint: 0,
        modifiers: 0,
      },));
      let handle_elapsed = handle_started.elapsed();

      let frame_started = Instant::now();
      let frame = app.frame_render_plan(id);
      let frame_elapsed = frame_started.elapsed();

      let after_scroll = app.active_editor_ref().view().scroll.row;
      let after_cursor = app.active_editor_ref().document().selection().ranges()[0]
        .cursor_line(app.active_editor_ref().document().text().slice(..));

      let handle_ms = handle_elapsed.as_secs_f64() * 1000.0;
      let frame_ms = frame_elapsed.as_secs_f64() * 1000.0;
      let scrolled = after_scroll != before_scroll;
      let active_plan = frame.active_plan();

      total_handle_ns += handle_elapsed.as_nanos();
      total_frame_ns += frame_elapsed.as_nanos();
      max_handle_ms = max_handle_ms.max(handle_ms);
      max_frame_ms = max_frame_ms.max(frame_ms);

      if scrolled {
        scroll_steps += 1;
        total_scroll_handle_ns += handle_elapsed.as_nanos();
        total_scroll_frame_ns += frame_elapsed.as_nanos();
      } else {
        in_view_steps += 1;
        total_in_view_handle_ns += handle_elapsed.as_nanos();
        total_in_view_frame_ns += frame_elapsed.as_nanos();
      }

      if scrolled || frame_ms >= 1.0 || handle_ms >= 1.0 {
        notable_steps.push(format!(
          "step={step} cursor_line={before_cursor}->{after_cursor} \
           scroll_row={before_scroll}->{after_scroll} scrolled={} handle_ms={handle_ms:.3} \
           frame_ms={frame_ms:.3} lines={}",
          if scrolled { 1 } else { 0 },
          active_plan.line_count(),
        ));
      }
    }

    fn avg_ms(total_ns: u128, steps: usize) -> f64 {
      if steps == 0 {
        0.0
      } else {
        (total_ns as f64 / steps as f64) / 1_000_000.0
      }
    }

    eprintln!(
      "plain_text_scroll_profile total_steps=96 in_view_steps={} scroll_steps={} \
       avg_handle_ms={:.3} avg_frame_ms={:.3} avg_in_view_handle_ms={:.3} \
       avg_in_view_frame_ms={:.3} avg_scroll_handle_ms={:.3} avg_scroll_frame_ms={:.3} \
       max_handle_ms={:.3} max_frame_ms={:.3}",
      in_view_steps,
      scroll_steps,
      avg_ms(total_handle_ns, 96),
      avg_ms(total_frame_ns, 96),
      avg_ms(total_in_view_handle_ns, in_view_steps),
      avg_ms(total_in_view_frame_ns, in_view_steps),
      avg_ms(total_scroll_handle_ns, scroll_steps),
      avg_ms(total_scroll_frame_ns, scroll_steps),
      max_handle_ms,
      max_frame_ms,
    );
    for line in notable_steps {
      eprintln!("{line}");
    }
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
  fn lsp_runtime_config_tracks_active_workspace_without_shared_broker_state() {
    let _guard = ffi_test_guard();
    let workspace_a = TempTestWorkspace::new("lsp-runtime-a", "one.txt", "alpha\n");
    let workspace_b = TempTestWorkspace::new("lsp-runtime-b", "two.txt", "beta\n");

    let mut app = App::new();
    let id = app.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });
    install_test_lsp_state(&mut app, id, workspace_a.file_path());
    assert_eq!(
      app.lsp_runtime.config().workspace_root(),
      workspace_a.root_path()
    );
    assert_eq!(
      app.lsp_document.as_ref().map(|state| state.path.as_path()),
      Some(workspace_a.file_path())
    );

    install_test_lsp_state(&mut app, id, workspace_b.file_path());
    assert_eq!(
      app.lsp_runtime.config().workspace_root(),
      workspace_b.root_path()
    );
    assert_eq!(
      app.lsp_document.as_ref().map(|state| state.path.as_path()),
      Some(workspace_b.file_path())
    );
  }

  #[test]
  fn separate_apps_keep_lsp_document_state_isolated() {
    let _guard = ffi_test_guard();
    let workspace_a = TempTestWorkspace::new("lsp-isolated-a", "one.txt", "alpha\n");
    let workspace_b = TempTestWorkspace::new("lsp-isolated-b", "two.txt", "beta\n");

    let mut app_a = App::new();
    let id_a = app_a.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });
    install_test_lsp_state(&mut app_a, id_a, workspace_a.file_path());

    let mut app_b = App::new();
    let id_b = app_b.create_editor("", default_viewport(), ffi::Position { row: 0, col: 0 });
    install_test_lsp_state(&mut app_b, id_b, workspace_b.file_path());

    assert_eq!(
      app_a.lsp_runtime.config().workspace_root(),
      workspace_a.root_path()
    );
    assert_eq!(
      app_b.lsp_runtime.config().workspace_root(),
      workspace_b.root_path()
    );
    assert_eq!(
      app_a
        .lsp_document
        .as_ref()
        .map(|state| state.path.as_path()),
      Some(workspace_a.file_path())
    );
    assert_eq!(
      app_b
        .lsp_document
        .as_ref()
        .map(|state| state.path.as_path()),
      Some(workspace_b.file_path())
    );

    app_a.on_before_quit();

    assert!(app_a.lsp_document.is_none());
    assert_eq!(
      app_b
        .lsp_document
        .as_ref()
        .map(|state| state.path.as_path()),
      Some(workspace_b.file_path())
    );
    assert_eq!(
      app_b.lsp_runtime.config().workspace_root(),
      workspace_b.root_path()
    );
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
