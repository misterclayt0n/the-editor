#![allow(unused_imports)]

use std::{
  borrow::Cow,
  collections::{
    BTreeSet,
    HashMap,
    VecDeque,
  },
  path::{
    Path,
    PathBuf,
  },
  sync::OnceLock,
};

use ropey::{
  Rope,
  RopeSlice,
};
use smallvec::SmallVec;
use the_core::{
  chars::{
    byte_to_char_idx,
    char_is_word,
  },
  grapheme::{
    next_grapheme_boundary,
    nth_next_grapheme_boundary,
    nth_prev_grapheme_boundary,
    prev_grapheme_boundary,
  },
  line_ending::{
    get_line_ending_of_str,
    line_end_char_index,
  },
};
use the_dispatch::define;
use the_lib::{
  Tendril,
  auto_pairs::{
    self,
    AutoPairs,
  },
  diff::compare_ropes,
  editor::Editor,
  history::{
    HistoryJump,
    UndoKind,
  },
  indent,
  match_brackets as mb,
  messages::{
    Message,
    MessageCenter,
    MessageLevel,
  },
  movement::{
    self,
    Direction as MoveDir,
    Movement,
    move_horizontally,
    move_vertically,
    move_vertically_visual,
  },
  object,
  position::{
    Position,
    char_idx_at_coords,
    coords_at_pos,
  },
  registers::Registers,
  render::{
    FrameRenderPlan,
    GutterConfig,
    GutterType,
    LineNumberMode,
    RenderPlan,
    RenderStyles,
    UiEvent,
    UiEventKind,
    UiEventOutcome,
    UiFocus,
    UiFocusKind,
    UiKey,
    UiKeyEvent,
    UiModifiers,
    UiState,
    UiTree,
    char_at_visual_pos,
    text_annotations::{
      InlineAnnotation,
      Overlay,
      TextAnnotations,
    },
    text_format::TextFormat,
    theme::Theme,
    ui_theme::resolve_ui_tree,
    visual_pos_at_char,
  },
  search::{
    build_regex,
    search_regex,
  },
  selection::{
    CursorId,
    CursorPick,
    Range,
    Selection,
    split_on_newline,
  },
  split_tree::{
    PaneDirection,
    SplitAxis,
  },
  surround,
  syntax::{
    Loader,
    config::{
      Configuration,
      IndentationHeuristic,
    },
    resources::NullResources,
  },
  text_object,
  transaction::Transaction,
  view::ViewState,
};
use the_stdx::rope::RopeSliceExt;

use crate::{
  Command,
  Direction,
  Key,
  KeyBinding,
  KeyEvent,
  KeyOutcome,
  Modifiers,
  Motion,
  PendingInput,
  WordMotion,
  command_palette::{
    CommandPaletteState,
    CommandPaletteStyle,
  },
  command_registry::{
    CommandEvent,
    CommandPromptState,
    CommandRegistry,
    handle_command_prompt_key,
  },
  completion_menu::CompletionMenuState,
  keymap::{
    Keymaps,
    Mode,
    ParseKeyBindingError,
    handle_key as keymap_handle_key,
  },
  message_bar::MessagePresentation,
  pending::WordJumpTarget,
  signature_help::SignatureHelpState,
};

define! {
  Default {
    pre_on_keypress: KeyEvent => (),
    on_keypress: KeyEvent => (),
    post_on_keypress: Command => (),
    pre_on_action: Command => (),
    on_action: Command => (),
    post_on_action: () => (),
    render_request: () => (),
    pre_render: () => (),
    on_render: () => RenderPlan,
    post_render: RenderPlan => RenderPlan,
    pre_render_with_styles: RenderStyles => RenderStyles,
    on_render_with_styles: RenderStyles => RenderPlan,
    pre_ui: () => (),
    on_ui: () => UiTree,
    post_ui: UiTree => UiTree,
    pre_ui_event: UiEvent => UiEventOutcome,
    on_ui_event: UiEvent => UiEventOutcome,
    post_ui_event: UiEventOutcome => UiEventOutcome,

    insert_char: char,
    insert_newline: (),
    delete_char: (),
    delete_char_forward: usize,
    delete_word_backward: usize,
    delete_word_forward: usize,
    kill_to_line_start: (),
    kill_to_line_end: (),
    insert_tab: (),
    goto_line_start: bool,
    goto_first_nonwhitespace: bool,
    goto_line_end: bool,
    page_up: bool,
    page_down: bool,
    move_cursor: Direction,
    add_cursor: Direction,
    motion: Motion,
    delete_selection: bool,
    change_selection: bool,
    replace_selection: char,
    replace_with_yanked: (),
    yank: (),
    paste: bool,
    switch_case: (),
    save: (),
    quit: (),
    find_char: (Direction, bool, bool),
    search: (),
    rsearch: (),
    select_regex: (),
    search_next_or_prev: (Direction, bool, usize),
    parent_node_end: bool,
    parent_node_start: bool,
    repeat_last_motion: (),
    switch_to_uppercase: (),
    switch_to_lowercase: (),
    insert_at_line_start: (),
    insert_at_line_end: (),
    append_mode: (),
    open_below: (),
    open_above: (),
    commit_undo_checkpoint: (),
    copy_selection_on_next_line: (),
    copy_selection_on_prev_line: (),
    select_all: (),
    extend_line_below: usize,
    extend_line_above: usize,
    extend_to_line_bounds: (),
    shrink_to_line_bounds: (),
    undo: usize,
    redo: usize,
    earlier: usize,
    later: usize,
    indent: usize,
    unindent: usize,
    replace: (),
    record_macro: (),
    replay_macro: (),
    match_brackets: (),
    surround_add: (),
    surround_delete: usize,
    surround_replace: usize,
    select_textobject_around: (),
    select_textobject_inner: (),
  }
}

pub type DefaultDispatchStatic<Ctx> = DefaultDispatch<
  Ctx,
  fn(&mut Ctx, KeyEvent),
  fn(&mut Ctx, KeyEvent),
  fn(&mut Ctx, Command),
  fn(&mut Ctx, Command),
  fn(&mut Ctx, Command),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()) -> RenderPlan,
  fn(&mut Ctx, RenderPlan) -> RenderPlan,
  fn(&mut Ctx, RenderStyles) -> RenderStyles,
  fn(&mut Ctx, RenderStyles) -> RenderPlan,
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()) -> UiTree,
  fn(&mut Ctx, UiTree) -> UiTree,
  fn(&mut Ctx, UiEvent) -> UiEventOutcome,
  fn(&mut Ctx, UiEvent) -> UiEventOutcome,
  fn(&mut Ctx, UiEventOutcome) -> UiEventOutcome,
  fn(&mut Ctx, char),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, Direction),
  fn(&mut Ctx, Direction),
  fn(&mut Ctx, Motion),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, char),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, (Direction, bool, bool)),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, (Direction, bool, usize)),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, bool),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, usize),
  fn(&mut Ctx, ()),
  fn(&mut Ctx, ()),
>;

#[derive(Clone, Copy)]
pub struct DispatchRef<Ctx> {
  ptr: *const DefaultDispatchStatic<Ctx>,
}

impl<Ctx> DispatchRef<Ctx> {
  pub fn from_ptr(ptr: *const DefaultDispatchStatic<Ctx>) -> Self {
    Self { ptr }
  }
}

impl<Ctx> std::ops::Deref for DispatchRef<Ctx> {
  type Target = DefaultDispatchStatic<Ctx>;

  fn deref(&self) -> &Self::Target {
    // Safety: DispatchRef is constructed from a valid dispatch pointer.
    unsafe { &*self.ptr }
  }
}

pub trait DefaultContext: Sized + 'static {
  fn editor(&mut self) -> &mut Editor;
  fn editor_ref(&self) -> &Editor;
  fn file_path(&self) -> Option<&Path>;
  fn request_render(&mut self);
  fn messages(&self) -> &MessageCenter;
  fn messages_mut(&mut self) -> &mut MessageCenter;
  fn push_message(
    &mut self,
    level: MessageLevel,
    source: impl Into<String>,
    text: impl Into<String>,
  ) -> Message {
    let message = self
      .messages_mut()
      .publish(level, Some(source.into()), text.into());
    self.request_render();
    message
  }
  fn push_info(&mut self, source: impl Into<String>, text: impl Into<String>) -> Message {
    self.push_message(MessageLevel::Info, source, text)
  }
  fn push_warning(&mut self, source: impl Into<String>, text: impl Into<String>) -> Message {
    self.push_message(MessageLevel::Warning, source, text)
  }
  fn push_error(&mut self, source: impl Into<String>, text: impl Into<String>) -> Message {
    self.push_message(MessageLevel::Error, source, text)
  }
  fn dismiss_active_message(&mut self) -> Option<Message> {
    let message = self.messages_mut().dismiss_active();
    if message.is_some() {
      self.request_render();
    }
    message
  }
  fn clear_messages(&mut self) {
    self.messages_mut().clear();
    self.request_render();
  }
  fn message_presentation(&self) -> MessagePresentation {
    MessagePresentation::InlineStatusline
  }
  fn lsp_statusline_text(&self) -> Option<String> {
    None
  }
  fn vcs_statusline_text(&self) -> Option<String> {
    None
  }
  fn watch_statusline_text(&self) -> Option<String> {
    None
  }
  fn watch_conflict_active(&self) -> bool {
    false
  }
  fn clear_watch_conflict(&mut self) {}
  fn watch_scope_name(&self) -> &'static str {
    "active-document"
  }
  fn apply_transaction(&mut self, transaction: &Transaction) -> bool {
    let changed = !transaction.changes().is_empty();
    let loader_ptr = self.syntax_loader().map(|loader| loader as *const Loader);
    let applied = {
      let doc = self.editor().document_mut();
      let loader = loader_ptr.map(|ptr| unsafe { &*ptr });
      doc
        .apply_transaction_with_syntax(transaction, loader)
        .is_ok()
    };
    if applied && changed {
      self.editor().mark_active_buffer_modified();
    }
    applied
  }
  fn build_render_plan(&mut self) -> RenderPlan;
  fn build_render_plan_with_styles(&mut self, styles: RenderStyles) -> RenderPlan {
    let _ = styles;
    self.build_render_plan()
  }
  fn build_frame_render_plan(&mut self) -> FrameRenderPlan {
    FrameRenderPlan::from_active_plan(self.build_render_plan())
  }
  fn build_frame_render_plan_with_styles(&mut self, styles: RenderStyles) -> FrameRenderPlan {
    FrameRenderPlan::from_active_plan(self.build_render_plan_with_styles(styles))
  }
  fn request_quit(&mut self);
  fn mode(&self) -> Mode;
  fn set_mode(&mut self, mode: Mode);
  fn keymaps(&mut self) -> &mut Keymaps;
  fn command_prompt_mut(&mut self) -> &mut CommandPromptState;
  fn command_prompt_ref(&self) -> &CommandPromptState;
  fn command_registry_mut(&mut self) -> &mut CommandRegistry<Self>;
  fn command_registry_ref(&self) -> &CommandRegistry<Self>;
  fn command_palette(&self) -> &CommandPaletteState;
  fn command_palette_mut(&mut self) -> &mut CommandPaletteState;
  fn command_palette_style(&self) -> &CommandPaletteStyle;
  fn command_palette_style_mut(&mut self) -> &mut CommandPaletteStyle;
  fn completion_menu(&self) -> &CompletionMenuState;
  fn completion_menu_mut(&mut self) -> &mut CompletionMenuState;
  fn completion_selection_changed(&mut self, _index: usize) {}
  fn completion_accept_selected(&mut self, _index: usize) -> bool {
    false
  }
  fn completion_accept_on_commit_char(&mut self, _ch: char) -> bool {
    false
  }
  fn completion_on_action(&mut self, _command: Command) -> bool {
    false
  }
  fn signature_help(&self) -> Option<&SignatureHelpState> {
    None
  }
  fn signature_help_mut(&mut self) -> Option<&mut SignatureHelpState> {
    None
  }
  fn file_picker(&self) -> &crate::file_picker::FilePickerState;
  fn file_picker_mut(&mut self) -> &mut crate::file_picker::FilePickerState;
  fn search_prompt_ref(&self) -> &crate::SearchPromptState;
  fn search_prompt_mut(&mut self) -> &mut crate::SearchPromptState;
  fn ui_state(&self) -> &UiState;
  fn ui_state_mut(&mut self) -> &mut UiState;
  fn dispatch(&self) -> DispatchRef<Self>;
  fn pending_input(&self) -> Option<&PendingInput>;
  fn set_pending_input(&mut self, pending: Option<PendingInput>);
  fn set_word_jump_annotations(&mut self, _inline: Vec<InlineAnnotation>, _overlay: Vec<Overlay>) {}
  fn clear_word_jump_annotations(&mut self) {}
  fn active_diagnostic_ranges(&self) -> Vec<Range> {
    Vec::new()
  }
  fn change_hunk_ranges(&self) -> Option<Vec<Range>> {
    None
  }
  fn registers(&self) -> &Registers;
  fn registers_mut(&mut self) -> &mut Registers;
  fn register(&self) -> Option<char>;
  fn set_register(&mut self, register: Option<char>);
  fn macro_recording(&self) -> &Option<(char, Vec<KeyBinding>)>;
  fn set_macro_recording(&mut self, recording: Option<(char, Vec<KeyBinding>)>);
  fn macro_replaying(&self) -> &Vec<char>;
  fn macro_replaying_mut(&mut self) -> &mut Vec<char>;
  fn macro_queue(&self) -> &VecDeque<KeyEvent>;
  fn macro_queue_mut(&mut self) -> &mut VecDeque<KeyEvent>;
  fn last_motion(&self) -> Option<Motion>;
  fn set_last_motion(&mut self, motion: Option<Motion>);
  fn text_format(&self) -> TextFormat;
  fn soft_wrap_enabled(&self) -> bool;
  fn set_soft_wrap_enabled(&mut self, enabled: bool);
  fn gutter_config(&self) -> &GutterConfig;
  fn gutter_config_mut(&mut self) -> &mut GutterConfig;
  fn text_annotations(&self) -> TextAnnotations<'_>;
  fn syntax_loader(&self) -> Option<&Loader>;
  fn ui_theme(&self) -> &Theme;
  fn set_file_path(&mut self, path: Option<PathBuf>);
  fn open_file(&mut self, path: &Path) -> std::io::Result<()>;
  fn goto_buffer(&mut self, _direction: Direction, _count: usize) -> bool {
    false
  }
  fn goto_last_accessed_buffer(&mut self) -> bool {
    false
  }
  fn goto_last_modified_buffer(&mut self) -> bool {
    false
  }
  fn save_current_buffer(&mut self, force: bool) -> Result<(), String> {
    let Some(path) = self.file_path().map(|path| path.to_path_buf()) else {
      return Err(
        "no file path set for current buffer; use :write <path> to save an untitled buffer"
          .to_string(),
      );
    };

    if self.watch_conflict_active() && !force {
      return Err(
        "external file conflict active; use :w! to overwrite disk or :rl to reload from disk"
          .to_string(),
      );
    }

    let text = self.editor_ref().document().text().to_string();
    let line_count = text.lines().count().max(1);
    let byte_count = text.len();
    std::fs::write(&path, &text)
      .map_err(|err| format!("Failed to write {}: {err}", path.display()))?;
    let _ = self.editor().document_mut().mark_saved();
    self.on_file_saved(&path, &text);
    if force {
      self.clear_watch_conflict();
    }

    let path_text = status_path_text(&path);
    let size_text = format_binary_size(byte_count);
    let forced_suffix = if force { " (forced)" } else { "" };
    self.push_info(
      "save",
      format!("'{path_text}' written, {line_count}L {size_text}{forced_suffix}"),
    );
    Ok(())
  }
  fn reload_file_preserving_view(&mut self, path: &Path) -> std::io::Result<()> {
    let disk_text = std::fs::read_to_string(path)?;
    let previous_text = self.editor_ref().document().text().clone();
    let previous_selection = self.editor_ref().document().selection().clone();
    let previous_scroll = self.editor_ref().view().scroll;
    let disk_rope = ropey::Rope::from_str(&disk_text);

    let tx = compare_ropes(&previous_text, &disk_rope);
    if !tx.changes().is_empty() && !self.apply_transaction(&tx) {
      return Err(std::io::Error::other("failed to apply reload transaction"));
    }

    {
      let doc = self.editor().document_mut();
      let new_text = doc.text().clone();
      let remapped = previous_selection.transform(|range| {
        let anchor_coords = coords_at_pos(previous_text.slice(..), range.anchor);
        let head_coords = coords_at_pos(previous_text.slice(..), range.head);
        Range::new(
          char_idx_at_coords(new_text.slice(..), anchor_coords),
          char_idx_at_coords(new_text.slice(..), head_coords),
        )
      });
      let _ = doc.set_selection(remapped);
      let _ = doc.mark_saved();
    }

    let max_row = self
      .editor_ref()
      .document()
      .text()
      .len_lines()
      .saturating_sub(1);
    self.editor().view_mut().scroll =
      Position::new(previous_scroll.row.min(max_row), previous_scroll.col);

    self.request_render();
    Ok(())
  }
  fn log_target_names(&self) -> &'static [&'static str] {
    &[]
  }
  fn log_path_for_target(&self, _target: &str) -> Option<PathBuf> {
    None
  }
  fn lsp_goto_declaration(&mut self) {}
  fn lsp_goto_definition(&mut self) {}
  fn lsp_goto_type_definition(&mut self) {}
  fn lsp_goto_implementation(&mut self) {}
  fn lsp_hover(&mut self) {}
  fn lsp_references(&mut self) {}
  fn lsp_document_symbols(&mut self) {}
  fn lsp_workspace_symbols(&mut self) {}
  fn lsp_completion(&mut self) {}
  fn lsp_signature_help(&mut self) {}
  fn lsp_code_actions(&mut self) {}
  fn lsp_rename(&mut self, _new_name: &str) {}
  fn lsp_format(&mut self) {}
  fn on_file_saved(&mut self, _path: &Path, _text: &str) {}
  fn on_before_quit(&mut self) {}
  fn scrolloff(&self) -> usize {
    5
  }
}

