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

// Global allocator with thread-safe support
var gpa = std.heap.GeneralPurposeAllocator(.{
    .thread_safe = true,
}){};

// Allocation tracking
var allocations: std.ArrayList(*ghostty_vt.Terminal) = undefined;
var mutex = std.Thread.Mutex{};
var initialized = false;

fn ensureInit() void {
    mutex.lock();
    defer mutex.unlock();

    if (!initialized) {
        allocations = std.ArrayList(*ghostty_vt.Terminal).initCapacity(gpa.allocator(), 16) catch return;
        initialized = true;
    }
}

/// Create a new terminal with the specified dimensions
export fn ghostty_terminal_new(opts: *const CTerminalOptions) ?*GhosttyTerminal {
    ensureInit();

    mutex.lock();
    defer mutex.unlock();

    const term = gpa.allocator().create(ghostty_vt.Terminal) catch return null;
    errdefer gpa.allocator().destroy(term);

    // Convert cols/rows from u32 to the correct type (I'm assuming this is u16 btw but who knows)
    term.* = ghostty_vt.Terminal.init(gpa.allocator(), .{
        .cols = @intCast(opts.cols),
        .rows = @intCast(opts.rows),
    }) catch return null;

    allocations.append(gpa.allocator(), term) catch return null;

    return @ptrCast(@alignCast(term));
}

/// Free a terminal instance
export fn ghostty_terminal_free(term: ?*GhosttyTerminal) void {
    if (term == null) return;

    const actual_term: *ghostty_vt.Terminal = @ptrCast(@alignCast(term));

    mutex.lock();
    defer mutex.unlock();

    // Find and remove from allocations
    for (allocations.items, 0..) |alloc, i| {
        if (alloc == actual_term) {
            _ = allocations.swapRemove(i);
            break;
        }
    }

    // Deinit and free
    actual_term.deinit(gpa.allocator());
    gpa.allocator().destroy(actual_term);
}

/// Write a UTF-8 string to the terminal
export fn ghostty_terminal_print_string(term: ?*GhosttyTerminal, s: [*]const u8, len: usize) bool {
    if (term == null) return false;

    const actual_term: *ghostty_vt.Terminal = @ptrCast(@alignCast(term));
    actual_term.printString(s[0..len]) catch return false;
    return true;
}

/// Get the width of the terminal in columns
export fn ghostty_terminal_cols(term: ?*const GhosttyTerminal) u32 {
    if (term == null) return 0;

    const actual_term: *const ghostty_vt.Terminal = @ptrCast(@alignCast(term));
    return actual_term.cols;
}

/// Get the height of the terminal in rows
export fn ghostty_terminal_rows(term: ?*const GhosttyTerminal) u32 {
    if (term == null) return 0;

    const actual_term: *const ghostty_vt.Terminal = @ptrCast(@alignCast(term));
    return actual_term.rows;
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

    const actual_term: *const ghostty_vt.Terminal = @ptrCast(@alignCast(term));
    const point: ghostty_vt.Point = .{
        .active = .{
            .x = @intCast(pt.col),
            .y = @intCast(pt.row),
        },
    };

    if (actual_term.screen.pages.getCell(point)) |cell| {
        // cell.cell is a pointer to the actual page.Cell struct
        const actual_cell = cell.cell;
        return CCell{
            .codepoint = actual_cell.content.codepoint,
            .cluster = 0, // Not exposed in page.Cell
            .style = 0,    // Not exposed in page.Cell
            .hyperlink_id = 0,  // Not exposed in page.Cell
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

    const actual_term: *const ghostty_vt.Terminal = @ptrCast(@alignCast(term));
    return CPoint{
        .row = actual_term.screen.cursor.y,
        .col = actual_term.screen.cursor.x,
    };
}
