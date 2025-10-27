//! C API wrapper for ghostty-vt Terminal
//!
//! This Zig file provides C-callable exports around ghostty's Terminal
//! so Rust can use it via FFI.
//!
//! Note: This is compiled as a library (object files), not an executable.
//! The export functions here will be available for FFI from Rust.

const std = @import("std");
const ghostty_vt = @import("ghostty-vt");

// Type definitions for C compatibility
pub const CPoint = extern struct {
    row: i32,
    col: i32,
};

pub const CCell = extern struct {
    codepoint: u32,
    cluster: u32,
    style: u64,
    hyperlink_id: u32,
};

pub const CTerminalOptions = extern struct {
    cols: u32,
    rows: u32,
};

// Opaque type for FFI - use a larger type to ensure proper alignment
pub const GhosttyTerminal = extern struct {
    _: [8]u8,
};

// Minimal stream handler that forwards Stream callbacks to Terminal
// The Stream parser uses @hasDecl to check which methods exist, so we implement
// all the critical VT100/ANSI handlers needed for a functional terminal.
//
// This adapter translates Stream's callback interface (e.g., setCursorUp)
// to Terminal's methods (e.g., cursorUp), similar to Ghostty's StreamHandler.
const MinimalHandler = struct {
    terminal: *ghostty_vt.Terminal,

    // ===== Printable Characters =====

    pub inline fn print(self: *MinimalHandler, cp: u21) !void {
        return self.terminal.print(cp);
    }

    pub inline fn printRepeat(self: *MinimalHandler, count: usize) !void {
        return self.terminal.printRepeat(count);
    }

    // ===== Control Characters =====

    pub inline fn linefeed(self: *MinimalHandler) !void {
        // Use index() as it's equivalent to linefeed and slightly faster
        return self.terminal.index();
    }

    pub inline fn carriageReturn(self: *MinimalHandler) !void {
        self.terminal.carriageReturn();
    }

    pub inline fn backspace(self: *MinimalHandler) !void {
        self.terminal.backspace();
    }

    pub inline fn horizontalTab(self: *MinimalHandler, count: u16) !void {
        var i: u16 = 0;
        while (i < count) : (i += 1) {
            try self.terminal.horizontalTab();
        }
    }

    pub inline fn horizontalTabBack(self: *MinimalHandler, count: u16) !void {
        var i: u16 = 0;
        while (i < count) : (i += 1) {
            try self.terminal.horizontalTabBack();
        }
    }

    pub inline fn bell(self: *MinimalHandler) !void {
        // Bell is typically a no-op for headless terminals
        _ = self;
    }

    // ===== Cursor Movement =====

    pub inline fn setCursorLeft(self: *MinimalHandler, amount: u16) !void {
        self.terminal.cursorLeft(amount);
    }

    pub inline fn setCursorRight(self: *MinimalHandler, amount: u16) !void {
        self.terminal.cursorRight(amount);
    }

    pub inline fn setCursorDown(self: *MinimalHandler, amount: u16, carriage: bool) !void {
        self.terminal.cursorDown(amount);
        if (carriage) self.terminal.carriageReturn();
    }

    pub inline fn setCursorUp(self: *MinimalHandler, amount: u16, carriage: bool) !void {
        self.terminal.cursorUp(amount);
        if (carriage) self.terminal.carriageReturn();
    }

    pub inline fn setCursorCol(self: *MinimalHandler, col: u16) !void {
        self.terminal.setCursorPos(self.terminal.screen.cursor.y + 1, col);
    }

    pub inline fn setCursorColRelative(self: *MinimalHandler, offset: u16) !void {
        self.terminal.setCursorPos(
            self.terminal.screen.cursor.y + 1,
            self.terminal.screen.cursor.x + 1 +| offset,
        );
    }

    pub inline fn setCursorRow(self: *MinimalHandler, row: u16) !void {
        self.terminal.setCursorPos(row, self.terminal.screen.cursor.x + 1);
    }

    pub inline fn setCursorRowRelative(self: *MinimalHandler, offset: u16) !void {
        self.terminal.setCursorPos(
            self.terminal.screen.cursor.y + 1 +| offset,
            self.terminal.screen.cursor.x + 1,
        );
    }

    pub inline fn setCursorPos(self: *MinimalHandler, row: u16, col: u16) !void {
        self.terminal.setCursorPos(row, col);
    }

    // ===== Screen Manipulation =====

    pub inline fn eraseDisplay(
        self: *MinimalHandler,
        mode: ghostty_vt.EraseDisplay,
        protected: bool,
    ) !void {
        self.terminal.eraseDisplay(mode, protected);
    }

    pub inline fn eraseLine(
        self: *MinimalHandler,
        mode: ghostty_vt.EraseLine,
        protected: bool,
    ) !void {
        self.terminal.eraseLine(mode, protected);
    }

    pub inline fn eraseChars(self: *MinimalHandler, count: usize) !void {
        self.terminal.eraseChars(count);
    }

    pub inline fn deleteChars(self: *MinimalHandler, count: usize) !void {
        self.terminal.deleteChars(count);
    }

    pub inline fn insertBlanks(self: *MinimalHandler, count: usize) !void {
        self.terminal.insertBlanks(count);
    }

    pub inline fn insertLines(self: *MinimalHandler, count: usize) !void {
        self.terminal.insertLines(count);
    }

    pub inline fn deleteLines(self: *MinimalHandler, count: usize) !void {
        self.terminal.deleteLines(count);
    }

    // ===== Scrolling =====

    pub inline fn scrollUp(self: *MinimalHandler, count: usize) !void {
        self.terminal.scrollUp(count);
    }

    pub inline fn scrollDown(self: *MinimalHandler, count: usize) !void {
        self.terminal.scrollDown(count);
    }

    pub inline fn index(self: *MinimalHandler) !void {
        return self.terminal.index();
    }

    pub inline fn reverseIndex(self: *MinimalHandler) !void {
        self.terminal.reverseIndex();
    }

    pub inline fn nextLine(self: *MinimalHandler) !void {
        self.terminal.carriageReturn();
        try self.terminal.index();
    }

    // ===== Text Attributes (Colors, Bold, etc.) =====

    pub inline fn setAttribute(self: *MinimalHandler, attr: ghostty_vt.Attribute) !void {
        try self.terminal.setAttribute(attr);
    }

    // ===== Margins and Tabs =====

    pub inline fn setTopAndBottomMargin(self: *MinimalHandler, top: u16, bot: u16) !void {
        self.terminal.setTopAndBottomMargin(top, bot);
    }

    pub inline fn setLeftAndRightMarginAmbiguous(self: *MinimalHandler) !void {
        if (self.terminal.modes.get(.enable_left_and_right_margin)) {
            try self.setLeftAndRightMargin(0, 0);
        } else {
            try self.saveCursor();
        }
    }

    pub inline fn setLeftAndRightMargin(self: *MinimalHandler, left: u16, right: u16) !void {
        self.terminal.setLeftAndRightMargin(left, right);
    }

    pub inline fn tabClear(self: *MinimalHandler, cmd: ghostty_vt.TabClear) !void {
        self.terminal.tabClear(cmd);
    }

    pub inline fn tabSet(self: *MinimalHandler) !void {
        self.terminal.tabSet();
    }

    pub inline fn tabReset(self: *MinimalHandler) !void {
        self.terminal.tabReset();
    }

    // ===== Cursor State =====

    pub inline fn saveCursor(self: *MinimalHandler) !void {
        self.terminal.saveCursor();
    }

    pub inline fn restoreCursor(self: *MinimalHandler) !void {
        try self.terminal.restoreCursor();
    }

    // ===== Terminal Modes =====

    pub inline fn setMode(self: *MinimalHandler, mode: ghostty_vt.Mode, enabled: bool) !void {
        self.terminal.modes.set(mode, enabled);
    }

    // ===== Character Sets =====

    pub inline fn configureCharset(
        self: *MinimalHandler,
        slot: ghostty_vt.CharsetSlot,
        set: ghostty_vt.Charset,
    ) !void {
        self.terminal.configureCharset(slot, set);
    }

    pub inline fn invokeCharset(
        self: *MinimalHandler,
        table: ghostty_vt.CharsetActiveSlot,
        slot: ghostty_vt.CharsetSlot,
        single_shift: bool,
    ) !void {
        self.terminal.invokeCharset(table, slot, single_shift);
    }

    // ===== Full Reset =====

    pub inline fn fullReset(self: *MinimalHandler) !void {
        self.terminal.fullReset();
    }

    pub inline fn decaln(self: *MinimalHandler) !void {
        try self.terminal.decaln();
    }

    // ===== Protected Mode =====

    pub inline fn setProtectedMode(self: *MinimalHandler, mode: ghostty_vt.ProtectedMode) !void {
        self.terminal.setProtectedMode(mode);
    }
};

