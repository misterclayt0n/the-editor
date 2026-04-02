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
  sync::mpsc,
};

use ropey::Rope;
use the_core::grapheme::grapheme_width;
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
  WorkingDirectoryState,
  build_dispatch,
  builtin_completion_menu_keymaps,
  builtin_keymaps,
  handle_key,
  install_default_wiring,
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
    FrameRenderPlan,
    NoHighlights,
    RenderDamageReason,
    RenderPlan,
    RenderSelectionKind,
    RenderStyles,
    build_plan,
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
  view::ViewState,
};
use unicode_segmentation::UnicodeSegmentation;

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

fn ffi_debug_log(message: impl AsRef<str>) {
  if env::var("THE_EDITOR_FFI_DEBUG_VIEWPORT").ok().as_deref() == Some("1") {
    eprintln!("[the-ffi] {}", message.as_ref());
  }
}

#[derive(Default)]
struct OwnedSnapshot {
  info:             the_editor_snapshot_info_t,
  lines:            Vec<LineRecord>,
  spans:            Vec<SpanRecord>,
  text_cells:       Vec<TextCellRecord>,
  cursors:          Vec<the_editor_snapshot_cursor_t>,
  selections:       Vec<the_editor_snapshot_selection_t>,
  overlays:         Vec<OverlayRecord>,
  strings:          Vec<CString>,
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
  ui_theme_name:                 String,
  ui_theme:                      Theme,
  surface:                       SurfaceConfig,
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
      .and_then(Path::parent)
      .map(Path::to_path_buf)
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

    Self {
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
      ui_theme_name: default_theme().name().to_string(),
      ui_theme: default_theme().clone(),
      surface,
    }
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
        self.workspace_root = path
          .parent()
          .map(Path::to_path_buf)
          .unwrap_or_else(default_workspace_root);
        self.working_directory.current = Some(self.workspace_root.clone());
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
    ffi_debug_log(format!(
      "configure_surface surface_px=({}, {}) cell_px=({}, {}) viewport=({}, {}) content_width={} changed={}",
      self.surface.width_px,
      self.surface.height_px,
      self.surface.metrics.cell_width_px,
      self.surface.metrics.cell_height_px,
      viewport.width,
      viewport.height,
      self.content_viewport_width(),
      changed,
    ));
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

  fn render_styles(&self) -> RenderStyles {
    RenderStyles {
      selection:                  Style::default().bg(Color::Rgb(46, 89, 160)),
      cursor:                     Style::default().bg(Color::Rgb(224, 224, 224)).fg(Color::Rgb(24, 24, 24)),
      active_cursor:              Style::default().bg(Color::Rgb(255, 255, 255)).fg(Color::Rgb(24, 24, 24)),
      cursor_kind:                CursorKind::Bar,
      active_cursor_kind:         CursorKind::Block,
      non_block_cursor_uses_head: true,
      gutter:                     Style::default().fg(Color::Rgb(110, 110, 110)),
      gutter_active:              Style::default().fg(Color::Rgb(180, 180, 180)),
    }
  }

  fn clamp_scroll(&mut self) {
    let max_row = max_scroll_row(
      self.editor.document().text().len_lines(),
      self.editor.view().viewport.height as usize,
    );
    if self.editor.view().scroll.row > max_row {
      self.editor.view_mut().scroll.row = max_row;
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
    let styles = self.render_styles();
    let frame = the_default::frame_render_plan_with_styles(self, styles);
    let plan = frame.active_plan();
    if let Some(plan) = plan {
      ffi_debug_log(format!(
        "snapshot layout_viewport=({}, {}) view_viewport=({}, {}) plan_viewport=({}, {}) content_offset_x={} scroll=({}, {})",
        self.editor.layout_viewport().width,
        self.editor.layout_viewport().height,
        self.editor.view().viewport.width,
        self.editor.view().viewport.height,
        plan.viewport.width,
        plan.viewport.height,
        plan.content_offset_x,
        self.editor.view().scroll.row,
        self.editor.view().scroll.col,
      ));
    }
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
    let mut highlights = NoHighlights;
    let (document, cache) = self.editor.document_and_cache();
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
  }
  fn build_frame_render_plan(&mut self) -> FrameRenderPlan {
    FrameRenderPlan::from_active_plan(self.build_render_plan())
  }
  fn build_frame_render_plan_with_styles(&mut self, styles: RenderStyles) -> FrameRenderPlan {
    FrameRenderPlan::from_active_plan(self.build_render_plan_with_styles(styles))
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
  fn syntax_loader(&self) -> Option<&the_lib::syntax::Loader> { None }
  fn scrolloff(&self) -> usize { SWIFT_SCROLLOFF }
  fn ui_theme(&self) -> &Theme { &self.ui_theme }
  fn ui_theme_name(&self) -> &str { &self.ui_theme_name }
  fn available_theme_names(&self) -> Vec<String> { vec![self.ui_theme_name.clone()] }
  fn set_ui_theme(&mut self, _theme_name: &str) -> Result<(), String> { Err("theme switching is not implemented in the swift POC".to_string()) }
  fn set_ui_theme_preview(&mut self, _theme_name: &str) -> Result<(), String> { Err("theme preview is not implemented in the swift POC".to_string()) }
  fn clear_ui_theme_preview(&mut self) {}
  fn set_file_path(&mut self, path: Option<PathBuf>) { self.file_path = path.clone(); self.editor.set_active_file_path(path); }
  fn open_file(&mut self, path: &Path) -> std::io::Result<()> {
    let contents = fs::read_to_string(path)?;
    let _ = self.editor.replace_active_buffer(Rope::from_str(&contents), Some(path.to_path_buf()));
    self.file_path = Some(path.to_path_buf());
    self.editor.set_active_file_path(Some(path.to_path_buf()));
    self.editor.document_mut().set_display_name(display_name_for_path(path));
    Ok(())
  }
}

impl OwnedSnapshot {
  fn from_editor(editor: &SwiftEditor, plan: Option<&RenderPlan>) -> Self {
    let viewport = editor.editor.view().viewport;
    let scroll = editor.editor.view().scroll;
    let mode = mode_code(editor.mode);
    let document_line_count = editor.editor.document().text().len_lines() as u32;

    let mut snapshot = Self {
      info: the_editor_snapshot_info_t {
        surface_width_px: editor.surface.width_px,
        surface_height_px: editor.surface.height_px,
        surface_metrics: editor.surface.metrics,
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

    let Some(plan) = plan else {
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

fn max_scroll_row(line_count: usize, viewport_height: usize) -> usize {
  line_count.saturating_sub(viewport_height.max(1))
}

fn default_workspace_root() -> PathBuf {
  std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn display_name_for_path(path: &Path) -> String {
  path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_else(|| path.display().to_string())
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
pub unsafe extern "C" fn the_editor_handle_key(handle: *mut the_editor_handle_t, event: the_editor_key_event_t) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else { return false; };
  handle.editor.handle_key_event(event)
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
  Box::into_raw(Box::new(the_editor_snapshot_t { snapshot: handle.editor.build_snapshot() }))
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
