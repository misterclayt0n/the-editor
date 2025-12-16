//! Terminal emulation crate for the-editor.
//!
//! This crate provides terminal emulation using `alacritty_terminal` as the backend.
//! It exposes a `Terminal` type that can be embedded as a view in the editor.

mod config;
mod event;
mod renderer;
mod terminal;

#[cfg(test)]
mod test_utils;

pub use config::TerminalConfig;
pub use event::TerminalEvent;
pub use renderer::{
  ColorScheme,
  CursorInfo,
  CursorShape,
  RenderCell,
};
pub use terminal::Terminal;

use std::num::NonZeroUsize;

/// Unique identifier for a terminal instance.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TerminalId(pub NonZeroUsize);

impl Default for TerminalId {
  fn default() -> Self {
    // SAFETY: 1 is non-zero
    TerminalId(unsafe { NonZeroUsize::new_unchecked(1) })
  }
}
