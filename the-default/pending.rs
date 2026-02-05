//! Pending input state for commands that wait on the next keypress.

use the_lib::text_object::TextObject;

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
  /// Await a character to replace the selection with.
  ReplaceSelection,
  /// Await a character to surround the selection with.
  SurroundAdd,
  /// Await a character to select which surround to delete.
  SurroundDelete { count: usize },
  /// Await a character to select which surround to replace.
  SurroundReplace { count: usize },
  /// Await a character to replace the surround with, after selecting positions.
  SurroundReplaceWith {
    positions:          Vec<(usize, usize)>,
    original_selection: Vec<(usize, usize)>,
  },
  /// Await a text-object key (w, W, p, etc.).
  SelectTextObject { kind: TextObject },
}
