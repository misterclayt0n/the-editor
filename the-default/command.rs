use bitflags::bitflags;

/// Direction for cursor movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
  Up,
  Down,
  Left,
  Right,
}

bitflags! {
  /// Keyboard modifiers (minimal, generic).
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub struct Modifiers: u8 {
    const CTRL = 0b0000_0001;
    const ALT  = 0b0000_0010;
  }
}

impl Modifiers {
  pub fn ctrl(self) -> bool {
    self.contains(Self::CTRL)
  }

  pub fn alt(self) -> bool {
    self.contains(Self::ALT)
  }
}

/// Generic key representation (platform-neutral).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
  Char(char),
  Enter,
  Backspace,
  Left,
  Right,
  Up,
  Down,
}

/// Generic key event (platform-neutral).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
  pub key:       Key,
  pub modifiers: Modifiers,
}

/// Default command set for the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
  InsertChar(char),
  DeleteChar,
  Move(Direction),
  AddCursor(Direction),
  Save,
  Quit,
}
