const std = @import("std");

pub fn main() !void {
    const stdin = std.io.getStdIn().reader();
    const stdout = std.io.getStdOut().writer();
    // const allocator = std.heap.page_allocator;

    while (true) {
        try stdout.print("> ", .{});
        try stdin.streamUntilDelimiter(stdout, '\n', null);

        try stdout.print("\n", .{});
    }
}
