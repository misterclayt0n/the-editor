//! Default dispatch behaviors and keymaps for the editor.
//!
//! This crate provides a reusable, policy-level layer that sits on top of
//! `the-lib` without hard-coding UI event types into `the-dispatch`.

mod command;
mod command_types;
mod command_registry;
mod input;
mod command_palette;
mod keymap;
mod pending;
mod render_pass;

pub use command_types::{
  Command,
  Direction,
  Motion,
  WordMotion,
};
pub use command::{
  DefaultApi,
  DefaultContext,
  DefaultDispatch,
  DefaultDispatchStatic,
  DispatchRef,
  build_dispatch,
  default_pre_on_keypress,
  render_plan,
  render_plan_with_styles,
  command_from_name,
  handle_command,
  handle_key,
};
pub use input::{
  Key,
  KeyEvent,
  KeyOutcome,
  Modifiers,
};
pub use pending::PendingInput;
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
pub use command_palette::{
  CommandPaletteLayout,
  CommandPaletteItem,
  CommandPaletteState,
  CommandPaletteStyle,
  CommandPaletteTheme,
  command_palette_default_selected,
  command_palette_filtered_indices,
  command_palette_selected_filtered_index,
  build_command_palette_overlay,
  build_command_palette_overlay_with_theme,
  build_command_palette_overlay_with_style,
  build_command_palette_overlay_bottom,
  build_command_palette_overlay_top,
};
pub use render_pass::{
  RenderPass,
  command_palette_overlay_pass,
  default_render_passes,
  run_render_passes,
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
