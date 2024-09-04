const std = @import("std");
const c = @cImport({
    @cInclude("unistd.h");
    @cInclude("ctype.h");
    @cInclude("termios.h");
});

var original_termios: c.termios = undefined;
const STDIN_FILENO = std.os.linux.STDIN_FILENO;

pub fn main() !void {
    try enable_raw_mode();
    defer disable_raw_mode();

    const stdin = std.io.getStdIn().reader();
    const stdout = std.io.getStdOut().writer();
    var buffer: [1]u8 = undefined;

    while (true) {
        const bytes_read = try stdin.read(&buffer);
        if (bytes_read == 0)  break;

        const buffer_val = buffer[0];

        if (buffer[0] == 'q') {
            break;
        }

        if (c.iscntrl(buffer_val) == 0) {
            try stdout.print("{d}\r\n", .{buffer_val});
        } else {
            try stdout.print("{d} ('{c}')\r\n", .{buffer_val, buffer_val});
        }
    }
}

fn enable_raw_mode() !void {
    _ = c.tcgetattr(STDIN_FILENO, &original_termios);

    var raw: c.termios = original_termios;

    raw.c_lflag &= ~@as(c_uint, c.ECHO|c.ICANON|c.ISIG|c.IEXTEN);
    raw.c_oflag &= ~@as(c_uint, c.OPOST);
    raw.c_iflag &= ~@as(c_uint, c.IXON|c.ICRNL|c.BRKINT|c.INPCK|c.ISTRIP);
    raw.c_oflag |= @as(c_uint, c.CS8);

    _ = c.tcsetattr(STDIN_FILENO, c.TCSAFLUSH, &raw);
}

fn disable_raw_mode() void {
    _ = c.tcsetattr(STDIN_FILENO, c.TCSAFLUSH, &original_termios);
}