pub fn build_dispatch<Ctx>() -> DefaultDispatchStatic<Ctx>
where
  Ctx: DefaultContext,
{
  DefaultDispatch::new()
    .with_pre_on_keypress(pre_on_keypress::<Ctx> as fn(&mut Ctx, KeyEvent))
    .with_on_keypress(on_keypress::<Ctx> as fn(&mut Ctx, KeyEvent))
    .with_post_on_keypress(post_on_keypress::<Ctx> as fn(&mut Ctx, Command))
    .with_pre_on_action(pre_on_action::<Ctx> as fn(&mut Ctx, Command))
    .with_on_action(on_action::<Ctx> as fn(&mut Ctx, Command))
    .with_post_on_action(post_on_action::<Ctx> as fn(&mut Ctx, ()))
    .with_render_request(render_request::<Ctx> as fn(&mut Ctx, ()))
    .with_pre_render(pre_render::<Ctx> as fn(&mut Ctx, ()))
    .with_on_render(on_render::<Ctx> as fn(&mut Ctx, ()) -> RenderPlan)
    .with_post_render(post_render::<Ctx> as fn(&mut Ctx, RenderPlan) -> RenderPlan)
    .with_pre_render_with_styles(
      pre_render_with_styles::<Ctx> as fn(&mut Ctx, RenderStyles) -> RenderStyles,
    )
    .with_on_render_with_styles(
      on_render_with_styles::<Ctx> as fn(&mut Ctx, RenderStyles) -> RenderPlan,
    )
    .with_pre_ui(pre_ui::<Ctx> as fn(&mut Ctx, ()))
    .with_on_ui(on_ui::<Ctx> as fn(&mut Ctx, ()) -> UiTree)
    .with_post_ui(post_ui::<Ctx> as fn(&mut Ctx, UiTree) -> UiTree)
    .with_pre_ui_event(pre_ui_event::<Ctx> as fn(&mut Ctx, UiEvent) -> UiEventOutcome)
    .with_on_ui_event(on_ui_event::<Ctx> as fn(&mut Ctx, UiEvent) -> UiEventOutcome)
    .with_post_ui_event(post_ui_event::<Ctx> as fn(&mut Ctx, UiEventOutcome) -> UiEventOutcome)
    .with_insert_char(insert_char::<Ctx> as fn(&mut Ctx, char))
    .with_insert_newline(insert_newline::<Ctx> as fn(&mut Ctx, ()))
    .with_delete_char(delete_char::<Ctx> as fn(&mut Ctx, ()))
    .with_delete_char_forward(delete_char_forward::<Ctx> as fn(&mut Ctx, usize))
    .with_delete_word_backward(delete_word_backward::<Ctx> as fn(&mut Ctx, usize))
    .with_delete_word_forward(delete_word_forward::<Ctx> as fn(&mut Ctx, usize))
    .with_kill_to_line_start(kill_to_line_start::<Ctx> as fn(&mut Ctx, ()))
    .with_kill_to_line_end(kill_to_line_end::<Ctx> as fn(&mut Ctx, ()))
    .with_insert_tab(insert_tab::<Ctx> as fn(&mut Ctx, ()))
    .with_goto_line_start(goto_line_start::<Ctx> as fn(&mut Ctx, bool))
    .with_goto_first_nonwhitespace(goto_first_nonwhitespace::<Ctx> as fn(&mut Ctx, bool))
    .with_goto_line_end(goto_line_end::<Ctx> as fn(&mut Ctx, bool))
    .with_page_up(page_up::<Ctx> as fn(&mut Ctx, bool))
    .with_page_down(page_down::<Ctx> as fn(&mut Ctx, bool))
    .with_move_cursor(move_cursor::<Ctx> as fn(&mut Ctx, Direction))
    .with_add_cursor(add_cursor::<Ctx> as fn(&mut Ctx, Direction))
    .with_motion(motion::<Ctx> as fn(&mut Ctx, Motion))
    .with_delete_selection(delete_selection::<Ctx> as fn(&mut Ctx, bool))
    .with_change_selection(change_selection::<Ctx> as fn(&mut Ctx, bool))
    .with_replace_selection(replace_selection::<Ctx> as fn(&mut Ctx, char))
    .with_replace_with_yanked(replace_with_yanked::<Ctx> as fn(&mut Ctx, ()))
    .with_yank(yank::<Ctx> as fn(&mut Ctx, ()))
    .with_paste(paste::<Ctx> as fn(&mut Ctx, bool))
    .with_switch_case(switch_case::<Ctx> as fn(&mut Ctx, ()))
    .with_save(save::<Ctx> as fn(&mut Ctx, ()))
    .with_quit(quit::<Ctx> as fn(&mut Ctx, ()))
    .with_find_char(find_char::<Ctx> as fn(&mut Ctx, (Direction, bool, bool)))
    .with_search(search::<Ctx> as fn(&mut Ctx, ()))
    .with_rsearch(rsearch::<Ctx> as fn(&mut Ctx, ()))
    .with_select_regex(select_regex::<Ctx> as fn(&mut Ctx, ()))
    .with_search_next_or_prev(search_next_or_prev::<Ctx> as fn(&mut Ctx, (Direction, bool, usize)))
    .with_parent_node_end(parent_node_end::<Ctx> as fn(&mut Ctx, bool))
    .with_parent_node_start(parent_node_start::<Ctx> as fn(&mut Ctx, bool))
    .with_repeat_last_motion(repeat_last_motion::<Ctx> as fn(&mut Ctx, ()))
    .with_switch_to_uppercase(switch_to_uppercase::<Ctx> as fn(&mut Ctx, ()))
    .with_switch_to_lowercase(switch_to_lowercase::<Ctx> as fn(&mut Ctx, ()))
    .with_insert_at_line_start(insert_at_line_start::<Ctx> as fn(&mut Ctx, ()))
    .with_insert_at_line_end(insert_at_line_end::<Ctx> as fn(&mut Ctx, ()))
    .with_append_mode(append_mode::<Ctx> as fn(&mut Ctx, ()))
    .with_open_below(open_below::<Ctx> as fn(&mut Ctx, ()))
    .with_open_above(open_above::<Ctx> as fn(&mut Ctx, ()))
    .with_commit_undo_checkpoint(commit_undo_checkpoint::<Ctx> as fn(&mut Ctx, ()))
    .with_copy_selection_on_next_line(copy_selection_on_next_line::<Ctx> as fn(&mut Ctx, ()))
    .with_copy_selection_on_prev_line(copy_selection_on_prev_line::<Ctx> as fn(&mut Ctx, ()))
    .with_select_all(select_all::<Ctx> as fn(&mut Ctx, ()))
    .with_extend_line_below(extend_line_below::<Ctx> as fn(&mut Ctx, usize))
    .with_extend_line_above(extend_line_above::<Ctx> as fn(&mut Ctx, usize))
    .with_extend_to_line_bounds(extend_to_line_bounds::<Ctx> as fn(&mut Ctx, ()))
    .with_shrink_to_line_bounds(shrink_to_line_bounds::<Ctx> as fn(&mut Ctx, ()))
    .with_undo(undo::<Ctx> as fn(&mut Ctx, usize))
    .with_redo(redo::<Ctx> as fn(&mut Ctx, usize))
    .with_earlier(earlier::<Ctx> as fn(&mut Ctx, usize))
    .with_later(later::<Ctx> as fn(&mut Ctx, usize))
    .with_indent(indent::<Ctx> as fn(&mut Ctx, usize))
    .with_unindent(unindent::<Ctx> as fn(&mut Ctx, usize))
    .with_replace(replace::<Ctx> as fn(&mut Ctx, ()))
    .with_record_macro(record_macro::<Ctx> as fn(&mut Ctx, ()))
    .with_replay_macro(replay_macro::<Ctx> as fn(&mut Ctx, ()))
    .with_match_brackets(match_brackets::<Ctx> as fn(&mut Ctx, ()))
    .with_surround_add(surround_add::<Ctx> as fn(&mut Ctx, ()))
    .with_surround_delete(surround_delete::<Ctx> as fn(&mut Ctx, usize))
    .with_surround_replace(surround_replace::<Ctx> as fn(&mut Ctx, usize))
    .with_select_textobject_around(select_textobject_around::<Ctx> as fn(&mut Ctx, ()))
    .with_select_textobject_inner(select_textobject_inner::<Ctx> as fn(&mut Ctx, ()))
}

pub fn handle_command<Ctx, D>(dispatch: &D, ctx: &mut Ctx, command: Command)
where
  Ctx: DefaultContext,
  D: DefaultApi<Ctx>,
{
  dispatch.pre_on_action(ctx, command);
}

pub fn handle_key<Ctx, D>(dispatch: &D, ctx: &mut Ctx, key: KeyEvent)
where
  Ctx: DefaultContext,
  D: DefaultApi<Ctx>,
{
  dispatch.pre_on_keypress(ctx, key);
}

pub fn render_plan<Ctx: DefaultContext>(ctx: &mut Ctx) -> RenderPlan {
  frame_render_plan(ctx)
    .into_active_plan()
    .unwrap_or_default()
}

pub fn render_plan_with_styles<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  styles: RenderStyles,
) -> RenderPlan {
  frame_render_plan_with_styles(ctx, styles)
    .into_active_plan()
    .unwrap_or_default()
}

pub fn frame_render_plan<Ctx: DefaultContext>(ctx: &mut Ctx) -> FrameRenderPlan {
  ctx.dispatch().pre_render(ctx, ());
  let mut frame = ctx.build_frame_render_plan();
  for pane in &mut frame.panes {
    pane.plan = ctx
      .dispatch()
      .post_render(ctx, std::mem::take(&mut pane.plan));
  }
  frame
}

pub fn frame_render_plan_with_styles<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  styles: RenderStyles,
) -> FrameRenderPlan {
  ctx.dispatch().pre_render(ctx, ());
  let styles = ctx.dispatch().pre_render_with_styles(ctx, styles);
  let mut frame = ctx.build_frame_render_plan_with_styles(styles);
  for pane in &mut frame.panes {
    pane.plan = ctx
      .dispatch()
      .post_render(ctx, std::mem::take(&mut pane.plan));
  }
  frame
}

pub fn ui_tree<Ctx: DefaultContext>(ctx: &mut Ctx) -> UiTree {
  ctx.dispatch().pre_ui(ctx, ());
  let tree = ctx.dispatch().on_ui(ctx, ());
  let mut tree = ctx.dispatch().post_ui(ctx, tree);
  if !crate::statusline::statusline_present(&tree) {
    let statusline = crate::statusline::build_statusline_ui(ctx);
    tree.overlays.insert(0, statusline);
  }
  if let Some(message_bar) = crate::message_bar::build_message_bar_ui(ctx) {
    tree.overlays.insert(1, message_bar);
  }
  resolve_ui_tree(&mut tree, ctx.ui_theme());
  tree
}

pub fn ui_event<Ctx: DefaultContext>(ctx: &mut Ctx, event: UiEvent) -> UiEventOutcome {
  let mut event = event;
  if event.target.is_none() {
    if let Some(focus) = ctx.ui_state().focus() {
      event.target = Some(focus.id.clone());
    }
  }

  let outcome = ctx.dispatch().pre_ui_event(ctx, event.clone());
  let outcome = if outcome.handled {
    outcome
  } else {
    ctx.dispatch().on_ui_event(ctx, event)
  };
  let outcome = ctx.dispatch().post_ui_event(ctx, outcome);
  if let Some(focus) = outcome.focus.clone() {
    ctx.ui_state_mut().set_focus(Some(focus));
  }
  outcome
}

pub fn default_pre_on_keypress<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) {
  if let Some(next) = ctx.macro_queue_mut().pop_front() {
    ctx.dispatch().on_keypress(ctx, next);
    if ctx.macro_queue().is_empty() {
      ctx.macro_replaying_mut().pop();
    }
    return;
  }

  if ctx.macro_replaying().is_empty() {
    if let Some((reg, mut keys)) = ctx.macro_recording().clone() {
      keys.push(KeyBinding::from_key_event(&key));
      ctx.set_macro_recording(Some((reg, keys)));
    }
  }

  ctx.dispatch().on_keypress(ctx, key);
}

fn pre_on_keypress<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) {
  default_pre_on_keypress(ctx, key);
}

fn handle_pending_input<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  pending: PendingInput,
  key: KeyEvent,
) -> bool {
  match pending {
    PendingInput::FindChar {
      direction,
      inclusive,
      extend,
      count,
    } => {
      if let Key::Char(ch) = key.key {
        find_char_impl(ctx, ch, direction, inclusive, extend, count);
      }
      true
    },
    PendingInput::InsertRegister => true, // TODO
    PendingInput::Placeholder => true,
    PendingInput::ReplaceSelection => {
      match key.key {
        Key::Char(ch) => ctx.dispatch().replace_selection(ctx, ch),
        Key::Enter | Key::NumpadEnter => {
          let line_ending = ctx.editor_ref().document().line_ending().as_str();
          replace_selection_with_str(ctx, line_ending);
        },
        _ => {},
      }
      true
    },
    PendingInput::SurroundAdd => {
      surround_add_impl(ctx, key);
      true
    },
    PendingInput::SurroundDelete { count } => {
      let surround_ch = match key.key {
        Key::Char('m') => None, // m selects the closest surround pair
        Key::Char(ch) => Some(ch),
        _ => return true,
      };
      surround_delete_impl(ctx, surround_ch, count);
      true
    },
    PendingInput::SurroundReplace { count } => {
      if let Key::Char(ch) = key.key {
        surround_replace_find(ctx, ch, count);
      }
      true
    },
    PendingInput::SurroundReplaceWith {
      positions,
      original_selection,
    } => {
      if let Key::Char(ch) = key.key {
        surround_replace_with(ctx, ch, &positions, &original_selection);
      }
      true
    },
    PendingInput::SelectTextObject { kind } => {
      if let Key::Char(ch) = key.key {
        select_textobject_impl(ctx, kind, ch);
      }
      true
    },
    PendingInput::WordJump {
      extend,
      first,
      targets,
    } => {
      let Key::Char(ch) = key.key else {
        ctx.clear_word_jump_annotations();
        return true;
      };
      if !key.modifiers.is_empty() {
        ctx.clear_word_jump_annotations();
        return true;
      }
      let typed = ch.to_ascii_lowercase();
      let alphabet: Vec<char> = WORD_JUMP_LABEL_ALPHABET.chars().collect();
      let Some(index) = alphabet.iter().position(|it| *it == typed) else {
        ctx.clear_word_jump_annotations();
        return true;
      };

      if let Some(outer) = first {
        let target_index = outer.saturating_add(index);
        if let Some(target) = targets.get(target_index) {
          apply_word_jump_target(ctx, target.range, extend);
        }
        ctx.clear_word_jump_annotations();
        return true;
      }

      let outer = index.saturating_mul(alphabet.len());
      if outer >= targets.len() {
        ctx.clear_word_jump_annotations();
        return true;
      }
      ctx.set_pending_input(Some(PendingInput::WordJump {
        extend,
        first: Some(outer),
        targets,
      }));
      true
    },
    PendingInput::CursorPick {
      remove,
      original_active,
      mut candidates,
      mut index,
    } => {
      let selection = ctx.editor_ref().document().selection();
      candidates.retain(|id| selection.range_by_id(*id).is_some());
      if candidates.is_empty() {
        ctx.editor().view_mut().active_cursor = original_active;
        return true;
      }

      index = index.min(candidates.len().saturating_sub(1));

      match key.key {
        Key::Escape => {
          ctx.editor().view_mut().active_cursor =
            original_active.filter(|id| selection.range_by_id(*id).is_some());
        },
        Key::Left | Key::Up => {
          index = if index == 0 {
            candidates.len() - 1
          } else {
            index - 1
          };
          ctx.editor().view_mut().active_cursor = Some(candidates[index]);
          ctx.set_pending_input(Some(PendingInput::CursorPick {
            remove,
            original_active,
            candidates,
            index,
          }));
        },
        Key::Right | Key::Down => {
          index = (index + 1) % candidates.len();
          ctx.editor().view_mut().active_cursor = Some(candidates[index]);
          ctx.set_pending_input(Some(PendingInput::CursorPick {
            remove,
            original_active,
            candidates,
            index,
          }));
        },
        Key::Enter | Key::NumpadEnter => {
          let target = candidates[index];
          if let Err(err) = apply_cursor_pick(ctx, target, remove) {
            ctx.push_warning("selection", err);
            ctx.editor().view_mut().active_cursor = original_active;
          }
        },
        _ => {
          ctx.set_pending_input(Some(PendingInput::CursorPick {
            remove,
            original_active,
            candidates,
            index,
          }));
        },
      }

      true
    },
  }
}

fn on_keypress<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) {
  ctx.dismiss_active_message();
  if ctx.file_picker().active {
    if crate::file_picker::handle_file_picker_key(ctx, key) {
      return;
    }
  }
  if ctx.search_prompt_ref().active {
    if crate::search_prompt::handle_search_prompt_key(ctx, key) {
      return;
    }
  }
  if let Some(pending) = ctx.pending_input().cloned() {
    ctx.set_pending_input(None);
    if handle_pending_input(ctx, pending, key) {
      ctx.dispatch().render_request(ctx, ());
      return;
    }
  }

  if handle_insert_mode_completion_key(ctx, key) {
    return;
  }

  if ctx.mode() == Mode::Command {
    if handle_command_prompt_key(ctx, key) {
      return;
    }
  }

  let outcome = keymap_handle_key(ctx, key);
  let _ = handle_key_outcome(ctx, outcome);
}

fn handle_key_outcome<Ctx: DefaultContext>(ctx: &mut Ctx, outcome: KeyOutcome) -> bool {
  match outcome {
    KeyOutcome::Command(command) => {
      ctx.dispatch().post_on_keypress(ctx, command);
      true
    },
    KeyOutcome::Commands(commands) => {
      for command in commands {
        ctx.dispatch().post_on_keypress(ctx, command);
      }
      true
    },
    KeyOutcome::Handled => {
      // Pending/cancelled keymap states must trigger a redraw so statusline
      // indicators (e.g. pending keys) are visible immediately.
      ctx.dispatch().render_request(ctx, ());
      true
    },
    KeyOutcome::Continue => false,
  }
}

fn handle_insert_mode_completion_key<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) -> bool {
  if ctx.mode() != Mode::Insert || !ctx.completion_menu().active {
    return false;
  }

  match key.key {
    Key::Up => {
      crate::completion_menu::completion_prev(ctx);
      true
    },
    Key::Down => {
      crate::completion_menu::completion_next(ctx);
      true
    },
    Key::Tab if key.modifiers.shift() => {
      crate::completion_menu::completion_prev(ctx);
      true
    },
    Key::Tab => {
      crate::completion_menu::completion_next(ctx);
      true
    },
    Key::Enter | Key::NumpadEnter => {
      crate::completion_menu::completion_accept(ctx);
      true
    },
    // Let keymaps decide escape behavior (normal mode by default).
    Key::Escape => false,
    _ => false,
  }
}

fn post_on_keypress<Ctx: DefaultContext>(ctx: &mut Ctx, command: Command) {
  ctx.dispatch().pre_on_action(ctx, command);
}

fn pre_on_action<Ctx: DefaultContext>(ctx: &mut Ctx, command: Command) {
  ctx.dispatch().on_action(ctx, command);
}