// Internal wrapper that combines Terminal with its Stream parser
const TerminalWrapper = struct {
    terminal: ghostty_vt.Terminal,
    handler: MinimalHandler,
    stream: ghostty_vt.Stream(*MinimalHandler),
};

// Global allocator with thread-safe support
var gpa = std.heap.GeneralPurposeAllocator(.{
    .thread_safe = true,
}){};

// Allocation tracking - now tracks wrappers instead of raw terminals
var allocations: std.ArrayList(*TerminalWrapper) = undefined;
var mutex = std.Thread.Mutex{};
var initialized = false;

fn ensureInit() void {
    mutex.lock();
    defer mutex.unlock();

    if (!initialized) {
        allocations = std.ArrayList(*TerminalWrapper).initCapacity(gpa.allocator(), 16) catch return;
        initialized = true;
    }
}

/// Create a new terminal with the specified dimensions
export fn ghostty_terminal_new(opts: *const CTerminalOptions) ?*GhosttyTerminal {
    ensureInit();

    mutex.lock();
    defer mutex.unlock();

    const wrapper = gpa.allocator().create(TerminalWrapper) catch return null;
    errdefer gpa.allocator().destroy(wrapper);

    // Initialize the terminal
    wrapper.terminal = ghostty_vt.Terminal.init(gpa.allocator(), .{
        .cols = @intCast(opts.cols),
        .rows = @intCast(opts.rows),
    }) catch return null;
    errdefer wrapper.terminal.deinit(gpa.allocator());

    // Initialize the handler that wraps the terminal
    wrapper.handler = MinimalHandler{ .terminal = &wrapper.terminal };

    // Initialize the stream with our handler
    wrapper.stream = ghostty_vt.Stream(*MinimalHandler).init(&wrapper.handler);

    allocations.append(gpa.allocator(), wrapper) catch return null;

    return @ptrCast(@alignCast(wrapper));
}

