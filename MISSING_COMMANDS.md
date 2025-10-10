# Missing Commands Tracking

This document tracks typable commands (`:command`) that are implemented in Helix but not yet in the-editor. Commands are organized by functional category for better tracking.

**Legend:**
- ✅ Fully implemented
- 🚧 Partially implemented / needs improvement
- ❌ Not implemented
- 🔧 Implementation in progress

---

## File Operations

### Basic File Commands
- ✅ `:open` (`:o`, `:e`, `:edit`) - Open a file for editing
- ✅ `:new` (`:n`) - Create a new buffer
- ✅ `:write` (`:w`) - Write buffer to file
- ❌ `:write!` (`:w!`) - Force write (create subdirectories)
- ❌ `:write-buffer-close` (`:wbc`) - Write and close buffer
- ❌ `:write-buffer-close!` (`:wbc!`) - Force write and close buffer
- ❌ `:update` (`:u`) - Write only if modified
- ❌ `:read` (`:r`) - Load a file into buffer at cursor position
- ❌ `:move` (`:mv`) - Move/rename current buffer file
- ❌ `:reload` (`:rl`) - Discard changes and reload from disk
- ❌ `:reload-all` (`:rla`) - Reload all documents from disk

### File Information
- ❌ `:encoding` - Set/show file encoding (based on WHATWG encoding spec)
- ❌ `:character-info` (`:char`) - Get info about character under cursor

---

## Buffer Management

### Buffer Navigation
- 🚧 `:buffer-next` (`:bn`, `:bnext`) - Go to next buffer (placeholder)
- 🚧 `:buffer-previous` (`:bp`, `:bprev`) - Go to previous buffer (placeholder)

### Buffer Closing
- ✅ `:buffer-close` (`:bc`, `:bclose`) - Close current buffer
- ❌ `:buffer-close!` (`:bc!`, `:bclose!`) - Force close buffer
- ❌ `:buffer-close-others` (`:bco`, `:bcloseother`) - Close all other buffers
  - Flag: `--skip-visible` / `-s` - Don't close visible buffers
- ❌ `:buffer-close-others!` (`:bco!`, `:bcloseother!`) - Force close others
- ❌ `:buffer-close-all` (`:bca`, `:bcloseall`) - Close all buffers
- ❌ `:buffer-close-all!` (`:bca!`, `:bcloseall!`) - Force close all

---

## Quit Commands

### Basic Quit
- ✅ `:quit` (`:q`) - Close the editor
- ✅ `:quit!` (`:q!`) - Force close without saving
- ✅ `:write-quit` (`:wq`, `:x`) - Write and quit
- ❌ `:write-quit!` (`:wq!`, `:x!`) - Force write and quit

### Multi-Buffer Quit
- ❌ `:write-all` (`:wa`) - Write all modified buffers
  - Flag: `--no-format` - Skip auto-formatting
- ❌ `:write-all!` (`:wa!`) - Force write all buffers
- ❌ `:write-all-quit` (`:wqa`, `:xa`) - Write all and quit
  - Flag: `--no-format` - Skip auto-formatting
- ❌ `:write-all-quit!` (`:wqa!`, `:xa!`) - Force write all and quit
- ❌ `:quit-all` (`:qa`) - Close all views
- ❌ `:quit-all!` (`:qa!`) - Force close all views
- ❌ `:cquit` (`:cq`) - Quit with exit code (default 1)
- ❌ `:cquit!` (`:cq!`) - Force quit with exit code

---

## View/Window Management

### Splits
- ❌ `:vsplit` (`:vs`) - Open file in vertical split
- ❌ `:vsplit-new` (`:vnew`) - Open scratch buffer in vertical split
- ❌ `:hsplit` (`:hs`, `:sp`) - Open file in horizontal split
- ❌ `:hsplit-new` (`:hnew`) - Open scratch buffer in horizontal split

---

## Editing Commands

### Formatting
- ✅ `:format` (`:fmt`) - Format current buffer

### Text Manipulation
- ❌ `:sort` - Sort ranges in selection
  - Flag: `--reverse` / `-r` - Reverse order
  - Flag: `--insensitive` / `-i` - Case insensitive