fn on_action<Ctx: DefaultContext>(ctx: &mut Ctx, command: Command) {
  match command {
    Command::InsertChar(ch) => {
      if ctx.completion_menu().active {
        let _ = ctx.completion_accept_on_commit_char(ch);
      }
      ctx.dispatch().insert_char(ctx, ch);
    },
    Command::InsertNewline => ctx.dispatch().insert_newline(ctx, ()),
    Command::DeleteChar => ctx.dispatch().delete_char(ctx, ()),
    Command::DeleteCharForward { count } => ctx.dispatch().delete_char_forward(ctx, count),
    Command::DeleteWordBackward { count } => ctx.dispatch().delete_word_backward(ctx, count),
    Command::DeleteWordForward { count } => ctx.dispatch().delete_word_forward(ctx, count),
    Command::KillToLineStart => ctx.dispatch().kill_to_line_start(ctx, ()),
    Command::KillToLineEnd => ctx.dispatch().kill_to_line_end(ctx, ()),
    Command::InsertTab => ctx.dispatch().insert_tab(ctx, ()),
    Command::GotoLineStart { extend } => ctx.dispatch().goto_line_start(ctx, extend),
    Command::GotoFirstNonWhitespace { extend } => {
      ctx.dispatch().goto_first_nonwhitespace(ctx, extend);
    },
    Command::GotoLineEnd { extend } => ctx.dispatch().goto_line_end(ctx, extend),
    Command::PageUp { extend } => ctx.dispatch().page_up(ctx, extend),
    Command::PageDown { extend } => ctx.dispatch().page_down(ctx, extend),
    Command::PageCursorHalfUp => page_cursor_half_up(ctx),
    Command::PageCursorHalfDown => page_cursor_half_down(ctx),
    Command::FindChar {
      direction,
      inclusive,
      extend,
    } => {
      ctx
        .dispatch()
        .find_char(ctx, (direction, inclusive, extend));
    },
    Command::ParentNodeEnd { extend } => ctx.dispatch().parent_node_end(ctx, extend),
    Command::ParentNodeStart { extend } => ctx.dispatch().parent_node_start(ctx, extend),
    Command::Move(dir) => ctx.dispatch().move_cursor(ctx, dir),
    Command::AddCursor(dir) => ctx.dispatch().add_cursor(ctx, dir),
    Command::Motion(motion) => {
      ctx.set_last_motion(Some(motion));
      ctx.dispatch().motion(ctx, motion);
    },
    Command::GotoNextBuffer { count } => {
      if !ctx.goto_buffer(Direction::Forward, count.max(1)) {
        ctx.push_warning(
          "buffer",
          "no next buffer available (this client currently has a single active buffer)",
        );
      }
    },
    Command::GotoPreviousBuffer { count } => {
      if !ctx.goto_buffer(Direction::Backward, count.max(1)) {
        ctx.push_warning(
          "buffer",
          "no previous buffer available (this client currently has a single active buffer)",
        );
      }
    },
    Command::GotoWindowTop { count } => goto_window(ctx, WindowAlign::Top, count.max(1)),
    Command::GotoWindowCenter => goto_window(ctx, WindowAlign::Center, 1),
    Command::GotoWindowBottom { count } => goto_window(ctx, WindowAlign::Bottom, count.max(1)),
    Command::RotateView => rotate_view(ctx),
    Command::HSplit => split_view(ctx, SplitAxis::Horizontal),
    Command::VSplit => split_view(ctx, SplitAxis::Vertical),
    Command::TransposeView => transpose_view(ctx),
    Command::WClose => close_view(ctx),
    Command::WOnly => only_view(ctx),
    Command::JumpViewLeft => jump_view(ctx, PaneDirection::Left),
    Command::JumpViewDown => jump_view(ctx, PaneDirection::Down),
    Command::JumpViewUp => jump_view(ctx, PaneDirection::Up),
    Command::JumpViewRight => jump_view(ctx, PaneDirection::Right),
    Command::SwapViewLeft => swap_view(ctx, PaneDirection::Left),
    Command::SwapViewDown => swap_view(ctx, PaneDirection::Down),
    Command::SwapViewUp => swap_view(ctx, PaneDirection::Up),
    Command::SwapViewRight => swap_view(ctx, PaneDirection::Right),
    Command::GotoFileHSplit => goto_file_split(ctx, SplitAxis::Horizontal),
    Command::GotoFileVSplit => goto_file_split(ctx, SplitAxis::Vertical),
    Command::HSplitNew => split_new_scratch(ctx, SplitAxis::Horizontal),
    Command::VSplitNew => split_new_scratch(ctx, SplitAxis::Vertical),
    Command::GotoLastAccessedFile => {
      if !ctx.goto_last_accessed_buffer() {
        ctx.push_warning("buffer", "no last accessed buffer");
      }
    },
    Command::GotoLastModifiedFile => {
      if !ctx.goto_last_modified_buffer() {
        ctx.push_warning("buffer", "no last modified buffer");
      }
    },
    Command::GotoLastModification => goto_last_modification(ctx),
    Command::GotoWord => goto_word(ctx, false),
    Command::ExtendToWord => goto_word(ctx, true),
    Command::SplitSelectionOnNewline => split_selection_on_newline(ctx),
    Command::MergeSelections => merge_selections(ctx),
    Command::MergeConsecutiveSelections => merge_consecutive_selections(ctx),
    Command::SplitSelection => split_selection(ctx),
    Command::JoinSelections => join_selections(ctx),
    Command::JoinSelectionsSpace => join_selections_space(ctx),
    Command::KeepSelections => keep_selections(ctx),
    Command::RemoveSelections => remove_selections(ctx),
    Command::AlignSelections => align_selections(ctx),
    Command::KeepActiveSelection => keep_active_selection(ctx),
    Command::RemoveActiveSelection => remove_active_selection(ctx),
    Command::TrimSelections => trim_selections(ctx),
    Command::CollapseSelection => collapse_selection(ctx),
    Command::FlipSelections => flip_selections(ctx),
    Command::ExpandSelection => expand_selection(ctx),
    Command::ShrinkSelection => shrink_selection(ctx),
    Command::SelectAllChildren => select_all_children(ctx),
    Command::SelectAllSiblings => select_all_siblings(ctx),
    Command::SelectPrevSibling => select_prev_sibling(ctx),
    Command::SelectNextSibling => select_next_sibling(ctx),
    Command::DeleteSelection { yank } => ctx.dispatch().delete_selection(ctx, yank),
    Command::ChangeSelection { yank } => ctx.dispatch().change_selection(ctx, yank),
    Command::Replace => ctx.dispatch().replace(ctx, ()),
    Command::ReplaceWithYanked => ctx.dispatch().replace_with_yanked(ctx, ()),
    Command::Yank => ctx.dispatch().yank(ctx, ()),
    Command::Paste { after } => ctx.dispatch().paste(ctx, after),
    Command::RecordMacro => ctx.dispatch().record_macro(ctx, ()),
    Command::ReplayMacro => ctx.dispatch().replay_macro(ctx, ()),
    Command::RepeatLastMotion => ctx.dispatch().repeat_last_motion(ctx, ()),
    Command::SwitchCase => ctx.dispatch().switch_case(ctx, ()),
    Command::SwitchToUppercase => ctx.dispatch().switch_to_uppercase(ctx, ()),
    Command::SwitchToLowercase => ctx.dispatch().switch_to_lowercase(ctx, ()),
    Command::InsertAtLineStart => ctx.dispatch().insert_at_line_start(ctx, ()),
    Command::InsertAtLineEnd => ctx.dispatch().insert_at_line_end(ctx, ()),
    Command::AppendMode => ctx.dispatch().append_mode(ctx, ()),
    Command::OpenBelow => ctx.dispatch().open_below(ctx, ()),
    Command::OpenAbove => ctx.dispatch().open_above(ctx, ()),
    Command::CommitUndoCheckpoint => ctx.dispatch().commit_undo_checkpoint(ctx, ()),
    Command::CopySelectionOnNextLine => ctx.dispatch().copy_selection_on_next_line(ctx, ()),
    Command::CopySelectionOnPrevLine => ctx.dispatch().copy_selection_on_prev_line(ctx, ()),
    Command::SelectAll => ctx.dispatch().select_all(ctx, ()),
    Command::ExtendLineBelow { count } => ctx.dispatch().extend_line_below(ctx, count),
    Command::ExtendLineAbove { count } => ctx.dispatch().extend_line_above(ctx, count),
    Command::ExtendToLineBounds => ctx.dispatch().extend_to_line_bounds(ctx, ()),
    Command::ShrinkToLineBounds => ctx.dispatch().shrink_to_line_bounds(ctx, ()),
    Command::Undo { count } => ctx.dispatch().undo(ctx, count),
    Command::Redo { count } => ctx.dispatch().redo(ctx, count),
    Command::Earlier { count } => ctx.dispatch().earlier(ctx, count),
    Command::Later { count } => ctx.dispatch().later(ctx, count),
    Command::Indent { count } => ctx.dispatch().indent(ctx, count),
    Command::Unindent { count } => ctx.dispatch().unindent(ctx, count),
    Command::MatchBrackets => ctx.dispatch().match_brackets(ctx, ()),
    Command::SurroundAdd => ctx.dispatch().surround_add(ctx, ()),
    Command::SurroundDelete { count } => ctx.dispatch().surround_delete(ctx, count),
    Command::SurroundReplace { count } => ctx.dispatch().surround_replace(ctx, count),
    Command::SelectTextobjectAround => ctx.dispatch().select_textobject_around(ctx, ()),
    Command::SelectTextobjectInner => ctx.dispatch().select_textobject_inner(ctx, ()),
    Command::GotoPrevDiag => goto_prev_diag(ctx),
    Command::GotoFirstDiag => goto_first_diag(ctx),
    Command::GotoNextDiag => goto_next_diag(ctx),
    Command::GotoLastDiag => goto_last_diag(ctx),
    Command::GotoPrevChange => goto_change(ctx, Direction::Backward),
    Command::GotoFirstChange => goto_first_change(ctx),
    Command::GotoNextChange => goto_change(ctx, Direction::Forward),
    Command::GotoLastChange => goto_last_change(ctx),
    Command::GotoPrevFunction => goto_ts_object(ctx, "function", Direction::Backward),
    Command::GotoNextFunction => goto_ts_object(ctx, "function", Direction::Forward),
    Command::GotoPrevClass => goto_ts_object(ctx, "class", Direction::Backward),
    Command::GotoNextClass => goto_ts_object(ctx, "class", Direction::Forward),
    Command::GotoPrevParameter => goto_ts_object(ctx, "parameter", Direction::Backward),
    Command::GotoNextParameter => goto_ts_object(ctx, "parameter", Direction::Forward),
    Command::GotoPrevComment => goto_ts_object(ctx, "comment", Direction::Backward),
    Command::GotoNextComment => goto_ts_object(ctx, "comment", Direction::Forward),
    Command::GotoPrevEntry => goto_ts_object(ctx, "entry", Direction::Backward),
    Command::GotoNextEntry => goto_ts_object(ctx, "entry", Direction::Forward),
    Command::GotoPrevTest => goto_ts_object(ctx, "test", Direction::Backward),
    Command::GotoNextTest => goto_ts_object(ctx, "test", Direction::Forward),
    Command::GotoPrevXmlElement => goto_ts_object(ctx, "xml-element", Direction::Backward),
    Command::GotoNextXmlElement => goto_ts_object(ctx, "xml-element", Direction::Forward),
    Command::GotoPrevParagraph => goto_paragraph(ctx, Direction::Backward),
    Command::GotoNextParagraph => goto_paragraph(ctx, Direction::Forward),
    Command::AddNewlineAbove => add_newline(ctx, OpenDirection::Above),
    Command::AddNewlineBelow => add_newline(ctx, OpenDirection::Below),
    Command::SearchSelectionDetectWordBoundaries => search_selection(ctx, true),
    Command::SearchSelection => search_selection(ctx, false),
    Command::Search => ctx.dispatch().search(ctx, ()),
    Command::RSearch => ctx.dispatch().rsearch(ctx, ()),
    Command::SelectRegex => ctx.dispatch().select_regex(ctx, ()),
    Command::FilePicker => crate::file_picker::open_file_picker(ctx),
    Command::LspGotoDeclaration => ctx.lsp_goto_declaration(),
    Command::LspGotoDefinition => ctx.lsp_goto_definition(),
    Command::LspGotoTypeDefinition => ctx.lsp_goto_type_definition(),
    Command::LspGotoImplementation => ctx.lsp_goto_implementation(),
    Command::LspHover => ctx.lsp_hover(),
    Command::LspReferences => ctx.lsp_references(),
    Command::LspDocumentSymbols => ctx.lsp_document_symbols(),
    Command::LspWorkspaceSymbols => ctx.lsp_workspace_symbols(),
    Command::LspCompletion => ctx.lsp_completion(),
    Command::CompletionNext => crate::completion_menu::completion_next(ctx),
    Command::CompletionPrev => crate::completion_menu::completion_prev(ctx),
    Command::CompletionAccept => crate::completion_menu::completion_accept(ctx),
    Command::CompletionCancel => crate::completion_menu::close_completion_menu(ctx),
    Command::CompletionDocsScrollUp => {
      if ctx.completion_menu().active {
        crate::completion_menu::completion_docs_scroll(ctx, -6);
      } else {
        ctx.dispatch().page_up(ctx, false);
      }
    },
    Command::CompletionDocsScrollDown => {
      if ctx.completion_menu().active {
        crate::completion_menu::completion_docs_scroll(ctx, 6);
      } else {
        ctx.dispatch().page_down(ctx, false);
      }
    },
    Command::LspSignatureHelp => ctx.lsp_signature_help(),
    Command::LspCodeActions => ctx.lsp_code_actions(),
    Command::LspFormat => ctx.lsp_format(),
    Command::SearchNextOrPrev {
      direction,
      extend,
      count,
    } => {
      ctx
        .dispatch()
        .search_next_or_prev(ctx, (direction, extend, count));
    },
    Command::Save => ctx.dispatch().save(ctx, ()),
    Command::Quit => ctx.dispatch().quit(ctx, ()),
  }

  let preserve_completion_menu =
    command_preserves_completion_menu(command) || ctx.completion_on_action(command);
  if ctx.completion_menu().active && !preserve_completion_menu {
    ctx.completion_menu_mut().clear();
  }

  ctx.dispatch().post_on_action(ctx, ());
}

fn command_preserves_completion_menu(command: Command) -> bool {
  matches!(
    command,
    Command::LspCompletion
      | Command::CompletionNext
      | Command::CompletionPrev
      | Command::CompletionAccept
      | Command::CompletionCancel
      | Command::CompletionDocsScrollUp
      | Command::CompletionDocsScrollDown
  )
}

fn repeat_last_motion<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  if let Some(motion) = ctx.last_motion() {
    ctx.dispatch().motion(ctx, motion);
  }
}

fn replace<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  ctx.set_pending_input(Some(PendingInput::ReplaceSelection));
}

fn surround_add<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  ctx.set_pending_input(Some(PendingInput::SurroundAdd));
}

fn surround_delete<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  ctx.set_pending_input(Some(PendingInput::SurroundDelete { count }));
}

fn surround_replace<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  ctx.set_pending_input(Some(PendingInput::SurroundReplace { count }));
}

fn select_textobject_around<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  ctx.set_pending_input(Some(PendingInput::SelectTextObject {
    kind: text_object::TextObject::Around,
  }));
}

fn select_textobject_inner<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  ctx.set_pending_input(Some(PendingInput::SelectTextObject {
    kind: text_object::TextObject::Inside,
  }));
}

fn post_on_action<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  if ctx.mode() != Mode::Insert {
    let _ = ctx.editor().document_mut().commit();
  }
  ctx.dispatch().render_request(ctx, ());
}

fn render_request<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  ctx.request_render();
}

fn pre_render<Ctx: DefaultContext>(_ctx: &mut Ctx, _unit: ()) {}

fn on_render<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) -> RenderPlan {
  ctx.build_render_plan()
}

fn post_render<Ctx: DefaultContext>(ctx: &mut Ctx, plan: RenderPlan) -> RenderPlan {
  let _ = ctx;
  plan
}

fn pre_render_with_styles<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  mut styles: RenderStyles,
) -> RenderStyles {
  // When picking a cursor to keep/remove, make the focused cursor use the
  // match cursor style so it stands out from the rest.
  if matches!(ctx.pending_input(), Some(PendingInput::CursorPick { .. })) {
    styles.active_cursor = ctx
      .ui_theme()
      .try_get("ui.cursor.match")
      .or_else(|| ctx.ui_theme().try_get("ui.cursor.active"))
      .or_else(|| ctx.ui_theme().try_get("ui.cursor"))
      .unwrap_or(styles.active_cursor);
  }

  styles
}

fn pre_ui<Ctx: DefaultContext>(_ctx: &mut Ctx, _unit: ()) {}

fn on_ui<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) -> UiTree {
  let mut tree = UiTree::new();
  let overlays = crate::command_palette::build_command_palette_ui(ctx);
  tree.overlays.extend(overlays);
  let overlays = crate::completion_menu::build_completion_menu_ui(ctx);
  tree.overlays.extend(overlays);
  let overlays = crate::signature_help::build_signature_help_ui(ctx);
  tree.overlays.extend(overlays);
  let overlays = crate::search_prompt::build_search_prompt_ui(ctx);
  tree.overlays.extend(overlays);
  let overlays = crate::file_picker::build_file_picker_ui(ctx);
  tree.overlays.extend(overlays);
  if ctx.file_picker().active {
    let cursor = byte_to_char_idx(&ctx.file_picker().query, ctx.file_picker().cursor);
    let focus = UiFocus {
      id:     "file_picker_input".to_string(),
      kind:   UiFocusKind::Input,
      cursor: Some(cursor),
    };
    tree.focus = Some(focus.clone());
    ctx.ui_state_mut().set_focus(Some(focus));
  } else if ctx.search_prompt_ref().active {
    let cursor = byte_to_char_idx(
      &ctx.search_prompt_ref().query,
      ctx.search_prompt_ref().cursor,
    );
    let focus = UiFocus {
      id:     "search_prompt_input".to_string(),
      kind:   UiFocusKind::Input,
      cursor: Some(cursor),
    };
    tree.focus = Some(focus.clone());
    ctx.ui_state_mut().set_focus(Some(focus));
  } else if ctx.command_palette().is_open {
    let cursor = if ctx.command_palette().query.is_empty() {
      1
    } else {
      byte_to_char_idx(
        &ctx.command_palette().query,
        ctx.command_palette().query.len(),
      ) + 1
    };
    let focus = UiFocus {
      id:     "command_palette_input".to_string(),
      kind:   UiFocusKind::Input,
      cursor: Some(cursor),
    };
    tree.focus = Some(focus.clone());
    ctx.ui_state_mut().set_focus(Some(focus));
  } else if ctx.completion_menu().active {
    let focus = UiFocus::list("completion_list");
    tree.focus = Some(focus.clone());
    ctx.ui_state_mut().set_focus(Some(focus));
  } else {
    tree.focus = ctx.ui_state().focus().cloned();
  }
  tree
}

fn post_ui<Ctx: DefaultContext>(_ctx: &mut Ctx, tree: UiTree) -> UiTree {
  tree
}

fn pre_ui_event<Ctx: DefaultContext>(_ctx: &mut Ctx, _event: UiEvent) -> UiEventOutcome {
  UiEventOutcome::r#continue()
}

fn on_ui_event<Ctx: DefaultContext>(ctx: &mut Ctx, event: UiEvent) -> UiEventOutcome {
  let focus = ctx.ui_state().focus().cloned();
  let target = event.target.as_deref();
  let focus_id = focus.as_ref().map(|f| f.id.as_str());
  let target_id = target.or(focus_id);

  let is_command_palette = target_id
    .map(|id| id.starts_with("command_palette"))
    .unwrap_or(false);

  let is_completion_menu = target_id
    .map(|id| id.starts_with("completion"))
    .unwrap_or(false);

  let is_search_prompt = target_id
    .map(|id| id.starts_with("search_prompt"))
    .unwrap_or(false);

  let is_file_picker = target_id
    .map(|id| id.starts_with("file_picker"))
    .unwrap_or(false);

  let is_signature_help = target_id
    .map(|id| id.starts_with("signature_help"))
    .unwrap_or(false);

  if is_file_picker {
    match &event.kind {
      UiEventKind::Key(key_event) => {
        if let Some(key_event) = ui_key_event_to_key_event(key_event.clone()) {
          if crate::file_picker::handle_file_picker_key(ctx, key_event) {
            return UiEventOutcome::handled();
          }
        }
      },
      UiEventKind::Activate => {
        crate::file_picker::submit_file_picker(ctx);
        return UiEventOutcome::handled();
      },
      UiEventKind::Dismiss => {
        crate::file_picker::close_file_picker(ctx);
        return UiEventOutcome::handled();
      },
      UiEventKind::Command(_) => {},
    }
  }

  if is_search_prompt {
    match &event.kind {
      UiEventKind::Key(key_event) => {
        if let Some(key_event) = ui_key_event_to_key_event(key_event.clone()) {
          if crate::search_prompt::handle_search_prompt_key(ctx, key_event) {
            return UiEventOutcome::handled();
          }
        }
      },
      UiEventKind::Activate => {
        if crate::search_prompt::handle_search_prompt_key(ctx, KeyEvent {
          key:       Key::Enter,
          modifiers: Modifiers::empty(),
        }) {
          return UiEventOutcome::handled();
        }
      },
      UiEventKind::Dismiss => {
        ctx.search_prompt_mut().clear();
        ctx.request_render();
        return UiEventOutcome::handled();
      },
      _ => {},
    }
  }

  if is_command_palette {
    match &event.kind {
      UiEventKind::Key(key_event) => {
        if let Some(key_event) = ui_key_event_to_key_event(key_event.clone()) {
          if handle_command_prompt_key(ctx, key_event) {
            return UiEventOutcome::handled();
          }
        }
      },
      UiEventKind::Activate => {
        if submit_command_palette_selected(ctx)
          || handle_command_prompt_key(ctx, KeyEvent {
            key:       Key::Enter,
            modifiers: Modifiers::empty(),
          })
        {
          return UiEventOutcome::handled();
        }
      },
      UiEventKind::Dismiss => {
        close_command_palette(ctx);
        return UiEventOutcome::handled();
      },
      UiEventKind::Command(command_line) => {
        let line = command_line.trim().trim_start_matches(':');
        if !line.is_empty() {
          let (command, args) = line
            .split_once(char::is_whitespace)
            .map(|(cmd, rest)| (cmd, rest.trim()))
            .unwrap_or((line, ""));
          let registry = ctx.command_registry_ref() as *const CommandRegistry<Ctx>;
          let result = unsafe { (&*registry).execute(ctx, command, args, CommandEvent::Validate) };
          if let Err(err) = result {
            let message = err.to_string();
            ctx.command_prompt_mut().error = Some(message.clone());
            ctx.push_error("command", message);
          }
          ctx.request_render();
        }
        return UiEventOutcome::handled();
      },
    }
  }

  if is_completion_menu {
    match &event.kind {
      UiEventKind::Key(key_event) => {
        if let Some(key_event) = ui_key_event_to_key_event(key_event.clone()) {
          match key_event.key {
            Key::Up => {
              crate::completion_menu::completion_prev(ctx);
              return UiEventOutcome::handled();
            },
            Key::Down => {
              crate::completion_menu::completion_next(ctx);
              return UiEventOutcome::handled();
            },
            Key::Tab if key_event.modifiers.shift() => {
              crate::completion_menu::completion_prev(ctx);
              return UiEventOutcome::handled();
            },
            Key::Tab => {
              crate::completion_menu::completion_next(ctx);
              return UiEventOutcome::handled();
            },
            Key::Enter | Key::NumpadEnter => {
              crate::completion_menu::completion_accept(ctx);
              return UiEventOutcome::handled();
            },
            Key::PageUp | Key::PageDown => {
              let outcome = keymap_handle_key(ctx, key_event);
              if handle_key_outcome(ctx, outcome) {
                return UiEventOutcome::handled();
              }
            },
            Key::Char('u') if key_event.modifiers.ctrl() => {
              let outcome = keymap_handle_key(ctx, key_event);
              if handle_key_outcome(ctx, outcome) {
                return UiEventOutcome::handled();
              }
            },
            Key::Char('d') if key_event.modifiers.ctrl() => {
              let outcome = keymap_handle_key(ctx, key_event);
              if handle_key_outcome(ctx, outcome) {
                return UiEventOutcome::handled();
              }
            },
            Key::Escape => {
              // Route through keymaps so escape mirrors keyboard behavior.
              let outcome = keymap_handle_key(ctx, key_event);
              if !handle_key_outcome(ctx, outcome) {
                crate::completion_menu::close_completion_menu(ctx);
              }
              return UiEventOutcome::handled();
            },
            _ => {},
          }
        }
      },
      UiEventKind::Activate => {
        crate::completion_menu::completion_accept(ctx);
        return UiEventOutcome::handled();
      },
      UiEventKind::Dismiss => {
        crate::completion_menu::close_completion_menu(ctx);
        return UiEventOutcome::handled();
      },
      UiEventKind::Command(_) => {},
    }
  }

  if is_signature_help {
    match &event.kind {
      UiEventKind::Key(key_event) => {
        if let Some(key_event) = ui_key_event_to_key_event(key_event.clone()) {
          match key_event.key {
            Key::Escape => {
              // Route through keymaps so escape mirrors keyboard behavior.
              let outcome = keymap_handle_key(ctx, key_event);
              if !handle_key_outcome(ctx, outcome) {
                crate::signature_help::close_signature_help(ctx);
              }
              return UiEventOutcome::handled();
            },
            Key::PageUp => {
              crate::signature_help::signature_help_docs_scroll(ctx, -6);
              return UiEventOutcome::handled();
            },
            Key::PageDown => {
              crate::signature_help::signature_help_docs_scroll(ctx, 6);
              return UiEventOutcome::handled();
            },
            _ => {},
          }
        }
      },
      UiEventKind::Dismiss => {
        crate::signature_help::close_signature_help(ctx);
        return UiEventOutcome::handled();
      },
      _ => {},
    }
  }

  UiEventOutcome::r#continue()
}

fn post_ui_event<Ctx: DefaultContext>(_ctx: &mut Ctx, outcome: UiEventOutcome) -> UiEventOutcome {
  outcome
}

fn ui_key_event_to_key_event(event: UiKeyEvent) -> Option<KeyEvent> {
  let key = match event.key {
    UiKey::Char(ch) => Key::Char(ch),
    UiKey::Enter => Key::Enter,
    UiKey::Escape => Key::Escape,
    UiKey::Tab => Key::Tab,
    UiKey::Backspace => Key::Backspace,
    UiKey::Delete => Key::Delete,
    UiKey::Up => Key::Up,
    UiKey::Down => Key::Down,
    UiKey::Left => Key::Left,
    UiKey::Right => Key::Right,
    UiKey::Home => Key::Home,
    UiKey::End => Key::End,
    UiKey::PageUp => Key::PageUp,
    UiKey::PageDown => Key::PageDown,
    UiKey::Unknown(_) => return None,
  };

  let mut modifiers = Modifiers::empty();
  let UiModifiers {
    ctrl,
    alt,
    shift,
    meta,
  } = event.modifiers;
  if ctrl {
    modifiers.insert(Modifiers::CTRL);
  }
  if alt {
    modifiers.insert(Modifiers::ALT);
  }
  if shift {
    modifiers.insert(Modifiers::SHIFT);
  }
  let _ = meta;

  Some(KeyEvent { key, modifiers })
}

fn submit_command_palette_selected<Ctx: DefaultContext>(ctx: &mut Ctx) -> bool {
  let palette = ctx.command_palette();
  if !palette.is_open {
    return false;
  }

  let filtered = crate::command_palette::command_palette_filtered_indices(palette);
  let selected = palette.selected.filter(|sel| filtered.contains(sel));

  let Some(item_idx) = selected else {
    return false;
  };

  let command_name = palette
    .items
    .get(item_idx)
    .map(|item| item.title.clone())
    .unwrap_or_default();

  if command_name.is_empty() {
    return false;
  }

  let registry = ctx.command_registry_ref() as *const CommandRegistry<Ctx>;
  let result = unsafe { (&*registry).execute(ctx, &command_name, "", CommandEvent::Validate) };

  match result {
    Ok(()) => {
      close_command_palette(ctx);
      true
    },
    Err(err) => {
      let message = err.to_string();
      ctx.command_prompt_mut().error = Some(message.clone());
      ctx.push_error("command_palette", message);
      ctx.request_render();
      true
    },
  }
}

fn close_command_palette<Ctx: DefaultContext>(ctx: &mut Ctx) {
  ctx.set_mode(Mode::Normal);
  ctx.command_prompt_mut().clear();
  let palette = ctx.command_palette_mut();
  palette.is_open = false;
  palette.query.clear();
  palette.selected = None;
  palette.prompt_text = None;
  ctx.request_render();
}

fn on_render_with_styles<Ctx: DefaultContext>(ctx: &mut Ctx, styles: RenderStyles) -> RenderPlan {
  ctx.build_render_plan_with_styles(styles)
}

