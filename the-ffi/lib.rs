use std::{
  collections::{
    BTreeMap,
    HashSet,
    VecDeque,
  },
  env,
  ffi::{
    CStr,
    CString,
  },
  fs,
  num::NonZeroUsize,
  os::raw::c_char,
  path::{
    Path,
    PathBuf,
  },
  ptr,
  sync::{
    Arc,
    mpsc,
  },
  time::{
    Duration,
    Instant,
  },
};

use ropey::Rope;
use the_core::{
  grapheme::grapheme_width,
  line_ending::LineEnding,
};
use the_default::{
  Command,
  CommandPaletteState,
  CommandPaletteStyle,
  CommandPromptState,
  CommandRegistry,
  CompletionMenuState,
  DefaultApi,
  DefaultContext,
  DispatchRef,
  FilePickerState,
  FileTreeState,
  Key,
  KeyBinding,
  KeyEvent,
  Keymaps,
  Mode,
  Motion,
  SearchPromptKind,
  PendingInput,
  PickerRuntimeStore,
  SearchPromptState,
  SignatureHelpState,
  ThemeCatalog,
  WorkingDirectoryState,
  build_dispatch,
  build_statusline_snapshot,
  completion_accept,
  completion_docs_panel_rect,
  completion_panel_rect,
  builtin_completion_menu_keymaps,
  builtin_keymaps,
  close_completion_menu,
  close_file_picker,
  file_picker_icon_name_for_path,
  command_palette_filtered_indices,
  command_palette_placeholder_text,
  command_palette_selected_filtered_index,
  file_picker_item_selectable,
  file_picker_preview_window,
  file_picker_row_data_for_kind,
  handle_command,
  handle_command_prompt_key,
  handle_key,
  handle_search_prompt_key,
  install_default_wiring,
  move_selection,
  notify_file_picker_query_changed,
  open_command_palette,
  poll_scan_results,
  StatuslineEmphasis,
  select_file_picker_index,
  set_file_picker_list_offset,
  set_file_picker_preview_offset,
  set_file_picker_query_text,
  set_file_picker_syntax_loader,
  set_picker_visible_rows,
  submit_command_palette as submit_command_palette_action,
  submit_file_picker,
  step_search_prompt,
  signature_help_markdown,
  signature_help_panel_rect,
  sync_command_palette_preview,
  update_command_palette_for_input,
  update_search_prompt_preview,
  FilePickerKind,
  FilePickerPreviewChangeKind,
  FilePickerPreviewLineKind,
  FilePickerPreviewNavigationMode,
  FilePickerPreviewWindowKind,
  FilePickerRowKind,
  FilePickerVcsDiffPreviewLineSource,
  FilePickerVcsDiffPreviewRowKind,
};
use the_lib::{
  document::{
    Document,
    DocumentId,
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
  editor::{
    Editor,
    EditorId,
  },
  messages::MessageCenter,
  position::Position,
  registers::Registers,
  render::{
    FrameGenerationState,
    FrameRenderPlan,
    NoHighlights,
    RenderDamageReason,
    RenderDiffGutterStyles,
    RenderGenerationState,
    RenderGutterDiffKind,
    RenderPlan,
    RenderSelectionKind,
    RenderStyles,
    SyntaxHighlightAdapter,
    base_render_layer_row_hashes,
    build_plan,
    finish_render_generations,
    gutter::{
      GutterConfig,
      GutterSlot,
      GutterType,
    },
    gutter_width_for_document,
    graphics::{
      Color,
      CursorKind,
      Modifier,
      Rect,
      Style,
      UnderlineStyle,
    },
    overlay::{
      OverlayNode,
      OverlayRectKind,
    },
    text_annotations::TextAnnotations,
    text_format::TextFormat,
    theme::{
      Theme,
      default_theme,
    },
  },
  selection::{
    CursorPick,
    Selection,
  },
  syntax::{
    Highlight,
    HighlightCache,
    Loader,
    Syntax,
  },
  view::ViewState,
};
use unicode_segmentation::UnicodeSegmentation;
use the_lsp::{
  LspCapability,
  LspCompletionContext,
  LspCompletionItem,
  LspCompletionItemKind,
  LspEvent,
  LspInsertTextFormat,
  LspPosition,
  LspProgressKind,
  LspRuntime,
  LspRuntimeConfig,
  LspServerConfig,
  LspSignatureHelpContext,
  TextDocumentSyncKind,
  completion_params,
  hover_params,
  jsonrpc,
  parse_completion_item_response,
  parse_completion_response_with_raw,
  parse_hover_response,
  parse_signature_help_response,
  render_lsp_snippet,
  signature_help_params,
  text_sync::{
    char_idx_to_utf16_position,
    did_change_params,
    did_close_params,
    did_open_params,
    did_save_params,
    file_uri_for_path,
    utf16_position_to_char_idx,
  },
};
use the_runtime::{
  file_watch::{
    PathEventKind,
    WatchHandle,
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

#[repr(C)]
pub struct the_editor_handle_t {
  editor: SwiftEditor,
}

pub struct the_editor_snapshot_t {
  snapshot: OwnedSnapshot,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct the_editor_key_event_t {
  pub kind:      u32,
  pub codepoint: u32,
  pub modifiers: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_rgba_t {
  pub present: bool,
  pub r:       u8,
  pub g:       u8,
  pub b:       u8,
  pub a:       u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_style_t {
  pub fg:              the_editor_rgba_t,
  pub bg:              the_editor_rgba_t,
  pub underline_color: the_editor_rgba_t,
  pub add_modifiers:   u16,
  pub remove_modifiers:u16,
  pub underline_style: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_surface_metrics_t {
  pub backing_scale:          f32,
  pub cell_width_px:          u16,
  pub cell_height_px:         u16,
  pub cell_baseline_px:       u16,
  pub underline_position_px:  u16,
  pub underline_thickness_px: u16,
  pub cursor_thickness_px:    u16,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_surface_config_t {
  pub width_px:  u32,
  pub height_px: u32,
  pub metrics:   the_editor_surface_metrics_t,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_info_t {
  pub surface_width_px:        u32,
  pub surface_height_px:       u32,
  pub surface_metrics:         the_editor_surface_metrics_t,
  pub background_color:        the_editor_rgba_t,
  pub gutter_background_color: the_editor_rgba_t,
  pub viewport_width:          u16,
  pub viewport_height:         u16,
  pub content_offset_x:        u16,
  pub damage_start_row:        u16,
  pub damage_end_row:          u16,
  pub damage_is_full:          bool,
  pub damage_reason:           u8,
  pub mode:                    u8,
  pub layout_generation:       u64,
  pub text_generation:         u64,
  pub decoration_generation:   u64,
  pub cursor_generation:       u64,
  pub scroll_generation:       u64,
  pub theme_generation:        u64,
  pub cursor_blink_generation: u64,
  pub scroll_row:              u32,
  pub scroll_col:              u32,
  pub document_line_count:     u32,
  pub line_count:              usize,
  pub cursor_count:            usize,
  pub selection_count:         usize,
  pub overlay_count:           usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_line_t {
  pub row:               u16,
  pub doc_line:          i32,
  pub first_visual_line: bool,
  pub span_count:        usize,
  pub text_cell_count:   usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct the_editor_snapshot_span_t {
  pub col:        u16,
  pub cols:       u16,
  pub text:       *const c_char,
  pub is_virtual: bool,
  pub style:      the_editor_style_t,
}

impl Default for the_editor_snapshot_span_t {
  fn default() -> Self {
    Self {
      col: 0,
      cols: 0,
      text: ptr::null(),
      is_virtual: false,
      style: the_editor_style_t::default(),
    }
  }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct the_editor_snapshot_text_cell_t {
  pub row:        u16,
  pub col:        u16,
  pub cols:       u16,
  pub text:       *const c_char,
  pub is_virtual: bool,
  pub style:      the_editor_style_t,
}

impl Default for the_editor_snapshot_text_cell_t {
  fn default() -> Self {
    Self {
      row: 0,
      col: 0,
      cols: 0,
      text: ptr::null(),
      is_virtual: false,
      style: the_editor_style_t::default(),
    }
  }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_document_t {
  pub name:             *const c_char,
  pub icon:             *const c_char,
  pub relative_path:    *const c_char,
  pub absolute_path:    *const c_char,
  pub vcs_text:         *const c_char,
  pub language_name:    *const c_char,
  pub encoding_name:    *const c_char,
  pub line_ending_name: *const c_char,
  pub is_modified:      bool,
  pub is_readonly:      bool,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_status_t {
  pub leading_text: *const c_char,
  pub item_count:    usize,
  pub cursor_text:   *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_status_item_t {
  pub icon:      *const c_char,
  pub text:      *const c_char,
  pub emphasis:  u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_command_palette_t {
  pub is_open:        bool,
  pub selected_index: i32,
  pub item_count:     usize,
  pub query:          *const c_char,
  pub placeholder:    *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_command_palette_item_t {
  pub title:         *const c_char,
  pub subtitle:      *const c_char,
  pub description:   *const c_char,
  pub badge:         *const c_char,
  pub leading_icon:  *const c_char,
  pub leading_color: the_editor_rgba_t,
  pub emphasis:      bool,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_completion_menu_t {
  pub is_open:        bool,
  pub col:            u16,
  pub row:            u16,
  pub width:          u16,
  pub height:         u16,
  pub selected_index: i32,
  pub item_count:     usize,
  pub scroll_offset:  usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_completion_menu_item_t {
  pub title:         *const c_char,
  pub subtitle:      *const c_char,
  pub leading_icon:  *const c_char,
  pub leading_color: the_editor_rgba_t,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_input_prompt_t {
  pub is_open:     bool,
  pub kind:        u8,
  pub title:       *const c_char,
  pub placeholder: *const c_char,
  pub query:       *const c_char,
  pub error:       *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_docs_panel_t {
  pub is_open:    bool,
  pub col:        u16,
  pub row:        u16,
  pub width:      u16,
  pub height:     u16,
  pub run_count:  usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_docs_run_t {
  pub text:  *const c_char,
  pub style: the_editor_style_t,
  pub kind:  u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_file_picker_t {
  pub is_open:                 bool,
  pub kind:                    u8,
  pub selected_index:          i32,
  pub matched_count:           usize,
  pub visible_item_start:      usize,
  pub visible_item_count:      usize,
  pub title:                   *const c_char,
  pub query:                   *const c_char,
  pub show_preview:            bool,
  pub loading:                 bool,
  pub error:                   *const c_char,
  pub preview_path:            *const c_char,
  pub preview_navigation_mode: u8,
  pub preview_kind:            u8,
  pub preview_total_rows:      usize,
  pub preview_offset:          usize,
  pub preview_window_start:    usize,
  pub preview_window_count:    usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_file_picker_item_t {
  pub stable_id:    u64,
  pub global_index: usize,
  pub row_kind:     u8,
  pub selectable:   bool,
  pub is_dir:       bool,
  pub icon:         *const c_char,
  pub primary:      *const c_char,
  pub secondary:    *const c_char,
  pub tertiary:     *const c_char,
  pub quaternary:   *const c_char,
  pub line:         u32,
  pub column:       u32,
  pub depth:        u16,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_file_picker_preview_line_t {
  pub virtual_row:  usize,
  pub kind:         u8,
  pub source:       u8,
  pub line_number:  i32,
  pub focused:      bool,
  pub marker:       *const c_char,
  pub segment_count: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct the_editor_snapshot_file_picker_preview_segment_t {
  pub text:        *const c_char,
  pub style:       the_editor_style_t,
  pub is_match:    bool,
  pub change_kind: i8,
}

impl Default for the_editor_snapshot_file_picker_preview_segment_t {
  fn default() -> Self {
    Self {
      text: ptr::null(),
      style: the_editor_style_t::default(),
      is_match: false,
      change_kind: -1,
    }
  }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_cursor_t {
  pub row:   u32,
  pub col:   u32,
  pub kind:  u8,
  pub style: the_editor_style_t,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct the_editor_snapshot_selection_t {
  pub x:      u16,
  pub y:      u16,
  pub width:  u16,
  pub height: u16,
  pub kind:   u8,
  pub style:  the_editor_style_t,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct the_editor_snapshot_overlay_t {
  pub kind:      u8,
  pub rect_kind: u8,
  pub x:         u16,
  pub y:         u16,
  pub width:     u16,
  pub height:    u16,
  pub radius:    u16,
  pub row:       u32,
  pub col:       u32,
  pub text:      *const c_char,
  pub style:     the_editor_style_t,
}

impl Default for the_editor_snapshot_overlay_t {
  fn default() -> Self {
    Self {
      kind: 0,
      rect_kind: 0,
      x: 0,
      y: 0,
      width: 0,
      height: 0,
      radius: 0,
      row: 0,
      col: 0,
      text: ptr::null(),
      style: the_editor_style_t::default(),
    }
  }
}

const THE_EDITOR_KEY_CHAR: u32 = 0;
const THE_EDITOR_KEY_ENTER: u32 = 1;
const THE_EDITOR_KEY_NUMPAD_ENTER: u32 = 2;
const THE_EDITOR_KEY_ESCAPE: u32 = 3;
const THE_EDITOR_KEY_BACKSPACE: u32 = 4;
const THE_EDITOR_KEY_TAB: u32 = 5;
const THE_EDITOR_KEY_DELETE: u32 = 6;
const THE_EDITOR_KEY_INSERT: u32 = 7;
const THE_EDITOR_KEY_HOME: u32 = 8;
const THE_EDITOR_KEY_END: u32 = 9;
const THE_EDITOR_KEY_PAGE_UP: u32 = 10;
const THE_EDITOR_KEY_PAGE_DOWN: u32 = 11;
const THE_EDITOR_KEY_LEFT: u32 = 12;
const THE_EDITOR_KEY_RIGHT: u32 = 13;
const THE_EDITOR_KEY_UP: u32 = 14;
const THE_EDITOR_KEY_DOWN: u32 = 15;
const THE_EDITOR_KEY_F1: u32 = 16;
const THE_EDITOR_KEY_F2: u32 = 17;
const THE_EDITOR_KEY_F3: u32 = 18;
const THE_EDITOR_KEY_F4: u32 = 19;
const THE_EDITOR_KEY_F5: u32 = 20;
const THE_EDITOR_KEY_F6: u32 = 21;
const THE_EDITOR_KEY_F7: u32 = 22;
const THE_EDITOR_KEY_F8: u32 = 23;
const THE_EDITOR_KEY_F9: u32 = 24;
const THE_EDITOR_KEY_F10: u32 = 25;
const THE_EDITOR_KEY_F11: u32 = 26;
const THE_EDITOR_KEY_F12: u32 = 27;
const THE_EDITOR_KEY_OTHER: u32 = 28;

const MOD_CTRL: u8 = 1 << 0;
const MOD_ALT: u8 = 1 << 1;
const MOD_SHIFT: u8 = 1 << 2;

const STYLE_BOLD: u16 = 1 << 0;
const STYLE_DIM: u16 = 1 << 1;
const STYLE_ITALIC: u16 = 1 << 2;
const STYLE_SLOW_BLINK: u16 = 1 << 3;
const STYLE_RAPID_BLINK: u16 = 1 << 4;
const STYLE_REVERSED: u16 = 1 << 5;
const STYLE_HIDDEN: u16 = 1 << 6;
const STYLE_CROSSED_OUT: u16 = 1 << 7;

const SWIFT_SCROLLOFF: usize = 0;

fn theme_perf_enabled() -> bool {
  env::var("THE_EDITOR_THEME_PROFILE").ok().as_deref() == Some("1")
}

fn theme_perf_log(message: impl AsRef<str>) {
  if theme_perf_enabled() {
    eprintln!("[the-ffi:perf] {}", message.as_ref());
  }
}

fn command_palette_debug_enabled() -> bool {
  env::var("THE_EDITOR_COMMAND_PALETTE_DEBUG").ok().as_deref() == Some("1")
}

fn command_palette_debug_log(message: impl AsRef<str>) {
  if command_palette_debug_enabled() {
    eprintln!("[the-ffi:command-palette] {}", message.as_ref());
  }
}

fn completion_trace_enabled() -> bool {
  env::var("THE_EDITOR_COMPLETION_TRACE").ok().as_deref() == Some("1")
}

fn completion_trace_log(message: impl AsRef<str>) {
  if completion_trace_enabled() {
    eprintln!("[the-ffi:completion] {}", message.as_ref());
  }
}

#[derive(Default)]
struct OwnedSnapshot {
  info:                  the_editor_snapshot_info_t,
  document:              DocumentRecord,
  status:                StatusRecord,
  status_items:          Vec<StatusItemRecord>,
  command_palette:       CommandPaletteRecord,
  command_palette_items: Vec<CommandPaletteItemRecord>,
  completion_menu:       CompletionMenuRecord,
  completion_menu_items: Vec<CompletionMenuItemRecord>,
  input_prompt:          InputPromptRecord,
  hover_docs:            DocsPanelRecord,
  hover_docs_runs:       Vec<DocsRunRecord>,
  completion_docs:       DocsPanelRecord,
  completion_docs_runs:  Vec<DocsRunRecord>,
  signature_help:        DocsPanelRecord,
  signature_help_runs:   Vec<DocsRunRecord>,
  file_picker:           FilePickerRecord,
  file_picker_items:     Vec<FilePickerItemRecord>,
  file_picker_preview_lines: Vec<FilePickerPreviewLineRecord>,
  file_picker_preview_segments: Vec<FilePickerPreviewSegmentRecord>,
  lines:                 Vec<LineRecord>,
  spans:                 Vec<SpanRecord>,
  text_cells:            Vec<TextCellRecord>,
  cursors:               Vec<the_editor_snapshot_cursor_t>,
  selections:            Vec<the_editor_snapshot_selection_t>,
  overlays:              Vec<OverlayRecord>,
  strings:               Vec<CString>,
}

#[derive(Clone, Copy, Default)]
struct DocumentRecord {
  document:               the_editor_snapshot_document_t,
  name_idx:               Option<usize>,
  icon_idx:               Option<usize>,
  relative_path_idx:      Option<usize>,
  absolute_path_idx:      Option<usize>,
  vcs_text_idx:           Option<usize>,
  language_name_idx:      Option<usize>,
  encoding_name_idx:      Option<usize>,
  line_ending_name_idx:   Option<usize>,
}

#[derive(Clone, Copy, Default)]
struct StatusRecord {
  status:             the_editor_snapshot_status_t,
  leading_text_idx:   Option<usize>,
  cursor_text_idx:    Option<usize>,
}

#[derive(Clone, Copy, Default)]
struct StatusItemRecord {
  item:               the_editor_snapshot_status_item_t,
  icon_idx:           Option<usize>,
  text_idx:           usize,
}

#[derive(Clone, Copy, Default)]
struct CommandPaletteRecord {
  palette:         the_editor_snapshot_command_palette_t,
  query_idx:       Option<usize>,
  placeholder_idx: Option<usize>,
}

#[derive(Clone, Copy, Default)]
struct CommandPaletteItemRecord {
  item:            the_editor_snapshot_command_palette_item_t,
  title_idx:       usize,
  subtitle_idx:    Option<usize>,
  description_idx: Option<usize>,
  badge_idx:       Option<usize>,
  leading_icon_idx: Option<usize>,
}

#[derive(Clone, Copy, Default)]
struct CompletionMenuRecord {
  menu: the_editor_snapshot_completion_menu_t,
}

#[derive(Clone, Copy, Default)]
struct CompletionMenuItemRecord {
  item:             the_editor_snapshot_completion_menu_item_t,
  title_idx:        usize,
  subtitle_idx:     Option<usize>,
  leading_icon_idx: Option<usize>,
}

#[derive(Clone, Copy, Default)]
struct InputPromptRecord {
  prompt:          the_editor_snapshot_input_prompt_t,
  title_idx:       Option<usize>,
  placeholder_idx: Option<usize>,
  query_idx:       Option<usize>,
  error_idx:       Option<usize>,
}

#[derive(Clone, Copy, Default)]
struct DocsPanelRecord {
  panel: the_editor_snapshot_docs_panel_t,
}

#[derive(Clone, Copy, Default)]
struct DocsRunRecord {
  run:      the_editor_snapshot_docs_run_t,
  text_idx: usize,
}

#[derive(Clone, Copy, Default)]
struct FilePickerRecord {
  picker:                      the_editor_snapshot_file_picker_t,
  title_idx:                   Option<usize>,
  query_idx:                   Option<usize>,
  error_idx:                   Option<usize>,
  preview_path_idx:            Option<usize>,
}

#[derive(Clone, Copy, Default)]
struct FilePickerItemRecord {
  item:            the_editor_snapshot_file_picker_item_t,
  icon_idx:        usize,
  primary_idx:     usize,
  secondary_idx:   Option<usize>,
  tertiary_idx:    Option<usize>,
  quaternary_idx:  Option<usize>,
}

#[derive(Clone, Copy, Default)]
struct FilePickerPreviewLineRecord {
  line:            the_editor_snapshot_file_picker_preview_line_t,
  marker_idx:      Option<usize>,
  segment_start:   usize,
}

#[derive(Clone, Copy, Default)]
struct FilePickerPreviewSegmentRecord {
  segment:         the_editor_snapshot_file_picker_preview_segment_t,
  text_idx:        usize,
}

#[derive(Clone, Copy, Default)]
struct LineRecord {
  line:            the_editor_snapshot_line_t,
  span_start:      usize,
  text_cell_start: usize,
}

#[derive(Clone, Copy, Default)]
struct SpanRecord {
  span:     the_editor_snapshot_span_t,
  text_idx: usize,
}

#[derive(Clone, Copy, Default)]
struct TextCellRecord {
  cell:     the_editor_snapshot_text_cell_t,
  text_idx: usize,
}

#[derive(Clone, Copy, Default)]
struct OverlayRecord {
  overlay:  the_editor_snapshot_overlay_t,
  text_idx: Option<usize>,
}

#[derive(Clone, Copy)]
struct SurfaceConfig {
  width_px:  u32,
  height_px: u32,
  metrics:   the_editor_surface_metrics_t,
}

impl Default for SurfaceConfig {
  fn default() -> Self {
    let metrics = the_editor_surface_metrics_t {
      backing_scale: 2.0,
      cell_width_px: 18,
      cell_height_px: 34,
      cell_baseline_px: 6,
      underline_position_px: 4,
      underline_thickness_px: 2,
      cursor_thickness_px: 4,
    };
    Self {
      width_px: 80 * metrics.cell_width_px as u32,
      height_px: 24 * metrics.cell_height_px as u32,
      metrics,
    }
  }
}

impl SurfaceConfig {
  fn from_ffi(config: the_editor_surface_config_t) -> Self {
    let backing_scale = if config.metrics.backing_scale.is_finite() && config.metrics.backing_scale > 0.0 {
      config.metrics.backing_scale
    } else {
      1.0
    };
    let metrics = the_editor_surface_metrics_t {
      backing_scale,
      cell_width_px: config.metrics.cell_width_px.max(1),
      cell_height_px: config.metrics.cell_height_px.max(1),
      cell_baseline_px: config.metrics.cell_baseline_px.max(1),
      underline_position_px: config.metrics.underline_position_px,
      underline_thickness_px: config.metrics.underline_thickness_px.max(1),
      cursor_thickness_px: config.metrics.cursor_thickness_px.max(1),
    };
    Self {
      width_px: config.width_px.max(metrics.cell_width_px as u32),
      height_px: config.height_px.max(metrics.cell_height_px as u32),
      metrics,
    }
  }

  fn viewport_cols(self) -> u16 {
    let cols = (self.width_px / self.metrics.cell_width_px as u32).max(1);
    cols.min(u16::MAX as u32) as u16
  }

  fn viewport_rows(self) -> u16 {
    let rows = (self.height_px / self.metrics.cell_height_px as u32).max(1);
    rows.min(u16::MAX as u32) as u16
  }
}

struct VcsStatuslineRefreshResult {
  generation: u64,
  path:       PathBuf,
  statusline: Option<String>,
  diff_base:  Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
struct LspDocumentSyncState {
  path:        PathBuf,
  uri:         String,
  language_id: String,
  version:     i32,
}

#[derive(Debug, Clone)]
struct PendingAutoCompletion {
  due_at:  Instant,
  trigger: LspCompletionContext,
}

#[derive(Debug, Clone)]
struct PendingAutoSignatureHelp {
  due_at:  Instant,
  trigger: LspSignatureHelpContext,
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

#[derive(Debug, Clone)]
enum PendingLspRequestKind {
  Hover { uri: String },
  Completion {
    uri:            String,
    generation:     u64,
    cursor:         usize,
    replace_start:  usize,
    announce_empty: bool,
  },
  CompletionResolve { uri: String, index: usize },
  SignatureHelp { uri: String },
}

impl PendingLspRequestKind {
  fn uri(&self) -> &str {
    match self {
      Self::Hover { uri }
      | Self::Completion { uri, .. }
      | Self::CompletionResolve { uri, .. }
      | Self::SignatureHelp { uri } => uri,
    }
  }

  fn label(&self) -> &'static str {
    match self {
      Self::Hover { .. } => "hover",
      Self::Completion { .. } => "completion",
      Self::CompletionResolve { .. } => "completion-resolve",
      Self::SignatureHelp { .. } => "signature-help",
    }
  }

  fn cancellation_key(&self) -> &'static str {
    match self {
      Self::Hover { .. } => "hover",
      Self::Completion { .. } => "completion",
      Self::CompletionResolve { .. } => "completion-resolve",
      Self::SignatureHelp { .. } => "signature-help",
    }
  }
}

struct ManagedLspRuntime {
  runtime:                 LspRuntime,
  ready:                   bool,
  opened_current_document: bool,
  statusline:              LspStatuslineState,
  active_progress_tokens:  HashSet<String>,
  pending_requests:        BTreeMap<u64, PendingLspRequestKind>,
}

impl ManagedLspRuntime {
  fn configured_server_name(&self) -> Option<&str> {
    self.runtime.config().server().map(|server| server.name())
  }
}

struct ActiveFileWatchState {
  stream:        WatchedFileEventsState,
  _watch_handle: WatchHandle,
}

struct SwiftEditor {
  editor:                        Editor,
  file_path:                     Option<PathBuf>,
  workspace_root:                PathBuf,
  working_directory:             WorkingDirectoryState,
  messages:                      MessageCenter,
  mode:                          Mode,
  dispatch:                      Box<dyn DefaultApi<SwiftEditor>>,
  keymaps:                       Keymaps,
  completion_menu_keymaps:       Keymaps,
  command_registry:              CommandRegistry<SwiftEditor>,
  command_prompt:                CommandPromptState,
  command_palette:               CommandPaletteState,
  command_palette_style:         CommandPaletteStyle,
  completion_menu:               CompletionMenuState,
  inline_completion:             the_default::InlineCompletionState,
  inline_completion_annotations: the_default::OwnedTextAnnotations,
  file_tree:                     FileTreeState,
  file_picker:                   FilePickerState,
  picker_runtime_store:          PickerRuntimeStore<SwiftEditor>,
  search_prompt:                 SearchPromptState,
  signature_help:                SignatureHelpState,
  hover_docs:                    Option<String>,
  lsp_completion_items:          Vec<LspCompletionItem>,
  lsp_completion_raw_items:      Vec<serde_json::Value>,
  lsp_completion_resolved:       HashSet<usize>,
  lsp_completion_resolve_supported: bool,
  lsp_completion_generation:     u64,
  lsp_completion_fallback_start: Option<usize>,
  lsp_completion_visible_indices: Vec<usize>,
  lsp_pending_auto_completion:   Option<PendingAutoCompletion>,
  lsp_pending_auto_signature_help: Option<PendingAutoSignatureHelp>,
  pending_input:                 Option<PendingInput>,
  registers:                     Registers,
  register:                      Option<char>,
  macro_recording:               Option<(char, Vec<KeyBinding>)>,
  macro_replaying:               Vec<char>,
  macro_queue:                   VecDeque<KeyEvent>,
  last_motion:                   Option<Motion>,
  text_format:                   TextFormat,
  soft_wrap_enabled:             bool,
  gutter_config:                 the_lib::render::GutterConfig,
  loader:                        Option<Arc<Loader>>,
  lsp_document:                  Option<LspDocumentSyncState>,
  lsp_runtimes:                  Vec<ManagedLspRuntime>,
  lsp_statusline:                LspStatuslineState,
  lsp_spinner_index:             usize,
  lsp_spinner_last_tick:         Instant,
  highlight_cache:               HighlightCache,
  file_picker_preview_visible_rows: usize,
  ui_theme_catalog:              ThemeCatalog,
  ui_theme_name:                 String,
  ui_theme_base:                 Theme,
  ui_theme_preview_name:         Option<String>,
  ui_theme:                      Theme,
  render_generation_state:       Option<RenderGenerationState>,
  frame_generation_state:        FrameGenerationState,
  render_theme_generation:       u64,
  surface:                       SurfaceConfig,
  vcs_provider:                  DiffProviderRegistry,
  vcs_statusline:                Option<String>,
  gutter_diff_signs:             BTreeMap<usize, RenderGutterDiffKind>,
  vcs_diff:                      Option<DiffHandle>,
  active_file_watch:             Option<ActiveFileWatchState>,
  vcs_statusline_refresh_in_flight: bool,
  vcs_statusline_refresh_generation: u64,
  vcs_statusline_refresh_tx:     mpsc::Sender<VcsStatuslineRefreshResult>,
  vcs_statusline_refresh_rx:     mpsc::Receiver<VcsStatuslineRefreshResult>,
}

impl SwiftEditor {
  fn content_viewport_width(&self) -> u16 {
    let view = self.editor.view();
    let gutter_width = gutter_width_for_document(self.editor.document(), view.viewport.width, &self.gutter_config);
    view.viewport.width.saturating_sub(gutter_width).max(1)
  }

  fn sync_text_viewport_width(&mut self) {
    self.text_format.viewport_width = self.content_viewport_width();
  }

  fn close_completion_menu_ui(&mut self) -> bool {
    if !self.completion_menu.active {
      return false;
    }
    close_completion_menu(self);
    true
  }

  fn select_completion_menu_index(&mut self, index: usize) -> bool {
    if !self.completion_menu.active || index >= self.completion_menu.items.len() {
      return false;
    }
    if self.completion_menu.selected == Some(index) {
      return false;
    }
    self.completion_menu.selected = Some(index);
    self.completion_menu.docs_scroll = 0;
    let visible_rows = completion_menu_visible_rows();
    if index < self.completion_menu.scroll {
      self.completion_menu.scroll = index;
    } else if index >= self.completion_menu.scroll.saturating_add(visible_rows) {
      self.completion_menu.scroll = index + 1 - visible_rows;
    }
    let max_scroll = self.completion_menu.items.len().saturating_sub(visible_rows);
    self.completion_menu.scroll = self.completion_menu.scroll.min(max_scroll);
    self.completion_selection_changed(index);
    self.request_render();
    true
  }

  fn submit_completion_menu_selection(&mut self) -> bool {
    if !self.completion_menu.active || self.completion_menu.items.is_empty() {
      return false;
    }
    completion_accept(self);
    true
  }

  fn set_completion_menu_scroll(&mut self, offset: usize) -> bool {
    if !self.completion_menu.active || self.completion_menu.items.is_empty() {
      return false;
    }
    let max_scroll = self.completion_menu.items.len().saturating_sub(completion_menu_visible_rows());
    let next = offset.min(max_scroll);
    if self.completion_menu.scroll == next {
      return false;
    }
    self.completion_menu.scroll = next;
    self.request_render();
    true
  }

  fn refresh_lsp_runtime_state(&mut self) {
    let old_document = self.lsp_document.clone();
    let mut old_runtimes = std::mem::take(&mut self.lsp_runtimes);

    self.signature_help.clear();
    self.clear_hover_state();
    self.clear_completion_state_with_reason("refresh-lsp-runtime-state");
    self.lsp_document = self
      .file_path
      .as_deref()
      .and_then(|path| build_lsp_document_state(path, self.loader.as_deref()));

    let Some(document) = self.lsp_document.as_ref() else {
      for runtime in &mut old_runtimes {
        close_lsp_document_for_runtime(runtime, old_document.as_ref());
        let _ = runtime.runtime.shutdown_detached();
      }
      self.lsp_statusline = LspStatuslineState::off(Some("unavailable".into()));
      self.lsp_spinner_index = 0;
      return;
    };

    let servers = resolve_lsp_servers(self.loader.as_deref(), Some(document.path.as_path()));
    if servers.is_empty() {
      for runtime in &mut old_runtimes {
        close_lsp_document_for_runtime(runtime, old_document.as_ref());
        let _ = runtime.runtime.shutdown_detached();
      }
      self.lsp_statusline = LspStatuslineState::off(Some("unavailable".into()));
      self.lsp_spinner_index = 0;
      return;
    }

    let workspace_root = resolved_workspace_root_for_path(&document.path);
    let document_changed = old_document
      .as_ref()
      .is_none_or(|old| old.uri != document.uri || old.language_id != document.language_id);

    let mut new_runtimes = Vec::new();
    for server in servers {
      if let Some(index) = old_runtimes.iter().position(|runtime| {
        lsp_server_configs_equal(runtime.runtime.config().server(), Some(&server))
          && runtime.runtime.config().workspace_root() == workspace_root.as_path()
      }) {
        let mut runtime = old_runtimes.remove(index);
        if document_changed {
          close_lsp_document_for_runtime(&mut runtime, old_document.as_ref());
        }
        new_runtimes.push(runtime);
        continue;
      }

      let runtime_config = lsp_runtime_config_for(server, workspace_root.clone());
      let server_name = runtime_config
        .server()
        .map(|server| server.name().to_string())
        .unwrap_or_else(|| "lsp".to_string());
      let mut runtime = LspRuntime::new(runtime_config);
      let start_result = runtime.start();
      let mut managed = ManagedLspRuntime {
        runtime,
        ready: false,
        opened_current_document: false,
        statusline: LspStatuslineState {
          phase: LspStatusPhase::Starting,
          detail: Some(clamp_status_text(&server_name, 28)),
        },
        active_progress_tokens: HashSet::new(),
        pending_requests: BTreeMap::new(),
      };
      if let Err(err) = start_result {
        managed.statusline = LspStatuslineState {
          phase: LspStatusPhase::Error,
          detail: Some(summarize_lsp_error(&err.to_string())),
        };
      }
      new_runtimes.push(managed);
    }

    for runtime in &mut old_runtimes {
      close_lsp_document_for_runtime(runtime, old_document.as_ref());
      let _ = runtime.runtime.shutdown_detached();
    }

    self.lsp_runtimes = new_runtimes;
    for runtime_index in 0..self.lsp_runtimes.len() {
      self.open_current_document_for_runtime(runtime_index);
    }
    self.sync_lsp_statusline();
  }

  fn sync_lsp_statusline(&mut self) {
    self.lsp_statusline = self
      .lsp_runtimes
      .first()
      .map(|runtime| runtime.statusline.clone())
      .unwrap_or_else(|| LspStatuslineState::off(Some("unavailable".into())));
    if !self.lsp_statusline.is_loading() {
      self.lsp_spinner_index = 0;
    }
  }

  fn set_lsp_status_for_runtime(&mut self, runtime_index: usize, phase: LspStatusPhase, detail: Option<String>) {
    if let Some(runtime) = self.lsp_runtimes.get_mut(runtime_index) {
      runtime.statusline = LspStatuslineState {
        phase,
        detail: detail.map(|value| clamp_status_text(&value, 28)),
      };
    }
    if runtime_index == 0 {
      self.sync_lsp_statusline();
    }
  }

  fn open_current_document_for_runtime(&mut self, runtime_index: usize) {
    let Some(document_state) = self.lsp_document.clone() else {
      return;
    };
    let Some(runtime) = self.lsp_runtimes.get_mut(runtime_index) else {
      return;
    };
    if !runtime.ready || runtime.opened_current_document {
      return;
    }
    let params = did_open_params(
      &document_state.uri,
      &document_state.language_id,
      document_state.version,
      self.editor.document().text(),
    );
    if runtime
      .runtime
      .send_notification("textDocument/didOpen", Some(params))
      .is_ok()
    {
      runtime.opened_current_document = true;
    }
  }

  fn lsp_sync_kind_for_runtime(runtime: &ManagedLspRuntime) -> Option<TextDocumentSyncKind> {
    let server_name = runtime.configured_server_name()?;
    runtime
      .runtime
      .server_capabilities(server_name)
      .map(|capabilities| capabilities.text_document_sync().kind)
  }

  fn lsp_save_include_text_for_runtime(runtime: &ManagedLspRuntime) -> bool {
    let Some(server_name) = runtime.configured_server_name() else {
      return false;
    };
    runtime
      .runtime
      .server_capabilities(server_name)
      .is_some_and(|capabilities| capabilities.text_document_sync().save_include_text)
  }

  fn lsp_send_did_change(&mut self, old_text: &Rope, changes: &the_lib::transaction::ChangeSet) {
    let Some(document_state) = self.lsp_document.clone() else {
      return;
    };
    let new_text = self.editor.document().text().clone();
    let next_version = self.editor.document().version() as i32;
    for runtime in &self.lsp_runtimes {
      if !runtime.opened_current_document {
        continue;
      }
      let Some(sync_kind) = Self::lsp_sync_kind_for_runtime(runtime) else {
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
      let _ = runtime.runtime.send_notification("textDocument/didChange", Some(params));
    }
    if let Some(document_state) = self.lsp_document.as_mut() {
      document_state.version = next_version;
    }
  }

  fn lsp_send_did_save(&mut self, text: Option<&str>) {
    let Some(document_state) = self.lsp_document.clone() else {
      return;
    };
    for runtime in &self.lsp_runtimes {
      if !runtime.opened_current_document {
        continue;
      }
      let payload_text = if Self::lsp_save_include_text_for_runtime(runtime) {
        text
      } else {
        None
      };
      let params = did_save_params(&document_state.uri, payload_text);
      let _ = runtime.runtime.send_notification("textDocument/didSave", Some(params));
    }
  }

  fn current_lsp_position(&self) -> Option<(String, LspPosition)> {
    let document = self.lsp_document.as_ref()?.clone();
    if !self.lsp_runtimes.iter().any(|runtime| runtime.opened_current_document) {
      return None;
    }

    let selection = self.editor.document().selection();
    let Ok((_, range)) = selection.pick(CursorPick::First) else {
      return None;
    };
    let cursor = range.cursor(self.editor.document().text().slice(..));
    let (line, character) = char_idx_to_utf16_position(self.editor.document().text(), cursor);
    Some((document.uri, LspPosition { line, character }))
  }

  fn active_cursor_char_idx(&self) -> Option<usize> {
    let selection = self.editor.document().selection();
    let Ok((_, range)) = selection.pick(CursorPick::First) else {
      return None;
    };
    Some(range.cursor(self.editor.document().text().slice(..)))
  }

  fn cursor_prev_char_is_word(&self) -> bool {
    let Some(cursor) = self.active_cursor_char_idx() else {
      return false;
    };
    self
      .editor
      .document()
      .text()
      .get_char(cursor.saturating_sub(1))
      .is_some_and(is_symbol_word_char)
  }

  fn completion_trace_state(&self) -> String {
    format!(
      "menu_active={} menu_items={} selected={:?} raw_items={} visible_items={} generation={} fallback_start={:?} cursor={:?}",
      self.completion_menu.active,
      self.completion_menu.items.len(),
      self.completion_menu.selected,
      self.lsp_completion_items.len(),
      self.lsp_completion_visible_indices.len(),
      self.lsp_completion_generation,
      self.lsp_completion_fallback_start,
      self.active_cursor_char_idx(),
    )
  }

  fn clear_completion_state_with_reason(&mut self, reason: &str) {
    completion_trace_log(format!("clear reason={} {}", reason, self.completion_trace_state()));
    self.clear_completion_state();
  }

  fn completion_replace_start_at_cursor(&self, cursor: usize) -> usize {
    let text = self.editor.document().text();
    let mut start = cursor.min(text.len_chars());
    while start > 0 && text.get_char(start - 1).is_some_and(is_completion_replace_char) {
      start -= 1;
    }
    start
  }

  fn handle_insert_mode_char_post_edit(&mut self, ch: char) {
    completion_trace_log(format!(
      "post_edit.char ch={:?} trigger_char_supported={} symbol_word={} {}",
      ch,
      self.lsp_completion_supports_trigger_char(ch),
      is_symbol_word_char(ch),
      self.completion_trace_state(),
    ));
    if self.lsp_signature_help_supports_trigger_char(ch) {
      self.clear_completion_state_with_reason("signature-trigger-char");
      let trigger = if self.signature_help.active {
        LspSignatureHelpContext::trigger_character_retrigger(ch)
      } else {
        LspSignatureHelpContext::trigger_character(ch)
      };
      let _ = self.schedule_auto_signature_help(trigger, Duration::from_millis(20));
      return;
    }

    if self.signature_help.active {
      let trigger = if self.lsp_signature_help_supports_retrigger_char(ch) {
        LspSignatureHelpContext::trigger_character_retrigger(ch)
      } else {
        LspSignatureHelpContext::content_change_retrigger()
      };
      let _ = self.schedule_auto_signature_help(trigger, Duration::from_millis(80));
    } else {
      self.cancel_auto_signature_help();
      if !is_symbol_word_char(ch) {
        self.signature_help.clear();
      }
    }

    if self.completion_menu.active {
      self.rebuild_completion_menu();
    }
    if self.lsp_completion_supports_trigger_char(ch) {
      let _ = self.schedule_auto_completion(
        LspCompletionContext::trigger_character(ch),
        Duration::from_millis(20),
      );
    } else if is_symbol_word_char(ch) {
      let _ = self.schedule_auto_completion(
        LspCompletionContext::invoked(),
        Duration::from_millis(80),
      );
    } else {
      self.clear_completion_state_with_reason("post-edit-non-word-char");
    }
  }

  fn handle_insert_mode_delete_post_edit(&mut self) {
    completion_trace_log(format!(
      "post_edit.delete prev_char_is_word={} {}",
      self.cursor_prev_char_is_word(),
      self.completion_trace_state(),
    ));
    if self.signature_help.active {
      let _ = self.schedule_auto_signature_help(
        LspSignatureHelpContext::content_change_retrigger(),
        Duration::from_millis(80),
      );
    } else {
      self.cancel_auto_signature_help();
    }

    if self.completion_menu.active {
      self.rebuild_completion_menu();
    }
    if self.completion_menu.active || self.cursor_prev_char_is_word() {
      let _ = self.schedule_auto_completion(
        LspCompletionContext::trigger_for_incomplete(),
        Duration::from_millis(80),
      );
    } else {
      self.clear_completion_state_with_reason("post-delete-no-word-context");
    }
  }

  fn handle_insert_mode_other_post_edit(&mut self) {
    completion_trace_log(format!("post_edit.other {}", self.completion_trace_state()));
    self.cancel_auto_signature_help();
    self.signature_help.clear();
    self.clear_completion_state_with_reason("post-edit-other");
  }

  fn lsp_supports_completion(&self) -> bool {
    self.lsp_runtimes.iter().any(|runtime| {
      runtime.ready
        && runtime
          .configured_server_name()
          .and_then(|server_name| runtime.runtime.server_capabilities(server_name))
          .is_some_and(|capabilities| capabilities.supports(LspCapability::Completion))
    })
  }

  fn lsp_supports_signature_help(&self) -> bool {
    self.lsp_runtimes.iter().any(|runtime| {
      runtime.ready
        && runtime
          .configured_server_name()
          .and_then(|server_name| runtime.runtime.server_capabilities(server_name))
          .is_some_and(|capabilities| capabilities.supports(LspCapability::SignatureHelp))
    })
  }

  fn lsp_provider_supports_single_char(&self, provider_key: &str, characters_key: &str, ch: char) -> bool {
    self.lsp_runtimes.iter().any(|runtime| {
      runtime.ready
        && runtime
          .configured_server_name()
          .and_then(|server_name| runtime.runtime.server_capabilities(server_name))
          .is_some_and(|capabilities| capabilities_support_single_char(capabilities.raw(), provider_key, characters_key, ch))
    })
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

  fn lsp_completion_server_supports_resolve(&self) -> bool {
    self.lsp_runtimes.iter().any(|runtime| {
      runtime.ready
        && runtime
          .configured_server_name()
          .and_then(|server_name| runtime.runtime.server_capabilities(server_name))
          .is_some_and(|capabilities| capabilities.supports_completion_item_resolve())
    })
  }

  fn schedule_auto_completion(&mut self, trigger: LspCompletionContext, delay: Duration) -> bool {
    if self.mode != Mode::Insert || !self.lsp_supports_completion() {
      completion_trace_log(format!(
        "schedule skip mode={:?} supports_completion={} trigger_kind={:?} trigger_char={:?}",
        self.mode,
        self.lsp_supports_completion(),
        trigger.trigger_kind,
        trigger.trigger_character,
      ));
      self.lsp_pending_auto_completion = None;
      return false;
    }
    completion_trace_log(format!(
      "schedule delay_ms={} trigger_kind={:?} trigger_char={:?} {}",
      delay.as_millis(),
      trigger.trigger_kind,
      trigger.trigger_character,
      self.completion_trace_state(),
    ));
    self.lsp_pending_auto_completion = Some(PendingAutoCompletion {
      due_at: Instant::now() + delay,
      trigger,
    });
    true
  }

  fn cancel_auto_completion(&mut self) {
    if self.lsp_pending_auto_completion.is_some() {
      completion_trace_log(format!("cancel_auto {}", self.completion_trace_state()));
    }
    self.lsp_pending_auto_completion = None;
  }

  fn schedule_auto_signature_help(&mut self, trigger: LspSignatureHelpContext, delay: Duration) -> bool {
    if self.mode != Mode::Insert || !self.lsp_supports_signature_help() {
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

  fn poll_lsp_completion_auto_trigger(&mut self) -> bool {
    let Some(pending) = self.lsp_pending_auto_completion.clone() else {
      return false;
    };
    if self.mode != Mode::Insert {
      completion_trace_log(format!("poll_auto drop-not-insert {}", self.completion_trace_state()));
      self.lsp_pending_auto_completion = None;
      return false;
    }
    if Instant::now() < pending.due_at {
      return false;
    }
    completion_trace_log(format!(
      "poll_auto fire trigger_kind={:?} trigger_char={:?} {}",
      pending.trigger.trigger_kind,
      pending.trigger.trigger_character,
      self.completion_trace_state(),
    ));
    self.lsp_pending_auto_completion = None;
    let _ = self.dispatch_completion_request(pending.trigger, false);
    false
  }

  fn poll_lsp_signature_help_auto_trigger(&mut self) -> bool {
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

  fn clear_completion_state(&mut self) {
    completion_trace_log(format!("clear.execute {}", self.completion_trace_state()));
    self.cancel_auto_completion();
    self.lsp_completion_items.clear();
    self.lsp_completion_raw_items.clear();
    self.lsp_completion_resolved.clear();
    self.lsp_completion_resolve_supported = false;
    self.lsp_completion_fallback_start = None;
    self.lsp_completion_visible_indices.clear();
    self.completion_menu.clear();
  }

  fn completion_filter_fragment(&self) -> Option<String> {
    let cursor = self.active_cursor_char_idx()?;
    let start = self.lsp_completion_fallback_start.unwrap_or(cursor).min(cursor);
    let text = self.editor.document().text();
    Some(text.slice(start..cursor).to_string())
  }

  fn completion_source_index_for_visible_index(&self, index: usize) -> Option<usize> {
    self.lsp_completion_visible_indices.get(index).copied()
  }

  fn completion_visible_index_for_source_index(&self, index: usize) -> Option<usize> {
    self.lsp_completion_visible_indices.iter().position(|visible| *visible == index)
  }

  fn rebuild_completion_menu(&mut self) {
    if self.lsp_completion_items.is_empty() {
      completion_trace_log(format!("rebuild raw_items=0 close {}", self.completion_trace_state()));
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

    self.lsp_completion_visible_indices = visible.iter().map(|(index, _)| *index).collect();
    completion_trace_log(format!(
      "rebuild fragment={:?} raw_items={} visible_items={} menu_active_before={} selected_before={:?}",
      fragment,
      self.lsp_completion_items.len(),
      self.lsp_completion_visible_indices.len(),
      self.completion_menu.active,
      self.completion_menu.selected,
    ));
    if self.lsp_completion_visible_indices.is_empty() {
      if self.completion_menu.active {
        completion_trace_log(format!("rebuild preserve-empty-visible {}", self.completion_trace_state()));
        return;
      }
      completion_trace_log(format!("rebuild close-empty-visible {}", self.completion_trace_state()));
      self.completion_menu.clear();
      return;
    }
    let items = self
      .lsp_completion_visible_indices
      .iter()
      .filter_map(|index| self.lsp_completion_items.get(*index))
      .map(completion_menu_item_for_lsp_item)
      .collect::<Vec<_>>();
    the_default::show_completion_menu(self, items);
    completion_trace_log(format!("rebuild show {}", self.completion_trace_state()));
  }

  fn dispatch_completion_request(&mut self, trigger: LspCompletionContext, announce_empty: bool) -> bool {
    let Some((uri, position)) = self.current_lsp_position() else {
      completion_trace_log("dispatch skip=no-lsp-position");
      return false;
    };
    let Some(cursor) = self.active_cursor_char_idx() else {
      completion_trace_log("dispatch skip=no-active-cursor");
      return false;
    };
    let replace_start = self.completion_replace_start_at_cursor(cursor);
    self.lsp_completion_generation = self.lsp_completion_generation.wrapping_add(1);
    let generation = self.lsp_completion_generation;
    completion_trace_log(format!(
      "dispatch generation={} trigger_kind={:?} trigger_char={:?} cursor={} replace_start={} announce_empty={} menu_active={} raw_items={} visible_items={}",
      generation,
      trigger.trigger_kind,
      trigger.trigger_character,
      cursor,
      replace_start,
      announce_empty,
      self.completion_menu.active,
      self.lsp_completion_items.len(),
      self.lsp_completion_visible_indices.len(),
    ));
    self.dispatch_lsp_request(
      "textDocument/completion",
      completion_params(&uri, position, &trigger),
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

  fn dispatch_signature_help_request(&mut self, context: LspSignatureHelpContext, announce_failures: bool) -> bool {
    if !self.lsp_supports_signature_help() {
      if announce_failures {
        self.push_warning("lsp", "signature help is not supported by the active server");
      }
      return false;
    }
    let Some((uri, position)) = self.current_lsp_position() else {
      if announce_failures {
        self.push_warning("lsp", "signature help unavailable: no active LSP document");
      }
      return false;
    };
    self.dispatch_lsp_request(
      "textDocument/signatureHelp",
      signature_help_params(&uri, position, &context),
      PendingLspRequestKind::SignatureHelp { uri },
    );
    true
  }

  fn handle_completion_response(
    &mut self,
    result: Option<&serde_json::Value>,
    generation: u64,
    request_cursor: usize,
    replace_start: usize,
    announce_empty: bool,
  ) -> bool {
    completion_trace_log(format!(
      "response generation={} current_generation={} request_cursor={} announce_empty={} mode={:?} {}",
      generation,
      self.lsp_completion_generation,
      request_cursor,
      announce_empty,
      self.mode,
      self.completion_trace_state(),
    ));
    if generation != self.lsp_completion_generation || self.mode != Mode::Insert {
      completion_trace_log(format!(
        "response ignore reason=stale-or-not-insert generation={} current_generation={} mode={:?}",
        generation,
        self.lsp_completion_generation,
        self.mode,
      ));
      return false;
    }
    let Some(current_cursor) = self.active_cursor_char_idx() else {
      completion_trace_log("response ignore reason=no-active-cursor");
      return false;
    };
    if current_cursor != request_cursor {
      completion_trace_log(format!(
        "response ignore reason=cursor-moved request_cursor={} current_cursor={}",
        request_cursor,
        current_cursor,
      ));
      return false;
    }

    let completion = match parse_completion_response_with_raw(result) {
      Ok(completion) => completion,
      Err(err) => {
        completion_trace_log(format!("response parse-error err={err}"));
        self.push_error("lsp", format!("failed to parse completion response: {err}"));
        return true;
      },
    };

    completion_trace_log(format!(
      "response parsed raw_items={} announce_empty={} menu_active_before={}",
      completion.items.len(),
      announce_empty,
      self.completion_menu.active,
    ));
    if completion.items.is_empty() {
      if !announce_empty && self.completion_menu.active {
        completion_trace_log(format!("response preserve-empty-auto {}", self.completion_trace_state()));
        return true;
      }
      self.clear_completion_state_with_reason("response-empty");
      if announce_empty {
        self.push_info("lsp", "no completion candidates");
      }
      return true;
    }

    self.lsp_completion_items = completion.items;
    self.lsp_completion_raw_items = completion.raw_items;
    self.lsp_completion_resolved.clear();
    self.lsp_completion_resolve_supported = self.lsp_completion_server_supports_resolve();
    self.lsp_completion_fallback_start = Some(replace_start.min(request_cursor));
    completion_trace_log(format!(
      "response apply raw_items={} fallback_start={:?} resolve_supported={}",
      self.lsp_completion_items.len(),
      self.lsp_completion_fallback_start,
      self.lsp_completion_resolve_supported,
    ));
    self.rebuild_completion_menu();
    true
  }

  fn resolve_completion_item_if_needed(&mut self, index: usize) {
    if !self.completion_menu.active || !self.lsp_completion_resolve_supported {
      return;
    }
    let Some(index) = self.completion_source_index_for_visible_index(index) else {
      return;
    };
    if self.lsp_completion_resolved.contains(&index) {
      return;
    }
    if index >= self.lsp_completion_items.len() || index >= self.lsp_completion_raw_items.len() {
      return;
    }
    let pending = self.lsp_runtimes.iter().any(|runtime| {
      runtime.pending_requests.values().any(|request| {
        matches!(request, PendingLspRequestKind::CompletionResolve { index: pending_index, .. } if *pending_index == index)
      })
    });
    if pending {
      return;
    }

    let Some(uri) = self.lsp_document.as_ref().map(|state| state.uri.clone()) else {
      return;
    };
    let params = self.lsp_completion_raw_items[index].clone();
    self.dispatch_lsp_request(
      "completionItem/resolve",
      params,
      PendingLspRequestKind::CompletionResolve { uri, index },
    );
  }

  fn handle_completion_resolve_response(&mut self, index: usize, response: &jsonrpc::Response) -> bool {
    if let Some(error) = response.error.as_ref() {
      self.lsp_completion_resolved.insert(index);
      self.push_warning("lsp", format!("completion resolve failed: {}", error.message));
      return true;
    }

    let resolved = match parse_completion_item_response(response.result.as_ref()) {
      Ok(item) => item,
      Err(err) => {
        self.push_warning("lsp", format!("failed to parse completion resolve response: {err}"));
        return true;
      },
    };

    self.lsp_completion_resolved.insert(index);
    let Some(resolved) = resolved else {
      return true;
    };
    let visible_index = self.completion_visible_index_for_source_index(index);
    let Some(item) = self.lsp_completion_items.get_mut(index) else {
      return true;
    };
    merge_resolved_completion_item(item, resolved);
    let updated_ui_item = completion_menu_item_for_lsp_item(item);
    if let Some(visible_index) = visible_index
      && let Some(ui_item) = self.completion_menu.items.get_mut(visible_index)
    {
      *ui_item = updated_ui_item;
    }
    self.request_render();
    true
  }

  fn apply_selected_completion(&mut self, index: usize) -> bool {
    let Some(source_index) = self.completion_source_index_for_visible_index(index) else {
      return false;
    };
    let Some(item) = self.lsp_completion_items.get(source_index).cloned() else {
      return false;
    };
    let prepared = normalize_completion_item_for_apply(item);
    let item = prepared.item;
    let doc = self.editor.document();
    let text = doc.text();
    let cursor = self.active_cursor_char_idx().unwrap_or(text.len_chars());
    let fallback_start = self.lsp_completion_fallback_start.unwrap_or(cursor).min(cursor);

    let (from, to, inserted_text) = if let Some(edit) = item.primary_edit.as_ref() {
      let from = utf16_position_to_char_idx(text, edit.range.start.line, edit.range.start.character);
      let to = utf16_position_to_char_idx(text, edit.range.end.line, edit.range.end.character);
      (from, to, completion_insert_text(&item, Some(&edit.new_text)))
    } else {
      (fallback_start, cursor, completion_insert_text(&item, None))
    };

    let Ok(transaction) = the_lib::transaction::Transaction::change(text, vec![(from, to, Some(inserted_text.clone().into()))])
    else {
      return false;
    };
    if !self.apply_transaction(&transaction) {
      return false;
    }

    let mapped_base = transaction.changes().map_pos(from, the_lib::transaction::Assoc::Before).ok();
    if let (Some(base), Some(range)) = (mapped_base, prepared.cursor_range.as_ref()) {
      set_completion_snippet_selection(self.editor.document_mut(), base, range);
    } else if let Some(base) = mapped_base {
      let _ = self.editor.document_mut().set_selection(Selection::point(base.saturating_add(inserted_text.chars().count())));
    }
    let _ = self.editor.document_mut().commit();
    self.clear_completion_state_with_reason("completion-applied");
    true
  }

  fn clear_hover_state(&mut self) {
    self.hover_docs = None;
  }

  fn close_docs_panels(&mut self) -> bool {
    let had_hover = self.hover_docs.is_some();
    let had_signature_help = self.signature_help.active;
    self.cancel_pending_lsp_requests_for("hover");
    self.cancel_pending_lsp_requests_for("signature-help");
    self.clear_hover_state();
    self.signature_help.clear();

    if self.mode == Mode::Insert {
      handle_key(
        self,
        KeyEvent {
          key: Key::Escape,
          modifiers: the_default::Modifiers::empty(),
        },
      );
      return true;
    }

    if had_hover || had_signature_help {
      self.request_render();
    }
    had_hover || had_signature_help
  }

  fn cancel_pending_lsp_requests_for(&mut self, target: &'static str) {
    for runtime in &mut self.lsp_runtimes {
      let ids = runtime
        .pending_requests
        .iter()
        .filter_map(|(id, pending)| (pending.cancellation_key() == target).then_some(*id))
        .collect::<Vec<_>>();
      for id in ids {
        runtime.pending_requests.remove(&id);
        let _ = runtime.runtime.cancel_request(id);
      }
    }
  }

  fn dispatch_lsp_request(&mut self, method: &'static str, params: serde_json::Value, pending: PendingLspRequestKind) {
    let Some((runtime_index, _)) = self
      .lsp_runtimes
      .iter()
      .enumerate()
      .find(|(_, runtime)| runtime.ready)
    else {
      self.push_error("lsp", format!("failed to dispatch {method}: no active language server"));
      return;
    };
    self.cancel_pending_lsp_requests_for(pending.cancellation_key());
    match self.lsp_runtimes[runtime_index].runtime.send_request(method, Some(params)) {
      Ok(request_id) => {
        self.lsp_runtimes[runtime_index]
          .pending_requests
          .insert(request_id, pending);
      },
      Err(err) => {
        self.push_error("lsp", format!("failed to dispatch {method}: {err}"));
      },
    }
  }

  fn handle_signature_help_response(&mut self, result: Option<&serde_json::Value>) -> bool {
    let signature = match parse_signature_help_response(result) {
      Ok(signature) => signature,
      Err(err) => {
        self.push_error("lsp", format!("failed to parse signature help response: {err}"));
        return true;
      },
    };

    let Some(signature) = signature else {
      self.signature_help.clear();
      return true;
    };

    if signature.signatures.is_empty() {
      self.signature_help.clear();
      return true;
    }

    let signatures = signature
      .signatures
      .into_iter()
      .map(|item| the_default::SignatureHelpItem {
        label: item.label,
        documentation: item.documentation,
        active_parameter: item.active_parameter,
        active_parameter_range: item.active_parameter_range,
      })
      .collect::<Vec<_>>();
    self.signature_help.set_signatures(signatures, signature.active_signature);
    true
  }

  fn handle_lsp_rpc_message(&mut self, runtime_index: usize, message: jsonrpc::Message) -> bool {
    let jsonrpc::Message::Response(response) = message else {
      return false;
    };
    let jsonrpc::Id::Number(id) = response.id else {
      return false;
    };
    let Some(kind) = self
      .lsp_runtimes
      .get_mut(runtime_index)
      .and_then(|runtime| runtime.pending_requests.remove(&id))
    else {
      return false;
    };

    if self.lsp_document.as_ref().map(|state| state.uri.as_str()) != Some(kind.uri()) {
      return false;
    }

    if let Some(error) = response.error {
      self.push_error("lsp", format!("lsp {} failed: {}", kind.label(), error.message));
      return true;
    }

    match kind {
      PendingLspRequestKind::Hover { .. } => {
        let hover = match parse_hover_response(response.result.as_ref()) {
          Ok(hover) => hover,
          Err(err) => {
            self.push_error("lsp", format!("failed to parse hover response: {err}"));
            return true;
          },
        };
        match hover {
          Some(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
              self.clear_hover_state();
            } else {
              self.hover_docs = Some(trimmed.to_string());
            }
          },
          None => self.clear_hover_state(),
        }
        true
      },
      PendingLspRequestKind::Completion {
        generation,
        cursor,
        replace_start,
        announce_empty,
        ..
      } => self.handle_completion_response(
        response.result.as_ref(),
        generation,
        cursor,
        replace_start,
        announce_empty,
      ),
      PendingLspRequestKind::CompletionResolve { index, .. } => {
        self.handle_completion_resolve_response(index, &response)
      },
      PendingLspRequestKind::SignatureHelp { .. } => self.handle_signature_help_response(response.result.as_ref()),
    }
  }

  fn lsp_statusline_text_value(&self) -> Option<String> {
    let has_server = !self.lsp_runtimes.is_empty();
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
        format!("lsp: {} {}", spinner_frame(self.lsp_spinner_index), detail_if_empty(detail, "starting"))
      },
      LspStatusPhase::Initializing => {
        format!("lsp: {} {}", spinner_frame(self.lsp_spinner_index), detail_if_empty(detail, "initializing"))
      },
      LspStatusPhase::Ready => {
        if detail.is_empty() {
          "lsp: ready".to_string()
        } else {
          format!("lsp: ready ({detail})")
        }
      },
      LspStatusPhase::Busy => {
        format!("lsp: {} {}", spinner_frame(self.lsp_spinner_index), detail_if_empty(detail, "working"))
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

  fn tick_lsp_statusline(&mut self) -> bool {
    if matches!(self.lsp_statusline.phase, LspStatusPhase::Busy)
      && self
        .lsp_runtimes
        .first()
        .is_some_and(|runtime| runtime.active_progress_tokens.is_empty() && runtime.ready)
    {
      self.set_lsp_status_for_runtime(0, LspStatusPhase::Ready, None);
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

  fn poll_lsp_events(&mut self) -> bool {
    let mut changed = false;
    for runtime_index in 0..self.lsp_runtimes.len() {
      loop {
        let event = {
          let Some(runtime) = self.lsp_runtimes.get(runtime_index) else {
            break;
          };
          runtime.runtime.try_recv_event()
        };
        let Some(event) = event else {
          break;
        };
        match event {
          LspEvent::Started { .. } => {
            self.set_lsp_status_for_runtime(runtime_index, LspStatusPhase::Starting, Some("starting".into()));
            changed = true;
          },
          LspEvent::ServerStarted { server_name, .. } => {
            if let Some(runtime) = self.lsp_runtimes.get_mut(runtime_index) {
              runtime.ready = false;
              runtime.opened_current_document = false;
              runtime.active_progress_tokens.clear();
              runtime.pending_requests.clear();
            }
            self.set_lsp_status_for_runtime(runtime_index, LspStatusPhase::Starting, Some(server_name));
            changed = true;
          },
          LspEvent::RequestDispatched { method, .. } => {
            if method == "initialize" {
              self.set_lsp_status_for_runtime(runtime_index, LspStatusPhase::Initializing, Some("initializing".into()));
              changed = true;
            }
          },
          LspEvent::CapabilitiesRegistered { server_name } => {
            if let Some(runtime) = self.lsp_runtimes.get_mut(runtime_index) {
              runtime.ready = true;
              runtime.active_progress_tokens.clear();
              runtime.pending_requests.clear();
            }
            self.open_current_document_for_runtime(runtime_index);
            self.set_lsp_status_for_runtime(runtime_index, LspStatusPhase::Ready, Some(server_name));
            changed = true;
          },
          LspEvent::Progress { progress } => {
            match progress.kind {
              LspProgressKind::Begin => {
                let text = format_lsp_progress_text(progress.title.as_deref(), progress.message.as_deref());
                if let Some(runtime) = self.lsp_runtimes.get_mut(runtime_index) {
                  runtime.active_progress_tokens.insert(progress.token);
                }
                self.set_lsp_status_for_runtime(runtime_index, LspStatusPhase::Busy, Some(text));
                changed = true;
              },
              LspProgressKind::Report => {
                let active = self
                  .lsp_runtimes
                  .get(runtime_index)
                  .is_some_and(|runtime| runtime.active_progress_tokens.contains(&progress.token));
                if active {
                  let text = format_lsp_progress_text(progress.title.as_deref(), progress.message.as_deref());
                  self.set_lsp_status_for_runtime(runtime_index, LspStatusPhase::Busy, Some(text));
                  changed = true;
                }
              },
              LspProgressKind::End => {
                if let Some(runtime) = self.lsp_runtimes.get_mut(runtime_index) {
                  runtime.active_progress_tokens.remove(&progress.token);
                  if runtime.ready && runtime.active_progress_tokens.is_empty() {
                    self.set_lsp_status_for_runtime(runtime_index, LspStatusPhase::Ready, None);
                    changed = true;
                  }
                }
              },
            }
          },
          LspEvent::ServerStopped { .. } | LspEvent::Stopped => {
            if let Some(runtime) = self.lsp_runtimes.get_mut(runtime_index) {
              runtime.ready = false;
              runtime.opened_current_document = false;
              runtime.active_progress_tokens.clear();
              runtime.pending_requests.clear();
            }
            self.set_lsp_status_for_runtime(runtime_index, LspStatusPhase::Starting, Some("restarting".into()));
            changed = true;
          },
          LspEvent::Error(message) => {
            if let Some(runtime) = self.lsp_runtimes.get_mut(runtime_index) {
              runtime.ready = false;
              runtime.opened_current_document = false;
              runtime.active_progress_tokens.clear();
              runtime.pending_requests.clear();
            }
            self.set_lsp_status_for_runtime(runtime_index, LspStatusPhase::Error, Some(summarize_lsp_error(&message)));
            self.push_error("lsp", message);
            changed = true;
          },
          LspEvent::RpcMessage { message } => {
            changed |= self.handle_lsp_rpc_message(runtime_index, message);
          },
          _ => {},
        }
      }
    }
    changed
  }

  fn clear_vcs_diff(&mut self) {
    self.vcs_diff = None;
    self.gutter_diff_signs.clear();
  }

  fn refresh_vcs_diff_document(&mut self) {
    let Some(handle) = self.vcs_diff.as_ref() else {
      return;
    };
    let _ = handle.update_document(self.editor.document().text().clone(), true);
    self.gutter_diff_signs = vcs_gutter_signs(handle);
  }

  fn apply_vcs_refresh_result(&mut self, statusline: Option<String>, diff_base: Option<Vec<u8>>) -> bool {
    let previous_statusline = self.vcs_statusline.clone();
    let previous_signs = self.gutter_diff_signs.clone();
    let previous_has_diff = self.vcs_diff.is_some();

    self.vcs_statusline = statusline;
    let Some(diff_base) = diff_base else {
      self.clear_vcs_diff();
      return self.vcs_statusline != previous_statusline || !previous_signs.is_empty() || previous_has_diff;
    };

    let diff_base = Rope::from_str(String::from_utf8_lossy(&diff_base).as_ref());
    let doc = self.editor.document().text().clone();
    let handle = DiffHandle::new(diff_base, doc);
    self.gutter_diff_signs = vcs_gutter_signs(&handle);
    self.vcs_diff = Some(handle);
    self.vcs_statusline != previous_statusline || self.gutter_diff_signs != previous_signs || !previous_has_diff
  }

  fn schedule_vcs_statusline_refresh(&mut self) {
    let Some(path) = self.file_path.clone() else {
      self.vcs_statusline = None;
      self.clear_vcs_diff();
      self.vcs_statusline_refresh_in_flight = false;
      return;
    };

    self.vcs_statusline = None;
    self.clear_vcs_diff();
    self.vcs_statusline_refresh_generation = self.vcs_statusline_refresh_generation.wrapping_add(1);
    self.vcs_statusline_refresh_in_flight = true;
    let generation = self.vcs_statusline_refresh_generation;
    let vcs_provider = self.vcs_provider.clone();
    let tx = self.vcs_statusline_refresh_tx.clone();

    std::thread::spawn(move || {
      let statusline = vcs_provider
        .get_statusline_info(&path)
        .map(|info| info.statusline_text());
      let diff_base = vcs_provider.get_diff_base(&path);
      let _ = tx.send(VcsStatuslineRefreshResult {
        generation,
        path,
        statusline,
        diff_base,
      });
    });
  }

  fn poll_vcs_statusline_refresh_results(&mut self) -> bool {
    let mut changed = false;
    loop {
      let result = match self.vcs_statusline_refresh_rx.try_recv() {
        Ok(result) => result,
        Err(mpsc::TryRecvError::Empty) | Err(mpsc::TryRecvError::Disconnected) => break,
      };

      if result.generation == self.vcs_statusline_refresh_generation {
        self.vcs_statusline_refresh_in_flight = false;
      }

      if self.file_path.as_deref() != Some(result.path.as_path())
        || result.generation != self.vcs_statusline_refresh_generation
      {
        continue;
      }

      changed |= self.apply_vcs_refresh_result(result.statusline, result.diff_base);
    }
    changed
  }

  fn sync_active_file_watch_state(&mut self) -> bool {
    let Some(path) = self.file_path.clone() else {
      return self.active_file_watch.take().is_some();
    };

    let current = self
      .active_file_watch
      .as_ref()
      .map(|watch| watch.stream.path.as_path());
    if current == Some(path.as_path()) {
      return false;
    }

    let (events_rx, watch_handle) = watch_path(&path, active_file_watch_latency());
    self.active_file_watch = Some(ActiveFileWatchState {
      stream: WatchedFileEventsState {
        path: path.clone(),
        uri: format!("file://{}", path.display()),
        events_rx,
        suppress_until: None,
        reload_state: FileWatchReloadState::Clean,
        reload_io: FileWatchReloadIoState::default(),
      },
      _watch_handle: watch_handle,
    });
    true
  }

  fn handle_active_file_watch_change(&mut self, watched_path: &Path, change_kind: PathEventKind) -> bool {
    let label = watched_path
      .file_name()
      .map(|name| name.to_string_lossy().to_string())
      .unwrap_or_else(|| watched_path.display().to_string());

    match change_kind {
      PathEventKind::Removed => {
        if let Some(watch) = self.active_file_watch.as_mut() {
          clear_reload_state(&mut watch.stream.reload_state);
        }
        self.push_warning("watch", format!("file deleted on disk: {label}"));
        true
      },
      PathEventKind::Created | PathEventKind::Changed => {
        let current = self.editor.document().text().clone();
        let buffer_modified = self.editor.document().flags().modified;
        let decision = match self.active_file_watch.as_mut() {
          Some(watch) => match evaluate_external_reload_from_disk(
            &mut watch.stream.reload_state,
            &mut watch.stream.reload_io,
            watched_path,
            &current,
            buffer_modified,
          ) {
            Ok(decision) => decision,
            Err(FileWatchReloadError::BackoffActive { .. }) => return false,
            Err(FileWatchReloadError::ReadFailed { error, retry_after, .. }) => {
              let retry_in_ms = retry_after.saturating_duration_since(Instant::now()).as_millis();
              self.push_warning(
                "watch",
                format!(
                  "failed to read '{label}' from disk: {error} (retrying in {retry_in_ms}ms)"
                ),
              );
              return true;
            },
          },
          None => return false,
        };

        match decision {
          FileWatchReloadDecision::Noop => false,
          FileWatchReloadDecision::ConflictEntered => {
            self.push_warning(
              "watch",
              format!(
                "file changed on disk: {label} (buffer has unsaved changes; run :rl to reload disk or :w! to overwrite disk)"
              ),
            );
            true
          },
          FileWatchReloadDecision::ConflictOngoing => false,
          FileWatchReloadDecision::ReloadNeeded => match <Self as DefaultContext>::reload_file_preserving_view(self, watched_path) {
            Ok(()) => {
              if let Some(watch) = self.active_file_watch.as_mut() {
                mark_reload_applied(&mut watch.stream.reload_state);
              }
              self.push_info("watch", format!("reloaded from disk: {label}"));
              true
            },
            Err(err) => {
              self.push_error("watch", format!("failed to reload '{label}': {err}"));
              true
            },
          },
        }
      },
    }
  }

  fn poll_active_file_watch(&mut self) -> bool {
    let mut changed = self.sync_active_file_watch_state();
    let outcome = match poll_watch_events(
      self.active_file_watch.as_mut().map(|watch| &mut watch.stream),
      Instant::now(),
      "swift",
      |_, _| {},
    ) {
      WatchPollOutcome::NoChanges => return changed,
      WatchPollOutcome::Disconnected { .. } => {
        self.active_file_watch = None;
        return self.sync_active_file_watch_state() || changed;
      },
      WatchPollOutcome::Changes { path, kinds, .. } => (path, kinds),
    };

    if let Some(change_kind) = outcome.1.last().copied() {
      changed |= self.handle_active_file_watch_change(&outcome.0, change_kind);
    }
    changed
  }

  fn poll_background_tasks(&mut self) -> bool {
    let mut changed = false;
    changed |= self.poll_active_file_watch();
    changed |= self.poll_lsp_events();
    changed |= self.poll_lsp_completion_auto_trigger();
    changed |= self.poll_lsp_signature_help_auto_trigger();
    changed |= self.tick_lsp_statusline();
    changed |= self.poll_vcs_statusline_refresh_results();
    changed |= self.refresh_picker_state();
    changed
  }

  fn new(path: Option<&Path>) -> Self {
    let workspace_root = path
      .map(resolved_workspace_root_for_path)
      .unwrap_or_else(default_workspace_root);
    let surface = SurfaceConfig::default();

    let mut document = Document::new(
      DocumentId::new(NonZeroUsize::new(1).expect("nonzero")),
      read_rope(path),
    );
    if let Some(path) = path {
      document.set_display_name(display_name_for_path(path));
    }

    let viewport = Rect::new(0, 0, surface.viewport_cols(), surface.viewport_rows());
    let view = ViewState::new(viewport, Position::new(0, 0));
    let mut editor = Editor::new(
      EditorId::new(NonZeroUsize::new(1).expect("nonzero")),
      document,
      view,
    );
    editor.set_active_file_path(path.map(Path::to_path_buf));

    let mut command_registry = CommandRegistry::new();
    install_default_wiring(&mut command_registry);

    let mut text_format = TextFormat::default();
    text_format.viewport_width = viewport.width;
    let ui_theme_catalog = ThemeCatalog::load(Some(&workspace_root));
    let (ui_theme_name, ui_theme) = select_ui_theme(&ui_theme_catalog);
    let loader = init_loader(&ui_theme).map(Arc::new).map_err(|err| {
      eprintln!("Warning: syntax highlighting unavailable: {err}");
      err
    }).ok();

    let (vcs_statusline_refresh_tx, vcs_statusline_refresh_rx) = mpsc::channel();

    let mut this = Self {
      editor,
      file_path: path.map(Path::to_path_buf),
      workspace_root: workspace_root.clone(),
      working_directory: WorkingDirectoryState {
        current:  Some(workspace_root),
        previous: None,
      },
      messages: MessageCenter::default(),
      mode: Mode::Normal,
      dispatch: Box::new(build_dispatch::<Self>()),
      keymaps: builtin_keymaps(),
      completion_menu_keymaps: builtin_completion_menu_keymaps(),
      command_registry,
      command_prompt: CommandPromptState::new(),
      command_palette: CommandPaletteState::default(),
      command_palette_style: CommandPaletteStyle::helix_bottom(),
      completion_menu: CompletionMenuState::default(),
      inline_completion: the_default::InlineCompletionState::default(),
      inline_completion_annotations: the_default::OwnedTextAnnotations::default(),
      file_tree: FileTreeState::default(),
      file_picker: FilePickerState::default(),
      picker_runtime_store: PickerRuntimeStore::default(),
      search_prompt: SearchPromptState::new(),
      signature_help: SignatureHelpState::default(),
      hover_docs: None,
      lsp_completion_items: Vec::new(),
      lsp_completion_raw_items: Vec::new(),
      lsp_completion_resolved: HashSet::new(),
      lsp_completion_resolve_supported: false,
      lsp_completion_generation: 0,
      lsp_completion_fallback_start: None,
      lsp_completion_visible_indices: Vec::new(),
      lsp_pending_auto_completion: None,
      lsp_pending_auto_signature_help: None,
      pending_input: None,
      registers: Registers::new(),
      register: None,
      macro_recording: None,
      macro_replaying: Vec::new(),
      macro_queue: VecDeque::new(),
      last_motion: None,
      text_format,
      soft_wrap_enabled: false,
      gutter_config: swift_gutter_config(),
      loader,
      lsp_document: None,
      lsp_runtimes: Vec::new(),
      lsp_statusline: LspStatuslineState::off(Some("unavailable".into())),
      lsp_spinner_index: 0,
      lsp_spinner_last_tick: Instant::now(),
      highlight_cache: HighlightCache::default(),
      file_picker_preview_visible_rows: 20,
      ui_theme_catalog,
      ui_theme_name: ui_theme_name.clone(),
      ui_theme_base: ui_theme.clone(),
      ui_theme_preview_name: None,
      ui_theme,
      render_generation_state: None,
      frame_generation_state: FrameGenerationState::default(),
      render_theme_generation: 0,
      surface,
      vcs_provider: DiffProviderRegistry::default(),
      vcs_statusline: None,
      gutter_diff_signs: BTreeMap::new(),
      vcs_diff: None,
      active_file_watch: None,
      vcs_statusline_refresh_in_flight: false,
      vcs_statusline_refresh_generation: 0,
      vcs_statusline_refresh_tx,
      vcs_statusline_refresh_rx,
    };

    set_file_picker_syntax_loader(&mut this.file_picker, this.loader.clone());
    this.refresh_active_document_syntax();
    this.refresh_lsp_runtime_state();
    this.sync_active_file_watch_state();
    this.schedule_vcs_statusline_refresh();
    this
  }

  fn open_path(&mut self, path: &Path) -> bool {
    match fs::read_to_string(path) {
      Ok(contents) => {
        let replaced = self
          .editor
          .replace_active_buffer(Rope::from_str(&contents), Some(path.to_path_buf()));
        self.file_path = Some(path.to_path_buf());
        self.editor.set_active_file_path(Some(path.to_path_buf()));
        self.editor
          .document_mut()
          .set_display_name(display_name_for_path(path));
        self.update_workspace_for_path(path);
        self.reload_theme_catalog();
        self.refresh_active_document_syntax();
        self.refresh_lsp_runtime_state();
        self.sync_active_file_watch_state();
        self.schedule_vcs_statusline_refresh();
        replaced
      },
      Err(err) => {
        self.push_error("open", format!("failed to open {}: {err}", path.display()));
        false
      },
    }
  }

  fn configure_surface(&mut self, config: the_editor_surface_config_t) -> bool {
    let surface = SurfaceConfig::from_ffi(config);
    let cols = surface.viewport_cols();
    let rows = surface.viewport_rows();
    let changed = self.surface.width_px != surface.width_px
      || self.surface.height_px != surface.height_px
      || self.surface.metrics.backing_scale != surface.metrics.backing_scale
      || self.surface.metrics.cell_width_px != surface.metrics.cell_width_px
      || self.surface.metrics.cell_height_px != surface.metrics.cell_height_px
      || self.surface.metrics.cell_baseline_px != surface.metrics.cell_baseline_px
      || self.surface.metrics.underline_position_px != surface.metrics.underline_position_px
      || self.surface.metrics.underline_thickness_px != surface.metrics.underline_thickness_px
      || self.surface.metrics.cursor_thickness_px != surface.metrics.cursor_thickness_px
      || self.editor.view().viewport.width != cols
      || self.editor.view().viewport.height != rows;
    self.surface = surface;
    let viewport = Rect::new(0, 0, cols, rows);
    self.editor.set_layout_viewport(viewport);
    self.editor.view_mut().viewport = viewport;
    self.sync_text_viewport_width();
    self.clamp_scroll();
    changed
  }

  fn set_viewport(&mut self, cols: u16, rows: u16) {
    let cols = cols.max(1);
    let rows = rows.max(1);
    let viewport = Rect::new(0, 0, cols, rows);
    self.editor.set_layout_viewport(viewport);
    self.editor.view_mut().viewport = viewport;
    self.sync_text_viewport_width();
    self.clamp_scroll();
  }

  fn set_scroll_row(&mut self, row: u32) -> bool {
    let max_row = max_scroll_row(
      self.editor.document().text().len_lines(),
      self.editor.view().viewport.height as usize,
    ) as u32;
    let next = row.min(max_row) as usize;
    if next == self.editor.view().scroll.row {
      return false;
    }
    self.editor.view_mut().scroll.row = next;
    true
  }

  fn set_scroll_col(&mut self, col: u32) -> bool {
    let max_col = max_scroll_col(self.editor.document(), &self.text_format, self.content_viewport_width() as usize) as u32;
    let next = col.min(max_col) as usize;
    if next == self.editor.view().scroll.col {
      return false;
    }
    self.editor.view_mut().scroll.col = next;
    true
  }

  fn handle_key_event(&mut self, raw: the_editor_key_event_t) -> bool {
    let Some(event) = translate_key_event(raw) else {
      return false;
    };
    let completion_key = event.key;
    let was_insert_mode = self.mode == Mode::Insert;
    handle_key(self, event);
    if self.mode == Mode::Insert {
      if was_insert_mode {
        match completion_key {
          Key::Char(ch) => self.handle_insert_mode_char_post_edit(ch),
          Key::Backspace | Key::Delete => self.handle_insert_mode_delete_post_edit(),
          Key::Enter | Key::NumpadEnter => self.handle_insert_mode_other_post_edit(),
          _ => {},
        }
      }
    } else {
      self.cancel_auto_signature_help();
      self.signature_help.clear();
      self.clear_completion_state_with_reason("left-insert-mode-key-event");
    }
    self.ensure_cursor_visible();
    true
  }

  fn insert_text(&mut self, text: &str) -> bool {
    let mut changed = false;
    let mut last_key = None;
    for ch in text.chars() {
      let key = match ch {
        '\n' => Key::Enter,
        '\t' => Key::Tab,
        _ => Key::Char(ch),
      };
      let event = KeyEvent {
        key,
        modifiers: the_default::Modifiers::empty(),
      };
      handle_key(self, event);
      changed = true;
      last_key = Some(key);
    }
    if changed {
      if self.mode == Mode::Insert {
        match last_key {
          Some(Key::Char(ch)) => self.handle_insert_mode_char_post_edit(ch),
          Some(Key::Backspace | Key::Delete) => self.handle_insert_mode_delete_post_edit(),
          Some(Key::Enter | Key::NumpadEnter | Key::Tab) | None => self.handle_insert_mode_other_post_edit(),
          _ => {},
        }
      } else {
        self.cancel_auto_signature_help();
        self.signature_help.clear();
        self.clear_completion_state_with_reason("left-insert-mode-insert-text");
      }
      self.ensure_cursor_visible();
    }
    changed
  }

  fn toggle_command_palette(&mut self) -> bool {
    if self.command_palette.is_open {
      self.close_command_palette()
    } else {
      open_command_palette(self);
      true
    }
  }

  fn close_command_palette(&mut self) -> bool {
    if !self.command_palette.is_open {
      return false;
    }
    handle_command_prompt_key(
      self,
      KeyEvent {
        key: Key::Escape,
        modifiers: the_default::Modifiers::empty(),
      },
    )
  }

  fn set_command_palette_query(&mut self, query: &str) -> bool {
    if !self.command_palette.is_open {
      open_command_palette(self);
    }
    command_palette_debug_log(format!(
      "set_query incoming={:?} prompt_before={:?} palette_query_before={:?} prompt_text_before={:?}",
      query,
      self.command_prompt.input,
      self.command_palette.query,
      self.command_palette.prompt_text,
    ));
    update_command_palette_for_input(self, query);
    command_palette_debug_log(format!(
      "set_query applied prompt_after={:?} palette_query_after={:?} prompt_text_after={:?} prefiltered={} selected={:?}",
      self.command_prompt.input,
      self.command_palette.query,
      self.command_palette.prompt_text,
      self.command_palette.prefiltered,
      self.command_palette.selected,
    ));
    true
  }

  fn move_command_palette_selection(&mut self, next: bool) -> bool {
    if !self.command_palette.is_open {
      return false;
    }
    let filtered = command_palette_filtered_indices(&self.command_palette);
    let Some(next_index) = (if filtered.is_empty() {
      None
    } else {
      let current = self
        .command_palette
        .selected
        .and_then(|sel| filtered.iter().position(|&idx| idx == sel));
      Some(match (next, current) {
        (true, Some(current)) => {
          if current >= filtered.len() - 1 { 0 } else { current + 1 }
        },
        (true, None) => 0,
        (false, Some(current)) => {
          if current == 0 { filtered.len() - 1 } else { current - 1 }
        },
        (false, None) => filtered.len() - 1,
      })
    }) else {
      return false;
    };
    self.command_palette.selected = Some(filtered[next_index]);
    command_palette_debug_log(format!(
      "ffi_move_selection next={} selected={:?} next_index={} filtered_len={}",
      next,
      self.command_palette.selected,
      next_index,
      filtered.len(),
    ));
    sync_command_palette_preview(self);
    true
  }

  fn select_command_palette_visible_index(&mut self, visible_index: usize) -> bool {
    if !self.command_palette.is_open {
      return false;
    }
    let filtered = command_palette_filtered_indices(&self.command_palette);
    let Some(item_index) = filtered.get(visible_index).copied() else {
      return false;
    };
    self.command_palette.selected = Some(item_index);
    sync_command_palette_preview(self);
    true
  }

  fn submit_command_palette(&mut self) -> bool {
    if !self.command_palette.is_open {
      return false;
    }
    command_palette_debug_log(format!(
      "submit prompt={:?} palette_query={:?} prompt_text={:?} prefiltered={} selected={:?} items_len={}",
      self.command_prompt.input,
      self.command_palette.query,
      self.command_palette.prompt_text,
      self.command_palette.prefiltered,
      self.command_palette.selected,
      self.command_palette.items.len(),
    ));

    if matches!(self.command_palette.source, the_default::CommandPaletteSource::CommandLine)
      && !self.command_palette.prefiltered
      && self.command_prompt.input.trim().trim_start_matches(':').is_empty()
      && let Some(item_idx) = self.command_palette.selected
      && let Some(command_name) = self.command_palette.items.get(item_idx).map(|item| item.title.clone())
      && self
        .command_registry_ref()
        .get(&command_name)
        .is_some_and(|command| {
          let (min, max) = command.signature.positionals;
          min > 0
            || max != Some(0)
            || command.signature.raw_after.is_some()
            || !command.signature.flags.is_empty()
        })
    {
      let input = format!("{command_name} ");
      command_palette_debug_log(format!("ffi_submit expanding empty prompt to {:?}", input));
      update_command_palette_for_input(self, &input);
      self.command_palette.selected = None;
      return true;
    }

    let submitted = submit_command_palette_action(self);
    command_palette_debug_log(format!(
      "submit result={} mode={:?} prompt_after={:?} palette_open={} palette_query_after={:?} prompt_text_after={:?}",
      submitted,
      self.mode,
      self.command_prompt.input,
      self.command_palette.is_open,
      self.command_palette.query,
      self.command_palette.prompt_text,
    ));
    submitted
  }

  fn update_workspace_for_path(&mut self, path: &Path) {
    let previous_workspace_root = self.workspace_root.clone();
    let follow_workspace_root = self
      .working_directory
      .current
      .as_ref()
      .is_none_or(|current| current == &previous_workspace_root);
    self.workspace_root = resolved_workspace_root_for_path(path);
    if follow_workspace_root {
      self.working_directory.current = Some(self.workspace_root.clone());
    }
  }

  fn configure_file_picker_layout(&mut self, list_visible_rows: usize, preview_visible_rows: usize) -> bool {
    let list_visible_rows = list_visible_rows.max(1);
    let preview_visible_rows = preview_visible_rows.max(1);
    let changed = self.file_picker.list_visible != list_visible_rows
      || self.file_picker_preview_visible_rows != preview_visible_rows;
    set_picker_visible_rows(&mut self.file_picker, list_visible_rows);
    self.file_picker_preview_visible_rows = preview_visible_rows;
    changed
  }

  fn close_file_picker(&mut self) -> bool {
    if !self.file_picker.active {
      return false;
    }
    close_file_picker(self);
    true
  }

  fn set_file_picker_query(&mut self, query: &str) -> bool {
    if !self.file_picker.active {
      return false;
    }
    let dynamic = set_file_picker_query_text(&mut self.file_picker, query);
    self.file_picker_selection_changed();
    if dynamic {
      notify_file_picker_query_changed(self, query);
    }
    true
  }

  fn move_file_picker_selection(&mut self, next: bool) -> bool {
    if !self.file_picker.active {
      return false;
    }
    move_selection(self, if next { 1 } else { -1 });
    true
  }

  fn set_file_picker_list_offset(&mut self, offset: usize) -> bool {
    if !self.file_picker.active {
      return false;
    }
    set_file_picker_list_offset(self, offset);
    true
  }

  fn set_file_picker_preview_offset(&mut self, offset: usize) -> bool {
    if !self.file_picker.active {
      return false;
    }
    set_file_picker_preview_offset(self, offset, self.file_picker_preview_visible_rows);
    true
  }

  fn select_file_picker_index(&mut self, index: usize) -> bool {
    if !self.file_picker.active || index >= self.file_picker.matched_count() {
      return false;
    }
    select_file_picker_index(self, index);
    true
  }

  fn submit_file_picker(&mut self) -> bool {
    if !self.file_picker.active {
      return false;
    }
    submit_file_picker(self);
    true
  }

  fn refresh_picker_state(&mut self) -> bool {
    if !self.file_picker.active {
      return false;
    }
    poll_scan_results(&mut self.file_picker)
  }

  fn refresh_active_document_syntax(&mut self) {
    self.highlight_cache.clear();
    let Some(loader) = self.loader.clone() else {
      self.editor.document_mut().clear_syntax();
      return;
    };
    let Some(path) = self.file_path.clone() else {
      self.editor.document_mut().clear_syntax();
      return;
    };
    let doc = self.editor.document_mut();
    if let Err(err) = setup_syntax(doc, &path, &loader) {
      command_palette_debug_log(format!("syntax setup skipped for {}: {}", path.display(), err));
      doc.clear_syntax();
    }
  }

  fn apply_effective_theme(&mut self, theme: Theme) {
    self.ui_theme = theme;
    if let Some(loader) = &self.loader {
      loader.set_scopes(self.ui_theme.scopes().to_vec());
    }
    self.highlight_cache.clear();
    self.render_theme_generation = self.render_theme_generation.wrapping_add(1);
  }

  fn set_ui_theme_named(&mut self, theme_name: &str) -> Result<(), String> {
    let started = Instant::now();
    let theme = self
      .ui_theme_catalog
      .load_theme(theme_name)
      .ok_or_else(|| format!("Could not load theme: {theme_name}"))?;
    let load_elapsed = started.elapsed();
    self.ui_theme_name = theme_name.to_string();
    self.ui_theme_base = theme.clone();
    self.ui_theme_preview_name = None;
    self.apply_effective_theme(theme);
    theme_perf_log(format!(
      "set_ui_theme name={} load_ms={:.2}",
      theme_name,
      load_elapsed.as_secs_f64() * 1000.0,
    ));
    Ok(())
  }

  fn set_ui_theme_preview_named(&mut self, theme_name: &str) -> Result<(), String> {
    let started = Instant::now();
    let theme = self
      .ui_theme_catalog
      .load_theme(theme_name)
      .ok_or_else(|| format!("Could not load theme: {theme_name}"))?;
    let load_elapsed = started.elapsed();
    self.ui_theme_preview_name = Some(theme_name.to_string());
    self.apply_effective_theme(theme);
    theme_perf_log(format!(
      "set_ui_theme_preview name={} load_ms={:.2}",
      theme_name,
      load_elapsed.as_secs_f64() * 1000.0,
    ));
    Ok(())
  }

  fn clear_ui_theme_preview_state(&mut self) {
    if self.ui_theme_preview_name.take().is_some() {
      self.apply_effective_theme(self.ui_theme_base.clone());
    }
  }

  fn reload_theme_catalog(&mut self) {
    let started = Instant::now();
    self.ui_theme_catalog = ThemeCatalog::load(Some(&self.workspace_root));
    let current_name = self.ui_theme_name.clone();
    let preview_name = self.ui_theme_preview_name.clone();
    if self.set_ui_theme_named(&current_name).is_err() {
      let default_name = default_theme().name().to_string();
      if let Some(theme) = self.ui_theme_catalog.load_theme(&default_name) {
        self.ui_theme_name = default_name;
        self.ui_theme_base = theme.clone();
        self.ui_theme_preview_name = None;
        self.apply_effective_theme(theme);
      }
    }
    if let Some(preview_name) = preview_name {
      let _ = self.set_ui_theme_preview_named(&preview_name);
    }
    theme_perf_log(format!(
      "reload_theme_catalog root={} names={} total_ms={:.2}",
      self.workspace_root.display(),
      self.ui_theme_catalog.names().len(),
      started.elapsed().as_secs_f64() * 1000.0,
    ));
  }

  fn render_styles(&self) -> RenderStyles {
    let theme = &self.ui_theme;
    RenderStyles {
      selection:                  theme.try_get("ui.selection").unwrap_or_default(),
      cursor:                     theme.try_get("ui.cursor").unwrap_or_default(),
      active_cursor:              theme
        .try_get("ui.cursor.active")
        .or_else(|| theme.try_get("ui.cursor"))
        .unwrap_or_default(),
      cursor_kind:                CursorKind::Bar,
      active_cursor_kind:         CursorKind::Block,
      non_block_cursor_uses_head: true,
      gutter:                     theme.try_get("ui.linenr").unwrap_or_default(),
      gutter_active:              theme
        .try_get("ui.linenr.selected")
        .or_else(|| theme.try_get("ui.linenr"))
        .unwrap_or_default(),
    }
  }

  fn clamp_scroll(&mut self) {
    let max_row = max_scroll_row(
      self.editor.document().text().len_lines(),
      self.editor.view().viewport.height as usize,
    );
    let max_col = max_scroll_col(self.editor.document(), &self.text_format, self.content_viewport_width() as usize);
    let scroll = self.editor.view().scroll;
    if scroll.row > max_row || scroll.col > max_col {
      let view = self.editor.view_mut();
      view.scroll.row = view.scroll.row.min(max_row);
      view.scroll.col = view.scroll.col.min(max_col);
    }
  }

  fn ensure_cursor_visible(&mut self) {
    let text = self.editor.document().text();
    let selection = self.editor.document().selection();
    let cursor_pos = self
      .editor
      .view()
      .active_cursor
      .and_then(|cursor_id| selection.range_by_id(cursor_id).copied())
      .or_else(|| selection.ranges().first().copied())
      .map(|range| range.cursor(text.slice(..)));
    let Some(cursor_pos) = cursor_pos else {
      return;
    };

    let cursor_line = text.char_to_line(cursor_pos);
    let cursor_col = cursor_pos.saturating_sub(text.line_to_char(cursor_line));
    let viewport_height = self.editor.view().viewport.height as usize;
    let gutter_width = gutter_width_for_document(self.editor.document(), self.editor.view().viewport.width, &self.gutter_config) as usize;
    let viewport_width = self.editor.view().viewport.width.saturating_sub(gutter_width as u16).max(1) as usize;
    if let Some(new_scroll) = the_lib::view::scroll_to_keep_visible(
      cursor_line,
      cursor_col,
      self.editor.view().scroll,
      viewport_height,
      viewport_width,
      SWIFT_SCROLLOFF,
    ) {
      self.editor.view_mut().scroll = new_scroll;
    }
  }

  fn build_snapshot(&mut self) -> OwnedSnapshot {
    self.poll_vcs_statusline_refresh_results();
    let _ = self.refresh_picker_state();
    let styles = self.render_styles();
    let frame = the_default::frame_render_plan_with_styles(self, styles);
    let plan = frame.active_plan();
    OwnedSnapshot::from_editor(self, plan)
  }

  fn open_search_prompt(&mut self) -> bool {
    handle_command(self, Command::Search);
    self.search_prompt.active
  }

  fn close_input_prompt(&mut self) -> bool {
    if !self.search_prompt.active {
      return false;
    }
    handle_search_prompt_key(self, KeyEvent {
      key: Key::Escape,
      modifiers: the_default::Modifiers::empty(),
    })
  }

  fn set_input_prompt_query(&mut self, query: &str) -> bool {
    if !self.search_prompt.active || self.search_prompt.query == query {
      return false;
    }
    self.search_prompt.query = query.to_string();
    self.search_prompt.cursor = self.search_prompt.query.len();
    self.search_prompt.selected = None;
    update_search_prompt_preview(self);
    true
  }

  fn submit_input_prompt(&mut self) -> bool {
    if !self.search_prompt.active {
      return false;
    }
    handle_search_prompt_key(self, KeyEvent {
      key: Key::Enter,
      modifiers: the_default::Modifiers::empty(),
    })
  }

  fn step_input_prompt(&mut self, direction: the_default::Direction) -> bool {
    if !self.search_prompt.active || self.search_prompt.kind != SearchPromptKind::Search {
      return false;
    }
    step_search_prompt(self, direction);
    true
  }

  fn primary_selection_utf16(&self) -> (u32, u32) {
    let selection = self.editor.document().selection();
    let Ok((_, range)) = selection.pick(CursorPick::First) else {
      return (0, 0);
    };
    let prefix = self.editor.document().text().slice(..range.from()).to_string();
    let selected = self.editor.document().text().slice(range.from()..range.to()).to_string();
    (
      prefix.encode_utf16().count() as u32,
      selected.encode_utf16().count() as u32,
    )
  }

  fn primary_selection_text(&self) -> String {
    let selection = self.editor.document().selection();
    let Ok((_, range)) = selection.pick(CursorPick::First) else {
      return String::new();
    };
    self.editor.document().text().slice(range.from()..range.to()).to_string()
  }
}

impl DefaultContext for SwiftEditor {
  fn editor(&mut self) -> &mut Editor { &mut self.editor }
  fn editor_ref(&self) -> &Editor { &self.editor }
  fn file_path(&self) -> Option<&Path> { self.file_path.as_deref() }
  fn workspace_root(&self) -> PathBuf { self.workspace_root.clone() }
  fn working_directory_state(&self) -> &WorkingDirectoryState { &self.working_directory }
  fn working_directory_state_mut(&mut self) -> &mut WorkingDirectoryState { &mut self.working_directory }
  fn request_render(&mut self) {}
  fn render_waker(&self) -> the_default::RenderWaker {
    let (tx, _rx) = mpsc::channel();
    the_default::RenderWaker::new(tx)
  }
  fn apply_transaction(&mut self, transaction: &the_lib::transaction::Transaction) -> bool {
    let old_text_for_lsp = self.editor.document().text().clone();
    if self
      .editor
      .apply_transaction_to_active_buffer(transaction, self.loader.as_deref())
      .is_err()
    {
      return false;
    }
    if !transaction.changes().is_empty() {
      self.highlight_cache.clear();
      self.lsp_send_did_change(&old_text_for_lsp, transaction.changes());
      self.refresh_vcs_diff_document();
    }
    true
  }
  fn messages(&self) -> &MessageCenter { &self.messages }
  fn messages_mut(&mut self) -> &mut MessageCenter { &mut self.messages }
  fn build_render_plan(&mut self) -> RenderPlan {
    self.build_render_plan_with_styles(self.render_styles())
  }
  fn build_render_plan_with_styles(&mut self, styles: RenderStyles) -> RenderPlan {
    let view = self.editor.view();
    let mut text_format = self.text_format.clone();
    text_format.viewport_width = self.content_viewport_width();
    let mut annotations = TextAnnotations::default();
    let loader = self.loader.clone();
    let line_range = view.scroll.row..(view.scroll.row + view.viewport.height as usize);
    let (document, cache) = self.editor.document_and_cache();
    let mut plan = if let (Some(loader), Some(syntax)) = (loader.as_deref(), document.syntax()) {
      let mut adapter = SyntaxHighlightAdapter::new(
        document.text().slice(..),
        syntax,
        loader,
        &mut self.highlight_cache,
        line_range,
        document.version(),
        document.syntax_version(),
        true,
      );
      build_plan(
        document,
        view,
        &text_format,
        &self.gutter_config,
        &mut annotations,
        &mut adapter,
        cache,
        styles,
      )
    } else {
      let mut highlights = NoHighlights;
      build_plan(
        document,
        view,
        &text_format,
        &self.gutter_config,
        &mut annotations,
        &mut highlights,
        cache,
        styles,
      )
    };
    let diff_styles = render_diff_styles_from_theme(&self.ui_theme);
    let diff_signs = self.gutter_diff_signs.clone();
    apply_swift_diff_gutter_markers(&mut plan, &diff_signs, diff_styles);
    let row_hashes = base_render_layer_row_hashes(&plan);
    let generation_state = finish_render_generations(
      &mut plan,
      self.render_generation_state.as_ref(),
      self.render_theme_generation,
      row_hashes,
    );
    self.render_generation_state = Some(generation_state);
    plan
  }
  fn build_frame_render_plan(&mut self) -> FrameRenderPlan {
    self.build_frame_render_plan_with_styles(self.render_styles())
  }
  fn build_frame_render_plan_with_styles(&mut self, styles: RenderStyles) -> FrameRenderPlan {
    let mut frame = FrameRenderPlan::from_active_plan(self.build_render_plan_with_styles(styles));
    let pane_state = self.render_generation_state.clone().unwrap_or_default();
    let pane_id = frame.active_pane;
    let pane_states = std::iter::once((pane_id, pane_state)).collect();
    self.frame_generation_state = the_lib::render::finish_frame_generations(
      &mut frame,
      Some(&self.frame_generation_state),
      pane_states,
    );
    frame
  }
  fn request_quit(&mut self) {}
  fn mode(&self) -> Mode { self.mode }
  fn set_mode(&mut self, mode: Mode) { self.mode = mode; }
  fn keymaps(&mut self) -> &mut Keymaps { &mut self.keymaps }
  fn command_prompt_mut(&mut self) -> &mut CommandPromptState { &mut self.command_prompt }
  fn command_prompt_ref(&self) -> &CommandPromptState { &self.command_prompt }
  fn command_registry_mut(&mut self) -> &mut CommandRegistry<Self> { &mut self.command_registry }
  fn command_registry_ref(&self) -> &CommandRegistry<Self> { &self.command_registry }
  fn command_palette(&self) -> &CommandPaletteState { &self.command_palette }
  fn command_palette_mut(&mut self) -> &mut CommandPaletteState { &mut self.command_palette }
  fn command_palette_style(&self) -> &CommandPaletteStyle { &self.command_palette_style }
  fn command_palette_style_mut(&mut self) -> &mut CommandPaletteStyle { &mut self.command_palette_style }
  fn completion_menu(&self) -> &CompletionMenuState { &self.completion_menu }
  fn completion_menu_mut(&mut self) -> &mut CompletionMenuState { &mut self.completion_menu }
  fn completion_menu_keymaps(&self) -> &Keymaps { &self.completion_menu_keymaps }
  fn completion_menu_keymaps_mut(&mut self) -> &mut Keymaps { &mut self.completion_menu_keymaps }
  fn completion_accept_selected(&mut self, index: usize) -> bool { self.apply_selected_completion(index) }
  fn completion_selection_changed(&mut self, index: usize) {
    self.resolve_completion_item_if_needed(index);
  }
  fn completion_menu_closed(&mut self) {
    completion_trace_log(format!("menu_closed {}", self.completion_trace_state()));
    self.lsp_completion_items.clear();
    self.lsp_completion_raw_items.clear();
    self.lsp_completion_resolved.clear();
    self.lsp_completion_resolve_supported = false;
    self.lsp_completion_fallback_start = None;
    self.lsp_completion_visible_indices.clear();
  }
  fn inline_completion(&self) -> &the_default::InlineCompletionState { &self.inline_completion }
  fn inline_completion_mut(&mut self) -> &mut the_default::InlineCompletionState { &mut self.inline_completion }
  fn set_inline_completion_annotations(&mut self, annotations: the_default::OwnedTextAnnotations) { self.inline_completion_annotations = annotations; }
  fn clear_inline_completion_annotations(&mut self) { self.inline_completion_annotations = the_default::OwnedTextAnnotations::default(); }
  fn file_tree(&self) -> &FileTreeState { &self.file_tree }
  fn file_tree_mut(&mut self) -> &mut FileTreeState { &mut self.file_tree }
  fn file_picker(&self) -> &FilePickerState { &self.file_picker }
  fn file_picker_mut(&mut self) -> &mut FilePickerState { &mut self.file_picker }
  fn picker_runtime_store(&self) -> &PickerRuntimeStore<Self> { &self.picker_runtime_store }
  fn picker_runtime_store_mut(&mut self) -> &mut PickerRuntimeStore<Self> { &mut self.picker_runtime_store }
  fn search_prompt_ref(&self) -> &SearchPromptState { &self.search_prompt }
  fn search_prompt_mut(&mut self) -> &mut SearchPromptState { &mut self.search_prompt }
  fn signature_help(&self) -> Option<&SignatureHelpState> { Some(&self.signature_help) }
  fn signature_help_mut(&mut self) -> Option<&mut SignatureHelpState> { Some(&mut self.signature_help) }
  fn dispatch(&self) -> DispatchRef<Self> { DispatchRef::from_ptr(self.dispatch.as_ref() as *const dyn DefaultApi<Self>) }
  fn pending_input(&self) -> Option<&PendingInput> { self.pending_input.as_ref() }
  fn set_pending_input(&mut self, pending: Option<PendingInput>) { self.pending_input = pending; }
  fn registers(&self) -> &Registers { &self.registers }
  fn registers_mut(&mut self) -> &mut Registers { &mut self.registers }
  fn register(&self) -> Option<char> { self.register }
  fn set_register(&mut self, register: Option<char>) { self.register = register; }
  fn macro_recording(&self) -> &Option<(char, Vec<KeyBinding>)> { &self.macro_recording }
  fn set_macro_recording(&mut self, recording: Option<(char, Vec<KeyBinding>)>) { self.macro_recording = recording; }
  fn macro_replaying(&self) -> &Vec<char> { &self.macro_replaying }
  fn macro_replaying_mut(&mut self) -> &mut Vec<char> { &mut self.macro_replaying }
  fn macro_queue(&self) -> &VecDeque<KeyEvent> { &self.macro_queue }
  fn macro_queue_mut(&mut self) -> &mut VecDeque<KeyEvent> { &mut self.macro_queue }
  fn last_motion(&self) -> Option<Motion> { self.last_motion }
  fn set_last_motion(&mut self, motion: Option<Motion>) { self.last_motion = motion; }
  fn text_format(&self) -> TextFormat { self.text_format.clone() }
  fn soft_wrap_enabled(&self) -> bool { self.soft_wrap_enabled }
  fn set_soft_wrap_enabled(&mut self, enabled: bool) { self.soft_wrap_enabled = enabled; self.text_format.soft_wrap = enabled; }
  fn gutter_config(&self) -> &GutterConfig { &self.gutter_config }
  fn gutter_config_mut(&mut self) -> &mut GutterConfig { &mut self.gutter_config }
  fn text_annotations(&self) -> TextAnnotations<'_> { TextAnnotations::default() }
  fn syntax_loader(&self) -> Option<&the_lib::syntax::Loader> { self.loader.as_deref() }
  fn scrolloff(&self) -> usize { SWIFT_SCROLLOFF }
  fn ui_theme(&self) -> &Theme { &self.ui_theme }
  fn ui_theme_name(&self) -> &str { &self.ui_theme_name }
  fn lsp_statusline_text(&self) -> Option<String> { self.lsp_statusline_text_value() }
  fn vcs_statusline_text(&self) -> Option<String> { self.vcs_statusline.clone() }
  fn watch_statusline_text(&self) -> Option<String> {
    self
      .active_file_watch
      .as_ref()
      .and_then(|watch| watch_statusline_text_for_state(watch.stream.reload_state))
  }
  fn watch_conflict_active(&self) -> bool {
    self
      .active_file_watch
      .as_ref()
      .is_some_and(|watch| watch.stream.reload_state == FileWatchReloadState::Conflict)
  }
  fn clear_watch_conflict(&mut self) {
    if let Some(watch) = self.active_file_watch.as_mut() {
      clear_reload_state(&mut watch.stream.reload_state);
    }
  }
  fn lsp_hover(&mut self) {
    let Some((uri, position)) = self.current_lsp_position() else {
      self.push_warning("lsp", "hover unavailable: no active LSP document");
      return;
    };
    self.clear_hover_state();
    self.dispatch_lsp_request(
      "textDocument/hover",
      hover_params(&uri, position),
      PendingLspRequestKind::Hover { uri },
    );
  }
  fn lsp_completion(&mut self) {
    self.cancel_auto_completion();
    let _ = self.dispatch_completion_request(LspCompletionContext::invoked(), true);
  }
  fn lsp_signature_help(&mut self) {
    let _ = self.dispatch_signature_help_request(LspSignatureHelpContext::invoked(), true);
  }

  fn available_theme_names(&self) -> Vec<String> { self.ui_theme_catalog.names() }
  fn set_ui_theme(&mut self, theme_name: &str) -> Result<(), String> { self.set_ui_theme_named(theme_name) }
  fn set_ui_theme_preview(&mut self, theme_name: &str) -> Result<(), String> { self.set_ui_theme_preview_named(theme_name) }
  fn clear_ui_theme_preview(&mut self) { self.clear_ui_theme_preview_state(); }
  fn set_file_path(&mut self, path: Option<PathBuf>) {
    self.file_path = path.clone();
    self.editor.set_active_file_path(path.clone());
    if let Some(path) = path {
      self.update_workspace_for_path(&path);
      self.reload_theme_catalog();
    }
    self.refresh_active_document_syntax();
    self.refresh_lsp_runtime_state();
    self.sync_active_file_watch_state();
    self.schedule_vcs_statusline_refresh();
  }
  fn open_file(&mut self, path: &Path) -> std::io::Result<()> {
    let contents = fs::read_to_string(path)?;
    let _ = self.editor.replace_active_buffer(Rope::from_str(&contents), Some(path.to_path_buf()));
    self.file_path = Some(path.to_path_buf());
    self.editor.set_active_file_path(Some(path.to_path_buf()));
    self.editor.document_mut().set_display_name(display_name_for_path(path));
    self.update_workspace_for_path(path);
    self.reload_theme_catalog();
    self.refresh_active_document_syntax();
    self.refresh_lsp_runtime_state();
    self.sync_active_file_watch_state();
    self.schedule_vcs_statusline_refresh();
    Ok(())
  }
  fn on_file_saved(&mut self, _path: &Path, text: &str) {
    if let Some(watch) = self.active_file_watch.as_mut() {
      watch.stream.suppress_until = Some(Instant::now() + active_file_self_save_suppress_window());
      clear_reload_state(&mut watch.stream.reload_state);
    }
    self.lsp_send_did_save(Some(text));
    self.schedule_vcs_statusline_refresh();
  }
}

impl OwnedSnapshot {
  fn from_editor(editor: &mut SwiftEditor, plan: Option<&RenderPlan>) -> Self {
    let viewport = editor.editor.view().viewport;
    let scroll = editor.editor.view().scroll;
    let mode = mode_code(editor.mode);
    let document_line_count = editor.editor.document().text().len_lines() as u32;

    let mut snapshot = Self {
      info: the_editor_snapshot_info_t {
        surface_width_px: editor.surface.width_px,
        surface_height_px: editor.surface.height_px,
        surface_metrics: editor.surface.metrics,
        background_color: theme_background_rgba(&editor.ui_theme),
        gutter_background_color: theme_gutter_background_rgba(&editor.ui_theme),
        viewport_width: viewport.width,
        viewport_height: viewport.height,
        content_offset_x: plan.map(|plan| plan.content_offset_x).unwrap_or(0),
        damage_start_row: plan.map(|plan| plan.damage_start_row).unwrap_or(0),
        damage_end_row: plan.map(|plan| plan.damage_end_row).unwrap_or(0),
        damage_is_full: plan.map(|plan| plan.damage_is_full).unwrap_or(false),
        damage_reason: plan.map(|plan| damage_reason_code(plan.damage_reason)).unwrap_or(damage_reason_code(RenderDamageReason::None)),
        mode,
        layout_generation: plan.map(|plan| plan.layout_generation).unwrap_or(0),
        text_generation: plan.map(|plan| plan.text_generation).unwrap_or(0),
        decoration_generation: plan.map(|plan| plan.decoration_generation).unwrap_or(0),
        cursor_generation: plan.map(|plan| plan.cursor_generation).unwrap_or(0),
        scroll_generation: plan.map(|plan| plan.scroll_generation).unwrap_or(0),
        theme_generation: plan.map(|plan| plan.theme_generation).unwrap_or(0),
        cursor_blink_generation: plan.map(|plan| plan.cursor_blink_generation).unwrap_or(0),
        scroll_row: scroll.row as u32,
        scroll_col: scroll.col as u32,
        document_line_count,
        line_count: 0,
        cursor_count: 0,
        selection_count: 0,
        overlay_count: 0,
      },
      ..Default::default()
    };

    let document_flags = editor.editor.document().flags();
    let document_name = editor.editor.document().display_name().to_string();
    let line_ending_name = line_ending_label(editor.editor.document().line_ending()).to_string();
    let absolute_path = editor.file_path().map(|path| path.display().to_string());
    let relative_path = editor
      .file_path()
      .map(|path| relative_document_path(path, &editor.workspace_root));
    let document_icon = editor
      .file_path()
      .map(file_picker_icon_name_for_path)
      .unwrap_or("doc")
      .to_string();
    let language_name = document_language_name(editor);
    let encoding_name = Some("UTF-8".to_string());
    let vcs_text = editor.vcs_statusline_text();

    snapshot.document = DocumentRecord {
      document: the_editor_snapshot_document_t {
        name: ptr::null(),
        icon: ptr::null(),
        relative_path: ptr::null(),
        absolute_path: ptr::null(),
        vcs_text: ptr::null(),
        language_name: ptr::null(),
        encoding_name: ptr::null(),
        line_ending_name: ptr::null(),
        is_modified: document_flags.modified,
        is_readonly: document_flags.readonly,
      },
      name_idx: Some(snapshot.push_string(&document_name)),
      icon_idx: Some(snapshot.push_string(&document_icon)),
      relative_path_idx: snapshot.push_optional_string(relative_path.as_deref()),
      absolute_path_idx: snapshot.push_optional_string(absolute_path.as_deref()),
      vcs_text_idx: snapshot.push_optional_string(vcs_text.as_deref()),
      language_name_idx: snapshot.push_optional_string(language_name.as_deref()),
      encoding_name_idx: snapshot.push_optional_string(encoding_name.as_deref()),
      line_ending_name_idx: snapshot.push_optional_string(Some(line_ending_name.as_str())),
    };

    let statusline = build_statusline_snapshot(editor);
    let leading_text = if editor.command_palette().is_open || editor.search_prompt_ref().active {
      Some(statusline.left)
    } else {
      None
    };
    let mut status_segments = statusline.right_segments;
    let cursor_text = status_segments.pop().map(|segment| segment.text).unwrap_or_default();
    snapshot.status = StatusRecord {
      status: the_editor_snapshot_status_t {
        leading_text: ptr::null(),
        item_count: 0,
        cursor_text: ptr::null(),
      },
      leading_text_idx: snapshot.push_optional_string(leading_text.as_deref()),
      cursor_text_idx: Some(snapshot.push_string(&cursor_text)),
    };
    for segment in status_segments {
      if matches!(segment.icon.as_deref(), Some("git_branch")) {
        continue;
      }
      let text_idx = snapshot.push_string(segment.text.as_str());
      let icon_idx = snapshot.push_optional_string(segment.icon.as_deref());
      snapshot.status_items.push(StatusItemRecord {
        item: the_editor_snapshot_status_item_t {
          icon: ptr::null(),
          text: ptr::null(),
          emphasis: statusline_emphasis_code(segment.emphasis),
        },
        icon_idx,
        text_idx,
      });
    }
    snapshot.status.status.item_count = snapshot.status_items.len();

    let palette = editor.command_palette();
    let palette_query = palette
      .prompt_text
      .as_deref()
      .unwrap_or(palette.query.as_str())
      .trim_start_matches(':')
      .to_string();
    let palette_placeholder = command_palette_placeholder_text(editor);
    let palette_query_idx = snapshot.push_string(&palette_query);
    let palette_placeholder_idx = snapshot.push_string(&palette_placeholder);
    snapshot.command_palette = CommandPaletteRecord {
      palette: the_editor_snapshot_command_palette_t {
        is_open: palette.is_open,
        selected_index: command_palette_selected_filtered_index(palette)
          .map(|index| index as i32)
          .unwrap_or(-1),
        item_count: 0,
        query: ptr::null(),
        placeholder: ptr::null(),
      },
      query_idx: Some(palette_query_idx),
      placeholder_idx: Some(palette_placeholder_idx),
    };
    command_palette_debug_log(format!(
      "snapshot_export prompt_input={:?} palette.query={:?} prompt_text={:?} exported_query={:?} prefiltered={} selected_filtered={:?} items_len={}",
      editor.command_prompt.input,
      palette.query,
      palette.prompt_text,
      palette_query,
      palette.prefiltered,
      command_palette_selected_filtered_index(palette),
      palette.items.len(),
    ));

    for item_index in command_palette_filtered_indices(palette) {
      let Some(item) = palette.items.get(item_index) else {
        continue;
      };
      let title_idx = snapshot.push_string(item.title.as_str());
      let subtitle_idx = snapshot.push_optional_string(item.subtitle.as_deref());
      let description_idx = snapshot.push_optional_string(item.description.as_deref());
      let badge_idx = snapshot.push_optional_string(item.badge.as_deref());
      let leading_icon_idx = snapshot.push_optional_string(item.leading_icon.as_deref());
      snapshot.command_palette_items.push(CommandPaletteItemRecord {
        item: the_editor_snapshot_command_palette_item_t {
          title: ptr::null(),
          subtitle: ptr::null(),
          description: ptr::null(),
          badge: ptr::null(),
          leading_icon: ptr::null(),
          leading_color: color_to_rgba(item.leading_color, &editor.ui_theme),
          emphasis: item.emphasis,
        },
        title_idx,
        subtitle_idx,
        description_idx,
        badge_idx,
        leading_icon_idx,
      });
    }
    snapshot.command_palette.palette.item_count = snapshot.command_palette_items.len();

    let completion_menu = editor.completion_menu();
    snapshot.completion_menu = CompletionMenuRecord {
      menu: the_editor_snapshot_completion_menu_t {
        is_open: completion_menu.active,
        col: 0,
        row: 0,
        width: 0,
        height: 0,
        selected_index: completion_menu.selected.map(|index| index as i32).unwrap_or(-1),
        item_count: completion_menu.items.len(),
        scroll_offset: completion_menu.scroll,
      },
    };
    for item in &completion_menu.items {
      let title_idx = snapshot.push_string(item.label.as_str());
      let subtitle_idx = snapshot.push_optional_string(item.detail.as_deref());
      let leading_icon_idx = snapshot.push_optional_string(item.kind_icon.as_deref());
      snapshot.completion_menu_items.push(CompletionMenuItemRecord {
        item: the_editor_snapshot_completion_menu_item_t {
          title: ptr::null(),
          subtitle: ptr::null(),
          leading_icon: ptr::null(),
          leading_color: color_to_rgba(item.kind_color, &editor.ui_theme),
        },
        title_idx,
        subtitle_idx,
        leading_icon_idx,
      });
    }

    let input_prompt = editor.search_prompt_ref();
    let input_prompt_title = input_prompt_title(input_prompt.kind).to_string();
    let input_prompt_placeholder = input_prompt_placeholder(input_prompt.kind).to_string();
    snapshot.input_prompt = InputPromptRecord {
      prompt: the_editor_snapshot_input_prompt_t {
        is_open: input_prompt.active,
        kind: input_prompt_kind_code(input_prompt.kind),
        title: ptr::null(),
        placeholder: ptr::null(),
        query: ptr::null(),
        error: ptr::null(),
      },
      title_idx: Some(snapshot.push_string(&input_prompt_title)),
      placeholder_idx: Some(snapshot.push_string(&input_prompt_placeholder)),
      query_idx: Some(snapshot.push_string(&input_prompt.query)),
      error_idx: snapshot.push_optional_string(input_prompt.error.as_deref()),
    };

    if let Some(plan) = plan {
      if editor.completion_menu.active && !editor.completion_menu.items.is_empty() {
        let cursor = active_docs_cursor_position(plan);
        let visible = editor.completion_menu.items.len().min(completion_menu_visible_rows());
        let panel_width = plan.viewport.width.saturating_mul(2).saturating_div(3).min(64).max(18);
        let panel_height = (visible as u16).max(1);
        let panel = completion_menu_panel_record(plan.viewport.width, plan.viewport.height, cursor, panel_width, panel_height);
        snapshot.completion_menu.menu.col = panel.col;
        snapshot.completion_menu.menu.row = panel.row;
        snapshot.completion_menu.menu.width = panel.width;
        snapshot.completion_menu.menu.height = panel.height;

        if let Some(markdown) = selected_completion_docs_text(editor) {
          let docs_width = plan.viewport.width.saturating_mul(2).saturating_div(3).min(84).max(28);
          let docs_height = completion_docs_target_height(plan.viewport.height, panel.height);
          if let Some(docs_panel) = completion_docs_panel_record(
            plan.viewport.width,
            plan.viewport.height,
            docs_width,
            docs_height,
            panel,
          ) {
            snapshot.completion_docs.panel = docs_panel;
            for run in flatten_docs_runs(markdown, editor) {
              let text_idx = snapshot.push_string(&run.text);
              snapshot.completion_docs_runs.push(DocsRunRecord {
                run: the_editor_snapshot_docs_run_t {
                  text: ptr::null(),
                  style: style_to_ffi(run.style, &editor.ui_theme),
                  kind: docs_run_kind_code(run.kind),
                },
                text_idx,
              });
            }
            snapshot.completion_docs.panel.run_count = snapshot.completion_docs_runs.len();
          }
        }
      }

      if let Some(markdown) = editor
        .hover_docs
        .as_deref()
        .map(str::trim)
        .filter(|docs| !docs.is_empty())
      {
        let cursor = active_docs_cursor_position(plan);
        let (width, height) = docs_panel_dimensions(markdown, editor, 24, 56, 5, 12);
        snapshot.hover_docs.panel = docs_panel_record(
          plan.viewport.width,
          plan.viewport.height,
          cursor,
          width,
          height,
          false,
        );
        for run in flatten_docs_runs(markdown, editor) {
          let text_idx = snapshot.push_string(&run.text);
          snapshot.hover_docs_runs.push(DocsRunRecord {
            run: the_editor_snapshot_docs_run_t {
              text: ptr::null(),
              style: style_to_ffi(run.style, &editor.ui_theme),
              kind: docs_run_kind_code(run.kind),
            },
            text_idx,
          });
        }
        snapshot.hover_docs.panel.run_count = snapshot.hover_docs_runs.len();
      }

      if let Some(markdown) = signature_help_markdown(&editor.signature_help) {
        let cursor = active_docs_cursor_position(plan);
        let (width, height) = docs_panel_dimensions(markdown.as_str(), editor, 18, 44, 3, 8);
        snapshot.signature_help.panel = docs_panel_record(
          plan.viewport.width,
          plan.viewport.height,
          cursor,
          width,
          height,
          true,
        );
        for run in flatten_docs_runs(markdown.as_str(), editor) {
          let text_idx = snapshot.push_string(&run.text);
          snapshot.signature_help_runs.push(DocsRunRecord {
            run: the_editor_snapshot_docs_run_t {
              text: ptr::null(),
              style: style_to_ffi(run.style, &editor.ui_theme),
              kind: docs_run_kind_code(run.kind),
            },
            text_idx,
          });
        }
        snapshot.signature_help.panel.run_count = snapshot.signature_help_runs.len();
      }
    }

    snapshot.populate_file_picker(editor);

    let Some(plan) = plan else {
      snapshot.command_palette.palette.query = snapshot.strings[palette_query_idx].as_ptr();
      snapshot.command_palette.palette.placeholder = snapshot.strings[palette_placeholder_idx].as_ptr();
      for item in &mut snapshot.command_palette_items {
        item.item.title = snapshot.strings[item.title_idx].as_ptr();
        if let Some(idx) = item.subtitle_idx {
          item.item.subtitle = snapshot.strings[idx].as_ptr();
        }
        if let Some(idx) = item.description_idx {
          item.item.description = snapshot.strings[idx].as_ptr();
        }
        if let Some(idx) = item.badge_idx {
          item.item.badge = snapshot.strings[idx].as_ptr();
        }
        if let Some(idx) = item.leading_icon_idx {
          item.item.leading_icon = snapshot.strings[idx].as_ptr();
        }
      }
      snapshot.finalize_document_and_status_strings();
      snapshot.finalize_completion_menu_strings();
      snapshot.finalize_input_prompt_strings();
      snapshot.finalize_docs_panel_strings();
      snapshot.finalize_file_picker_strings();
      return snapshot;
    };

    let base_text_style = editor.ui_theme.try_get("ui.text").unwrap_or_default();

    for row in 0..plan.viewport.height {
      let doc_line = plan
        .visible_rows
        .iter()
        .find(|visible| visible.row == row)
        .map(|visible| visible.doc_line as i32)
        .unwrap_or(-1);
      let first_visual_line = plan
        .visible_rows
        .iter()
        .find(|visible| visible.row == row)
        .map(|visible| visible.first_visual_line)
        .unwrap_or(false);

      let span_start = snapshot.spans.len();
      let text_cell_start = snapshot.text_cells.len();

      if let Some(gutter) = plan.gutter_lines.iter().find(|line| line.row == row) {
        for span in &gutter.spans {
          let style = style_to_ffi(span.style, &editor.ui_theme);
          let text_idx = snapshot.push_string(span.text.as_str());
          snapshot.spans.push(SpanRecord {
            span: the_editor_snapshot_span_t {
              col: span.col,
              cols: span.text.chars().count() as u16,
              text: ptr::null(),
              is_virtual: false,
              style,
            },
            text_idx,
          });
          snapshot.push_text_cells(row, span.col, span.text.as_str(), false, style);
        }
      }

      if let Some(line) = plan.lines.iter().find(|line| line.row == row) {
        for span in &line.spans {
          let text_idx = snapshot.push_string(span.text.as_str());
          let highlight_style = span.highlight.map(|highlight| editor.ui_theme.highlight(highlight)).unwrap_or_default();
          let style = style_to_ffi(base_text_style.patch(highlight_style), &editor.ui_theme);
          let col = plan.content_offset_x.saturating_add(span.col);
          snapshot.spans.push(SpanRecord {
            span: the_editor_snapshot_span_t {
              col,
              cols: span.cols,
              text: ptr::null(),
              is_virtual: span.is_virtual,
              style,
            },
            text_idx,
          });
          snapshot.push_text_cells(row, col, span.text.as_str(), span.is_virtual, style);
        }
      }

      let span_count = snapshot.spans.len().saturating_sub(span_start);
      let text_cell_count = snapshot.text_cells.len().saturating_sub(text_cell_start);
      snapshot.lines.push(LineRecord {
        line: the_editor_snapshot_line_t {
          row,
          doc_line,
          first_visual_line,
          span_count,
          text_cell_count,
        },
        span_start,
        text_cell_start,
      });
    }

    snapshot.cursors = plan
      .cursors
      .iter()
      .map(|cursor| the_editor_snapshot_cursor_t {
        row: cursor.pos.row as u32,
        col: (plan.content_offset_x as usize + cursor.pos.col) as u32,
        kind: cursor_kind_code(cursor.kind),
        style: style_to_ffi(cursor.style, &editor.ui_theme),
      })
      .collect();

    snapshot.selections = plan
      .selections
      .iter()
      .map(|selection| the_editor_snapshot_selection_t {
        x: plan.content_offset_x.saturating_add(selection.rect.x),
        y: selection.rect.y,
        width: selection.rect.width,
        height: selection.rect.height,
        kind: selection_kind_code(selection.kind),
        style: style_to_ffi(selection.style, &editor.ui_theme),
      })
      .collect();

    for overlay in &plan.overlays {
      match overlay {
        OverlayNode::Rect(rect) => {
          snapshot.overlays.push(OverlayRecord {
            overlay: the_editor_snapshot_overlay_t {
              kind: 0,
              rect_kind: overlay_rect_kind_code(rect.kind),
              x: rect.rect.x,
              y: rect.rect.y,
              width: rect.rect.width,
              height: rect.rect.height,
              radius: rect.radius,
              row: 0,
              col: 0,
              text: ptr::null(),
              style: style_to_ffi(rect.style, &editor.ui_theme),
            },
            text_idx: None,
          });
        },
        OverlayNode::Text(text) => {
          let text_idx = snapshot.push_string(text.text.as_str());
          snapshot.overlays.push(OverlayRecord {
            overlay: the_editor_snapshot_overlay_t {
              kind: 1,
              rect_kind: 0,
              x: 0,
              y: 0,
              width: 0,
              height: 0,
              radius: 0,
              row: text.pos.row as u32,
              col: text.pos.col as u32,
              text: ptr::null(),
              style: style_to_ffi(text.style, &editor.ui_theme),
            },
            text_idx: Some(text_idx),
          });
        },
      }
    }

    if let Some(query_idx) = snapshot.command_palette.query_idx {
      snapshot.command_palette.palette.query = snapshot.strings[query_idx].as_ptr();
    }
    if let Some(placeholder_idx) = snapshot.command_palette.placeholder_idx {
      snapshot.command_palette.palette.placeholder = snapshot.strings[placeholder_idx].as_ptr();
    }
    for item in &mut snapshot.command_palette_items {
      item.item.title = snapshot.strings[item.title_idx].as_ptr();
      if let Some(idx) = item.subtitle_idx {
        item.item.subtitle = snapshot.strings[idx].as_ptr();
      }
      if let Some(idx) = item.description_idx {
        item.item.description = snapshot.strings[idx].as_ptr();
      }
      if let Some(idx) = item.badge_idx {
        item.item.badge = snapshot.strings[idx].as_ptr();
      }
      if let Some(idx) = item.leading_icon_idx {
        item.item.leading_icon = snapshot.strings[idx].as_ptr();
      }
    }
    for span in &mut snapshot.spans {
      span.span.text = snapshot.strings[span.text_idx].as_ptr();
    }
    for text_cell in &mut snapshot.text_cells {
      text_cell.cell.text = snapshot.strings[text_cell.text_idx].as_ptr();
    }
    for overlay in &mut snapshot.overlays {
      if let Some(text_idx) = overlay.text_idx {
        overlay.overlay.text = snapshot.strings[text_idx].as_ptr();
      }
    }
    snapshot.finalize_document_and_status_strings();
    snapshot.finalize_completion_menu_strings();
    snapshot.finalize_input_prompt_strings();
    snapshot.finalize_docs_panel_strings();
    snapshot.finalize_file_picker_strings();

    snapshot.info.line_count = snapshot.lines.len();
    snapshot.info.cursor_count = snapshot.cursors.len();
    snapshot.info.selection_count = snapshot.selections.len();
    snapshot.info.overlay_count = snapshot.overlays.len();
    snapshot
  }

  fn push_string(&mut self, text: &str) -> usize {
    let c_string = CString::new(text).unwrap_or_else(|_| CString::new(text.replace('\0', "")).expect("cstring"));
    self.strings.push(c_string);
    self.strings.len() - 1
  }

  fn push_optional_string(&mut self, text: Option<&str>) -> Option<usize> {
    text.map(|text| self.push_string(text))
  }

  fn push_text_cells(&mut self, row: u16, col: u16, text: &str, is_virtual: bool, style: the_editor_style_t) {
    let mut next_col = col;
    for grapheme in text.graphemes(true) {
      if grapheme.is_empty() {
        continue;
      }
      let cols = grapheme_width(grapheme).min(u16::MAX as usize) as u16;
      let text_idx = self.push_string(grapheme);
      self.text_cells.push(TextCellRecord {
        cell: the_editor_snapshot_text_cell_t {
          row,
          col: next_col,
          cols,
          text: ptr::null(),
          is_virtual,
          style,
        },
        text_idx,
      });
      next_col = next_col.saturating_add(cols);
    }
  }

  fn populate_file_picker(&mut self, editor: &SwiftEditor) {
    let picker = &editor.file_picker;
    let matched_count = if picker.active { picker.matched_count() } else { 0 };
    let visible_item_start = picker.list_offset.min(matched_count);
    let visible_item_count = matched_count
      .saturating_sub(visible_item_start)
      .min(picker.list_visible.max(1));
    let preview_window = if picker.active && picker.show_preview {
      file_picker_preview_window(picker, picker.preview_scroll, editor.file_picker_preview_visible_rows, 2)
    } else {
      file_picker_preview_window(picker, 0, 1, 0)
    };

    self.file_picker = FilePickerRecord {
      picker: the_editor_snapshot_file_picker_t {
        is_open: picker.active,
        kind: file_picker_kind_code(picker.kind),
        selected_index: picker.selected.map(|index| index as i32).unwrap_or(-1),
        matched_count,
        visible_item_start,
        visible_item_count,
        title: ptr::null(),
        query: ptr::null(),
        show_preview: picker.show_preview,
        loading: picker.scanning || picker.matcher_running || picker.dynamic_running || picker.preview_loading(),
        error: ptr::null(),
        preview_path: ptr::null(),
        preview_navigation_mode: file_picker_preview_navigation_code(preview_window.navigation_mode),
        preview_kind: file_picker_preview_kind_code(preview_window.kind),
        preview_total_rows: preview_window.total_virtual_rows,
        preview_offset: picker.preview_scroll,
        preview_window_start: preview_window.window_start,
        preview_window_count: if let Some(vcs_diff) = &preview_window.vcs_diff {
          vcs_diff.lines.len()
        } else {
          preview_window.lines.len()
        },
      },
      title_idx: Some(self.push_string(&picker.title)),
      query_idx: Some(self.push_string(&picker.query)),
      error_idx: self.push_optional_string(picker.error.as_deref()),
      preview_path_idx: picker
        .preview_path
        .as_ref()
        .map(|path| self.push_string(path.to_string_lossy().as_ref())),
    };

    for global_index in visible_item_start..visible_item_start.saturating_add(visible_item_count) {
      let Some(item) = picker.matched_item(global_index) else {
        continue;
      };
      let row_data = file_picker_row_data_for_kind(picker.kind, &item);
      let icon_idx = self.push_string(&item.icon);
      let primary_idx = self.push_string(&row_data.primary);
      let secondary_idx = self.push_optional_string((!row_data.secondary.is_empty()).then_some(row_data.secondary.as_str()));
      let tertiary_idx = self.push_optional_string((!row_data.tertiary.is_empty()).then_some(row_data.tertiary.as_str()));
      let quaternary_idx = self.push_optional_string((!row_data.quaternary.is_empty()).then_some(row_data.quaternary.as_str()));
      self.file_picker_items.push(FilePickerItemRecord {
        item: the_editor_snapshot_file_picker_item_t {
          stable_id: item.stable_id(),
          global_index,
          row_kind: file_picker_row_kind_code(row_data.kind),
          selectable: file_picker_item_selectable(&item),
          is_dir: item.is_dir,
          icon: ptr::null(),
          primary: ptr::null(),
          secondary: ptr::null(),
          tertiary: ptr::null(),
          quaternary: ptr::null(),
          line: row_data.line as u32,
          column: row_data.column as u32,
          depth: row_data.depth.min(u16::MAX as usize) as u16,
        },
        icon_idx,
        primary_idx,
        secondary_idx,
        tertiary_idx,
        quaternary_idx,
      });
    }

    let base_text_style = editor.ui_theme.try_get("ui.text").unwrap_or_default();
    if let Some(vcs_diff) = preview_window.vcs_diff {
      for line in vcs_diff.lines {
        let segment_start = self.file_picker_preview_segments.len();
        for segment in line.segments {
          self.push_file_picker_preview_segment(editor, base_text_style, segment);
        }
        let marker_idx = (!line.message.is_empty()).then(|| self.push_string(&line.message));
        self.file_picker_preview_lines.push(FilePickerPreviewLineRecord {
          line: the_editor_snapshot_file_picker_preview_line_t {
            virtual_row: line.virtual_row,
            kind: file_picker_preview_line_code_for_vcs(line.kind),
            source: file_picker_preview_source_code(line.source),
            line_number: line.line_number.map(|value| value as i32).unwrap_or(-1),
            focused: false,
            marker: ptr::null(),
            segment_count: self.file_picker_preview_segments.len().saturating_sub(segment_start),
          },
          marker_idx,
          segment_start,
        });
      }
      return;
    }

    for line in preview_window.lines {
      let segment_start = self.file_picker_preview_segments.len();
      for segment in line.segments {
        self.push_file_picker_preview_segment(editor, base_text_style, segment);
      }
      let marker_idx = (!line.marker.is_empty()).then(|| self.push_string(&line.marker));
      self.file_picker_preview_lines.push(FilePickerPreviewLineRecord {
        line: the_editor_snapshot_file_picker_preview_line_t {
          virtual_row: line.virtual_row,
          kind: file_picker_preview_line_code(line.kind),
          source: 0,
          line_number: line.line_number.map(|value| value as i32).unwrap_or(-1),
          focused: line.focused,
          marker: ptr::null(),
          segment_count: self.file_picker_preview_segments.len().saturating_sub(segment_start),
        },
        marker_idx,
        segment_start,
      });
    }
  }

  fn push_file_picker_preview_segment(
    &mut self,
    editor: &SwiftEditor,
    base_text_style: Style,
    segment: the_default::FilePickerPreviewSegment,
  ) {
    let highlight_style = segment
      .highlight_id
      .map(Highlight::new)
      .map(|highlight| editor.ui_theme.highlight(highlight))
      .unwrap_or_default();
    let style = style_to_ffi(base_text_style.patch(highlight_style), &editor.ui_theme);
    let text_idx = self.push_string(&segment.text);
    self.file_picker_preview_segments.push(FilePickerPreviewSegmentRecord {
      segment: the_editor_snapshot_file_picker_preview_segment_t {
        text: ptr::null(),
        style,
        is_match: segment.is_match,
        change_kind: file_picker_preview_change_kind_code(segment.change_kind),
      },
      text_idx,
    });
  }

  fn finalize_document_and_status_strings(&mut self) {
    if let Some(name_idx) = self.document.name_idx {
      self.document.document.name = self.strings[name_idx].as_ptr();
    }
    if let Some(icon_idx) = self.document.icon_idx {
      self.document.document.icon = self.strings[icon_idx].as_ptr();
    }
    if let Some(relative_path_idx) = self.document.relative_path_idx {
      self.document.document.relative_path = self.strings[relative_path_idx].as_ptr();
    }
    if let Some(absolute_path_idx) = self.document.absolute_path_idx {
      self.document.document.absolute_path = self.strings[absolute_path_idx].as_ptr();
    }
    if let Some(vcs_text_idx) = self.document.vcs_text_idx {
      self.document.document.vcs_text = self.strings[vcs_text_idx].as_ptr();
    }
    if let Some(language_name_idx) = self.document.language_name_idx {
      self.document.document.language_name = self.strings[language_name_idx].as_ptr();
    }
    if let Some(encoding_name_idx) = self.document.encoding_name_idx {
      self.document.document.encoding_name = self.strings[encoding_name_idx].as_ptr();
    }
    if let Some(line_ending_name_idx) = self.document.line_ending_name_idx {
      self.document.document.line_ending_name = self.strings[line_ending_name_idx].as_ptr();
    }

    if let Some(leading_text_idx) = self.status.leading_text_idx {
      self.status.status.leading_text = self.strings[leading_text_idx].as_ptr();
    }
    if let Some(cursor_text_idx) = self.status.cursor_text_idx {
      self.status.status.cursor_text = self.strings[cursor_text_idx].as_ptr();
    }
    for item in &mut self.status_items {
      if let Some(icon_idx) = item.icon_idx {
        item.item.icon = self.strings[icon_idx].as_ptr();
      }
      item.item.text = self.strings[item.text_idx].as_ptr();
    }
  }

  fn finalize_completion_menu_strings(&mut self) {
    for item in &mut self.completion_menu_items {
      item.item.title = self.strings[item.title_idx].as_ptr();
      if let Some(idx) = item.subtitle_idx {
        item.item.subtitle = self.strings[idx].as_ptr();
      }
      if let Some(idx) = item.leading_icon_idx {
        item.item.leading_icon = self.strings[idx].as_ptr();
      }
    }
  }

  fn finalize_input_prompt_strings(&mut self) {
    if let Some(title_idx) = self.input_prompt.title_idx {
      self.input_prompt.prompt.title = self.strings[title_idx].as_ptr();
    }
    if let Some(placeholder_idx) = self.input_prompt.placeholder_idx {
      self.input_prompt.prompt.placeholder = self.strings[placeholder_idx].as_ptr();
    }
    if let Some(query_idx) = self.input_prompt.query_idx {
      self.input_prompt.prompt.query = self.strings[query_idx].as_ptr();
    }
    if let Some(error_idx) = self.input_prompt.error_idx {
      self.input_prompt.prompt.error = self.strings[error_idx].as_ptr();
    }
  }

  fn finalize_docs_panel_strings(&mut self) {
    for run in &mut self.hover_docs_runs {
      run.run.text = self.strings[run.text_idx].as_ptr();
    }
    for run in &mut self.completion_docs_runs {
      run.run.text = self.strings[run.text_idx].as_ptr();
    }
    for run in &mut self.signature_help_runs {
      run.run.text = self.strings[run.text_idx].as_ptr();
    }
  }

  fn finalize_file_picker_strings(&mut self) {
    if let Some(title_idx) = self.file_picker.title_idx {
      self.file_picker.picker.title = self.strings[title_idx].as_ptr();
    }
    if let Some(query_idx) = self.file_picker.query_idx {
      self.file_picker.picker.query = self.strings[query_idx].as_ptr();
    }
    if let Some(error_idx) = self.file_picker.error_idx {
      self.file_picker.picker.error = self.strings[error_idx].as_ptr();
    }
    if let Some(preview_path_idx) = self.file_picker.preview_path_idx {
      self.file_picker.picker.preview_path = self.strings[preview_path_idx].as_ptr();
    }
    for item in &mut self.file_picker_items {
      item.item.icon = self.strings[item.icon_idx].as_ptr();
      item.item.primary = self.strings[item.primary_idx].as_ptr();
      if let Some(idx) = item.secondary_idx {
        item.item.secondary = self.strings[idx].as_ptr();
      }
      if let Some(idx) = item.tertiary_idx {
        item.item.tertiary = self.strings[idx].as_ptr();
      }
      if let Some(idx) = item.quaternary_idx {
        item.item.quaternary = self.strings[idx].as_ptr();
      }
    }
    for line in &mut self.file_picker_preview_lines {
      if let Some(marker_idx) = line.marker_idx {
        line.line.marker = self.strings[marker_idx].as_ptr();
      }
    }
    for segment in &mut self.file_picker_preview_segments {
      segment.segment.text = self.strings[segment.text_idx].as_ptr();
    }
  }
}

fn translate_key_event(raw: the_editor_key_event_t) -> Option<KeyEvent> {
  let modifiers = translate_modifiers(raw.modifiers);
  let key = match raw.kind {
    THE_EDITOR_KEY_CHAR => Key::Char(char::from_u32(raw.codepoint)?),
    THE_EDITOR_KEY_ENTER => Key::Enter,
    THE_EDITOR_KEY_NUMPAD_ENTER => Key::NumpadEnter,
    THE_EDITOR_KEY_ESCAPE => Key::Escape,
    THE_EDITOR_KEY_BACKSPACE => Key::Backspace,
    THE_EDITOR_KEY_TAB => Key::Tab,
    THE_EDITOR_KEY_DELETE => Key::Delete,
    THE_EDITOR_KEY_INSERT => Key::Insert,
    THE_EDITOR_KEY_HOME => Key::Home,
    THE_EDITOR_KEY_END => Key::End,
    THE_EDITOR_KEY_PAGE_UP => Key::PageUp,
    THE_EDITOR_KEY_PAGE_DOWN => Key::PageDown,
    THE_EDITOR_KEY_LEFT => Key::Left,
    THE_EDITOR_KEY_RIGHT => Key::Right,
    THE_EDITOR_KEY_UP => Key::Up,
    THE_EDITOR_KEY_DOWN => Key::Down,
    THE_EDITOR_KEY_F1 => Key::F1,
    THE_EDITOR_KEY_F2 => Key::F2,
    THE_EDITOR_KEY_F3 => Key::F3,
    THE_EDITOR_KEY_F4 => Key::F4,
    THE_EDITOR_KEY_F5 => Key::F5,
    THE_EDITOR_KEY_F6 => Key::F6,
    THE_EDITOR_KEY_F7 => Key::F7,
    THE_EDITOR_KEY_F8 => Key::F8,
    THE_EDITOR_KEY_F9 => Key::F9,
    THE_EDITOR_KEY_F10 => Key::F10,
    THE_EDITOR_KEY_F11 => Key::F11,
    THE_EDITOR_KEY_F12 => Key::F12,
    THE_EDITOR_KEY_OTHER => Key::Other,
    _ => return None,
  };
  Some(KeyEvent { key, modifiers })
}

fn translate_modifiers(raw: u8) -> the_default::Modifiers {
  let mut modifiers = the_default::Modifiers::empty();
  if (raw & MOD_CTRL) != 0 { modifiers.insert(the_default::Modifiers::CTRL); }
  if (raw & MOD_ALT) != 0 { modifiers.insert(the_default::Modifiers::ALT); }
  if (raw & MOD_SHIFT) != 0 { modifiers.insert(the_default::Modifiers::SHIFT); }
  modifiers
}

fn mode_code(mode: Mode) -> u8 {
  match mode {
    Mode::Normal => 0,
    Mode::Insert => 1,
    Mode::Select => 2,
    Mode::Command => 3,
  }
}

fn input_prompt_kind_code(kind: SearchPromptKind) -> u8 {
  match kind {
    SearchPromptKind::Search => 0,
    SearchPromptKind::SelectRegex => 1,
    SearchPromptKind::SplitSelection => 2,
    SearchPromptKind::KeepSelections => 3,
    SearchPromptKind::RemoveSelections => 4,
    SearchPromptKind::RenameSymbol => 5,
    SearchPromptKind::ShellPipe => 6,
    SearchPromptKind::ShellPipeTo => 7,
    SearchPromptKind::ShellInsertOutput => 8,
    SearchPromptKind::ShellAppendOutput => 9,
    SearchPromptKind::ShellKeepPipe => 10,
  }
}

fn input_prompt_title(kind: SearchPromptKind) -> &'static str {
  match kind {
    SearchPromptKind::Search => "Search",
    SearchPromptKind::SelectRegex => "Select",
    SearchPromptKind::SplitSelection => "Split",
    SearchPromptKind::KeepSelections => "Keep",
    SearchPromptKind::RemoveSelections => "Remove",
    SearchPromptKind::RenameSymbol => "Rename",
    SearchPromptKind::ShellPipe => "Pipe",
    SearchPromptKind::ShellPipeTo => "Pipe To",
    SearchPromptKind::ShellInsertOutput => "Insert Output",
    SearchPromptKind::ShellAppendOutput => "Append Output",
    SearchPromptKind::ShellKeepPipe => "Keep Pipe",
  }
}

fn input_prompt_placeholder(kind: SearchPromptKind) -> &'static str {
  match kind {
    SearchPromptKind::Search => "Search",
    SearchPromptKind::SelectRegex => "Regular expression",
    SearchPromptKind::SplitSelection => "Regular expression",
    SearchPromptKind::KeepSelections => "Regular expression",
    SearchPromptKind::RemoveSelections => "Regular expression",
    SearchPromptKind::RenameSymbol => "Rename symbol",
    SearchPromptKind::ShellPipe => "Shell command",
    SearchPromptKind::ShellPipeTo => "Shell command",
    SearchPromptKind::ShellInsertOutput => "Shell command",
    SearchPromptKind::ShellAppendOutput => "Shell command",
    SearchPromptKind::ShellKeepPipe => "Shell command",
  }
}

fn lsp_server_from_env() -> Option<LspServerConfig> {
  let command = env::var("THE_EDITOR_LSP_COMMAND").ok()?.trim().to_string();
  if command.is_empty() {
    return None;
  }

  let mut server = LspServerConfig::new(command.clone(), command);
  if let Ok(args) = env::var("THE_EDITOR_LSP_ARGS") {
    let args: Vec<String> = args.split_whitespace().map(|value| value.to_string()).collect();
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

fn lsp_runtime_config_for(server: LspServerConfig, workspace_root: PathBuf) -> LspRuntimeConfig {
  LspRuntimeConfig::new(workspace_root)
    .with_server(server)
    .with_restart_policy(true, Duration::from_millis(250))
    .with_restart_limits(6, Duration::from_secs(30))
    .with_request_policy(Duration::from_secs(8), 1)
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

fn close_lsp_document_for_runtime(runtime: &mut ManagedLspRuntime, document: Option<&LspDocumentSyncState>) {
  if runtime.opened_current_document && let Some(document) = document {
    let params = did_close_params(&document.uri);
    let _ = runtime.runtime.send_notification("textDocument/didClose", Some(params));
  }
  let pending_ids = runtime.pending_requests.keys().copied().collect::<Vec<_>>();
  for request_id in pending_ids {
    let _ = runtime.runtime.cancel_request(request_id);
  }
  runtime.pending_requests.clear();
  runtime.opened_current_document = false;
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

#[derive(Clone)]
struct DocsStyledRun {
  text:  String,
  style: Style,
  kind:  DocsSemanticKind,
}

#[derive(Clone, Copy)]
struct DocsPanelStyles {
  base:             Style,
  heading:          [Style; 6],
  bullet:           Style,
  quote:            Style,
  code:             Style,
  active_parameter: Style,
  link:             Style,
  rule:             Style,
}

impl DocsPanelStyles {
  fn default(base: Style) -> Self {
    let heading = [
      base.add_modifier(Modifier::BOLD),
      base.add_modifier(Modifier::BOLD),
      base.add_modifier(Modifier::BOLD),
      base.add_modifier(Modifier::BOLD),
      base.add_modifier(Modifier::BOLD),
      base.add_modifier(Modifier::BOLD),
    ];
    Self {
      base,
      heading,
      bullet: base.add_modifier(Modifier::BOLD),
      quote: base.add_modifier(Modifier::DIM),
      code: base.add_modifier(Modifier::DIM),
      active_parameter: base.add_modifier(Modifier::BOLD),
      link: base.underline_style(UnderlineStyle::Line),
      rule: base.add_modifier(Modifier::DIM),
    }
  }
}

fn docs_theme_style_or(theme: &Theme, scope: &str, fallback: Style) -> Style {
  theme
    .try_get(scope)
    .map(|style| fallback.patch(style))
    .unwrap_or(fallback)
}

fn docs_panel_styles(theme: &Theme, base: Style) -> DocsPanelStyles {
  let mut styles = DocsPanelStyles::default(base);
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

fn push_docs_run(runs: &mut Vec<DocsStyledRun>, text: String, style: Style, kind: DocsSemanticKind) {
  if text.is_empty() {
    return;
  }
  if let Some(last) = runs.last_mut()
    && last.style == style
    && last.kind == kind
  {
    last.text.push_str(&text);
    return;
  }
  runs.push(DocsStyledRun { text, style, kind });
}

fn docs_runs_from_inline(
  inline_runs: &[DocsInlineRun],
  styles: &DocsPanelStyles,
  base_style: Style,
  default_kind: DocsSemanticKind,
) -> Vec<DocsStyledRun> {
  let mut runs = Vec::new();
  for inline in inline_runs {
    let (kind, mut style) = match inline.kind {
      DocsInlineKind::Text => (default_kind, base_style),
      DocsInlineKind::Link => (DocsSemanticKind::Link, base_style.patch(styles.link)),
      DocsInlineKind::InlineCode => (DocsSemanticKind::InlineCode, base_style.patch(styles.code)),
    };
    if inline.strong {
      style = style.add_modifier(Modifier::BOLD);
    }
    if inline.emphasis {
      style = style.add_modifier(Modifier::ITALIC);
    }
    push_docs_run(&mut runs, inline.text.clone(), style, kind);
  }
  runs
}

fn strip_signature_active_markers_from_line(line: &str) -> (String, Option<std::ops::Range<usize>>) {
  let mut cleaned = String::with_capacity(line.len());
  let mut idx = 0usize;
  let mut start = None;
  let mut end = None;

  while idx < line.len() {
    if line[idx..].starts_with(the_default::SIGNATURE_HELP_ACTIVE_PARAM_START_MARKER) {
      if start.is_none() {
        start = Some(cleaned.len());
      }
      idx += the_default::SIGNATURE_HELP_ACTIVE_PARAM_START_MARKER.len();
      continue;
    }
    if line[idx..].starts_with(the_default::SIGNATURE_HELP_ACTIVE_PARAM_END_MARKER) {
      if start.is_some() && end.is_none() {
        end = Some(cleaned.len());
      }
      idx += the_default::SIGNATURE_HELP_ACTIVE_PARAM_END_MARKER.len();
      continue;
    }
    let mut chars = line[idx..].chars();
    let Some(ch) = chars.next() else { break; };
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

fn strip_signature_active_markers_from_lines(code_lines: &[String]) -> (Vec<String>, Option<std::ops::Range<usize>>) {
  let mut cleaned_lines = Vec::with_capacity(code_lines.len());
  let mut active_range = None;
  let mut line_start = 0usize;

  for (idx, line) in code_lines.iter().enumerate() {
    let (cleaned, line_range) = strip_signature_active_markers_from_line(line);
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

fn byte_range_overlaps_active(
  byte_start: usize,
  byte_end: usize,
  active_range: Option<&std::ops::Range<usize>>,
) -> bool {
  active_range.is_some_and(|active| byte_start < active.end && byte_end > active.start)
}

fn preview_highlight_at(
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

fn render_code_lines_with_active_style(
  code_lines: &[String],
  base_style: Style,
  active_parameter_style: Style,
  active_range: Option<&std::ops::Range<usize>>,
) -> Vec<Vec<DocsStyledRun>> {
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
      if byte_range_overlaps_active(byte_idx, byte_end, active_range) {
        style = style.patch(active_parameter_style);
        kind = DocsSemanticKind::ActiveParameter;
      }
      if (style != run_style || kind != run_kind) && !piece.is_empty() {
        push_docs_run(&mut runs, std::mem::take(&mut piece), run_style, run_kind);
      }
      run_style = style;
      run_kind = kind;
      piece.push(ch);
      byte_idx = byte_end;
    }

    push_docs_run(&mut runs, piece, run_style, run_kind);
    if runs.is_empty() {
      runs.push(DocsStyledRun {
        text: String::new(),
        style: base_style,
        kind: DocsSemanticKind::Code,
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

fn highlighted_code_block_lines(
  code_lines: &[String],
  styles: &DocsPanelStyles,
  editor: &SwiftEditor,
  language: Option<&str>,
) -> Vec<Vec<DocsStyledRun>> {
  if code_lines.is_empty() {
    return vec![Vec::new()];
  }
  let (code_lines, active_range) = strip_signature_active_markers_from_lines(code_lines);
  if code_lines.is_empty() {
    return vec![Vec::new()];
  }

  let Some(loader) = editor.loader.as_deref() else {
    return render_code_lines_with_active_style(
      &code_lines,
      styles.code,
      styles.active_parameter,
      active_range.as_ref(),
    );
  };

  let resolved_language = language.and_then(|marker| {
    let marker = marker.trim();
    let marker_lower = marker.to_ascii_lowercase();
    loader
      .language_for_name(marker)
      .or_else(|| loader.language_for_name(marker_lower.as_str()))
      .or_else(|| loader.language_for_scope(marker))
      .or_else(|| loader.language_for_scope(marker_lower.as_str()))
      .or_else(|| {
        language_filename_hints(marker)
          .into_iter()
          .find_map(|hint| loader.language_for_filename(Path::new(format!("tmp.{hint}").as_str())))
      })
  });
  let current_buffer_language = editor
    .file_path
    .as_deref()
    .and_then(|path| loader.language_for_filename(path))
    .or_else(|| {
      editor
        .lsp_document
        .as_ref()
        .and_then(|state| loader.language_for_name(state.language_id.as_str()))
    });
  let Some(language) = resolved_language.or(current_buffer_language) else {
    return render_code_lines_with_active_style(
      &code_lines,
      styles.code,
      styles.active_parameter,
      active_range.as_ref(),
    );
  };

  let joined = code_lines.join("\n");
  let rope = Rope::from_str(&joined);
  let Ok(syntax) = Syntax::new(rope.slice(..), language, loader) else {
    return render_code_lines_with_active_style(
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
      let mut style = preview_highlight_at(&highlights, byte_idx)
        .map(|highlight| styles.code.patch(editor.ui_theme.highlight(highlight)))
        .unwrap_or(styles.code);
      let mut kind = DocsSemanticKind::Code;
      if byte_range_overlaps_active(byte_idx, byte_end, active_range.as_ref()) {
        style = style.patch(styles.active_parameter);
        kind = DocsSemanticKind::ActiveParameter;
      }
      if (style != active_style || kind != active_kind) && !piece.is_empty() {
        push_docs_run(&mut runs, std::mem::take(&mut piece), active_style, active_kind);
      }
      active_style = style;
      active_kind = kind;
      piece.push(ch);
      byte_idx = byte_end;
    }

    push_docs_run(&mut runs, piece, active_style, active_kind);
    if runs.is_empty() {
      runs.push(DocsStyledRun {
        text: String::new(),
        style: styles.code,
        kind: DocsSemanticKind::Code,
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

fn docs_markdown_lines(markdown: &str, styles: &DocsPanelStyles, editor: &SwiftEditor) -> Vec<Vec<DocsStyledRun>> {
  let mut lines = Vec::new();
  for block in parse_markdown_blocks(markdown) {
    match block {
      DocsBlock::Paragraph(inline_runs) => {
        lines.push(docs_runs_from_inline(&inline_runs, styles, styles.base, DocsSemanticKind::Body));
      },
      DocsBlock::Heading { level, runs } => {
        let level_idx = level.saturating_sub(1).min(5) as usize;
        lines.push(docs_runs_from_inline(&runs, styles, styles.heading[level_idx], DocsSemanticKind::from_heading_level(level)));
      },
      DocsBlock::ListItem { marker, runs: inline_runs } => {
        let marker_text = match marker {
          DocsListMarker::Bullet => "• ".to_string(),
          DocsListMarker::Ordered(marker) => format!("{marker} "),
        };
        let mut runs = Vec::new();
        push_docs_run(&mut runs, marker_text, styles.bullet, DocsSemanticKind::ListMarker);
        runs.extend(docs_runs_from_inline(&inline_runs, styles, styles.base, DocsSemanticKind::Body));
        lines.push(runs);
      },
      DocsBlock::Quote(inline_runs) => {
        let mut runs = Vec::new();
        push_docs_run(&mut runs, "│ ".to_string(), styles.quote, DocsSemanticKind::QuoteMarker);
        runs.extend(docs_runs_from_inline(&inline_runs, styles, styles.quote, DocsSemanticKind::QuoteText));
        lines.push(runs);
      },
      DocsBlock::CodeFence { language, lines: code_lines } => {
        lines.extend(highlighted_code_block_lines(&code_lines, styles, editor, language.as_deref()));
      },
      DocsBlock::Rule => {
        lines.push(vec![DocsStyledRun {
          text: "───".to_string(),
          style: styles.rule,
          kind: DocsSemanticKind::Rule,
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

fn flatten_docs_runs(markdown: &str, editor: &SwiftEditor) -> Vec<DocsStyledRun> {
  let base_style = editor.ui_theme.try_get("ui.text").unwrap_or_default();
  let styles = docs_panel_styles(&editor.ui_theme, base_style);
  let lines = docs_markdown_lines(markdown, &styles, editor);
  let total_lines = lines.len();
  let mut runs = Vec::new();
  for (index, line) in lines.into_iter().enumerate() {
    runs.extend(line);
    if index + 1 < total_lines {
      push_docs_run(&mut runs, "\n".to_string(), styles.base, DocsSemanticKind::Body);
    }
  }
  runs
}

fn docs_run_kind_code(kind: DocsSemanticKind) -> u8 {
  match kind {
    DocsSemanticKind::Body => 0,
    DocsSemanticKind::Heading1 => 1,
    DocsSemanticKind::Heading2 => 2,
    DocsSemanticKind::Heading3 => 3,
    DocsSemanticKind::Heading4 => 4,
    DocsSemanticKind::Heading5 => 5,
    DocsSemanticKind::Heading6 => 6,
    DocsSemanticKind::ListMarker => 7,
    DocsSemanticKind::QuoteMarker => 8,
    DocsSemanticKind::QuoteText => 9,
    DocsSemanticKind::Link => 10,
    DocsSemanticKind::InlineCode => 11,
    DocsSemanticKind::Code => 12,
    DocsSemanticKind::ActiveParameter => 13,
    DocsSemanticKind::Rule => 14,
  }
}

fn active_docs_cursor_position(plan: &RenderPlan) -> Option<(u16, u16)> {
  plan.cursors.first().map(|cursor| {
    (
      plan.content_offset_x.saturating_add(cursor.pos.col as u16),
      cursor.pos.row as u16,
    )
  })
}

fn capabilities_support_single_char(
  raw_capabilities: &serde_json::Value,
  provider_key: &str,
  characters_key: &str,
  ch: char,
) -> bool {
  let Some(values) = raw_capabilities
    .get(provider_key)
    .and_then(|provider| provider.get(characters_key))
    .and_then(serde_json::Value::as_array)
  else {
    return false;
  };

  values.iter().filter_map(serde_json::Value::as_str).any(|value| {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first == ch && chars.next().is_none())
  })
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

fn completion_kind_icon(kind: LspCompletionItemKind) -> &'static str {
  use LspCompletionItemKind::*;
  match kind {
    Text => "w",
    Method | Function | Constructor => "f",
    Field | Property => "m",
    Variable | Value => "v",
    Class => "c",
    Interface => "i",
    Module => "M",
    Unit => "u",
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

fn completion_kind_color(kind: LspCompletionItemKind) -> Color {
  use LspCompletionItemKind::*;
  match kind {
    Method | Function | Constructor | Operator => the_lib::render::graphics::Color::Rgb(0xDB, 0xBF, 0xEF),
    Field | Variable | Property | Value | Reference => the_lib::render::graphics::Color::Rgb(0xA4, 0xA0, 0xE8),
    Class | Interface | Enum | Struct | TypeParameter => the_lib::render::graphics::Color::Rgb(0xEF, 0xBA, 0x5D),
    Module | Folder | EnumMember | Constant => the_lib::render::graphics::Color::Rgb(0xE8, 0xDC, 0xA0),
    Keyword => the_lib::render::graphics::Color::Rgb(0xEC, 0xCD, 0xBA),
    Snippet => the_lib::render::graphics::Color::Rgb(0x9F, 0xF2, 0x8F),
    Event => the_lib::render::graphics::Color::Rgb(0xF4, 0x78, 0x68),
    Text | Unit | Color | File => the_lib::render::graphics::Color::Rgb(0xCC, 0xCC, 0xCC),
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

fn completion_insert_text(item: &LspCompletionItem, preferred_text: Option<&str>) -> String {
  let mut text = preferred_text
    .map(ToOwned::to_owned)
    .or_else(|| item.insert_text.clone())
    .unwrap_or_else(|| item.label.clone());
  if item.insert_text_format == Some(LspInsertTextFormat::Snippet) {
    text = render_lsp_snippet(&text).text;
  }
  text
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
  item:         LspCompletionItem,
  cursor_range: Option<std::ops::Range<usize>>,
}

fn normalize_completion_item_for_apply(mut item: LspCompletionItem) -> CompletionApplyItem {
  let mut cursor_range = None;
  if item.insert_text_format == Some(LspInsertTextFormat::Snippet) {
    if let Some(insert_text) = item.insert_text.as_mut() {
      let rendered = render_lsp_snippet(insert_text);
      if item.primary_edit.is_none() {
        cursor_range = rendered.cursor_char_range.clone();
      }
      *insert_text = rendered.text;
    }
    if let Some(primary_edit) = item.primary_edit.as_mut() {
      let rendered = render_lsp_snippet(&primary_edit.new_text);
      cursor_range = rendered.cursor_char_range.clone();
      primary_edit.new_text = rendered.text;
    }
    for additional in &mut item.additional_edits {
      additional.new_text = render_lsp_snippet(&additional.new_text).text;
    }
  }
  if cursor_range.is_none()
    && let Some((_origin, range)) = promote_callable_completion_fallback(&mut item)
  {
    cursor_range = Some(range);
  }
  CompletionApplyItem { item, cursor_range }
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
    (&mut primary.new_text, CompletionSnippetCursorOrigin::PrimaryEdit)
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
  doc: &mut Document,
  mapped_base: usize,
  cursor_range: &std::ops::Range<usize>,
) {
  let max = doc.text().len_chars();
  let anchor = mapped_base.saturating_add(cursor_range.start).min(max);
  let head = mapped_base.saturating_add(cursor_range.end).min(max);
  let _ = doc.set_selection(Selection::single(anchor, head));
}

fn selected_completion_docs_text(editor: &SwiftEditor) -> Option<&str> {
  editor
    .completion_menu
    .selected
    .and_then(|idx| editor.completion_menu.items.get(idx))
    .and_then(|item| item.documentation.as_deref())
    .map(str::trim)
    .filter(|docs| !docs.is_empty())
}

const fn completion_menu_visible_rows() -> usize { 10 }

fn completion_docs_target_height(viewport_height: u16, completion_panel_height: u16) -> u16 {
  viewport_height.min(completion_panel_height.max(8)).max(1)
}

fn docs_panel_line_width(line: &[DocsStyledRun]) -> u16 {
  line
    .iter()
    .flat_map(|run| run.text.graphemes(true))
    .map(|grapheme| grapheme_width(grapheme))
    .sum::<usize>()
    .min(u16::MAX as usize) as u16
}

fn docs_panel_dimensions(
  markdown: &str,
  editor: &SwiftEditor,
  min_width: u16,
  max_width: u16,
  min_height: u16,
  max_height: u16,
) -> (u16, u16) {
  let base_style = editor.ui_theme.try_get("ui.text").unwrap_or_default();
  let styles = docs_panel_styles(&editor.ui_theme, base_style);
  let lines = docs_markdown_lines(markdown, &styles, editor);
  let line_count = lines.len().max(1).min(u16::MAX as usize) as u16;
  let widest_line = lines.iter().map(|line| docs_panel_line_width(line)).max().unwrap_or(0);
  let width = widest_line.saturating_add(4).clamp(min_width, max_width);
  let height = line_count.saturating_add(2).clamp(min_height, max_height);
  (width, height)
}

fn completion_menu_panel_record(
  viewport_width: u16,
  viewport_height: u16,
  cursor: Option<(u16, u16)>,
  panel_width: u16,
  panel_height: u16,
) -> the_editor_snapshot_completion_menu_t {
  let area = the_default::OverlayRect::new(0, 0, viewport_width, viewport_height);
  let rect = completion_panel_rect(area, panel_width, panel_height, cursor);
  the_editor_snapshot_completion_menu_t {
    is_open: true,
    col: rect.x,
    row: rect.y,
    width: rect.width,
    height: rect.height,
    selected_index: -1,
    item_count: 0,
    scroll_offset: 0,
  }
}

fn completion_docs_panel_record(
  viewport_width: u16,
  viewport_height: u16,
  panel_width: u16,
  panel_height: u16,
  completion_panel: the_editor_snapshot_completion_menu_t,
) -> Option<the_editor_snapshot_docs_panel_t> {
  let area = the_default::OverlayRect::new(0, 0, viewport_width, viewport_height);
  let completion_rect = the_default::OverlayRect::new(
    completion_panel.col,
    completion_panel.row,
    completion_panel.width,
    completion_panel.height,
  );
  let rect = completion_docs_panel_rect(area, panel_width, panel_height, completion_rect)?;
  Some(the_editor_snapshot_docs_panel_t {
    is_open: true,
    col: rect.x,
    row: rect.y,
    width: rect.width,
    height: rect.height,
    run_count: 0,
  })
}

fn docs_panel_record(
  viewport_width: u16,
  viewport_height: u16,
  cursor: Option<(u16, u16)>,
  panel_width: u16,
  panel_height: u16,
  signature: bool,
) -> the_editor_snapshot_docs_panel_t {
  let area = the_default::OverlayRect::new(0, 0, viewport_width, viewport_height);
  let rect = if signature {
    signature_help_panel_rect(area, panel_width, panel_height, cursor)
  } else {
    completion_panel_rect(area, panel_width, panel_height, cursor)
  };
  the_editor_snapshot_docs_panel_t {
    is_open: true,
    col: rect.x,
    row: rect.y,
    width: rect.width,
    height: rect.height,
    run_count: 0,
  }
}

fn is_symbol_word_char(ch: char) -> bool {
  ch == '_' || ch.is_alphanumeric()
}

fn is_completion_replace_char(ch: char) -> bool { is_symbol_word_char(ch) }

fn active_file_watch_latency() -> Duration {
  Duration::from_millis(120)
}

fn active_file_self_save_suppress_window() -> Duration {
  Duration::from_millis(500)
}

fn watch_statusline_text_for_state(state: FileWatchReloadState) -> Option<String> {
  match state {
    FileWatchReloadState::Conflict => Some("watch: conflict".to_string()),
    FileWatchReloadState::ReloadNeeded => Some("watch: reload pending".to_string()),
    FileWatchReloadState::Clean => None,
  }
}

fn damage_reason_code(reason: RenderDamageReason) -> u8 {
  match reason {
    RenderDamageReason::None => 0,
    RenderDamageReason::Full => 1,
    RenderDamageReason::Layout => 2,
    RenderDamageReason::Text => 3,
    RenderDamageReason::Decoration => 4,
    RenderDamageReason::Cursor => 5,
    RenderDamageReason::Scroll => 6,
    RenderDamageReason::Theme => 7,
    RenderDamageReason::PaneStructure => 8,
  }
}

fn cursor_kind_code(kind: CursorKind) -> u8 {
  match kind {
    CursorKind::Block => 0,
    CursorKind::Bar => 1,
    CursorKind::Underline => 2,
    CursorKind::Hollow => 3,
    CursorKind::Hidden => 4,
  }
}

fn selection_kind_code(kind: RenderSelectionKind) -> u8 {
  match kind {
    RenderSelectionKind::Primary => 0,
    RenderSelectionKind::Match => 1,
    RenderSelectionKind::Hover => 2,
  }
}

fn overlay_rect_kind_code(kind: OverlayRectKind) -> u8 {
  match kind {
    OverlayRectKind::Panel => 0,
    OverlayRectKind::Divider => 1,
    OverlayRectKind::Highlight => 2,
    OverlayRectKind::Backdrop => 3,
  }
}

fn file_picker_kind_code(kind: FilePickerKind) -> u8 {
  match kind {
    FilePickerKind::Generic => 0,
    FilePickerKind::Diagnostics => 1,
    FilePickerKind::Symbols => 2,
    FilePickerKind::LiveGrep => 3,
    FilePickerKind::VcsDiff => 4,
  }
}

fn file_picker_row_kind_code(kind: FilePickerRowKind) -> u8 {
  match kind {
    FilePickerRowKind::Generic => 0,
    FilePickerRowKind::Diagnostics => 1,
    FilePickerRowKind::Symbols => 2,
    FilePickerRowKind::LiveGrepHeader => 3,
    FilePickerRowKind::LiveGrepMatch => 4,
    FilePickerRowKind::VcsDiffHeader => 5,
    FilePickerRowKind::VcsDiffHunk => 6,
  }
}

fn file_picker_preview_navigation_code(mode: FilePickerPreviewNavigationMode) -> u8 {
  match mode {
    FilePickerPreviewNavigationMode::Static => 0,
    FilePickerPreviewNavigationMode::Scrollable => 1,
    FilePickerPreviewNavigationMode::Anchored => 2,
  }
}

fn file_picker_preview_kind_code(kind: FilePickerPreviewWindowKind) -> u8 {
  match kind {
    FilePickerPreviewWindowKind::Empty => 0,
    FilePickerPreviewWindowKind::Source => 1,
    FilePickerPreviewWindowKind::Text => 2,
    FilePickerPreviewWindowKind::Message => 3,
    FilePickerPreviewWindowKind::VcsDiff => 4,
  }
}

fn file_picker_preview_line_code(kind: FilePickerPreviewLineKind) -> u8 {
  match kind {
    FilePickerPreviewLineKind::Content => 0,
    FilePickerPreviewLineKind::TruncatedAbove => 1,
    FilePickerPreviewLineKind::TruncatedBelow => 2,
  }
}

fn file_picker_preview_line_code_for_vcs(kind: FilePickerVcsDiffPreviewRowKind) -> u8 {
  match kind {
    FilePickerVcsDiffPreviewRowKind::Context => 0,
    FilePickerVcsDiffPreviewRowKind::SectionHeader => 3,
    FilePickerVcsDiffPreviewRowKind::Info => 4,
    FilePickerVcsDiffPreviewRowKind::Added => 5,
    FilePickerVcsDiffPreviewRowKind::Removed => 6,
    FilePickerVcsDiffPreviewRowKind::Modified => 7,
    FilePickerVcsDiffPreviewRowKind::CollapsedAbove => 1,
    FilePickerVcsDiffPreviewRowKind::CollapsedBelow => 2,
  }
}

fn file_picker_preview_source_code(source: FilePickerVcsDiffPreviewLineSource) -> u8 {
  match source {
    FilePickerVcsDiffPreviewLineSource::Base => 1,
    FilePickerVcsDiffPreviewLineSource::Worktree => 2,
    FilePickerVcsDiffPreviewLineSource::Meta => 3,
  }
}

fn file_picker_preview_change_kind_code(kind: Option<FilePickerPreviewChangeKind>) -> i8 {
  match kind {
    None => -1,
    Some(FilePickerPreviewChangeKind::Added) => 0,
    Some(FilePickerPreviewChangeKind::Removed) => 1,
    Some(FilePickerPreviewChangeKind::Modified) => 2,
  }
}

fn style_to_ffi(style: Style, theme: &Theme) -> the_editor_style_t {
  the_editor_style_t {
    fg: color_to_rgba(style.fg, theme),
    bg: color_to_rgba(style.bg, theme),
    underline_color: color_to_rgba(style.underline_color, theme),
    add_modifiers: modifier_bits(style.add_modifier),
    remove_modifiers: modifier_bits(style.sub_modifier),
    underline_style: underline_style_code(style.underline_style),
  }
}

fn underline_style_code(style: Option<UnderlineStyle>) -> u8 {
  match style {
    None | Some(UnderlineStyle::Reset) => 0,
    Some(UnderlineStyle::Line) => 1,
    Some(UnderlineStyle::Curl) => 2,
    Some(UnderlineStyle::Dotted) => 3,
    Some(UnderlineStyle::Dashed) => 4,
    Some(UnderlineStyle::DoubleLine) => 5,
  }
}

fn modifier_bits(modifier: Modifier) -> u16 {
  let mut bits = 0;
  if modifier.contains(Modifier::BOLD) { bits |= STYLE_BOLD; }
  if modifier.contains(Modifier::DIM) { bits |= STYLE_DIM; }
  if modifier.contains(Modifier::ITALIC) { bits |= STYLE_ITALIC; }
  if modifier.contains(Modifier::SLOW_BLINK) { bits |= STYLE_SLOW_BLINK; }
  if modifier.contains(Modifier::RAPID_BLINK) { bits |= STYLE_RAPID_BLINK; }
  if modifier.contains(Modifier::REVERSED) { bits |= STYLE_REVERSED; }
  if modifier.contains(Modifier::HIDDEN) { bits |= STYLE_HIDDEN; }
  if modifier.contains(Modifier::CROSSED_OUT) { bits |= STYLE_CROSSED_OUT; }
  bits
}

fn color_to_rgba(color: Option<Color>, theme: &Theme) -> the_editor_rgba_t {
  let Some(color) = color else {
    return the_editor_rgba_t::default();
  };
  let (r, g, b) = resolve_color(color, theme);
  the_editor_rgba_t {
    present: true,
    r,
    g,
    b,
    a: 255,
  }
}

fn resolve_color(color: Color, theme: &Theme) -> (u8, u8, u8) {
  match color {
    Color::Reset => (0, 0, 0),
    Color::Black => ansi_color(0, theme),
    Color::Red => ansi_color(1, theme),
    Color::Green => ansi_color(2, theme),
    Color::Yellow => ansi_color(3, theme),
    Color::Blue => ansi_color(4, theme),
    Color::Magenta => ansi_color(5, theme),
    Color::Cyan => ansi_color(6, theme),
    Color::Gray => ansi_color(7, theme),
    Color::LightRed => ansi_color(8, theme),
    Color::LightGreen => ansi_color(9, theme),
    Color::LightYellow => ansi_color(10, theme),
    Color::LightBlue => ansi_color(11, theme),
    Color::LightMagenta => ansi_color(12, theme),
    Color::LightCyan => ansi_color(13, theme),
    Color::LightGray => ansi_color(14, theme),
    Color::White => ansi_color(15, theme),
    Color::Rgb(r, g, b) => (r, g, b),
    Color::Indexed(index) => indexed_color(index),
  }
}

fn ansi_color(index: usize, theme: &Theme) -> (u8, u8, u8) {
  if let Some(color) = theme.ghostty().palette_color(index) {
    return resolve_color(color, theme);
  }
  const ANSI: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (205, 49, 49),
    (13, 188, 121),
    (229, 229, 16),
    (36, 114, 200),
    (188, 63, 188),
    (17, 168, 205),
    (229, 229, 229),
    (102, 102, 102),
    (241, 76, 76),
    (35, 209, 139),
    (245, 245, 67),
    (59, 142, 234),
    (214, 112, 214),
    (41, 184, 219),
    (255, 255, 255),
  ];
  ANSI[index.min(15)]
}

fn indexed_color(index: u8) -> (u8, u8, u8) {
  if index < 16 {
    return ansi_color(index as usize, default_theme());
  }
  if index >= 232 {
    let gray = 8 + (index - 232) * 10;
    return (gray, gray, gray);
  }
  let index = index - 16;
  let r = index / 36;
  let g = (index % 36) / 6;
  let b = index % 6;
  (
    if r == 0 { 0 } else { r * 40 + 55 },
    if g == 0 { 0 } else { g * 40 + 55 },
    if b == 0 { 0 } else { b * 40 + 55 },
  )
}

fn init_loader(theme: &Theme) -> Result<Loader, String> {
  use the_lib::syntax::{
    config::Configuration,
    runtime_loader::RuntimeLoader,
  };
  use the_loader::config::user_lang_config;

  let config_value = user_lang_config().map_err(|err| err.to_string())?;
  let config: Configuration = config_value.try_into().map_err(|err| err.to_string())?;
  let loader = Loader::new(config, RuntimeLoader::new()).map_err(|err| err.to_string())?;
  loader.set_scopes(theme.scopes().iter().cloned().collect());
  Ok(loader)
}

fn setup_syntax(doc: &mut Document, path: &Path, loader: &Arc<Loader>) -> Result<(), String> {
  let lang = loader
    .language_for_filename(path)
    .ok_or_else(|| format!("unknown language for {}", path.display()))?;
  let syntax = Syntax::new(doc.text().slice(..), lang, loader.as_ref()).map_err(|err| err.to_string())?;
  doc.set_syntax_with_loader(syntax, loader.clone());
  Ok(())
}

fn theme_background_rgba(theme: &Theme) -> the_editor_rgba_t {
  color_to_rgba(
    theme
      .ghostty()
      .background()
      .or_else(|| theme.try_get("ui.background").and_then(|style| style.bg)),
    theme,
  )
}

fn theme_gutter_background_rgba(theme: &Theme) -> the_editor_rgba_t {
  let gutter_background = theme
    .try_get("ui.linenr")
    .and_then(|style| style.bg)
    .or_else(|| theme.try_get("ui.gutter").and_then(|style| style.bg))
    .or_else(|| theme.try_get("ui.background").and_then(|style| style.bg))
    .or_else(|| theme.ghostty().background());
  color_to_rgba(gutter_background, theme)
}

fn swift_gutter_config() -> GutterConfig {
  GutterConfig {
    layout: vec![
      GutterSlot::builtin(GutterType::Diff),
      GutterSlot::builtin(GutterType::Spacer),
      GutterSlot::builtin(GutterType::LineNumbers),
      GutterSlot::builtin(GutterType::Spacer),
      GutterSlot::builtin(GutterType::Diagnostics),
    ],
    ..GutterConfig::default()
  }
}

fn render_diff_styles_from_theme(theme: &Theme) -> RenderDiffGutterStyles {
  RenderDiffGutterStyles {
    added: theme
      .try_get("diff.plus")
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    modified: theme
      .try_get("diff.delta")
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
    removed: theme
      .try_get("diff.minus")
      .or_else(|| theme.try_get("ui.linenr"))
      .unwrap_or_default(),
  }
}

fn apply_swift_diff_gutter_markers(
  plan: &mut RenderPlan,
  diff_by_line: &BTreeMap<usize, RenderGutterDiffKind>,
  styles: RenderDiffGutterStyles,
) {
  if plan.gutter_column(GutterType::Diff).is_none() {
    return;
  }
  plan.clear_builtin_gutter_slot(GutterType::Diff);

  for (&doc_line, kind) in diff_by_line {
    let style = match kind {
      RenderGutterDiffKind::Added => styles.added,
      RenderGutterDiffKind::Modified => styles.modified,
      RenderGutterDiffKind::Removed => styles.removed,
    };
    let _ = plan.set_builtin_gutter_text(GutterType::Diff, doc_line, "▎", style);
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

fn select_ui_theme(catalog: &ThemeCatalog) -> (String, Theme) {
  if let Ok(theme_name) = std::env::var("THE_EDITOR_THEME") {
    let theme_name = theme_name.trim();
    if !theme_name.is_empty() {
      if let Some(theme) = catalog.load_theme(theme_name) {
        return (theme_name.to_string(), theme);
      }
      eprintln!("Unknown theme '{theme_name}', falling back to default theme.");
    }
  }

  let default_name = default_theme().name().to_string();
  (
    default_name.clone(),
    catalog
      .load_theme(&default_name)
      .unwrap_or_else(|| default_theme().clone()),
  )
}

fn max_scroll_row(line_count: usize, viewport_height: usize) -> usize {
  line_count.saturating_sub(viewport_height.max(1))
}

fn max_scroll_col(doc: &Document, text_format: &TextFormat, viewport_width: usize) -> usize {
  if text_format.soft_wrap {
    return 0;
  }

  let viewport_width = viewport_width.max(1);
  let mut longest_visual_col = 1usize;
  for line in doc.text().slice(..).lines() {
    let line = line.to_string();
    let mut visual_col = 0usize;
    for grapheme in UnicodeSegmentation::graphemes(line.as_str(), true) {
      visual_col = visual_col.saturating_add(grapheme_width(grapheme));
    }
    visual_col = visual_col.saturating_add(1);
    longest_visual_col = longest_visual_col.max(visual_col);
  }

  longest_visual_col.saturating_sub(viewport_width.saturating_sub(1))
}

fn default_workspace_root() -> PathBuf {
  std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn resolved_workspace_root_for_path(path: &Path) -> PathBuf {
  let base = path.parent().unwrap_or(path);
  the_default::workspace_root(base)
}

fn display_name_for_path(path: &Path) -> String {
  path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_else(|| path.display().to_string())
}

fn relative_document_path(path: &Path, workspace_root: &Path) -> String {
  let parent = path.parent().unwrap_or(path);
  if let Ok(relative) = parent.strip_prefix(workspace_root) {
    let relative = relative.display().to_string();
    if relative.is_empty() {
      workspace_root.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_else(|| workspace_root.display().to_string())
    } else {
      relative
    }
  } else {
    parent.display().to_string()
  }
}

fn title_case_status_label(text: &str) -> String {
  text
    .split(['-', '_', '.'])
    .filter(|part| !part.is_empty())
    .map(|part| {
      let mut chars = part.chars();
      let Some(first) = chars.next() else { return String::new(); };
      let mut out = String::new();
      out.extend(first.to_uppercase());
      out.push_str(chars.as_str());
      out
    })
    .collect::<Vec<_>>()
    .join(" ")
}

fn document_language_name(editor: &SwiftEditor) -> Option<String> {
  let loader = editor.loader.as_deref()?;
  let path = editor.file_path()?;
  let language = loader.language_for_filename(path)?;
  let language_id = loader.language(language).config().language_id();
  Some(title_case_status_label(language_id))
}

fn line_ending_label(line_ending: LineEnding) -> &'static str {
  match line_ending {
    LineEnding::Crlf => "CRLF",
    LineEnding::LF => "LF",
    LineEnding::VT => "VT",
    LineEnding::FF => "FF",
    LineEnding::CR => "CR",
    LineEnding::Nel => "NEL",
    LineEnding::LS => "LS",
    LineEnding::PS => "PS",
  }
}

fn statusline_emphasis_code(emphasis: StatuslineEmphasis) -> u8 {
  match emphasis {
    StatuslineEmphasis::Normal => 0,
    StatuslineEmphasis::Muted => 1,
    StatuslineEmphasis::Strong => 2,
  }
}

fn read_rope(path: Option<&Path>) -> Rope {
  path
    .and_then(|path| fs::read_to_string(path).ok())
    .map(|text| Rope::from_str(&text))
    .unwrap_or_else(Rope::new)
}

unsafe fn path_from_c(path: *const c_char) -> Option<PathBuf> {
  if path.is_null() {
    return None;
  }
  let path = unsafe { CStr::from_ptr(path) }.to_string_lossy().to_string();
  if path.trim().is_empty() {
    None
  } else {
    Some(PathBuf::from(path))
  }
}

unsafe fn string_from_c(ptr: *const c_char) -> Option<String> {
  if ptr.is_null() {
    return None;
  }
  Some(unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_new(path: *const c_char) -> *mut the_editor_handle_t {
  let path = unsafe { path_from_c(path) };
  let handle = the_editor_handle_t {
    editor: SwiftEditor::new(path.as_deref()),
  };
  Box::into_raw(Box::new(handle))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_free(handle: *mut the_editor_handle_t) {
  if handle.is_null() { return; }
  drop(unsafe { Box::from_raw(handle) });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_open(handle: *mut the_editor_handle_t, path: *const c_char) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  let Some(path) = (unsafe { path_from_c(path) }) else { return false; };
  handle.editor.open_path(&path)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_configure_surface(handle: *mut the_editor_handle_t, config: the_editor_surface_config_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.configure_surface(config)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_set_viewport(handle: *mut the_editor_handle_t, cols: u16, rows: u16) {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return; };
  handle.editor.set_viewport(cols, rows);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_set_scroll_row(handle: *mut the_editor_handle_t, row: u32) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.set_scroll_row(row)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_set_scroll_col(handle: *mut the_editor_handle_t, col: u32) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.set_scroll_col(col)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_handle_key(handle: *mut the_editor_handle_t, event: the_editor_key_event_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.handle_key_event(event)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_toggle_command_palette(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.toggle_command_palette()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_close_command_palette(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.close_command_palette()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_command_palette_set_query(handle: *mut the_editor_handle_t, query: *const c_char) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  let Some(query) = (unsafe { string_from_c(query) }) else { return false; };
  handle.editor.set_command_palette_query(&query)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_command_palette_select_next(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.move_command_palette_selection(true)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_command_palette_select_previous(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.move_command_palette_selection(false)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_command_palette_select_visible_index(handle: *mut the_editor_handle_t, visible_index: usize) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.select_command_palette_visible_index(visible_index)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_command_palette_submit(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.submit_command_palette()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_close_completion_menu(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.close_completion_menu_ui()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_completion_menu_select_index(handle: *mut the_editor_handle_t, index: usize) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.select_completion_menu_index(index)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_set_completion_menu_scroll(handle: *mut the_editor_handle_t, offset: usize) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.set_completion_menu_scroll(offset)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_completion_menu_submit(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.submit_completion_menu_selection()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_poll_background_tasks(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.poll_background_tasks()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_open_search_prompt(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.open_search_prompt()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_close_input_prompt(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.close_input_prompt()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_input_prompt_set_query(handle: *mut the_editor_handle_t, query: *const c_char) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  let Some(query) = (unsafe { string_from_c(query) }) else { return false; };
  handle.editor.set_input_prompt_query(&query)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_input_prompt_submit(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.submit_input_prompt()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_input_prompt_step_next(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.step_input_prompt(the_default::Direction::Forward)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_input_prompt_step_previous(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.step_input_prompt(the_default::Direction::Backward)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_close_docs_panels(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.close_docs_panels()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_configure_file_picker(handle: *mut the_editor_handle_t, list_visible_rows: usize, preview_visible_rows: usize) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.configure_file_picker_layout(list_visible_rows, preview_visible_rows)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_close_file_picker(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.close_file_picker()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_file_picker_set_query(handle: *mut the_editor_handle_t, query: *const c_char) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  let Some(query) = (unsafe { string_from_c(query) }) else { return false; };
  handle.editor.set_file_picker_query(&query)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_file_picker_select_next(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.move_file_picker_selection(true)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_file_picker_select_previous(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.move_file_picker_selection(false)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_file_picker_set_list_offset(handle: *mut the_editor_handle_t, offset: usize) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.set_file_picker_list_offset(offset)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_file_picker_set_preview_offset(handle: *mut the_editor_handle_t, offset: usize, _visible_rows: usize) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.set_file_picker_preview_offset(offset)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_file_picker_select_index(handle: *mut the_editor_handle_t, index: usize) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.select_file_picker_index(index)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_file_picker_submit(handle: *mut the_editor_handle_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.submit_file_picker()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_insert_text(handle: *mut the_editor_handle_t, text: *const c_char) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  let Some(text) = (unsafe { string_from_c(text) }) else { return false; };
  handle.editor.insert_text(&text)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_primary_selection_utf16_location(handle: *mut the_editor_handle_t) -> u32 {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return 0; };
  handle.editor.primary_selection_utf16().0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_primary_selection_utf16_length(handle: *mut the_editor_handle_t) -> u32 {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return 0; };
  handle.editor.primary_selection_utf16().1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_primary_selection_text(handle: *mut the_editor_handle_t) -> *mut c_char {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return ptr::null_mut(); };
  match CString::new(handle.editor.primary_selection_text()) {
    Ok(value) => value.into_raw(),
    Err(_) => ptr::null_mut(),
  }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_create(handle: *mut the_editor_handle_t) -> *mut the_editor_snapshot_t {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return ptr::null_mut(); };
  let started = Instant::now();
  let snapshot = handle.editor.build_snapshot();
  theme_perf_log(format!(
    "snapshot_create theme_gen={} lines={} spans={} cells={} palette_items={} total_ms={:.2}",
    snapshot.info.theme_generation,
    snapshot.lines.len(),
    snapshot.spans.len(),
    snapshot.text_cells.len(),
    snapshot.command_palette_items.len(),
    started.elapsed().as_secs_f64() * 1000.0,
  ));
  Box::into_raw(Box::new(the_editor_snapshot_t { snapshot }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_free(snapshot: *mut the_editor_snapshot_t) {
  if snapshot.is_null() { return; }
  drop(unsafe { Box::from_raw(snapshot) });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_info(snapshot: *const the_editor_snapshot_t) -> the_editor_snapshot_info_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_info_t::default(); };
  snapshot.snapshot.info
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_document(snapshot: *const the_editor_snapshot_t) -> the_editor_snapshot_document_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_document_t::default(); };
  snapshot.snapshot.document.document
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_status(snapshot: *const the_editor_snapshot_t) -> the_editor_snapshot_status_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_status_t::default(); };
  snapshot.snapshot.status.status
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_status_item_at(snapshot: *const the_editor_snapshot_t, item_index: usize) -> the_editor_snapshot_status_item_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_status_item_t::default(); };
  snapshot.snapshot.status_items.get(item_index).map(|record| record.item).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_command_palette(snapshot: *const the_editor_snapshot_t) -> the_editor_snapshot_command_palette_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_command_palette_t::default(); };
  snapshot.snapshot.command_palette.palette
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_command_palette_item_at(snapshot: *const the_editor_snapshot_t, item_index: usize) -> the_editor_snapshot_command_palette_item_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_command_palette_item_t::default(); };
  snapshot.snapshot.command_palette_items.get(item_index).map(|record| record.item).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_completion_menu(snapshot: *const the_editor_snapshot_t) -> the_editor_snapshot_completion_menu_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_completion_menu_t::default(); };
  snapshot.snapshot.completion_menu.menu
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_completion_menu_item_at(snapshot: *const the_editor_snapshot_t, item_index: usize) -> the_editor_snapshot_completion_menu_item_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_completion_menu_item_t::default(); };
  snapshot.snapshot.completion_menu_items.get(item_index).map(|record| record.item).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_input_prompt(snapshot: *const the_editor_snapshot_t) -> the_editor_snapshot_input_prompt_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_input_prompt_t::default(); };
  snapshot.snapshot.input_prompt.prompt
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_hover_docs_panel(snapshot: *const the_editor_snapshot_t) -> the_editor_snapshot_docs_panel_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_docs_panel_t::default(); };
  snapshot.snapshot.hover_docs.panel
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_hover_docs_run_at(snapshot: *const the_editor_snapshot_t, run_index: usize) -> the_editor_snapshot_docs_run_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_docs_run_t::default(); };
  snapshot.snapshot.hover_docs_runs.get(run_index).map(|record| record.run).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_completion_docs_panel(snapshot: *const the_editor_snapshot_t) -> the_editor_snapshot_docs_panel_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_docs_panel_t::default(); };
  snapshot.snapshot.completion_docs.panel
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_completion_docs_run_at(snapshot: *const the_editor_snapshot_t, run_index: usize) -> the_editor_snapshot_docs_run_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_docs_run_t::default(); };
  snapshot.snapshot.completion_docs_runs.get(run_index).map(|record| record.run).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_signature_help_panel(snapshot: *const the_editor_snapshot_t) -> the_editor_snapshot_docs_panel_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_docs_panel_t::default(); };
  snapshot.snapshot.signature_help.panel
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_signature_help_run_at(snapshot: *const the_editor_snapshot_t, run_index: usize) -> the_editor_snapshot_docs_run_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_docs_run_t::default(); };
  snapshot.snapshot.signature_help_runs.get(run_index).map(|record| record.run).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_file_picker(snapshot: *const the_editor_snapshot_t) -> the_editor_snapshot_file_picker_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_file_picker_t::default(); };
  snapshot.snapshot.file_picker.picker
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_file_picker_item_at(snapshot: *const the_editor_snapshot_t, item_index: usize) -> the_editor_snapshot_file_picker_item_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_file_picker_item_t::default(); };
  snapshot.snapshot.file_picker_items.get(item_index).map(|record| record.item).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_file_picker_preview_line_at(snapshot: *const the_editor_snapshot_t, line_index: usize) -> the_editor_snapshot_file_picker_preview_line_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_file_picker_preview_line_t::default(); };
  snapshot.snapshot.file_picker_preview_lines.get(line_index).map(|record| record.line).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_file_picker_preview_segment_at(snapshot: *const the_editor_snapshot_t, line_index: usize, segment_index: usize) -> the_editor_snapshot_file_picker_preview_segment_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_file_picker_preview_segment_t::default(); };
  let Some(line) = snapshot.snapshot.file_picker_preview_lines.get(line_index) else { return the_editor_snapshot_file_picker_preview_segment_t::default(); };
  if segment_index >= line.line.segment_count { return the_editor_snapshot_file_picker_preview_segment_t::default(); }
  snapshot.snapshot.file_picker_preview_segments.get(line.segment_start + segment_index).map(|record| record.segment).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_line_at(snapshot: *const the_editor_snapshot_t, line_index: usize) -> the_editor_snapshot_line_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_line_t::default(); };
  snapshot.snapshot.lines.get(line_index).map(|record| record.line).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_span_at(snapshot: *const the_editor_snapshot_t, line_index: usize, span_index: usize) -> the_editor_snapshot_span_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_span_t::default(); };
  let Some(line) = snapshot.snapshot.lines.get(line_index) else { return the_editor_snapshot_span_t::default(); };
  if span_index >= line.line.span_count { return the_editor_snapshot_span_t::default(); }
  snapshot.snapshot.spans.get(line.span_start + span_index).map(|record| record.span).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_text_cell_at(snapshot: *const the_editor_snapshot_t, line_index: usize, text_cell_index: usize) -> the_editor_snapshot_text_cell_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_text_cell_t::default(); };
  let Some(line) = snapshot.snapshot.lines.get(line_index) else { return the_editor_snapshot_text_cell_t::default(); };
  if text_cell_index >= line.line.text_cell_count { return the_editor_snapshot_text_cell_t::default(); }
  snapshot.snapshot.text_cells.get(line.text_cell_start + text_cell_index).map(|record| record.cell).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_cursor_at(snapshot: *const the_editor_snapshot_t, cursor_index: usize) -> the_editor_snapshot_cursor_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_cursor_t::default(); };
  snapshot.snapshot.cursors.get(cursor_index).copied().unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_selection_at(snapshot: *const the_editor_snapshot_t, selection_index: usize) -> the_editor_snapshot_selection_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_selection_t::default(); };
  snapshot.snapshot.selections.get(selection_index).copied().unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_overlay_at(snapshot: *const the_editor_snapshot_t, overlay_index: usize) -> the_editor_snapshot_overlay_t {
  let Some(snapshot) = (unsafe { snapshot.as_ref() }) else { return the_editor_snapshot_overlay_t::default(); };
  snapshot.snapshot.overlays.get(overlay_index).map(|record| record.overlay).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_string_free(value: *mut c_char) {
  if value.is_null() { return; }
  drop(unsafe { CString::from_raw(value) });
}
