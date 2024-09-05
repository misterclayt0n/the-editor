const std = @import("std");
const editor = @import("editor.zig");

pub fn main() !void {
    const allocator = std.heap.page_allocator;
    var e = try editor.Editor.init(allocator);
    try e.enableRawMode();

    while (true) {
        try e.refreshScreen();
        try e.processKeyPressed();
    }

    try e.disableRawMode();
}
