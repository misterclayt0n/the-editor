//! C API wrapper for ghostty-vt Terminal
//!
//! This Zig file provides C-callable exports around ghostty's Terminal
//! so Rust can use it via FFI.
//!
//! Note: This is compiled as a library (object files), not an executable.
//! The export functions here will be available for FFI from Rust.

const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const modes = ghostty_vt.modes;
const SizeReportStyle = ghostty_vt.SizeReportStyle;
const osc = ghostty_vt.osc;
const DCS = ghostty_vt.DCS;

pub const std_options = std.Options{
    .log_level = .warn,
};

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

pub const CColor = extern struct {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    is_set: bool,
};

pub const CCellExt = extern struct {
    codepoint: u32,
    cluster: u32,
    style: u64,
    hyperlink_id: u32,
    fg: CColor,
    bg: CColor,
    underline: CColor,
    flags: u32,
    width: u8,
    selected: bool, // True if this cell is in the current selection
};

fn makeColor(rgb: ghostty_vt.color.RGB, is_set: bool) CColor {
    return CColor{
        .r = rgb.r,
        .g = rgb.g,
        .b = rgb.b,
        .a = 255,
        .is_set = is_set,
    };
}

const EMPTY_CELL_EXT = CCellExt{
    .codepoint = 0,
    .cluster = 0,
    .style = 0,
    .hyperlink_id = 0,
    .fg = makeColor(.{}, false),
    .bg = makeColor(.{}, false),
    .underline = makeColor(.{}, false),
    .flags = 0,
    .width = 0,
    .selected = false,
};

// Opaque type for FFI - use a larger type to ensure proper alignment
pub const GhosttyTerminal = extern struct {
    _: [8]u8,
};

// Callback function type for sending responses back to the PTY
// This is called when the terminal needs to send data back to the shell
// (e.g., cursor position reports, color queries, etc.)
pub const ResponseCallback = *const fn (ctx: ?*anyopaque, data: [*]const u8, len: usize) callconv(.c) void;

