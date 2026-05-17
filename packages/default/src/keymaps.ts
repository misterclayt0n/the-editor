import { commandId, dispatch, keymap, keymapProfile, type EditorModule, type KeymapNodeSpec } from "@the-editor/editor";

export function defaultKeymaps(): EditorModule {
  return (editor) => {
    editor.interaction.registerProfile(keymapProfile({
      id: "default.helix",
      title: "Default Helix",
      description: "First-party Helix-shaped interaction profile",
      initialMode: "normal",
      modeSnapshots: {
        normal: { title: "NORMAL", cursorShape: "block", scopes: ["editor.normal"] },
        insert: { title: "INSERT", cursorShape: "bar", scopes: ["editor.insert"] },
        select: { title: "SELECT", cursorShape: "block", scopes: ["editor.select"] },
      },
      reduceUnmatched(ctx, input) {
        if (ctx.editor.input.current().kind === "editor" && ctx.state.mode === "insert" && input.text) {
          return {
            state: ctx.state,
            handled: true,
            effects: [{ kind: "command.dispatch", dispatch: { id: commandId("edit.insertText"), args: { text: input.text } } }],
          };
        }
        const focus = ctx.editor.input.current();
        if (focus.kind === "role" && focus.role === "commandPalette" && input.text) {
          return {
            state: ctx.state,
            handled: true,
            effects: [{ kind: "command.dispatch", dispatch: { id: commandId("commandPalette.appendText"), args: { text: input.text } } }],
          };
        }
        return { state: ctx.state, handled: false };
      },
    }));
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
          t: "goto_window_top",
          c: "goto_window_center",
          b: "goto_window_bottom",
          a: "goto_last_accessed_file",
          m: "goto_last_modified_file",
          n: "goto_next_buffer",
          p: "goto_previous_buffer",
          k: "extend_line_up",
          j: "extend_line_below",
          // ".": "goto_last_modification",
          // w: "goto_word",
        }),
        ":": "commandPalette.open",

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
        "A-o | A-up": "expand_selection",
        "A-i | A-down": "shrink_selection",
        "A-I | A-S-down": "select_all_children",
        "A-p | A-left": "select_prev_sibling",
        "A-n | A-right": "select_next_sibling",
        "A-e": "move_parent_node_end",
        "A-b": "move_parent_node_start",
        "A-a": "select_all_siblings",

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
        
        "[": keymap("Left bracket", {
          d: "goto_prev_diag",
          D: "goto_first_diag",
          g: "goto_prev_change",
          G: "goto_first_change",
          f: "goto_prev_function",
          t: "goto_prev_class",
          a: "goto_prev_parameter",
          c: "goto_prev_comment",
          e: "goto_prev_entry",
          T: "goto_prev_test",
          p: "goto_prev_paragraph",
          x: "goto_prev_xml_element",
          space: "add_newline_above",
        }),
        "]": keymap("Right bracket", {
          d: "goto_next_diag",
          D: "goto_last_diag",
          g: "goto_next_change",
          G: "goto_last_change",
          f: "goto_next_function",
          t: "goto_next_class",
          a: "goto_next_parameter",
          c: "goto_next_comment",
          e: "goto_next_entry",
          T: "goto_next_test",
          p: "goto_next_paragraph",
          x: "goto_next_xml_element",
          space: "add_newline_below",
        }),

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

        "A-:": "ensure_selections_forward",

        esc: "normal_mode",
        "C-b | pageup": "page_up",
        "C-f | pagedown": "page_down",
        "C-u": "page_cursor_half_up",
        "C-d": "page_cursor_half_down",

        "C-w": keymap("Window", {
          "C-w | w": "rotate_view",
          "C-s | s": "hsplit",
          "C-v | v": "vsplit",
          "C-t | t": "transpose_view",
          f: "goto_file_hsplit",
          F: "goto_file_vsplit",
          "C-q | q": "wclose",
          "C-o | o": "wonly",
          "C-h | h | left": "jump_view_left",
          "C-j | j | down": "jump_view_down",
          "C-k | k | up": "jump_view_up",
          "C-l | l | right": "jump_view_right",
          L: "swap_view_right",
          K: "swap_view_up",
          H: "swap_view_left",
          J: "swap_view_down",
          n: keymap("New split scratch buffer", {
            "C-s | s": "hsplit_new",
            "C-v | v": "vsplit_new",
          }),
        }),

        "C-c": "toggle_comments",

        "C-i | tab": "jump_forward",
        "C-o": "jump_backward",
        "C-s": "save_selection",

        space: keymap("Space", {
          f: "file_picker",
          // F: "file_picker_in_current_directory",
          // e: "file_explorer",
          // E: "edit_current_file_path",
          // b: "buffer_picker",
          // j: "jumplist_picker",
          d: "diagnostics_picker",
          D: "workspace_diagnostics_picker",
          // g: "changed_file_picker",
          y: "yank_to_clipboard",
          Y: "yank_main_selection_to_clipboard",
          p: "paste_clipboard_after",
          P: "paste_clipboard_before",
          // R: "replace_selections_with_clipboard",
          // "/": "global_search",
          k: "hover.show",
          h: "select_references_to_symbol_under_cursor",
          // c: "toggle_comments",
          // C: "toggle_block_comments",
          // "A-c": "toggle_line_comments",
          // "?": "command_palette",
        }),
        // z: keymap("View", { z/c/t/b/m/k/up/j/down/pages/search bindings }),
        // Z: keymap("View", { z/c/t/b/m/k/up/j/down/pages/search bindings }, { sticky: true }),

        "\"": "select_register",
        "|": "shell_pipe",
        "A-|": "shell_pipe_to",
        "!": "shell_insert_output",
        "A-!": "shell_append_output",
        $: "shell_keep_pipe",
        // "C-z": "suspend",
        "C-p | M-k": "commandPalette.open",

        "C-a": "increment",
        "C-x": "decrement",
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

        "A-e": "extend_parent_node_end",
        "A-b": "extend_parent_node_start",

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
        esc: "exit_insert_mode",
        "C-p | M-k": "commandPalette.open",

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
