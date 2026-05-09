import { dispatch, keymap, type EditorModule, type KeymapNodeSpec } from "@the-editor/editor";

export function defaultKeymaps(): EditorModule {
  return (editor) => {
    editor.interaction.registerProfile({
      id: "default.helix",
      title: "Default Helix",
      description: "First-party Helix-shaped interaction profile",
    });
    editor.interaction.useProfile("default.helix");

    const normalTree = keymap("Normal mode", {
        "h | left": "move_char_left",
        "j | down": "move_visual_line_down",
        "k | up": "move_visual_line_up",
        "l | right": "move_char_right",

        t: "find_till_char",
        f: "find_next_char",
        T: "till_prev_char",
        F: "find_prev_char",
        r: "replace",
        R: "replace_with_yanked",
        "A-.": "repeat_last_motion",

        "~": "switch_case",
        "`": "switch_to_lowercase",
        "A-`": "switch_to_uppercase",

        home: "goto_line_start",
        end: "goto_line_end",

        w: "move_next_word_start",
        b: "move_prev_word_start",
        e: "move_next_word_end",

        W: "move_next_long_word_start",
        B: "move_prev_long_word_start",
        E: "move_next_long_word_end",

        v: "select_mode",
        // G: "goto_line",
        g: keymap("Goto", {
          g: "goto_file_start",
          // "|": "goto_column",
          e: "goto_last_line",
          // f: "goto_file",
          h: "goto_line_start",
          l: "goto_line_end",
          s: "goto_first_nonwhitespace",
          // d: "goto_definition",
          // D: "goto_declaration",
          // y: "goto_type_definition",
          // r: "goto_reference",
          // i: "goto_implementation",
          // t: "goto_window_top",
          // c: "goto_window_center",
          // b: "goto_window_bottom",
          // a: "goto_last_accessed_file",
          // m: "goto_last_modified_file",
          // n: "goto_next_buffer",
          // p: "goto_previous_buffer",
          k: "extend_line_up",
          j: "extend_line_below",
          // ".": "goto_last_modification",
          // w: "goto_word",
        }),
        ":": dispatch("command_palette.open", { initialQuery: ":" }),

        i: "insert_mode",
        I: "insert_at_line_start",
        a: "append_mode",
        A: "insert_at_line_end",
        o: "open_below",
        O: "open_above",

        d: "delete_selection",
        "A-d": "delete_selection_noyank",
        c: "change_selection",
        "A-c": "change_selection_noyank",

        C: "copy_selection_on_next_line",
        "A-C": "copy_selection_on_prev_line",

        s: "select_regex",
        "A-s": "split_selection_on_newline",
        "A-minus": "merge_selections",
        "A-_": "merge_consecutive_selections",
        S: "split_selection",
        ";": "collapse_selection",
        "A-;": "flip_selections",
        // "A-o | A-up": "expand_selection",
        // "A-i | A-down": "shrink_selection",
        // "A-I | A-S-down": "select_all_children",
        // "A-p | A-left": "select_prev_sibling",
        // "A-n | A-right": "select_next_sibling",
        // "A-e": "move_parent_node_end",
        // "A-b": "move_parent_node_start",
        // "A-a": "select_all_siblings",

        "%": "select_all",
        x: "extend_line_below",
        X: "extend_to_line_bounds",
        "A-x": "shrink_to_line_bounds",

        m: keymap("Match", {
          m: "match_brackets",
          s: "surround_add",
          r: "surround_replace",
          d: "surround_delete",
          a: "select_textobject_around",
          i: "select_textobject_inner",
        }),
        // "[": keymap("Left bracket", { d/D/g/G/f/t/a/c/e/T/p/x/space }),
        // "]": keymap("Right bracket", { d/D/g/G/f/t/a/c/e/T/p/x/space }),

        "/": "search",
        "?": "rsearch",
        n: "search_next",
        N: "search_prev",
        "*": "search_selection_detect_word_boundaries",
        "A-*": "search_selection",

        u: "undo",
        U: "redo",
        "A-u": "earlier",
        "A-U": "later",

        y: "yank",
        p: "paste_after",
        P: "paste_before",

        // Q: "record_macro",
        // q: "replay_macro",

        ">": "indent",
        "<": "unindent",
        // "=": "format_selections",
        J: "join_selections",
        "A-J": "join_selections_space",
        K: "keep_selections",
        "A-K": "remove_selections",

        ",": "keep_active_selection",
        "A-,": "remove_active_selection",

        "&": "align_selections",
        _: "trim_selections",

        // "(": "rotate_selections_backward",
        // ")": "rotate_selections_forward",
        // "A-(": "rotate_selection_contents_backward",
        // "A-)": "rotate_selection_contents_forward",

        // "A-:": "ensure_selections_forward",

        esc: "normal_mode",
        // "C-b | pageup": "page_up",
        // "C-f | pagedown": "page_down",
        // "C-u": "page_cursor_half_up",
        // "C-d": "page_cursor_half_down",

        // "C-w": keymap("Window", { C-w/w, C-s/s, C-v/v, C-t/t, f/F, C-q/q, C-o/o, C-h/h/left, C-j/j/down, C-k/k/up, C-l/l/right, L/K/H/J, n }),

        // "C-c": "toggle_comments",

        // "C-i | tab": "jump_forward",
        // "C-o": "jump_backward",
        // "C-s": "save_selection",

        space: keymap("Space", {
          // f: "file_picker",
          // F: "file_picker_in_current_directory",
          // e: "file_explorer",
          // E: "edit_current_file_path",
          // b: "buffer_picker",
          // j: "jumplist_picker",
          // s: "lsp_document_symbols",
          // S: "lsp_workspace_symbols",
          // d: "diagnostics_picker",
          // D: "workspace_diagnostics_picker",
          // g: "changed_file_picker",
          // a: "lsp_code_actions",
          y: "yank_to_clipboard",
          Y: "yank_main_selection_to_clipboard",
          p: "paste_clipboard_after",
          P: "paste_clipboard_before",
          // R: "replace_selections_with_clipboard",
          // "/": "global_search",
          // k: "lsp_hover",
          // r: "rename_symbol",
          // h: "select_references_to_symbol_under_cursor",
          // c: "toggle_comments",
          // C: "toggle_block_comments",
          // "A-c": "toggle_line_comments",
          // "?": "command_palette",
        }),
        // z: keymap("View", { z/c/t/b/m/k/up/j/down/pages/search bindings }),
        // Z: keymap("View", { z/c/t/b/m/k/up/j/down/pages/search bindings }, { sticky: true }),

        // "\"": "select_register",
        // "|": "shell_pipe",
        // "A-|": "shell_pipe_to",
        // "!": "shell_insert_output",
        // "A-!": "shell_append_output",
        // $: "shell_keep_pipe",
        // "C-z": "suspend",
        "C-p": "command_palette.open",

        // "C-a": "increment",
        // "C-x": "decrement",
      });

    editor.keymaps.register({
      id: "default.helix.normal",
      scope: "normal",
      tree: normalTree,
    });

    editor.keymaps.register({
      id: "default.helix.select",
      scope: "select",
      tree: keymap("Select mode", {
        ...normalTree.bindings,
        "h | left": "extend_char_left",
        "j | down": "extend_visual_line_down",
        "k | up": "extend_visual_line_up",
        "l | right": "extend_char_right",

        w: "extend_next_word_start",
        b: "extend_prev_word_start",
        e: "extend_next_word_end",
        W: "extend_next_long_word_start",
        B: "extend_prev_long_word_start",
        E: "extend_next_long_word_end",

        // "A-e": "extend_parent_node_end",
        // "A-b": "extend_parent_node_start",

        n: "extend_search_next",
        N: "extend_search_prev",

        t: "extend_till_char",
        f: "extend_next_char",
        T: "extend_till_prev_char",
        F: "extend_prev_char",

        home: "extend_to_line_start",
        end: "extend_to_line_end",
        esc: "exit_select_mode",

        v: "normal_mode",
        m: keymap("Match", {
          m: "extend_match_brackets",
          s: "surround_add",
          r: "surround_replace",
          d: "surround_delete",
          a: "select_textobject_around",
          i: "select_textobject_inner",
        }),
        g: keymap("Goto", {
          ...((normalTree.bindings.g as KeymapNodeSpec).bindings),
          g: "extend_to_file_start",
          // "|": "extend_to_column",
          e: "extend_to_last_line",
          h: "extend_to_line_start",
          l: "extend_to_line_end",
          s: "extend_to_first_nonwhitespace",
          k: "extend_line_up",
          j: "extend_line_below",
          // w: "extend_to_word",
        }),
      }),
    });

    editor.keymaps.register({
      id: "default.helix.insert",
      scope: "insert",
      tree: keymap("Insert mode", {
        esc: "normal_mode",
        "C-p": "command_palette.open",

        // "C-s": "commit_undo_checkpoint",
        // "C-x": "completion",
        // "C-r": "insert_register",

        "C-w | A-backspace": "delete_word_backward",
        "A-d | A-del": "delete_word_forward",
        "C-u": "kill_to_line_start",
        "C-k": "kill_to_line_end",
        "C-h | backspace | S-backspace": "delete_char_backward",
        "C-d | del": "delete_char_forward",
        "C-j | ret": "insert_newline",
        tab: "smart_tab",
        "S-tab": "insert_tab",

        up: "move_visual_line_up",
        down: "move_visual_line_down",
        left: "move_char_left",
        right: "move_char_right",
        // pageup: "page_up",
        // pagedown: "page_down",
        home: "goto_line_start",
        end: "goto_line_end_newline",
      }),
    });
  };
}
