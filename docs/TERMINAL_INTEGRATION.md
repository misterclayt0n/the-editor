# Terminal Integration Guide

This document describes how the terminal emulator is integrated into the editor, its current architecture, and future improvement opportunities.

## Overview

The editor embeds the [Ghostty](https://ghostty.org) terminal emulator via FFI (Foreign Function Interface), providing users with an integrated terminal experience similar to VSCode or other modern editors. The terminal runs actual shell processes (bash, zsh, etc.) and supports VT100 escape sequences for full terminal emulation.

## Architecture

### Component Structure

The terminal integration spans three main crates:

```
the-terminal/          # Terminal emulation and PTY management
├── zig/
│   └── wrapper.zig   # C FFI wrapper for ghostty-vt
├── src/
│   ├── ffi.rs        # Rust FFI bindings to Zig wrapper
│   ├── terminal.rs   # Safe Rust wrapper around Terminal
│   ├── pty.rs        # PTY session management (pty-process)
│   └── terminal_session.rs  # Combined Terminal + PTY

the-editor/           # Editor integration
├── ui/components/
│   └── terminal.rs   # TerminalView component for rendering
├── terminal_manager.rs  # Manages multiple terminal instances
└── application.rs    # App integration and lifecycle
```

### Data Flow

```
User Input (keyboard)
    ↓
TerminalView::handle_event()
    ↓
key_to_bytes() - Convert KeyBinding → VT100 sequences
    ↓
TerminalSession::send_input()
    ↓
PtySession (async writer task)
    ↓
PTY → Shell Process

Shell Process → PTY output
    ↓
PtySession (async reader task)
    ↓
TerminalSession::update()
    ↓
Terminal::print_string() - Parse VT100 sequences
    ↓
ghostty-vt updates grid state
    ↓
TerminalView::render() - Read grid and render to screen
```

## Key Components

### 1. FFI Layer (`the-terminal/zig/wrapper.zig`)

**Purpose**: Bridge between Rust and Ghostty's Zig implementation

**Exports**:
- `ghostty_terminal_new()` - Create terminal instance
- `ghostty_terminal_free()` - Cleanup terminal
- `ghostty_terminal_print_string()` - Send VT100 sequences to parser
- `ghostty_terminal_get_cell()` - Read cell from grid
- `ghostty_terminal_cursor_pos()` - Get cursor position
- `ghostty_terminal_cols()` / `ghostty_terminal_rows()` - Get dimensions

**Key Structure**: `CCell`
```zig
pub const CCell = extern struct {
    codepoint: u32,      // Unicode character
    cluster: u32,        // Grapheme cluster (currently unused)
    style: u64,          // Style ID (0 = default)
    hyperlink_id: u32,   // Hyperlink identifier
};
```

### 2. Terminal Emulation (`the-terminal/src/terminal.rs`)

**Purpose**: Safe Rust wrapper around ghostty-vt FFI

**Key Type**: `Terminal`
```rust
pub struct Terminal {
    inner: *mut GhosttyTerminal,  // FFI pointer
}

impl Terminal {
    pub fn new(cols: u16, rows: u16) -> Result<Self>
    pub fn print_string(&mut self, s: &str) -> Result<()>
    pub fn get_cell(&self, row: u16, col: u16) -> Cell
    pub fn cursor_pos(&self) -> (u16, u16)
    pub fn grid(&self) -> Grid<'_>
}
```

**Responsibilities**:
- Memory safety around FFI pointers
- VT100 sequence parsing (delegated to ghostty-vt)
- Grid state management
- Send/Sync implementation for thread safety

### 3. PTY Management (`the-terminal/src/pty.rs`)

**Purpose**: Manage shell process and bidirectional I/O

**Key Type**: `PtySession`
```rust
pub struct PtySession {
    pty_write: Arc<Mutex<OwnedWritePty>>,
    child: tokio::process::Child,
    output_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    input_tx: mpsc::UnboundedSender<Vec<u8>>,
    rows: u16,
    cols: u16,
}
```

**Async Architecture**:
- **Reader task**: Continuously reads PTY output → sends to channel
- **Writer task**: Receives input from channel → writes to PTY (with flush!)
- **Main thread**: Non-blocking I/O via `try_recv_output()`

**Critical Implementation Detail**:
```rust
// Writer task MUST flush after each write for real-time interaction
writer.write_all(&data).await?;
writer.flush().await?;  // ← Essential for command execution
```

### 4. Terminal Session (`the-terminal/src/terminal_session.rs`)

**Purpose**: Combines Terminal + PTY into single interface

**Key Type**: `TerminalSession`
```rust
pub struct TerminalSession {
    terminal: Terminal,  // VT100 emulation
    pty: PtySession,     // Process I/O
}
```

**Main Loop Pattern**:
```rust
loop {
    session.update();  // Pull PTY output → feed to Terminal
    let grid = session.terminal().grid();
    // ... render grid ...
    session.send_input(user_input)?;
}
```

### 5. UI Component (`the-editor/ui/components/terminal.rs`)

**Purpose**: Integrate terminal into editor's Component system

**Key Type**: `TerminalView`
```rust
pub struct TerminalView {
    session: RefCell<TerminalSession>,  // Interior mutability for update()
    id: u32,
    dirty: bool,
    last_cols: u16,
    last_rows: u16,
}

impl Component for TerminalView {
    fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult
    fn render(&mut self, area: Rect, surface: &mut Surface, ctx: &mut Context)
    fn cursor(&self, area: Rect, ctx: &Editor) -> (Option<Position>, CursorKind)
}
```

**Key Input Handling**: `key_to_bytes()`

Maps editor key bindings to VT100 escape sequences:

```rust
Key::Up         → b"\x1b[A"
Key::Backspace  → b"\x7f"      // DEL, NOT \x08 (BS)
Key::Enter      → b"\r"
Key::Char('c') with Ctrl → [0x03]  // Ctrl+C
```

**Rendering Loop**:
```rust
fn render(&mut self, area: Rect, surface: &mut Surface, ctx: &mut Context) {
    self.session.borrow_mut().update();  // Pull PTY output
    let grid = session.terminal().grid();

    for row in 0..grid.rows() {
        for col in 0..grid.cols() {
            let cell = grid.get(row, col);
            if let Some(ch) = cell.character() {
                // Render character at (x, y)
                surface.draw_text_immediate(section);
            }
        }
    }

    // Render cursor
    let (cursor_row, cursor_col) = session.terminal().cursor_pos();
    surface.draw_rect(cursor_x, cursor_y, width, height, color);
}
```

### 6. Terminal Manager (`the-editor/terminal_manager.rs`)

**Purpose**: Manage multiple terminal instances (like tabs)

**Key Type**: `TerminalManager`
```rust
pub struct TerminalManager {
    terminals: HashMap<u32, TerminalView>,
    active_terminal: Option<u32>,
    next_id: u32,
}
```

**Responsibilities**:
- Create/destroy terminal instances
- Track active terminal
- Switch between terminals (not yet implemented in UI)
- Clean up dead terminals

### 7. Editor Integration (`the-editor/application.rs`)

**Command Flow**:
```
Keymap (Space + t)
    ↓
keymap/default.rs: "t" => spawn_terminal
    ↓
core/commands.rs: spawn_terminal(cx)
    ↓
Sets: cx.editor.pending_action = Some(Action::SpawnTerminal)
    ↓
application.rs: handle_pending_action()
    ↓
Creates TerminalView and pushes to Compositor
```

## Current Keybindings

- **Space + t**: Spawn new terminal (in normal mode)

## Current Limitations

### 1. No Color/Styling Support

**Status**: Basic infrastructure in place, but not fully implemented

**Current State**:
- All text renders as white on black
- Style IDs are extracted from cells but not decoded
- No bold, italic, underline rendering

**Why Complex**:
Ghostty's style system is sophisticated:
```rust
// Cell contains style_id, not the style itself
cell.style_id: u64  // Index into style map

// Actual style requires lookup in Terminal's style map
// Style contains:
struct Style {
    fg_color: Color,     // enum: none, palette(u8), rgb(RGB)
    bg_color: Color,
    underline_color: Color,
    flags: Flags,        // bold, italic, faint, blink, inverse, etc.
}
```

**What's Needed**:
1. Expose ghostty's style map through FFI
2. Implement style lookup in `terminal.rs`
3. Decode palette indices to RGB colors
4. Pass color/style info to renderer's TextSegment
5. Support text attributes (bold → different font weight, etc.)

### 2. No Terminal Resize on Editor Resize

**Current Behavior**: Terminal is created with fixed size (80x24)

**Issue**:
- `TerminalView::required_size()` returns fixed dimensions
- `Event::Resize` is handled but doesn't propagate to PTY properly
- PTY resize needs SIGWINCH signal to shell

**Solution Needed**:
```rust
fn render(&mut self, area: Rect, ...) {
    let new_cols = area.width / cell_width;
    let new_rows = area.height / cell_height;

    if new_cols != self.last_cols || new_rows != self.last_rows {
        self.session.borrow_mut().resize(new_rows, new_cols)?;
        // This should:
        // 1. Resize Terminal (needs FFI: ghostty_terminal_resize())
        // 2. Resize PTY (needs pty-process::Pty handle)
        // 3. Send SIGWINCH to shell
    }
}
```

**Blockers**:
- `pty-process` crate consumes `Pty` in `into_split()`, no handle retained
- Ghostty FFI doesn't expose `ghostty_terminal_resize()` yet
- Need to track PTY master file descriptor or use different PTY library

### 3. No Terminal Switching UI

**Current State**: `TerminalManager` can manage multiple terminals, but no UI

**Missing Features**:
- List active terminals (picker component)
- Switch between terminals (keybinding + UI indicator)
- Close terminal tab
- Status line showing active terminal

**Implementation Ideas**:
```rust
// Keybindings
"space" => {
    "t" => spawn_terminal,
    "T" => terminal_picker,  // NEW: show list of terminals
    "]t" => next_terminal,   // NEW: cycle forward
    "[t" => prev_terminal,   // NEW: cycle backward
}

// UI Component
fn terminal_picker(cx: &mut Context) {
    let terminals = cx.app.terminal_manager.list();
    let picker = Picker::new(
        terminals,
        |term| format!("Terminal {}: {}", term.id, term.shell_cwd()),
        |cx, term| cx.app.terminal_manager.switch_to(term.id),
    );
    cx.compositor.push(Box::new(picker));
}
```

### 4. No Terminal Mode

**Current Behavior**: Terminal input goes through normal editor keybindings first

**Problem**: Conflicts with editor shortcuts
- Example: Ctrl+S in terminal would save file, not send Ctrl+S to shell
- Example: Space in terminal triggers command palette, not types space

**Solution**: Dedicated "Terminal Mode"
```rust
enum EditorMode {
    Normal,
    Insert,
    Select,
    Terminal,  // NEW
}

impl TerminalView {
    fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult {
        match event {
            Event::Key(key) if key.code == Key::Escape => {
                // Exit terminal mode
                ctx.editor.mode = Mode::Normal;
                EventResult::Consumed(None)
            }
            Event::Key(key) => {
                // In terminal mode: ALL keys go to terminal (except Esc)
                let bytes = Self::key_to_bytes(key);
                self.send_input(bytes)?;
                EventResult::Consumed(None)
            }
            _ => EventResult::Ignored(None),
        }
    }
}
```

### 5. No Scrollback Support

**Current State**: Only shows visible grid (24 rows)

**Ghostty Capability**: Supports scrollback buffer of thousands of lines

**Missing Features**:
- Scroll up to view command history
- Search through scrollback
- Copy text from scrollback

**Implementation**:
```rust
// Ghostty has scrollback in `screen.pages.scrollback`
// Need FFI to access:
pub fn get_scrollback_line(&self, line: i32) -> Option<String>
pub fn scrollback_len(&self) -> usize

// TerminalView needs scroll offset:
struct TerminalView {
    scroll_offset: usize,  // Lines scrolled up from bottom
    ...
}

// Handle scroll events:
Event::Mouse(MouseEvent::ScrollUp) => {
    self.scroll_offset += 1;
}
```

### 6. No Shell Integration

**Current State**: Terminal is "dumb" - doesn't understand shell semantics

**Missing Features**:
- Detect current working directory (OSC 7)
- Semantic prompt detection (OSC 133)
- Jump between prompts
- Re-run previous commands
- Show exit codes

**Implementation**: Listen for OSC sequences
```rust
// Ghostty emits OSC 7 for CWD:
// ESC ] 7 ; file://hostname/path ST

// Need to hook into Terminal to intercept OSC sequences
impl TerminalView {
    fn on_osc_command(&mut self, cmd: OscCommand) {
        match cmd {
            OscCommand::ReportPwd(path) => {
                self.current_dir = path;
                // Update status line, file picker default, etc.
            }
            OscCommand::PromptStart => {
                self.prompts.push(self.session.terminal().cursor_pos());
            }
            _ => {}
        }
    }
}
```

### 7. No Terminal Persistence

**Current State**: Terminals die when editor closes

**Desired**:
- Detach terminals (like tmux)
- Reconnect to terminals on restart
- Serialize terminal state

**Very Complex**: Requires PTY detachment, process migration, or tmux-like architecture

### 8. Missing Visual Polish

**Current Issues**:
- No background colors (all cells transparent)
- No selection support (can't copy text)
- Cursor rendering is basic (solid rectangle)
- No cursor blinking
- No visual indication of terminal mode vs normal mode
- Fixed font (should respect editor font settings)

## Future Roadmap

### Phase 1: Core Functionality (HIGH PRIORITY)

1. **Implement Full Color Support**
   - Extract style map from ghostty via FFI
   - Decode foreground/background colors
   - Support 256-color palette + RGB
   - Render bold/italic/underline

2. **Terminal Resize**
   - Expose `ghostty_terminal_resize()` in FFI
   - Retain PTY handle for SIGWINCH
   - Update on editor window resize

3. **Terminal Mode**
   - Add Mode::Terminal to editor
   - Route ALL input to terminal when active
   - Escape key exits to normal mode
   - Visual indicator (status line)

### Phase 2: UX Improvements (MEDIUM PRIORITY)

4. **Terminal Switching UI**
   - Terminal picker (Space + T)
   - Cycle between terminals (]t / [t)
   - Status line shows terminal count
   - Close terminal command

5. **Scrollback Support**
   - PageUp/PageDown to scroll
   - Mouse wheel scrolling
   - Search in scrollback
   - Configurable scrollback limit

6. **Selection and Copy**
   - Mouse selection in terminal
   - Keyboard selection mode
   - Copy to clipboard (Space + y in terminal)
   - Integration with editor's clipboard

### Phase 3: Advanced Features (LOW PRIORITY)

7. **Shell Integration**
   - OSC 7: Track working directory
   - OSC 133: Semantic prompts
   - Jump between prompts (] Space / [ Space)
   - Show command exit codes
   - Re-run commands from picker

8. **Terminal Persistence**
   - Detach/reattach terminals
   - Save terminal state on exit
   - Restore terminals on startup

9. **Visual Polish**
   - Background colors
   - Better cursor rendering (styles: block, underline, beam)
   - Cursor blinking
   - Smooth scrolling animations
   - Configurable terminal theme

10. **Configuration**
    ```toml
    [terminal]
    shell = "$SHELL"
    scrollback_lines = 10000
    font_size = 12
    cursor_style = "block"
    cursor_blink = true
    colors = "theme"  # or "default"
    ```

## Testing Strategy

### Unit Tests

Current coverage:
- `the-terminal/src/terminal.rs`: FFI, grid operations
- `the-terminal/src/pty.rs`: Process lifecycle (limited - needs shell)
- `the-editor/ui/components/terminal.rs`: Key encoding

**Needed**:
- VT100 sequence parsing (verify ghostty handles correctly)
- Color decoding
- Resize behavior
- Terminal mode state transitions

### Integration Tests

**Needed**:
- Spawn terminal → run command → verify output
- Test backspace, arrow keys, Ctrl+C, etc.
- Multi-line input
- ANSI color sequences
- Scrollback behavior

### Manual Testing Checklist

- [ ] Basic commands: `ls`, `cd`, `echo`, `pwd`
- [ ] Interactive programs: `vim`, `htop`, `less`
- [ ] Multi-line input: Python REPL, bash here-docs
- [ ] Special keys: arrows, home/end, page up/down
- [ ] Ctrl sequences: Ctrl+C, Ctrl+Z, Ctrl+D
- [ ] Colors: `ls --color`, `git status`, `cargo build`
- [ ] Long output: `find /`, `cat large-file.txt`
- [ ] Terminal size: `echo $COLUMNS $LINES`, `tput cols`

## Performance Considerations

### Current Performance

**Good**:
- Async I/O prevents blocking main thread
- Non-blocking PTY reads via `try_recv_output()`
- Grid access is O(1) via ghostty-vt

**Concerns**:
- Re-render entire grid every frame (80×24 = 1,920 cells/frame)
- No dirty tracking per cell
- No render caching

### Optimization Opportunities

1. **Dirty Rectangle Tracking**
   ```rust
   struct TerminalView {
       dirty_region: Option<Rect>,  // Only re-render this region
   }
   ```

2. **Cell-Level Dirty Tracking**
   ```rust
   // Ghostty tracks dirty cells internally
   // Need FFI to expose: ghostty_terminal_get_dirty_cells()
   for (row, col) in terminal.dirty_cells() {
       render_cell(row, col);
   }
   ```

3. **Text Batching**
   ```rust
   // Instead of draw_text_immediate() per cell:
   let mut batch = TextBatch::new();
   for cell in row {
       batch.add(cell.char, x, y, style);
   }
   surface.draw_text_batch(batch);
   ```

4. **Limit Update Frequency**
   ```rust
   // Only call update() at max 60 FPS
   if now - last_update > 16ms {
       session.update();
       last_update = now;
   }
   ```

## Known Issues

1. **Backspace sends DEL (0x7F)** ✅ FIXED
   - Was sending BS (0x08) causing `^H` to appear
   - Changed in terminal.rs:161

2. **Commands don't execute** ✅ FIXED
   - PTY writer wasn't flushing
   - Added `flush()` after every `write_all()` in pty.rs:114

3. **No colors** ⚠️ PARTIAL
   - Style IDs extracted but not decoded
   - Needs full style map implementation

4. **Terminal size hardcoded** ⚠️ KNOWN
   - Fixed at 80×24
   - Needs resize implementation

5. **No multi-terminal UI** ⚠️ KNOWN
   - Manager exists but no picker/switcher

## References

- [Ghostty Documentation](https://ghostty.org)
- [VT100 Escape Sequences](https://vt100.net/docs/vt100-ug/chapter3.html)
- [Kitty Terminal Protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/)
- [OSC Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h3-Operating-System-Commands)
- [PTY Programming](https://man7.org/linux/man-pages/man7/pty.7.html)

## Contributing

When working on terminal integration:

1. **Test with real shells**: bash, zsh, fish
2. **Test interactive programs**: vim, htop, less
3. **Verify VT100 compliance**: Use `vttest` utility
4. **Check performance**: Monitor frame times with large output
5. **Follow NO GAMBIARRA**: Clean, modular, well-documented code

---

Last updated: 2025-10-27
