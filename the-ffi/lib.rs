use std::{
  collections::VecDeque,
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
  sync::mpsc,
};

use ropey::Rope;
use serde::Serialize;
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
    GutterConfig,
    NoHighlights,
    RenderPlan,
    RenderStyles,
    build_plan,
    graphics::{
      Color,
      CursorKind,
      Rect,
      Style,
    },
    text_annotations::TextAnnotations,
    text_format::TextFormat,
    theme::{
      Theme,
      default_theme,
    },
  },
  view::ViewState,
};

#[repr(C)]
pub struct the_editor_handle_t {
  editor: SwiftEditor,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct the_editor_key_event_t {
  pub kind:      u32,
  pub codepoint: u32,
  pub modifiers: u8,
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
  gutter_config:                 GutterConfig,
  ui_theme_name:                 String,
  ui_theme:                      Theme,
}

impl SwiftEditor {
  fn new(path: Option<&Path>) -> Self {
    let workspace_root = path
      .and_then(Path::parent)
      .map(Path::to_path_buf)
      .unwrap_or_else(default_workspace_root);

    let mut document = Document::new(
      DocumentId::new(NonZeroUsize::new(1).expect("nonzero")),
      read_rope(path),
    );
    if let Some(path) = path {
      document.set_display_name(display_name_for_path(path));
    }

    let viewport = Rect::new(0, 0, 80, 24);
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
      gutter_config: GutterConfig::default(),
      ui_theme_name: default_theme().name().to_string(),
      ui_theme: default_theme().clone(),
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

  fn set_viewport(&mut self, cols: u16, rows: u16) {
    let cols = cols.max(1);
    let rows = rows.max(1);
    self.editor.view_mut().viewport = Rect::new(0, 0, cols, rows);
    self.text_format.viewport_width = cols;
    self.clamp_scroll();
  }

  fn handle_key_event(&mut self, raw: the_editor_key_event_t) -> bool {
    let Some(event) = translate_key_event(raw) else {
      return false;
    };
    handle_key(self, event);
    true
  }

  fn scroll_lines(&mut self, delta_lines: i32) -> bool {
    if delta_lines == 0 {
      return false;
    }

    let current = self.editor.view().scroll.row as i32;
    let max_row = max_scroll_row(
      self.editor.document().text().len_lines(),
      self.editor.view().viewport.height as usize,
    ) as i32;
    let next = (current + delta_lines).clamp(0, max_row) as usize;
    if next == self.editor.view().scroll.row {
      return false;
    }

    self.editor.view_mut().scroll.row = next;
    true
  }

  fn snapshot_json(&mut self) -> String {
    let frame = the_default::frame_render_plan(self);
    let active_plan = frame.active_plan();
    let snapshot = EditorSnapshot::from_editor(self, active_plan);
    serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string())
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
}

impl DefaultContext for SwiftEditor {
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
    self.workspace_root.clone()
  }

  fn working_directory_state(&self) -> &WorkingDirectoryState {
    &self.working_directory
  }

  fn working_directory_state_mut(&mut self) -> &mut WorkingDirectoryState {
    &mut self.working_directory
  }

  fn request_render(&mut self) {}

  fn render_waker(&self) -> the_default::RenderWaker {
    let (tx, _rx) = mpsc::channel();
    the_default::RenderWaker::new(tx)
  }

  fn messages(&self) -> &MessageCenter {
    &self.messages
  }

  fn messages_mut(&mut self) -> &mut MessageCenter {
    &mut self.messages
  }

