## Missing Keybindings from Helix
This document is just so I can have a little basis btw

### Normal Mode

#### Editing

• "="   -> format_selections

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
