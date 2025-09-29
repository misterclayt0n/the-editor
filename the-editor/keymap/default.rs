use std::collections::HashMap;

use super::{
  KeyTrie,
  Mode,
};

pub fn default() -> HashMap<Mode, KeyTrie> {
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

    "y"         => yank,
    "p"         => paste_after,
    "P"         => paste_before,

    "C"         => copy_selection_on_next_line,
    "A-C"       => copy_selection_on_prev_line,

    "C-b"       => toggle_debug_panel,
    "C-n"       => toggle_button,

    "%"         => select_all,
    "x"         => extend_line_below,
    "X"         => extend_to_line_bounds,
    "A-x"       => shrink_to_line_bounds,

    "u"         => undo,
    "U"         => redo,
    "A-u"       => earlier,
    "A-U"       => later,
    ","         => keep_primary_selection,
    "A-,"       => remove_primary_selection,

    ">"         => indent,
    "<"         => unindent,

    "Q"         => record_macro,
    "q"         => replay_macro,

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

  let insert = crate::keymap!({ "Insert"
    "esc"                               => normal_mode,

    "C-s"                               => commit_undo_checkpoint,
    // "C-x"                               => completion,
    // "C-r"                               => insert_register,

    "C-w" | "A-backspace"               => delete_word_backward,
    "A-d" | "A-del"                     => delete_word_forward,
    "C-u"                               => kill_to_line_start,
    "C-k"                               => kill_to_line_end,
    "C-h" | "backspace" | "S-backspace" => delete_char_backward,
    "C-d" | "del"                       => delete_char_forward,
    "C-j" | "ret"                       => insert_newline,
    "tab"                               => smart_tab,
    "S-tab"                             => insert_tab,

    "up"                                => move_visual_line_up,
    "down"                              => move_visual_line_down,
    "left"                              => move_char_left,
    "right"                             => move_char_right,
    // "pageup"                            => page_up,
    // "pagedown"                          => page_down,
    "home"                              => goto_line_start,
    "end"                               => goto_line_end_newline,
  });

  // Visual mode: movement extends selection, Esc exits visual mode
  let select = crate::keymap!({ "Visual"
   "esc"       => normal_mode,
    
    "h" | Left  => extend_char_left,
    "j" | Down  => extend_visual_line_down,
    "k" | Up    => extend_visual_line_up,
    "l" | Right => extend_char_right,

    "f"         => extend_next_char,
    "t"         => extend_till_char,
    "F"         => extend_prev_char,
    "T"         => extend_till_prev_char,


    "w"         => extend_next_word_start,
    "b"         => extend_prev_word_start,
    "e"         => extend_next_word_end,
    "W"         => extend_next_long_word_start,
    "B"         => extend_prev_long_word_start,
    "E"         => extend_next_long_word_end,
    
    "d"         => delete_selection,
    "c"         => change_selection,

    "g" => { "Goto"
      "g" => extend_to_file_start,
      "|" => extend_to_column,
      "e" => extend_to_last_line,
      "k" => extend_line_up,
      "j" => extend_line_down,
      // "w" => extend_to_word,
    },
  });

  let mut map = HashMap::new();
  map.insert(Mode::Normal, normal);
  map.insert(Mode::Insert, insert);
  map.insert(Mode::Select, select);
  map
}
