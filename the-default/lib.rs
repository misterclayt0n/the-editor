//! Default dispatch behaviors and keymaps for the editor.
//!
//! This crate provides a reusable, policy-level layer that sits on top of
//! `the-lib` without hard-coding UI event types into `the-dispatch`.

mod command;
mod command_types;
mod command_registry;
mod input;
mod keymap;
mod pending;

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
