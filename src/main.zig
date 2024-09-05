const std = @import("std");
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


fn CTRL_KEY(k: u8) u8 {
    return k & 0x1f;
}

const Editor = struct {
    original_termios: c.termios,
    screen_rows: i32,
    screen_cols: i32,

    pub fn init() !Editor {
        var editor = Editor{
            .original_termios = undefined,
            .screen_rows = 0,
            .screen_cols = 0,
        };

        if (try getWindowSize(&editor.screen_rows, &editor.screen_cols, &editor) == -1) {
            return error.WindowSizeFailed;
        }

        return editor;
    }

    pub fn readKey(self: @This()) !u8 {
        _ = self;
        var buffer: [1]u8 = undefined;

        while (true) {
            const bytes_read = stdin.read(&buffer) catch |err| {
                std.debug.print("Could not read from stdin", .{});
                return err;
            };

            if (bytes_read == 1) {
                return buffer[0];
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
            CTRL_KEY('q') => {
                try stdout.print("\x1b[2J", .{});
                try stdout.print("\x1b[H", .{});
                try self.disableRawMode();
                std.os.linux.exit(0);
            },
            else => {}
        }
    }

    pub fn refreshScreen(self: @This()) !void {
        try stdout.print("\x1b[2J", .{});
        try stdout.print("\x1b[H", .{});

        try self.editorDrawRows();
        try stdout.print("\x1b[H", .{});
    }

    pub fn editorDrawRows(self: @This()) !void {
        var y: usize = 0;
        while (y < self.screen_rows) : (y += 1) {
            try stdout.print("~\r\n", .{});
        }
    }

    pub fn getCursorPosition(self: *@This(), rows: *i32, cols: *i32) !i32 {
        _ = self;
        var buffer: [32]u8 = undefined;
        var i: usize = 0;

        if (try stdout.write("\x1b[6n") != 4) return -1;

        while (i < buffer.len - 1) : (i += 1) {
            if (try stdin.read(buffer[i..i+1]) != 1) break;
            if (buffer[i] == 'R') break;
        }

        buffer[i] = '0';

        // make sure response starts with '\x1b' e '['
        if (buffer[0] != '\x1b' or buffer[1] != '[') {
            return -1;
        }

        const semicolon_index = std.mem.indexOf(u8, buffer[2..i], ";\u{00}") orelse return -1;

        const row_str = buffer[2..semicolon_index];
        const col_str = buffer[semicolon_index+1..i];

        // convert string to int
        const row = try std.fmt.parseInt(i32, row_str, 10);
        const col = try std.fmt.parseInt(i32, col_str, 10);

        rows.* = row;
        cols.* = col;

        return 0;
    }
};

pub fn getWindowSize(rows: *i32, cols: *i32, editor: *Editor) !i32 {
    var ws: c.winsize = undefined;

    // try to get window size with ioctl
    const result = c.ioctl(STDOUT_FILENO, c.TIOCGWINSZ, &ws);

    if (result == -1 or ws.ws_col == 0) {
        // ioctl may fail
        if (try stdout.write("\x1b[999C\x1b[999B") != 12)  return -1;

        // read key to make sure the cursor has repositioned
        return editor.getCursorPosition(rows, cols);
    } else {
        cols.* = ws.ws_col;
        rows.* = ws.ws_row;
        return 0;
    }
}

pub fn main() !void {
    var editor = try Editor.init();
    try editor.enableRawMode();

    while (true) {
        try editor.refreshScreen();
        try editor.processKeyPressed();
    }

    try editor.disableRawMode();
}