fn insert_char<Ctx: DefaultContext>(ctx: &mut Ctx, ch: char) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();

  let pairs = AutoPairs::default();
  if let Ok(Some(tx)) = auto_pairs::hook(doc.text(), &selection, ch, &pairs) {
    let _ = ctx.apply_transaction(&tx);
    return;
  }

  let mut text = Tendril::new();
  text.push(ch);

  let cursors = selection.clone().cursors(doc.text().slice(..));
  let Ok(tx) = Transaction::insert(doc.text(), &cursors, text) else {
    return;
  };

  let _ = ctx.apply_transaction(&tx);
}

fn delete_char<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let pairs = AutoPairs::default();
  if let Ok(Some(tx)) = auto_pairs::delete_hook(doc.text(), &selection, &pairs) {
    let _ = ctx.apply_transaction(&tx);
    return;
  }

  let tab_width: usize = 4;
  let indent_width = doc.indent_style().indent_width(tab_width);

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    let pos = range.cursor(slice);
    if pos == 0 {
      return (pos, pos);
    }

    let line_start_pos = slice.line_to_char(range.cursor_line(slice));
    let fragment: Cow<'_, str> = Cow::from(slice.slice(line_start_pos..pos));

    if !fragment.is_empty() && fragment.chars().all(|ch| ch == ' ' || ch == '\t') {
      if slice.get_char(pos.saturating_sub(1)) == Some('\t') {
        return (nth_prev_grapheme_boundary(slice, pos, 1), pos);
      }

      let width: usize = fragment
        .chars()
        .map(|ch| if ch == '\t' { tab_width } else { 1 })
        .sum();
      let mut drop = width % indent_width;
      if drop == 0 {
        drop = indent_width;
      }

      let mut chars = fragment.chars().rev();
      let mut start = pos;
      for _ in 0..drop {
        match chars.next() {
          Some(' ') => start = start.saturating_sub(1),
          _ => break,
        }
      }
      (start, pos)
    } else {
      let count = 1;
      (nth_prev_grapheme_boundary(slice, pos, count), pos)
    }
  });

  let Ok(tx) = tx else {
    return;
  };

  let _ = ctx.apply_transaction(&tx);
}

fn delete_char_forward<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);
  let count = count.max(1);

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    let pos = range.cursor(slice);
    (pos, nth_next_grapheme_boundary(slice, pos, count))
  });

  let Ok(tx) = tx else {
    return;
  };

  let _ = ctx.apply_transaction(&tx);
}

fn delete_word_backward<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);
  let count = count.max(1);

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    let cursor_pos = range.cursor(slice);
    if cursor_pos == 0 {
      return (0, 0);
    }
    let target = movement::move_prev_word_start(slice, *range, count);
    (target.from(), cursor_pos)
  });

  let Ok(tx) = tx else {
    return;
  };

  let _ = ctx.apply_transaction(&tx);
}

fn delete_word_forward<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);
  let count = count.max(1);
  let text_len = slice.len_chars();

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    let cursor_pos = range.cursor(slice);
    if cursor_pos >= text_len {
      return (cursor_pos, cursor_pos);
    }
    let target = movement::move_next_word_end(slice, *range, count);
    (cursor_pos, target.to())
  });

  let Ok(tx) = tx else {
    return;
  };

  let _ = ctx.apply_transaction(&tx);
}

fn kill_to_line_start<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    let cursor_pos = range.cursor(slice);
    let line = range.cursor_line(slice);
    let line_start = slice.line_to_char(line);

    let head = if cursor_pos == line_start && line != 0 {
      // At line start, delete back to end of previous line (join lines)
      let prev_line = slice.line(line - 1);
      let prev_line_start = slice.line_to_char(line - 1);
      // Line end is before the newline character
      prev_line_start + prev_line.len_chars().saturating_sub(1)
    } else if let Some(first_non_ws) = slice.line(line).first_non_whitespace_char() {
      let first_non_ws_pos = line_start + first_non_ws;
      if first_non_ws_pos < cursor_pos {
        // Cursor is after first non-whitespace, delete to first non-whitespace
        first_non_ws_pos
      } else {
        // Delete to line start
        line_start
      }
    } else {
      // Blank line, delete to line start
      line_start
    };

    (head, cursor_pos)
  });

  let Ok(tx) = tx else {
    return;
  };

  let _ = ctx.apply_transaction(&tx);
}

fn kill_to_line_end<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    let cursor_pos = range.cursor(slice);
    let line = range.cursor_line(slice);
    let line_end_pos = line_end_char_index(&slice, line);

    if cursor_pos == line_end_pos {
      // Cursor is at line end, delete the newline (join with next line)
      let next_line_start = slice.line_to_char(line + 1);
      (cursor_pos, next_line_start)
    } else {
      // Delete from cursor to line end
      (cursor_pos, line_end_pos)
    }
  });

  let Ok(tx) = tx else {
    return;
  };

  let _ = ctx.apply_transaction(&tx);
}

fn insert_tab<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();

  let indent = Tendril::from(doc.indent_style().as_str());
  let cursors = selection.cursors(doc.text().slice(..));

  let Ok(tx) = Transaction::insert(doc.text(), &cursors, indent) else {
    return;
  };

  let _ = ctx.apply_transaction(&tx);
}

fn insert_newline<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let contents = doc.text();
  let text = contents.slice(..);
  let selection = doc.selection().clone();
  let line_ending = doc.line_ending().as_str();
  let indent_unit = doc.indent_style().as_str();

  let mut ranges = SmallVec::with_capacity(selection.len());
  let mut global_offs: isize = 0;
  let mut new_text = String::new();
  let mut last_pos = 0;
  let pairs = AutoPairs::default();

  let tx = Transaction::change_by_selection(contents, &selection, |range| {
    let pos = range.cursor(text);
    let current_line = text.char_to_line(pos);
    let line_start = text.line_to_char(current_line);
    let mut chars_deleted = 0usize;

    let (from, to, local_offs) =
      if let Some(idx) = text.slice(line_start..pos).last_non_whitespace_char() {
        let first_trailing_whitespace_char = (line_start + idx + 1).clamp(last_pos, pos);
        last_pos = pos;
        chars_deleted = pos - first_trailing_whitespace_char;

        let line = text.line(current_line);
        let indent = match line.first_non_whitespace_char() {
          Some(pos) => line.slice(..pos).to_string(),
          None => String::new(),
        };

        let prev = if pos == 0 {
          ' '
        } else {
          contents.char(pos - 1)
        };
        let curr = contents.get_char(pos).unwrap_or(' ');

        let on_auto_pair = pairs
          .pairs()
          .iter()
          .any(|pair| pair.open_last_char() == Some(prev) && pair.close_first_char() == Some(curr));

        if on_auto_pair {
          let inner_indent = indent.clone() + indent_unit;
          new_text.reserve_exact(line_ending.len() * 2 + indent.len() + inner_indent.len());
          new_text.push_str(line_ending);
          new_text.push_str(&inner_indent);
          let local_offs = new_text.chars().count();
          new_text.push_str(line_ending);
          new_text.push_str(&indent);
          (
            first_trailing_whitespace_char,
            pos,
            local_offs as isize - chars_deleted as isize,
          )
        } else {
          new_text.reserve_exact(line_ending.len() + indent.len());
          new_text.push_str(line_ending);
          new_text.push_str(&indent);
          (
            first_trailing_whitespace_char,
            pos,
            new_text.chars().count() as isize - chars_deleted as isize,
          )
        }
      } else {
        new_text.push_str(line_ending);
        (line_start, line_start, new_text.chars().count() as isize)
      };

    let new_range = if range.cursor(text) > range.anchor {
      Range::new(
        (range.anchor as isize + global_offs) as usize,
        (range.head as isize + local_offs + global_offs) as usize,
      )
    } else {
      Range::new(
        (range.anchor as isize + local_offs + global_offs) as usize,
        (range.head as isize + local_offs + global_offs) as usize,
      )
    };

    ranges.push(new_range);
    global_offs += new_text.chars().count() as isize - chars_deleted as isize;

    let tendril = Tendril::from(new_text.as_str());
    new_text.clear();

    (from, to, Some(tendril))
  });

  let Ok(tx) = tx else {
    return;
  };

  let cursor_ids: SmallVec<[CursorId; 1]> = selection.cursor_ids().iter().copied().collect();
  let new_selection = Selection::new_with_ids(ranges, cursor_ids).unwrap_or_else(|_| selection);
  let tx = tx.with_selection(new_selection);
  let _ = ctx.apply_transaction(&tx);
}

fn goto_line_start<Ctx: DefaultContext>(ctx: &mut Ctx, extend: bool) {
  let extend = extend || ctx.mode() == Mode::Select;
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let new_selection = selection.transform(|range| {
    let line = range.cursor_line(slice);
    let pos = slice.line_to_char(line);
    range.put_cursor(slice, pos, extend)
  });

  let _ = doc.set_selection(new_selection);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowAlign {
  Top,
  Center,
  Bottom,
}

fn goto_window<Ctx: DefaultContext>(ctx: &mut Ctx, align: WindowAlign, count: usize) {
  let view = ctx.editor_ref().view();
  let height = view.viewport.height as usize;
  if height == 0 {
    return;
  }

  let count = count.max(1).saturating_sub(1);
  let scrolloff = ctx.scrolloff().min(height.saturating_sub(1) / 2);
  let last_row = height.saturating_sub(1);
  let min_row = scrolloff;
  let max_row = last_row.saturating_sub(scrolloff);

  let row_in_window = match align {
    WindowAlign::Top => scrolloff.saturating_add(count),
    WindowAlign::Center => last_row / 2,
    WindowAlign::Bottom => last_row.saturating_sub(scrolloff.saturating_add(count)),
  };
  let row_in_window = row_in_window.clamp(min_row, max_row);
  let target_line = view.scroll.row.saturating_add(row_in_window);
  let extend = ctx.mode() == Mode::Select;

  let doc = ctx.editor().document_mut();
  let slice = doc.text().slice(..);
  let target = char_idx_at_coords(slice, Position::new(target_line, 0));
  let selection = doc.selection().clone();
  let new_selection = selection.transform(|range| range.put_cursor(slice, target, extend));
  let _ = doc.set_selection(new_selection);
}

fn rotate_view<Ctx: DefaultContext>(ctx: &mut Ctx) {
  if !ctx.editor().rotate_active_pane(true) {
    ctx.push_warning("window", "no other view to rotate");
    return;
  }
  ctx.request_render();
}

fn split_view<Ctx: DefaultContext>(ctx: &mut Ctx, axis: SplitAxis) {
  if !ctx.editor().split_active_pane(axis) {
    ctx.push_warning("window", "failed to split view");
    return;
  }
  ctx.request_render();
}

fn transpose_view<Ctx: DefaultContext>(ctx: &mut Ctx) {
  if !ctx.editor().transpose_active_pane_branch() {
    ctx.push_warning("window", "no split branch to transpose");
    return;
  }
  ctx.request_render();
}

fn close_view<Ctx: DefaultContext>(ctx: &mut Ctx) {
  if !ctx.editor().close_active_pane() {
    ctx.push_warning("window", "cannot close the last view");
    return;
  }
  ctx.request_render();
}

fn only_view<Ctx: DefaultContext>(ctx: &mut Ctx) {
  if !ctx.editor().only_active_pane() {
    ctx.push_warning("window", "already in a single view");
    return;
  }
  ctx.request_render();
}

fn jump_view<Ctx: DefaultContext>(ctx: &mut Ctx, direction: PaneDirection) {
  if !ctx.editor().jump_active_pane(direction) {
    ctx.push_warning("window", "no view in that direction");
    return;
  }
  ctx.request_render();
}

fn swap_view<Ctx: DefaultContext>(ctx: &mut Ctx, direction: PaneDirection) {
  if !ctx.editor().swap_active_pane(direction) {
    ctx.push_warning("window", "no view to swap in that direction");
    return;
  }
  ctx.request_render();
}

fn goto_file_split<Ctx: DefaultContext>(ctx: &mut Ctx, axis: SplitAxis) {
  crate::file_picker::open_file_picker_with_split(ctx, Some(axis));
}

fn split_new_scratch<Ctx: DefaultContext>(ctx: &mut Ctx, axis: SplitAxis) {
  if !ctx.editor().split_active_pane(axis) {
    ctx.push_warning("window", "failed to split view");
    return;
  }

  let viewport = ctx.editor_ref().view().viewport;
  let view = ViewState::new(viewport, Position::new(0, 0));
  let _ = ctx.editor().open_buffer(Rope::new(), view, None);
  ctx.set_file_path(None);
  ctx.request_render();
}

fn goto_last_modification<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let Some(pos) = ctx.editor_ref().last_modification_position() else {
    return;
  };
  let extend = ctx.mode() == Mode::Select;
  let doc = ctx.editor().document_mut();
  let slice = doc.text().slice(..);
  let selection = doc.selection().clone();
  let new_selection = selection.transform(|range| range.put_cursor(slice, pos, extend));
  let _ = doc.set_selection(new_selection);
}

const WORD_JUMP_LABEL_ALPHABET: &str = "abcdefghijklmnopqrstuvwxyz";

fn active_or_first_range(editor: &Editor) -> Option<Range> {
  let doc = editor.document();
  let selection = doc.selection();
  if let Some(active_cursor) = editor.view().active_cursor
    && let Some(range) = selection.range_by_id(active_cursor)
  {
    return Some(*range);
  }
  selection.ranges().first().copied()
}

fn active_or_fallback_pick(editor: &Editor, fallback: CursorPick) -> CursorPick {
  let selection = editor.document().selection();
  if let Some(active_cursor) = editor.view().active_cursor
    && selection.range_by_id(active_cursor).is_some()
  {
    return CursorPick::Id(active_cursor);
  }
  fallback
}

fn goto_word<Ctx: DefaultContext>(ctx: &mut Ctx, extend: bool) {
  let targets = collect_word_jump_targets(ctx);
  if targets.is_empty() {
    ctx.clear_word_jump_annotations();
    return;
  }

  set_word_jump_annotations(ctx, &targets);
  ctx.set_pending_input(Some(PendingInput::WordJump {
    extend,
    first: None,
    targets,
  }));
}

fn collect_word_jump_targets<Ctx: DefaultContext>(ctx: &Ctx) -> Vec<WordJumpTarget> {
  let view = ctx.editor_ref().view();
  let doc = ctx.editor_ref().document();
  let text = doc.text().slice(..);
  let alphabet: Vec<char> = WORD_JUMP_LABEL_ALPHABET.chars().collect();
  if alphabet.is_empty() || view.viewport.height == 0 {
    return Vec::new();
  }
  let alphabet_len = alphabet.len();
  let jump_label_limit = alphabet_len * alphabet_len;

  if text.len_lines() == 0 {
    return Vec::new();
  }

  let start_line = view.scroll.row.min(text.len_lines().saturating_sub(1));
  let end_line_exclusive = (start_line + view.viewport.height as usize).min(text.len_lines());
  let start = text.line_to_char(start_line);
  let end = text.line_to_char(end_line_exclusive);
  let active_range = active_or_first_range(ctx.editor_ref()).unwrap_or_else(|| Range::point(start));
  let cursor = active_range.cursor(text);

  let mut words = Vec::with_capacity(jump_label_limit);
  let mut cursor_fwd = Range::point(cursor);
  let mut cursor_rev = Range::point(cursor);
  if text.get_char(cursor).is_some_and(|c| !c.is_whitespace()) {
    let cursor_word_end = movement::move_next_word_end(text, cursor_fwd, 1);
    if cursor_word_end.anchor == cursor {
      cursor_fwd = cursor_word_end;
    }
    let cursor_word_start = movement::move_prev_word_start(text, cursor_rev, 1);
    if cursor_word_start.anchor == next_grapheme_boundary(text, cursor) {
      cursor_rev = cursor_word_start;
    }
  }

  'outer: loop {
    let mut changed = false;
    while cursor_fwd.head < end {
      cursor_fwd = movement::move_next_word_end(text, cursor_fwd, 1);
      let add_label = text
        .slice(..cursor_fwd.head)
        .graphemes_rev()
        .take(2)
        .take_while(|g| g.chars().all(char_is_word))
        .count()
        == 2;
      if !add_label {
        continue;
      }
      changed = true;
      cursor_fwd.anchor += text
        .chars_at(cursor_fwd.anchor)
        .take_while(|&c| !char_is_word(c))
        .count();
      words.push(cursor_fwd);
      if words.len() == jump_label_limit {
        break 'outer;
      }
      break;
    }

    while cursor_rev.head > start {
      cursor_rev = movement::move_prev_word_start(text, cursor_rev, 1);
      let add_label = text
        .slice(cursor_rev.head..)
        .graphemes()
        .take(2)
        .take_while(|g| g.chars().all(char_is_word))
        .count()
        == 2;
      if !add_label {
        continue;
      }
      changed = true;
      cursor_rev.anchor -= text
        .chars_at(cursor_rev.anchor)
        .reversed()
        .take_while(|&c| !char_is_word(c))
        .count();
      words.push(cursor_rev);
      if words.len() == jump_label_limit {
        break 'outer;
      }
      break;
    }

    if !changed {
      break;
    }
  }

  words
    .into_iter()
    .enumerate()
    .map(|(idx, range)| {
      WordJumpTarget {
        label: [alphabet[idx / alphabet_len], alphabet[idx % alphabet_len]],
        range,
      }
    })
    .collect()
}

fn set_word_jump_annotations<Ctx: DefaultContext>(ctx: &mut Ctx, targets: &[WordJumpTarget]) {
  let text = ctx.editor_ref().document().text().slice(..);
  let mut overlay = Vec::new();

  for target in targets {
    let from = target.range.from();
    overlay.push(Overlay::new(from, target.label[0].to_string()));
    overlay.push(Overlay::new(
      next_grapheme_boundary(text, from),
      target.label[1].to_string(),
    ));
  }

  overlay.sort_by_key(|annotation| annotation.char_idx);
  ctx.set_word_jump_annotations(Vec::new(), overlay);
}

fn apply_word_jump_target<Ctx: DefaultContext>(ctx: &mut Ctx, mut range: Range, extend: bool) {
  if extend {
    let active_range =
      active_or_first_range(ctx.editor_ref()).unwrap_or_else(|| Range::point(range.anchor));
    range = if range.anchor < range.head {
      let from = active_range.from();
      let anchor = if range.anchor < from {
        range.anchor
      } else {
        from
      };
      Range::new(anchor, range.head)
    } else {
      let to = active_range.to();
      let anchor = if range.anchor > to { range.anchor } else { to };
      Range::new(anchor, range.head)
    };
  } else {
    range = range.with_direction(MoveDir::Forward);
  }

  let _ = ctx.editor().document_mut().set_selection(range.into());
}

fn goto_prev_diag<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let diagnostics = ctx.active_diagnostic_ranges();
  let Some(next) = ({
    let doc = ctx.editor_ref().document();
    let text = doc.text().slice(..);
    let cursor = active_or_first_range(ctx.editor_ref())
      .unwrap_or_else(|| Range::point(0))
      .cursor(text);
    diagnostics
      .iter()
      .rev()
      .find(|range| range.from() < cursor)
      .copied()
      .map(|range| Selection::single(range.to(), range.from()))
  }) else {
    return;
  };
  let _ = ctx.editor().document_mut().set_selection(next);
}

fn goto_first_diag<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let diagnostics = ctx.active_diagnostic_ranges();
  let Some(range) = diagnostics.first().copied() else {
    return;
  };
  let _ = ctx
    .editor()
    .document_mut()
    .set_selection(Selection::single(range.from(), range.to()));
}

fn goto_next_diag<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let diagnostics = ctx.active_diagnostic_ranges();
  let Some(next) = ({
    let doc = ctx.editor_ref().document();
    let text = doc.text().slice(..);
    let cursor = active_or_first_range(ctx.editor_ref())
      .unwrap_or_else(|| Range::point(0))
      .cursor(text);
    diagnostics
      .iter()
      .find(|range| range.from() > cursor)
      .copied()
      .map(|range| Selection::single(range.from(), range.to()))
  }) else {
    return;
  };
  let _ = ctx.editor().document_mut().set_selection(next);
}

fn goto_last_diag<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let diagnostics = ctx.active_diagnostic_ranges();
  let Some(range) = diagnostics.last().copied() else {
    return;
  };
  let _ = ctx
    .editor()
    .document_mut()
    .set_selection(Selection::single(range.from(), range.to()));
}

fn goto_change<Ctx: DefaultContext>(ctx: &mut Ctx, direction: Direction) {
  let Some(change_ranges) = ctx.change_hunk_ranges() else {
    ctx.push_warning("goto", "Diff is not available in current buffer");
    return;
  };
  if change_ranges.is_empty() {
    return;
  }

  let select_mode = ctx.mode() == Mode::Select;
  let next = {
    let doc = ctx.editor_ref().document();
    let text = doc.text().slice(..);
    let selection = doc.selection().clone();
    selection.transform(|range| {
      let cursor_line = range.cursor_line(text);
      let target = match direction {
        Direction::Forward => {
          change_ranges
            .iter()
            .find(|candidate| text.char_to_line(candidate.from()) > cursor_line)
        },
        Direction::Backward => {
          change_ranges
            .iter()
            .rev()
            .find(|candidate| text.char_to_line(candidate.from()) < cursor_line)
        },
        _ => None,
      };
      let Some(target) = target.copied() else {
        return range;
      };

      if select_mode {
        let head = if target.head < range.anchor {
          target.anchor
        } else {
          target.head
        };
        Range::new(range.anchor, head)
      } else {
        let movement_direction = match direction {
          Direction::Forward => MoveDir::Forward,
          Direction::Backward => MoveDir::Backward,
          _ => MoveDir::Forward,
        };
        target.with_direction(movement_direction)
      }
    })
  };

  let _ = ctx.editor().document_mut().set_selection(next);
}

fn goto_first_change<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let Some(change_ranges) = ctx.change_hunk_ranges() else {
    ctx.push_warning("goto", "Diff is not available in current buffer");
    return;
  };
  let Some(range) = change_ranges.first().copied() else {
    return;
  };
  let _ = ctx
    .editor()
    .document_mut()
    .set_selection(Selection::single(range.from(), range.to()));
}

