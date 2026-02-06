//! Default dispatch behaviors and keymaps for the editor.
//!
//! This crate provides a reusable, policy-level layer that sits on top of
//! `the-lib` without hard-coding UI event types into `the-dispatch`.

mod command;
mod command_palette;
mod command_registry;
mod command_types;
mod file_picker;
mod input;
mod keymap;
mod pending;
mod search_prompt;
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
};
pub use command_types::{
  Command,
  Direction,
  Motion,
  WordMotion,
};
pub use file_picker::{
  FilePickerItem,
  FilePickerPreview,
  FilePickerState,
  build_file_picker_ui,
  close_file_picker,
  handle_file_picker_key,
  open_file_picker,
  set_picker_visible_rows,
  submit_file_picker,
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
pub use pending::PendingInput;
pub use search_prompt::{
  SearchPromptState,
  finalize_search,
  update_search_preview,
};
