const std = @import("std");
const editor = @import("editor.zig");

pub fn main() !void {
    var e = try editor.Editor.init();
    try e.enableRawMode();

    while (true) {
        try e.refreshScreen();
        try e.processKeyPressed();
    }

    try e.disableRawMode();
}
