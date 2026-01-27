use crate::{
  Command,
  Direction,
  Key,
  KeyEvent,
};

/// Default keymap (minimal, terminal-inspired).
#[derive(Debug, Default, Clone)]
pub struct DefaultKeyMap;

impl DefaultKeyMap {
  pub fn command_for_key(&self, key: KeyEvent) -> Option<Command> {
    command_for_key(key)
  }
}

/// Map a key event to a default command.
pub fn command_for_key(key: KeyEvent) -> Option<Command> {
  let ctrl = key.modifiers.ctrl();
  let alt = key.modifiers.alt();

  match key.key {
    Key::Char(c) if !ctrl && !alt => Some(Command::InsertChar(c)),
    Key::Enter if !ctrl && !alt => Some(Command::InsertChar('\n')),
    Key::Backspace => Some(Command::DeleteChar),

    Key::Left if !alt => Some(Command::Move(Direction::Left)),
    Key::Right if !alt => Some(Command::Move(Direction::Right)),
    Key::Up if !alt => Some(Command::Move(Direction::Up)),
    Key::Down if !alt => Some(Command::Move(Direction::Down)),

    Key::Up if alt => Some(Command::AddCursor(Direction::Up)),
    Key::Down if alt => Some(Command::AddCursor(Direction::Down)),

    Key::Char('s') if ctrl => Some(Command::Save),
    Key::Char('q') if ctrl => Some(Command::Quit),

    _ => None,
  }
}
