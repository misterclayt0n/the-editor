const std = @import("std");
const c = @cImport({
    @cInclude("unistd.h");
    @cInclude("ctype.h");
    @cInclude("termios.h");
});

var original_termios: c.termios = undefined;
const STDIN_FILENO = std.os.linux.STDIN_FILENO;
const stdin = std.io.getStdIn().reader();
const stdout = std.io.getStdOut().writer();


fn CTRL_KEY(k: u8) u8 {
    return k & 0x1f;
}

const Editor = struct {
    pub fn init() Editor {
        return Editor{};
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

    pub fn processKeyPressed(self: @This()) !void {
        const key = try self.readKey();

        switch (key) {
            CTRL_KEY('q') => {
                try disableRawMode();
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
        _ = self;
        var y: usize = 0;
        while (y < 24) : (y += 1) {
            try stdout.print("~\r\n", .{});
        }
    }
};

pub fn main() !void {
    try enableRawMode();

    var editor = Editor.init();

    while (true) {
        try editor.refreshScreen();
        try editor.processKeyPressed();
    }

    try disableRawMode();
}

fn enableRawMode() !void {
    _ = c.tcgetattr(STDIN_FILENO, &original_termios);

    var raw: c.termios = original_termios;

    raw.c_lflag &= ~@as(c_uint, c.ECHO|c.ICANON|c.ISIG|c.IEXTEN);
    raw.c_oflag &= ~@as(c_uint, c.OPOST);
    raw.c_iflag &= ~@as(c_uint, c.IXON|c.ICRNL|c.BRKINT|c.INPCK|c.ISTRIP);
    raw.c_oflag |= @as(c_uint, c.CS8);
    raw.c_cc[c.VMIN] = 1;
    raw.c_cc[c.VTIME] = 0;

    const status_code = c.tcsetattr(STDIN_FILENO, c.TCSAFLUSH, &raw);

    if (status_code != 0) {
        return error.EnableRawModeFailed;
    }
}

fn disableRawMode() !void {
    const status_code = c.tcsetattr(STDIN_FILENO, c.TCSAFLUSH, &original_termios);

    if (status_code != 0) {
        return error.DisableRawModeFailed;
    }
}
