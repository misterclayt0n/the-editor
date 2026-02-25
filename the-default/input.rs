//! Input event types for editor dispatch.

use smallvec::SmallVec;

use crate::Command;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerButton {
  Left,
  Middle,
  Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerKind {
  Down(PointerButton),
  Drag(PointerButton),
  Up(PointerButton),
  Move,
  Scroll,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEvent {
  pub kind:        PointerKind,
  pub x:           i32,
  pub y:           i32,
  pub logical_col: Option<u16>,
  pub logical_row: Option<u16>,
  pub modifiers:   Modifiers,
  pub click_count: u8,
  pub scroll_x:    f32,
  pub scroll_y:    f32,
  pub surface_id:  Option<u64>,
}

impl PointerEvent {
  #[must_use]
  pub const fn new(kind: PointerKind, x: i32, y: i32) -> Self {
    Self {
      kind,
      x,
      y,
      logical_col: None,
      logical_row: None,
      modifiers: Modifiers::empty(),
      click_count: 0,
      scroll_x: 0.0,
      scroll_y: 0.0,
      surface_id: None,
    }
  }

  #[must_use]
  pub const fn with_logical_pos(mut self, col: u16, row: u16) -> Self {
    self.logical_col = Some(col);
    self.logical_row = Some(row);
    self
  }

  #[must_use]
  pub const fn with_modifiers(mut self, modifiers: Modifiers) -> Self {
    self.modifiers = modifiers;
    self
  }

  #[must_use]
  pub const fn with_click_count(mut self, click_count: u8) -> Self {
    self.click_count = click_count;
    self
  }

  #[must_use]
  pub const fn with_surface_id(mut self, surface_id: u64) -> Self {
    self.surface_id = Some(surface_id);
    self
  }

  #[must_use]
  pub const fn with_scroll_delta(mut self, scroll_x: f32, scroll_y: f32) -> Self {
    self.scroll_x = scroll_x;
    self.scroll_y = scroll_y;
    self
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerEventOutcome {
  #[default]
  Continue,
  Handled,
}

impl PointerEventOutcome {
  #[must_use]
  pub const fn handled(self) -> bool {
    matches!(self, Self::Handled)
  }

  #[must_use]
  pub const fn from_handled(handled: bool) -> Self {
    if handled { Self::Handled } else { Self::Continue }
  }
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
