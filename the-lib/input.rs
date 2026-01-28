//! Input event types for editor dispatch.
//!
//! I don't like this but it kind of works

use smallvec::SmallVec;

use crate::command::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Modifiers {
  bits: u8,
}

impl Modifiers {
  pub const CTRL: u8 = 0b0000_0001;
  pub const ALT: u8 = 0b0000_0010;
  pub const SHIFT: u8 = 0b0000_0100;

  #[must_use]
  pub const fn empty() -> Self {
    Self { bits: 0 }
  }

  #[must_use]
  pub const fn is_empty(self) -> bool {
    self.bits == 0
  }

  #[must_use]
  pub const fn ctrl(self) -> bool {
    (self.bits & Self::CTRL) != 0
  }

  #[must_use]
  pub const fn alt(self) -> bool {
    (self.bits & Self::ALT) != 0
  }

  #[must_use]
  pub const fn shift(self) -> bool {
    (self.bits & Self::SHIFT) != 0
  }

  pub fn insert(&mut self, bits: u8) {
    self.bits |= bits;
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
  Char(char),
  Enter,
  NumpadEnter,
  Escape,
  Backspace,
  Tab,
  Delete,
  Insert,
  Home,
  End,
  PageUp,
  PageDown,
  Left,
  Right,
  Up,
  Down,
  F1,
  F2,
  F3,
  F4,
  F5,
  F6,
  F7,
  F8,
  F9,
  F10,
  F11,
  F12,
  Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
  pub key:       Key,
  pub modifiers: Modifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyOutcome {
  Continue,
  Handled,
  Command(Command),
  Commands(SmallVec<[Command; 4]>),
}

impl Default for KeyOutcome {
  fn default() -> Self {
    Self::Continue
  }
}
