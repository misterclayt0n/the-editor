//! Pending input state for commands that wait on the next keypress.

use crate::Direction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingInput {
  /// Placeholder variant for future expansions.
  Placeholder,
  /// Await a character to complete a find-char motion (f/t/F/T).
  FindChar {
    direction: Direction,
    inclusive: bool,
    extend:    bool,
    count:     usize,
  },
  /// Await a register name (e.g. insert-register).
  InsertRegister,
}