fn goto_last_change<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let Some(change_ranges) = ctx.change_hunk_ranges() else {
    ctx.push_warning("goto", "Diff is not available in current buffer");
    return;
  };
  let Some(range) = change_ranges.last().copied() else {
    return;
  };
  let _ = ctx
    .editor()
    .document_mut()
    .set_selection(Selection::single(range.from(), range.to()));
}

fn goto_ts_object<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  object_name: &'static str,
  direction: Direction,
) {
  let move_direction = match direction {
    Direction::Forward => MoveDir::Forward,
    Direction::Backward => MoveDir::Backward,
    _ => return,
  };
  let Some(selection) = ({
    let doc = ctx.editor_ref().document();
    doc
      .syntax()
      .zip(ctx.syntax_loader())
      .map(|(syntax, loader)| {
        let select_mode = ctx.mode() == Mode::Select;
        let text = doc.text().slice(..);
        let root = syntax.tree().root_node();
        doc.selection().clone().transform(|range| {
          let new_range = movement::goto_treesitter_object(
            text,
            range,
            object_name,
            move_direction,
            &root,
            syntax,
            loader,
            1,
          );
          if select_mode {
            let head = if new_range.head < range.anchor {
              new_range.anchor
            } else {
              new_range.head
            };
            Range::new(range.anchor, head)
          } else {
            new_range.with_direction(move_direction)
          }
        })
      })
  }) else {
    ctx.push_warning("goto", "Syntax-tree is not available in current buffer");
    return;
  };

  let _ = ctx.editor().document_mut().set_selection(selection);
}

fn goto_paragraph<Ctx: DefaultContext>(ctx: &mut Ctx, direction: Direction) {
  let movement = if ctx.mode() == Mode::Select {
    Movement::Extend
  } else {
    Movement::Move
  };
  let next = {
    let doc = ctx.editor_ref().document();
    let text = doc.text().slice(..);
    let selection = doc.selection().clone();
    selection.transform(|range| {
      match direction {
        Direction::Forward => movement::move_next_paragraph(text, range, 1, movement),
        Direction::Backward => movement::move_prev_paragraph(text, range, 1, movement),
        _ => range,
      }
    })
  };
  let _ = ctx.editor().document_mut().set_selection(next);
}

fn split_selection_on_newline<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let next = {
    let doc = ctx.editor_ref().document();
    let text = doc.text().slice(..);
    split_on_newline(text, doc.selection())
  };
  match next {
    Ok(selection) => {
      let _ = ctx.editor().document_mut().set_selection(selection);
    },
    Err(err) => {
      ctx.push_error(
        "selection",
        format!("failed to split selection on newline: {err}"),
      );
    },
  }
}

fn merge_selections<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let selection = ctx
    .editor_ref()
    .document()
    .selection()
    .clone()
    .merge_ranges();
  let _ = ctx.editor().document_mut().set_selection(selection);
}

fn merge_consecutive_selections<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let selection = ctx
    .editor_ref()
    .document()
    .selection()
    .clone()
    .merge_consecutive_ranges();
  let _ = ctx.editor().document_mut().set_selection(selection);
}

fn split_selection<Ctx: DefaultContext>(ctx: &mut Ctx) {
  crate::search_prompt::open_split_selection_prompt(ctx);
}

fn join_selections<Ctx: DefaultContext>(ctx: &mut Ctx) {
  join_selections_impl(ctx, false);
}

fn join_selections_space<Ctx: DefaultContext>(ctx: &mut Ctx) {
  join_selections_impl(ctx, true);
}

fn keep_selections<Ctx: DefaultContext>(ctx: &mut Ctx) {
  crate::search_prompt::open_keep_selections_prompt(ctx);
}

fn remove_selections<Ctx: DefaultContext>(ctx: &mut Ctx) {
  crate::search_prompt::open_remove_selections_prompt(ctx);
}

fn align_selections<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let tx = (|| -> Result<Option<Transaction>, String> {
    let doc = ctx.editor_ref().document();
    let text = doc.text().slice(..);
    let selection = doc.selection();
    let text_fmt = ctx.text_format();
    let mut annotations = ctx.text_annotations();

    let mut column_widths: Vec<Vec<(usize, usize)>> = Vec::new();
    let mut last_line = text.len_lines().saturating_add(1);
    let mut col_idx = 0usize;

    for range in selection.iter() {
      let Some(coords) = visual_pos_at_char(text, &text_fmt, &mut annotations, range.head) else {
        return Ok(None);
      };
      let Some(anchor_coords) = visual_pos_at_char(text, &text_fmt, &mut annotations, range.anchor)
      else {
        return Ok(None);
      };

      if coords.row != anchor_coords.row {
        return Err("align cannot work with multi line selections".to_string());
      }

      col_idx = if coords.row == last_line {
        col_idx + 1
      } else {
        0
      };

      if col_idx >= column_widths.len() {
        column_widths.push(Vec::new());
      }
      column_widths[col_idx].push((range.from(), coords.col));
      last_line = coords.row;
    }

    let mut changes: Vec<(usize, usize, Option<Tendril>)> =
      Vec::with_capacity(selection.ranges().len());
    let mut offsets = vec![0usize; column_widths.first().map(Vec::len).unwrap_or(0)];

    for column in column_widths {
      let max_col = column
        .iter()
        .enumerate()
        .map(|(row, (_, cursor_col))| *cursor_col + offsets.get(row).copied().unwrap_or(0))
        .max()
        .unwrap_or(0);

      for (row, (insert_pos, last_col)) in column.into_iter().enumerate() {
        if row >= offsets.len() {
          offsets.resize(row + 1, 0);
        }

        let insert_count = max_col.saturating_sub(last_col + offsets[row]);
        if insert_count == 0 {
          continue;
        }

        offsets[row] += insert_count;
        changes.push((
          insert_pos,
          insert_pos,
          Some(" ".repeat(insert_count).into()),
        ));
      }
    }

    if changes.is_empty() {
      Ok(None)
    } else {
      changes.sort_unstable_by_key(|(from, ..)| *from);
      Transaction::change(doc.text(), changes.into_iter())
        .map(Some)
        .map_err(|err| format!("failed to build align transaction: {err}"))
    }
  })();

  let tx = match tx {
    Ok(tx) => tx,
    Err(err) => {
      ctx.push_error("align", err);
      return;
    },
  };

  if let Some(tx) = tx
    && !ctx.apply_transaction(&tx)
  {
    ctx.push_error("align", "failed to apply align transaction");
    return;
  }

  if ctx.mode() == Mode::Select {
    ctx.set_mode(Mode::Normal);
  }
}

fn trim_selections<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let trimmed = {
    let doc = ctx.editor_ref().document();
    let text = doc.text().slice(..);
    let mut ranges = SmallVec::<[Range; 1]>::new();
    let mut cursor_ids = SmallVec::<[CursorId; 1]>::new();

    for (cursor_id, range) in doc.selection().iter_with_ids() {
      if range.is_empty() || range.slice(text).chars().all(|ch| ch.is_whitespace()) {
        continue;
      }

      let mut start = range.from();
      let mut end = range.to();
      start = movement::skip_while(text, start, |ch| ch.is_whitespace()).unwrap_or(start);
      end = movement::backwards_skip_while(text, end, |ch| ch.is_whitespace()).unwrap_or(end);

      ranges.push(Range::new(start, end).with_direction(range.direction()));
      cursor_ids.push(cursor_id);
    }

    Selection::new_with_ids(ranges, cursor_ids).ok()
  };

  if let Some(selection) = trimmed {
    let _ = ctx.editor().document_mut().set_selection(selection);
    return;
  }

  let collapsed = {
    let editor = ctx.editor_ref();
    let pick = active_or_fallback_pick(editor, CursorPick::First);
    let doc = editor.document();
    let text = doc.text().slice(..);
    let collapsed = doc
      .selection()
      .clone()
      .transform(|range| Range::point(range.cursor(text)));
    collapsed.collapse(pick).ok()
  };

  if let Some(selection) = collapsed {
    let _ = ctx.editor().document_mut().set_selection(selection);
  }
}

fn keep_active_selection<Ctx: DefaultContext>(ctx: &mut Ctx) {
  enter_cursor_pick_mode(ctx, false);
}

fn remove_active_selection<Ctx: DefaultContext>(ctx: &mut Ctx) {
  enter_cursor_pick_mode(ctx, true);
}

fn enter_cursor_pick_mode<Ctx: DefaultContext>(ctx: &mut Ctx, remove: bool) {
  let Some((original_active, candidates, index)) = cursor_pick_candidates(ctx.editor_ref()) else {
    ctx.push_warning("selection", "no cursor available");
    return;
  };

  ctx.editor().view_mut().active_cursor = Some(candidates[index]);
  ctx.set_pending_input(Some(PendingInput::CursorPick {
    remove,
    original_active,
    candidates,
    index,
  }));
  ctx.push_info(
    "selection",
    if remove {
      "remove cursor: use arrows to choose, enter to confirm, esc to cancel"
    } else {
      "collapse cursor: use arrows to choose, enter to confirm, esc to cancel"
    },
  );
}

fn cursor_pick_candidates(editor: &Editor) -> Option<(Option<CursorId>, Vec<CursorId>, usize)> {
  let doc = editor.document();
  let view = editor.view();
  let selection = doc.selection();
  if selection.cursor_ids().is_empty() {
    return None;
  }

  let text = doc.text().slice(..);
  let row_start = view.scroll.row;
  let row_end = row_start.saturating_add(view.viewport.height as usize);

  let mut visible = Vec::new();
  if row_end > row_start {
    for (cursor_id, range) in selection.iter_with_ids() {
      let line = text.char_to_line(range.cursor(text));
      if line >= row_start && line < row_end {
        visible.push(cursor_id);
      }
    }
  }

  let candidates = if visible.is_empty() {
    selection.cursor_ids().to_vec()
  } else {
    visible
  };
  let original_active = view.active_cursor;
  let index = original_active
    .and_then(|id| candidates.iter().position(|candidate| *candidate == id))
    .unwrap_or(0);

  Some((original_active, candidates, index))
}

fn apply_cursor_pick<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  target: CursorId,
  remove: bool,
) -> Result<(), String> {
  let editor = ctx.editor();
  let doc = editor.document_mut();
  let selection = doc.selection().clone();

  if remove {
    let Some(index) = selection.index_of(target) else {
      return Err("selected cursor is no longer available".to_string());
    };
    if selection.ranges().len() <= 1 {
      return Err("cannot remove the last cursor".to_string());
    }

    let next_selection = selection.remove(index).map_err(|err| err.to_string())?;
    let next_active = next_selection
      .cursor_ids()
      .get(index.min(next_selection.cursor_ids().len().saturating_sub(1)))
      .copied();
    doc
      .set_selection(next_selection)
      .map_err(|err| err.to_string())?;
    editor.view_mut().active_cursor = next_active;
  } else {
    let next_selection = selection
      .collapse(CursorPick::Id(target))
      .map_err(|err| err.to_string())?;
    doc
      .set_selection(next_selection)
      .map_err(|err| err.to_string())?;
    editor.view_mut().active_cursor = Some(target);
  }

  Ok(())
}

fn join_selections_impl<Ctx: DefaultContext>(ctx: &mut Ctx, select_space: bool) {
  use movement::skip_while;

  let tx = {
    let doc = ctx.editor_ref().document();
    let text = doc.text();
    let slice = text.slice(..);
    let mut comment_tokens: Vec<&str> = match (ctx.syntax_loader(), doc.syntax()) {
      (Some(loader), Some(syntax)) if syntax.root_language().idx() < loader.languages().len() => {
        loader
          .language(syntax.root_language())
          .config()
          .syntax
          .comment_tokens
          .as_deref()
          .unwrap_or(&[])
          .iter()
          .map(String::as_str)
          .collect()
      },
      _ => Vec::new(),
    };
    // Sort by length so longer comment markers (e.g. ///) match before shorter ones
    // (//).
    comment_tokens.sort_unstable_by_key(|token| std::cmp::Reverse(token.len()));

    let mut changes = Vec::new();
    for selection in doc.selection().iter() {
      let (start, mut end) = selection.line_range(slice);
      if start == end {
        end = (end + 1).min(text.len_lines().saturating_sub(1));
      }

      let lines = start..end;
      changes.reserve(lines.len());

      let first_line_idx = slice.line_to_char(start);
      let first_line_idx =
        skip_while(slice, first_line_idx, |ch| matches!(ch, ' ' | '\t')).unwrap_or(first_line_idx);
      let first_line = slice.slice(first_line_idx..);
      let mut current_comment_token = comment_tokens
        .iter()
        .find(|token| first_line.starts_with(token))
        .copied();

      for line in lines {
        let from = line_end_char_index(&slice, line);
        let mut to = text.line_to_char(line + 1);
        to = skip_while(slice, to, |ch| matches!(ch, ' ' | '\t')).unwrap_or(to);

        let slice_from_end = slice.slice(to..);
        if let Some(token) = comment_tokens
          .iter()
          .find(|token| slice_from_end.starts_with(token))
          .copied()
        {
          if Some(token) == current_comment_token {
            to += token.chars().count();
            to = skip_while(slice, to, |ch| matches!(ch, ' ' | '\t')).unwrap_or(to);
          } else {
            current_comment_token = Some(token);
          }
        }

        let separator = if to == line_end_char_index(&slice, line + 1) {
          None
        } else {
          Some(Tendril::from(" "))
        };
        changes.push((from, to, separator));
      }
    }

    if changes.is_empty() {
      return;
    }

    changes.sort_unstable_by_key(|(from, _to, _text)| *from);
    changes.dedup();

    if select_space {
      let mut offset = 0usize;
      let ranges: SmallVec<[Range; 1]> = changes
        .iter()
        .filter_map(|change| {
          if change.2.is_some() {
            let range = Range::point(change.0.saturating_sub(offset));
            offset += change.1.saturating_sub(change.0).saturating_sub(1);
            Some(range)
          } else {
            offset += change.1.saturating_sub(change.0);
            None
          }
        })
        .collect();

      match Transaction::change(text, changes.into_iter()) {
        Ok(tx) => {
          if ranges.is_empty() {
            Ok(tx)
          } else {
            Selection::new(ranges)
              .map(|selection| tx.with_selection(selection))
              .map_err(|err| err.to_string())
          }
        },
        Err(err) => Err(err.to_string()),
      }
    } else {
      Transaction::change(text, changes.into_iter()).map_err(|err| err.to_string())
    }
  };

  let tx = match tx {
    Ok(tx) => tx,
    Err(err) => {
      ctx.push_error(
        "join",
        format!("failed to build join-selections transaction: {err}"),
      );
      return;
    },
  };

  if !ctx.apply_transaction(&tx) {
    ctx.push_error("join", "failed to apply join-selections transaction");
  }
}

fn collapse_selection<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let selection = {
    let doc = ctx.editor_ref().document();
    let text = doc.text().slice(..);
    doc
      .selection()
      .clone()
      .transform(|range| Range::point(range.cursor(text)))
  };
  let _ = ctx.editor().document_mut().set_selection(selection);
}

fn flip_selections<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let selection = ctx
    .editor_ref()
    .document()
    .selection()
    .clone()
    .transform(|range| range.flip());
  let _ = ctx.editor().document_mut().set_selection(selection);
}

fn expand_selection<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let Some((current, expanded)) = ({
    let doc = ctx.editor_ref().document();
    doc.syntax().and_then(|syntax| {
      let text = doc.text().slice(..);
      let current = doc.selection().clone();
      let expanded = object::expand_selection(syntax, text, current.clone());
      (expanded != current).then_some((current, expanded))
    })
  }) else {
    return;
  };

  let editor = ctx.editor();
  editor.push_object_selection(current);
  let _ = editor.document_mut().set_selection(expanded);
}

fn shrink_selection<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let current = ctx.editor_ref().document().selection().clone();
  let editor = ctx.editor();
  if let Some(previous) = editor.pop_object_selection() {
    if current.contains(&previous) {
      let _ = editor.document_mut().set_selection(previous);
      return;
    }
    editor.clear_object_selections();
  }

  let Some(shrunk) = ({
    let doc = editor.document();
    doc.syntax().map(|syntax| {
      let text = doc.text().slice(..);
      object::shrink_selection(syntax, text, current)
    })
  }) else {
    return;
  };

  let _ = editor.document_mut().set_selection(shrunk);
}

fn apply_object_selection_transform<Ctx, F>(ctx: &mut Ctx, transform: F)
where
  Ctx: DefaultContext,
  F: for<'a> Fn(&the_lib::syntax::Syntax, RopeSlice<'a>, Selection) -> Selection,
{
  let Some(next) = ({
    let doc = ctx.editor_ref().document();
    doc.syntax().map(|syntax| {
      let text = doc.text().slice(..);
      let current = doc.selection().clone();
      transform(syntax, text, current)
    })
  }) else {
    return;
  };

  let _ = ctx.editor().document_mut().set_selection(next);
}

fn select_all_children<Ctx: DefaultContext>(ctx: &mut Ctx) {
  apply_object_selection_transform(ctx, object::select_all_children);
}

fn select_all_siblings<Ctx: DefaultContext>(ctx: &mut Ctx) {
  apply_object_selection_transform(ctx, object::select_all_siblings);
}

fn select_prev_sibling<Ctx: DefaultContext>(ctx: &mut Ctx) {
  apply_object_selection_transform(ctx, object::select_prev_sibling);
}

fn select_next_sibling<Ctx: DefaultContext>(ctx: &mut Ctx) {
  apply_object_selection_transform(ctx, object::select_next_sibling);
}

fn goto_first_nonwhitespace<Ctx: DefaultContext>(ctx: &mut Ctx, extend: bool) {
  let extend = extend || ctx.mode() == Mode::Select;
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let new_selection = selection.transform(|range| {
    let line = range.cursor_line(slice);
    let line_slice = slice.line(line);
    if let Some(first_non_whitespace) = line_slice.first_non_whitespace_char() {
      let pos = slice.line_to_char(line) + first_non_whitespace;
      range.put_cursor(slice, pos, extend)
    } else {
      range
    }
  });

  let _ = doc.set_selection(new_selection);
}

fn goto_line_end<Ctx: DefaultContext>(ctx: &mut Ctx, extend: bool) {
  let extend = extend || ctx.mode() == Mode::Select;
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let new_selection = selection.transform(|range| {
    let line = range.cursor_line(slice);
    let pos = line_end_char_index(&slice, line);
    range.put_cursor(slice, pos, extend)
  });

  let _ = doc.set_selection(new_selection);
}

fn page_up<Ctx: DefaultContext>(ctx: &mut Ctx, extend: bool) {
  let height = ctx.editor().view().viewport.height as usize;
  let count = height.saturating_sub(2).max(1); // Leave some overlap
  page_cursor_by_rows(ctx, count, MoveDir::Backward, extend);
}

fn page_down<Ctx: DefaultContext>(ctx: &mut Ctx, extend: bool) {
  let height = ctx.editor().view().viewport.height as usize;
  let count = height.saturating_sub(2).max(1); // Leave some overlap
  page_cursor_by_rows(ctx, count, MoveDir::Forward, extend);
}

fn page_cursor_half_up<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let height = ctx.editor().view().viewport.height as usize;
  let count = (height / 2).max(1);
  page_cursor_by_rows(ctx, count, MoveDir::Backward, false);
}

fn page_cursor_half_down<Ctx: DefaultContext>(ctx: &mut Ctx) {
  let height = ctx.editor().view().viewport.height as usize;
  let count = (height / 2).max(1);
  page_cursor_by_rows(ctx, count, MoveDir::Forward, false);
}

fn page_cursor_by_rows<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  count: usize,
  direction: MoveDir,
  extend: bool,
) {
  let new_selection = {
    let doc = ctx.editor().document_mut();
    let selection = doc.selection().clone();
    let slice = doc.text().slice(..);

    let text_fmt = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    let behavior = if extend {
      Movement::Extend
    } else {
      Movement::Move
    };

    selection.transform(|range| {
      move_vertically(
        slice,
        range,
        direction,
        count,
        behavior,
        &text_fmt,
        &mut annotations,
      )
    })
  };

  let _ = ctx.editor().document_mut().set_selection(new_selection);
}

fn find_char_impl<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  ch: char,
  direction: Direction,
  inclusive: bool,
  extend: bool,
  count: usize,
) {
  use the_lib::search::{
    SearchDirection,
    SearchStart,
    find_nth,
  };

  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let search_dir = match direction {
    Direction::Forward => SearchDirection::Next,
    Direction::Backward => SearchDirection::Prev,
    _ => return, // Only Forward/Backward make sense for find_char
  };

  let new_selection = selection.transform(|range| {
    let cursor = range.cursor(slice);
    // Start search from position after/before cursor (exclusive of current
    // position)
    let search_pos = match direction {
      Direction::Forward => cursor + 1,
      Direction::Backward => cursor,
      _ => return range, // Should not happen
    };

    if let Some(found) = find_nth(
      slice,
      ch,
      search_pos,
      count,
      search_dir,
      SearchStart::Inclusive,
    ) {
      let target = if inclusive {
        found
      } else {
        // "till" - stop one char before the match
        match direction {
          Direction::Forward => found.saturating_sub(1),
          Direction::Backward => found + 1,
          _ => return range, // Should not happen
        }
      };
      range.put_cursor(slice, target, extend)
    } else {
      range // No match found, keep original
    }
  });

  let _ = doc.set_selection(new_selection);
}

fn find_char<Ctx: DefaultContext>(ctx: &mut Ctx, params: (Direction, bool, bool)) {
  let (direction, inclusive, extend) = params;
  ctx.set_pending_input(Some(PendingInput::FindChar {
    direction,
    inclusive,
    extend,
    count: 1,
  }));
}

fn search<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  crate::search_prompt::open_search_prompt(ctx, Direction::Forward);
}

fn rsearch<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  crate::search_prompt::open_search_prompt(ctx, Direction::Backward);
}

