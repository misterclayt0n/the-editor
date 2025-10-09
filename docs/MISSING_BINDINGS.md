## Missing Keybindings from Helix
This document is just so I can have a little basis btw

### Normal Mode

#### Left Bracket ([)

• "D"     -> goto_first_diag
• "g"     -> goto_prev_change
• "G"     -> goto_first_change
• "f"     -> goto_prev_function
• "t"     -> goto_prev_class
• "a"     -> goto_prev_parameter
• "c"     -> goto_prev_comment
• "e"     -> goto_prev_entry
• "T"     -> goto_prev_test
• "p"     -> goto_prev_paragraph
• "x"     -> goto_prev_xml_element
• "space" -> add_newline_above

#### Right Bracket (])

• "D"     -> goto_last_diag
• "g"     -> goto_next_change
• "G"     -> goto_last_change
• "f"     -> goto_next_function
• "t"     -> goto_next_class
• "a"     -> goto_next_parameter
• "c"     -> goto_next_comment
• "e"     -> goto_next_entry
• "T"     -> goto_next_test
• "p"     -> goto_next_paragraph
• "x"     -> goto_next_xml_element
• "space" -> add_newline_below

#### Search

• "/"   -> search
• "?"   -> rsearch
• "n"   -> search_next
• "N"   -> search_prev
• "*"   -> search_selection_detect_word_boundaries
• "A-*" -> search_selection

#### Editing

• "="   -> format_selections
• "J"   -> join_selections
• "A-J" -> join_selections_space
• "K"   -> keep_selections
• "A-K" -> remove_selections
• "&"   -> align_selections
• "_"   -> trim_selections

#### Selection Rotation

• "("   -> rotate_selections_backward
• ")"   -> rotate_selections_forward
• "A-(" -> rotate_selection_contents_backward
• "A-)" -> rotate_selection_contents_forward
• "A-:" -> ensure_selections_forward

#### Paging (in Normal mode)

• "C-b" | "pageup"   -> page_up
• "C-f" | "pagedown" -> page_down
• "C-u"              -> page_cursor_half_up
• "C-d"              -> page_cursor_half_down

#### Window Management (C-w)

• "C-w" | "w"           -> rotate_view
• "C-s" | "s"           -> hsplit
• "C-v" | "v"           -> vsplit
• "C-t" | "t"           -> transpose_view
• "f"                   -> goto_file_hsplit
• "F"                   -> goto_file_vsplit
• "C-q" | "q"           -> wclose
• "C-o" | "o"           -> wonly
• "C-h" | "h" | "left"  -> jump_view_left
• "C-j" | "j" | "down"  -> jump_view_down
• "C-k" | "k" | "up"    -> jump_view_up
• "C-l" | "l" | "right" -> jump_view_right
• "L"                   -> swap_view_right
• "K"                   -> swap_view_up
• "H"                   -> swap_view_left
• "J"                   -> swap_view_down
• "n"                   -> submenu for new split scratch buffer
 • "C-s" | "s"          -> hsplit_new
 • "C-v" | "v"          -> vsplit_new


#### Space submenu

• "F"   -> file_picker_in_current_directory
• "e"   -> file_explorer
• "E"   -> file_explorer_in_current_buffer_directory
• "b"   -> buffer_picker
• "j"   -> jumplist_picker
• "g"   -> changed_file_picker
• "'"   -> last_picker
• "w"   -> Window submenu (duplicate of C-w)
• "c"   -> toggle_comments
• "C"   -> toggle_block_comments
• "A-c" -> toggle_line_comments
• "/"   -> global_search
• "?"   -> command_palette

#### View submenu (z and Z)

• "z" | "c"           -> align_view_center
• "t"                 -> align_view_top
• "b"                 -> align_view_bottom
• "m"                 -> align_view_middle
• "k" | "up"          -> scroll_up
• "j" | "down"        -> scroll_down
• "C-b" | "pageup"    -> page_up
• "C-f" | "pagedown"  -> page_down
• "C-u" | "backspace" -> page_cursor_half_up
• "C-d" | "space"     -> page_cursor_half_down
• "/"                 -> search
• "?"                 -> rsearch
• "n"                 -> search_next
• "N"                 -> search_prev
• "Z" submenu         -> Sticky version of "z" (same bindings, but stays in view mode)

#### Misc

• "C-c"         -> toggle_comments
• "C-i" | "tab" -> jump_forward
• "C-o"         -> jump_backward
• "C-s"         -> save_selection
• "\""          -> select_register
• "|"           -> shell_pipe
• "A-|"         -> shell_pipe_to
• "!"           -> shell_insert_output
• "A-!"         -> shell_append_output
• "$"           -> shell_keep_pipe
• "C-z"         -> suspend
• "C-a"         -> increment
• "C-x"         -> decrement

---

### Select Mode

#### Selection Extension

• "A-e"  -> extend_parent_node_end
• "A-b"  -> extend_parent_node_start
• "n"    -> extend_search_next
• "N"    -> extend_search_prev
• "home" -> extend_to_line_start
• "end"  -> extend_to_line_end
• "v"    -> normal_mode (toggle back)

#### Goto submenu in Select mode

• "w" -> extend_to_word

---

### Insert Mode

• "C-r"      -> insert_register
• "pageup"   -> page_up
• "pagedown" -> page_down
