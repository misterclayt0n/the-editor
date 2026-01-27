//! Editor command types used by dispatch and clients.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
  Up,
  Down,
  Left,
  Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
  InsertChar(char),
  DeleteChar,
  Move(Direction),
  AddCursor(Direction),
  Save,
  Quit,
}
