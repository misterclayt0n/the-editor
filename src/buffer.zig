const std = @import("std");

pub const AppendBuffer = struct {
    buffer: std.ArrayList(u8),

    pub fn init(allocator: *std.mem.Allocator) !AppendBuffer {
        return AppendBuffer{
            .buffer = try std.ArrayList(u8).init(allocator),
        };
    }
};
