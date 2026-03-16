use std::{
  collections::VecDeque,
  num::NonZeroUsize,
  path::{
    Path,
    PathBuf,
  },
};

use ropey::Rope;
use tempfile::tempdir;
use the_default::{
  CommandEvent,
  CommandPaletteState,
  CommandPaletteStyle,
  CommandPromptState,
  CommandRegistry,
  CompletionMenuState,
  DefaultContext,
  DispatchRef,
  ExtensionStateStore,
  FilePickerState,
  FileTreeState,
  KeyBinding,
  KeyEvent,
  Keymaps,
  Mode,
  Motion,
  PendingInput,
  PickerBuilder,
  PickerItemSpec,
  PickerRoot,
  SearchPromptState,
  WorkingDirectoryState,
  effective_working_directory,
  open_file_picker,
  poll_scan_results,
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
    RenderPlan,
    UiState,
    graphics::Rect,
    text_annotations::TextAnnotations,
    text_format::TextFormat,
    theme::Theme,
  },
  syntax::Loader,
  view::ViewState,
};

struct TestCtx {
  editor:            Editor,
  messages:          MessageCenter,
  workspace_root:    PathBuf,
  working_directory: WorkingDirectoryState,
  file_tree:         FileTreeState,
  file_picker:       FilePickerState,
  extension_state:   ExtensionStateStore,
  opened_paths:      Vec<PathBuf>,
}

impl TestCtx {
  fn new(workspace_root: PathBuf) -> Self {
    let doc = Document::new(DocumentId::new(NonZeroUsize::new(1).unwrap()), Rope::new());
    let view = ViewState::new(Rect::new(0, 0, 80, 24), Position::new(0, 0));
    let editor = Editor::new(EditorId::new(NonZeroUsize::new(1).unwrap()), doc, view);
    Self {
      editor,
      messages: MessageCenter::default(),
      working_directory: WorkingDirectoryState {
        current:  Some(workspace_root.clone()),
        previous: None,
      },
      workspace_root,
      file_tree: FileTreeState::with_working_directory(workspace_root.clone()),
      file_picker: FilePickerState::default(),
      extension_state: ExtensionStateStore::default(),
      opened_paths: Vec::new(),
    }
  }
}

impl DefaultContext for TestCtx {
  fn editor(&mut self) -> &mut Editor {
    &mut self.editor
  }

  fn editor_ref(&self) -> &Editor {
    &self.editor
  }

