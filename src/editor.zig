const std = @import("std");
const ab = @import("buffer.zig");
const c = @cImport({
    @cInclude("unistd.h");
    @cInclude("ctype.h");
    @cInclude("termios.h");
    @cInclude("sys/ioctl.h");
});

const STDIN_FILENO = std.os.linux.STDIN_FILENO;
const STDOUT_FILENO = std.os.linux.STDOUT_FILENO;
const stdin = std.io.getStdIn().reader();
const stdout = std.io.getStdOut().writer();
const VERSION = "0.0.1";

fn CTRL_KEY(k: u8) u8 {
    return k & 0x1f;
}

const EditorKey = enum(u32) { ARROW_LEFT = 1000,
    ARROW_RIGHT,
    ARROW_UP,
    ARROW_DOWN,
    PAGE_UP,
    PAGE_DOWN,
    HOME_KEY,
    END_KEY,
    ESC = 27,
    CTRL_Q = CTRL_KEY('q')
};

pub const Editor = struct {
    cx: i32,
    cy: i32,
    original_termios: c.termios,
    screen_rows: i32,
    screen_cols: i32,
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) !Editor {
        var editor = Editor{
            .cx = 0,
            .cy = 0,
            .original_termios = undefined,
            .screen_rows = 0,
            .screen_cols = 0,
            .allocator = allocator,
        };

        if (try getWindowSize(&editor.screen_rows, &editor.screen_cols, &editor) == -1) {
            return error.WindowSizeFailed;
        }

        return editor;
    }

    pub fn readKey(self: @This()) !EditorKey {
        _ = self;
        var buffer: [1]u8 = undefined;

        while (true) {
            _ = stdin.read(&buffer) catch |err| {
                std.debug.print("Could not read from stdin", .{});
                return err;
            };

            if (buffer[0] == '\x1b') {
                var sequence: [3]u8 = undefined;
                if (try stdin.read(sequence[0..1]) != 1) return EditorKey.ESC;
                if (try stdin.read(sequence[1..2]) != 1) return EditorKey.ESC;

                // check if its an arrow key or other special key
                if (sequence[0] == '[') {
                    if (sequence[1] >= '0' and sequence[1] <= '9') {
                        // handle sequences with numbers, e.g., Page Up, Page Down
                        if (try stdin.read(sequence[2..3]) != 1) return EditorKey.ESC;
                        if (sequence[2] == '~') {
                            switch (sequence[1]) {
                                '1' => return EditorKey.HOME_KEY,
                                '4' => return EditorKey.END_KEY,
                                '5' => return EditorKey.PAGE_UP,
                                '6' => return EditorKey.PAGE_DOWN,
                                '7' => return EditorKey.HOME_KEY,
                                '8' => return EditorKey.END_KEY,
                                else => return EditorKey.ESC,
                            }
                        }
                    } else {
                        // handle arrow keys and some other specific keys
                        switch (sequence[1]) {
                            'A' => return EditorKey.ARROW_UP,   // up
                            'B' => return EditorKey.ARROW_DOWN, // down
                            'C' => return EditorKey.ARROW_RIGHT,// right
                            'D' => return EditorKey.ARROW_LEFT, // left
                            'H' => return EditorKey.HOME_KEY,   // home
                            'F' => return EditorKey.END_KEY,    // end
                            else => return EditorKey.ESC,       // fallback to ESC
                        }
                    }
                    return EditorKey.ESC;
                } else if (sequence[0] == 'O') {
                    // handle 'O' sequences for HOME and END keys
                    switch (sequence[1]) {
                        'H' => return EditorKey.HOME_KEY,
                        'F' => return EditorKey.END_KEY,
                        else => return EditorKey.ESC,
                    }
                }
                return EditorKey.ESC;
            }

            switch (buffer[0]) {
                CTRL_KEY('q') => return EditorKey.CTRL_Q,
                else => return EditorKey.ESC, // default ESC for unknown values
            }
        }
    }


    pub fn enableRawMode(self: *@This()) !void {
        _ = c.tcgetattr(STDIN_FILENO, &self.original_termios);

        var raw: c.termios = self.original_termios;

        raw.c_lflag &= ~@as(c_uint, c.ECHO | c.ICANON | c.ISIG | c.IEXTEN);
        raw.c_oflag &= ~@as(c_uint, c.OPOST);
        raw.c_iflag &= ~@as(c_uint, c.IXON | c.ICRNL | c.BRKINT | c.INPCK | c.ISTRIP);
        raw.c_cflag |= @as(c_uint, c.CS8);
        raw.c_cc[c.VMIN] = 1;
        raw.c_cc[c.VTIME] = 0;

        const status_code = c.tcsetattr(STDIN_FILENO, c.TCSAFLUSH, &raw);

        if (status_code != 0) {
            return error.EnableRawModeFailed;
        }
    }

    pub fn disableRawMode(self: *@This()) !void {
        const status_code = c.tcsetattr(STDIN_FILENO, c.TCSAFLUSH, &self.original_termios);
        if (status_code != 0) {
            return error.DisableRawModeFailed;
        }
    }

    pub fn processKeyPressed(self: *@This()) !void {
        const key = try self.readKey();

        switch (key) {
            EditorKey.CTRL_Q => {
                try stdout.print("\x1b[2J", .{});
                try stdout.print("\x1b[H", .{});
                try self.disableRawMode();
                std.os.linux.exit(0);
            },
            EditorKey.ARROW_UP, EditorKey.ARROW_LEFT, EditorKey.ARROW_DOWN,  EditorKey.ARROW_RIGHT,
            EditorKey.PAGE_DOWN, EditorKey.PAGE_UP, EditorKey.HOME_KEY, EditorKey.END_KEY => {
                self.moveCursor(key);
            },
            else => {},
        }
    }

    pub fn refreshScreen(self: @This()) !void {
        var ab_buffer = ab.AppendBuffer.init(self.allocator);
        defer ab_buffer.deinit();

        // try buffer.append("\x1b[2J");  // clear screen
        try ab_buffer.append("\x1b[H"); // move cursor to the top

        try self.drawRows(&ab_buffer);

        var cursor_position: [32]u8 = undefined;
        const cursor_str = try std.fmt.bufPrint(&cursor_position, "\x1b[{d};{d}H", .{ self.cy + 1, self.cx + 1 });
        try ab_buffer.append(cursor_position[0..cursor_str.len]);

        // try ab_buffer.append("\x1b[H"); // move cursor back to top
        try ab_buffer.append("\x1b[?25h"); // show cursor

        try ab_buffer.writeTo(stdout); // write to stdout
    }

    pub fn drawRows(self: @This(), buffer: *ab.AppendBuffer) !void {
        var y: usize = 0;

        while (y < self.screen_rows) : (y += 1) {
            if (y == @divTrunc(self.screen_rows, 3)) {
                var welcome: [80]u8 = undefined;
                const welcome_text = try std.fmt.bufPrint(&welcome, "The editor -- version {s}", .{VERSION});
                const cols: usize = @intCast(self.screen_cols);

                const welcome_len = if (welcome_text.len > cols) cols else welcome_text.len;

                // center welcome message
                var padding: usize = (cols - welcome_len) / 2;
                if (padding > 0) {
                    try buffer.append("~");
                    padding -= 1;
                }

                while (padding != 0) : (padding -= 1) {
                    try buffer.append(" ");
                }

                // add welcome message
                try buffer.append(welcome[0..welcome_len]);
            } else {
                try buffer.append("~");
            }

            try buffer.append("\x1b[K"); // clean line

            if (y < self.screen_rows - 1) {
                try buffer.append("\r\n"); // add line if not the last
            }
        }
    }

    pub fn getCursorPosition(self: *@This(), rows: *i32, cols: *i32) !i32 {
        _ = self;
        var buffer: [32]u8 = undefined;
        var i: usize = 0;

        if (try stdout.write("\x1b[6n") != 4) return -1;

        while (i < buffer.len - 1) : (i += 1) {
            if (try stdin.read(buffer[i .. i + 1]) != 1) break;
            if (buffer[i] == 'R') break;
        }

        buffer[i] = '0';

        // make sure response starts with '\x1b' e '['
        if (buffer[0] != '\x1b' or buffer[1] != '[') {
            return -1;
        }

        const semicolon_index = std.mem.indexOf(u8, buffer[2..i], ";\u{00}") orelse return -1;

        const row_str = buffer[2..semicolon_index];
        const col_str = buffer[semicolon_index + 1 .. i];

        // convert string to int
        const row = try std.fmt.parseInt(i32, row_str, 10);
        const col = try std.fmt.parseInt(i32, col_str, 10);

        rows.* = row;
        cols.* = col;

        return 0;
    }

    pub fn moveCursor(self: *@This(), key: EditorKey) void {
        switch (key) {
            EditorKey.ARROW_LEFT => {
                if (self.cx != 0) {
                    self.cx -= 1;
                }
            },
            EditorKey.ARROW_RIGHT => {
                if (self.cx != self.screen_cols - 1) {
                    self.cx += 1;
                }
            },
            EditorKey.ARROW_UP => {
                if (self.cy != 0) {
                    self.cy -= 1;
                }
            },
            EditorKey.ARROW_DOWN => {
                if (self.cy != self.screen_rows - 1) {
                    self.cy += 1;
                }
            },
            EditorKey.PAGE_DOWN => {
                // move cursor by one screen height (screen_rows)
                if (self.cy > 0) {
                    self.cy -= self.screen_rows;
                    if (self.cy < 0) {
                        self.cy = 0;
                    }
                }
            },
            EditorKey.PAGE_UP => {
                // move cursor down by one screen height (screen_rows)
                if (self.cy < self.screen_rows - 1) {
                    self.cy += self.screen_rows;
                    if (self.cy >= self.screen_rows) {
                        self.cy = self.screen_rows - 1;
                    }
                }
            },
            EditorKey.HOME_KEY => {
                self.cx = 0;
            },
            EditorKey.END_KEY => {
                self.cx = self.screen_cols - 1;
            },
            else => {},
        }
    }
};

pub fn getWindowSize(rows: *i32, cols: *i32, editor: *Editor) !i32 {
    var ws: c.winsize = undefined;

    // try to get window size with ioctl
    const result = c.ioctl(STDOUT_FILENO, c.TIOCGWINSZ, &ws);

    if (result == -1 or ws.ws_col == 0) {
        // ioctl may fail
        if (try stdout.write("\x1b[999C\x1b[999B") != 12) return -1;

        // read key to make sure the cursor has repositioned
        return editor.getCursorPosition(rows, cols);
    } else {
        cols.* = ws.ws_col;
        rows.* = ws.ws_row;
        return 0;
    }
}