// Minimal stream handler that forwards Stream callbacks to Terminal
// The Stream parser uses @hasDecl to check which methods exist, so we implement
// all the critical VT100/ANSI handlers needed for a functional terminal.
//
// This adapter translates Stream's callback interface (e.g., setCursorUp)
// to Terminal's methods (e.g., cursorUp), similar to Ghostty's StreamHandler.
const MinimalHandler = struct {
    terminal: *ghostty_vt.Terminal,
    wrapper: *TerminalWrapper,
    callback: ?ResponseCallback, // Direct callback field
    callback_ctx: ?*anyopaque, // Direct context field

    /// Send a response back to the PTY (e.g., cursor position report)
    inline fn writeResponse(self: *MinimalHandler, data: []const u8) void {
        if (self.callback) |cb| {
            cb(self.callback_ctx, data.ptr, data.len);
        }
    }

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

    pub fn requestMode(self: *MinimalHandler, mode_raw: u16, ansi: bool) !void {
        const code: u8 = blk: {
            const mode = modes.modeFromInt(mode_raw, ansi) orelse break :blk 0;
            if (self.terminal.modes.get(mode)) break :blk 1;
            break :blk 2;
        };

        var buf: [32]u8 = undefined;
        const resp = std.fmt.bufPrint(
            &buf,
            "\x1B[{s}{};{}$y",
            .{ if (ansi) "" else "?", mode_raw, code },
        ) catch return;
        self.writeResponse(resp);
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

    // ===== Device Status Report (CRITICAL - prevents shell freezes) =====

    pub fn deviceStatusReport(
        self: *MinimalHandler,
        req: ghostty_vt.device_status.Request,
    ) !void {
        switch (req) {
            .operating_status => {
                // Report terminal is OK (0n = no malfunction)
                self.writeResponse("\x1B[0n");
            },
            .cursor_position => {
                // Report cursor position as ESC[{row};{col}R
                // This is CRITICAL - shells send ESC[6n and wait for this response
                const pos = self.terminal.screen.cursor;
                var buf: [32]u8 = undefined;
                const resp = std.fmt.bufPrint(&buf, "\x1B[{d};{d}R", .{
                    pos.y + 1, // VT100 uses 1-indexed positions
                    pos.x + 1,
                }) catch return;
                self.writeResponse(resp);
            },
            else => {
                // Ignore other DSR requests for now
            },
        }
    }

    pub fn sendSizeReport(self: *MinimalHandler, style: SizeReportStyle) void {
        const cols: u32 = @intCast(self.terminal.cols);
        const rows: u32 = @intCast(self.terminal.rows);
        const cell_w: u32 = if (self.wrapper.cell_width_px == 0) 1 else self.wrapper.cell_width_px;
        const cell_h: u32 = if (self.wrapper.cell_height_px == 0) 1 else self.wrapper.cell_height_px;

        var buf: [64]u8 = undefined;
        const response: ?[]u8 = switch (style) {
            .csi_14_t => std.fmt.bufPrint(&buf, "\x1B[4;{d};{d}t", .{ rows * cell_h, cols * cell_w }) catch return,
            .csi_16_t => std.fmt.bufPrint(&buf, "\x1B[6;{d};{d}t", .{ cell_h, cell_w }) catch return,
            .csi_18_t => std.fmt.bufPrint(&buf, "\x1B[8;{d};{d}t", .{ rows, cols }) catch return,
            .csi_21_t => null,
        };

        if (response) |resp| {
            self.writeResponse(resp);
        }
    }

    // ===== OSC Handlers (prevent warnings) =====

    pub fn changeWindowTitle(self: *MinimalHandler, title: []const u8) !void {
        // Store title if needed, but for now just prevent the warning
        _ = self;
        _ = title;
    }

    pub fn reportPwd(self: *MinimalHandler, url: []const u8) !void {
        // Could parse and store pwd for later use, but for now just prevent warning
        _ = self;
        _ = url;
    }

    pub fn promptStart(self: *MinimalHandler, aid: ?[]const u8, redraw: bool) !void {
        _ = aid;
        self.terminal.markSemanticPrompt(.prompt);
        self.terminal.flags.shell_redraws_prompt = redraw;
    }

    pub fn promptContinuation(self: *MinimalHandler, aid: ?[]const u8) !void {
        _ = aid;
        self.terminal.markSemanticPrompt(.prompt_continuation);
    }

    pub fn promptEnd(self: *MinimalHandler) !void {
        self.terminal.markSemanticPrompt(.input);
    }

    pub fn endOfInput(self: *MinimalHandler) !void {
        self.terminal.markSemanticPrompt(.command);
    }

    pub fn startHyperlink(self: *MinimalHandler, uri: []const u8, id: ?[]const u8) !void {
        _ = self;
        _ = uri;
        _ = id;
    }

    pub fn endHyperlink(self: *MinimalHandler) !void {
        _ = self;
    }

    pub fn endOfCommand(self: *MinimalHandler, exit_code: ?u8) !void {
        // Shell integration - record command exit status
        _ = self;
        _ = exit_code;
    }

    pub fn handleColorOperation(
        self: *MinimalHandler,
        op: osc.color.Operation,
        requests: *const osc.color.List,
        terminator: osc.Terminator,
    ) !void {
        if (requests.count() == 0) return;

        var it = requests.constIterator(0);
        while (it.next()) |req| {
            switch (req.*) {
                .set => |set| switch (set.target) {
                    .dynamic => |dynamic| switch (dynamic) {
                        .background => {
                            self.wrapper.background_color = .{
                                set.color.r,
                                set.color.g,
                                set.color.b,
                            };
                        },
                        .foreground => {
                            self.wrapper.foreground_color = .{
                                set.color.r,
                                set.color.g,
                                set.color.b,
                            };
                        },
                        else => {},
                    },
                    else => {},
                },
                .query => |target| {
                    const color_bytes = switch (target) {
                        .dynamic => |dynamic| switch (dynamic) {
                            .background => if (op == .osc_11)
                                self.wrapper.background_color
                            else
                                continue,
                            .foreground => if (op == .osc_10)
                                self.wrapper.foreground_color
                            else
                                continue,
                            else => continue,
                        },
                        else => continue,
                    };

                    // Respond with the appropriate OSC sequence
                    const osc_code: []const u8 = switch (op) {
                        .osc_10 => "10",
                        .osc_11 => "11",
                        else => continue,
                    };

                    var buf: [64]u8 = undefined;
                    const resp = std.fmt.bufPrint(
                        &buf,
                        "\x1B]{s};rgb:{x:0>2}/{x:0>2}/{x:0>2}{s}",
                        .{ osc_code, color_bytes[0], color_bytes[1], color_bytes[2], terminator.string() },
                    ) catch continue;
                    self.writeResponse(resp);
                },
                else => {},
            }
        }
    }

    pub fn dcsHook(self: *MinimalHandler, _header: DCS) !void {
        _ = self;
        _ = _header;
    }

    pub fn dcsPut(self: *MinimalHandler, _byte: u8) !void {
        _ = self;
        _ = _byte;
    }

    pub fn dcsUnhook(self: *MinimalHandler) !void {
        _ = self;
    }

    // ===== Device Attributes (prevent CSI [ c warnings) =====

    pub fn deviceAttributes(
        self: *MinimalHandler,
        req: ghostty_vt.DeviceAttributeReq,
        params: []const u16,
    ) !void {
        _ = params;

        // Report as VT220 with basic capabilities
        switch (req) {
            .primary => {
                // 62 = VT220 conformance level
                // 22 = Color text support
                self.writeResponse("\x1B[?62;22c");
            },
            .secondary => {
                // Report version info: VT220, firmware version 1.0
                self.writeResponse("\x1B[>1;10;0c");
            },
            else => {
                // Ignore tertiary and other requests
            },
        }
    }
};

// Internal wrapper that combines Terminal with its Stream parser
const TerminalWrapper = struct {
    terminal: ghostty_vt.Terminal,
    handler: MinimalHandler,
    stream: ghostty_vt.Stream(*MinimalHandler),
    cell_width_px: u16,
    cell_height_px: u16,
    background_color: [3]u8,
    foreground_color: [3]u8,
};

fn populateCellExt(
    wrapper: *const TerminalWrapper,
    page_ptr: *const ghostty_vt.Page,
    cell: *const ghostty_vt.page.Cell,
    pin_opt: ?*const ghostty_vt.PageList.Pin,
    cell_x: usize,
    out_cell: *CCellExt,
) void {
    var style_value: ghostty_vt.Style = .{};
    if (cell.*.style_id != 0) {
        style_value = page_ptr.styles.get(page_ptr.memory, cell.*.style_id).*;
    }

    const palette = &wrapper.terminal.color_palette.colors;
    const default_fg = palette[@intFromEnum(ghostty_vt.color.Name.white)];
    const default_bg = palette[@intFromEnum(ghostty_vt.color.Name.black)];

    var fg_rgb = style_value.fg(.{
        .default = default_fg,
        .palette = palette,
        .bold = null,
    });
    var bg_rgb = style_value.bg(cell, palette) orelse default_bg;

    // Check if this cell is selected (ghostty's approach)
    const selected = if (wrapper.terminal.screen.selection) |sel| blk: {
        if (pin_opt) |pin| {
            // Create a pin pointing to this specific cell
            const cell_pin: ghostty_vt.PageList.Pin = .{
                .node = pin.node,
                .y = pin.y,
                .x = @intCast(cell_x),
            };
            break :blk sel.contains(&wrapper.terminal.screen, cell_pin);
        }
        break :blk false;
    } else false;

    if (style_value.flags.inverse or wrapper.terminal.modes.get(.reverse_colors)) {
        const tmp = fg_rgb;
        fg_rgb = bg_rgb;
        bg_rgb = tmp;
    }

    const underline_rgb = style_value.underlineColor(palette);

    var flags: u32 = 0;
    if (style_value.flags.bold) flags |= 1 << 0;
    if (style_value.flags.italic) flags |= 1 << 1;
    if (style_value.flags.faint) flags |= 1 << 2;
    if (style_value.flags.inverse) flags |= 1 << 3;
    if (style_value.flags.blink) flags |= 1 << 4;
    if (style_value.flags.strikethrough) flags |= 1 << 5;
    if (style_value.flags.overline) flags |= 1 << 6;
    if (style_value.flags.underline != .none) flags |= 1 << 7;

    var underline_color = makeColor(.{}, false);
    if (underline_rgb) |u| {
        underline_color = makeColor(u, true);
    }

    const width: u8 = switch (cell.*.wide) {
        .wide => 2,
        .spacer_tail, .spacer_head => 0,
        else => 1,
    };

    out_cell.* = .{
        .codepoint = cell.codepoint(),
        .cluster = 0,
        .style = cell.*.style_id,
        .hyperlink_id = 0,
        .fg = makeColor(fg_rgb, true),
        .bg = makeColor(bg_rgb, true),
        .underline = underline_color,
        .flags = flags,
        .width = width,
        .selected = selected,
    };
}

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
    // Callback fields are null initially, will be set via ghostty_terminal_set_callback
    wrapper.handler = MinimalHandler{
        .terminal = &wrapper.terminal,
        .wrapper = wrapper,
        .callback = null,
        .callback_ctx = null,
    };

    // Initialize the stream with our handler
    wrapper.stream = ghostty_vt.Stream(*MinimalHandler).init(&wrapper.handler);

    // CRITICAL: Set allocator for OSC parser to handle color queries (OSC 10/11)
    // Without this, OSC 10/11 commands will log warnings and fail
    wrapper.stream.parser.osc_parser.alloc = gpa.allocator();

    wrapper.cell_width_px = 0;
    wrapper.cell_height_px = 0;
    wrapper.background_color = .{ 0, 0, 0 };
    wrapper.foreground_color = .{ 255, 255, 255 }; // Default to white foreground

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

export fn ghostty_terminal_set_cell_pixel_size(term: ?*GhosttyTerminal, width: u16, height: u16) void {
    if (term == null) return;

    const wrapper: *TerminalWrapper = @ptrCast(@alignCast(term));
    wrapper.cell_width_px = if (width == 0) 1 else width;
    wrapper.cell_height_px = if (height == 0) 1 else height;
}

export fn ghostty_terminal_set_background_color(term: ?*GhosttyTerminal, r: u8, g: u8, b: u8) void {
    if (term == null) return;

    const wrapper: *TerminalWrapper = @ptrCast(@alignCast(term));
    wrapper.background_color = .{ r, g, b };
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
            .style = actual_cell.style_id, // Style ID from the cell
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

/// Extended cell information including resolved colors and attributes
export fn ghostty_terminal_get_cell_ext(
    term: ?*const GhosttyTerminal,
    pt: CPoint,
    out_cell: *CCellExt,
) bool {
    if (term == null) {
        out_cell.* = CCellExt{
            .codepoint = 0,
            .cluster = 0,
            .style = 0,
            .hyperlink_id = 0,
            .fg = makeColor(.{}, false),
            .bg = makeColor(.{}, false),
            .underline = makeColor(.{}, false),
            .flags = 0,
            .width = 1,
            .selected = false,
        };
        return false;
    }

    const wrapper: *const TerminalWrapper = @ptrCast(@alignCast(term));
    const point: ghostty_vt.Point = .{
        .viewport = .{
            .x = @intCast(pt.col),
            .y = @intCast(pt.row),
        },
    };

    if (wrapper.terminal.screen.pages.getCell(point)) |cell| {
        const page_ptr: *const ghostty_vt.Page = &cell.node.data;
        // Create a pin for this cell using the original point coordinates
        // We need to pin the point to get the PageList.Pin structure for selection checking
        if (wrapper.terminal.screen.pages.pin(point)) |pin| {
            populateCellExt(wrapper, page_ptr, cell.cell, &pin, @intCast(pt.col), out_cell);
            return true;
        }
    }

    out_cell.* = EMPTY_CELL_EXT;
    return false;
}

/// Copy an entire row of extended cell information into an output buffer.
/// Returns the number of cells written, clamped to the provided max_len.
export fn ghostty_terminal_copy_row_cells_ext(
    term: ?*const GhosttyTerminal,
    row_index: u32,
    out_cells: [*]CCellExt,
    max_len: usize,
) usize {
    if (term == null or max_len == 0) return 0;

    const wrapper: *const TerminalWrapper = @ptrCast(@alignCast(term));
    if (row_index >= wrapper.terminal.rows) return 0;

    var pin = wrapper.terminal.screen.pages.pin(.{
        .viewport = .{
            .x = 0,
            .y = @intCast(row_index),
        },
    }) orelse return 0;

    pin.x = 0;

    const page_ptr: *const ghostty_vt.Page = &pin.node.data;
    const cells = pin.cells(.all);

    const visible_cols: usize = @intCast(wrapper.terminal.cols);
    const available: usize = @min(cells.len, visible_cols);
    const limit = @min(available, max_len);

    var i: usize = 0;
    while (i < limit) : (i += 1) {
        populateCellExt(wrapper, page_ptr, &cells[i], &pin, i, &out_cells[i]);
    }

    // Zero any remaining slots if we're truncating the row so that callers don't
    // accidentally read stale data.
    var j = limit;
    while (j < max_len) : (j += 1) {
        out_cells[j] = EMPTY_CELL_EXT;
    }

    return limit;
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

/// Set the callback for terminal responses (e.g., cursor position reports)
///
/// This must be called after ghostty_terminal_new() to enable bidirectional communication.
/// The callback will be invoked whenever the terminal needs to send data back to the PTY.
export fn ghostty_terminal_set_callback(
    term: ?*GhosttyTerminal,
    callback: ResponseCallback,
    ctx: ?*anyopaque,
) void {
    if (term == null) return;

    const wrapper: *TerminalWrapper = @ptrCast(@alignCast(term));
    // Set callback fields directly on the handler
    wrapper.handler.callback = callback;
    wrapper.handler.callback_ctx = ctx;
}

/// Get the dirty rows in the terminal
///
/// This returns a dynamically allocated array of row indices that need to be re-rendered.
/// The caller must free the returned array using ghostty_terminal_free_dirty_rows().
///
/// Returns: Pointer to array of u32 row indices, or null if no rows are dirty or on error
/// out_count: Set to the number of dirty rows
export fn ghostty_terminal_get_dirty_rows(
    term: ?*const GhosttyTerminal,
    out_count: *usize,
) ?[*]u32 {
    if (term == null) {
        out_count.* = 0;
        return null;
    }

    const wrapper: *const TerminalWrapper = @ptrCast(@alignCast(term));
    const screen = &wrapper.terminal.screen;

    // Use ghostty's row iterator for efficient traversal (same as ghostty's renderer)
    var row_it = screen.pages.rowIterator(.left_up, .{ .viewport = .{} }, null);

    // First pass: count dirty rows using row-level dirty bits
    var dirty_count: usize = 0;
    while (row_it.next()) |row| {
        if (row.isDirty()) {
            dirty_count += 1;
        }
    }

    if (dirty_count == 0) {
        out_count.* = 0;
        return null;
    }

    // Allocate array for dirty rows
    const dirty_rows = gpa.allocator().alloc(u32, dirty_count) catch {
        out_count.* = 0;
        return null;
    };

    // Second pass: fill array with dirty row indices
    // Reset iterator to start from top again
    row_it = screen.pages.rowIterator(.left_up, .{ .viewport = .{} }, null);
    var idx: usize = 0;
    var y: u32 = 0;
    while (row_it.next()) |row| {
        if (row.isDirty()) {
            dirty_rows[idx] = y;
            idx += 1;
        }
        y += 1;
    }

    out_count.* = dirty_count;
    return dirty_rows.ptr;
}

/// Free the dirty rows array returned by ghostty_terminal_get_dirty_rows()
export fn ghostty_terminal_free_dirty_rows(rows: ?[*]u32, count: usize) void {
    if (rows) |ptr| {
        const slice = ptr[0..count];
        gpa.allocator().free(slice);
    }
}

/// Clear all dirty bits in the terminal
///
/// This should be called after rendering all dirty rows to reset the dirty state.
/// Check if the terminal needs a full rebuild
///
/// Returns true if terminal-level or screen-level dirty flags are set,
/// indicating that a full screen rebuild is needed (not just dirty rows).
/// This matches Ghostty's own logic for determining full vs incremental renders.
export fn ghostty_terminal_needs_full_rebuild(term: ?*const GhosttyTerminal) bool {
    if (term == null) return false;

    const wrapper: *const TerminalWrapper = @ptrCast(@alignCast(term));

    // Check terminal-level dirty flags (e.g., from eraseDisplay, resize, etc.)
    // Use @typeInfo to get the backing integer type dynamically (same as Ghostty's renderer)
    {
        const Int = @typeInfo(ghostty_vt.Terminal.Dirty).@"struct".backing_integer.?;
        const v: Int = @bitCast(wrapper.terminal.flags.dirty);
        if (v > 0) return true;
    }

    // Check screen-level dirty flags
    {
        const Int = @typeInfo(ghostty_vt.Screen.Dirty).@"struct".backing_integer.?;
        const v: Int = @bitCast(wrapper.terminal.screen.dirty);
        if (v > 0) return true;
    }

    return false;
}

export fn ghostty_terminal_clear_dirty(term: ?*GhosttyTerminal) void {
    if (term == null) return;

    const wrapper: *TerminalWrapper = @ptrCast(@alignCast(term));

    // CRITICAL: Clear terminal-level and screen-level dirty flags
    // These are set by operations like eraseDisplay, resize, mode changes, etc.
    // Ghostty clears these BEFORE rendering (generic.zig:1216-1218)
    wrapper.terminal.flags.dirty = .{};
    wrapper.terminal.screen.dirty = .{};

    // Also clear row-level dirty bits
    const screen = &wrapper.terminal.screen;
    var it = screen.pages.pageIterator(.right_down, .{ .screen = .{} }, null);

    while (it.next()) |chunk| {
        var dirty_set = chunk.node.data.dirtyBitSet();
        dirty_set.unsetAll();
    }
}

// ===== PIN-BASED ZERO-COPY ITERATION =====
//
// Pins provide direct access to terminal page memory without copying.
// This is how Ghostty achieves zero-copy rendering.
//
// Usage pattern from Rust:
//   1. Create pin for row: ghostty_terminal_pin_row()
//   2. Get direct pointer to cells: ghostty_terminal_pin_cells()
//   3. Read cells directly (zero-copy)
//   4. Free pin: ghostty_terminal_pin_free()

/// Opaque pin handle for FFI
pub const GhosttyPin = extern struct {
    _: [8]u8,
};

/// Pin a specific row in the terminal viewport.
///
/// Returns an opaque handle that provides zero-copy access to cell data.
/// The caller MUST call ghostty_terminal_pin_free() when done.
///
/// Returns null if row is out of bounds or terminal is invalid.
export fn ghostty_terminal_pin_row(
    term: ?*const GhosttyTerminal,
    row: u32,
) ?*GhosttyPin {
    if (term == null) return null;

    const wrapper: *const TerminalWrapper = @ptrCast(@alignCast(term));
    if (row >= wrapper.terminal.rows) return null;

    // Pin the row in viewport coordinates
    const point: ghostty_vt.Point = .{
        .viewport = .{
            .x = 0,
            .y = @intCast(row),
        },
    };

    const pin = wrapper.terminal.screen.pages.pin(point) orelse return null;

    // Allocate heap memory for the pin (so it survives this function)
    const pin_ptr = gpa.allocator().create(ghostty_vt.PageList.Pin) catch return null;
    pin_ptr.* = pin;

    return @ptrCast(@alignCast(pin_ptr));
}

/// Get direct pointer to cell array from a pinned row.
///
/// Returns a pointer to the cell array and writes the count to out_count.
/// The returned pointer is valid until ghostty_terminal_pin_free() is called.
///
/// CRITICAL: The returned cells are ghostty internal cells, NOT CCellExt.
/// Use ghostty_terminal_pin_populate_cell_ext() to convert cells.
///
/// Returns null if pin is invalid.
export fn ghostty_terminal_pin_cells(
    term: ?*const GhosttyTerminal,
    pin: ?*GhosttyPin,
    out_count: *usize,
) ?[*]const ghostty_vt.page.Cell {
    if (term == null or pin == null) {
        out_count.* = 0;
        return null;
    }

    const wrapper: *const TerminalWrapper = @ptrCast(@alignCast(term));
    const pin_ptr: *ghostty_vt.PageList.Pin = @ptrCast(@alignCast(pin));

    // Get cells from the pinned row
    const cells = pin_ptr.cells(.all);

    // Limit to visible columns
    const visible_cols: usize = @intCast(wrapper.terminal.cols);
    const count = @min(cells.len, visible_cols);

    out_count.* = count;
    return cells.ptr;
}

/// Populate a CCellExt from a pin's internal cell.
///
/// This converts a ghostty internal cell to the FFI-safe CCellExt struct,
/// resolving colors and attributes.
///
/// # Arguments
/// * term - Terminal instance
/// * pin - Pin handle
/// * cell_index - Index into the cell array from ghostty_terminal_pin_cells()
/// * out_cell - Output CCellExt struct
///
/// Returns true on success, false if indices are invalid.
export fn ghostty_terminal_pin_populate_cell_ext(
    term: ?*const GhosttyTerminal,
    pin: ?*GhosttyPin,
    cell_index: usize,
    out_cell: *CCellExt,
) bool {
    if (term == null or pin == null) {
        out_cell.* = EMPTY_CELL_EXT;
        return false;
    }

    const wrapper: *const TerminalWrapper = @ptrCast(@alignCast(term));
    const pin_ptr: *ghostty_vt.PageList.Pin = @ptrCast(@alignCast(pin));

    const cells = pin_ptr.cells(.all);
    if (cell_index >= cells.len) {
        out_cell.* = EMPTY_CELL_EXT;
        return false;
    }

    const page_ptr: *const ghostty_vt.Page = &pin_ptr.node.data;
    populateCellExt(wrapper, page_ptr, &cells[cell_index], pin_ptr, cell_index, out_cell);
    return true;
}

/// Free a pin handle.
///
/// MUST be called for every pin returned by ghostty_terminal_pin_row().
export fn ghostty_terminal_pin_free(pin: ?*GhosttyPin) void {
    if (pin == null) return;

    const pin_ptr: *ghostty_vt.PageList.Pin = @ptrCast(@alignCast(pin));
    gpa.allocator().destroy(pin_ptr);
}

// ==============================================================================
// ROW ITERATOR (GHOSTTY PATTERN)
// ==============================================================================

/// Row iterator wrapper for FFI.
///
/// This wraps ghostty's PageList.RowIterator and tracks the current row index
/// for viewport-relative indexing (0 = top of viewport).
const RowIteratorWrapper = struct {
    iterator: ghostty_vt.PageList.RowIterator,
    current_row: u32,
};

/// Opaque row iterator handle for FFI.
///
/// This iterator provides efficient row-by-row traversal of the terminal
/// viewport, matching ghostty's rendering approach. Each row yields its
/// index and dirty status without allocating memory for row lists.
pub const GhosttyRowIterator = extern struct {
    _: [128]u8, // Enough space for RowIteratorWrapper
};

/// Create a row iterator for the terminal viewport.
///
/// Returns an iterator that yields rows from top to bottom. Each row
/// includes its index and dirty flag. The iterator handles page boundaries
/// automatically and supports zero-copy traversal.
///
/// **Usage pattern**:
/// ```
/// iter = ghostty_terminal_row_iterator_new(term);
/// while (ghostty_terminal_row_iterator_next(term, iter, &row, &is_dirty)) {
///     if (is_dirty) {
///         pin = ghostty_terminal_pin_row(term, row);
///         // render row...
///         ghostty_terminal_pin_free(pin);
///     }
/// }
/// ghostty_terminal_row_iterator_free(iter);
/// ```
///
/// Returns null if terminal is invalid or iterator creation fails.
export fn ghostty_terminal_row_iterator_new(
    term: ?*const GhosttyTerminal,
) ?*GhosttyRowIterator {
    if (term == null) return null;

    const wrapper: *const TerminalWrapper = @ptrCast(@alignCast(term));
    const screen = &wrapper.terminal.screen;

    // Allocate iterator wrapper on heap
    const iter_wrapper = gpa.allocator().create(RowIteratorWrapper) catch return null;

    // Initialize row iterator for viewport (top to bottom)
    iter_wrapper.* = .{
        .iterator = screen.pages.rowIterator(.left_up, .{ .viewport = .{} }, null),
        .current_row = 0,
    };

    return @ptrCast(@alignCast(iter_wrapper));
}

/// Get the next row from the iterator.
///
/// Advances the iterator and returns the next row's index and dirty status.
/// Returns false when iteration is complete.
///
/// **Arguments**:
/// - `term`: Terminal handle (for validation)
/// - `iter`: Iterator handle from ghostty_terminal_row_iterator_new()
/// - `out_row_index`: Receives the row index (0-based from top of viewport)
/// - `out_is_dirty`: Receives true if the row needs re-rendering
///
/// **Returns**: true if a row was yielded, false if iteration is complete
export fn ghostty_terminal_row_iterator_next(
    term: ?*const GhosttyTerminal,
    iter: ?*GhosttyRowIterator,
    out_row_index: *u32,
    out_is_dirty: *bool,
) bool {
    _ = term; // Not needed for iteration, but kept for API consistency

    if (iter == null) return false;

    const iter_wrapper: *RowIteratorWrapper = @ptrCast(@alignCast(iter));

    // Get next row from iterator
    if (iter_wrapper.iterator.next()) |row| {
        // Return current row index and dirty status
        out_row_index.* = iter_wrapper.current_row;
        out_is_dirty.* = row.isDirty();

        // Increment row counter for next iteration
        iter_wrapper.current_row += 1;

        return true;
    }

    return false;
}

/// Free a row iterator.
///
/// MUST be called for every iterator returned by ghostty_terminal_row_iterator_new().
export fn ghostty_terminal_row_iterator_free(iter: ?*GhosttyRowIterator) void {
    if (iter == null) return;

    const iter_wrapper: *RowIteratorWrapper = @ptrCast(@alignCast(iter));
    gpa.allocator().destroy(iter_wrapper);
}

// ==============================================================================
// TERMINAL MODES (CURSOR VISIBILITY, ETC.)
// ==============================================================================

/// Query a terminal mode state.
///
/// This allows checking if specific terminal modes are enabled, such as:
/// - Cursor visibility (DEC mode 25 - DECTCEM)
/// - Application cursor keys (DEC mode 1 - DECCKM)
/// - Bracketed paste (ANSI mode 2004)
/// - Etc.
///
/// **Arguments**:
/// - `term`: Terminal handle
/// - `mode_value`: Numeric mode identifier (e.g., 25 for cursor_visible)
/// - `ansi`: If true, use ANSI mode space; if false, use DEC private mode space
///
/// **Returns**: true if mode is enabled, false if disabled or mode doesn't exist
///
/// **Common modes**:
/// - DEC mode 25 (ansi=false): cursor_visible (DECTCEM)
/// - DEC mode 1 (ansi=false): application_cursor_keys (DECCKM)
/// - ANSI mode 2004 (ansi=true): bracketed_paste
///
/// **Example** (checking if cursor is visible):
/// ```
/// bool visible = ghostty_terminal_get_mode(term, 25, false);
/// ```
export fn ghostty_terminal_get_mode(
    term: ?*const GhosttyTerminal,
    mode_value: u16,
    ansi: bool,
) bool {
    if (term == null) return false;

    const wrapper: *const TerminalWrapper = @ptrCast(@alignCast(term));

    // Convert mode value to Mode enum
    const mode = modes.modeFromInt(mode_value, ansi) orelse return false;

    // Query mode state from terminal
    return wrapper.terminal.modes.get(mode);
}

/// Get the terminal's default background color.
///
/// This is the background color used for cells that don't have an explicit
/// background color set. Applications can change this with OSC 11 sequences.
///
/// **Returns**: RGB color as a CColor struct
///
/// **Example**:
/// ```
/// CColor bg = ghostty_terminal_get_default_background(term);
/// // Use bg.r, bg.g, bg.b for rendering
/// ```
export fn ghostty_terminal_get_default_background(
    term: ?*const GhosttyTerminal,
) CColor {
    // Default to black if terminal is invalid
    if (term == null) {
        return .{ .r = 0, .g = 0, .b = 0, .a = 255, .is_set = true };
    }

    const wrapper: *const TerminalWrapper = @ptrCast(@alignCast(term));

    return .{
        .r = wrapper.background_color[0],
        .g = wrapper.background_color[1],
        .b = wrapper.background_color[2],
        .a = 255,
        .is_set = true,
    };
}

/// Check if the viewport is at the bottom of the scrollback.
///
/// This is critical for cursor rendering - ghostty only renders the cursor
/// when the viewport is at the bottom. This prevents rendering the cursor
/// when scrolled back in history.
///
/// **Returns**: true if viewport is at bottom, false otherwise
///
/// **Example**:
/// ```
/// bool at_bottom = ghostty_terminal_is_viewport_at_bottom(term);
/// if (at_bottom && cursor_visible) {
///     // render cursor
/// }
/// ```
export fn ghostty_terminal_is_viewport_at_bottom(
    term: ?*const GhosttyTerminal,
) bool {
    if (term == null) return false;

    const wrapper: *const TerminalWrapper = @ptrCast(@alignCast(term));
    return wrapper.terminal.screen.viewportIsBottom();
}