- ❌ `:reflow` - Hard-wrap selection to given width
- ❌ `:indent-style` - Set indentation style (tabs or 1-16 spaces)
- ❌ `:line-ending` - Set line ending (crlf, lf, cr, ff, nel)

### History
- ❌ `:earlier` (`:ear`) - Jump back in edit history (steps or time span)
- ❌ `:later` (`:lat`) - Jump forward in edit history (steps or time span)

---

## Shell Integration

- ❌ `:insert-output` - Run shell command, insert output before selections
- ❌ `:append-output` - Run shell command, append output after selections
- ❌ `:pipe` (`:│`) - Pipe selections through shell command
- ❌ `:pipe-to` - Pipe selections to shell command (ignore output)
- ❌ `:run-shell-command` (`:sh`, `:!`) - Run a shell command

---

## Clipboard Operations

### System Clipboard
- ❌ `:clipboard-yank` - Yank main selection to system clipboard
- ❌ `:clipboard-yank-join` - Yank joined selections to system clipboard
- ❌ `:clipboard-paste-after` - Paste system clipboard after selections
- ❌ `:clipboard-paste-before` - Paste system clipboard before selections
- ❌ `:clipboard-paste-replace` - Replace selections with clipboard

### Primary Clipboard (X11)
- ❌ `:primary-clipboard-yank` - Yank to primary clipboard
- ❌ `:primary-clipboard-yank-join` - Yank joined to primary clipboard
- ❌ `:primary-clipboard-paste-after` - Paste primary clipboard after
- ❌ `:primary-clipboard-paste-before` - Paste primary clipboard before
- ❌ `:primary-clipboard-paste-replace` - Replace with primary clipboard

### Clipboard Info
- ❌ `:show-clipboard-provider` - Show clipboard provider name
- ❌ `:yank-join` - Yank joined selections (with custom separator)

---

## Register Operations

- ❌ `:clear-register` - Clear given register (or all registers)
- ❌ `:yank-diagnostic` - Yank diagnostic messages to register

---

## LSP Commands

- ❌ `:lsp-workspace-command` - Open workspace command picker
- ❌ `:lsp-restart` - Restart language servers
- ❌ `:lsp-stop` - Stop language servers
- ❌ `:set-language` (`:lang`) - Set language of current buffer

---

## Tree-sitter Commands

- ❌ `:tree-sitter-scopes` - Display tree-sitter scopes (for theming)
- ❌ `:tree-sitter-highlight-name` - Display highlight scope under cursor
- ❌ `:tree-sitter-subtree` - Display tree-sitter subtree for selection

---

## Debug Adapter Protocol (DAP)

- ❌ `:debug-start` (`:dbg`) - Start debug session from template
- ❌ `:debug-remote` (`:dbg-tcp`) - Connect to debug adapter via TCP
- ❌ `:debug-eval` - Evaluate expression in debug context

---

## Configuration

### Runtime Config
- ❌ `:set-option` (`:set`) - Set config option at runtime
  - Example: `:set search.smart-case false`
- ❌ `:toggle-option` (`:toggle`) - Toggle boolean config option
  - Example: `:toggle search.smart-case`
- ❌ `:get-option` (`:get`) - Get current value of config option

### Config Files
- ❌ `:config-reload` - Refresh user config
- ❌ `:config-open` - Open user config.toml file
- ❌ `:config-open-workspace` - Open workspace config.toml file
- ❌ `:log-open` - Open the log file

---

## Navigation

- ❌ `:goto` (`:g`) - Goto line number
  - Should support preview while typing
- ❌ `:change-current-directory` (`:cd`) - Change working directory
- ❌ `:show-directory` (`:pwd`) - Show current working directory

---

## Version Control

- ❌ `:reset-diff-change` (`:diffget`, `:diffg`) - Reset diff change at cursor

---

## Theme

- ✅ `:theme` - Change editor theme (shows current if no arg)

---

## Miscellaneous