fn search_selection<Ctx: DefaultContext>(ctx: &mut Ctx, detect_word_boundaries: bool) {
  fn is_at_word_start(text: RopeSlice<'_>, index: usize) -> bool {
    if index == text.len_chars() {
      return false;
    }
    let ch = text.char(index);
    if index == 0 {
      return char_is_word(ch);
    }
    let prev_ch = text.char(index - 1);
    !char_is_word(prev_ch) && char_is_word(ch)
  }

  fn is_at_word_end(text: RopeSlice<'_>, index: usize) -> bool {
    if index == 0 || index == text.len_chars() {
      return false;
    }
    let ch = text.char(index);
    let prev_ch = text.char(index - 1);
    char_is_word(prev_ch) && !char_is_word(ch)
  }

  let register = ctx.register().unwrap_or('/');
  let doc = ctx.editor_ref().document();
  let text = doc.text().slice(..);

  let regex = doc
    .selection()
    .iter()
    .map(|selection| {
      let add_boundary_prefix = detect_word_boundaries && is_at_word_start(text, selection.from());
      let add_boundary_suffix = detect_word_boundaries && is_at_word_end(text, selection.to());

      let prefix = if add_boundary_prefix { "\\b" } else { "" };
      let suffix = if add_boundary_suffix { "\\b" } else { "" };
      let word = regex::escape(&selection.fragment(text));
      format!("{prefix}{word}{suffix}")
    })
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect::<Vec<_>>()
    .join("|");

  let msg = format!("register '{register}' set to '{regex}'");
  match ctx.registers_mut().push(register, regex) {
    Ok(()) => {
      ctx.registers_mut().last_search_register = register;
      ctx.push_info("search", msg);
    },
    Err(err) => {
      ctx.push_error("search", err.to_string());
    },
  }
}

fn select_regex<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  crate::search_prompt::open_select_regex_prompt(ctx);
}

fn search_next_or_prev<Ctx: DefaultContext>(ctx: &mut Ctx, params: (Direction, bool, usize)) {
  let (direction, extend, count) = params;
  let direction = match direction {
    Direction::Forward => MoveDir::Forward,
    Direction::Backward => MoveDir::Backward,
    _ => return,
  };
  let pick = if extend {
    match direction {
      MoveDir::Forward => CursorPick::Last,
      MoveDir::Backward => CursorPick::First,
    }
  } else {
    CursorPick::First
  };
  let pick = active_or_fallback_pick(ctx.editor_ref(), pick);

  let register = ctx
    .register()
    .unwrap_or(ctx.registers().last_search_register);
  let query = {
    let doc = ctx.editor_ref().document();
    ctx
      .registers()
      .first(register, doc)
      .map(|query| query.into_owned())
  };
  let Some(query) = query else {
    return;
  };
  if query.is_empty() {
    return;
  }

  let Ok(regex) = build_regex(&query, true) else {
    return;
  };

  let movement = if extend {
    Movement::Extend
  } else {
    Movement::Move
  };

  for _ in 0..count.max(1) {
    let next = {
      let doc = ctx.editor_ref().document();
      let text = doc.text().slice(..);
      let selection = doc.selection().clone();
      search_regex(text, &selection, pick, &regex, movement, direction, true)
    };

    let Some(next) = next else {
      break;
    };

    let _ = ctx.editor().document_mut().set_selection(next);
  }
}

fn parent_node_end<Ctx: DefaultContext>(ctx: &mut Ctx, extend: bool) {
  use the_lib::movement::{
    Direction as MoveDirection,
    move_parent_node_end,
  };

  let doc = ctx.editor().document_mut();
  let Some(syntax) = doc.syntax() else {
    return; // No syntax tree available
  };

  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);
  let movement = if extend {
    Movement::Extend
  } else {
    Movement::Move
  };

  let new_selection =
    move_parent_node_end(syntax, slice, selection, MoveDirection::Forward, movement);
  let _ = doc.set_selection(new_selection);
}

fn parent_node_start<Ctx: DefaultContext>(ctx: &mut Ctx, extend: bool) {
  use the_lib::movement::{
    Direction as MoveDirection,
    move_parent_node_end,
  };

  let doc = ctx.editor().document_mut();
  let Some(syntax) = doc.syntax() else {
    return; // No syntax tree available
  };

  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);
  let movement = if extend {
    Movement::Extend
  } else {
    Movement::Move
  };

  let new_selection =
    move_parent_node_end(syntax, slice, selection, MoveDirection::Backward, movement);
  let _ = doc.set_selection(new_selection);
}

fn move_cursor<Ctx: DefaultContext>(ctx: &mut Ctx, direction: Direction) {
  {
    let editor = ctx.editor();
    let doc = editor.document_mut();
    let selection = doc.selection().clone();

    let (dir, vertical) = match direction {
      Direction::Left | Direction::Backward => (MoveDir::Backward, false),
      Direction::Right | Direction::Forward => (MoveDir::Forward, false),
      Direction::Up => (MoveDir::Backward, true),
      Direction::Down => (MoveDir::Forward, true),
    };

    let new_selection = {
      let slice = doc.text().slice(..);
      let text_fmt = TextFormat::default();
      let mut annotations = TextAnnotations::default();
      let mut ranges: SmallVec<_> = SmallVec::with_capacity(selection.ranges().len());
      let mut ids: SmallVec<_> = SmallVec::with_capacity(selection.cursor_ids().len());

      for (cursor_id, range) in selection.iter_with_ids() {
        let new_range = if vertical {
          move_vertically(
            slice,
            *range,
            dir,
            1,
            Movement::Move,
            &text_fmt,
            &mut annotations,
          )
        } else {
          move_horizontally(
            slice,
            *range,
            dir,
            1,
            Movement::Move,
            &text_fmt,
            &mut annotations,
          )
        };
        ranges.push(new_range);
        ids.push(cursor_id);
      }

      Selection::new_with_ids(ranges, ids).ok()
    };

    if let Some(selection) = new_selection {
      let _ = doc.set_selection(selection);
    }
  }
}

fn add_cursor<Ctx: DefaultContext>(ctx: &mut Ctx, direction: Direction) {
  let dir = match direction {
    Direction::Up => MoveDir::Backward,
    Direction::Down => MoveDir::Forward,
    _ => return,
  };
  {
    let editor = ctx.editor();
    let doc = editor.document_mut();
    let selection = doc.selection().clone();

    let new_selection = {
      let slice = doc.text().slice(..);
      let text_fmt = TextFormat::default();
      let mut annotations = TextAnnotations::default();
      let mut ranges: SmallVec<_> = selection.ranges().iter().copied().collect();
      let mut ids: SmallVec<_> = selection.cursor_ids().iter().copied().collect();

      for range in selection.ranges() {
        let moved = move_vertically(
          slice,
          *range,
          dir,
          1,
          Movement::Move,
          &text_fmt,
          &mut annotations,
        );
        ranges.push(moved);
        ids.push(CursorId::fresh());
      }

      Selection::new_with_ids(ranges, ids).ok()
    };

    if let Some(selection) = new_selection {
      let _ = doc.set_selection(selection);
    }
  }
}

fn motion<Ctx: DefaultContext>(ctx: &mut Ctx, motion: Motion) {
  let viewport_width = ctx.editor().view().viewport.width;
  let is_select = ctx.mode() == Mode::Select;
  let next = {
    let editor = ctx.editor();
    let doc = editor.document_mut();
    let selection = doc.selection().clone();
    let slice = doc.text().slice(..);

    let mut text_fmt = TextFormat::default();
    text_fmt.viewport_width = viewport_width;
    let mut annotations = TextAnnotations::default();

    match motion {
      Motion::Char { dir, extend, count } => {
        let dir = match dir {
          Direction::Left => MoveDir::Backward,
          Direction::Right => MoveDir::Forward,
          _ => return,
        };
        let behavior = if extend || is_select {
          Movement::Extend
        } else {
          Movement::Move
        };
        let count = count.max(1);
        selection.clone().transform(|range| {
          move_horizontally(
            slice,
            range,
            dir,
            count,
            behavior,
            &text_fmt,
            &mut annotations,
          )
        })
      },
      Motion::Line { dir, extend, count } => {
        let dir = match dir {
          Direction::Up => MoveDir::Backward,
          Direction::Down => MoveDir::Forward,
          _ => return,
        };
        let behavior = if extend || is_select {
          Movement::Extend
        } else {
          Movement::Move
        };
        let count = count.max(1);
        selection.clone().transform(|range| {
          move_vertically(
            slice,
            range,
            dir,
            count,
            behavior,
            &text_fmt,
            &mut annotations,
          )
        })
      },
      Motion::VisualLine { dir, extend, count } => {
        let dir = match dir {
          Direction::Up => MoveDir::Backward,
          Direction::Down => MoveDir::Forward,
          _ => return,
        };
        let behavior = if extend || is_select {
          Movement::Extend
        } else {
          Movement::Move
        };
        let count = count.max(1);
        selection.clone().transform(|range| {
          move_vertically_visual(
            slice,
            range,
            dir,
            count,
            behavior,
            &text_fmt,
            &mut annotations,
          )
        })
      },
      Motion::Word {
        kind,
        extend,
        count,
      } => {
        let count = count.max(1);
        if extend || is_select {
          selection.clone().transform(|range| {
            let word = match kind {
              WordMotion::NextWordStart => movement::move_next_word_start(slice, range, count),
              WordMotion::PrevWordStart => movement::move_prev_word_start(slice, range, count),
              WordMotion::NextWordEnd => movement::move_next_word_end(slice, range, count),
              WordMotion::PrevWordEnd => movement::move_prev_word_end(slice, range, count),
              WordMotion::NextLongWordStart => {
                movement::move_next_long_word_start(slice, range, count)
              },
              WordMotion::PrevLongWordStart => {
                movement::move_prev_long_word_start(slice, range, count)
              },
              WordMotion::NextLongWordEnd => movement::move_next_long_word_end(slice, range, count),
              WordMotion::PrevLongWordEnd => movement::move_prev_long_word_end(slice, range, count),
              WordMotion::NextSubWordStart => {
                movement::move_next_sub_word_start(slice, range, count)
              },
              WordMotion::PrevSubWordStart => {
                movement::move_prev_sub_word_start(slice, range, count)
              },
              WordMotion::NextSubWordEnd => movement::move_next_sub_word_end(slice, range, count),
              WordMotion::PrevSubWordEnd => movement::move_prev_sub_word_end(slice, range, count),
            };
            let pos = word.cursor(slice);
            range.put_cursor(slice, pos, true)
          })
        } else {
          selection.clone().transform(|range| {
            match kind {
              WordMotion::NextWordStart => movement::move_next_word_start(slice, range, count),
              WordMotion::PrevWordStart => movement::move_prev_word_start(slice, range, count),
              WordMotion::NextWordEnd => movement::move_next_word_end(slice, range, count),
              WordMotion::PrevWordEnd => movement::move_prev_word_end(slice, range, count),
              WordMotion::NextLongWordStart => {
                movement::move_next_long_word_start(slice, range, count)
              },
              WordMotion::PrevLongWordStart => {
                movement::move_prev_long_word_start(slice, range, count)
              },
              WordMotion::NextLongWordEnd => movement::move_next_long_word_end(slice, range, count),
              WordMotion::PrevLongWordEnd => movement::move_prev_long_word_end(slice, range, count),
              WordMotion::NextSubWordStart => {
                movement::move_next_sub_word_start(slice, range, count)
              },
              WordMotion::PrevSubWordStart => {
                movement::move_prev_sub_word_start(slice, range, count)
              },
              WordMotion::NextSubWordEnd => movement::move_next_sub_word_end(slice, range, count),
              WordMotion::PrevSubWordEnd => movement::move_prev_sub_word_end(slice, range, count),
            }
          })
        }
      },
      Motion::FileStart { extend } => {
        let extend = extend || is_select;
        selection
          .clone()
          .transform(|range| range.put_cursor(slice, 0, extend))
      },
      Motion::FileEnd { extend } => {
        let extend = extend || is_select;
        let pos = slice.len_chars();
        selection
          .clone()
          .transform(|range| range.put_cursor(slice, pos, extend))
      },
      Motion::LastLine { extend } => {
        let extend = extend || is_select;
        let line = slice.len_lines().saturating_sub(1);
        let pos = slice.line_to_char(line);
        selection
          .clone()
          .transform(|range| range.put_cursor(slice, pos, extend))
      },
      Motion::Column { col, extend } => {
        let extend = extend || is_select;
        let col = col.saturating_sub(1);
        selection.clone().transform(|range| {
          let line = slice.char_to_line(range.cursor(slice));
          let pos = char_idx_at_coords(slice, Position::new(line, col));
          range.put_cursor(slice, pos, extend)
        })
      },
    }
  };

  {
    let doc = ctx.editor().document_mut();
    let _ = doc.set_selection(next);
  }
}

fn save<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  if let Err(err) = ctx.save_current_buffer(false) {
    if err.starts_with("Failed to write ") {
      ctx.push_error("save", err);
    } else {
      ctx.push_warning("save", err);
    }
  }
}

fn status_path_text(path: &Path) -> String {
  if let Ok(cwd) = std::env::current_dir() {
    if let Ok(relative) = path.strip_prefix(&cwd) {
      return relative.display().to_string();
    }
  }
  path.display().to_string()
}

fn format_binary_size(bytes: usize) -> String {
  const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
  if bytes < 1024 {
    return format!("{bytes}B");
  }

  let mut unit_index = 0usize;
  let mut value = bytes as f64;
  while value >= 1024.0 && unit_index < UNITS.len() - 1 {
    value /= 1024.0;
    unit_index += 1;
  }

  format!("{value:.1}{}", UNITS[unit_index])
}

fn quit<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  ctx.on_before_quit();
  ctx.request_quit();
}

fn delete_selection<Ctx: DefaultContext>(ctx: &mut Ctx, yank: bool) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  // Collect fragments, treating empty selections as 1-char selections
  let fragments: Vec<String> = selection
    .ranges()
    .iter()
    .map(|range| {
      let (from, to) = if range.is_empty() {
        (
          range.from(),
          nth_next_grapheme_boundary(slice, range.from(), 1),
        )
      } else {
        (range.from(), range.to())
      };
      slice.slice(from..to).to_string()
    })
    .collect();

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    // For empty selections (cursor only), delete the grapheme at cursor
    if range.is_empty() {
      (
        range.from(),
        nth_next_grapheme_boundary(slice, range.from(), 1),
      )
    } else {
      (range.from(), range.to())
    }
  });

  let tx = match tx {
    Ok(tx) => tx,
    Err(err) => {
      ctx.push_error("edit", format!("failed to build delete transaction: {err}"));
      return;
    },
  };
  if !ctx.apply_transaction(&tx) {
    ctx.push_error("edit", "failed to apply delete transaction");
    return;
  }

  if yank {
    let _ = ctx.registers_mut().write('"', fragments);
  }

  ctx.set_mode(Mode::Normal);
  ctx.request_render();
}

fn selection_is_linewise(selection: &Selection, text: &ropey::Rope) -> bool {
  selection.ranges().iter().all(|range| {
    let slice = text.slice(..);
    if range.slice(slice).len_lines() < 2 {
      return false;
    }
    let (start_line, end_line) = range.line_range(slice);
    let start = text.line_to_char(start_line);
    let end = text.line_to_char((end_line + 1).min(text.len_lines()));
    start == range.from() && end == range.to()
  })
}

fn change_selection<Ctx: DefaultContext>(ctx: &mut Ctx, yank: bool) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let only_whole_lines = selection_is_linewise(&selection, doc.text());
  let slice = doc.text().slice(..);

  // Collect fragments, treating empty selections as 1-char selections
  let fragments: Vec<String> = selection
    .ranges()
    .iter()
    .map(|range| {
      let (from, to) = if range.is_empty() {
        (
          range.from(),
          nth_next_grapheme_boundary(slice, range.from(), 1),
        )
      } else {
        (range.from(), range.to())
      };
      slice.slice(from..to).to_string()
    })
    .collect();

  let tx = Transaction::delete_by_selection(doc.text(), &selection, |range| {
    // For empty selections (cursor only), delete the grapheme at cursor
    if range.is_empty() {
      (
        range.from(),
        nth_next_grapheme_boundary(slice, range.from(), 1),
      )
    } else {
      (range.from(), range.to())
    }
  });

  let tx = match tx {
    Ok(tx) => tx,
    Err(err) => {
      ctx.push_error("edit", format!("failed to build change transaction: {err}"));
      return;
    },
  };
  if !ctx.apply_transaction(&tx) {
    ctx.push_error("edit", "failed to apply change transaction");
    return;
  }

  if yank {
    let _ = ctx.registers_mut().write('"', fragments);
  }

  if only_whole_lines {
    open(ctx, OpenDirection::Above, CommentContinuation::Disabled);
  } else {
    ctx.set_mode(Mode::Insert);
    ctx.request_render();
  }
}

fn replace_selection<Ctx: DefaultContext>(ctx: &mut Ctx, ch: char) {
  let mut buf = [0u8; 4];
  let replacement = ch.encode_utf8(&mut buf);
  replace_selection_with_str(ctx, replacement);
}

fn replace_selection_with_str<Ctx: DefaultContext>(ctx: &mut Ctx, replacement: &str) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  // Create transaction that replaces each range with the character repeated.
  // In normal mode our selection ranges may be empty; treat those as replacing
  // the grapheme under the cursor.
  let tx = Transaction::change_by_selection(doc.text(), &selection, |range| {
    let (from, to) = if range.is_empty() {
      (
        range.from(),
        nth_next_grapheme_boundary(slice, range.from(), 1),
      )
    } else {
      (range.from(), range.to())
    };

    let graphemes = slice.slice(from..to).graphemes().count();
    if graphemes == 0 {
      return (from, to, None);
    }
    let mut out = Tendril::new();
    for _ in 0..graphemes {
      out.push_str(replacement);
    }
    (from, to, Some(out))
  });

  if let Ok(tx) = tx {
    let _ = ctx.apply_transaction(&tx);
  }

  if ctx.mode() == Mode::Select {
    ctx.set_mode(Mode::Normal);
  }
  ctx.request_render();
}

fn replace_with_yanked<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  // Read the register values using shared references
  let replacement: Option<String> = {
    let doc = ctx.editor_ref().document();
    ctx
      .registers()
      .read('"', doc)
      .and_then(|mut values| values.next().map(|v| v.into_owned()))
  };

  let Some(replacement) = replacement else {
    ctx.set_mode(Mode::Normal);
    ctx.request_render();
    return;
  };

  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  // Replace each selection range with the yanked content
  let tx = Transaction::change_by_selection(doc.text(), &selection, |range| {
    // For empty selections (cursor only), replace the grapheme at cursor
    let (from, to) = if range.is_empty() {
      (
        range.from(),
        nth_next_grapheme_boundary(slice, range.from(), 1),
      )
    } else {
      (range.from(), range.to())
    };
    (from, to, Some(Tendril::from(replacement.as_str())))
  });

  if let Ok(tx) = tx {
    let _ = ctx.apply_transaction(&tx);
  }

  ctx.set_mode(Mode::Normal);
  ctx.request_render();
}

fn yank<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let fragments: Vec<String> = selection.fragments(slice).map(Cow::into_owned).collect();

  let _ = ctx.registers_mut().write('"', fragments);

  ctx.set_mode(Mode::Normal);
  ctx.request_render();
}

fn paste<Ctx: DefaultContext>(ctx: &mut Ctx, after: bool) {
  let values: Option<Vec<String>> = {
    let doc = ctx.editor_ref().document();
    ctx
      .registers()
      .read('"', doc)
      .map(|iter| iter.map(|v| v.into_owned()).collect())
  };

  let Some(values) = values else {
    ctx.request_render();
    return;
  };

  if values.is_empty() {
    ctx.request_render();
    return;
  }

  let mode = ctx.mode();
  let doc = ctx.editor().document_mut();
  let text = doc.text();
  let selection = doc.selection().clone();
  let line_ending = doc.line_ending().as_str();

  let linewise = values
    .iter()
    .any(|value| get_line_ending_of_str(value).is_some());

  let normalize_line_endings = |value: &str| {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
      match ch {
        '\r' => {
          if chars.peek() == Some(&'\n') {
            chars.next();
          }
          out.push_str(line_ending);
        },
        '\n' => out.push_str(line_ending),
        _ => out.push(ch),
      }
    }
    out
  };

  let map_value = |value: &str| {
    let normalized = normalize_line_endings(value);
    Tendril::from(normalized.as_str())
  };

  let last = map_value(values.last().unwrap());
  let mut values = values
    .iter()
    .map(|value| map_value(value))
    .chain(std::iter::repeat(last));

  let mut offset = 0usize;
  let mut ranges = SmallVec::with_capacity(selection.len());

  let Ok(mut tx) = Transaction::change_by_selection(text, &selection, |range| {
    let pos = if linewise {
      if after {
        let line = range.line_range(text.slice(..)).1;
        text.line_to_char((line + 1).min(text.len_lines()))
      } else {
        text.line_to_char(text.char_to_line(range.from()))
      }
    } else if after {
      range.to()
    } else {
      range.from()
    };

    let value = values.next();
    let value_len = value
      .as_ref()
      .map(|content| content.chars().count())
      .unwrap_or_default();
    let anchor = offset + pos;

    let new_range = Range::new(anchor, anchor + value_len).with_direction(range.direction());
    ranges.push(new_range);
    offset += value_len;

    (pos, pos, value)
  }) else {
    return;
  };

  if mode == Mode::Normal {
    let cursor_ids: SmallVec<[CursorId; 1]> = selection.cursor_ids().iter().copied().collect();
    let new_selection =
      Selection::new_with_ids(ranges, cursor_ids).unwrap_or_else(|_| selection.clone());
    tx = tx.with_selection(new_selection);
  }

  let _ = ctx.apply_transaction(&tx);

  ctx.set_mode(Mode::Normal);
  ctx.request_render();
}

fn record_macro<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  if let Some((reg, mut keys)) = ctx.macro_recording().clone() {
    // Remove the keypress which ends the recording (Q)
    keys.pop();
    let s = keys
      .into_iter()
      .map(|key| {
        let s = key.to_string();
        if s.chars().count() == 1 {
          s
        } else {
          format!("<{}>", s)
        }
      })
      .collect::<String>();
    ctx.set_macro_recording(None);
    let _ = ctx.registers_mut().write(reg, vec![s]);
  } else {
    let reg = ctx.register().unwrap_or('@');
    ctx.set_macro_recording(Some((reg, Vec::new())));
  }
  ctx.request_render();
}

