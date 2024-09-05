const std = @import("std");
const stdout = std.io.getStdOut().writer();

pub const AppendBuffer = struct {
    buffer: std.ArrayList(u8),

    /// initialize append buffer
    pub fn init(allocator: *std.mem.Allocator) !AppendBuffer {
        return AppendBuffer{
            .buffer = try std.ArrayList(u8).init(allocator),
        };
    }

    /// add a string to buffer
    pub fn append(self: *@This(), data: []const u8) !void {
        try self.buffer.appendSlice(data);
    }

    /// free append buffer
    pub fn free(self: *@This()) void {
        try self.free();
    }
};
