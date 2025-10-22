## Missing Keybindings from Helix
This document is just so I can have a little basis btw

The bindings left are mostly the ones I don't care about btw. Apart from the command_palette

### Normal Mode

#### Editing

• "="   -> format_selections

#### Space submenu

• "'"   -> last_picker
• "w"   -> Window submenu (duplicate of C-w)
• "c"   -> toggle_comments
• "C"   -> toggle_block_comments
• "A-c" -> toggle_line_comments
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

• "C-z"         -> suspend

### Insert Mode

• "C-r"      -> insert_register
• "pageup"   -> page_up
• "pagedown" -> page_down