- ❌ `:tutor` - Open the tutorial
- ❌ `:echo` - Print arguments to statusline
- ❌ `:redraw` - Clear and re-render the whole UI
- ❌ `:noop` - Does nothing (useful for testing/keybinds)
- ✅ `:help` (`:h`) - Show help for commands

---

## Implementation Priority Suggestions

### High Priority (Core Functionality)
1. `:write!` - Force write with directory creation
2. `:buffer-close!` - Force close buffer
3. `:quit-all` / `:quit-all!` - Close all views
4. `:write-all` / `:write-all-quit` - Multi-buffer operations
5. `:set-option` / `:get-option` / `:toggle-option` - Runtime config
6. `:goto` - Line number navigation with preview
7. `:vsplit` / `:hsplit` - Window splits
8. `:earlier` / `:later` - Undo/redo history navigation

### Medium Priority (Enhanced Workflow)
1. `:buffer-close-others` / `:buffer-close-all` - Buffer management
2. `:reload` / `:reload-all` - Discard changes
3. `:update` - Conditional save
4. `:sort` - Selection sorting
5. `:indent-style` / `:line-ending` - File formatting options
6. `:lsp-restart` / `:lsp-stop` - LSP management
7. `:set-language` - Manual language override
8. Shell commands (`:pipe`, `:insert-output`, etc.)
9. `:change-current-directory` / `:show-directory` - Directory navigation

### Lower Priority (Nice to Have)
1. Clipboard commands (`:clipboard-yank`, etc.)
2. `:yank-join` - Custom separator yank
3. `:tree-sitter-*` - Tree-sitter debugging
4. `:reflow` - Text wrapping
5. `:encoding` - Encoding management
6. `:character-info` - Character inspection
7. `:config-open` / `:config-reload` - Config file access
8. `:tutor` - Tutorial
9. `:echo` / `:noop` - Utility commands
10. `:redraw` - Force redraw

### Future Considerations
1. Debug Adapter Protocol commands (`:debug-*`)
2. `:move` - File rename/move
3. `:read` - Insert file contents
4. `:reset-diff-change` - VCS integration
5. `:clear-register` / `:yank-diagnostic` - Advanced register ops
6. `:cquit` - Exit code control

---

## Implementation Notes

### Command Argument Parsing
Helix has a sophisticated argument parsing system with:
- **Flags**: `--no-format`, `--skip-visible`, `--reverse`, `--insensitive`
- **Flag aliases**: `-r` for `--reverse`, `-i` for `--insensitive`
- **Positional arguments**: With validation and type checking
- **Variadic arguments**: Commands that accept multiple args
- **Raw arguments**: Commands that parse args themselves (shell commands)
- **Expansion**: `%{variable}` and shell expansion in arguments

Current implementation uses simple space-split args. Consider:
1. Implementing a proper argument parser (see `helix_core::command_line`)
2. Adding flag support with `--flag` and `-f` syntax
3. Supporting quoted arguments with spaces
4. Adding variable expansion for file paths

### Command Categories to Consider
1. **Write flags**: Many write commands support `--no-format` flag
2. **Force variants**: Commands with `!` suffix force operations
3. **Multi-buffer operations**: Commands that operate on all buffers
4. **Preview support**: Commands like `:goto` show preview while typing
5. **Completers**: Each command needs appropriate argument completion

### Architecture Improvements Needed
1. **Event-based validation**: Commands like `:goto` need `PromptEvent` enum
   - `Validate`: Execute command
   - `Update`: Update preview
   - `Abort`: Cancel/restore state
2. **Job system**: Async commands (format, LSP, shell) need job queue
3. **Split management**: Commands need view/split tree manipulation
4. **Config system**: Runtime config needs event-based updates

---

## Current Statistics

- **Total Helix Commands**: ~65 (including aliases)
- **Currently Implemented**: ~12
- **Partially Implemented**: ~2
- **Not Implemented**: ~51
- **Completion**: ~18%

---

## Related Documents

- See `MISSING_BINDINGS.md` for keybinding implementation status
- See `CLAUDE.md` for project architecture and conventions
- See Helix's `helix-term/src/commands/typed.rs` for reference implementation
