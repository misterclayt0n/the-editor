# Missing Commands Implementation Plan

This document tracks typable commands (`:command`) that need to be implemented in the-editor. Commands are organized by implementation priority.

**Legend:**
- ✅ Fully implemented
- 🚧 Partially implemented / needs improvement
- ❌ Not implemented

---

## Implementation Priority

### High 

1. ✅ `:write!` - Force write with directory creation
2. ✅ `:buffer-close!` - Force close buffer
3. ✅ `:quit-all` / `:quit-all!` - Close all views
4. ✅ `:write-all` / `:write-all-quit` - Multi-buffer operations
5. ❌ `:set-option` / `:get-option` / `:toggle-option` - Runtime config
6. ✅ `:goto` - Line number navigation with preview
7. ✅ `:vsplit` / `:hsplit` - Window splits
8. ❌ `:earlier` / `:later` - Undo/redo history navigation

### Medium

1. ❌ `:buffer-close-others` / `:buffer-close-all` - Buffer management
   - Flag: `--skip-visible` / `-s` - Don't close visible buffers
2. ❌ `:reload` / `:reload-all` - Discard changes and reload from disk
3. ❌ `:update` - Write only if modified
4. ❌ `:sort` - Selection sorting with flags (`--reverse`, `--insensitive`)
5. ❌ `:indent-style` / `:line-ending` - File formatting options
6. ✅ `:lsp-restart` / `:lsp-stop` - LSP management
7. ❌ `:set-language` - Manual language override
8. ❌ Shell commands (`:pipe`, `:insert-output`, `:append-output`, `:run-shell-command`)
9. ❌ `:change-current-directory` / `:show-directory` - Directory navigation
10. ❌ `:write-quit!` - Force write and quit
11. ❌ `:write-buffer-close` / `:write-buffer-close!` - Write and close buffer

### Lower

1. ❌ Clipboard commands (`:clipboard-yank`, `:clipboard-paste-*`, etc.)
2. ❌ Primary clipboard commands (X11 specific)
3. ❌ `:yank-join` - Custom separator yank
4. ❌ `:tree-sitter-*` - Tree-sitter debugging commands
5. ❌ `:reflow` - Hard-wrap text to given width
6. ❌ `:encoding` - Set/show file encoding
7. ❌ `:character-info` - Character inspection
8. ❌ `:config-open` / `:config-reload` / `:config-open-workspace` - Config file access
9. ❌ `:tutor` - Open tutorial
10. ❌ `:echo` / `:noop` - Utility commands
11. ❌ `:redraw` - Force UI redraw
12. ❌ `:move` - Rename/move current file
13. ❌ `:read` - Insert file contents at cursor
14. ❌ `:log-open` - Open log file

### Future Considerations

1. ❌ Debug Adapter Protocol commands (`:debug-*`)
2. ❌ `:reset-diff-change` - VCS integration
3. ❌ `:clear-register` / `:yank-diagnostic` - Advanced register operations
4. ❌ `:cquit` / `:cquit!` - Quit with exit code
5. ❌ `:show-clipboard-provider` - Clipboard provider info
6. ❌ `:lsp-workspace-command` - LSP workspace command picker

## Current Statistics

- **Total Helix Commands**: ~65 (including aliases)
- **Currently Implemented**: ~18
- **Partially Implemented**: ~2
- **Not Implemented**: ~45
- **Completion**: ~28%
