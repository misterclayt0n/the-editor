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
    "C-s"       => toggle_statusline,
    "C-g"       => toggle_line_numbers,
    "C-t"       => toggle_diff_gutter,
    "C-="       => increase_font_size,
    "C-minus"   => decrease_font_size,

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
      "d" => goto_definition,
      "D" => goto_declaration,
      "y" => goto_type_definition,
      "i" => goto_implementation,
      "r" => goto_reference,
      "f" => goto_file,
      "t" => goto_window_top,
      "c" => goto_window_center,
      "b" => goto_window_bottom,
      "a" => goto_last_accessed_file,
      "m" => goto_last_modified_file,
      "n" => goto_next_buffer,
      "p" => goto_previous_buffer,
      "k" => move_line_up,
      "j" => move_line_down,
      "." => goto_last_modification,
      "w" => goto_word,
    },

    "space" => { "Space"
      "f" => file_picker,
      "s" => document_symbols,
      "S" => workspace_symbols,
      "d" => document_diagnostics,
      "D" => workspace_diagnostics,
      "k" => hover,
      "a" => code_action,
      "r" => rename_symbol,
      "y" => yank_to_clipboard,
      "Y" => yank_main_selection_to_clipboard,
      "p" => paste_clipboard_after,
      "P" => paste_clipboard_before,
      "R" => replace_selections_with_clipboard,
      "h" => select_references,
    },
    
    "s"                => select_regex,
    "A-s"              => split_selection_on_newline,
    "A-minus"          => merge_selections,
    "A-_"              => merge_consecutive_selections,
    "S"                => split_selection,
    ";"                => collapse_selection,
    "A-;"              => flip_selections,
    "A-o" | "A-up"     => expand_selection,
    "A-i" | "A-down"   => shrink_selection,
    "A-I" | "A-S-down" => select_all_children,
    "A-p" | "A-left"   => select_prev_sibling,
    "A-n" | "A-right"  => select_next_sibling,
    "A-e"              => move_parent_node_end,
    "A-b"              => move_parent_node_start,
    "A-a"              => select_all_siblings,

    "[" => { "Left bracket"
      "d"     => goto_prev_diag,
      "D"     => goto_first_diag,
      "g"     => goto_prev_change,
      "G"     => goto_first_change,
      "f"     => goto_prev_function,
      "t"     => goto_prev_class,
      "a"     => goto_prev_parameter,
      "c"     => goto_prev_comment,
      "e"     => goto_prev_entry,
      "T"     => goto_prev_test,
      "p"     => goto_prev_paragraph,
      "x"     => goto_prev_xml_element,
      "space" => add_newline_above,
    },

    "]" => { "Right bracket"
      "d"     => goto_next_diag,
      "D"     => goto_last_diag,
      "g"     => goto_next_change,
      "G"     => goto_last_change,
      "f"     => goto_next_function,
      "t"     => goto_next_class,
      "a"     => goto_next_parameter,
      "c"     => goto_next_comment,
      "e"     => goto_next_entry,
      "T"     => goto_next_test,
      "p"     => goto_next_paragraph,
      "x"     => goto_next_xml_element,
      "space" => add_newline_below,
    },
    
    "/"   => search,
    "?"   => rsearch,
    "n"   => search_next,
    "N"   => search_prev,
    "*"   => search_selection_detect_word_boundaries,
    "A-*" => search_selection,
    
    "J"   => join_selections,
    "A-J" => join_selections_space,
    "K"   => keep_selections,
    "A-K" => remove_selections,
    "&"   => align_selections,
    "_"   => trim_selections,
    
    "("   => rotate_selections_backward,
    ")"   => rotate_selections_forward,
    "A-(" => rotate_selection_contents_backward,
    "A-)" => rotate_selection_contents_forward,
    
    "C-b" | "pageup"   => page_up,
    "C-f" | "pagedown" => page_down,
    "C-u"              => page_cursor_half_up,
    "C-d"              => page_cursor_half_down,
    
    "C-w" => { "Window"
      "C-w" | "w"           => rotate_view,
      "C-s" | "s"           => hsplit,
      "C-v" | "v"           => vsplit,
      "C-t" | "t"           => transpose_view,
      "f"                   => goto_file_hsplit,
      "F"                   => goto_file_vsplit,
      "C-q" | "q"           => wclose,
      "C-o" | "o"           => wonly,
      "C-h" | "h" | "left"  => jump_view_left,
      "C-j" | "j" | "down"  => jump_view_down,
      "C-k" | "k" | "up"    => jump_view_up,
      "C-l" | "l" | "right" => jump_view_right,
      "L"                   => swap_view_right,
      "K"                   => swap_view_up,
      "H"                   => swap_view_left,
      "J"                   => swap_view_down,
      
      "n" => { "New split scratch buffer"
          "C-s" | "s" => hsplit_new,
          "C-v" | "v" => vsplit_new,
      },
    },
    
    "C-c"         => toggle_comments,
    "C-i" | "tab" => jump_forward, // tab == <C-i>
    "C-o"         => jump_backward,
    "C-s"         => save_selection,
    "\""          => select_register,
    "|"           => shell_pipe,
    "A-|"         => shell_pipe_to,
    "!"           => shell_insert_output,
    "A-!"         => shell_append_output,
    "$"           => shell_keep_pipe,
    "C-a"         => increment,
    "C-x"         => decrement,
  });

  let insert = crate::keymap!({ "Insert"
    "esc"                               => normal_mode,

    "C-s"                               => commit_undo_checkpoint,
    "C-x"                               => completion,
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

  // Visual mode: inherits from Normal, overrides movement to extend selection
  let mut select = normal.clone();
  select.merge_nodes(crate::keymap!({ "Visual"
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

    "g" => { "Goto"
      "g" => extend_to_file_start,
      "|" => extend_to_column,
      "e" => extend_to_last_line,
      "k" => extend_line_up,
      "j" => extend_line_down,
      "w" => extend_to_word,
    },
  }));

  // Command mode: inherits from Normal, but blocks most keys for text input
  let mut command = normal.clone();
  command.merge_nodes(crate::keymap!({ "Command"
    "esc" => normal_mode,
    // Command mode specific bindings are handled by the Prompt component
    // This just provides fallback behavior for unhandled keys
  }));

  let mut map = HashMap::new();
  map.insert(Mode::Normal, normal);
  map.insert(Mode::Insert, insert);
  map.insert(Mode::Select, select);
  map.insert(Mode::Command, command);
  map
}