/// Free a terminal instance
export fn ghostty_terminal_free(term: ?*GhosttyTerminal) void {
    if (term == null) return;

    const wrapper: *TerminalWrapper = @ptrCast(@alignCast(term));

    mutex.lock();
    defer mutex.unlock();

    // Find and remove from allocations
    for (allocations.items, 0..) |alloc, i| {
        if (alloc == wrapper) {
            _ = allocations.swapRemove(i);
            break;
        }
    }

    // Deinit stream and terminal
    wrapper.stream.deinit();
    wrapper.terminal.deinit(gpa.allocator());
    gpa.allocator().destroy(wrapper);
}

/// Write a UTF-8 string to the terminal (DEPRECATED - use ghostty_terminal_write instead)
/// This function prints raw text without parsing escape sequences
export fn ghostty_terminal_print_string(term: ?*GhosttyTerminal, s: [*]const u8, len: usize) bool {
    if (term == null) return false;

    const wrapper: *TerminalWrapper = @ptrCast(@alignCast(term));
    wrapper.terminal.printString(s[0..len]) catch return false;
    return true;
}

/// Write raw bytes to the terminal, parsing VT100/ANSI escape sequences
/// This is the correct function to use for PTY output
export fn ghostty_terminal_write(term: ?*GhosttyTerminal, data: [*]const u8, len: usize) bool {
    if (term == null) return false;

    const wrapper: *TerminalWrapper = @ptrCast(@alignCast(term));
    wrapper.stream.nextSlice(data[0..len]) catch return false;
    return true;
}

/// Get the width of the terminal in columns
export fn ghostty_terminal_cols(term: ?*const GhosttyTerminal) u32 {
    if (term == null) return 0;

    const wrapper: *const TerminalWrapper = @ptrCast(@alignCast(term));
    return wrapper.terminal.cols;
}

/// Get the height of the terminal in rows
export fn ghostty_terminal_rows(term: ?*const GhosttyTerminal) u32 {
    if (term == null) return 0;

    const wrapper: *const TerminalWrapper = @ptrCast(@alignCast(term));
    return wrapper.terminal.rows;
}

/// Get a cell from the terminal grid
export fn ghostty_terminal_get_cell(term: ?*const GhosttyTerminal, pt: CPoint) CCell {
    if (term == null) {
        return CCell{
            .codepoint = 0,
            .cluster = 0,
            .style = 0,
            .hyperlink_id = 0,
        };
    }

    const wrapper: *const TerminalWrapper = @ptrCast(@alignCast(term));
    // Use viewport coordinates to get what's actually visible on screen
    // (not .active which is cursor-addressable space)
    const point: ghostty_vt.Point = .{
        .viewport = .{
            .x = @intCast(pt.col),
            .y = @intCast(pt.row),
        },
    };

    if (wrapper.terminal.screen.pages.getCell(point)) |cell| {
        // cell.cell is a pointer to the actual page.Cell struct
        const actual_cell = cell.cell;
        return CCell{
            // Use codepoint() method to properly handle all content_tag cases
            // (codepoint, codepoint_grapheme, color_palette, color_rgb)
            .codepoint = actual_cell.codepoint(),
            .cluster = 0, // Not directly exposed
            .style = actual_cell.style_id,    // Style ID from the cell
            .hyperlink_id = 0, // Hyperlink support removed or changed in newer ghostty
        };
    }

    return CCell{
        .codepoint = 0,
        .cluster = 0,
        .style = 0,
        .hyperlink_id = 0,
    };
}

/// Get the current cursor position
export fn ghostty_terminal_cursor_pos(term: ?*const GhosttyTerminal) CPoint {
    if (term == null) {
        return CPoint{ .row = 0, .col = 0 };
    }

    const wrapper: *const TerminalWrapper = @ptrCast(@alignCast(term));
    return CPoint{
        .row = wrapper.terminal.screen.cursor.y,
        .col = wrapper.terminal.screen.cursor.x,
    };
}

/// Resize the terminal to new dimensions
export fn ghostty_terminal_resize(term: ?*GhosttyTerminal, cols: u32, rows: u32) bool {
    if (term == null) return false;

    const wrapper: *TerminalWrapper = @ptrCast(@alignCast(term));
    wrapper.terminal.resize(
        gpa.allocator(),
        @intCast(cols),
        @intCast(rows),
    ) catch return false;
    return true;
}
