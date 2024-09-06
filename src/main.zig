const std = @import("std");
const editor = @import("editor.zig");

pub fn main() !void {
    const allocator = std.heap.page_allocator;
    var args = std.process.args();
    _ = args.skip();

    const file_path = args.next();
    var e = try editor.Editor.init(allocator);
    try e.enableRawMode();

    if (file_path != null) {
        try e.open(file_path.?);
    }

    while (true) {
        try e.refreshScreen();
        try e.processKeyPressed();
    }

    try e.disableRawMode();
}