fn replay_macro<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let reg = ctx.register().unwrap_or('@');
  if ctx.macro_replaying().contains(&reg) {
    return;
  }

  let keys: Vec<KeyBinding> = if let Some(keys) = ctx
    .registers()
    .read(reg, ctx.editor_ref().document())
    .filter(|values| values.len() == 1)
    .map(|mut values| values.next().unwrap())
  {
    match parse_macro(keys.as_ref()) {
      Ok(keys) => keys,
      Err(_) => return,
    }
  } else {
    return;
  };

  ctx.macro_replaying_mut().push(reg);
  ctx
    .macro_queue_mut()
    .extend(keys.iter().map(|key| key.to_key_event()));
  while let Some(next) = ctx.macro_queue_mut().pop_front() {
    ctx.dispatch().on_keypress(ctx, next);
  }
  ctx.macro_replaying_mut().pop();
}

fn parse_macro(keys: &str) -> Result<Vec<KeyBinding>, ParseKeyBindingError> {
  let mut out = Vec::new();
  let mut chars = keys.chars().peekable();

  while let Some(ch) = chars.next() {
    if ch == '<' {
      let mut token = String::new();
      while let Some(next) = chars.next() {
        if next == '>' {
          break;
        }
        token.push(next);
      }

      if token.is_empty() {
        return Err(ParseKeyBindingError("empty macro token".into()));
      }

      out.push(token.parse()?);
      continue;
    }

    out.push(KeyBinding::new(Key::Char(ch)));
  }

  Ok(out)
}

fn switch_case<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  switch_case_impl(ctx, |s| {
    s.chars()
      .flat_map(|ch| {
        if ch.is_lowercase() {
          CaseSwitcher::Upper(ch.to_uppercase())
        } else if ch.is_uppercase() {
          CaseSwitcher::Lower(ch.to_lowercase())
        } else {
          CaseSwitcher::Keep(Some(ch))
        }
      })
      .collect()
  });
}

fn switch_to_uppercase<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  switch_case_impl(ctx, |s| s.to_uppercase().into());
}

fn switch_to_lowercase<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  switch_case_impl(ctx, |s| s.to_lowercase().into());
}

fn switch_case_impl<Ctx, F>(ctx: &mut Ctx, change_fn: F)
where
  Ctx: DefaultContext,
  F: Fn(&str) -> Tendril,
{
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let tx = Transaction::change_by_selection(doc.text(), &selection, |range| {
    let (from, to) = if range.is_empty() {
      (
        range.from(),
        nth_next_grapheme_boundary(slice, range.from(), 1),
      )
    } else {
      (range.from(), range.to())
    };
    let text: Tendril = change_fn(&slice.slice(from..to).to_string());
    (from, to, Some(text))
  });

  if let Ok(tx) = tx {
    let _ = ctx.apply_transaction(&tx);
  }

  ctx.set_mode(Mode::Normal);
  ctx.request_render();
}

enum CaseSwitcher {
  Upper(std::char::ToUppercase),
  Lower(std::char::ToLowercase),
  Keep(Option<char>),
}

impl Iterator for CaseSwitcher {
  type Item = char;

  fn next(&mut self) -> Option<Self::Item> {
    match self {
      CaseSwitcher::Upper(u) => u.next(),
      CaseSwitcher::Lower(l) => l.next(),
      CaseSwitcher::Keep(ch) => ch.take(),
    }
  }
}

fn insert_at_line_start<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let new_selection = selection.transform(|range| {
    let line = range.cursor_line(slice);
    let line_start = slice.line_to_char(line);
    let pos = slice
      .line(line)
      .first_non_whitespace_char()
      .map(|offset| line_start + offset)
      .unwrap_or(line_start);
    range.put_cursor(slice, pos, false)
  });

  let _ = doc.set_selection(new_selection);
  ctx.set_mode(Mode::Insert);
  ctx.request_render();
}

fn insert_at_line_end<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let new_selection = selection.transform(|range| {
    let line = range.cursor_line(slice);
    let pos = line_end_char_index(&slice, line);
    range.put_cursor(slice, pos, false)
  });

  let _ = doc.set_selection(new_selection);
  ctx.set_mode(Mode::Insert);
  ctx.request_render();
}

fn append_mode<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let slice = doc.text().slice(..);

  let new_selection = selection.transform(|range| {
    let pos = nth_next_grapheme_boundary(slice, range.cursor(slice), 1);
    range.put_cursor(slice, pos, false)
  });

  let _ = doc.set_selection(new_selection);
  ctx.set_mode(Mode::Insert);
  ctx.request_render();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpenDirection {
  Above,
  Below,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommentContinuation {
  Enabled,
  #[allow(dead_code)]
  Disabled,
}

fn syntax_loader() -> &'static Loader {
  static LOADER: OnceLock<Loader> = OnceLock::new();
  LOADER.get_or_init(|| {
    let config = Configuration {
      language:        Vec::new(),
      language_server: HashMap::new(),
    };
    Loader::new(config, NullResources).expect("syntax loader")
  })
}

fn open_below<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  open(ctx, OpenDirection::Below, CommentContinuation::Enabled);
}

fn open_above<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  open(ctx, OpenDirection::Above, CommentContinuation::Enabled);
}

fn add_newline<Ctx: DefaultContext>(ctx: &mut Ctx, direction: OpenDirection) {
  let line_ending = ctx
    .editor_ref()
    .document()
    .line_ending()
    .as_str()
    .to_string();
  let tx = {
    let doc = ctx.editor_ref().document();
    let text = doc.text();
    let selection = doc.selection().clone();
    let slice = text.slice(..);
    let changes = selection.ranges().iter().map(|range| {
      let (start, end) = range.line_range(slice);
      let line = match direction {
        OpenDirection::Above => start,
        OpenDirection::Below => end.saturating_add(1),
      }
      .min(text.len_lines());
      let pos = text.line_to_char(line);
      (pos, pos, Some(line_ending.clone().into()))
    });
    Transaction::change(text, changes)
  };
  let Ok(tx) = tx else {
    ctx.push_error("edit", "failed to build add-newline transaction");
    return;
  };
  if !ctx.apply_transaction(&tx) {
    ctx.push_error("edit", "failed to apply add-newline transaction");
  }
}

fn commit_undo_checkpoint<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let _ = ctx.editor().document_mut().commit();
}

fn copy_selection_on_next_line<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  copy_selection_on_line(ctx, Direction::Forward);
}

fn copy_selection_on_prev_line<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  copy_selection_on_line(ctx, Direction::Backward);
}

fn select_all<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let end = doc.text().len_chars();
  let _ = doc.set_selection(Selection::single(0, end));
}

#[derive(Clone, Copy)]
enum ExtendDirection {
  Above,
  Below,
}

fn extend_line_below<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  extend_line_impl(ctx, ExtendDirection::Below, count.max(1));
}

fn extend_line_above<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  extend_line_impl(ctx, ExtendDirection::Above, count.max(1));
}

fn extend_to_line_bounds<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let text = doc.text();
  let selection = doc.selection().clone();

  let new_selection = selection.transform(|range| {
    let slice = text.slice(..);
    let (start_line, end_line) = range.line_range(slice);
    let start = text.line_to_char(start_line);
    let end = text.line_to_char((end_line + 1).min(text.len_lines()));

    Range::new(start, end).with_direction(range.direction())
  });

  let _ = doc.set_selection(new_selection);
}

fn shrink_to_line_bounds<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let doc = ctx.editor().document_mut();
  let text = doc.text();
  let selection = doc.selection().clone();

  let new_selection = selection.transform(|range| {
    let slice = text.slice(..);
    let (start_line, end_line) = range.line_range(slice);

    if start_line == end_line {
      return range;
    }

    let mut start = text.line_to_char(start_line);
    let mut end = text.line_to_char((end_line + 1).min(text.len_lines()));

    if start != range.from() {
      start = text.line_to_char((start_line + 1).min(text.len_lines()));
    }

    if end != range.to() {
      end = text.line_to_char(end_line);
    }

    Range::new(start, end).with_direction(range.direction())
  });

  let _ = doc.set_selection(new_selection);
}

fn extend_line_impl<Ctx: DefaultContext>(ctx: &mut Ctx, extend: ExtendDirection, count: usize) {
  let doc = ctx.editor().document_mut();
  let text = doc.text();
  let selection = doc.selection().clone();

  let new_selection = selection.transform(|range| {
    let slice = text.slice(..);
    let (start_line, end_line) = range.line_range(slice);

    let start = text.line_to_char(start_line);
    let end = text.line_to_char((end_line + 1).min(text.len_lines()));

    let (anchor, head) = if range.from() == start && range.to() == end {
      match extend {
        ExtendDirection::Above => (end, text.line_to_char(start_line.saturating_sub(count))),
        ExtendDirection::Below => {
          (
            start,
            text.line_to_char((end_line + count + 1).min(text.len_lines())),
          )
        },
      }
    } else {
      match extend {
        ExtendDirection::Above => (end, text.line_to_char(start_line.saturating_sub(count - 1))),
        ExtendDirection::Below => {
          (
            start,
            text.line_to_char((end_line + count).min(text.len_lines())),
          )
        },
      }
    };

    Range::new(anchor, head)
  });

  let _ = doc.set_selection(new_selection);
}

fn apply_prepared_history_jump<Ctx: DefaultContext>(ctx: &mut Ctx, jump: &HistoryJump) -> bool {
  for transaction in &jump.transactions {
    if !ctx.apply_transaction(transaction) {
      return false;
    }
  }
  ctx
    .editor()
    .document_mut()
    .finish_history_jump(jump)
    .is_ok()
}

fn undo<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let count = count.max(1);
  for _ in 0..count {
    let Some(jump) = ctx.editor_ref().document().prepare_undo_jump() else {
      break;
    };
    if !apply_prepared_history_jump(ctx, &jump) {
      break;
    }
  }
}

fn redo<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let count = count.max(1);
  for _ in 0..count {
    let Some(jump) = ctx.editor_ref().document().prepare_redo_jump() else {
      break;
    };
    if !apply_prepared_history_jump(ctx, &jump) {
      break;
    }
  }
}

fn earlier<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let count = count.max(1);
  for _ in 0..count {
    let Some(jump) = ctx
      .editor_ref()
      .document()
      .prepare_earlier_jump(UndoKind::Steps(1))
      .ok()
    else {
      break;
    };
    if !apply_prepared_history_jump(ctx, &jump) {
      break;
    }
  }
}

fn later<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let count = count.max(1);
  for _ in 0..count {
    let Some(jump) = ctx
      .editor_ref()
      .document()
      .prepare_later_jump(UndoKind::Steps(1))
      .ok()
    else {
      break;
    };
    if !apply_prepared_history_jump(ctx, &jump) {
      break;
    }
  }
}

fn indent<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let count = count.max(1);
  let doc = ctx.editor().document_mut();
  let text = doc.text();
  let selection = doc.selection().clone();
  let indent_str = doc.indent_style().as_str().repeat(count);
  let indent = Tendril::from(indent_str.as_str());

  let changes: Vec<_> = selection
    .line_ranges(text.slice(..))
    .flat_map(|(start_line, end_line)| start_line..=end_line)
    .filter_map(|line| {
      let is_blank = text.line(line).chars().all(|ch| ch.is_whitespace());
      if is_blank {
        return None;
      }
      let pos = text.line_to_char(line);
      Some((pos, pos, Some(indent.clone())))
    })
    .collect();

  let Ok(tx) = Transaction::change(doc.text(), changes.into_iter()) else {
    return;
  };
  let _ = ctx.apply_transaction(&tx);

  if ctx.mode() == Mode::Select {
    ctx.set_mode(Mode::Normal);
  }
}

fn unindent<Ctx: DefaultContext>(ctx: &mut Ctx, count: usize) {
  let count = count.max(1);
  let tab_width = 4usize;
  let doc = ctx.editor().document_mut();
  let text = doc.text();
  let selection = doc.selection().clone();
  let indent_width = count * doc.indent_style().indent_width(tab_width);

  let changes: Vec<_> = selection
    .line_ranges(text.slice(..))
    .flat_map(|(start_line, end_line)| start_line..=end_line)
    .filter_map(|line_idx| {
      let line = text.line(line_idx);
      let mut width = 0usize;
      let mut pos = 0usize;

      for ch in line.chars() {
        match ch {
          ' ' => width += 1,
          '\t' => width = (width / tab_width + 1) * tab_width,
          _ => break,
        }
        pos += 1;
        if width >= indent_width {
          break;
        }
      }

      if pos > 0 {
        let start = text.line_to_char(line_idx);
        Some((start, start + pos, None))
      } else {
        None
      }
    })
    .collect();

  let Ok(tx) = Transaction::change(doc.text(), changes.into_iter()) else {
    return;
  };
  let _ = ctx.apply_transaction(&tx);

  if ctx.mode() == Mode::Select {
    ctx.set_mode(Mode::Normal);
  }
}

fn match_brackets<Ctx: DefaultContext>(ctx: &mut Ctx, _unit: ()) {
  let is_select = ctx.mode() == Mode::Select;
  let doc = ctx.editor().document_mut();
  let text = doc.text();
  let text_slice = text.slice(..);
  let selection = doc.selection().clone();
  let syntax = doc.syntax();

  let new_selection = selection.transform(|range| {
    let pos = range.cursor(text_slice);
    let matched_pos = if let Some(syn) = syntax {
      mb::find_matching_bracket_fuzzy(syn, text_slice, pos)
    } else {
      mb::find_matching_bracket_plaintext(text_slice, pos)
    };

    if let Some(matched) = matched_pos {
      range.put_cursor(text_slice, matched, is_select)
    } else {
      range
    }
  });

  let _ = doc.set_selection(new_selection);
}

fn select_textobject_impl<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  kind: text_object::TextObject,
  ch: char,
) {
  let count = 1usize;
  let new_selection = {
    let editor = ctx.editor_ref();
    let doc = editor.document();
    let text = doc.text().slice(..);
    let selection = doc.selection().clone();
    let syntax = doc.syntax();
    let loader = match ctx.syntax_loader() {
      Some(loader) => loader,
      None => syntax_loader(),
    };

    let textobject_treesitter = |obj_name: &str, range: Range| -> Range {
      let Some(syntax) = syntax else {
        return range;
      };
      text_object::textobject_treesitter(text, range, kind, obj_name, syntax, loader, count)
    };

    selection.transform(|range| {
      match ch {
        'w' => text_object::textobject_word(text, range, kind, count, false),
        'W' => text_object::textobject_word(text, range, kind, count, true),
        't' => textobject_treesitter("class", range),
        'f' => textobject_treesitter("function", range),
        'a' => textobject_treesitter("parameter", range),
        'c' => textobject_treesitter("comment", range),
        'T' => textobject_treesitter("test", range),
        'e' => textobject_treesitter("entry", range),
        'x' => textobject_treesitter("xml-element", range),
        'p' => text_object::textobject_paragraph(text, range, kind, count),
        'm' => text_object::textobject_pair_surround_closest(syntax, text, range, kind, count),
        ch if !ch.is_ascii_alphanumeric() => {
          text_object::textobject_pair_surround(syntax, text, range, kind, ch, count)
        },
        _ => range,
      }
    })
  };

  let doc = ctx.editor().document_mut();
  let _ = doc.set_selection(new_selection);
}

fn surround_add_impl<Ctx: DefaultContext>(ctx: &mut Ctx, key: KeyEvent) {
  let (open, close, surround_len) = match key.key {
    Key::Char(ch) => {
      let (o, c) = mb::get_pair(ch);
      let mut open = Tendril::new();
      open.push(o);
      let mut close = Tendril::new();
      close.push(c);
      (open, close, 2usize)
    },
    Key::Enter | Key::NumpadEnter => {
      let line_ending = ctx.editor_ref().document().line_ending().as_str();
      (
        Tendril::from(line_ending),
        Tendril::from(line_ending),
        2 * line_ending.chars().count(),
      )
    },
    _ => return,
  };

  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let mut changes = Vec::with_capacity(selection.ranges().len() * 2);
  let mut ranges: SmallVec<[Range; 1]> = SmallVec::with_capacity(selection.ranges().len());
  let mut offs = 0usize;

  for range in selection.ranges() {
    changes.push((range.from(), range.from(), Some(open.clone())));
    changes.push((range.to(), range.to(), Some(close.clone())));

    ranges.push(
      Range::new(offs + range.from(), offs + range.to() + surround_len)
        .with_direction(range.direction()),
    );

    offs += surround_len;
  }

  let cursor_ids: SmallVec<[CursorId; 1]> = selection.cursor_ids().iter().copied().collect();
  let Ok(tx) = Transaction::change(doc.text(), changes.into_iter()) else {
    return;
  };
  let new_selection =
    Selection::new_with_ids(ranges, cursor_ids).unwrap_or_else(|_| selection.clone());
  let tx = tx.with_selection(new_selection);
  let _ = ctx.apply_transaction(&tx);

  if ctx.mode() == Mode::Select {
    ctx.set_mode(Mode::Normal);
  }
}

fn surround_delete_impl<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  surround_ch: Option<char>,
  count: usize,
) {
  let doc = ctx.editor().document_mut();
  let selection = doc.selection().clone();
  let text = doc.text().slice(..);

  let mut change_pos =
    match surround::get_surround_pos(doc.syntax(), text, &selection, surround_ch, count) {
      Ok(c) => c,
      Err(_) => return,
    };

  change_pos.sort_unstable();

  let changes: Vec<_> = change_pos
    .into_iter()
    .flat_map(|(open, close)| {
      let open_end = the_core::grapheme::next_grapheme_boundary(text, open);
      let close_end = the_core::grapheme::next_grapheme_boundary(text, close);
      vec![
        (open, open_end, None::<Tendril>),
        (close, close_end, None::<Tendril>),
      ]
    })
    .collect();

  let Ok(tx) = Transaction::change(doc.text(), changes.into_iter()) else {
    return;
  };

  let _ = ctx.apply_transaction(&tx);

  if ctx.mode() == Mode::Select {
    ctx.set_mode(Mode::Normal);
  }
}

fn surround_replace_find<Ctx: DefaultContext>(ctx: &mut Ctx, ch: char, count: usize) {
  let doc = ctx.editor_ref().document();
  let selection = doc.selection().clone();
  let text = doc.text().slice(..);

  let Ok(positions) = surround::get_surround_pos(doc.syntax(), text, &selection, Some(ch), count)
  else {
    return;
  };

  let original_selection: Vec<(usize, usize)> = selection
    .ranges()
    .iter()
    .map(|r| (r.anchor, r.head))
    .collect();

  ctx.set_pending_input(Some(PendingInput::SurroundReplaceWith {
    positions: positions.into_vec(),
    original_selection,
  }));
}

fn surround_replace_with<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  ch: char,
  positions: &[(usize, usize)],
  original_selection: &[(usize, usize)],
) {
  let (open, close) = mb::get_pair(ch);

  let doc = ctx.editor().document_mut();
  let text = doc.text();

  let mut changes: Vec<(usize, usize, Option<Tendril>)> = Vec::with_capacity(positions.len() * 2);
  for &(open_pos, close_pos) in positions {
    let open_end = the_core::grapheme::next_grapheme_boundary(text.slice(..), open_pos);
    let close_end = the_core::grapheme::next_grapheme_boundary(text.slice(..), close_pos);

    let mut open_str = Tendril::new();
    open_str.push(open);
    let mut close_str = Tendril::new();
    close_str.push(close);

    changes.push((open_pos, open_end, Some(open_str)));
    changes.push((close_pos, close_end, Some(close_str)));
  }

  changes.sort_by_key(|(from, ..)| *from);

  let Ok(tx) = Transaction::change(text, changes.into_iter()) else {
    return;
  };

  let ranges: SmallVec<[Range; 1]> = original_selection
    .iter()
    .map(|&(anchor, head)| Range::new(anchor, head))
    .collect();
  let cursor_ids: SmallVec<[CursorId; 1]> = (0..ranges.len()).map(|_| CursorId::fresh()).collect();
  let new_selection =
    Selection::new_with_ids(ranges, cursor_ids).unwrap_or_else(|_| doc.selection().clone());

  let tx = tx.with_selection(new_selection);
  let _ = ctx.apply_transaction(&tx);
}

fn copy_selection_on_line<Ctx: DefaultContext>(ctx: &mut Ctx, direction: Direction) {
  let count = 1usize;
  let selection = {
    let doc = ctx.editor_ref().document();
    let text = doc.text().slice(..);
    let selection = doc.selection().clone();

    let mut ranges: SmallVec<[Range; 1]> =
      SmallVec::with_capacity(selection.ranges().len() * (count + 1));
    let mut cursor_ids: SmallVec<[CursorId; 1]> =
      SmallVec::with_capacity(selection.ranges().len() * (count + 1));

    ranges.extend_from_slice(selection.ranges());
    cursor_ids.extend_from_slice(selection.cursor_ids());

    let text_fmt = ctx.text_format();
    let mut annotations = ctx.text_annotations();

    for (_cursor_id, range) in selection.iter_with_ids() {
      let (head, anchor) = if range.is_empty() {
        (range.head, range.anchor)
      } else if range.anchor < range.head {
        (range.head.saturating_sub(1), range.anchor)
      } else {
        (range.head, range.anchor.saturating_sub(1))
      };

      let Some(head_pos) = visual_pos_at_char(text, &text_fmt, &mut annotations, head) else {
        continue;
      };
      let Some(anchor_pos) = visual_pos_at_char(text, &text_fmt, &mut annotations, anchor) else {
        continue;
      };

      let height = head_pos
        .row
        .max(anchor_pos.row)
        .saturating_sub(head_pos.row.min(anchor_pos.row))
        + 1;

      let mut sels = 0;
      let mut i = 0usize;
      while sels < count {
        let offset = (i + 1) * height;
        let anchor_row = match direction {
          Direction::Forward => anchor_pos.row + offset,
          Direction::Backward => anchor_pos.row.saturating_sub(offset),
          _ => anchor_pos.row,
        };
        let head_row = match direction {
          Direction::Forward => head_pos.row + offset,
          Direction::Backward => head_pos.row.saturating_sub(offset),
          _ => head_pos.row,
        };

        if anchor_row >= text.len_lines() || head_row >= text.len_lines() {
          break;
        }

        let Some(anchor_idx) = char_at_visual_pos(
          text,
          &text_fmt,
          &mut annotations,
          Position::new(anchor_row, anchor_pos.col),
        ) else {
          break;
        };
        let Some(head_idx) = char_at_visual_pos(
          text,
          &text_fmt,
          &mut annotations,
          Position::new(head_row, head_pos.col),
        ) else {
          break;
        };

        let anchor_ok = visual_pos_at_char(text, &text_fmt, &mut annotations, anchor_idx)
          .is_some_and(|pos| pos.col == anchor_pos.col);
        let head_ok = visual_pos_at_char(text, &text_fmt, &mut annotations, head_idx)
          .is_some_and(|pos| pos.col == head_pos.col);

        if anchor_ok && head_ok {
          ranges.push(Range::point(anchor_idx).put_cursor(text, head_idx, true));
          cursor_ids.push(CursorId::fresh());
          sels += 1;
        }

        if anchor_row == 0 && head_row == 0 {
          break;
        }

        i += 1;
      }
    }

    Selection::new_with_ids(ranges, cursor_ids).ok()
  };

  let Some(selection) = selection else {
    return;
  };

  let doc = ctx.editor().document_mut();
  let _ = doc.set_selection(selection);
}