  fn build_render_plan(&mut self) -> RenderPlan {
    let view = self.editor.view();
    let styles = self.render_styles();
    let mut annotations = TextAnnotations::default();
    let mut highlights = NoHighlights;
    let (document, cache) = self.editor.document_and_cache();
    build_plan(
      document,
      view,
      &self.text_format,
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

  fn request_quit(&mut self) {}

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

  fn completion_menu(&self) -> &CompletionMenuState {
    &self.completion_menu
  }

  fn completion_menu_mut(&mut self) -> &mut CompletionMenuState {
    &mut self.completion_menu
  }

  fn completion_menu_keymaps(&self) -> &Keymaps {
    &self.completion_menu_keymaps
  }

  fn completion_menu_keymaps_mut(&mut self) -> &mut Keymaps {
    &mut self.completion_menu_keymaps
  }

  fn inline_completion(&self) -> &the_default::InlineCompletionState {
    &self.inline_completion
  }

  fn inline_completion_mut(&mut self) -> &mut the_default::InlineCompletionState {
    &mut self.inline_completion
  }

  fn set_inline_completion_annotations(&mut self, annotations: the_default::OwnedTextAnnotations) {
    self.inline_completion_annotations = annotations;
  }

  fn clear_inline_completion_annotations(&mut self) {
    self.inline_completion_annotations = the_default::OwnedTextAnnotations::default();
  }

  fn file_tree(&self) -> &FileTreeState {
    &self.file_tree
  }

  fn file_tree_mut(&mut self) -> &mut FileTreeState {
    &mut self.file_tree
  }

  fn file_picker(&self) -> &FilePickerState {
    &self.file_picker
  }

  fn file_picker_mut(&mut self) -> &mut FilePickerState {
    &mut self.file_picker
  }

  fn picker_runtime_store(&self) -> &PickerRuntimeStore<Self> {
    &self.picker_runtime_store
  }

  fn picker_runtime_store_mut(&mut self) -> &mut PickerRuntimeStore<Self> {
    &mut self.picker_runtime_store
  }

  fn search_prompt_ref(&self) -> &SearchPromptState {
    &self.search_prompt
  }

  fn search_prompt_mut(&mut self) -> &mut SearchPromptState {
    &mut self.search_prompt
  }

  fn dispatch(&self) -> DispatchRef<Self> {
    DispatchRef::from_ptr(self.dispatch.as_ref() as *const dyn DefaultApi<Self>)
  }

  fn pending_input(&self) -> Option<&PendingInput> {
    self.pending_input.as_ref()
  }

  fn set_pending_input(&mut self, pending: Option<PendingInput>) {
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
    self.soft_wrap_enabled
  }

  fn set_soft_wrap_enabled(&mut self, enabled: bool) {
    self.soft_wrap_enabled = enabled;
    self.text_format.soft_wrap = enabled;
  }

  fn gutter_config(&self) -> &GutterConfig {
    &self.gutter_config
  }

  fn gutter_config_mut(&mut self) -> &mut GutterConfig {
    &mut self.gutter_config
  }

  fn text_annotations(&self) -> TextAnnotations<'_> {
    TextAnnotations::default()
  }

  fn syntax_loader(&self) -> Option<&the_lib::syntax::Loader> {
    None
  }

  fn ui_theme(&self) -> &Theme {
    &self.ui_theme
  }

  fn ui_theme_name(&self) -> &str {
    &self.ui_theme_name
  }

  fn available_theme_names(&self) -> Vec<String> {
    vec![self.ui_theme_name.clone()]
  }

  fn set_ui_theme(&mut self, _theme_name: &str) -> Result<(), String> {
    Err("theme switching is not implemented in the swift POC".to_string())
  }

  fn set_ui_theme_preview(&mut self, _theme_name: &str) -> Result<(), String> {
    Err("theme preview is not implemented in the swift POC".to_string())
  }

  fn clear_ui_theme_preview(&mut self) {}

  fn set_file_path(&mut self, path: Option<PathBuf>) {
    self.file_path = path.clone();
    self.editor.set_active_file_path(path);
  }

  fn open_file(&mut self, path: &Path) -> std::io::Result<()> {
    match fs::read_to_string(path) {
      Ok(contents) => {
        let _ = self
          .editor
          .replace_active_buffer(Rope::from_str(&contents), Some(path.to_path_buf()));
        self.file_path = Some(path.to_path_buf());
        self.editor.set_active_file_path(Some(path.to_path_buf()));
        self.editor
          .document_mut()
          .set_display_name(display_name_for_path(path));
        Ok(())
      },
      Err(err) => Err(err),
    }
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorSnapshot {
  display_name:    String,
  file_path:       Option<String>,
  mode:            String,
  viewport_width:  u16,
  viewport_height: u16,
  scroll_row:      usize,
  scroll_col:      usize,
  content_offset_x: u16,
  line_count:      usize,
  message:         Option<String>,
  damage_reason:   String,
  lines:           Vec<EditorSnapshotLine>,
  cursors:         Vec<EditorSnapshotCursor>,
  selections:      Vec<EditorSnapshotSelection>,
}

impl EditorSnapshot {
  fn from_editor(editor: &SwiftEditor, plan: Option<&RenderPlan>) -> Self {
    let viewport = editor.editor.view().viewport;
    let scroll = editor.editor.view().scroll;
    let line_count = editor.editor.document().text().len_lines();
    let message = editor.messages.active().map(|message| message.text.clone());

    if let Some(plan) = plan {
      let lines = (0..plan.viewport.height)
        .map(|row| {
          let gutter = plan
            .gutter_lines
            .iter()
            .find(|line| line.row == row)
            .map(concat_gutter)
            .unwrap_or_default();
          let spans = plan
            .lines
            .iter()
            .find(|line| line.row == row)
            .map(snapshot_spans)
            .unwrap_or_default();
          let doc_line = plan
            .visible_rows
            .iter()
            .find(|visible| visible.row == row)
            .map(|visible| visible.doc_line);
          EditorSnapshotLine {
            row,
            doc_line,
            gutter,
            spans,
          }
        })
        .collect();
      let cursors = plan
        .cursors
        .iter()
        .map(|cursor| EditorSnapshotCursor {
          row: cursor.pos.row,
          col: cursor.pos.col,
          kind: cursor_kind_name(cursor.kind).to_string(),
        })
        .collect();
      let selections = plan
        .selections
        .iter()
        .map(|selection| EditorSnapshotSelection {
          x:      selection.rect.x,
          y:      selection.rect.y,
          width:  selection.rect.width,
          height: selection.rect.height,
        })
        .collect();

      Self {
        display_name: editor.editor.document().display_name().into_owned(),
        file_path: editor.file_path.as_ref().map(|path| path.display().to_string()),
        mode: mode_name(editor.mode).to_string(),
        viewport_width: viewport.width,
        viewport_height: viewport.height,
        scroll_row: scroll.row,
        scroll_col: scroll.col,
        content_offset_x: plan.content_offset_x,
        line_count,
        message,
        damage_reason: damage_reason_name(plan.damage_reason).to_string(),
        lines,
        cursors,
        selections,
      }
    } else {
      Self {
        display_name: editor.editor.document().display_name().into_owned(),
        file_path: editor.file_path.as_ref().map(|path| path.display().to_string()),
        mode: mode_name(editor.mode).to_string(),
        viewport_width: viewport.width,
        viewport_height: viewport.height,
        scroll_row: scroll.row,
        scroll_col: scroll.col,
        content_offset_x: 0,
        line_count,
        message,
        damage_reason: "none".to_string(),
        lines: Vec::new(),
        cursors: Vec::new(),
        selections: Vec::new(),
      }
    }
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorSnapshotLine {
  row:      u16,
  doc_line: Option<usize>,
  gutter:   String,
  spans:    Vec<EditorSnapshotSpan>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorSnapshotSpan {
  col:        u16,
  cols:       u16,
  text:       String,
  is_virtual: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorSnapshotCursor {
  row:  usize,
  col:  usize,
  kind: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorSnapshotSelection {
  x:      u16,
  y:      u16,
  width:  u16,
  height: u16,
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
  if (raw & MOD_CTRL) != 0 {
    modifiers.insert(the_default::Modifiers::CTRL);
  }
  if (raw & MOD_ALT) != 0 {
    modifiers.insert(the_default::Modifiers::ALT);
  }
  if (raw & MOD_SHIFT) != 0 {
    modifiers.insert(the_default::Modifiers::SHIFT);
  }
  modifiers
}

fn max_scroll_row(line_count: usize, viewport_height: usize) -> usize {
  line_count.saturating_sub(viewport_height.max(1))
}

fn snapshot_spans(line: &the_lib::render::RenderLine) -> Vec<EditorSnapshotSpan> {
  line
    .spans
    .iter()
    .map(|span| EditorSnapshotSpan {
      col: span.col,
      cols: span.cols,
      text: span.text.to_string(),
      is_virtual: span.is_virtual,
    })
    .collect()
}

fn concat_gutter(line: &the_lib::render::RenderGutterLine) -> String {
  let mut text = String::new();
  for span in &line.spans {
    text.push_str(span.text.as_str());
  }
  text
}

fn cursor_kind_name(kind: CursorKind) -> &'static str {
  match kind {
    CursorKind::Block => "block",
    CursorKind::Bar => "bar",
    CursorKind::Underline => "underline",
    CursorKind::Hollow => "hollow",
    CursorKind::Hidden => "hidden",
  }
}

fn damage_reason_name(reason: the_lib::render::RenderDamageReason) -> &'static str {
  match reason {
    the_lib::render::RenderDamageReason::None => "none",
    the_lib::render::RenderDamageReason::Full => "full",
    the_lib::render::RenderDamageReason::Layout => "layout",
    the_lib::render::RenderDamageReason::Text => "text",
    the_lib::render::RenderDamageReason::Decoration => "decoration",
    the_lib::render::RenderDamageReason::Cursor => "cursor",
    the_lib::render::RenderDamageReason::Scroll => "scroll",
    the_lib::render::RenderDamageReason::Theme => "theme",
    the_lib::render::RenderDamageReason::PaneStructure => "paneStructure",
  }
}

fn mode_name(mode: Mode) -> &'static str {
  match mode {
    Mode::Normal => "normal",
    Mode::Insert => "insert",
    Mode::Select => "select",
    Mode::Command => "command",
  }
}

fn default_workspace_root() -> PathBuf {
  std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn display_name_for_path(path: &Path) -> String {
  path
    .file_name()
    .map(|name| name.to_string_lossy().to_string())
    .unwrap_or_else(|| path.display().to_string())
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
  if handle.is_null() {
    return;
  }
  drop(unsafe { Box::from_raw(handle) });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_open(
  handle: *mut the_editor_handle_t,
  path: *const c_char,
) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else {
    return false;
  };
  let Some(path) = (unsafe { path_from_c(path) }) else {
    return false;
  };
  handle.editor.open_path(&path)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_set_viewport(
  handle: *mut the_editor_handle_t,
  cols: u16,
  rows: u16,
) {
  let Some(handle) = (unsafe { handle.as_mut() }) else {
    return;
  };
  handle.editor.set_viewport(cols, rows);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_handle_key(
  handle: *mut the_editor_handle_t,
  event: the_editor_key_event_t,
) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else {
    return false;
  };
  handle.editor.handle_key_event(event)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_scroll_lines(
  handle: *mut the_editor_handle_t,
  delta_lines: i32,
) -> bool {
  let Some(handle) = (unsafe { handle.as_mut() }) else {
    return false;
  };
  handle.editor.scroll_lines(delta_lines)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_snapshot_json(
  handle: *mut the_editor_handle_t,
) -> *mut c_char {
  let Some(handle) = (unsafe { handle.as_mut() }) else {
    return std::ptr::null_mut();
  };
  match CString::new(handle.editor.snapshot_json()) {
    Ok(value) => value.into_raw(),
    Err(_) => std::ptr::null_mut(),
  }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn the_editor_string_free(value: *mut c_char) {
  if value.is_null() {
    return;
  }
  drop(unsafe { CString::from_raw(value) });
}
