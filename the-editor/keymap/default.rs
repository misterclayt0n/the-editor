use std::collections::HashMap;

use super::{
  Command,
  KeyTrie,
  Mode,
};

pub fn default() -> HashMap<Mode, KeyTrie> {
  // Normal mode: hjkl + arrows move, 'i' enters insert
  let normal = crate::keymap!({ "Normal"
    "h" | Left  => move_char_left,
    "j" | Down  => move_visual_line_down,
    "k" | Up    => move_visual_line_up,
    "l" | Right => move_char_right,

    "w"         => move_next_word_start,
    "b"         => move_prev_word_start,
    "e"         => move_next_word_end,

    "W"         => move_next_long_word_start,
    "B"         => move_prev_long_word_start,
    "E"         => move_next_long_word_end,

    "f"         => find_next_char,
    "t"         => find_till_char,
    "F"         => find_prev_char,
    "T"         => till_prev_char,

    "d"         => delete_selection,
    "A-d"       => delete_selection_noyank,
    "c"         => change_selection,
    "A-c"       => change_selection_noyank,

    "r"         => replace,
    "R"         => replace_with_yanked,

    "A-."       => repeat_last_motion,

    "~"         => switch_case,
    "`"         => switch_to_lowercase,
    "A-`"       => switch_to_uppercase,

    "home"      => goto_line_start,
    "end"       => goto_line_end,

    "v"         => select_mode,
    "i"         => insert_mode,
    "I"         => insert_at_line_start,
    "a"         => append_mode,
    "A"         => insert_at_line_end,

    "o"         => open_below,
    "O"         => open_above,

    ":"         => command_mode,

    "y" => yank,
    "p" => paste_after,
    "P" => paste_before,
    
    "C"   => copy_selection_on_next_line,
    "A-C" => copy_selection_on_prev_line,

    "C-b" => toggle_debug_panel,
    "C-n" => toggle_button,
    
    "%"   => select_all,
    "x"   => extend_line_below,
    "X"   => extend_to_line_bounds,
    "A-x" => shrink_to_line_bounds,
    
    "u"   => undo,
    "U"   => redo,
    "A-u" => earlier,
    "A-U" => later,
    ","   => keep_primary_selection,
    "A-," => remove_primary_selection,
    
    ">" => indent,
    "<" => unindent,

    
    "Q" => record_macro,
    "q" => replay_macro,
    
    "m" => { "Match"
      "m" => match_brackets,
      "s" => surround_add,
      "r" => surround_replace,
      "d" => surround_delete,
      "a" => select_textobject_around,
      "i" => select_textobject_inner,
    },

    "G" => goto_line,
    "g" => { "Goto"
      "g" => goto_file_start,
      "|" => goto_column,
      "e" => goto_last_line,
      "h" => goto_line_start,
      "l" => goto_line_end,
      "s" => goto_first_nonwhitespace,
    },

    "space" => { "Space"
      "y" => yank_to_clipboard,
      "Y" => yank_main_selection_to_clipboard,
      "p" => paste_clipboard_after,
      "P" => paste_clipboard_before,
      "R" => replace_selections_with_clipboard,
    },
  });

  // Insert mode: text input handled via InputEvent::Text; map Esc and Backspace
  let mut insert = crate::keymap!({ "Insert"
    Backspace     => delete_char_backward,
    "C-j" | "ret" => insert_newline,
  });
  // Add: insert 'Esc' -> exit insert mode
  if let KeyTrie::Node(ref mut node) = insert {
    node
      .map
      .insert(crate::key!(Esc), KeyTrie::Command(Command::Execute(crate::core::commands::normal_mode)));
  }

  // Visual mode: movement extends selection, Esc exits visual mode
  let mut visual = crate::keymap!({ "Visual"
    'h' | Left  => extend_char_left,
    'j' | Down  => extend_visual_line_down,
    'k' | Up    => extend_visual_line_up,
    'l' | Right => extend_char_right,
    'f'         => extend_next_char,
    't'         => extend_till_char,
    'F'         => extend_prev_char,
    'T'         => extend_till_prev_char,
    'd'         => delete_selection,
  });
  // Add: visual 'Esc' -> exit visual mode
  if let KeyTrie::Node(ref mut node) = visual {
    node
      .map
      .insert(crate::key!(Esc), KeyTrie::Command(Command::Execute(crate::core::commands::normal_mode)));
  }

  let mut map = HashMap::new();
  map.insert(Mode::Normal, normal);
  map.insert(Mode::Insert, insert);
  map.insert(Mode::Select, visual);
  map
}
