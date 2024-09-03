const std = @import("std");
const c = @cImport({
    @cInclude("termio.h");
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
        if (bytes_read == 0) break;

        if (buffer[0] == 'q') {
            break;
        }

        try stdout.print("{c}", .{buffer[0]});
    }
}

fn enable_raw_mode() !void {
    _ = c.tcgetattr(STDIN_FILENO, &original_termios);

    var raw: c.termios = original_termios;

    raw.c_lflag &= ~@as(c_uint, c.ECHO);

    _ = c.tcsetattr(STDIN_FILENO, c.TCSAFLUSH, &raw);
}

fn disable_raw_mode() void {
    _ = c.tcsetattr(STDIN_FILENO, c.TCSAFLUSH, &original_termios);
}
