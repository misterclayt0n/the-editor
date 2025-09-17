use std::collections::HashMap;

use super::{
  Command,
  KeyTrie,
  Mode,
};

pub fn default() -> HashMap<Mode, KeyTrie> {
  // Normal mode: hjkl + arrows move, 'i' enters insert
  let mut normal = crate::keymap!({ "Normal"
    'h' | Left  => move_char_left,
    'j' | Down  => move_char_down,
    'k' | Up    => move_char_up,
    'l' | Right => move_char_right,
    'w'         => move_next_word_start,
    'b'         => move_prev_word_start,
    'e'         => move_next_word_end,
    'd'         => delete_selection,

    // Minimal examples of prefix maps
    'g' => { "Goto"
      'g' => move_char_left, // placeholder examples
      'e' => move_char_right,
    },
  });

  // Add: normal 'i' -> enter insert mode, 'v' -> enter visual mode
  if let KeyTrie::Node(ref mut node) = normal {
    node
      .map
      .insert(crate::key!('i'), KeyTrie::Command(Command::EnterInsertMode));
    node
      .map
      .insert(crate::key!('v'), KeyTrie::Command(Command::EnterVisualMode));
  }

  // Insert mode: text input handled via InputEvent::Text; map Esc and Backspace
  let mut insert = crate::keymap!({ "Insert"
    Backspace => delete_char_backward,
  });
  // Add: insert 'Esc' -> exit insert mode
  if let KeyTrie::Node(ref mut node) = insert {
    node
      .map
      .insert(crate::key!(Esc), KeyTrie::Command(Command::ExitInsertMode));
  }

  // Visual mode: movement extends selection, Esc exits visual mode
  let mut visual = crate::keymap!({ "Visual"
    'h' | Left  => extend_char_left,
    'j' | Down  => extend_char_down,
    'k' | Up    => extend_char_up,
    'l' | Right => extend_char_right,
  });
  // Add: visual 'Esc' -> exit visual mode
  if let KeyTrie::Node(ref mut node) = visual {
    node
      .map
      .insert(crate::key!(Esc), KeyTrie::Command(Command::ExitVisualMode));
  }

  let mut map = HashMap::new();
  map.insert(Mode::Normal, normal);
  map.insert(Mode::Insert, insert);
  map.insert(Mode::Select, visual);
  map
}

// Helper to expose mode switching commands to the editor executor if desired
// later.
#[allow(dead_code)]
pub const ENTER_INSERT: Command = Command::EnterInsertMode;
#[allow(dead_code)]
pub const EXIT_INSERT: Command = Command::ExitInsertMode;
