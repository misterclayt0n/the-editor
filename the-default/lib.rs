//! Default dispatch behaviors and keymaps for the editor.
//!
//! This crate provides a reusable, policy-level layer that sits on top of
//! `the-lib` without hard-coding UI event types into `the-dispatch`.

mod command;
mod command_registry;
mod keymap;

pub use command::{
  Command,
  DefaultApi,
  DefaultContext,
  DefaultDispatch,
  DefaultDispatchStatic,
  Direction,
  DispatchRef,
  Key,
  KeyEvent,
  KeyOutcome,
  Modifiers,
  build_dispatch,
  command_from_name,
  handle_command,
  handle_key,
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