  fn file_path(&self) -> Option<&Path> {
    None
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

  fn messages(&self) -> &MessageCenter {
    &self.messages
  }

  fn messages_mut(&mut self) -> &mut MessageCenter {
    &mut self.messages
  }

  fn build_render_plan(&mut self) -> RenderPlan {
    todo!()
  }

  fn build_frame_render_plan(&mut self) -> FrameRenderPlan {
    todo!()
  }

  fn request_quit(&mut self) {}

  fn mode(&self) -> Mode {
    Mode::Normal
  }

  fn set_mode(&mut self, _mode: Mode) {}

  fn keymaps(&mut self) -> &mut Keymaps {
    todo!()
  }

  fn extension_states(&self) -> &ExtensionStateStore {
    &self.extension_state
  }

  fn extension_states_mut(&mut self) -> &mut ExtensionStateStore {
    &mut self.extension_state
  }

  fn command_prompt_mut(&mut self) -> &mut CommandPromptState {
    todo!()
  }

  fn command_prompt_ref(&self) -> &CommandPromptState {
    todo!()
  }

  fn command_registry_mut(&mut self) -> &mut CommandRegistry<Self> {
    todo!()
  }

  fn command_registry_ref(&self) -> &CommandRegistry<Self> {
    todo!()
  }

  fn command_palette(&self) -> &CommandPaletteState {
    todo!()
  }

  fn command_palette_mut(&mut self) -> &mut CommandPaletteState {
    todo!()
  }

  fn command_palette_style(&self) -> &CommandPaletteStyle {
    todo!()
  }

  fn command_palette_style_mut(&mut self) -> &mut CommandPaletteStyle {
    todo!()
  }

  fn completion_menu(&self) -> &CompletionMenuState {
    todo!()
  }

  fn completion_menu_mut(&mut self) -> &mut CompletionMenuState {
    todo!()
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

  fn search_prompt_ref(&self) -> &SearchPromptState {
    todo!()
  }

  fn search_prompt_mut(&mut self) -> &mut SearchPromptState {
    todo!()
  }

  fn ui_state(&self) -> &UiState {
    todo!()
  }

  fn ui_state_mut(&mut self) -> &mut UiState {
    todo!()
  }

  fn dispatch(&self) -> DispatchRef<Self> {
    todo!()
  }

  fn pending_input(&self) -> Option<&PendingInput> {
    None
  }

  fn set_pending_input(&mut self, _pending: Option<PendingInput>) {}

  fn registers(&self) -> &Registers {
    todo!()
  }

  fn registers_mut(&mut self) -> &mut Registers {
    todo!()
  }

  fn register(&self) -> Option<char> {
    None
  }

  fn set_register(&mut self, _register: Option<char>) {}

  fn macro_recording(&self) -> &Option<(char, Vec<KeyBinding>)> {
    todo!()
  }

  fn set_macro_recording(&mut self, _recording: Option<(char, Vec<KeyBinding>)>) {}

  fn macro_replaying(&self) -> &Vec<char> {
    todo!()
  }

  fn macro_replaying_mut(&mut self) -> &mut Vec<char> {
    todo!()
  }

  fn macro_queue(&self) -> &VecDeque<KeyEvent> {
    todo!()
  }

  fn macro_queue_mut(&mut self) -> &mut VecDeque<KeyEvent> {
    todo!()
  }

  fn last_motion(&self) -> Option<Motion> {
    None
  }

  fn set_last_motion(&mut self, _motion: Option<Motion>) {}

  fn text_format(&self) -> TextFormat {
    TextFormat::default()
  }

  fn soft_wrap_enabled(&self) -> bool {
    false
  }

  fn set_soft_wrap_enabled(&mut self, _enabled: bool) {}

  fn gutter_config(&self) -> &GutterConfig {
    todo!()
  }

  fn gutter_config_mut(&mut self) -> &mut GutterConfig {
    todo!()
  }

  fn text_annotations(&self) -> TextAnnotations<'_> {
    todo!()
  }

  fn syntax_loader(&self) -> Option<&Loader> {
    None
  }

  fn ui_theme(&self) -> &Theme {
    todo!()
  }

  fn ui_theme_name(&self) -> &str {
    "test"
  }

  fn available_theme_names(&self) -> Vec<String> {
    vec!["test".to_string()]
  }

  fn set_ui_theme(&mut self, _theme_name: &str) -> Result<(), String> {
    Ok(())
  }

  fn set_ui_theme_preview(&mut self, _theme_name: &str) -> Result<(), String> {
    Ok(())
  }

  fn clear_ui_theme_preview(&mut self) {}

  fn set_file_path(&mut self, _path: Option<PathBuf>) {}

  fn open_file(&mut self, path: &Path) -> std::io::Result<()> {
    self.opened_paths.push(path.to_path_buf());
    Ok(())
  }
}

#[test]
fn change_current_directory_is_isolated_per_context() {
  let workspace_a = tempdir().unwrap();
  let workspace_b = tempdir().unwrap();
  let nested_a = workspace_a.path().join("nested");
  std::fs::create_dir_all(&nested_a).unwrap();
  let nested_a = std::fs::canonicalize(&nested_a).unwrap();

  let mut ctx_a = TestCtx::new(workspace_a.path().to_path_buf());
  let ctx_b = TestCtx::new(workspace_b.path().to_path_buf());
  let mut registry = CommandRegistry::<TestCtx>::new();
  the_default::install_builtin_commands(&mut registry);

  assert_eq!(effective_working_directory(&ctx_a), workspace_a.path());
  assert_eq!(effective_working_directory(&ctx_b), workspace_b.path());

  registry
    .execute(
      &mut ctx_a,
      "cd",
      nested_a.to_string_lossy().as_ref(),
      CommandEvent::Validate,
    )
    .unwrap();

  open_file_picker(&mut ctx_a);
  assert_eq!(ctx_a.file_picker.root, nested_a);

  registry
    .execute(&mut ctx_a, "open", "file.txt", CommandEvent::Validate)
    .unwrap();

  assert_eq!(effective_working_directory(&ctx_a), nested_a);
  assert_eq!(ctx_a.opened_paths.last(), Some(&nested_a.join("file.txt")));
  assert_eq!(effective_working_directory(&ctx_b), workspace_b.path());
}

#[derive(Debug, PartialEq, Eq)]
struct TestExtensionState {
  counter: usize,
}

#[test]
fn default_context_extension_state_helpers_round_trip_typed_state() {
  let workspace = tempdir().unwrap();
  let mut ctx = TestCtx::new(workspace.path().to_path_buf());

  assert!(ctx.extension_state::<TestExtensionState>().is_none());

  let state = ctx.extension_state_or_insert_with(|| TestExtensionState { counter: 4 });
  assert_eq!(state.counter, 4);
  state.counter += 3;

  assert_eq!(
    ctx.extension_state::<TestExtensionState>(),
    Some(&TestExtensionState { counter: 7 })
  );
  assert_eq!(
    ctx.remove_extension_state::<TestExtensionState>(),
    Some(TestExtensionState { counter: 7 })
  );
  assert!(ctx.extension_state::<TestExtensionState>().is_none());
}

#[test]
fn dynamic_picker_builder_populates_items_from_query_callback() {
  let workspace = tempdir().unwrap();
  let mut ctx = TestCtx::new(workspace.path().to_path_buf());

  let picker = PickerBuilder::<TestCtx>::dynamic("Demo", |_ctx, query| {
    vec![PickerItemSpec::custom(format!("match:{query}"))]
  })
  .initial_query("rust");

  picker.open(&mut ctx);

  assert!(ctx.file_picker.active);
  assert_eq!(ctx.file_picker.matched_count(), 1);
  let item = ctx.file_picker.current_item().expect("picker item");
  assert_eq!(item.display, "match:rust");
}

#[test]
fn file_picker_builder_reuses_scan_pipeline_with_extension_filter() {
  let workspace = tempdir().unwrap();
  std::fs::write(workspace.path().join("main.rs"), "fn main() {}\n").unwrap();
  std::fs::write(workspace.path().join("main.py"), "print('hi')\n").unwrap();

  let mut ctx = TestCtx::new(workspace.path().to_path_buf());
  let picker = PickerBuilder::<TestCtx>::files("Rust Files")
    .root(PickerRoot::Fixed(workspace.path().to_path_buf()))
    .extension("rs");

  picker.open(&mut ctx);
  let _ = poll_scan_results(ctx.file_picker_mut());

  assert!(ctx.file_picker.active);
  assert_eq!(ctx.file_picker.matched_count(), 1);
  let item = ctx.file_picker.current_item().expect("scanned item");
  assert_eq!(item.display, "main.rs");
}