fn open<Ctx: DefaultContext>(
  ctx: &mut Ctx,
  open: OpenDirection,
  comment_continuation: CommentContinuation,
) {
  let loader_ptr = ctx.syntax_loader().map(|loader| loader as *const Loader);
  let fallback_loader = syntax_loader() as *const Loader;

  // NOTE: count support isn't wired yet in the new context.
  let count = 1usize;
  ctx.set_mode(Mode::Insert);

  let doc = ctx.editor().document_mut();
  let contents = doc.text();
  let text = contents.slice(..);
  let selection = doc.selection().clone();
  let line_ending = doc.line_ending();

  let mut offs: usize = 0;
  let mut ranges = SmallVec::with_capacity(selection.len());

  // We don't have language config access yet, so comment continuation is
  // disabled.
  let continue_comment_tokens: Option<&[String]> = match comment_continuation {
    CommentContinuation::Enabled => None,
    CommentContinuation::Disabled => None,
  };

  let tab_width = 4usize;
  let indent_heuristic = IndentationHeuristic::default();
  let loader = unsafe { &*loader_ptr.unwrap_or(fallback_loader) };
  let loader_language_count = loader.languages().len();

  let tx = Transaction::change_by_selection(contents, &selection, |range| {
    let curr_line_num = text.char_to_line(match open {
      OpenDirection::Below => {
        if range.is_empty() {
          range.cursor(text)
        } else {
          prev_grapheme_boundary(text, range.to())
        }
      },
      OpenDirection::Above => range.from(),
    });

    let next_new_line_num = match open {
      OpenDirection::Below => curr_line_num + 1,
      OpenDirection::Above => curr_line_num,
    };

    let above_next_new_line_num = next_new_line_num.saturating_sub(1);

    let continue_comment_token = continue_comment_tokens
      .and_then(|tokens| the_lib::comment::get_comment_token(text, tokens, curr_line_num));

    let (above_next_line_end_index, above_next_line_end_width) = if next_new_line_num == 0 {
      (0, 0)
    } else {
      (
        line_end_char_index(&text, above_next_new_line_num),
        line_ending.len_chars(),
      )
    };

    let line = text.line(curr_line_num);
    let syntax = if loader_language_count == 0 {
      None
    } else {
      match doc.syntax() {
        Some(syntax) if syntax.root_language().idx() < loader_language_count => Some(syntax),
        _ => None,
      }
    };
    let indent = match line.first_non_whitespace_char() {
      Some(pos) if continue_comment_token.is_some() => line.slice(..pos).to_string(),
      _ => {
        indent_for_newline_safe(
          loader,
          syntax,
          &indent_heuristic,
          &doc.indent_style(),
          tab_width,
          text,
          above_next_new_line_num,
          above_next_line_end_index,
          curr_line_num,
        )
      },
    };

    let indent_len = indent.chars().count();
    let mut insert = String::with_capacity(1 + indent_len);

    if open == OpenDirection::Above && next_new_line_num == 0 {
      insert.push_str(&indent);
      if let Some(token) = continue_comment_token {
        insert.push_str(token);
        insert.push(' ');
      }
      insert.push_str(line_ending.as_str());
    } else {
      insert.push_str(line_ending.as_str());
      insert.push_str(&indent);
      if let Some(token) = continue_comment_token {
        insert.push_str(token);
        insert.push(' ');
      }
    }

    let insert = insert.repeat(count);

    let pos = offs + above_next_line_end_index + above_next_line_end_width;
    let comment_len = continue_comment_token
      .map(|token| token.len() + 1)
      .unwrap_or_default();
    for i in 0..count {
      ranges.push(Range::point(
        pos + (i * (line_ending.len_chars() + indent_len + comment_len)) + indent_len + comment_len,
      ));
    }

    offs += insert.chars().count();

    (
      above_next_line_end_index,
      above_next_line_end_index,
      Some(insert.into()),
    )
  });

  let Ok(tx) = tx else {
    return;
  };

  let cursor_ids: SmallVec<[CursorId; 1]> = selection.cursor_ids().iter().copied().collect();
  let new_selection = if cursor_ids.len() == ranges.len() {
    Selection::new_with_ids(ranges, cursor_ids).unwrap_or_else(|_| selection)
  } else {
    Selection::new(ranges).unwrap_or_else(|_| selection)
  };

  let tx = tx.with_selection(new_selection);
  let _ = ctx.apply_transaction(&tx);

  // Clamp selection to document bounds to avoid out-of-range cursor panics.
  {
    let doc = ctx.editor().document_mut();
    let max = doc.text().len_chars();
    let clamped = doc
      .selection()
      .clone()
      .transform(|range| Range::new(range.anchor.min(max), range.head.min(max)));
    let _ = doc.set_selection(clamped);
  }

  ctx.request_render();
}

pub fn command_from_name(name: &str) -> Option<Command> {
  match name {
    "move_char_left" => Some(Command::move_char_left(1)),
    "move_char_right" => Some(Command::move_char_right(1)),
    "move_char_up" => Some(Command::move_char_up(1)),
    "move_char_down" => Some(Command::move_char_down(1)),
    "move_visual_line_up" => Some(Command::move_visual_line_up(1)),
    "move_visual_line_down" => Some(Command::move_visual_line_down(1)),

    "extend_char_left" => Some(Command::extend_char_left(1)),
    "extend_char_right" => Some(Command::extend_char_right(1)),
    "extend_char_up" => Some(Command::extend_char_up(1)),
    "extend_char_down" => Some(Command::extend_char_down(1)),
    "extend_visual_line_up" => Some(Command::extend_visual_line_up(1)),
    "extend_visual_line_down" => Some(Command::extend_visual_line_down(1)),
    "extend_line_up" => Some(Command::extend_line_up(1)),
    "extend_line_down" => Some(Command::extend_line_down(1)),

    "move_next_word_start" => Some(Command::move_next_word_start(1)),
    "move_prev_word_start" => Some(Command::move_prev_word_start(1)),
    "move_next_word_end" => Some(Command::move_next_word_end(1)),
    "move_prev_word_end" => Some(Command::move_prev_word_end(1)),
    "move_next_long_word_start" => Some(Command::move_next_long_word_start(1)),
    "move_prev_long_word_start" => Some(Command::move_prev_long_word_start(1)),
    "move_next_long_word_end" => Some(Command::move_next_long_word_end(1)),
    "move_prev_long_word_end" => Some(Command::move_prev_long_word_end(1)),
    "move_next_sub_word_start" => Some(Command::move_next_sub_word_start(1)),
    "move_prev_sub_word_start" => Some(Command::move_prev_sub_word_start(1)),
    "move_next_sub_word_end" => Some(Command::move_next_sub_word_end(1)),
    "move_prev_sub_word_end" => Some(Command::move_prev_sub_word_end(1)),

    "extend_next_word_start" => Some(Command::extend_next_word_start(1)),
    "extend_prev_word_start" => Some(Command::extend_prev_word_start(1)),
    "extend_next_word_end" => Some(Command::extend_next_word_end(1)),
    "extend_prev_word_end" => Some(Command::extend_prev_word_end(1)),
    "extend_next_long_word_start" => Some(Command::extend_next_long_word_start(1)),
    "extend_prev_long_word_start" => Some(Command::extend_prev_long_word_start(1)),
    "extend_next_long_word_end" => Some(Command::extend_next_long_word_end(1)),
    "extend_prev_long_word_end" => Some(Command::extend_prev_long_word_end(1)),
    "extend_next_sub_word_start" => Some(Command::extend_next_sub_word_start(1)),
    "extend_prev_sub_word_start" => Some(Command::extend_prev_sub_word_start(1)),
    "extend_next_sub_word_end" => Some(Command::extend_next_sub_word_end(1)),
    "extend_prev_sub_word_end" => Some(Command::extend_prev_sub_word_end(1)),

    "extend_to_file_start" => Some(Command::extend_to_file_start()),
    "extend_to_file_end" => Some(Command::extend_to_file_end()),
    "extend_to_last_line" => Some(Command::extend_to_last_line()),
    "extend_to_column" => Some(Command::extend_to_column(1)),
    "goto_column" => Some(Command::goto_column(1)),

    "delete_word_backward" => Some(Command::delete_word_backward(1)),
    "delete_word_forward" => Some(Command::delete_word_forward(1)),
    "kill_to_line_start" => Some(Command::kill_to_line_start()),
    "kill_to_line_end" => Some(Command::kill_to_line_end()),

    "delete_char_backward" => Some(Command::DeleteChar),
    "delete_char_forward" => Some(Command::delete_char_forward(1)),

    "insert_newline" => Some(Command::insert_newline()),
    "insert_tab" | "smart_tab" => Some(Command::insert_tab()),

    "goto_line_start" => Some(Command::goto_line_start()),
    "extend_to_line_start" => Some(Command::extend_to_line_start()),
    "goto_first_nonwhitespace" => Some(Command::goto_first_nonwhitespace()),
    "extend_to_first_nonwhitespace" => Some(Command::extend_to_first_nonwhitespace()),
    "goto_line_end" => Some(Command::goto_line_end()),
    "extend_to_line_end" => Some(Command::extend_to_line_end()),

    "page_up" => Some(Command::page_up()),
    "page_down" => Some(Command::page_down()),
    "page_cursor_half_up" => Some(Command::page_cursor_half_up()),
    "page_cursor_half_down" => Some(Command::page_cursor_half_down()),

    "find_next_char" => Some(Command::find_next_char()),
    "find_till_char" => Some(Command::find_till_char()),
    "find_prev_char" => Some(Command::find_prev_char()),
    "till_prev_char" => Some(Command::till_prev_char()),
    "extend_next_char" => Some(Command::extend_next_char()),
    "extend_till_char" => Some(Command::extend_till_char()),
    "extend_prev_char" => Some(Command::extend_prev_char()),
    "extend_till_prev_char" => Some(Command::extend_till_prev_char()),

    "move_parent_node_end" => Some(Command::move_parent_node_end()),
    "extend_parent_node_end" => Some(Command::extend_parent_node_end()),
    "move_parent_node_start" => Some(Command::move_parent_node_start()),
    "extend_parent_node_start" => Some(Command::extend_parent_node_start()),
    "goto_file_start" => Some(Command::goto_file_start()),
    "goto_last_line" => Some(Command::goto_last_line()),
    "goto_next_buffer" => Some(Command::goto_next_buffer(1)),
    "goto_previous_buffer" => Some(Command::goto_previous_buffer(1)),
    "goto_window_top" => Some(Command::goto_window_top(1)),
    "goto_window_center" => Some(Command::goto_window_center()),
    "goto_window_bottom" => Some(Command::goto_window_bottom(1)),
    "rotate_view" => Some(Command::rotate_view()),
    "hsplit" => Some(Command::hsplit()),
    "vsplit" => Some(Command::vsplit()),
    "transpose_view" => Some(Command::transpose_view()),
    "wclose" => Some(Command::wclose()),
    "wonly" => Some(Command::wonly()),
    "jump_view_left" => Some(Command::jump_view_left()),
    "jump_view_down" => Some(Command::jump_view_down()),
    "jump_view_up" => Some(Command::jump_view_up()),
    "jump_view_right" => Some(Command::jump_view_right()),
    "swap_view_left" => Some(Command::swap_view_left()),
    "swap_view_down" => Some(Command::swap_view_down()),
    "swap_view_up" => Some(Command::swap_view_up()),
    "swap_view_right" => Some(Command::swap_view_right()),
    "goto_file_hsplit" => Some(Command::goto_file_hsplit()),
    "goto_file_vsplit" => Some(Command::goto_file_vsplit()),
    "hsplit_new" => Some(Command::hsplit_new()),
    "vsplit_new" => Some(Command::vsplit_new()),
    "goto_last_accessed_file" => Some(Command::goto_last_accessed_file()),
    "goto_last_modified_file" => Some(Command::goto_last_modified_file()),
    "goto_last_modification" => Some(Command::goto_last_modification()),
    "goto_word" => Some(Command::goto_word()),
    "extend_to_word" => Some(Command::extend_to_word()),
    "split_selection_on_newline" => Some(Command::split_selection_on_newline()),
    "merge_selections" => Some(Command::merge_selections()),
    "merge_consecutive_selections" => Some(Command::merge_consecutive_selections()),
    "split_selection" => Some(Command::split_selection()),
    "join_selections" => Some(Command::join_selections()),
    "join_selections_space" => Some(Command::join_selections_space()),
    "keep_selections" => Some(Command::keep_selections()),
    "remove_selections" => Some(Command::remove_selections()),
    "align_selections" => Some(Command::align_selections()),
    "keep_active_selection" => Some(Command::keep_active_selection()),
    "remove_active_selection" => Some(Command::remove_active_selection()),
    "trim_selections" => Some(Command::trim_selections()),
    "collapse_selection" => Some(Command::collapse_selection()),
    "flip_selections" => Some(Command::flip_selections()),
    "expand_selection" => Some(Command::expand_selection()),
    "shrink_selection" => Some(Command::shrink_selection()),
    "select_all_children" => Some(Command::select_all_children()),
    "select_all_siblings" => Some(Command::select_all_siblings()),
    "select_prev_sibling" => Some(Command::select_prev_sibling()),
    "select_next_sibling" => Some(Command::select_next_sibling()),
    "search" => Some(Command::search()),
    "rsearch" => Some(Command::rsearch()),
    "search_selection_detect_word_boundaries" => {
      Some(Command::search_selection_detect_word_boundaries())
    },
    "search_selection" => Some(Command::search_selection()),
    "select_regex" => Some(Command::select_regex()),
    "file_picker" => Some(Command::file_picker()),
    "lsp_goto_declaration" => Some(Command::lsp_goto_declaration()),
    "goto_declaration" => Some(Command::lsp_goto_declaration()),
    "lsp_goto_definition" => Some(Command::lsp_goto_definition()),
    "goto_definition" => Some(Command::lsp_goto_definition()),
    "lsp_goto_type_definition" => Some(Command::lsp_goto_type_definition()),
    "goto_type_definition" => Some(Command::lsp_goto_type_definition()),
    "lsp_goto_implementation" => Some(Command::lsp_goto_implementation()),
    "goto_implementation" => Some(Command::lsp_goto_implementation()),
    "lsp_hover" => Some(Command::lsp_hover()),
    "hover" => Some(Command::lsp_hover()),
    "lsp_references" => Some(Command::lsp_references()),
    "goto_reference" => Some(Command::lsp_references()),
    "lsp_document_symbols" => Some(Command::lsp_document_symbols()),
    "document_symbols" => Some(Command::lsp_document_symbols()),
    "lsp_workspace_symbols" => Some(Command::lsp_workspace_symbols()),
    "workspace_symbols" => Some(Command::lsp_workspace_symbols()),
    "lsp_completion" => Some(Command::lsp_completion()),
    "completion" => Some(Command::lsp_completion()),
    "completion_next" => Some(Command::completion_next()),
    "completion_prev" => Some(Command::completion_prev()),
    "completion_accept" => Some(Command::completion_accept()),
    "completion_cancel" => Some(Command::completion_cancel()),
    "completion_docs_scroll_up" => Some(Command::completion_docs_scroll_up()),
    "completion_docs_scroll_down" => Some(Command::completion_docs_scroll_down()),
    "lsp_signature_help" => Some(Command::lsp_signature_help()),
    "signature_help" => Some(Command::lsp_signature_help()),
    "lsp_code_actions" => Some(Command::lsp_code_actions()),
    "code_action" => Some(Command::lsp_code_actions()),
    "lsp_format" => Some(Command::lsp_format()),
    "format" => Some(Command::lsp_format()),
    "search_next" => Some(Command::search_next()),
    "search_prev" => Some(Command::search_prev()),
    "extend_search_next" => Some(Command::extend_search_next()),
    "extend_search_prev" => Some(Command::extend_search_prev()),

    "delete_selection" => Some(Command::delete_selection()),
    "delete_selection_noyank" => Some(Command::delete_selection_noyank()),
    "change_selection" => Some(Command::change_selection()),
    "change_selection_noyank" => Some(Command::change_selection_noyank()),
    "replace" => Some(Command::replace()),
    "replace_with_yanked" => Some(Command::replace_with_yanked()),
    "repeat_last_motion" => Some(Command::repeat_last_motion()),
    "switch_case" => Some(Command::switch_case()),
    "switch_to_uppercase" => Some(Command::switch_to_uppercase()),
    "switch_to_lowercase" => Some(Command::switch_to_lowercase()),
    "insert_at_line_start" => Some(Command::insert_at_line_start()),
    "insert_at_line_end" => Some(Command::insert_at_line_end()),
    "append_mode" => Some(Command::append_mode()),
    "open_below" => Some(Command::open_below()),
    "open_above" => Some(Command::open_above()),
    "commit_undo_checkpoint" => Some(Command::commit_undo_checkpoint()),
    "yank" => Some(Command::yank()),
    "paste_after" => Some(Command::paste_after()),
    "paste_before" => Some(Command::paste_before()),
    "record_macro" => Some(Command::record_macro()),
    "replay_macro" => Some(Command::replay_macro()),
    "copy_selection_on_next_line" => Some(Command::copy_selection_on_next_line()),
    "copy_selection_on_prev_line" => Some(Command::copy_selection_on_prev_line()),
    "select_all" => Some(Command::select_all()),
    "extend_line_below" => Some(Command::extend_line_below(1)),
    "extend_line_above" => Some(Command::extend_line_above(1)),
    "extend_to_line_bounds" => Some(Command::extend_to_line_bounds()),
    "shrink_to_line_bounds" => Some(Command::shrink_to_line_bounds()),
    "undo" => Some(Command::undo(1)),
    "redo" => Some(Command::redo(1)),
    "earlier" => Some(Command::earlier(1)),
    "later" => Some(Command::later(1)),
    "indent" => Some(Command::indent(1)),
    "unindent" => Some(Command::unindent(1)),
    "match_brackets" => Some(Command::match_brackets()),
    "surround_add" => Some(Command::surround_add()),
    "surround_delete" => Some(Command::surround_delete(1)),
    "surround_replace" => Some(Command::surround_replace(1)),
    "select_textobject_around" => Some(Command::select_textobject_around()),
    "select_textobject_inner" => Some(Command::select_textobject_inner()),
    "goto_prev_diag" => Some(Command::goto_prev_diag()),
    "goto_first_diag" => Some(Command::goto_first_diag()),
    "goto_next_diag" => Some(Command::goto_next_diag()),
    "goto_last_diag" => Some(Command::goto_last_diag()),
    "goto_prev_change" => Some(Command::goto_prev_change()),
    "goto_first_change" => Some(Command::goto_first_change()),
    "goto_next_change" => Some(Command::goto_next_change()),
    "goto_last_change" => Some(Command::goto_last_change()),
    "goto_prev_function" => Some(Command::goto_prev_function()),
    "goto_next_function" => Some(Command::goto_next_function()),
    "goto_prev_class" => Some(Command::goto_prev_class()),
    "goto_next_class" => Some(Command::goto_next_class()),
    "goto_prev_parameter" => Some(Command::goto_prev_parameter()),
    "goto_next_parameter" => Some(Command::goto_next_parameter()),
    "goto_prev_comment" => Some(Command::goto_prev_comment()),
    "goto_next_comment" => Some(Command::goto_next_comment()),
    "goto_prev_entry" => Some(Command::goto_prev_entry()),
    "goto_next_entry" => Some(Command::goto_next_entry()),
    "goto_prev_test" => Some(Command::goto_prev_test()),
    "goto_next_test" => Some(Command::goto_next_test()),
    "goto_prev_xml_element" => Some(Command::goto_prev_xml_element()),
    "goto_next_xml_element" => Some(Command::goto_next_xml_element()),
    "goto_prev_paragraph" => Some(Command::goto_prev_paragraph()),
    "goto_next_paragraph" => Some(Command::goto_next_paragraph()),
    "add_newline_above" => Some(Command::add_newline_above()),
    "add_newline_below" => Some(Command::add_newline_below()),

    _ => None,
  }
}

fn indent_for_newline_safe(
  loader: &Loader,
  syntax: Option<&the_lib::syntax::Syntax>,
  indent_heuristic: &IndentationHeuristic,
  indent_style: &the_lib::indent::IndentStyle,
  tab_width: usize,
  text: ropey::RopeSlice,
  line_before: usize,
  line_before_end_pos: usize,
  current_line: usize,
) -> String {
  match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    indent::indent_for_newline(
      loader,
      syntax,
      indent_heuristic,
      indent_style,
      tab_width,
      text,
      line_before,
      line_before_end_pos,
      current_line,
    )
  })) {
    Ok(indent) => indent,
    Err(_) => {
      let line = text.line(current_line);
      line
        .first_non_whitespace_char()
        .map(|pos| line.slice(..pos).to_string())
        .unwrap_or_default()
    },
  }
}
