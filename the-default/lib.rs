#![recursion_limit = "512"]

//! Default dispatch behaviors and keymaps for the editor.
//!
//! This crate provides a reusable, policy-level layer that sits on top of
//! `the-lib`.

mod buffer_tabs;
mod command;
mod command_palette;
mod command_registry;
mod command_types;
mod completion_menu;
mod context_menu;
mod defaults;
mod file_picker;
mod file_tree;
mod global_search;
mod increment;
mod input;
mod keymap;
mod message_bar;
mod overlay_layout;
mod pending;
mod render_waker;
mod search_prompt;
mod signature_help;
mod statusline;
mod theme_catalog;

pub use buffer_tabs::{
  BufferTabItemSnapshot,
  BufferTabsOrder,
  BufferTabsSnapshot,
  BufferTabsSnapshotOptions,
  activate_buffer_tab,
  buffer_tabs_snapshot,
  buffer_tabs_snapshot_for_editor,
  buffer_tabs_snapshot_for_editor_with_options,
  buffer_tabs_snapshot_with_options,
  close_buffer_tab,
};
pub use command::{
  DefaultApi,
  DefaultContext,
  DefaultDispatch,
  DefaultDispatchBuiltin,
  DispatchRef,
  WorkingDirectoryState,
  build_dispatch,
  command_from_name,
  default_pre_on_keypress,
  frame_render_plan,
  frame_render_plan_with_styles,
  handle_command,
  handle_key,
  handle_pointer_event,
  render_plan,
  render_plan_with_styles,
};
pub use command_palette::{
  CommandPaletteAction,
  CommandPaletteItem,
  CommandPaletteLayout,
  CommandPaletteSource,
  CommandPaletteState,
  CommandPaletteStyle,
  CommandPaletteTheme,
  command_palette_default_selected,
  command_palette_filtered_indices,
  command_palette_placeholder_text,
  command_palette_selected_filtered_index,
};
pub use command_registry::{
  CommandBuilder,
  CommandCompleter,
  CommandError,
  CommandEvent,
  CommandPromptState,
  CommandRegistry,
  CommandResult,
  Completion,
  DirectoryCompletionAction,
  TypableCommand,
  apply_command_palette_completion,
  command_palette_completion_action,
  completers,
  effective_working_directory,
  handle_command_prompt_key,
  install_builtin_commands,
  submit_command_palette,
  sync_command_palette_preview,
  update_action_palette_for_input,
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
  close_completion_menu,
  completion_accept,
  completion_docs_scroll,
  completion_next,
  completion_prev,
  set_completion_docs_scroll,
  show_builtin_completion_menu,
  show_completion_menu,
};
pub use context_menu::{
  ContextMenuActionId,
  ContextMenuItem,
  ContextMenuSection,
  ContextMenuSnapshot,
  EditorContextMenuOptions,
  EditorContextMenuRequest,
  build_editor_context_menu,
};
pub use defaults::{
  BuiltinCompletionMenuKind,
  CursorShapes,
  Defaults,
  EditorDefaults,
  TermDefaults,
  default_defaults,
  install_default_wiring,
};
pub use file_picker::{
  FilePickerChangedFileItem,
  FilePickerChangedKind,
  FilePickerDiagnosticItem,
  FilePickerItem,
  FilePickerItemAction,
  FilePickerItemPayload,
  FilePickerKind,
  FilePickerOptions,
  FilePickerPreview,
  FilePickerPreviewLineKind,
  FilePickerPreviewNavigationMode,
  FilePickerPreviewSegment,
  FilePickerPreviewWindow,
  FilePickerPreviewWindowLine,
  FilePickerRowData,
  FilePickerRowKind,
  FilePickerSourcePreview,
  FilePickerState,
  PickerBuilder,
  PickerItemSpec,
  PickerItemSpecAction,
  PickerRoot,
  PickerRuntimeSession,
  PickerRuntimeSessionId,
  PickerRuntimeStore,
  PickerSubmitHandlerRef,
  PickerSubmitResult,
  close_file_picker,
  file_picker_icon_glyph,
  file_picker_icon_name_for_path,
  file_picker_kind_from_title,
  file_picker_preview_window,
  file_picker_row_data,
  handle_file_picker_key,
  handle_query_change,
  move_selection,
  notify_file_picker_query_changed,
  open_buffer_picker,
  open_changed_file_picker,
  open_custom_picker,
  open_custom_picker_with_query_handler,
  open_diagnostics_picker,
  open_dynamic_picker,
  open_dynamic_picker_with_handler,
  open_file_picker,
  open_file_picker_in_current_directory,
  open_file_picker_index,
  open_file_picker_with_root_and_split,
  open_jumplist_picker,
  poll_scan_results,
  refresh_file_picker_preview,
  refresh_matcher_state,
  replace_file_picker_items,
  scroll_file_picker_list,
  scroll_file_picker_preview,
  select_file_picker_index,
  set_file_picker_list_offset,
  set_file_picker_options,
  set_file_picker_preview_offset,
  set_file_picker_query_text,
  set_file_picker_syntax_loader,
  set_file_picker_wake_sender,
  set_picker_visible_rows,
  submit_file_picker,
  workspace_root,
};
pub use file_tree::{
  FileTreeRow,
  FileTreeSnapshot,
  FileTreeState,
  activate_file_tree_index,
  close_file_tree,
  file_tree_snapshot,
  file_tree_surface_id,
  handle_file_tree_key,
  install_builtin_file_tree_commands,
  is_active_file_tree,
  is_file_tree_surface,
  refresh_file_tree,
  remember_active_editor_pane,
  reveal_current_file,
  scroll_file_tree,
  select_file_tree_index,
  set_file_tree_visible_rows,
  sync_file_tree_to_active_file,
  toggle_file_tree,
  toggle_file_tree_in_current_buffer_directory,
};
pub use global_search::{
  GlobalSearchDocumentSnapshot,
  GlobalSearchOptions,
  GlobalSearchResponse,
  GlobalSearchState,
};
pub use input::{
  Key,
  KeyEvent,
  KeyOutcome,
  Modifiers,
  PointerButton,
  PointerEvent,
  PointerEventOutcome,
  PointerKind,
};
pub use keymap::{
  IntoKeyBinding,
  KeyAction,
  KeyBinding,
  KeyTrie,
  KeyTrieNode,
  KeymapResult,
  Keymaps,
  Mode,
  ParseKeyBindingError,
  action_from_name,
  builtin_completion_menu_keymaps,
  builtin_keymaps,
  open_action_palette,
  open_action_palette_with_items,
  open_command_palette,
  open_command_palette_with_input,
};
pub use message_bar::MessagePresentation;
pub use overlay_layout::{
  OverlayRect,
  completion_docs_panel_rect,
  completion_panel_rect,
  signature_help_panel_rect,
};
pub use pending::PendingInput;
pub use render_waker::RenderWaker;
pub use search_prompt::{
  SearchPromptKind,
  SearchPromptState,
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
  open_rename_symbol_prompt,
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
  SignatureHelpPresentation,
  SignatureHelpState,
  close_signature_help,
  show_builtin_signature_help,
  show_signature_help,
  signature_help_docs_scroll,
  signature_help_markdown,
  signature_help_next,
  signature_help_prev,
};
pub use statusline::{
  StatuslineEmphasis,
  StatuslineSegment,
  StatuslineSnapshot,
  build_statusline_snapshot,
};
pub use the_lib::{
  editor::OpenTarget,
  messages::{
    Message,
    MessageCenter,
    MessageEvent,
    MessageEventKind,
    MessageLevel,
    MessageSnapshot,
  },
  render::{
    LineNumberMode,
    OwnedTextAnnotations,
    graphics::CursorKind,
  },
  split_tree::{
    PaneDirection,
    PaneId,
    PaneNeighbors,
    SplitAxis,
    SplitNodeId,
  },
};
pub use theme_catalog::ThemeCatalog;
