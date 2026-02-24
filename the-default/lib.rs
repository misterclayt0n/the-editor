#![recursion_limit = "512"]

//! Default dispatch behaviors and keymaps for the editor.
//!
//! This crate provides a reusable, policy-level layer that sits on top of
//! `the-lib` without hard-coding UI event types into `the-dispatch`.

mod command;
mod command_palette;
mod command_registry;
mod command_types;
mod completion_menu;
mod file_picker;
mod global_search;
mod input;
mod keymap;
mod message_bar;
mod overlay_layout;
mod pending;
mod search_prompt;
mod signature_help;
mod statusline;

pub use command::{
  DefaultApi,
  DefaultContext,
  DefaultDispatch,
  DefaultDispatchStatic,
  DispatchRef,
  build_dispatch,
  command_from_name,
  default_pre_on_keypress,
  frame_render_plan,
  frame_render_plan_with_styles,
  handle_command,
  handle_key,
  render_plan,
  render_plan_with_styles,
  ui_event,
  ui_tree,
};
pub use command_palette::{
  CommandPaletteItem,
  CommandPaletteLayout,
  CommandPaletteState,
  CommandPaletteStyle,
  CommandPaletteTheme,
  command_palette_default_selected,
  command_palette_filtered_indices,
  command_palette_selected_filtered_index,
};
pub use command_registry::{
  CommandCompleter,
  CommandError,
  CommandEvent,
  CommandPromptState,
  CommandRegistry,
  CommandResult,
  Completion,
  TypableCommand,
  completers,
  handle_command_prompt_key,
  update_command_palette_for_input,
};
pub use command_types::{
  Command,
  Direction,
  Motion,
  WordMotion,
};
pub use completion_menu::{
  CompletionMenuItem,
  CompletionMenuState,
  build_completion_menu_ui,
  close_completion_menu,
  completion_accept,
  completion_docs_scroll,
  completion_next,
  completion_prev,
  set_completion_docs_scroll,
  show_completion_menu,
};
pub use file_picker::{
  FilePickerChangedFileItem,
  FilePickerChangedKind,
  FilePickerConfig,
  FilePickerDiagnosticItem,
  FilePickerItem,
  FilePickerItemAction,
  FilePickerKind,
  FilePickerPreview,
  FilePickerPreviewLineKind,
  FilePickerPreviewSegment,
  FilePickerPreviewWindow,
  FilePickerPreviewWindowLine,
  FilePickerRowData,
  FilePickerRowKind,
  FilePickerSourcePreview,
  FilePickerState,
  build_file_picker_ui,
  close_file_picker,
  file_picker_icon_glyph,
  file_picker_icon_name_for_path,
  file_picker_kind_from_title,
  file_picker_preview_window,
  file_picker_row_data,
  handle_file_picker_key,
  handle_query_change,
  open_buffer_picker,
  open_changed_file_picker,
  open_custom_picker,
  open_diagnostics_picker,
  open_file_picker,
  open_file_picker_in_current_directory,
  open_file_picker_index,
  open_file_picker_with_root_and_split,
  open_jumplist_picker,
  poll_scan_results,
  refresh_matcher_state,
  replace_file_picker_items,
  scroll_file_picker_list,
  scroll_file_picker_preview,
  select_file_picker_index,
  set_file_picker_config,
  set_file_picker_list_offset,
  set_file_picker_preview_offset,
  set_file_picker_query_external,
  set_file_picker_syntax_loader,
  set_file_picker_wake_sender,
  set_picker_visible_rows,
  submit_file_picker,
  workspace_root,
};
pub use global_search::{
  GlobalSearchResponse,
  GlobalSearchState,
};
pub use input::{
  Key,
  KeyEvent,
  KeyOutcome,
  Modifiers,
};
pub use keymap::{
  KeyAction,
  KeyBinding,
  KeyTrie,
  KeyTrieNode,
  KeymapResult,
  Keymaps,
  Mode,
  action_from_name,
  default,
};
pub use message_bar::{
  MessagePresentation,
  build_message_bar_ui,
};
pub use overlay_layout::{
  OverlayRect,
  completion_docs_panel_rect,
  completion_panel_rect,
  signature_help_panel_rect,
};
pub use pending::PendingInput;
pub use search_prompt::{
  SearchPromptKind,
  SearchPromptState,
  finalize_keep_selections,
  finalize_remove_selections,
  finalize_search,
  finalize_select_regex,
  finalize_split_selection,
  update_keep_selections_preview,
  update_remove_selections_preview,
  update_search_preview,
  update_search_prompt_preview,
  update_select_regex_preview,
  update_split_selection_preview,
};
pub use signature_help::{
  SIGNATURE_HELP_ACTIVE_PARAM_END_MARKER,
  SIGNATURE_HELP_ACTIVE_PARAM_START_MARKER,
  SignatureHelpItem,
  SignatureHelpState,
  build_signature_help_ui,
  close_signature_help,
  show_signature_help,
  signature_help_docs_scroll,
  signature_help_markdown,
  signature_help_next,
  signature_help_prev,
};
pub use the_lib::messages::{
  Message,
  MessageCenter,
  MessageEvent,
  MessageEventKind,
  MessageLevel,
  MessageSnapshot,
};
