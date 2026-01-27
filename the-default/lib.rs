//! Default dispatch behaviors and keymaps for the editor.
//!
//! This crate provides a reusable, policy-level layer that sits on top of
//! `the-lib` without hard-coding UI event types into `the-dispatch`.

mod command;
mod keymap;
mod plugin;

pub use command::{
  Command,
  Direction,
  Key,
  KeyEvent,
  Modifiers,
};
pub use keymap::{
  DefaultKeyMap,
  command_for_key,
};
pub use plugin::{
  DefaultContext,
  DefaultPlugin,
  handle_key,
};
