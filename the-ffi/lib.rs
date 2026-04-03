use std::{
  collections::VecDeque,
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
  time::Instant,
};

use ropey::Rope;
use the_core::{
  grapheme::grapheme_width,
  line_ending::LineEnding,
};
use the_default::{
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
  PendingInput,
  PickerRuntimeStore,
  SearchPromptState,
  ThemeCatalog,
  WorkingDirectoryState,
  build_dispatch,
  build_statusline_snapshot,
  builtin_completion_menu_keymaps,
  builtin_keymaps,
  close_file_picker,
  file_picker_icon_name_for_path,
  command_palette_filtered_indices,
  command_palette_placeholder_text,
  command_palette_selected_filtered_index,
  file_picker_item_selectable,
  file_picker_preview_window,
  file_picker_row_data_for_kind,
  handle_command_prompt_key,
  handle_key,
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
  sync_command_palette_preview,
  update_command_palette_for_input,
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
    RenderGenerationState,
    RenderPlan,
    RenderSelectionKind,
    RenderStyles,
    SyntaxHighlightAdapter,
    base_render_layer_row_hashes,
    build_plan,
    finish_render_generations,
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
  selection::CursorPick,
  syntax::{
    Highlight,
    HighlightCache,
    Loader,
    Syntax,
  },
  view::ViewState,
};
use unicode_segmentation::UnicodeSegmentation;
use the_vcs::DiffProviderRegistry;

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

#[derive(Default)]
struct OwnedSnapshot {
  info:                  the_editor_snapshot_info_t,
  document:              DocumentRecord,
  status:                StatusRecord,
  status_items:          Vec<StatusItemRecord>,
  command_palette:       CommandPaletteRecord,
  command_palette_items: Vec<CommandPaletteItemRecord>,
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
      pending_input: None,
      registers: Registers::new(),
      register: None,
      macro_recording: None,
      macro_replaying: Vec::new(),
      macro_queue: VecDeque::new(),
      last_motion: None,
      text_format,
      soft_wrap_enabled: false,
      gutter_config: the_lib::render::GutterConfig::default(),
      loader,
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
    };

    set_file_picker_syntax_loader(&mut this.file_picker, this.loader.clone());
    this.refresh_active_document_syntax();
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
    handle_key(self, event);
    self.ensure_cursor_visible();
    true
  }

  fn insert_text(&mut self, text: &str) -> bool {
    let mut changed = false;
    for ch in text.chars() {
      let event = match ch {
        '\n' => KeyEvent { key: Key::Enter, modifiers: the_default::Modifiers::empty() },
        '\t' => KeyEvent { key: Key::Tab, modifiers: the_default::Modifiers::empty() },
        _ => KeyEvent { key: Key::Char(ch), modifiers: the_default::Modifiers::empty() },
      };
      handle_key(self, event);
      changed = true;
    }
    if changed {
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
    let _ = self.refresh_picker_state();
    let styles = self.render_styles();
    let frame = the_default::frame_render_plan_with_styles(self, styles);
    let plan = frame.active_plan();
    OwnedSnapshot::from_editor(self, plan)
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
    if self
      .editor
      .apply_transaction_to_active_buffer(transaction, self.loader.as_deref())
      .is_err()
    {
      return false;
    }
    if !transaction.changes().is_empty() {
      self.highlight_cache.clear();
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
  fn gutter_config(&self) -> &the_lib::render::GutterConfig { &self.gutter_config }
  fn gutter_config_mut(&mut self) -> &mut the_lib::render::GutterConfig { &mut self.gutter_config }
  fn text_annotations(&self) -> TextAnnotations<'_> { TextAnnotations::default() }
  fn syntax_loader(&self) -> Option<&the_lib::syntax::Loader> { self.loader.as_deref() }
  fn scrolloff(&self) -> usize { SWIFT_SCROLLOFF }
  fn ui_theme(&self) -> &Theme { &self.ui_theme }
  fn ui_theme_name(&self) -> &str { &self.ui_theme_name }
  fn vcs_statusline_text(&self) -> Option<String> {
    let path = self.file_path.as_deref()?;
    self.vcs_provider.get_statusline_info(path).map(|info| info.statusline_text())
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
    Ok(())
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
